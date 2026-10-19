//! Funções de compatibilidade: ASC, DBCS, BAHTTEXT, FONÉTICA, EUROCONVERT,
//! FILTROXML e as que dependem de servidor, DLL ou serviço na internet
//! (cubo OLAP, RTD, CHAMAR, REGISTRO.ID, HISTÓRICODEAÇÕES, TRADUZIR,
//! DETECTARIDIOMA). Estas últimas são reconhecidas, para a fórmula de um
//! arquivo do Excel abrir sem `#NOME?`, e devolvem `#N/D` com o motivo: o
//! FeatherSheet não conecta a servidor nem executa código de fora.

use crate::celulas::values::{CellError, Computed, ErrorCode, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::*;

// --- texto -------------------------------------------------------------------

/// Largura inteira (U+FF01–FF5E e o espaço ideográfico) → meia largura.
fn meia_largura(c: char) -> char {
    match c as u32 {
        0xFF01..=0xFF5E => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        0x3000 => ' ',
        _ => c,
    }
}

fn largura_inteira(c: char) -> char {
    match c as u32 {
        0x21..=0x7E => char::from_u32(c as u32 + 0xFEE0).unwrap_or(c),
        0x20 => '\u{3000}',
        _ => c,
    }
}

/// `=ASC(texto)` / `=ASC(text)`
pub fn fn_asc(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    // shortcut: só letras, dígitos e símbolos latinos; o katakana de meia largura não é convertido.
    por_elemento(args, ev, 1, 1, |e| texto(e.texto(0)?.chars().map(meia_largura).collect::<String>()))
}

/// `=DBCS(texto)` / `=DBCS(text)`
pub fn fn_dbcs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| texto(e.texto(0)?.chars().map(largura_inteira).collect::<String>()))
}

/// `=FONÉTICA(ref)` / `=PHONETIC(reference)`: o texto da célula (o motor não
/// guarda furigana, e sem ele o Excel devolve o próprio texto).
pub fn fn_phonetic(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 1, || {
        if node_rect(&args[0], ev).is_none() {
            return Err(e_valor());
        }
        texto(arg_texto(args, ev, 0)?)
    })
}

const DIGITOS_TAI: [&str; 10] = ["ศูนย์", "หนึ่ง", "สอง", "สาม", "สี่", "ห้า", "หก", "เจ็ด", "แปด", "เก้า"];
const CASAS_TAI: [&str; 6] = ["", "สิบ", "ร้อย", "พัน", "หมื่น", "แสน"];

/// Um número de 0 a 999.999 por extenso em tailandês; `antes` diz se já há
/// algo escrito à esquerda (muda o 1 das unidades para "เอ็ด").
fn grupo_tai(n: u64, antes: bool) -> String {
    let digitos: Vec<u64> = (0..6).map(|i| n / 10u64.pow(i) % 10).collect();
    let mut s = String::new();
    for casa in (0..6).rev() {
        let d = digitos[casa] as usize;
        if d == 0 {
            continue;
        }
        match (casa, d) {
            (1, 1) => s.push_str("สิบ"),
            (1, 2) => s.push_str("ยี่สิบ"),
            (0, 1) if antes || n > 9 => s.push_str("เอ็ด"),
            _ => {
                s.push_str(DIGITOS_TAI[d]);
                s.push_str(CASAS_TAI[casa]);
            }
        }
    }
    s
}

fn extenso_tai(mut n: u64) -> String {
    let mut grupos = Vec::new();
    while n > 0 {
        grupos.push(n % 1_000_000);
        n /= 1_000_000;
    }
    let mut s = String::new();
    for (i, g) in grupos.iter().enumerate().rev() {
        if *g > 0 {
            s.push_str(&grupo_tai(*g, !s.is_empty()));
        }
        if i > 0 {
            s.push_str("ล้าน");
        }
    }
    s
}

