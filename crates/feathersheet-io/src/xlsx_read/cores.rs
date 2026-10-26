//! Cores do Excel: tema (`xl/theme/theme1.xml`), paleta indexada legada
//! (`indexed="n"`, com o `<indexedColors>` do `styles.xml` por cima), o
//! `tint` aplicado na luminância HSL como o Excel faz, e as cores do
//! DrawingML das formas (`srgbClr`, `schemeClr`, `sysClr` com `lumMod`,
//! `lumOff`, `shade`, `tint`).
//!
//! A paleta do arquivo vale durante a leitura dele: quem abre chama
//! [`instalar`] e segura a guarda até o fim; a guarda devolve a paleta
//! padrão ao sair. Sem guarda (testes de unidade, leitores avulsos) vale o
//! tema Office 2013+.

use super::xml::{parse_xml, XmlNode};
use std::cell::RefCell;

/// Tema na ordem do índice `theme="n"` do SpreadsheetML: lt1, dk1, lt2, dk2,
/// accent1..6, hlink, folHlink (o Excel troca claro/escuro nos dois pares).
pub type Tema = [u32; 12];

/// Office 2013+ (o que o Excel grava quando não há `theme1.xml`).
pub const TEMA_PADRAO: Tema = [
    0xFFFFFF, 0x000000, 0xE7E6E6, 0x44546A, 0x4472C4, 0xED7D31, 0xA5A5A5, 0xFFC000, 0x5B9BD5,
    0x70AD47, 0x0563C1, 0x954F72,
];

/// Paleta indexada padrão (0..=63); 64 = texto do sistema, 65 = fundo.
const INDEXADAS: [u32; 66] = [
    0x000000, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x000000,
    0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0xFF00FF, 0x00FFFF, 0x800000, 0x008000,
    0x000080, 0x808000, 0x800080, 0x008080, 0xC0C0C0, 0x808080, 0x9999FF, 0x993366, 0xFFFFCC,
    0xCCFFFF, 0x660066, 0xFF8080, 0x0066CC, 0xCCCCFF, 0x000080, 0xFF00FF, 0xFFFF00, 0x00FFFF,
    0x800080, 0x800000, 0x008080, 0x0000FF, 0x00CCFF, 0xCCFFFF, 0xCCFFCC, 0xFFFF99, 0x99CCFF,
    0xFF99CC, 0xCC99FF, 0xFFCC99, 0x3366FF, 0x33CCCC, 0x99CC00, 0xFFCC00, 0xFF9900, 0xFF6600,
    0x666699, 0x969696, 0x003366, 0x339966, 0x003300, 0x333300, 0x993300, 0x993366, 0x333399,
    0x333333, 0x000000, 0xFFFFFF,
];

#[derive(Clone)]
struct Paleta {
    tema: Tema,
    indexadas: Vec<u32>,
}

impl Default for Paleta {
    fn default() -> Self {
        Self { tema: TEMA_PADRAO, indexadas: INDEXADAS.to_vec() }
    }
}

thread_local! {
    static ATUAL: RefCell<Paleta> = RefCell::new(Paleta::default());
}

/// Devolve a paleta anterior ao sair.
pub struct Guarda(Option<Paleta>);

impl Drop for Guarda {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            ATUAL.with(|a| *a.borrow_mut() = p);
        }
    }
}

/// Instala o tema e as cores indexadas do arquivo nesta thread.
pub fn instalar(theme_xml: &str, styles_xml: &str) -> Guarda {
    let mut nova = Paleta::default();
    if let Some(t) = ler_tema(theme_xml) {
        nova.tema = t;
    }
    if let Some(cores) = ler_indexadas(styles_xml) {
        for (i, c) in cores.into_iter().enumerate() {
            if let Some(v) = nova.indexadas.get_mut(i) {
                *v = c;
            }
        }
    }
    let antiga = ATUAL.with(|a| std::mem::replace(&mut *a.borrow_mut(), nova));
    Guarda(Some(antiga))
}

