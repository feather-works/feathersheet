//! As `<row>` do `sheetData` lidas direto nos bytes, sem a máquina genérica
//! do quick_xml (eventos, decodificação, escapes por evento). No `.xlsx` de
//! 1 milhão de linhas (668 MB de XML) o quick_xml era mais da metade da thread
//! que lê a aba, e ela era o gargalo de abrir o arquivo (28/10/2026).
//!
//! Preenche as mesmas [`LinhaXml`]/[`CelulaXml`] e segue as mesmas regras do
//! [`super::ler_linha`]: o resto do documento (colunas, mesclas, regras…) vai
//! para o quick_xml como sempre, sem as linhas.

use super::{Alvo, Campo, LinhaXml};
use std::io::BufRead;

/// Lê até a abertura do `sheetData` (inclusive), guardando tudo em
/// `prefixo`. `Some((true, sobra))`: era `<sheetData/>`, sem linhas;
/// `Some((false, sobra))`: as linhas vêm a seguir. `sobra` é o que já saiu do
/// fluxo depois da tag. `None`: não achou (o documento inteiro ficou no `prefixo`).
pub(super) fn ate_sheet_data<R: BufRead>(src: &mut R, prefixo: &mut Vec<u8>) -> Result<Option<(bool, Vec<u8>)>, String> {
    let mut procurado_ate = 0;
    loop {
        if let Some(i) = memchr::memmem::find(&prefixo[procurado_ate..], b"sheetData").map(|i| i + procurado_ate) {
            // `<sheetData` ou `<x:sheetData`, seguido de fim de nome.
            let antes = &prefixo[..i];
            let abre = antes.iter().rposition(|&b| b == b'<');
            let seguinte = prefixo.get(i + 9).copied();
            let nome_ok = abre.is_some_and(|a| antes[a + 1..].iter().all(|&b| b.is_ascii_alphanumeric() || b == b':' || b == b'_'))
                && abre.is_some_and(|a| prefixo.get(a + 1) != Some(&b'/'));
            match seguinte {
                None => {}
                Some(b) if nome_ok && (b == b'>' || b == b'/' || b.is_ascii_whitespace()) => {
                    if let Some(fim) = fim_da_tag(prefixo, i) {
                        let vazio = prefixo[fim - 1] == b'/';
                        let sobra = prefixo.split_off(fim + 1);
                        return Ok(Some((vazio, sobra)));
                    }
                }
                Some(_) => procurado_ate = i + 1,
            }
        }
        let pedaco = src.fill_buf().map_err(|e| format!("XML: {e}"))?;
        if pedaco.is_empty() {
            return Ok(None);
        }
        procurado_ate = prefixo.len().saturating_sub(16).max(procurado_ate.min(prefixo.len()));
        let n = pedaco.len();
        prefixo.extend_from_slice(pedaco);
        src.consume(n);
    }
}

/// O `>` que fecha a tag aberta perto de `de` (aspas respeitadas).
fn fim_da_tag(b: &[u8], de: usize) -> Option<usize> {
    let mut aspa = None;
    for (i, &c) in b.iter().enumerate().skip(de) {
        match (aspa, c) {
            (None, b'"' | b'\'') => aspa = Some(c),
            (Some(a), _) if c == a => aspa = None,
            (None, b'>') => return Some(i),
            _ => {}
        }
    }
    None
}

/// Nome local (sem prefixo) e se é tag de fechamento, de `<…` em `b[i]`.
fn nome_em(b: &[u8], i: usize) -> (bool, &[u8], usize) {
    let fecha = b.get(i + 1) == Some(&b'/');
    let ini = i + 1 + usize::from(fecha);
    let fim = b[ini..].iter().position(|&c| c.is_ascii_whitespace() || c == b'>' || c == b'/').map_or(b.len(), |p| ini + p);
    let nome = &b[ini..fim];
    let local = nome.iter().rposition(|&c| c == b':').map_or(nome, |p| &nome[p + 1..]);
    (fecha, local, fim)
}