/// `=BAHTTEXT(número)` / `=BAHTTEXT(number)`: o valor em baht por extenso, em tailandês.
pub fn fn_bahttext(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let x = e.num(0)?;
        if x.abs() >= 1e15 {
            return Err(e_num());
        }
        let centavos = (x.abs() * 100.0).round() as u64;
        let (baht, satang) = (centavos / 100, centavos % 100);
        let mut s = String::new();
        if x < 0.0 && centavos > 0 {
            s.push_str("ลบ");
        }
        if baht > 0 {
            s.push_str(&extenso_tai(baht));
            s.push_str("บาท");
        }
        if satang == 0 {
            if baht == 0 {
                s.push_str("ศูนย์บาท");
            }
            s.push_str("ถ้วน");
        } else {
            s.push_str(&grupo_tai(satang, false));
            s.push_str("สตางค์");
        }
        texto(s)
    })
}

// --- EUROCONVERT --------------------------------------------------------------

/// Moedas que viraram euro: unidades por euro (taxa fixa e irrevogável) e
/// casas decimais de exibição.
const MOEDAS_EURO: &[(&str, f64, i32)] = &[
    ("EUR", 1.0, 2),
    ("BEF", 40.3399, 0),
    ("LUF", 40.3399, 0),
    ("DEM", 1.95583, 2),
    ("ESP", 166.386, 0),
    ("FRF", 6.55957, 2),
    ("IEP", 0.787564, 2),
    ("ITL", 1936.27, 0),
    ("NLG", 2.20371, 2),
    ("ATS", 13.7603, 2),
    ("PTE", 200.482, 0),
    ("FIM", 5.94573, 2),
    ("GRD", 340.750, 0),
    ("SIT", 239.640, 2),
    ("CYP", 0.585274, 2),
    ("MTL", 0.429300, 2),
    ("SKK", 30.1260, 2),
    ("EEK", 15.6466, 2),
    ("LVL", 0.702804, 2),
    ("LTL", 3.45280, 2),
    ("HRK", 7.53450, 2),
];

fn arredondar(x: f64, casas: i32) -> f64 {
    let f = 10f64.powi(casas);
    (x * f).round() / f
}

/// `=EUROCONVERT(número; origem; destino; [precisão_total]; [precisão_triangulação])` / `=EUROCONVERT(number, source, target, [full_precision], [triangulation_precision])`
pub fn fn_euroconvert(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 5, || {
        let x = arg_num(args, ev, 0)?;
        let moeda = |i: usize| -> Res<(f64, i32, bool)> {
            let codigo = arg_texto(args, ev, i)?.trim().to_uppercase();
            MOEDAS_EURO
                .iter()
                .find(|(c, _, _)| *c == codigo)
                .map(|(c, taxa, casas)| (*taxa, *casas, *c == "EUR"))
                .ok_or_else(e_valor)
        };
        let (taxa_o, _, euro_o) = moeda(1)?;
        let (taxa_d, casas_d, euro_d) = moeda(2)?;
        let total = if has(args, 3) { arg_logico(args, ev, 3)? } else { false };
        let mut euros = x / taxa_o;
        if has(args, 4) && !euro_o && !euro_d {
            let tp = arg_int(args, ev, 4)?;
            if tp < 3 {
                return Err(e_valor());
            }
            euros = arredondar(euros, tp as i32);
        }
        let r = euros * taxa_d;
        numero(if total { r } else { arredondar(r, casas_d) })
    })
}

// --- FILTROXML ----------------------------------------------------------------

#[derive(Debug, Default)]
struct Elemento {
    nome: String,
    atributos: Vec<(String, String)>,
    filhos: Vec<Elemento>,
    texto: String,
}

impl Elemento {
    /// Todo o texto de dentro, como o `string()` do XPath.
    fn texto_todo(&self) -> String {
        let mut s = self.texto.clone();
        for f in &self.filhos {
            s.push_str(&f.texto_todo());
        }
        s
    }

    fn descendentes<'a>(&'a self, saida: &mut Vec<&'a Elemento>) {
        for f in &self.filhos {
            saida.push(f);
            f.descendentes(saida);
        }
    }
}

fn entidades(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        saida.push_str(&resto[..i]);
        resto = &resto[i..];
        let Some(fim) = resto.find(';') else { break };
        let nome = &resto[1..fim];
        let c = match nome {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if nome.starts_with("#x") => u32::from_str_radix(&nome[2..], 16).ok().and_then(char::from_u32),
            _ if nome.starts_with('#') => nome[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                saida.push(c);
                resto = &resto[fim + 1..];
            }
            None => {
                saida.push('&');
                resto = &resto[1..];
            }
        }
    }
    saida.push_str(resto);
    saida
}

