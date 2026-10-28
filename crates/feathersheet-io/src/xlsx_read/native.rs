//! Importação XLSX para o motor colunar sem mapa de células nem JSON intermediário.
//! XML de cada worksheet e sharedStrings são consumidos direto das entradas ZIP.
use super::{anexar_elementos, em_paralelo, entry_text, sheet, styles, xml};
use super::refs::{parse_cell_ref, MAX_COLS};
use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::sheet::{carga::{CelulaPreparada, LoteBruto, LotePreparado, Padroes, Sobra}, Sheet};
use feathersheet_core::planilha::workbook::Workbook;
use std::collections::HashMap;
use std::io::{BufReader, Cursor};
use zip::{result::ZipError, ZipArchive};

// Não há teto de células: um teto por contagem cortava planilhas legítimas
// (1 milhão de linhas × 13 colunas passa de 10 milhões). Quem para a leitura
// é a vigia de memória da suíte (`feather_io::memoria`), pelo limite
// de RAM escolhido nas Configurações. Os tetos abaixo protegem só contra
// XLSX malicioso (bomba de ZIP, formatação vazia sem fim).
const MAX_CELULAS_VAZIAS_FORMATADAS: usize = 300_000;
/// O mesmo teto do gravador (`zip_write`: 4 GB por parte, ZIP sem ZIP64):
/// o que o app salva ele abre de volta (#61). A aba é lida em fluxo.
const MAX_WORKSHEET_XML: u64 = 4 * 1024 * 1024 * 1024;
/// O arquivo compactado inteiro, também o teto do ZIP sem ZIP64.
const LIMITE_XLSX_NATIVO: u64 = 4 * 1024 * 1024 * 1024;
const MAX_SHARED_STRINGS_XML: u64 = 1024 * 1024 * 1024;
const MAX_TEXTO_COMPARTILHADO: usize = 256 * 1024 * 1024;

fn strings_compartilhadas(archive: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>) -> Result<Vec<String>, String> {
    let arquivo = match archive.by_name("xl/sharedStrings.xml") {
        Ok(arquivo) => arquivo,
        Err(ZipError::FileNotFound) => return Ok(Vec::new()),
        Err(e) => return Err(format!("XLSX: erro no sharedStrings.xml: {e}")),
    };
    if arquivo.size() > MAX_SHARED_STRINGS_XML {
        return Err("XLSX: sharedStrings.xml excede 1 GiB descomprimido".into());
    }
    let mut strings = Vec::new();
    let mut tamanho = 0usize;
    xml::parse_xml_items_reader_result(BufReader::new(arquivo), "sst", "si", &mut |si| {
        let texto = if let Some(direto) = si.child("t") {
            direto.text.clone()
        } else {
            si.children_named("r")
                .filter_map(|r| r.child("t").map(|t| t.text.as_str()))
                .collect::<String>()
        };
        // `<si/>` vazio também ocupa um `String` na lista: conta o cabeçalho junto.
        tamanho = tamanho.saturating_add(texto.len() + std::mem::size_of::<String>());
        if tamanho > MAX_TEXTO_COMPARTILHADO {
            return Err("XLSX: strings compartilhadas excedem 256 MiB de texto; importação interrompida para proteger a RAM".into());
        }
        strings.push(texto);
        Ok(())
    })?;
    Ok(strings)
}

fn estilos_parciais(xfs: &[styles::XfStyle]) -> Vec<Option<EstiloParcial>> {
    // Converte cada XF uma vez: células com mesmo estilo não passam por serde_json.
    let normais: Vec<Option<EstiloParcial>> = xfs.iter().map(|xf| {
        if !xf.style.has_any() { return None; }
        let json = serde_json::to_value(&xf.style).ok()?;
        crate::para_workbook::estilo_parcial_de_json(&json)
    }).collect();
    // Segunda metade: o mesmo estilo para célula numa linha com estilo
    // próprio (`<row s=".." customFormat="1">`). No Excel o `s` da célula é
    // completo e cobre o da linha; aqui as camadas se mesclam, então o que o
    // XF não tem (fundo, borda, cor) entra como "apagar" ([`Campo::Nulo`]).
    let sobre: Vec<Option<EstiloParcial>> = normais.iter().map(|e| {
        let mut e = e.clone().unwrap_or_default();
        for chave in CHAVES_QUE_COBREM_A_LINHA {
            e.entry((*chave).into()).or_insert(Campo::Nulo);
        }
        Some(e)
    }).collect();
    normais.into_iter().chain(sobre).collect()
}

/// O que um estilo de linha costuma trazer e o da célula precisa cobrir.
const CHAVES_QUE_COBREM_A_LINHA: &[&str] = &["bg", "borders", "color"];

/// Células por lote entre os estágios da leitura. Com 32.768 uma aba de
/// 48 mil células era só 2 lotes e os estágios quase não se sobrepunham.
const LOTE_CARGA: usize = 8_192;

/// Abas lidas ao mesmo tempo; os núcleos se dividem entre elas. Mais que
/// isso só aumentava a RAM (cada aba tem os seus lotes em trânsito).
const ABAS_SIMULTANEAS: usize = 4;

/// Marca, no texto bruto do lote, o que a leitura deixou para converter
/// aqui, em paralelo: fórmula a traduzir do Excel e número a formatar pelo
/// estilo. Sem marca, o texto já é o final.
/// Soma das imagens "Colocar na célula" de uma aba (ver `bytes_de_imagem`).
const MAX_BYTES_IMAGEM_NA_ABA: usize = 512 << 20;

const MARCA_FORMULA: char = '\u{1}';
const MARCA_NUMERO: char = '\u{2}';
/// Texto do Excel (`t="s"`, `str`, `inlineStr`): entra como texto, sem passar
/// pelo analisador do que se digita ("00123" não vira 123, "=x" não vira fórmula).
const MARCA_TEXTO: char = '\u{3}';

/// O texto final da célula a partir do bruto que a leitura guardou.
fn converter<'a>(bruto: &'a str, xf: usize, xfs: &[styles::XfStyle]) -> std::borrow::Cow<'a, str> {
    if let Some(f) = bruto.strip_prefix(MARCA_FORMULA) {
        return sheet::formula_da_celula(f).into();
    }
    if let Some(t) = bruto.strip_prefix(MARCA_TEXTO) {
        return t.into();
    }
    if let Some(n) = bruto.strip_prefix(MARCA_NUMERO) {
        // Na linha com estilo próprio o índice vem somado de `xfs.len()`.
        let xf = if xf >= xfs.len() && !xfs.is_empty() { xf - xfs.len() } else { xf };
        return sheet::numero_da_celula(n.trim(), xfs.get(xf)).into();
    }
    bruto.into()
}