/// `<a:clrScheme>` do `theme1.xml`, na ordem do índice `theme="n"`.
pub fn ler_tema(xml: &str) -> Option<Tema> {
    if xml.trim().is_empty() {
        return None;
    }
    let raiz = parse_xml(xml).ok()?;
    let esquema = raiz.child("themeElements")?.child("clrScheme")?;
    let cor = |nome: &str| -> Option<u32> {
        let no = esquema.child(nome)?;
        if let Some(s) = no.child("srgbClr").and_then(|c| c.attr("val")) {
            return u32::from_str_radix(s, 16).ok();
        }
        let sys = no.child("sysClr")?;
        let v = sys.attr("lastClr").or(match sys.attr("val") {
            Some("window") => Some("FFFFFF"),
            Some("windowText") => Some("000000"),
            _ => None,
        })?;
        u32::from_str_radix(v, 16).ok()
    };
    let mut t = TEMA_PADRAO;
    let ordem = ["lt1", "dk1", "lt2", "dk2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"];
    for (i, nome) in ordem.iter().enumerate() {
        if let Some(c) = cor(nome) {
            t[i] = c;
        }
    }
    Some(t)
}

fn ler_indexadas(styles_xml: &str) -> Option<Vec<u32>> {
    if !styles_xml.contains("indexedColors") {
        return None;
    }
    let raiz = parse_xml(styles_xml).ok()?;
    let lista = raiz.child("colors")?.child("indexedColors")?;
    let cores: Vec<u32> = lista
        .children_named("rgbColor")
        .map(|c| c.attr("rgb").filter(|v| v.is_ascii()).map(|v| if v.len() == 8 { &v[2..] } else { v }).and_then(|v| u32::from_str_radix(v, 16).ok()).unwrap_or(0))
        .collect();
    (!cores.is_empty()).then_some(cores)
}

pub fn hex(c: u32) -> String {
    format!("#{:06x}", c & 0xFF_FFFF)
}

/// Cor do índice `theme="n"` do arquivo em leitura.
pub fn do_tema(i: usize) -> Option<u32> {
    ATUAL.with(|a| a.borrow().tema.get(i).copied())
}

/// Cor do índice `indexed="n"`. 64 (automático) e 65 (fundo) entram como
/// preto e branco; quem precisa tratar "automático" checa antes.
pub fn indexada(i: usize) -> Option<u32> {
    ATUAL.with(|a| a.borrow().indexadas.get(i).copied())
}

fn para_hsl(c: u32) -> (f64, f64, f64) {
    let r = ((c >> 16) & 0xFF) as f64 / 255.0;
    let g = ((c >> 8) & 0xFF) as f64 / 255.0;
    let b = (c & 0xFF) as f64 / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-12 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    } / 6.0;
    (h, s, l)
}

fn de_hsl(h: f64, s: f64, l: f64) -> u32 {
    let canal = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    if s <= 0.0 {
        let v = canal(l);
        return (v << 16) | (v << 8) | v;
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 { t += 1.0; }
        if t > 1.0 { t -= 1.0; }
        if t < 1.0 / 6.0 { p + (q - p) * 6.0 * t }
        else if t < 0.5 { q }
        else if t < 2.0 / 3.0 { p + (q - p) * (2.0 / 3.0 - t) * 6.0 }
        else { p }
    };
    (canal(f(h + 1.0 / 3.0)) << 16) | (canal(f(h)) << 8) | canal(f(h - 1.0 / 3.0))
}

/// `tint` do SpreadsheetML: escurece (negativo) ou clareia (positivo) a
/// luminância HSL, como o Excel (`L·(1+t)` ou `L·(1−t) + t`).
pub fn aplicar_tint(c: u32, tint: f64) -> u32 {
    if tint == 0.0 || !tint.is_finite() {
        return c;
    }
    let (h, s, l) = para_hsl(c);
    let l = if tint < 0.0 { l * (1.0 + tint) } else { l * (1.0 - tint) + tint };
    de_hsl(h, s, l)
}

/// `lumMod`/`lumOff` do DrawingML (frações, já divididas por 100000).
fn lum(c: u32, modulo: f64, deslocamento: f64) -> u32 {
    let (h, s, l) = para_hsl(c);
    de_hsl(h, s, (l * modulo + deslocamento).clamp(0.0, 1.0))
}