/// Lê o XML; `None` se estiver malformado.
fn ler_xml(xml: &str) -> Option<Elemento> {
    let mut pilha: Vec<Elemento> = vec![Elemento::default()];
    let mut resto = xml;
    while !resto.is_empty() {
        let Some(i) = resto.find('<') else {
            pilha.last_mut()?.texto.push_str(&entidades(resto));
            break;
        };
        if i > 0 {
            let t = &resto[..i];
            if pilha.len() > 1 {
                pilha.last_mut()?.texto.push_str(&entidades(t));
            }
        }
        resto = &resto[i..];
        if let Some(r) = resto.strip_prefix("<!--") {
            resto = &r[r.find("-->")? + 3..];
        } else if let Some(r) = resto.strip_prefix("<![CDATA[") {
            let fim = r.find("]]>")?;
            pilha.last_mut()?.texto.push_str(&r[..fim]);
            resto = &r[fim + 3..];
        } else if resto.starts_with("<?") || resto.starts_with("<!") {
            resto = &resto[resto.find('>')? + 1..];
        } else if let Some(r) = resto.strip_prefix("</") {
            let fim = r.find('>')?;
            let nome = r[..fim].trim();
            let el = pilha.pop()?;
            if el.nome != nome || pilha.is_empty() {
                return None;
            }
            pilha.last_mut()?.filhos.push(el);
            resto = &r[fim + 1..];
        } else {
            let fim = resto.find('>')?;
            let dentro = &resto[1..fim];
            let fecha = dentro.ends_with('/');
            let dentro = dentro.trim_end_matches('/');
            let (nome, mut attrs) = match dentro.find(char::is_whitespace) {
                Some(j) => (&dentro[..j], &dentro[j..]),
                None => (dentro, ""),
            };
            let mut el = Elemento { nome: nome.to_string(), ..Default::default() };
            loop {
                attrs = attrs.trim_start();
                let Some(eq) = attrs.find('=') else { break };
                let chave = attrs[..eq].trim().to_string();
                let v = attrs[eq + 1..].trim_start();
                let aspa = v.chars().next()?;
                if aspa != '"' && aspa != '\'' {
                    return None;
                }
                let fim_v = v[1..].find(aspa)? + 1;
                el.atributos.push((chave, entidades(&v[1..fim_v])));
                attrs = &v[fim_v + 1..];
            }
            if fecha {
                pilha.last_mut()?.filhos.push(el);
            } else {
                pilha.push(el);
            }
            resto = &resto[fim + 1..];
        }
    }
    if pilha.len() != 1 {
        return None;
    }
    pilha.pop()
}

/// Um passo do caminho: eixo (filho ou descendente), nome e predicado.
struct Passo<'a> {
    descendente: bool,
    nome: &'a str,
    predicado: Option<&'a str>,
}

fn passos(xpath: &str) -> Option<Vec<Passo<'_>>> {
    let mut saida = Vec::new();
    let mut resto = xpath.trim();
    if !resto.starts_with('/') {
        return None;
    }
    while !resto.is_empty() {
        let descendente = resto.starts_with("//");
        resto = resto.trim_start_matches('/');
        let mut fim = 0;
        let mut colchete = 0;
        for (i, c) in resto.char_indices() {
            match c {
                '[' => colchete += 1,
                ']' => colchete -= 1,
                '/' if colchete == 0 => break,
                _ => {}
            }
            fim = i + c.len_utf8();
        }
        let parte = &resto[..fim];
        resto = &resto[fim..];
        let (nome, predicado) = match parte.find('[') {
            Some(i) => (&parte[..i], Some(parte[i + 1..].strip_suffix(']')?)),
            None => (parte, None),
        };
        saida.push(Passo { descendente, nome, predicado });
    }
    Some(saida)
}

fn passa(el: &Elemento, pred: &str) -> bool {
    let pred = pred.trim();
    if let Some(attr) = pred.strip_prefix('@') {
        return match attr.split_once('=') {
            Some((k, v)) => {
                let v = v.trim().trim_matches(['"', '\'']);
                el.atributos.iter().any(|(a, x)| a == k.trim() && x == v)
            }
            None => el.atributos.iter().any(|(a, _)| a == attr),
        };
    }
    match pred.split_once('=') {
        Some((filho, v)) => {
            let v = v.trim().trim_matches(['"', '\'']);
            el.filhos.iter().any(|f| f.nome == filho.trim() && f.texto_todo() == v)
        }
        None => el.filhos.iter().any(|f| f.nome == pred),
    }
}