/// A fórmula do Excel com as referências de célula trocadas pela posição
/// relativa à célula (`row`, `col`; com `$`, a absoluta): duas células com a
/// mesma chave têm a mesma fórmula, só deslocada. Devolve `false` (sem
/// chave) quando não reconheceu nada como referência.
///
/// Referência é `$?LETRAS$?DÍGITOS` (até XFD e 1.048.576… o teto da aba)
/// fora de aspas, sem letra, dígito, `_`, `.` ou `\` antes e sem letra,
/// dígito, `_`, `(`, `!`, `.` ou `[` depois — assim `LOG10(`, `Q1!A1` (o `Q1`
/// é nome de aba) e nomes definidos não viram referência. Se algo fosse lido
/// como referência sem ser, as duas células teriam chaves diferentes (o
/// deslocamento não bate) e cada uma seguiria pelo caminho completo.
fn chave_relativa(f: &str, row: u32, col: u32, saida: &mut String) -> bool {
    let b = f.as_bytes();
    let antes_ok = |i: usize| i == 0 || !(b[i - 1].is_ascii_alphanumeric() || matches!(b[i - 1], b'_' | b'.' | b'\\' | b'$'));
    let mut achou = false;
    let mut i = 0;
    let mut trecho = 0;
    while i < b.len() {
        match b[i] {
            // Texto entre aspas e nome de aba entre apóstrofos: literal.
            q @ (b'"' | b'\'') => {
                i += 1;
                while i < b.len() {
                    if b[i] == q {
                        if b.get(i + 1) == Some(&q) { i += 2; continue; }
                        break;
                    }
                    i += 1;
                }
                i += 1;
            }
            c if (c == b'$' || c.is_ascii_uppercase()) && antes_ok(i) => {
                let inicio = i;
                let mut j = i;
                let col_fixa = b[j] == b'$';
                if col_fixa { j += 1; }
                let letras = j;
                while j < b.len() && b[j].is_ascii_uppercase() && j - letras < 3 { j += 1; }
                let n_letras = j - letras;
                let lin_fixa = j < b.len() && b[j] == b'$';
                if lin_fixa { j += 1; }
                let digitos = j;
                while j < b.len() && b[j].is_ascii_digit() && j - digitos < 8 { j += 1; }
                let n_digitos = j - digitos;
                let depois_ok = j >= b.len() || !(b[j].is_ascii_alphanumeric() || matches!(b[j], b'_' | b'(' | b'!' | b'.' | b'['));
                let coluna = b[letras..letras + n_letras].iter().fold(0u32, |a, &l| a * 26 + u32::from(l - b'A' + 1));
                let linha: u32 = f[digitos..j].parse().unwrap_or(0);
                if n_letras > 0 && n_digitos > 0 && depois_ok && coluna >= 1 && coluna <= 16_384 && linha >= 1
                    && !(n_digitos > 1 && b[digitos] == b'0') {
                    saida.push_str(&f[trecho..inicio]);
                    let (c0, r0) = (coluna - 1, linha - 1);
                    saida.push_str(if col_fixa { "\u{1}C$" } else { "\u{1}C" });
                    escrever_inteiro(saida, if col_fixa { i64::from(c0) } else { i64::from(c0) - i64::from(col) });
                    saida.push_str(if lin_fixa { "R$" } else { "R" });
                    escrever_inteiro(saida, if lin_fixa { i64::from(r0) } else { i64::from(r0) - i64::from(row) });
                    saida.push('\u{1}');
                    trecho = j;
                    achou = true;
                }
                // Não era referência: segue depois do nome inteiro.
                i = if j > i { j } else { i + 1 };
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') { i += 1; }
            }
            _ => i += 1,
        }
    }
    saida.push_str(&f[trecho..]);
    achou
}

/// Prepara o lote em `nucleos` threads, mantendo a ordem: cada thread
/// converte (traduz a fórmula, formata o número) e analisa a sua parte num
/// texto próprio; no fim os textos se juntam e as posições se deslocam.
///
/// Fórmula repetida linha a linha (`E2*F2`, `E3*F3`…) é traduzida e
/// analisada uma vez: as outras com a mesma [`chave_relativa`] reaproveitam
/// o padrão em pt-BR, que só ganha a posição da célula. Traduzir e analisar
/// eram ~4 µs por fórmula — 28 s de CPU nos 7 milhões de fórmulas da
/// Planilha Pesada (16/10/2026).
fn preparar_em_paralelo(lote: LoteBruto, nucleos: usize, xfs: &[styles::XfStyle], estilos: &[Option<EstiloParcial>]) -> LotePreparado {
    preparar_em_paralelo_com(lote, nucleos, xfs, estilos, true)
}

fn preparar_em_paralelo_com(lote: LoteBruto, nucleos: usize, xfs: &[styles::XfStyle], estilos: &[Option<EstiloParcial>], reaproveitar: bool) -> LotePreparado {
    use feathersheet_core::planilha::sheet::carga::Preparo;
    use std::sync::Arc;
    type Bruta = (u32, u32, usize, Option<Value>, (u32, u32));
    let LoteBruto { texto, celulas } = lote;
    let texto = &texto;
    // O mesmo estilo que a gravação passa ao `carregar_preparada`.
    let estilo_de = |xf: usize| estilos.get(xf).and_then(Option::as_ref);
    let parte_pronta = |parte: Vec<Bruta>| -> (String, Vec<CelulaPreparada>) {
        let mut padroes = Padroes::default();
        // Chave relativa → (padrão em pt-BR, volátil, com árvore própria).
        let mut ja_vistas: HashMap<String, (Arc<str>, bool, bool)> = HashMap::new();
        let mut chave = String::new();
        let mut saida = String::with_capacity(parte.len() * 12);
        let prontas = parte.into_iter().map(|(r, c, xf, v, faixa)| {
            let bruto = &texto[faixa.0 as usize..faixa.1 as usize];
            let formula = bruto.strip_prefix(MARCA_FORMULA).map(str::trim);
            chave.clear();
            let com_chave = reaproveitar && formula.is_some_and(|f| chave_relativa(f, r, c, &mut chave));
            if com_chave {
                if let Some((padrao, volatil, com_arvore)) = ja_vistas.get(chave.as_str()) {
                    let inicio = saida.len() as u32;
                    // Texto vazio: o `carregar_preparada` monta pelo padrão só
                    // se a célula não couber na coluna empacotada.
                    if *com_arvore {
                        saida.push_str(&feathersheet_core::formula::formula_pattern::expand_pattern(padrao, r as i32, c as i32));
                    }
                    // Fora da coluna empacotada (`SOMASES(Outra!C:C;…)`) a árvore
                    // é de cada célula: só a análise do texto é refeita.
                    let arvore = com_arvore.then(|| {
                        let texto = saida[inicio as usize..].trim();
                        feathersheet_core::formula::parser::parse_formula(texto.strip_prefix('=').unwrap_or(texto)).map_err(|e| e.message)
                    });
                    return CelulaPreparada { row: r, col: c, estilo: xf, faixa: (inicio, saida.len() as u32),
                        preparo: Preparo::Formula { padrao: Some(padrao.clone()), arvore, volatil: *volatil }, guardado: v, ida_e_volta: None };
                }
            }
            if let Some(t) = bruto.strip_prefix(MARCA_TEXTO) {
                let inicio = saida.len() as u32;
                saida.push_str(t);
                return CelulaPreparada { row: r, col: c, estilo: xf, faixa: (inicio, saida.len() as u32),
                    preparo: Preparo::Literal(feathersheet_core::Value::Text(t.into())), guardado: v, ida_e_volta: None }
                    .com_ida_e_volta(t, estilo_de(xf));
            }
            let raw = converter(bruto, xf, xfs);
            let inicio = saida.len() as u32;
            saida.push_str(&raw);
            let pronta = CelulaPreparada::preparar(r, c, &raw, (inicio, saida.len() as u32), xf, v, &mut padroes)
                .com_ida_e_volta(&raw, estilo_de(xf));
            if com_chave {
                if let Preparo::Formula { padrao: Some(p), arvore, volatil } = &pronta.preparo {
                    ja_vistas.insert(chave.clone(), (p.clone(), *volatil, arvore.is_some()));
                }
            }
            pronta
        }).collect();
        (saida, prontas)
    };
    let partes_prontas: Vec<(String, Vec<CelulaPreparada>)> = if nucleos <= 1 || celulas.len() < 1_024 {
        vec![parte_pronta(celulas)]
    } else {
        let tamanho = celulas.len().div_ceil(nucleos);
        let mut partes = Vec::with_capacity(nucleos);
        let mut resto = celulas;
        while resto.len() > tamanho {
            let depois = resto.split_off(tamanho);
            partes.push(std::mem::replace(&mut resto, depois));
        }
        partes.push(resto);
        std::thread::scope(|escopo| {
            let tarefas: Vec<_> = partes.into_iter().map(|parte| escopo.spawn(move || parte_pronta(parte))).collect();
            tarefas.into_iter().map(|t| t.join().expect("preparar células")).collect()
        })
    };
    let mut final_texto = String::with_capacity(partes_prontas.iter().map(|(t, _)| t.len()).sum());
    let mut final_celulas = Vec::with_capacity(partes_prontas.iter().map(|(_, c)| c.len()).sum());
    for (t, cs) in partes_prontas {
        let desloc = final_texto.len() as u32;
        final_texto.push_str(&t);
        final_celulas.extend(cs.into_iter().map(|mut c| { c.faixa = (c.faixa.0 + desloc, c.faixa.1 + desloc); c }));
    }
    LotePreparado { texto: final_texto, celulas: final_celulas }
}

