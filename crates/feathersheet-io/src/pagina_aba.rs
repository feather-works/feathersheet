//! A configuração de página da aba (`Sheet::pg`, JSON): o Layout da Página e o diálogo
//! de impressão do app, o `.fshz` e o `.xlsx` (`pageSetup`, `pageMargins`,
//! `printOptions`, `headerFooter`, `_xlnm.Print_Area`/`_xlnm.Print_Titles`).
use feathersheet_core::celulas::refs::{col_name, quote_sheet};
use serde::{Deserialize, Serialize};

/// Última linha/coluna do Excel (área de impressão de coluna/linha inteira).
const ULTIMA_LINHA: u32 = 1_048_575;
const ULTIMA_COLUNA: u32 = 16_383;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PaginaAba {
    /// Código `paperSize` do Excel: 9 A4, 1 Carta, 5 Ofício, 8 A3, 11 A5, 12 B4, 13 B5,
    /// 7 Executivo, 3 Tabloide.
    pub papel: u16,
    pub paisagem: bool,
    /// Porcentagem (vale sem `ajustar`).
    pub escala: u16,
    /// Dimensionar para Ajustar: páginas de largura e de altura (0 = automático).
    pub ajustar: Option<[u16; 2]>,
    /// Em mm: esquerda, direita, topo, base, cabeçalho, rodapé. `None` = margem Normal do app.
    pub margens: Option<[f32; 6]>,
    /// `None` = o padrão do app (ligado).
    pub grade: Option<bool>,
    pub cabecalhos: Option<bool>,
    /// Texto simples do cabeçalho/rodapé, como o app mostra.
    pub cabecalho: String,
    pub rodape: String,
    /// O `oddHeader`/`oddFooter` do `.xlsx` como veio (códigos `&L`, `&P`…): volta igual
    /// se o texto simples não mudou.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xlsx_cabecalho: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xlsx_rodape: Option<String>,
    /// Áreas de impressão `[r0, c0, r1, c1]`, na ordem.
    pub areas: Vec<[u32; 4]>,
    /// Imprimir títulos: linhas e colunas que repetem (`[início, fim]`).
    pub titulos_linhas: Option<[u32; 2]>,
    pub titulos_colunas: Option<[u32; 2]>,
}

impl Default for PaginaAba {
    fn default() -> Self {
        PaginaAba {
            papel: 9, paisagem: false, escala: 100, ajustar: None, margens: None,
            grade: None, cabecalhos: None, cabecalho: String::new(), rodape: String::new(),
            xlsx_cabecalho: None, xlsx_rodape: None, areas: Vec::new(),
            titulos_linhas: None, titulos_colunas: None,
        }
    }
}

/// Lê o `pg` da aba (vazio ou inválido = o padrão).
pub fn ler(json: Option<&str>) -> PaginaAba {
    json.and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default()
}

/// O texto que vai para `Sheet::pg` (`None` quando é tudo padrão).
pub fn gravar(p: &PaginaAba) -> Option<String> {
    if *p == PaginaAba::default() { None } else { serde_json::to_string(p).ok() }
}

/// `&L&"Arial,Negrito"&12Texto&C&P` → `Texto` (o app não tem os códigos).
pub fn texto_simples(codigos: &str) -> String {
    let mut saida = String::new();
    let mut it = codigos.chars().peekable();
    while let Some(c) = it.next() {
        if c != '&' { saida.push(c); continue; }
        match it.next() {
            Some('&') => saida.push('&'),
            Some('"') => { for d in it.by_ref() { if d == '"' { break; } } }
            Some(d) if d.is_ascii_digit() => { while it.peek().is_some_and(|x| x.is_ascii_digit()) { it.next(); } }
            Some('K') => { for _ in 0..6 { it.next(); } }
            _ => {}
        }
    }
    saida.trim().to_string()
}

/// O `oddHeader` a gravar: o original quando o texto não mudou; senão o texto à esquerda.
fn codigos(texto: &str, original: Option<&str>) -> Option<String> {
    if let Some(o) = original.filter(|o| texto_simples(o) == texto.trim()) { return Some(o.to_string()); }
    (!texto.trim().is_empty()).then(|| format!("&L{}", texto.replace('&', "&&")))
}

fn polegadas(mm: f32) -> String {
    let p = (mm as f64 / 25.4 * 10_000.0).round() / 10_000.0;
    format!("{p}")
}