/// Lê as linhas até o fim do `sheetData`, chamando `ler` a cada uma. Devolve
/// o que sobrou do fluxo já lido (a partir do `</sheetData>`), para o resto
/// do documento seguir pelo quick_xml.
pub(super) fn linhas<R: BufRead>(src: &mut R, inicio: Vec<u8>, linha: &mut LinhaXml,
    ler: &mut dyn FnMut(&LinhaXml) -> Result<(), String>) -> Result<Vec<u8>, String> {
    let mut buf = inicio;
    buf.reserve(1 << 20);
    let mut pos = 0usize;
    let mut fim_do_fluxo = false;
    loop {
        // Uma linha inteira no buffer, ou o fim do sheetData.
        // `Some(Ok(Some((fim, era_linha))))`: avança até `fim`.
        let pronto = (|| -> Option<Result<Option<(usize, bool)>, String>> {
            let lt = pos + memchr::memchr(b'<', &buf[pos..])?;
            let (fecha, nome, fim_nome) = nome_em(&buf, lt);
            if fim_nome >= buf.len() { return None; }
            if nome == b"sheetData" && fecha {
                return Some(Ok(None));
            }
            if buf[lt + 1..].starts_with(b"!--") {
                let f = memchr::memmem::find(&buf[lt..], b"-->")?;
                return Some(Ok(Some((lt + f + 3, false))));
            }
            if buf[lt + 1..].starts_with(b"?") {
                let f = memchr::memmem::find(&buf[lt..], b"?>")?;
                return Some(Ok(Some((lt + f + 2, false))));
            }
            if fecha || nome != b"row" {
                return Some(Err(format!("XML: <{}> inesperado no sheetData", String::from_utf8_lossy(nome))));
            }
            let fim_abre = fim_da_tag(&buf, fim_nome)?;
            let fim = if buf[fim_abre - 1] == b'/' {
                fim_abre + 1
            } else {
                let mut de = fim_abre;
                loop {
                    let f = de + memchr::memmem::find(&buf[de..], b"row>")?;
                    // `</row>` ou `</x:row>`.
                    let tag = buf[..f].iter().rposition(|&c| c == b'<')?;
                    if buf.get(tag + 1) == Some(&b'/') && buf[tag + 2..f].iter().all(|&c| c != b'>' && (c == b':' || c.is_ascii_alphanumeric()))
                        && (f == tag + 2 || buf[f - 1] == b':') {
                        break f + 4;
                    }
                    de = f + 4;
                }
            };
            if let Err(e) = ler_linha(&buf[lt..fim], linha) { return Some(Err(e)); }
            Some(Ok(Some((fim, true))))
        })();
        match pronto {
            Some(Ok(Some((fim, era_linha)))) => {
                if era_linha {
                    ler(linha)?;
                }
                pos = fim;
                continue;
            }
            Some(Ok(None)) => return Ok(buf.split_off(pos)),
            Some(Err(e)) => return Err(e),
            None if fim_do_fluxo => return Err("XML inválido: sheetData sem fim".into()),
            None => {}
        }
        // Falta texto: descarta o já lido e pede mais ao fluxo.
        if pos > 0 {
            buf.drain(..pos);
            pos = 0;
        }
        let pedaco = src.fill_buf().map_err(|e| format!("XML: {e}"))?;
        if pedaco.is_empty() {
            fim_do_fluxo = true;
            continue;
        }
        let n = pedaco.len();
        buf.extend_from_slice(pedaco);
        src.consume(n);
    }
}

/// Uma `<row …>…</row>` (ou `<row …/>`) inteira em `b`.
pub(super) fn ler_linha(b: &[u8], linha: &mut LinhaXml) -> Result<(), String> {
    linha.limpar();
    let (_, _, fim_nome) = nome_em(b, 0);
    let fim_abre = fim_da_tag(b, fim_nome).ok_or("XML: <row> sem fim")?;
    let vazio = b[fim_abre - 1] == b'/';
    for (k, v) in atributos(&b[fim_nome..if vazio { fim_abre - 1 } else { fim_abre }]) {
        let campo = match k {
            b"r" => &mut linha.r,
            b"s" => &mut linha.s,
            b"customFormat" => &mut linha.custom_format,
            b"hidden" => &mut linha.hidden,
            b"ht" => &mut linha.ht,
            b"customHeight" => &mut linha.custom_height,
            b"outlineLevel" => &mut linha.outline_level,
            b"collapsed" => &mut linha.collapsed,
            _ => continue,
        };
        definir(campo, v);
    }
    if vazio {
        return Ok(());
    }
    let mut alvo = Alvo::Nada;
    let mut dentro_is = false;
    let mut em_rph = false;
    let mut p = fim_abre + 1;
    while let Some(off) = memchr::memchr(b'<', &b[p..]) {
        let lt = p + off;
        if alvo != Alvo::Nada && lt > p {
            texto(&b[p..lt], linha, alvo);
        }
        if b[lt + 1..].starts_with(b"![CDATA[") {
            let f = memchr::memmem::find(&b[lt..], b"]]>").ok_or("XML: CDATA sem fim")?;
            if alvo != Alvo::Nada {
                super::empurrar(linha, alvo, &String::from_utf8_lossy(&b[lt + 9..lt + f]));
            }
            p = lt + f + 3;
            continue;
        }
        if b[lt + 1..].starts_with(b"!--") {
            p = lt + memchr::memmem::find(&b[lt..], b"-->").ok_or("XML: comentário sem fim")? + 3;
            continue;
        }
        if b[lt + 1..].starts_with(b"?") {
            p = lt + memchr::memmem::find(&b[lt..], b"?>").ok_or("XML: instrução sem fim")? + 2;
            continue;
        }
        let (fecha, nome, fim_nome) = nome_em(b, lt);
        let fim_tag = fim_da_tag(b, fim_nome).ok_or("XML: tag sem fim")?;
        p = fim_tag + 1;
        if fecha {
            match nome {
                b"row" => return Ok(()),
                b"f" | b"v" | b"t" => alvo = Alvo::Nada,
                b"is" => dentro_is = false,
                b"rPh" => em_rph = false,
                _ => {}
            }
            continue;
        }
        let vazio = b[fim_tag - 1] == b'/';
        let attrs = &b[fim_nome..if vazio { fim_tag - 1 } else { fim_tag }];
        match nome {
            b"c" => {
                let c = linha.nova_celula();
                for (k, v) in atributos(attrs) {
                    let campo = match k {
                        b"r" => &mut c.r,
                        b"s" => &mut c.s,
                        b"t" => &mut c.t,
                        b"cm" => &mut c.cm,
                        b"vm" => &mut c.vm,
                        _ => continue,
                    };
                    definir(campo, v);
                }
                alvo = Alvo::Nada;
            }
            b"f" if linha.usadas > 0 => {
                let c = &mut linha.celulas[linha.usadas - 1];
                c.f.marcar();
                for (k, v) in atributos(attrs) {
                    let campo = match k {
                        b"t" => &mut c.f_t,
                        b"si" => &mut c.f_si,
                        b"ref" => &mut c.f_ref,
                        _ => continue,
                    };
                    definir(campo, v);
                }
                if !vazio { alvo = Alvo::F; }
            }
            b"v" if linha.usadas > 0 => {
                linha.celulas[linha.usadas - 1].v.marcar();
                if !vazio { alvo = Alvo::V; }
            }
            b"is" if linha.usadas > 0 => {
                linha.celulas[linha.usadas - 1].inline.marcar();
                dentro_is = !vazio;
            }
            b"rPh" if dentro_is && !vazio => em_rph = true,
            b"t" if dentro_is && !em_rph && !vazio => alvo = Alvo::Inline,
            _ => {}
        }
    }
    Err("XML inválido: <row> sem fim".into())
}