/// O `write!` genérico era a maior parte da [`chave_relativa`] (perf, 28/10).
fn escrever_inteiro(saida: &mut String, n: i64) {
    if n < 0 { saida.push('-'); }
    let mut v = n.unsigned_abs();
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    loop {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
        if v == 0 { break; }
    }
    saida.push_str(std::str::from_utf8(&buf[i..]).expect("dígitos"));
}

/// O XML que outra thread descompacta, em pedaços de 1 MB (`ler_aba`).
struct FluxoDescompactado {
    fila: std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
    atual: Vec<u8>,
    pos: usize,
}

impl std::io::Read for FluxoDescompactado {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let disponivel = std::io::BufRead::fill_buf(self)?;
        let n = disponivel.len().min(buf.len());
        buf[..n].copy_from_slice(&disponivel[..n]);
        std::io::BufRead::consume(self, n);
        Ok(n)
    }
}

impl std::io::BufRead for FluxoDescompactado {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos >= self.atual.len() {
            // Fila fechada: acabou o arquivo.
            match self.fila.recv() {
                Ok(pedaco) => self.atual = pedaco?,
                Err(_) => self.atual.clear(),
            }
            self.pos = 0;
        }
        Ok(&self.atual[self.pos..])
    }
    fn consume(&mut self, n: usize) {
        self.pos += n;
    }
}