fn filtrar_xml(raiz: &Elemento, xpath: &str) -> Option<Vec<String>> {
    let passos = passos(xpath)?;
    let mut atuais: Vec<&Elemento> = vec![raiz];
    for (i, p) in passos.iter().enumerate() {
        let ultimo = i + 1 == passos.len();
        if ultimo && p.nome == "text()" {
            return Some(atuais.iter().map(|e| e.texto.clone()).collect());
        }
        if let Some(attr) = p.nome.strip_prefix('@') {
            return ultimo.then(|| {
                atuais.iter().filter_map(|e| e.atributos.iter().find(|(a, _)| a == attr).map(|(_, v)| v.clone())).collect()
            });
        }
        let mut proximos = Vec::new();
        for e in &atuais {
            let mut candidatos = Vec::new();
            if p.descendente {
                e.descendentes(&mut candidatos);
            } else {
                candidatos.extend(e.filhos.iter());
            }
            let candidatos: Vec<&Elemento> = candidatos.into_iter().filter(|c| p.nome == "*" || c.nome == p.nome).collect();
            match p.predicado {
                Some(pr) if pr.trim().parse::<usize>().is_ok() => {
                    let k: usize = pr.trim().parse().ok()?;
                    proximos.extend(candidatos.get(k.wrapping_sub(1)).copied());
                }
                Some(pr) if pr.trim() == "last()" => proximos.extend(candidatos.last().copied()),
                Some(pr) => proximos.extend(candidatos.into_iter().filter(|c| passa(c, pr))),
                None => proximos.extend(candidatos),
            }
        }
        atuais = proximos;
    }
    Some(atuais.iter().map(|e| e.texto_todo()).collect())
}

/// `=FILTROXML(xml; xpath)` / `=FILTERXML(xml, xpath)`
pub fn fn_filterxml(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
        let xml = arg_texto(args, ev, 0)?;
        let caminho = arg_texto(args, ev, 1)?;
        let raiz = ler_xml(&xml).ok_or_else(e_valor)?;
        let achados = filtrar_xml(&raiz, &caminho).ok_or_else(e_valor)?;
        if achados.is_empty() {
            return Err(e_valor());
        }
        let celula = |t: String| match t.trim().parse::<f64>() {
            Ok(n) if !t.trim().is_empty() => Value::Number(n),
            _ => Value::Text(t),
        };
        montar_matriz(achados.into_iter().map(|t| vec![celula(t)]).collect())
    })
}

// --- sem servidor -------------------------------------------------------------

