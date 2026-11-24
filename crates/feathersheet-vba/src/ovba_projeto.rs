//! Gravar o projeto VBA (`xl/vbaProject.bin`), o caminho de volta do [`crate::ovba`]: montar um projeto do zero
//! (pasta sem VBA) ou trocar e acrescentar módulos num que veio do `.xlsm`, guardando formulários, referências e
//! módulos das abas. O código vai sem o p-code e o `_VBA_PROJECT` diz "versão desconhecida": o Excel recompila o
//! texto ao abrir (MS-OVBA §2.3.4.1). Os campos `CMG`/`DPB`/`GC` do fluxo `PROJECT` usam a cifra do §2.4.3.

use crate::ovba::{descomprimir, LIMITE_DE_BYTES};
use crate::ovba_gravar::comprimir;
use std::io::{Cursor, Read, Write};

/// `_VBA_PROJECT` sem cache de desempenho: versão 0xFFFF, o Excel recompila (MS-OVBA §2.3.4.1).
const VBA_PROJECT: [u8; 7] = [0xCC, 0x61, 0xFF, 0xFF, 0x00, 0x00, 0x00];
const BASE_PASTA: &str = "0{00020819-0000-0000-C000-000000000046}";
const BASE_ABA: &str = "0{00020820-0000-0000-C000-000000000046}";
const ID_PROJETO: &str = "{00000000-0000-0000-0000-000000000000}";

/// Um módulo para gravar: o nome e o código VBA (sem as linhas `Attribute`, que o gravador põe).
#[derive(Debug, Clone, PartialEq)]
pub struct ModuloNovo {
    pub nome: String,
    pub codigo: String,
}

/// Texto → bytes na página de código do projeto (1252 ou UTF-8); o que não cabe vira `?`.
pub fn codificar(texto: &str, pagina: u16) -> Vec<u8> {
    if pagina == 65001 {
        return texto.as_bytes().to_vec();
    }
    const DE_0X80: &str = "€\u{81}‚ƒ„…†‡ˆ‰Š‹Œ\u{8d}Ž\u{8f}\u{90}‘’“”•–—˜™š›œ\u{9d}žŸ";
    texto.chars().map(|c| match u32::from(c) {
        n if n < 0x80 || (0xA0..0x100).contains(&n) => n as u8,
        _ => DE_0X80.chars().position(|x| x == c).map_or(b'?', |i| 0x80 + i as u8),
    }).collect()
}

/// Cifra de dados do MS-OVBA §2.4.3.2 (o `CMG`, `DPB` e `GC` do fluxo `PROJECT`), em hexadecimal maiúsculo.
pub fn cifrar(dados: &[u8], semente: u8) -> String {
    let chave = ID_PROJETO.bytes().fold(0u8, |s, b| s.wrapping_add(b));
    let versao = 2u8;
    let (versao_c, chave_c) = (semente ^ versao, semente ^ chave);
    let mut saida = vec![semente, versao_c, chave_c];
    let (mut claro1, mut cifrado1, mut cifrado2) = (chave, chave_c, versao_c);
    let mut por = |b: u8, saida: &mut Vec<u8>| {
        let c = b ^ cifrado2.wrapping_add(claro1);
        saida.push(c);
        (cifrado2, cifrado1, claro1) = (cifrado1, c, b);
    };
    for _ in 0..(semente & 6) / 2 {
        por(0, &mut saida);
    }
    for b in (dados.len() as u32).to_le_bytes().into_iter().chain(dados.iter().copied()) {
        por(b, &mut saida);
    }
    saida.iter().map(|b| format!("{b:02X}")).collect()
}

/// O inverso do [`cifrar`]: os dados (ou `None` se o texto não for uma cifra válida).
pub fn decifrar(hex: &str) -> Option<Vec<u8>> {
    let b: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect::<Option<_>>()?;
    let (&semente, &versao_c, &chave_c) = (b.first()?, b.get(1)?, b.get(2)?);
    let (chave, _versao) = (semente ^ chave_c, semente ^ versao_c);
    let (mut claro1, mut cifrado1, mut cifrado2) = (chave, chave_c, versao_c);
    let mut claros = Vec::new();
    for &c in &b[3..] {
        let x = c ^ cifrado2.wrapping_add(claro1);
        claros.push(x);
        (cifrado2, cifrado1, claro1) = (cifrado1, c, x);
    }
    let ignorados = usize::from((semente & 6) / 2);
    let tamanho = u32::from_le_bytes(claros.get(ignorados..ignorados + 4)?.try_into().ok()?) as usize;
    claros.get(ignorados + 4..ignorados + 4 + tamanho).map(<[u8]>::to_vec)
}