fn ler_aba(
    archive: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>,
    caminho: &str,
    nome: &str,
    strings: &[String],
    xfs: &[styles::XfStyle],
    estilos: &[Option<EstiloParcial>],
    rels: &HashMap<String, String>,
    imagens: &HashMap<u32, String>,
    folha: &mut Sheet,
    nucleos: usize,
) -> Result<sheet::SheetJson, String> {
    let arquivo = match archive.by_name(caminho) {
        Ok(arquivo) => arquivo,
        Err(ZipError::FileNotFound) => {
            return Ok(sheet::SheetJson { name: nome.to_string(), ..Default::default() });
        }
        Err(e) => return Err(format!("XLSX: erro ao ler {caminho}: {e}")),
    };
    if arquivo.size() > MAX_WORKSHEET_XML {
        return Err(format!("XLSX: '{caminho}' excede 4 GiB de XML descomprimido"));
    }
    let mut shared = HashMap::<String, sheet::SharedFormula>::new();
    let mut alturas = Vec::<sheet::AlturaLida>::new();
    let mut ultima_visivel = -1_i64;
    let mut so_formatadas = 0usize;
    // Cada célula com a imagem leva uma cópia da data URL: uma figura citada
    // por 100 mil células pediria terabytes (#53).
    let mut bytes_de_imagem = 0usize;
    // Áreas de matriz dinâmica (`cm`): as células derramadas vêm só com valor e
    // não podem virar literais, senão bloqueiam o derrame (#SPILL!).
    let mut derramadas = Vec::<(i64, i64, i64, i64)>::new();
    let mut linhas_com_estilo = Vec::<(u32, usize)>::new();
    let mut vigia = feather_io::memoria::Vigia::new("A leitura do XLSX", 2_000);
    let inicio = std::time::Instant::now();

    // Três estágios, cada um na sua thread, passando lotes adiante:
    // ler o XML (esta thread) → preparar as células em `nucleos` threads
    // (literal, fórmula e padrão analisados) → gravar na aba (uma thread,
    // sem desfazer nem cascata). Antes era tudo aqui, célula por célula.
    let (para_preparar, a_preparar) = std::sync::mpsc::sync_channel::<LoteBruto>(4);
    let (para_gravar, a_gravar) = std::sync::mpsc::sync_channel::<LotePreparado>(4);
    let mut celulas = 0u64;
    let aba: &mut Sheet = &mut *folha;
    let (raiz, t_preparar, t_gravar) = std::thread::scope(|escopo| -> Result<_, String> {
        let preparador = escopo.spawn(move || {
            let mut gasto = std::time::Duration::ZERO;
            for lote in a_preparar {
                let t0 = std::time::Instant::now();
                let prontas = preparar_em_paralelo(lote, nucleos, xfs, estilos);
                gasto += t0.elapsed();
                if para_gravar.send(prontas).is_err() { break; }
            }
            gasto
        });
        // Solta o que a gravação não guardou (textos, árvores) longe da
        // thread que grava: liberar lá custava 17 s em 13 milhões de células.
        let (para_lixeira, lixeira) = std::sync::mpsc::sync_channel::<(String, Vec<Sobra>)>(8);
        escopo.spawn(move || for lote in lixeira { drop(lote); });
        let gravador = escopo.spawn(move || {
            let mut gasto = std::time::Duration::ZERO;
            // Cada estilo do arquivo é registrado na aba uma vez só.
            let mut indices: Vec<Option<Option<usize>>> = vec![None; estilos.len()];
            for LotePreparado { texto, celulas } in a_gravar {
                let t0 = std::time::Instant::now();
                let mut sobras = Vec::with_capacity(celulas.len());
                for celula in celulas {
                    let raw = celula.raw(&texto);
                    let estilo = estilos.get(celula.estilo).and_then(Option::as_ref);
                    let idx = match (estilo, indices.get_mut(celula.estilo)) {
                        (Some(e), Some(guardado)) => *guardado.get_or_insert_with(|| aba.registrar_parcial(e.clone())),
                        _ => None,
                    };
                    aba.carregar_preparada(celula, raw, idx, estilo, &mut sobras);
                }
                gasto += t0.elapsed();
                let _ = para_lixeira.send((texto, sobras));
            }
            aba.terminar_carga();
            gasto
        });

        let mut lote = LoteBruto::com_capacidade(LOTE_CARGA);
        let mut bruto = String::with_capacity(256);
        // A closure conserva uma única linha de XML por vez. Se o arquivo
        // extrapolar o limite, a leitura ZIP é cancelada imediatamente.
        // Leitor de linhas sem árvore: cada `<c>` vai para buffers
        // reaproveitados, sem objeto nem mapa de atributos por nó (a árvore
        // genérica custava ~20 s em 13 milhões de células).
        // Descompactar numa thread, analisar nesta: eram ~0,6 s de 5 s na
        // mesma thread, no XML de 668 MB do arquivo de 1 M linhas. Só com
        // núcleo sobrando: com 4 núcleos a thread a mais piorava (4,2 → 4,6 s).
        let fonte: Box<dyn std::io::BufRead + '_> = if nucleos >= 4 {
            let (para_xml, de_zip) = std::sync::mpsc::sync_channel::<std::io::Result<Vec<u8>>>(8);
            escopo.spawn(move || {
                let mut arquivo = crate::progresso::ContadorEtapa(arquivo);
                loop {
                    let mut pedaco = Vec::with_capacity(1 << 20);
                    match std::io::Read::read_to_end(&mut std::io::Read::take(&mut arquivo, 1 << 20), &mut pedaco) {
                        Ok(0) => break,
                        Ok(_) => if para_xml.send(Ok(pedaco)).is_err() { break },
                        Err(e) => { let _ = para_xml.send(Err(e)); break }
                    }
                }
            });
            Box::new(FluxoDescompactado { fila: de_zip, atual: Vec::new(), pos: 0 })
        } else {
            Box::new(BufReader::with_capacity(1 << 20, crate::progresso::ContadorEtapa(arquivo)))
        };
        let raiz = xml::parse_xml_linhas_rapidas(
            fonte, &mut |row| {
                let Some(linha) = row.attr("r").and_then(|r| r.parse::<i64>().ok()).map(|r| r - 1) else {
                    return Ok(());
                };
                if !(0..1_048_576).contains(&linha) { return Ok(()); }
                vigia.tique()?;
                let hidden = matches!(row.attr("hidden"), Some("1") | Some("true"));
                if !hidden { ultima_visivel = ultima_visivel.max(linha); }
                // Estilo da linha inteira: vale nas células vazias dela.
                let estilo_linha = row.attr("s").and_then(|v| v.parse::<usize>().ok())
                    .filter(|_| matches!(row.attr("customFormat"), Some("1") | Some("true")))
                    .filter(|&s| estilos.get(s).is_some_and(Option::is_some));
                if let Some(s) = estilo_linha { linhas_com_estilo.push((linha as u32, s)); }
                let ht = row.attr("ht").and_then(|v| v.parse::<f64>().ok());
                let propria = matches!(row.attr("customHeight"), Some("1") | Some("true"));
                let nivel = row.attr("outlineLevel").and_then(|s| s.parse::<u8>().ok()).unwrap_or(0).min(7);
                let recolhida = matches!(row.attr("collapsed"), Some("1") | Some("true"));
                if hidden || ht.is_some() || nivel > 0 || recolhida {
                    alturas.push((linha, ht, hidden, propria, nivel, recolhida));
                }
                // As linhas vêm em ordem: área que já acabou não pega mais
                // ninguém, e a lista fica só com as que cruzam esta linha.
                derramadas.retain(|&(_, _, r1, _)| r1 >= linha);
                for celula in row.celulas() {
                    let Some(at) = parse_cell_ref(celula.attr("r").unwrap_or("")) else { continue };
                    if at.row < 0 || at.row >= 1_048_576 || at.col < 0 || at.col >= MAX_COLS {
                        continue;
                    }
                    if !derramadas.is_empty() && derramadas.iter().any(|&(r0, c0, r1, c1)| (r0..=r1).contains(&at.row) && (c0..=c1).contains(&at.col)) {
                        continue;
                    }
                    if celula.attr("cm").is_some() {
                        if celula.f.get().is_some() && celula.f_t.get() == Some("array") {
                            let mut p = celula.f_ref.get().unwrap_or("").split(':');
                            // Área de uma célula só (a própria âncora) não tem vizinha a pular.
                            if let (Some(a), Some(b)) = (p.next().and_then(parse_cell_ref), p.next().and_then(parse_cell_ref)) {
                                if (a.row, a.col) != (b.row, b.col) {
                                    derramadas.push((a.row, a.col, b.row, b.col));
                                }
                            }
                        }
                    }
                    let xf_index = celula.attr("s").and_then(|s| s.parse::<usize>().ok()).unwrap_or(0);
                    let xf = xfs.get(xf_index);
                    // Fórmula comum e número ficam para o "preparar" (em
                    // paralelo); só o que depende de ordem (fórmula
                    // compartilhada) ou é barato (texto, lógico) sai aqui.
                    let tipo = celula.attr("t").unwrap_or("n");
                    let formula_comum = celula.f.get().filter(|f| !f.trim().is_empty() && celula.f_t.get() != Some("shared"));
                    // O texto vai para um buffer reaproveitado (sem alocar por célula).
                    bruto.clear();
                    // Imagem "Colocar na célula": a figura (data URL) no lugar do #VALUE!.
                    let imagem = celula.f.get().is_none().then(|| celula.vm.get()).flatten()
                        .and_then(|vm| vm.parse::<u32>().ok()).and_then(|vm| imagens.get(&vm));
                    if let Some(url) = imagem {
                        bytes_de_imagem += url.len();
                        if bytes_de_imagem > MAX_BYTES_IMAGEM_NA_ABA {
                            return Err(format!("XLSX: imagens nas células da aba '{nome}' passam de {} MB.", MAX_BYTES_IMAGEM_NA_ABA >> 20));
                        }
                        bruto.push_str(url);
                    } else if let Some(f) = formula_comum {
                        bruto.push(MARCA_FORMULA);
                        bruto.push_str(f);
                    } else if celula.f.get().is_none() && tipo == "n" {
                        if let Some(v) = celula.v.get().filter(|v| !v.trim().is_empty()) {
                            bruto.push(MARCA_NUMERO);
                            bruto.push_str(v);
                        }
                    } else if celula.f.get().is_none() && matches!(tipo, "s" | "str" | "inlineStr") {
                        let texto = sheet::cell_raw(celula, strings, xf, &mut shared, at.row, at.col);
                        if !texto.is_empty() {
                            bruto.push(MARCA_TEXTO);
                            bruto.push_str(&texto);
                        }
                    } else {
                        bruto.push_str(&sheet::cell_raw(celula, strings, xf, &mut shared, at.row, at.col));
                    }
                    let raw = &bruto;
                    // Numa linha com estilo, a célula usa a variante que cobre o da linha.
                    let xf_estilo = if estilo_linha.is_some() && xf_index < xfs.len() { xf_index + xfs.len() } else { xf_index };
                    let tem_estilo = estilos.get(xf_estilo).is_some_and(Option::is_some);
                    if raw.is_empty() && !tem_estilo { continue; }
                    if raw.is_empty() {
                        so_formatadas += 1;
                        if so_formatadas > MAX_CELULAS_VAZIAS_FORMATADAS {
                            return Err(format!(
                                "XLSX contém mais de {MAX_CELULAS_VAZIAS_FORMATADAS} células vazias apenas formatadas na aba '{nome}'. Remova a formatação de áreas sem dados."
                            ));
                        }
                    }
                    celulas += 1;
                    let guardado = sheet::valor_guardado(celula, strings);
                    lote.push(at.row as u32, at.col as u32, raw, xf_estilo, guardado);
                    if lote.len() >= LOTE_CARGA {
                        let cheio = std::mem::replace(&mut lote, LoteBruto::com_capacidade(LOTE_CARGA));
                        para_preparar.send(cheio).map_err(|_| "XLSX: a gravação parou no meio".to_string())?;
                    }
                }
                Ok(())
            },
        );
        if raiz.is_ok() && !lote.is_empty() {
            let _ = para_preparar.send(lote);
        }
        // Fechar o canal encerra o preparador, que encerra o gravador.
        drop(para_preparar);
        let t_preparar = preparador.join().map_err(|_| "XLSX: falha ao preparar as células".to_string())?;
        let t_gravar = gravador.join().map_err(|_| "XLSX: falha ao gravar as células".to_string())?;
        Ok((raiz?, t_preparar, t_gravar))
    })?;
    let total_aba = inicio.elapsed();
    if celulas > 0 {
        feathersheet_core::anotar!(
            "[abrir] aba '{nome}': {celulas} células em {:.1} s · {nucleos} núcleos · preparar {:.1} s · gravar no motor {:.1} s (ler o XML corre junto)",
            total_aba.as_secs_f64(), t_preparar.as_secs_f64(), t_gravar.as_secs_f64());
    }
    // Metadados pequenos (dimensões, regras, mesclas) são analisados depois
    // das linhas, sem reter o sheetData.
    let meta = sheet::ler_metadados(nome, &raiz, rels, alturas, ultima_visivel, xfs);
    for (linha, s) in linhas_com_estilo {
        if let Some(Some(e)) = estilos.get(s) { folha.row_styles.insert(linha, e.clone()); }
    }
    // Estilo de coluna inteira (`<col style="..">`).
    if let Some(cols) = raiz.child("cols") {
        // Faixas em ordem e sem sobreposição, como em `ler_metadados` (#53).
        let mut proxima = 0u32;
        for col in cols.children_named("col") {
            let Some(Some(e)) = col.attr("style").and_then(|v| v.parse::<usize>().ok()).and_then(|s| estilos.get(s)) else { continue };
            let min = (col.attr("min").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).max(1) - 1).max(proxima);
            let max = (col.attr("max").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).max(1) - 1).min(MAX_COLS as u32 - 1);
            proxima = proxima.max(max.saturating_add(1));
            for c in min..=max { folha.col_styles.insert(c, e.clone()); }
        }
    }

    // Hiperlinks ficam nas relações e podem aparecer depois de sheetData.
    // Visitamos só células ocupadas; intervalos A:XFD jamais geram células vazias.
    if let Some(hyperlinks) = raiz.child("hyperlinks") {
        for h in hyperlinks.children_named("hyperlink") {
            let Some(url) = h.attr("r:id").and_then(|id| rels.get(id)) else { continue };
            let ref_text = h.attr("ref").unwrap_or("").replace('$', "");
            let mut partes = ref_text.split(':');
            let Some(inicio) = partes.next().and_then(parse_cell_ref) else { continue };
            let fim = partes.next().and_then(parse_cell_ref).unwrap_or(inicio);
            if fim.row < inicio.row || fim.col < inicio.col { continue; }
            let range = feathersheet_core::celulas::refs::Rect::new(
                inicio.row as u32, inicio.col as u32,
                fim.row.min(1_048_575) as u32, fim.col.min(MAX_COLS - 1) as u32,
            );
            for chave in folha.chaves_no_retangulo(&range) {
                let (r, c) = Sheet::key_to_coord(chave);
                if let Some(mut snap) = folha.snapshot(r, c) {
                    if snap.raw.is_empty() { continue; }
                    snap.style.get_or_insert_with(EstiloParcial::new)
                        .insert("link".into(), Campo::Texto(url.clone()));
                    folha.apply(r, c, Some(snap), false);
                }
            }
        }
    }
    Ok(meta)
}