/// `printOptions`, `pageMargins`, `pageSetup` e `headerFooter`, na ordem do CT_Worksheet
/// (depois de `hyperlinks`). `escape` é o do gravador.
pub fn xml_da_aba(p: &PaginaAba, escape: impl Fn(&str) -> String) -> String {
    let mut s = String::new();
    let (grade, cab) = (p.grade.unwrap_or(true), p.cabecalhos.unwrap_or(true));
    if grade || cab {
        s.push_str("<printOptions");
        if cab { s.push_str(" headings=\"1\""); }
        if grade { s.push_str(" gridLines=\"1\""); }
        s.push_str("/>");
    }
    let m = p.margens.unwrap_or([12.0, 12.0, 12.0, 12.0, 7.62, 7.62]);
    s.push_str(&format!("<pageMargins left=\"{}\" right=\"{}\" top=\"{}\" bottom=\"{}\" header=\"{}\" footer=\"{}\"/>",
        polegadas(m[0]), polegadas(m[1]), polegadas(m[2]), polegadas(m[3]), polegadas(m[4]), polegadas(m[5])));
    s.push_str(&format!("<pageSetup paperSize=\"{}\"", p.papel));
    if p.escala != 100 { s.push_str(&format!(" scale=\"{}\"", p.escala.clamp(10, 400))); }
    if let Some([l, a]) = p.ajustar {
        s.push_str(&format!(" fitToWidth=\"{l}\" fitToHeight=\"{a}\""));
    }
    s.push_str(if p.paisagem { " orientation=\"landscape\"/>" } else { " orientation=\"portrait\"/>" });
    let (c, r) = (codigos(&p.cabecalho, p.xlsx_cabecalho.as_deref()), codigos(&p.rodape, p.xlsx_rodape.as_deref()));
    if c.is_some() || r.is_some() {
        s.push_str("<headerFooter>");
        if let Some(c) = c { s.push_str(&format!("<oddHeader>{}</oddHeader>", escape(&c))); }
        if let Some(r) = r { s.push_str(&format!("<oddFooter>{}</oddFooter>", escape(&r))); }
        s.push_str("</headerFooter>");
    }
    s
}

/// O texto do `_xlnm.Print_Area` (`None` sem área).
pub fn texto_area(p: &PaginaAba, aba: &str) -> Option<String> {
    let q = quote_sheet(aba);
    let partes: Vec<String> = p.areas.iter().map(|[r0, c0, r1, c1]| {
        format!("{q}!${}${}:${}${}", col_name(*c0), r0 + 1, col_name(*c1), r1 + 1)
    }).collect();
    (!partes.is_empty()).then(|| partes.join(","))
}

/// O texto do `_xlnm.Print_Titles` (`'Aba'!$1:$2,'Aba'!$A:$B`).
pub fn texto_titulos(p: &PaginaAba, aba: &str) -> Option<String> {
    let q = quote_sheet(aba);
    let mut partes = Vec::new();
    if let Some([a, b]) = p.titulos_colunas { partes.push(format!("{q}!${}:${}", col_name(a), col_name(b))); }
    if let Some([a, b]) = p.titulos_linhas { partes.push(format!("{q}!${}:${}", a + 1, b + 1)); }
    (!partes.is_empty()).then(|| partes.join(","))
}

/// Uma ponta de referência sem aba: `$A$1`, `$A` ou `$1` → (linha, coluna), `None` no que falta.
fn ponta(t: &str) -> Option<(Option<u32>, Option<u32>)> {
    let t = t.replace('$', "");
    let letras: String = t.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let numero = &t[letras.len()..];
    let col = if letras.is_empty() { None } else {
        Some(letras.to_ascii_uppercase().bytes().try_fold(0u32, |n, b| n.checked_mul(26)?.checked_add((b - b'A' + 1) as u32))? - 1)
    };
    let lin = if numero.is_empty() { None } else { Some(numero.parse::<u32>().ok()?.checked_sub(1)?) };
    if col.is_none() && lin.is_none() { return None; }
    Some((lin, col))
}

