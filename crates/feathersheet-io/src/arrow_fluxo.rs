//! Leitor em fluxo do bloco Arrow IPC (formato de arquivo) de uma aba do
//! `.fshz`: lê mensagem a mensagem direto do gzip e põe cada buffer na sua
//! coluna, sem guardar o bloco nem as tabelas Arrow.
//!
//! Antes (até 17/10/2026) a abertura guardava o bloco inteiro descomprimido,
//! decodificava em `RecordBatch` (outra cópia) e só então copiava cada coluna
//! para um vetor (a terceira): o `2026_03.fshz`, 13,7 milhões de linhas que
//! ocupam 4,4 GB abertas, batia 13–14 GB de pico. Aqui o pico é o das colunas
//! mais um buffer de cada vez — inclusive nos arquivos da v0.1.0, que gravavam
//! a aba num lote só.
//!
//! Só o que o gravador do `.fshz` produz: campos `Float64` e `Int32`, sem
//! compressão de buffer nem dicionário Arrow. Outra coisa é recusada.
use std::collections::HashMap;
use std::io::Read;

use arrow::ipc::{root_as_message, MessageHeader, Type};

/// Uma coluna lida: valores (posição sem dado fica 0) e máscara de presença
/// (1 bit por linha, menos significativo primeiro).
pub(crate) enum ColunaArrow {
    Float(Vec<f64>, Vec<u8>),
    Int(Vec<i32>, Vec<u8>),
}

/// Lê `n` bytes sem reservar o tamanho declarado de antemão: um arquivo
/// forjado pode dizer gigabytes e trazer meia dúzia de bytes.
fn ler_exato<R: Read>(r: &mut std::io::Take<R>, n: usize) -> Result<Vec<u8>, String> {
    if n as u64 > r.limit() {
        return Err("Bloco Arrow truncado".into());
    }
    let mut v = Vec::new();
    r.by_ref().take(n as u64).read_to_end(&mut v).map_err(|e| format!("Bloco Arrow truncado: {e}"))?;
    if v.len() != n {
        return Err("Bloco Arrow truncado".into());
    }
    Ok(v)
}

fn pular<R: Read>(r: &mut R, n: u64) -> Result<(), String> {
    let pulados = std::io::copy(&mut r.by_ref().take(n), &mut std::io::sink()).map_err(|e| e.to_string())?;
    if pulados != n {
        return Err("Bloco Arrow truncado".into());
    }
    Ok(())
}