/// Um registro do fluxo `dir`: id e dados.
type Registro = (u16, Vec<u8>);

fn ler_registros(dir: &[u8]) -> Result<Vec<Registro>, String> {
    let mut saida = Vec::new();
    let mut i = 0;
    while i + 6 <= dir.len() {
        let id = u16::from_le_bytes([dir[i], dir[i + 1]]);
        let mut tamanho = u32::from_le_bytes([dir[i + 2], dir[i + 3], dir[i + 4], dir[i + 5]]) as usize;
        // PROJECTVERSION diz 4 no tamanho, mas tem 6 bytes; o terminador 0x0010 tem 4 de reservado.
        if id == 0x0009 { tamanho = 6; }
        let dado = dir.get(i + 6..i + 6 + tamanho).ok_or_else(|| feathersheet_core::tr!("O índice do projeto VBA está corrompido."))?;
        saida.push((id, dado.to_vec()));
        i += 6 + tamanho;
    }
    Ok(saida)
}

fn gravar_registros(registros: &[Registro]) -> Vec<u8> {
    let mut saida = Vec::new();
    for (id, dado) in registros {
        saida.extend_from_slice(&id.to_le_bytes());
        let tamanho = if *id == 0x0009 { 4 } else { dado.len() as u32 };
        saida.extend_from_slice(&tamanho.to_le_bytes());
        saida.extend_from_slice(dado);
    }
    saida
}