fn por_canal(c: u32, f: impl Fn(f64) -> f64) -> u32 {
    let ch = |sh: u32| (f(((c >> sh) & 0xFF) as f64 / 255.0).clamp(0.0, 1.0) * 255.0).round() as u32;
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// Cor do DrawingML dentro de `no` (`solidFill`, `gs`, `fillRef`…): o
/// primeiro filho `srgbClr`/`schemeClr`/`sysClr`/`prstClr`/`scrgbClr`, com
/// as transformações.
pub fn cor_drawingml(no: &XmlNode) -> Option<String> {
    let alvo = no.children.iter().find(|c| matches!(c.name.as_str(), "srgbClr" | "schemeClr" | "sysClr" | "prstClr" | "scrgbClr"))?;
    let base = match alvo.name.as_str() {
        "srgbClr" => u32::from_str_radix(alvo.attr("val")?, 16).ok()?,
        "sysClr" => u32::from_str_radix(alvo.attr("lastClr").unwrap_or(if alvo.attr("val") == Some("window") { "FFFFFF" } else { "000000" }), 16).ok()?,
        "schemeClr" => {
            let i = match alvo.attr("val")? {
                "lt1" | "bg1" => 0,
                "dk1" | "tx1" => 1,
                "lt2" | "bg2" => 2,
                "dk2" | "tx2" => 3,
                "accent1" => 4,
                "accent2" => 5,
                "accent3" => 6,
                "accent4" => 7,
                "accent5" => 8,
                "accent6" => 9,
                "hlink" => 10,
                "folHlink" => 11,
                _ => return None,
            };
            do_tema(i)?
        }
        "prstClr" => match alvo.attr("val")? {
            "black" => 0x000000,
            "white" => 0xFFFFFF,
            "red" => 0xFF0000,
            "green" => 0x008000,
            "blue" => 0x0000FF,
            "yellow" => 0xFFFF00,
            _ => return None,
        },
        _ => {
            let p = |n: &str| alvo.attr(n).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100_000.0;
            let ch = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
            (ch(p("r")) << 16) | (ch(p("g")) << 8) | ch(p("b"))
        }
    };
    let mut c = base;
    let mut modulo = 1.0;
    let mut deslocamento = 0.0;
    for t in &alvo.children {
        let v = t.attr("val").and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) / 100_000.0;
        match t.name.as_str() {
            "lumMod" => modulo = v,
            "lumOff" => deslocamento = v,
            "shade" => c = por_canal(c, |x| x * v),
            "tint" => c = por_canal(c, |x| 1.0 - (1.0 - x) * v),
            _ => {}
        }
    }
    if modulo != 1.0 || deslocamento != 0.0 {
        c = lum(c, modulo, deslocamento);
    }
    Some(hex(c))
}

/// Família de cor do estilo de tabela embutido do Excel
/// (`TableStyleLight1`…`TableStyleDark11`) no tema mais próximo do
/// FeatherSheet. A cada sete estilos a cor recomeça: escuro 1 (cinza/preto)
/// e depois ênfase 1 a 6, cuja cor vem do tema do arquivo.
pub fn tema_de_tabela(nome: &str) -> Option<&'static str> {
    let resto = nome.strip_prefix("TableStyle")?;
    let n: usize = resto.trim_start_matches(|c: char| c.is_ascii_alphabetic()).parse().ok()?;
    if n == 0 { return None; }
    let posicao = (n - 1) % 7;
    if posicao == 0 { return Some("cinza"); }
    Some(familia(do_tema(3 + posicao)?))
}