/// Autor, última alteração e empresa (`docProps/core.xml` e `app.xml`);
/// vazio quando o arquivo não traz essas partes.
fn propriedades(arquivo: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>) -> feathersheet_core::planilha::workbook::Propriedades {
    let mut ler = |parte: &str, campos: &[&str]| -> Vec<String> {
        let no = entry_text(arquivo, parte).ok().and_then(|t| xml::parse_xml(&t).ok());
        campos.iter().map(|c| no.as_ref().and_then(|n| n.child(c)).map(|n| n.text.trim().to_owned()).unwrap_or_default()).collect()
    };
    let core = ler("docProps/core.xml", &["creator", "lastModifiedBy"]);
    let app = ler("docProps/app.xml", &["Company"]);
    feathersheet_core::planilha::workbook::Propriedades {
        autor: core[0].clone(), modificado_por: core[1].clone(), organizacao: app[0].clone(),
        protecao_estrutura: None,
    }
}

/// Verifica o tamanho antes de reservar a memória do ZIP compactado.
pub fn ler_xlsx_arquivo(path: &std::path::Path) -> Result<(Workbook, usize), String> {
    let tamanho = std::fs::metadata(path)
        .map_err(|e| format!("Não foi possível acessar {}: {e}", path.display()))?
        .len();
    if tamanho > LIMITE_XLSX_NATIVO {
        return Err(format!(
            "Arquivo XLSX possui {} MiB, acima do limite de {} MiB compactados",
            tamanho / 1_048_576, LIMITE_XLSX_NATIVO / 1_048_576,
        ));
    }
    let bytes = std::fs::read(path)
        .map_err(|e| format!("Não foi possível ler {}: {e}", path.display()))?;
    ler_xlsx_workbook(bytes)
}