/// As partes `Aba!X:Y` de um nome definido, sem a aba: `(r0, c0, r1, c1)` com linha/coluna
/// inteira completada.
fn partes(texto: &str) -> Vec<(Option<u32>, Option<u32>, Option<u32>, Option<u32>)> {
    texto.split(',').filter_map(|parte| {
        let ref_ = parte.rsplit_once('!').map_or(parte, |(_, r)| r).trim();
        let (a, b) = ref_.split_once(':').unwrap_or((ref_, ref_));
        let ((r0, c0), (r1, c1)) = (ponta(a)?, ponta(b)?);
        Some((r0, c0, r1, c1))
    }).collect()
}

/// Lê o `_xlnm.Print_Area` para `p.areas`.
pub fn ler_area(p: &mut PaginaAba, texto: &str) {
    p.areas = partes(texto).into_iter().map(|(r0, c0, r1, c1)| [
        r0.unwrap_or(0), c0.unwrap_or(0), r1.unwrap_or(ULTIMA_LINHA), c1.unwrap_or(ULTIMA_COLUNA),
    ]).collect();
}

/// Lê o `_xlnm.Print_Titles`: a parte só de linhas e a só de colunas.
pub fn ler_titulos(p: &mut PaginaAba, texto: &str) {
    for (r0, c0, r1, c1) in partes(texto) {
        match (r0, c0, r1, c1) {
            (Some(a), None, Some(b), None) => p.titulos_linhas = Some([a, b]),
            (None, Some(a), None, Some(b)) => p.titulos_colunas = Some([a, b]),
            _ => {}
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn json_vai_e_volta_e_padrao_nao_grava() {
        assert_eq!(gravar(&PaginaAba::default()), None);
        let p = PaginaAba { papel: 8, paisagem: true, ajustar: Some([1, 0]), areas: vec![[0, 0, 9, 2]], ..Default::default() };
        assert_eq!(ler(gravar(&p).as_deref()), p);
        assert_eq!(ler(Some("lixo")), PaginaAba::default());
        assert_eq!(ler(Some(r#"{"papel":1}"#)).escala, 100, "campo que falta usa o padrão");
    }

    #[test]
    fn codigos_do_cabecalho_viram_texto_e_voltam_iguais() {
        let original = "&L&\"Arial,Negrito\"&12Relatório &&Cia&C&P de &N&R&KFF0000x";
        assert_eq!(texto_simples(original), "Relatório &Cia de x");
        assert_eq!(codigos("Relatório &Cia de x", Some(original)).as_deref(), Some(original));
        assert_eq!(codigos("Novo & texto", Some(original)).as_deref(), Some("&LNovo && texto"));
        assert_eq!(codigos("  ", None), None);
    }

    #[test]
    fn areas_e_titulos_do_nome_definido() {
        let mut p = PaginaAba::default();
        ler_area(&mut p, "'Minha aba'!$A$1:$C$5,'Minha aba'!$E$2:$F$3,'Minha aba'!$H:$H");
        assert_eq!(p.areas, vec![[0, 0, 4, 2], [1, 4, 2, 5], [0, 7, ULTIMA_LINHA, 7]]);
        ler_titulos(&mut p, "Plan1!$A:$B,Plan1!$1:$2");
        assert_eq!((p.titulos_linhas, p.titulos_colunas), (Some([0, 1]), Some([0, 1])));
        assert_eq!(texto_titulos(&p, "Plan1").as_deref(), Some("Plan1!$A:$B,Plan1!$1:$2"));
        p.areas.truncate(2);
        assert_eq!(texto_area(&p, "Minha aba").as_deref(), Some("'Minha aba'!$A$1:$C$5,'Minha aba'!$E$2:$F$3"));
        ler_area(&mut p, "#REF!");
        assert!(p.areas.is_empty());
    }

    #[test]
    fn xml_na_ordem_e_em_polegadas() {
        let p = PaginaAba { papel: 1, paisagem: true, ajustar: Some([1, 0]), grade: Some(false), cabecalhos: Some(false),
            margens: Some([25.4, 25.4, 19.05, 19.05, 7.62, 7.62]), rodape: "a<b".into(), ..Default::default() };
        let x = xml_da_aba(&p, |t| t.replace('<', "&lt;"));
        assert_eq!(x, "<pageMargins left=\"1\" right=\"1\" top=\"0.75\" bottom=\"0.75\" header=\"0.3\" footer=\"0.3\"/>\
<pageSetup paperSize=\"1\" fitToWidth=\"1\" fitToHeight=\"0\" orientation=\"landscape\"/>\
<headerFooter><oddFooter>&La&lt;b</oddFooter></headerFooter>");
    }
}