/// Nome da família (azul, verde, laranja, vermelho, roxo, cinza) pela matiz.
fn familia(c: u32) -> &'static str {
    let (h, s, _) = para_hsl(c);
    if s < 0.18 { return "cinza"; }
    match (h * 360.0) as u32 {
        0..=14 | 330..=360 => "vermelho",
        15..=49 => "laranja",
        50..=69 => "laranja",
        70..=169 => "verde",
        170..=259 => "azul",
        _ => "roxo",
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn cor_indexada_com_acento_nao_entra_em_panico() {
        let xml = r#"<styleSheet><colors><indexedColors><rgbColor rgb="Fé12345"/><rgbColor rgb="FF00FF00"/></indexedColors></colors></styleSheet>"#;
        assert_eq!(ler_indexadas(xml), Some(vec![0, 0x00FF00]));
    }

    #[test]
    fn familia_do_estilo_de_tabela() {
        assert_eq!(tema_de_tabela("TableStyleMedium8"), Some("cinza"));
        assert_eq!(tema_de_tabela("TableStyleMedium9"), Some("azul"));
        assert_eq!(tema_de_tabela("TableStyleLight14"), Some("verde"));
        assert_eq!(tema_de_tabela("Personalizado"), None);
    }


    #[test]
    fn tint_na_luminancia_como_o_excel() {
        // Branco −15 % = D9D9D9 (o "Branco, Plano de Fundo 1, Mais Escuro 15%").
        assert_eq!(hex(aplicar_tint(0xFFFFFF, -0.14999847407452621)), "#d9d9d9");
        // Verde-oliva do tema 2007 (9BBB59) +60 % = D7E4BD.
        assert_eq!(hex(aplicar_tint(0x9BBB59, 0.59999389629810485)), "#d7e4bd");
        // Azul do Office 2013 (4472C4) +80 % = DAE3F3 (o Excel grava D9E1F2 em
        // 2016 por arredondar HLS em 0..240; um ponto de diferença é aceito).
        let c = aplicar_tint(0x4472C4, 0.7999816888943144);
        let (r, g, b) = ((c >> 16) as i32, ((c >> 8) & 0xFF) as i32, (c & 0xFF) as i32);
        assert!((r - 0xD9).abs() <= 2 && (g - 0xE1).abs() <= 2 && (b - 0xF2).abs() <= 2, "{}", hex(c));
    }

    #[test]
    fn tema_do_arquivo_troca_claro_e_escuro() {
        let xml = r#"<a:theme xmlns:a="a"><a:themeElements><a:clrScheme name="Office">
          <a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1>
          <a:dk2><a:srgbClr val="1F497D"/></a:dk2><a:lt2><a:srgbClr val="EEECE1"/></a:lt2>
          <a:accent1><a:srgbClr val="4F81BD"/></a:accent1><a:accent2><a:srgbClr val="C0504D"/></a:accent2>
          <a:accent3><a:srgbClr val="9BBB59"/></a:accent3></a:clrScheme></a:themeElements></a:theme>"#;
        let t = ler_tema(xml).unwrap();
        assert_eq!((t[0], t[1], t[2], t[3], t[5], t[6]), (0xFFFFFF, 0, 0xEEECE1, 0x1F497D, 0xC0504D, 0x9BBB59));
        let _g = instalar(xml, "");
        assert_eq!(do_tema(6), Some(0x9BBB59));
    }

    #[test]
    fn indexadas_do_arquivo_e_padrao() {
        assert_eq!(indexada(10), Some(0xFF0000));
        let s = r#"<styleSheet><colors><indexedColors><rgbColor rgb="FF123456"/></indexedColors></colors></styleSheet>"#;
        let g = instalar("", s);
        assert_eq!(indexada(0), Some(0x123456));
        drop(g);
        assert_eq!(indexada(0), Some(0x000000));
    }

    #[test]
    fn cor_de_forma_com_esquema_e_lummod() {
        let x = parse_xml(r#"<f><a:schemeClr val="accent2"><a:lumMod val="50000"/></a:schemeClr></f>"#).unwrap();
        // ED7D31 com metade da luminância: o Excel mostra 843C0C ("Mais
        // escuro 50%"); a conta em ponto flutuante pode cair um ponto abaixo.
        let c = u32::from_str_radix(&cor_drawingml(&x).unwrap()[1..], 16).unwrap();
        let excel = 0x843C0Cu32;
        for sh in [16, 8, 0] {
            assert!((((c >> sh) & 0xFF) as i32 - ((excel >> sh) & 0xFF) as i32).abs() <= 1, "{c:06x}");
        }
        let s = parse_xml(r#"<f><a:srgbClr val="00B050"/></f>"#).unwrap();
        assert_eq!(cor_drawingml(&s).as_deref(), Some("#00b050"));
    }
}