/// Abre XLSX grande no Workbook nativo. Não há segunda cópia dos dados em JSON.
/// Retorna a pasta pronta e a aba ativa, como os outros codecs da GUI.
pub fn ler_xlsx_workbook(bytes: Vec<u8>) -> Result<(Workbook, usize), String> {
    if bytes.len() as u64 > LIMITE_XLSX_NATIVO {
        return Err(format!("Arquivo XLSX excede {} MiB compactados", LIMITE_XLSX_NATIVO / 1_048_576));
    }
    let mut arquivo = ZipArchive::new(Cursor::new(std::sync::Arc::<[u8]>::from(bytes)))
        .map_err(|e| format!("Não parece um arquivo XLSX/ZIP válido: {e}"))?;
    let book_xml = entry_text(&mut arquivo, "xl/workbook.xml")?;
    let book = xml::parse_xml(&book_xml)?;
    if book.name != "workbook" {
        return Err("XLSX inválido: falta xl/workbook.xml".into());
    }
    let rels = sheet::read_rels(&entry_text(&mut arquivo, "xl/_rels/workbook.xml.rels")?);
    let strings = strings_compartilhadas(&mut arquivo)?;
    let estilos_xml = entry_text(&mut arquivo, "xl/styles.xml")?;
    // Tema e cores indexadas do arquivo valem até o fim da leitura (regras
    // condicionais e gráficos também resolvem cor por tema).
    let tema_xml = entry_text(&mut arquivo, "xl/theme/theme1.xml").unwrap_or_default();
    let _paleta = super::cores::instalar(&tema_xml, &estilos_xml);
    let xfs = styles::read_styles(&estilos_xml);
    let estilos = estilos_parciais(&xfs);
    // Imagens "Colocar na célula" (vm → data URL), lidas uma vez para todas as abas.
    let imagens_vm = super::imagens_celula::ler(&mut arquivo);
    let mut workbook = Workbook::vazia();
    let mut resumos = Vec::new();
    let mut caminhos = Vec::new();
    let mut folhas = Vec::new();
    // A barra da montagem anda pelos bytes de XML das abas.
    let xml_total: u64 = (0..arquivo.len())
        .filter_map(|i| arquivo.by_index_raw(i).ok().filter(|e| e.name().starts_with("xl/worksheets/")).map(|e| e.size()))
        .sum();
    crate::progresso::etapa_total(xml_total);
    if let Some(sheets) = book.child("sheets") {
        for (i, node) in sheets.children_named("sheet").enumerate() {
            let target = node.attr("r:id")
                .and_then(|id| rels.get(id))
                .cloned()
                .unwrap_or_else(|| format!("worksheets/sheet{}.xml", i + 1));
            let caminho = if let Some(abs) = target.strip_prefix('/') {
                abs.to_string()
            } else {
                format!("xl/{}", target.strip_prefix("./").unwrap_or(&target))
            };
            let nome = node.attr("name").filter(|n| !n.is_empty())
                .map(str::to_owned).unwrap_or_else(|| format!("Planilha{}", i + 1));
            let oculta = matches!(node.attr("state"), Some("hidden") | Some("veryHidden"));
            let idx = workbook.criar_aba(nome.clone());
            // A aba sai da pasta para ser lida noutra thread e volta depois.
            let folha = std::mem::replace(&mut workbook.aba_mut(idx).expect("aba recém-criada").sheet, Sheet::new(""));
            caminhos.push((nome, caminho, oculta));
            folhas.push((idx, folha));
        }
    }
    let simultaneas = ABAS_SIMULTANEAS.min(folhas.len()).max(1);
    // Os núcleos vão para cada aba pelo tamanho do XML dela: dividir por igual
    // deixava a aba de 13 milhões de células com 1 núcleo em máquina de 4.
    let nucleos = feathersheet_core::paralelismo::nucleos();
    let tamanhos: Vec<u64> = caminhos.iter()
        .map(|(_, caminho, _)| arquivo.by_name(caminho).map(|f| f.size()).unwrap_or(0))
        .collect();
    let total = tamanhos.iter().sum::<u64>().max(1);
    // `nucleos()` já deixa uma thread para a janela; mais uma fica para ler o
    // XML e gravar no motor. Mais de 4 preparando só disputavam com elas
    // (medido: 4 núcleos 7,2 → 4,3 s; 16: 3,3 s).
    let teto = nucleos.saturating_sub(1).clamp(1, 4);
    let por_aba: Vec<usize> = tamanhos.iter()
        .map(|&t| ((nucleos as u64 * t).div_ceil(total) as usize).clamp(1, teto))
        .collect();
    let lidas = em_paralelo(&arquivo, simultaneas, caminhos.iter().zip(folhas).zip(por_aba).collect(), |zip, (((nome, caminho, _), (idx, mut folha)), por_aba)| {
        // A paleta é por thread (`cores::instalar`).
        let _paleta = super::cores::instalar(&tema_xml, &estilos_xml);
        let (pasta, basename) = caminho.rsplit_once('/').unwrap_or(("", caminho.as_str()));
        let rels_aba = sheet::read_rels(&entry_text(zip, &format!("{pasta}/_rels/{basename}.rels"))?);
        let resumo = ler_aba(zip, caminho, nome, &strings, &xfs, &estilos, &rels_aba, &imagens_vm, &mut folha, por_aba)?;
        folha.comentarios = super::comentarios::ler_comentarios(zip, caminho);
        Ok::<_, String>((idx, folha, resumo))
    });
    for (lida, (_, _, oculta)) in lidas.into_iter().zip(&caminhos) {
        let (idx, mut folha, resumo) = lida?;
        folha.oculta = *oculta;
        workbook.aba_mut(idx).expect("aba recém-criada").sheet = folha;
        resumos.push(resumo);
    }
    if workbook.is_empty() { workbook.criar_aba("Planilha1"); }
    workbook.propriedades = propriedades(&mut arquivo);
    workbook.propriedades.protecao_estrutura = book.child("workbookProtection").and_then(|no| {
        matches!(no.attr("lockStructure"), Some("1" | "true")).then(|| feathersheet_core::planilha::workbook::ProtecaoPasta {
            // O `workbookPassword` antigo (16 bits) não é guardado: abre protegida sem senha.
            senha: (|| Some(feathersheet_core::planilha::sheet::protecao::SenhaPlanilha {
                algoritmo: no.attr("workbookAlgorithmName")?.to_string(),
                hash: no.attr("workbookHashValue")?.to_string(),
                sal: no.attr("workbookSaltValue")?.to_string(),
                giros: no.attr("workbookSpinCount")?.parse().ok()?,
            }))(),
        })
    });
    // Aba com desenho: linhas na altura do arquivo, antes de ancorar
    // gráficos e formas (ver `desenho_objetos::alturas_das_abas_com_desenho`).
    {
        let folhas: Vec<String> = caminhos.iter().map(|c| c.1.clone()).collect();
        let ultimas: Vec<u32> = (0..folhas.len()).map(|i| workbook.aba(i).map_or(0, |a| a.sheet.max_row)).collect();
        for (aba, px, ultima) in super::desenho_objetos::alturas_das_abas_com_desenho(&mut arquivo, &folhas, &ultimas) {
            let Some(resumo) = resumos.get_mut(aba) else { continue };
            let proprias: std::collections::HashSet<i64> = resumo.row_heights.iter().map(|(r, _)| *r).collect();
            for r in 0..=i64::from(ultima) {
                if !proprias.contains(&r) { resumo.row_heights.push((r, px as i64)); }
            }
        }
    }
    // Gráficos, tabelas e pivôs continuam íntegros: só os dados das células
    // não passam mais pelo JSON intermediário.
    anexar_elementos(&mut arquivo, &caminhos, &mut resumos)?;
    for (i, mut resumo) in resumos.into_iter().enumerate() {
        let Some(viva) = workbook.aba_mut(i) else { continue };
        let folha = &mut viva.sheet;
        folha.largura_padrao = resumo.largura_padrao.filter(|&w| w > 0).map(|w| w as u32);
        folha.linhas_ocultas_desde = resumo.linhas_ocultas_desde;
        folha.colunas_ocultas_desde = resumo.colunas_ocultas_desde;
        folha.topicos = std::mem::take(&mut resumo.topicos);
        folha.protecao = resumo.protecao.take();
        if !resumo.validacoes.is_empty() {
            folha.va = serde_json::to_string(&resumo.validacoes).ok();
        }
        folha.col_widths.extend(resumo.col_widths.into_iter()
            .filter(|(c, _)| *c >= 0)
            .map(|(c, w)| (c as u32, w.max(0) as u32)));
        folha.row_heights.extend(resumo.row_heights.into_iter()
            .filter(|(r, _)| *r >= 0)
            .map(|(r, h)| (r as u32, h.max(0) as u32)));
        if !resumo.charts.is_empty() { folha.g = Some(serde_json::Value::Array(resumo.charts).to_string()); }
        if !resumo.rules.is_empty() { folha.r = Some(serde_json::Value::Array(resumo.rules).to_string()); }
        if !resumo.mesclas.is_empty() { folha.mc = Some(serde_json::Value::Array(resumo.mesclas).to_string()); }
        if !resumo.tabelas.is_empty() { folha.tb = Some(serde_json::Value::Array(resumo.tabelas).to_string()); }
        if !resumo.dinamicas.is_empty() { folha.dn = Some(serde_json::Value::Array(resumo.dinamicas).to_string()); }
    }
    // Formas dos desenhos (retângulos, setas, caixas de texto) como objetos
    // soltos. No `.xlsm` as que têm texto viram botão de macro na abertura.
    let reguas: Vec<super::desenho_objetos::ReguaLida> = (0..caminhos.len()).map(|i| {
        let Some(a) = workbook.aba(i) else { return Default::default() };
        let s = &a.sheet;
        super::desenho_objetos::ReguaLida {
            larguras: s.col_widths.iter().map(|(&c, &w)| (c, w as f32)).collect(),
            largura_padrao: s.largura_padrao.filter(|w| *w > 0).map_or(super::desenho_objetos::LARGURA_GRADE, |w| w as f32),
            alturas: s.row_heights.iter().map(|(&r, &h)| (r, h as f32)).collect(),
            altura_padrao: super::desenho_objetos::ALTURA_GRADE,
        }
    }).collect();
    for (aba, objetos) in super::desenho_objetos::objetos_dos_desenhos(&mut arquivo, &reguas) {
        let mut lista = crate::objetos_aba::ObjetosAba::default();
        for o in objetos.into_iter().take(crate::objetos_aba::MAX_OBJETOS) { lista.adicionar(o); }
        if let Some(viva) = workbook.aba_mut(aba) { viva.sheet.ob = lista.gravar(); }
    }
    for n in sheet::read_names(&book) {
        // Nomes ocultos que o Excel cria para os gráficos novos (`chartEx`).
        if n.name.starts_with("_xlchart.") { continue; }
        if n.r0 < 0 || n.c0 < 0 || n.r1 < n.r0 || n.c1 < n.c0 { continue; }
        let mut area = feathersheet_core::celulas::refs::Rect::new(
            n.r0 as u32, n.c0 as u32, n.r1 as u32, n.c1 as u32,
        );
        area.sheet = n.sheet;
        workbook.definir_nome(&n.name, area);
    }
    let externos = super::vinculos::ler(&mut arquivo, &book, &rels);
    super::vinculos::instalar(&mut workbook, externos);
    // Dinâmica sem resultado nas células (Google Planilhas, `refreshOnLoad`)
    // é calculada aqui; com resultado, só ganha a área para o painel editar.
    super::dinamica_importada::materializar(&mut workbook);
    // A aba que o Excel mostrava ao salvar (`activeTab`), se estiver visível.
    let ativa = aba_ativa(&book).filter(|&i| workbook.aba(i).is_some_and(|a| !a.sheet.oculta)).unwrap_or(0);
    Ok((workbook, ativa))
}