/// Texto entre tags, com as entidades resolvidas como o quick_xml fazia
/// (`&amp;`, `&#233;`…; entidade desconhecida some).
fn texto(b: &[u8], linha: &mut LinhaXml, alvo: Alvo) {
    let Some(c) = linha.usadas.checked_sub(1).map(|i| &mut linha.celulas[i]) else { return };
    let campo = match alvo {
        Alvo::F => &mut c.f,
        Alvo::V => &mut c.v,
        Alvo::Inline => &mut c.inline,
        Alvo::Nada => return,
    };
    desescapar(b, &mut campo.texto);
}

/// Junta `b` a `saida` resolvendo as entidades. `false` se havia entidade
/// desconhecida (no texto ela some; no atributo o quick_xml dava erro).
fn desescapar(b: &[u8], saida: &mut String) -> bool {
    let mut ok = true;
    let mut p = 0;
    while let Some(off) = memchr::memchr(b'&', &b[p..]) {
        let amp = p + off;
        saida.push_str(std::str::from_utf8(&b[p..amp]).unwrap_or_default());
        let Some(fim) = memchr::memchr(b';', &b[amp..]).map(|f| amp + f) else {
            ok = false;
            p = b.len();
            break;
        };
        let nome = &b[amp + 1..fim];
        let c = match nome {
            b"lt" => Some('<'),
            b"gt" => Some('>'),
            b"amp" => Some('&'),
            b"quot" => Some('"'),
            b"apos" => Some('\''),
            _ if nome.first() == Some(&b'#') => {
                let num = std::str::from_utf8(&nome[1..]).unwrap_or_default();
                let n = match num.strip_prefix('x') {
                    Some(h) => u32::from_str_radix(h, 16).ok(),
                    None => num.parse::<u32>().ok(),
                };
                n.and_then(char::from_u32)
            }
            _ => None,
        };
        match c {
            Some(c) => saida.push(c),
            None => ok = false,
        }
        p = fim + 1;
    }
    saida.push_str(std::str::from_utf8(&b[p..]).unwrap_or_default());
    ok
}

/// Os atributos de `b` (o miolo da tag, depois do nome): chave e valor cru.
fn atributos(b: &[u8]) -> impl Iterator<Item = (&[u8], &[u8])> {
    let mut p = 0;
    std::iter::from_fn(move || {
        while p < b.len() && b[p].is_ascii_whitespace() { p += 1; }
        let igual = p + memchr::memchr(b'=', &b[p..])?;
        let chave = b[p..igual].trim_ascii();
        let mut q = igual + 1;
        while q < b.len() && b[q].is_ascii_whitespace() { q += 1; }
        let aspa = *b.get(q).filter(|c| **c == b'"' || **c == b'\'')?;
        let fim = q + 1 + memchr::memchr(aspa, &b[q + 1..])?;
        p = fim + 1;
        Some((chave, &b[q + 1..fim]))
    })
}

/// O valor do atributo no campo, com as entidades resolvidas; entidade
/// desconhecida marca o campo sem texto (o quick_xml dava erro e o leitor
/// antigo fazia `marcar`).
fn definir(campo: &mut Campo, valor: &[u8]) {
    campo.limpar();
    if desescapar(valor, &mut campo.texto) {
        campo.tem = true;
    } else {
        campo.texto.clear();
        campo.marcar();
    }
}