macro_rules! sem_servico {
    ($($(#[$m:meta])* $nome:ident => $motivo:expr;)*) => {
        $(
            $(#[$m])*
            pub fn $nome(_args: &[Node], _ev: &Evaluator<'_>) -> Computed {
                Computed::Scalar(Value::Error(CellError::com_motivo(ErrorCode::Na, $motivo)))
            }
        )*
    };
}

const OLAP: &str = "Função de cubo OLAP: precisa de conexão com um servidor de dados, que o FeatherSheet não faz";

sem_servico! {
    /// `=MEMBROKPICUBO(…)` / `=CUBEKPIMEMBER(connection, kpi_name, kpi_property, [caption])`
    fn_cubekpimember => OLAP;
    /// `=MEMBROCUBO(…)` / `=CUBEMEMBER(connection, member_expression, [caption])`
    fn_cubemember => OLAP;
    /// `=PROPRIEDADEMEMBROCUBO(…)` / `=CUBEMEMBERPROPERTY(connection, member_expression, property)`
    fn_cubememberproperty => OLAP;
    /// `=MEMBROCLASSIFICADOCUBO(…)` / `=CUBERANKEDMEMBER(connection, set_expression, rank, [caption])`
    fn_cuberankedmember => OLAP;
    /// `=CONJUNTOCUBO(…)` / `=CUBESET(connection, set_expression, [caption], [sort_order], [sort_by])`
    fn_cubeset => OLAP;
    /// `=CONTAGEMCONJUNTOCUBO(…)` / `=CUBESETCOUNT(set)`
    fn_cubesetcount => OLAP;
    /// `=VALORCUBO(…)` / `=CUBEVALUE(connection, [member_expression1], …)`
    fn_cubevalue => OLAP;
    /// `=RTD(id_prog; servidor; tópico1; …)` / `=RTD(progID, server, topic1, …)`
    fn_rtd => "RTD lê de um servidor COM em tempo real, que o FeatherSheet não executa";
    /// `=CHAMAR(…)` / `=CALL(…)`
    fn_call => "CHAMAR executa código de uma DLL: o FeatherSheet não roda código de fora da planilha";
    /// `=REGISTRO.ID(…)` / `=REGISTER.ID(…)`
    fn_register_id => "REGISTRO.ID registra código de uma DLL: o FeatherSheet não roda código de fora da planilha";
    /// `=HISTÓRICODEAÇÕES(…)` / `=STOCKHISTORY(stock, start_date, …)`
    fn_stockhistory => "Cotações vêm de um serviço na internet; use IMPORTARDADOS com a URL de uma fonte de cotações";
    /// `=TRADUZIR(texto; [origem]; [destino])` / `=TRANSLATE(text, [source_language], [target_language])`
    fn_translate => "Tradução depende de serviço na internet: o texto da planilha não sai do computador";
    /// `=DETECTARIDIOMA(texto)` / `=DETECTLANGUAGE(text)`
    fn_detectlanguage => "Detecção de idioma depende de serviço na internet: o texto da planilha não sai do computador";
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::{ErrorCode, Value};
    use crate::funcoes::fn_helpers::apoio_teste::*;

    #[test]
    fn largura_dos_caracteres() {
        is_text(calc("ASC(\"ＡＢＣ　１２\")"), "ABC 12");
        is_text(calc("DBCS(\"AB 1\")"), "ＡＢ　１");
        is_text(calc_cel(&[("A1", Value::Text("東京".into()))], "FONÉTICA(A1)"), "東京");
    }

    #[test]
    fn baht_por_extenso() {
        is_text(calc("BAHTTEXT(1234)"), "หนึ่งพันสองร้อยสามสิบสี่บาทถ้วน");
        is_text(calc("BAHTTEXT(21,5)"), "ยี่สิบเอ็ดบาทห้าสิบสตางค์");
        is_text(calc("BAHTTEXT(11)"), "สิบเอ็ดบาทถ้วน");
        is_text(calc("BAHTTEXT(0)"), "ศูนย์บาทถ้วน");
        is_text(calc("BAHTTEXT(1000000)"), "หนึ่งล้านบาทถ้วน");
    }

    #[test]
    fn euroconvert_pelas_taxas_fixas() {
        close(calc("EUROCONVERT(1;\"FRF\";\"DEM\")"), 0.3);
        close(calc("EUROCONVERT(1;\"EUR\";\"PTE\")"), 200.0);
        close(calc("EUROCONVERT(100;\"DEM\";\"EUR\")"), 51.13);
        is_err(calc("EUROCONVERT(1;\"USD\";\"EUR\")"), ErrorCode::Value);
    }

    #[test]
    fn filtroxml_com_caminhos_simples() {
        let xml = "\"<a><b n='1'>x</b><b n='2'>7</b><c><b>z</b></c></a>\"";
        is_text(calc(&format!("FILTROXML({xml};\"/a/b[1]\")")), "x");
        close(calc(&format!("FILTROXML({xml};\"/a/b[@n='2']\")")), 7.0);
        close(calc(&format!("FILTROXML({xml};\"//b/@n\")")), 1.0);
        let m = calc_matriz(&format!("FILTERXML({xml};\"//b\")"));
        assert_eq!(m.len(), 3);
        is_err(calc(&format!("FILTROXML({xml};\"/a/zz\")")), ErrorCode::Value);
        is_err(calc("FILTROXML(\"<a><b></a>\";\"/a\")"), ErrorCode::Value);
    }

    #[test]
    fn sem_servidor_da_nd() {
        is_err(calc("VALORCUBO(\"x\";\"y\")"), ErrorCode::Na);
        is_err(calc("TRADUZIR(\"olá\")"), ErrorCode::Na);
    }
}