/// `<bookViews><workbookView activeTab="n"/>`.
pub(super) fn aba_ativa(book: &xml::XmlNode) -> Option<usize> {
    book.child("bookViews")?.child("workbookView")?.attr("activeTab")?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    fn xlsx(linhas: &str, shared: Option<&str>) -> Vec<u8> {
        let mut saida = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut saida);
            let mut partes = vec![
                ("xl/workbook.xml", r#"<workbook><sheets><sheet name="Dados" r:id="rId1"/></sheets></workbook>"#),
                ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#),
                ("xl/worksheets/sheet1.xml", linhas),
            ];
            if let Some(s) = shared { partes.push(("xl/sharedStrings.xml", s)); }
            for (path, data) in partes {
                zip.start_file(path, SimpleFileOptions::default()).unwrap();
                zip.write_all(data.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        saida.into_inner()
    }

    #[test]
    fn col_repetido_nao_multiplica_as_larguras() {
        let cols = r#"<col min="2" max="16383" hidden="1"/><col min="1" max="16384" width="20" customWidth="1"/>"#.repeat(2000);
        let xml = format!(r#"<worksheet><cols>{cols}</cols><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData></worksheet>"#);
        let (wb, _) = ler_xlsx_workbook(xlsx(&xml, None)).unwrap();
        assert!(wb.aba(0).unwrap().sheet.col_widths.len() <= 16_384);
    }

    #[test]
    fn texto_do_excel_continua_texto() {
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1" t="s"><v>1</v></c><c r="C1" t="inlineStr"><is><t>=SOMA(1;2)</t></is></c><c r="D1" t="s"><v>2</v></c></row></sheetData></worksheet>"#;
        let sst = r#"<sst><si><t>00123</t></si><si><t>VERDADEIRO</t></si><si><t>R$ 5</t></si></sst>"#;
        let (wb, _) = ler_xlsx_workbook(xlsx(xml, Some(sst))).unwrap();
        let sh = &wb.aba(0).unwrap().sheet;
        assert_eq!(sh.get_value(0, 0), Value::Text("00123".into()));
        // shortcut: o texto "VERDADEIRO" ainda volta lógico — a coluna de dados
        // guarda o lógico como esse mesmo texto no pool (data_table.rs, #66).
        assert_eq!(sh.get_value(0, 2), Value::Text("=SOMA(1;2)".into()));
        assert_eq!(sh.get_value(0, 3), Value::Text("R$ 5".into()));
    }

    #[test]
    fn importa_sem_json_com_formula_estilo_e_strings_compartilhadas() {
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><v>42</v></c><c r="C1"><f>B1*2</f><v>84</v></c></row></sheetData></worksheet>"#;
        let sst = r#"<sst><si><t>Olá &amp; mundo</t></si></sst>"#;
        let (wb, _) = ler_xlsx_workbook(xlsx(xml, Some(sst))).unwrap();
        let sh = &wb.aba(0).unwrap().sheet;
        assert_eq!(sh.get_raw(0, 0), "Olá & mundo");
        assert_eq!(sh.get_raw(0, 1), "42");
        assert_eq!(sh.get_raw(0, 2), "=B1*2");
    }

    #[test]
    fn estilo_de_linha_e_coluna_colunas_ocultas_e_aba_ativa() {
        let estilos = r#"<styleSheet><fonts><font/></fonts>
          <fills><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill>
            <fill><patternFill patternType="solid"><fgColor rgb="FF000000"/></patternFill></fill></fills>
          <cellXfs><xf fillId="0"/><xf fillId="2"/><xf fillId="0" numFmtId="14"/></cellXfs></styleSheet>"#;
        let aba = r#"<worksheet><cols><col min="1" max="1" width="9" style="1" customWidth="1"/><col min="4" max="16384" width="9" hidden="1"/></cols>
          <sheetData><row r="1" s="1" customFormat="1"><c r="B1" s="2"><v>1</v></c></row><row r="2"><c r="B2"><v>2</v></c></row></sheetData></worksheet>"#;
        let mut saida = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut saida);
            for (p, d) in [
                ("xl/workbook.xml", r#"<workbook><bookViews><workbookView activeTab="1"/></bookViews><sheets><sheet name="A" r:id="rId1"/><sheet name="B" r:id="rId1"/></sheets></workbook>"#),
                ("xl/_rels/workbook.xml.rels", r#"<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>"#),
                ("xl/worksheets/sheet1.xml", aba),
                ("xl/styles.xml", estilos),
            ] {
                zip.start_file(p, SimpleFileOptions::default()).unwrap();
                zip.write_all(d.as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let (wb, ativa) = ler_xlsx_workbook(saida.into_inner()).unwrap();
        assert_eq!(ativa, 1);
        let sh = &wb.aba(0).unwrap().sheet;
        // Linha 1 pintada de preto; a célula B1 (com estilo próprio, sem fundo) não.
        assert_eq!(sh.estilo_resolvido(0, 5).and_then(|e| e.bg).as_deref(), Some("#000000"));
        assert_eq!(sh.estilo_resolvido(0, 1).and_then(|e| e.bg), None);
        // Coluna A pintada (estilo de coluna); linha 2 sem estilo próprio.
        assert_eq!(sh.estilo_resolvido(1, 0).and_then(|e| e.bg).as_deref(), Some("#000000"));
        assert_eq!(sh.estilo_resolvido(1, 2).and_then(|e| e.bg), None);
        // D até XFD ocultas: um número só.
        assert_eq!(sh.colunas_ocultas_desde, Some(3));
        assert!(sh.coluna_oculta(16383) && !sh.coluna_oculta(2));
    }

    #[test]
    fn mantem_texto_original_de_codigo_com_zeros_a_esquerda() {
        let xml = r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c></row></sheetData></worksheet>"#;
        let shared = r#"<sst><si><t>0007</t></si></sst>"#;
        let (wb, _) = ler_xlsx_workbook(xlsx(xml, Some(shared))).unwrap();
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0, 0), "0007");
    }

    #[test]
    fn strings_vazias_tambem_contam_no_teto() {
        // 12 milhões de `<si/>` passam de 256 MiB em cabeçalhos de `String`.
        let mut sst = String::with_capacity(12_000_000 * 5 + 16);
        sst.push_str("<sst>");
        for _ in 0..12_000_000 {
            sst.push_str("<si/>");
        }
        sst.push_str("</sst>");
        let xml = r#"<worksheet><sheetData/></worksheet>"#;
        assert!(ler_xlsx_workbook(xlsx(xml, Some(&sst))).is_err());
    }

    #[test]
    fn muitas_matrizes_de_uma_celula_nao_ficam_quadraticas() {
        use std::fmt::Write as _;
        let mut xml = String::from("<worksheet><sheetData>");
        for row in 1..=100_000 {
            write!(xml, "<row r=\"{row}\"><c r=\"A{row}\" cm=\"1\"><f t=\"array\" ref=\"A{row}:A{row}\">1</f><v>1</v></c></row>").unwrap();
        }
        xml.push_str("</sheetData></worksheet>");
        let inicio = std::time::Instant::now();
        let (wb, _) = ler_xlsx_workbook(xlsx(&xml, None)).unwrap();
        assert!(inicio.elapsed().as_secs() < 20, "{:?}", inicio.elapsed());
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(99_999, 0), "=1");
    }

    #[test]
    fn importa_mais_de_250_mil_celulas_sem_teto_antigo() {
        let mut xml = String::from("<worksheet><sheetData>");
        for row in 1..=260_000 {
            use std::fmt::Write as _;
            write!(xml, "<row r=\"{row}\"><c r=\"A{row}\"><v>{row}</v></c></row>").unwrap();
        }
        xml.push_str("</sheetData></worksheet>");
        let (wb, _) = ler_xlsx_workbook(xlsx(&xml, None)).unwrap();
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(259_999, 0), "260000");
    }
}

#[cfg(test)]
mod testes_reaproveitar {
    use super::*;
    use feathersheet_core::planilha::sheet::carga::Preparo;

    /// Com e sem reaproveitar a tradução, o lote preparado é o mesmo:
    /// o mesmo texto em cada célula e o mesmo padrão.
    #[test]
    fn reaproveitar_a_traducao_da_o_mesmo_resultado() {
        let modelos: Vec<fn(u32) -> String> = vec![
            |r| format!("E{r}*F{r}"),
            |r| format!("IF(E{r}>50,G{r}*0.1,0)"),
            |r| format!("K{}+I{r}", r.max(2) - 1),
            |r| format!("_xlfn.XLOOKUP(D{r},Produtos!$A$2:$A$201,Produtos!$C$2:$C$201)"),
            |r| format!("IF(I{r}>2000,\"Alto A{r}\",IF(I{r}>800,\"Médio\",\"Baixo\"))"),
            |r| format!("SUM($A$1:A{r})"),
            |r| format!("LOG10(A{r})+ATAN2(B{r},C{r})"),
            |r| format!("'Plano 2023'!B{r}*Q1!C{r}"),
            |r| format!("SUMIFS(Dados!$I$2:$I$1000001,Dados!$C$2:$C$1000001,$A{r},Dados!$J$2:$J$1000001,{r})"),
            |r| format!("SUMIFS(Raw_Beverages!C:C,Raw_Beverages!$A:$A,$A{r},Raw_Beverages!$B:$B,\"Store\")"),
            |_| "A:A".to_string(),
            |_| "TODAY()".to_string(),
            |r| format!("IFERROR(1/(A{r}-1),\"x\")&TEXT(B{r},\"0.00\")"),
            |r| format!("XFD{r}+AA{r}*$B${r}"),
            |r| format!("SUM(A{r}:C{r})/COUNT(1:1)"),
            |r| format!("A{r}=TRUE"),
            |r| format!("{{1,2;3,4}}*A{r}"),
            |r| format!("Tabela1[Coluna]+A{r}"),
            |r| format!("LEN(\"B{r}\")+B{r}"),
        ];
        let mut lote = LoteBruto::com_capacidade(4096);
        for r in 1..400u32 {
            for (c, m) in modelos.iter().enumerate() {
                let f = m(r + 1);
                lote.push(r, c as u32 + 3, &format!("{MARCA_FORMULA}{f}"), 0, None);
            }
            lote.push(r, 30, &format!("{MARCA_NUMERO}{r}.5"), 0, None);
            lote.push(r, 31, "texto", 0, None);
        }
        let copia = LoteBruto { texto: lote.texto.clone(), celulas: lote.celulas.clone() };
        let com = preparar_em_paralelo_com(lote, 4, &[], &[], true);
        let sem = preparar_em_paralelo_com(copia, 4, &[], &[], false);
        assert_eq!(com.celulas.len(), sem.celulas.len());
        let padrao = |p: &Preparo| match p {
            Preparo::Formula { padrao, arvore, volatil } => format!("{padrao:?} {arvore:?} {volatil}"),
            outro => format!("{outro:?}"),
        };
        for (a, b) in com.celulas.iter().zip(&sem.celulas) {
            assert_eq!((a.row, a.col), (b.row, b.col));
            // Reaproveitada: texto vazio, montado pelo padrão na gravação.
            let texto_a = match (&a.preparo, a.raw(&com.texto)) {
                (Preparo::Formula { padrao: Some(p), .. }, "") =>
                    feathersheet_core::formula::formula_pattern::expand_pattern(p, a.row as i32, a.col as i32),
                (_, t) => t.to_string(),
            };
            assert_eq!(texto_a.trim(), b.raw(&sem.texto).trim(), "célula ({}, {})", a.row, a.col);
            assert_eq!(padrao(&a.preparo), padrao(&b.preparo), "célula ({}, {}) {}", a.row, a.col, a.raw(&com.texto));
        }
    }

    #[test]
    fn chave_relativa_reconhece_so_referencias() {
        let mut k = String::new();
        assert!(chave_relativa("E3*F3", 2, 6, &mut k));
        let mut k2 = String::new();
        assert!(chave_relativa("E4*F4", 3, 6, &mut k2));
        assert_eq!(k, k2, "a mesma fórmula uma linha abaixo tem a mesma chave");
        for (f, tem) in [("LOG10(2)", false), ("Q1!A1", true), ("\"A1\"", false), ("SUM(A:A)", false), ("NOME_DEFINIDO", false), ("$B$2", true)] {
            k.clear();
            assert_eq!(chave_relativa(f, 0, 0, &mut k), tem, "{f}: {k:?}");
        }
        k.clear();
        chave_relativa("Q1!A1", 0, 0, &mut k);
        assert!(k.starts_with("Q1!"), "o nome da aba fica como texto: {k:?}");
    }
}