/// Lê os `len` bytes do bloco Arrow da aba em `origem` e devolve as colunas
/// pelo nome do campo, cada uma com `linhas` posições.
pub(crate) fn ler_colunas<R: Read>(origem: R, len: usize, linhas: u32) -> Result<HashMap<String, ColunaArrow>, String> {
    if linhas > feathersheet_core::MAX_ROWS {
        return Err(format!("Aba com {linhas} linhas passa do limite da planilha"));
    }
    let linhas = linhas as usize;
    let mut r = origem.take(len as u64);
    let magico = ler_exato(&mut r, 6)?;
    if magico != b"ARROW1" {
        return Err("Bloco Arrow sem assinatura".into());
    }
    // Depois da assinatura vêm zeros até o alinhamento (8 ou 64 bytes,
    // conforme a versão do arrow que gravou); a primeira mensagem começa no
    // primeiro byte diferente de zero (o 0xFF da continuação).
    let mut primeiro = [0u8; 1];
    let mut zeros = 0;
    loop {
        r.read_exact(&mut primeiro).map_err(|e| format!("Bloco Arrow truncado: {e}"))?;
        if primeiro[0] != 0 { break; }
        zeros += 1;
        if zeros > 64 { return Err("Bloco Arrow sem mensagens".into()); }
    }
    let mut adiantado = Some(primeiro[0]);
    let mut nomes: Vec<String> = Vec::new();
    let mut colunas: Vec<ColunaArrow> = Vec::new();
    let mut linha0 = 0usize;
    loop {
        let mut quatro = [0u8; 4];
        match adiantado.take() {
            Some(b) => {
                quatro[0] = b;
                r.read_exact(&mut quatro[1..]).map_err(|e| format!("Bloco Arrow truncado: {e}"))?;
            }
            None => r.read_exact(&mut quatro).map_err(|e| format!("Bloco Arrow truncado: {e}"))?,
        }
        let mut tamanho = i32::from_le_bytes(quatro);
        if tamanho == -1 {
            r.read_exact(&mut quatro).map_err(|e| format!("Bloco Arrow truncado: {e}"))?;
            tamanho = i32::from_le_bytes(quatro);
        }
        // Fim do fluxo; depois dele vem só o rodapé do formato de arquivo.
        if tamanho <= 0 {
            break;
        }
        let meta = ler_exato(&mut r, tamanho as usize)?;
        let msg = root_as_message(&meta).map_err(|e| format!("Mensagem Arrow inválida: {e}"))?;
        let corpo = u64::try_from(msg.bodyLength()).map_err(|_| "Corpo Arrow inválido")?;
        match msg.header_type() {
            MessageHeader::Schema => {
                let esquema = msg.header_as_schema().ok_or("Esquema Arrow inválido")?;
                for campo in esquema.fields().into_iter().flatten() {
                    let nome = campo.name().unwrap_or_default().to_owned();
                    let mascara = vec![0u8; linhas.div_ceil(8)];
                    colunas.push(match campo.type_type() {
                        Type::FloatingPoint => ColunaArrow::Float(vec![0.0; linhas], mascara),
                        Type::Int if campo.type_as_int().is_some_and(|i| i.bitWidth() == 32) => ColunaArrow::Int(vec![0; linhas], mascara),
                        outro => return Err(format!("Tipo Arrow {outro:?} no campo \"{nome}\" não é do .fshz")),
                    });
                    nomes.push(nome);
                }
                pular(&mut r, corpo)?;
            }
            MessageHeader::RecordBatch => {
                let lote = msg.header_as_record_batch().ok_or("Lote Arrow inválido")?;
                if lote.compression().is_some() {
                    return Err("Lote Arrow comprimido não é do .fshz".into());
                }
                let n = usize::try_from(lote.length()).map_err(|_| "Lote Arrow inválido")?;
                let nos = lote.nodes().ok_or("Lote Arrow sem campos")?;
                let buffers = lote.buffers().ok_or("Lote Arrow sem buffers")?;
                if nos.len() != colunas.len() || buffers.len() != 2 * colunas.len() {
                    return Err("Lote Arrow não bate com o esquema".into());
                }
                let mut pos = 0u64;
                for (i, coluna) in colunas.iter_mut().enumerate() {
                    let (validade, valores) = (buffers.get(2 * i), buffers.get(2 * i + 1));
                    let nulos = nos.get(i).null_count();
                    // A máscara do lote (vazia = todas presentes).
                    let inicio = u64::try_from(validade.offset()).map_err(|_| "Buffer Arrow inválido")?;
                    pular(&mut r, inicio.checked_sub(pos).ok_or("Buffers Arrow fora de ordem")?)?;
                    let bits = ler_exato(&mut r, usize::try_from(validade.length()).map_err(|_| "Buffer Arrow inválido")?)?;
                    pos = inicio + bits.len() as u64;
                    if bits.is_empty() && nulos > 0 {
                        return Err("Lote Arrow com nulos sem máscara".into());
                    }
                    let presente = |k: usize| bits.is_empty() || bits.get(k >> 3).is_some_and(|b| b & (1 << (k & 7)) != 0);
                    let inicio = u64::try_from(valores.offset()).map_err(|_| "Buffer Arrow inválido")?;
                    pular(&mut r, inicio.checked_sub(pos).ok_or("Buffers Arrow fora de ordem")?)?;
                    let largura = match coluna { ColunaArrow::Float(..) => 8, ColunaArrow::Int(..) => 4 };
                    let total = usize::try_from(valores.length()).map_err(|_| "Buffer Arrow inválido")?;
                    let tamanho_do_lote = n.checked_mul(largura).ok_or("Lote Arrow inválido")?;
                    if total < tamanho_do_lote {
                        return Err("Buffer Arrow menor que o lote".into());
                    }
                    // Os valores em pedaços de 1 MB, direto para a coluna.
                    let mut pedaco = vec![0u8; (1 << 20) / largura * largura];
                    let mut k = 0usize;
                    let mut lidos = 0usize;
                    while lidos < tamanho_do_lote {
                        let quer = pedaco.len().min(tamanho_do_lote - lidos);
                        r.read_exact(&mut pedaco[..quer]).map_err(|e| format!("Bloco Arrow truncado: {e}"))?;
                        for bytes in pedaco[..quer].chunks_exact(largura) {
                            let linha = linha0 + k;
                            if linha < linhas && presente(k) {
                                match coluna {
                                    ColunaArrow::Float(v, m) => {
                                        v[linha] = f64::from_le_bytes(bytes.try_into().expect("8 bytes"));
                                        m[linha >> 3] |= 1 << (linha & 7);
                                    }
                                    ColunaArrow::Int(v, m) => {
                                        v[linha] = i32::from_le_bytes(bytes.try_into().expect("4 bytes"));
                                        m[linha >> 3] |= 1 << (linha & 7);
                                    }
                                }
                            }
                            k += 1;
                        }
                        lidos += quer;
                    }
                    pular(&mut r, (total - lidos) as u64)?;
                    pos = inicio + total as u64;
                }
                pular(&mut r, corpo.checked_sub(pos).ok_or("Corpo Arrow menor que os buffers")?)?;
                linha0 += n;
            }
            _ => pular(&mut r, corpo)?,
        }
    }
    // O rodapé do formato de arquivo: não serve para ler em sequência.
    let resto = r.limit();
    pular(&mut r, resto)?;
    Ok(nomes.into_iter().zip(colunas).collect())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn tamanhos_forjados_dao_erro_sem_reservar_memoria() {
        // Linhas acima do teto da planilha: antes reservava 32 GB por campo.
        assert!(ler_colunas(&b"ARROW1"[..], 6, u32::MAX).is_err());
        // Mensagem que diz 2 GB num bloco de 14 bytes.
        let mut b = b"ARROW1\0\0".to_vec();
        b.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]);
        assert!(ler_colunas(b.as_slice(), usize::MAX, 10).is_err());
    }
    use arrow::array::{ArrayRef, Float64Array, Int32Array, RecordBatch};
    use arrow::datatypes::{DataType, Field, Schema};
    use arrow::ipc::writer::FileWriter;
    use std::sync::Arc;

    /// Dois lotes, com nulos, como o gravador em fatias produz.
    #[test]
    fn le_lotes_com_nulos_igual_ao_leitor_do_arrow() {
        let esquema = Arc::new(Schema::new(vec![
            Field::new("n0", DataType::Float64, true),
            Field::new("t1", DataType::Int32, true),
        ]));
        let mut bytes = Vec::new();
        {
            let mut w = FileWriter::try_new(&mut bytes, &esquema).unwrap();
            for base in [0i32, 5] {
                let n: ArrayRef = Arc::new(Float64Array::from_iter((0..5).map(|k| (k != 2).then_some((base + k) as f64))));
                let t: ArrayRef = Arc::new(Int32Array::from_iter((0..5).map(|k| Some(base + k * 10))));
                w.write(&RecordBatch::try_new(esquema.clone(), vec![n, t]).unwrap()).unwrap();
            }
            w.finish().unwrap();
        }
        let mut colunas = ler_colunas(bytes.as_slice(), bytes.len(), 10).unwrap();
        let Some(ColunaArrow::Float(v, m)) = colunas.remove("n0") else { panic!("n0") };
        assert_eq!(v, [0., 1., 0., 3., 4., 5., 6., 0., 8., 9.]);
        // Linhas 2 e 7 vazias.
        assert_eq!(m, [0b0111_1011, 0b0000_0011]);
        let Some(ColunaArrow::Int(t, _)) = colunas.remove("t1") else { panic!("t1") };
        assert_eq!(t, [0, 10, 20, 30, 40, 5, 15, 25, 35, 45]);
    }
}