fn utf16(t: &str) -> Vec<u8> {
    t.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Os registros de um módulo no `dir` (MS-OVBA §2.3.4.2.3.2); `documento` = módulo da pasta ou de uma aba.
fn registros_do_modulo(nome: &str, pagina: u16, documento: bool) -> Vec<Registro> {
    vec![
        (0x0019, codificar(nome, pagina)), (0x0047, utf16(nome)),
        (0x001A, codificar(nome, pagina)), (0x0032, utf16(nome)),
        (0x001C, Vec::new()), (0x0048, Vec::new()),
        (0x0031, 0u32.to_le_bytes().to_vec()),
        (0x001E, 0u32.to_le_bytes().to_vec()),
        (0x002C, 0xFFFFu16.to_le_bytes().to_vec()),
        (if documento { 0x0022 } else { 0x0021 }, Vec::new()),
        (0x002B, Vec::new()),
    ]
}

/// As linhas `Attribute` que o editor do VBA põe no topo de cada módulo.
fn atributos(nome: &str, base: Option<&str>) -> String {
    let mut s = format!("Attribute VB_Name = \"{nome}\"\r\n");
    if let Some(b) = base {
        s.push_str(&format!("Attribute VB_Base = \"{b}\"\r\nAttribute VB_GlobalNameSpace = False\r\nAttribute VB_Creatable = False\r\n\
            Attribute VB_PredeclaredId = True\r\nAttribute VB_Exposed = True\r\nAttribute VB_TemplateDerived = False\r\nAttribute VB_Customizable = True\r\n"));
    }
    s
}

/// Código com fim de linha do Windows, terminando em quebra.
fn crlf(codigo: &str) -> String {
    let mut s = codigo.replace("\r\n", "\n").replace('\n', "\r\n");
    if !s.ends_with("\r\n") { s.push_str("\r\n"); }
    s
}

fn erro_ole(e: std::io::Error) -> String {
    feathersheet_core::trf!("Não consegui gravar o projeto VBA: {e}")
}

/// Um projeto novo, para uma pasta que não tinha VBA: o módulo da pasta (`nome_pasta`), um por aba (`abas`, os
/// codinomes) e os módulos padrão.
pub fn projeto_novo(nome_pasta: &str, abas: &[String], modulos: &[ModuloNovo]) -> Result<Vec<u8>, String> {
    let pagina = 1252u16;
    let mut dir: Vec<Registro> = vec![
        (0x0001, 1u32.to_le_bytes().to_vec()),
        (0x0002, 0x0409u32.to_le_bytes().to_vec()),
        (0x0014, 0x0409u32.to_le_bytes().to_vec()),
        (0x0003, pagina.to_le_bytes().to_vec()),
        (0x0004, b"VBAProject".to_vec()),
        (0x0005, Vec::new()), (0x0040, Vec::new()),
        (0x0006, Vec::new()), (0x003D, Vec::new()),
        (0x0007, 0u32.to_le_bytes().to_vec()),
        (0x0008, 0u32.to_le_bytes().to_vec()),
        (0x0009, [1u32.to_le_bytes().as_slice(), 0u16.to_le_bytes().as_slice()].concat()),
        (0x000C, Vec::new()), (0x003C, Vec::new()),
    ];
    // Referência à OLE Automation (stdole), como todo projeto do Excel.
    let libid = b"*\\G{00020430-0000-0000-C000-000000000046}#2.0#0#C:\\Windows\\System32\\stdole2.tlb#OLE Automation";
    dir.push((0x0016, b"stdole".to_vec()));
    dir.push((0x003E, utf16("stdole")));
    let mut registrada = (libid.len() as u32).to_le_bytes().to_vec();
    registrada.extend_from_slice(libid);
    registrada.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
    dir.push((0x000D, registrada));
    let total = 1 + abas.len() + modulos.len();
    dir.push((0x000F, (total as u16).to_le_bytes().to_vec()));
    dir.push((0x0013, 0xFFFFu16.to_le_bytes().to_vec()));
    let mut fontes: Vec<(String, String)> = Vec::new();
    let mut projeto = format!("ID=\"{ID_PROJETO}\"\r\n");
    for (nome, base) in std::iter::once((nome_pasta, BASE_PASTA)).chain(abas.iter().map(|a| (a.as_str(), BASE_ABA))) {
        dir.extend(registros_do_modulo(nome, pagina, true));
        fontes.push((nome.to_string(), atributos(nome, Some(base))));
        projeto.push_str(&format!("Document={nome}/&H00000000\r\n"));
    }
    for m in modulos {
        dir.extend(registros_do_modulo(&m.nome, pagina, false));
        fontes.push((m.nome.clone(), format!("{}{}", atributos(&m.nome, None), crlf(&m.codigo))));
        projeto.push_str(&format!("Module={}\r\n", m.nome));
    }
    // Terminador do `dir`: id 0x0010 e 4 bytes reservados (0), o mesmo desenho de um registro vazio.
    dir.push((0x0010, Vec::new()));
    projeto.push_str(&format!("Name=\"VBAProject\"\r\nHelpContextID=\"0\"\r\nVersionCompatible32=\"393222000\"\r\n\
        CMG=\"{}\"\r\nDPB=\"{}\"\r\nGC=\"{}\"\r\n\r\n[Host Extender Info]\r\n&H00000001={{3832D640-CF90-11CF-8E43-00A0C911005A}};VBE;&H00000000\r\n",
        cifrar(&[0, 0, 0, 0], 0x11), cifrar(&[0], 0x23), cifrar(&[0xFF], 0x35)));

    let mut cfb = cfb::CompoundFile::create(Cursor::new(Vec::new())).map_err(erro_ole)?;
    cfb.create_storage("/VBA").map_err(erro_ole)?;
    let grava = |cfb: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, caminho: &str, dados: &[u8]| -> Result<(), String> {
        cfb.create_stream(caminho).map_err(erro_ole)?.write_all(dados).map_err(erro_ole)
    };
    grava(&mut cfb, "/VBA/_VBA_PROJECT", &VBA_PROJECT)?;
    grava(&mut cfb, "/VBA/dir", &comprimir(&gravar_registros(&dir))?)?;
    for (nome, fonte) in &fontes {
        grava(&mut cfb, &format!("/VBA/{nome}"), &comprimir(&codificar(fonte, pagina))?)?;
    }
    grava(&mut cfb, "/PROJECT", &codificar(&projeto, pagina))?;
    let mut wm = Vec::new();
    for (nome, _) in &fontes {
        wm.extend(codificar(nome, pagina)); wm.push(0);
        wm.extend(utf16(nome)); wm.extend([0, 0]);
    }
    wm.extend([0, 0]);
    grava(&mut cfb, "/PROJECTwm", &wm)?;
    cfb.flush().map_err(erro_ole)?;
    Ok(cfb.into_inner().into_inner())
}

/// Troca o código dos módulos que existem (pelo nome, sem diferenciar maiúsculas) e acrescenta como módulos padrão
/// os que não existem. Formulários, referências e o resto do projeto ficam como estavam.
pub fn alterar_projeto(bin: &[u8], modulos: &[ModuloNovo]) -> Result<Vec<u8>, String> {
    let mut cfb = cfb::CompoundFile::open(Cursor::new(bin.to_vec()))
        .map_err(|_| feathersheet_core::tr!("O projeto VBA (vbaProject.bin) não é um arquivo OLE válido."))?;
    let ler = |cfb: &mut cfb::CompoundFile<Cursor<Vec<u8>>>, caminho: &str| -> Result<Vec<u8>, String> {
        let mut v = Vec::new();
        cfb.open_stream(caminho).map_err(|_| feathersheet_core::trf!("O projeto VBA não tem o fluxo {nome}.", nome = caminho))?
            .take(LIMITE_DE_BYTES as u64 + 1).read_to_end(&mut v).map_err(erro_ole)?;
        Ok(v)
    };
    let mut dir = ler_registros(&descomprimir(&ler(&mut cfb, "/VBA/dir")?)?)?;
    let pagina = dir.iter().find(|(id, _)| *id == 0x0003).and_then(|(_, d)| d.get(..2)).map_or(1252, |d| u16::from_le_bytes([d[0], d[1]]));
    let mut projeto = crate::ovba::decodificar(&ler(&mut cfb, "/PROJECT")?, pagina);
    let mut wm = ler(&mut cfb, "/PROJECTwm").unwrap_or_default();
    // Nome → (posição do registro de deslocamento, nome do fluxo).
    let mut existentes: Vec<(String, usize, String)> = Vec::new();
    let mut nome_atual = String::new();
    for (i, (id, d)) in dir.iter().enumerate() {
        match id {
            0x0019 => nome_atual = crate::ovba::decodificar(d, pagina),
            0x001A => existentes.push((nome_atual.clone(), 0, crate::ovba::decodificar(d, pagina))),
            0x0031 => if let Some(e) = existentes.last_mut() { e.1 = i; },
            _ => {}
        }
    }
    let mut novos = 0u16;
    for m in modulos {
        if let Some((_, pos, fluxo)) = existentes.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(&m.nome)).cloned() {
            // Guarda as linhas `Attribute` do código antigo (VB_Base do módulo de aba, por exemplo).
            let antigo = ler(&mut cfb, &format!("/VBA/{fluxo}"))?;
            let desloc = dir.get(pos).and_then(|(_, d)| d.get(..4)).map_or(0, |d| u32::from_le_bytes([d[0], d[1], d[2], d[3]]) as usize);
            let texto_antigo = antigo.get(desloc..).map(descomprimir).transpose()?.map(|b| crate::ovba::decodificar(&b, pagina)).unwrap_or_default();
            let cabeca: String = texto_antigo.split_inclusive('\n').take_while(|l| l.starts_with("Attribute ")).collect();
            let cabeca = if cabeca.is_empty() { atributos(&m.nome, None) } else { cabeca.replace("\r\n", "\n").replace('\n', "\r\n") };
            let fonte = format!("{cabeca}{}", crlf(&m.codigo));
            cfb.create_stream(format!("/VBA/{fluxo}")).map_err(erro_ole)?.write_all(&comprimir(&codificar(&fonte, pagina))?).map_err(erro_ole)?;
            dir[pos].1 = 0u32.to_le_bytes().to_vec();
        } else {
            let fim = dir.iter().rposition(|(id, _)| *id == 0x0010).ok_or_else(|| feathersheet_core::tr!("O índice do projeto VBA está corrompido."))?;
            dir.splice(fim..fim, registros_do_modulo(&m.nome, pagina, false));
            let fonte = format!("{}{}", atributos(&m.nome, None), crlf(&m.codigo));
            cfb.create_stream(format!("/VBA/{}", m.nome)).map_err(erro_ole)?.write_all(&comprimir(&codificar(&fonte, pagina))?).map_err(erro_ole)?;
            let linha = format!("Module={}\r\n", m.nome);
            let onde = projeto.find("\r\nName=").map_or(projeto.len(), |i| i + 2);
            projeto.insert_str(onde, &linha);
            if wm.len() >= 2 { wm.truncate(wm.len() - 2); }
            wm.extend(codificar(&m.nome, pagina)); wm.push(0);
            wm.extend(utf16(&m.nome)); wm.extend([0, 0, 0, 0]);
            novos += 1;
        }
    }
    if let Some((_, d)) = dir.iter_mut().find(|(id, _)| *id == 0x000F) {
        let n = u16::from_le_bytes([d[0], d[1]]) + novos;
        *d = n.to_le_bytes().to_vec();
    }
    cfb.create_stream("/VBA/dir").map_err(erro_ole)?.write_all(&comprimir(&gravar_registros(&dir))?).map_err(erro_ole)?;
    cfb.create_stream("/VBA/_VBA_PROJECT").map_err(erro_ole)?.write_all(&VBA_PROJECT).map_err(erro_ole)?;
    cfb.create_stream("/PROJECT").map_err(erro_ole)?.write_all(&codificar(&projeto, pagina)).map_err(erro_ole)?;
    if !wm.is_empty() { cfb.create_stream("/PROJECTwm").map_err(erro_ole)?.write_all(&wm).map_err(erro_ole)?; }
    // O cache de desempenho (`__SRP_*`) é do p-code antigo: sai, como o Excel faz ao recompilar.
    let srp: Vec<String> = cfb.walk().filter(|e| e.is_stream() && e.name().starts_with("__SRP_")).map(|e| e.path().to_string_lossy().into_owned()).collect();
    for caminho in srp { let _ = cfb.remove_stream(caminho); }
    cfb.flush().map_err(erro_ole)?;
    Ok(cfb.into_inner().into_inner())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::ovba::{ler_projeto, TipoDeModulo};

    #[test]
    fn cifra_vai_e_volta() {
        for semente in [0u8, 0x11, 0x23, 0x35, 0xFE] {
            for dados in [&[0u8, 0, 0, 0][..], &[0xFF], &[1, 2, 3, 4, 5, 6, 7, 8, 9]] {
                assert_eq!(decifrar(&cifrar(dados, semente)).as_deref(), Some(dados), "semente {semente}");
            }
        }
    }

    #[test]
    fn projeto_novo_le_de_volta_com_acento_e_moeda() {
        let m = ModuloNovo { nome: "Modulo1".into(), codigo: "Sub Ola()\n    Range(\"A1\").Value = \"Preço: R$ 1.234,50 — ok\"\nEnd Sub".into() };
        let bin = projeto_novo("EstaPasta_de_trabalho", &["Planilha1".into()], std::slice::from_ref(&m)).unwrap();
        let lidos = ler_projeto(&bin).unwrap();
        let nomes: Vec<(&str, TipoDeModulo)> = lidos.iter().map(|x| (x.nome.as_str(), x.tipo)).collect();
        assert_eq!(nomes, [("EstaPasta_de_trabalho", TipoDeModulo::Pasta), ("Planilha1", TipoDeModulo::Aba), ("Modulo1", TipoDeModulo::Padrao)]);
        assert!(lidos[2].codigo.contains("Preço: R$ 1.234,50 — ok"), "{}", lidos[2].codigo);
        assert!(lidos[2].codigo.starts_with("Attribute VB_Name = \"Modulo1\""));
    }

    #[test]
    fn alterar_troca_um_e_acrescenta_outro_sem_mexer_no_resto() {
        let a = ModuloNovo { nome: "Modulo1".into(), codigo: "Sub A()\nEnd Sub".into() };
        let bin = projeto_novo("ThisWorkbook", &["Sheet1".into()], &[a]).unwrap();
        let novo = alterar_projeto(&bin, &[
            ModuloNovo { nome: "modulo1".into(), codigo: "Sub A()\n    MsgBox \"trocado\"\nEnd Sub".into() },
            ModuloNovo { nome: "Extra".into(), codigo: "Sub B()\nEnd Sub".into() },
            ModuloNovo { nome: "Sheet1".into(), codigo: "Private Sub Worksheet_Activate()\nEnd Sub".into() },
        ]).unwrap();
        let lidos = ler_projeto(&novo).unwrap();
        let nomes: Vec<&str> = lidos.iter().map(|x| x.nome.as_str()).collect();
        assert_eq!(nomes, ["ThisWorkbook", "Sheet1", "Modulo1", "Extra"]);
        assert!(lidos[2].codigo.contains("trocado"));
        assert_eq!(lidos[1].tipo, TipoDeModulo::Aba, "o VB_Base da aba ficou");
        assert!(lidos[1].codigo.contains("Worksheet_Activate"));
        let mut cfb = cfb::CompoundFile::open(Cursor::new(novo)).unwrap();
        let mut projeto = String::new();
        cfb.open_stream("/PROJECT").unwrap().read_to_string(&mut projeto).unwrap();
        assert!(projeto.contains("Module=Modulo1\r\nModule=Extra\r\nName="), "{projeto}");
        let mut v = Vec::new();
        cfb.open_stream("/VBA/_VBA_PROJECT").unwrap().read_to_end(&mut v).unwrap();
        assert_eq!(v, VBA_PROJECT, "o Excel recompila");
    }
}
