//! O desenho das abas (`xl/drawings/drawingN.xml`) como objetos soltos
//! (`Sheet.ob`): formas (`sp`), conectores (`cxnSp`), grupos (`grpSp`, com
//! a transformação dos filhos) e figuras (`pic`), na ordem de empilhamento
//! do arquivo. Gráficos (`graphicFrame`) ficam com `graficos_ancora`; a
//! segmentação de dados (`mc:AlternateContent` com `graphicFrame`) fica de fora.
//!
//! Cores pelo tema do arquivo (`cores::cor_drawingml`, com a guarda da
//! paleta instalada por quem lê), mais o `alpha` do DrawingML.

use super::formas::caminho_relativo;
use super::sheet::read_rels;
use super::xml::{parse_xml, XmlNode};
use super::entry_text;
use crate::objetos_aba::{
    AlinharH, AlinharTexto, AlinharV, Ancora, Aparencia, FonteTexto, Gradiente, ImagemObjeto, ObjetoSolto,
    Regua, Retangulo, Sombra, TipoObjeto,
};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

const EMU_POR_PX: f64 = 9525.0;
/// Largura e altura das colunas/linhas sem tamanho próprio na grade
/// (`feathersheet-gui/src/grade.rs`: `LARGURA_PADRAO`, `ALTURA_PADRAO`).
pub const LARGURA_GRADE: f32 = 122.0;
pub const ALTURA_GRADE: f32 = 28.0;
/// Teto dos bytes de uma figura levada para dentro do `ob`.
const MAX_FIGURA: usize = 4 * 1024 * 1024;

/// Régua de uma aba lida: larguras/alturas em px (zoom 100 %), como a GUI.
#[derive(Debug, Clone, Default)]
pub struct ReguaLida {
    pub larguras: HashMap<u32, f32>,
    pub largura_padrao: f32,
    pub alturas: HashMap<u32, f32>,
    pub altura_padrao: f32,
}

impl ReguaLida {
    fn inicio(mapa: &HashMap<u32, f32>, padrao: f32, i: u32) -> f32 {
        let proprias: f32 = mapa.iter().filter(|(k, _)| **k < i).map(|(_, v)| v - padrao).sum();
        i as f32 * padrao + proprias
    }
    fn em(mapa: &HashMap<u32, f32>, padrao: f32, v: f32) -> u32 {
        // Busca binária pelo início (monótono).
        let (mut lo, mut hi) = (0u32, 1_048_576u32);
        while lo + 1 < hi {
            let meio = lo + (hi - lo) / 2;
            if Self::inicio(mapa, padrao, meio) <= v { lo = meio } else { hi = meio }
        }
        lo
    }
}

impl Regua for ReguaLida {
    fn inicio_coluna(&self, c: u32) -> f32 { Self::inicio(&self.larguras, self.largura_padrao, c) }
    fn inicio_linha(&self, r: u32) -> f32 { Self::inicio(&self.alturas, self.altura_padrao, r) }
    fn coluna_em(&self, x: f32) -> u32 { Self::em(&self.larguras, self.largura_padrao, x).min(16_383) }
    fn linha_em(&self, y: f32) -> u32 { Self::em(&self.alturas, self.altura_padrao, y) }
}

fn numero(no: Option<&XmlNode>) -> f64 {
    no.and_then(|n| n.text.trim().parse::<f64>().ok()).unwrap_or(0.0)
}

fn attr_f(no: Option<&XmlNode>, nome: &str) -> Option<f64> {
    no.and_then(|n| n.attr(nome)).and_then(|v| v.parse::<f64>().ok())
}

fn verdade(v: Option<&str>) -> bool {
    matches!(v, Some("1") | Some("true"))
}

/// Cor de um nó que contém a cor (`solidFill`, `gs`, `fontRef`…) com o
/// `alpha` como `#rrggbbaa` (opaca fica `#rrggbb`).
pub(super) fn cor_alfa(no: &XmlNode) -> Option<String> {
    let base = super::cores::cor_drawingml(no)?;
    let alvo = no.children.iter().find(|c| matches!(c.name.as_str(), "srgbClr" | "schemeClr" | "sysClr" | "prstClr" | "scrgbClr"))?;
    let alfa = alvo.children.iter().find(|c| c.name == "alpha")
        .and_then(|a| a.attr("val")).and_then(|v| v.parse::<f64>().ok()).map(|v| (v / 100_000.0).clamp(0.0, 1.0));
    Some(match alfa {
        Some(a) if a < 0.999 => format!("{}{:02x}", base.to_lowercase(), (a * 255.0).round() as u8),
        _ => base.to_lowercase(),
    })
}

/// Cor de um elemento de cor solto (`srgbClr`…), como no `duotone`.
fn cor_do_elemento(el: &XmlNode) -> Option<String> {
    cor_alfa(&XmlNode { name: "c".into(), attrs: HashMap::new(), children: vec![el.clone()], text: String::new() })
}

/// Preenchimento herdado do grupo (`grpFill`).
#[derive(Clone, Default)]
struct Herdado {
    cor: Option<String>,
    gradiente: Option<Gradiente>,
}

/// O que um `spPr` (ou o `style`) diz do preenchimento.
fn preenchimento(sp: &XmlNode, herdado: &Herdado) -> (Option<String>, Option<Gradiente>) {
    if let Some(pr) = sp.child("spPr") {
        if pr.child("noFill").is_some() { return (None, None); }
        if let Some(s) = pr.child("solidFill") { return (cor_alfa(s), None); }
        if let Some(g) = pr.child("gradFill") {
            let mut paradas: Vec<(f32, String)> = g.child("gsLst").into_iter().flat_map(|l| l.children_named("gs"))
                .filter_map(|p| Some(((attr_f(Some(p), "pos").unwrap_or(0.0) / 100_000.0) as f32, cor_alfa(p)?)))
                .collect();
            paradas.sort_by(|a, b| a.0.total_cmp(&b.0));
            if paradas.is_empty() { return (None, None); }
            let radial = g.child("path").is_some();
            let angulo = attr_f(g.child("lin"), "ang").map(|a| (a / 60_000.0) as f32).unwrap_or(0.0);
            let meio = paradas[paradas.len() / 2].1.clone();
            return (Some(meio), Some(Gradiente { radial, angulo, paradas }));
        }
        if let Some(p) = pr.child("pattFill") { return (p.child("fgClr").and_then(cor_alfa), None); }
        if pr.child("grpFill").is_some() { return (herdado.cor.clone(), herdado.gradiente.clone()); }
        if pr.child("blipFill").is_some() { return (None, None); }
    }
    match sp.child("style").and_then(|s| s.child("fillRef")) {
        Some(r) if r.attr("idx") != Some("0") => (cor_alfa(r), None),
        _ => (None, None),
    }
}

/// Contorno: `spPr/ln` ou o `lnRef` do estilo. Largura em px.
fn contorno(sp: &XmlNode) -> (Option<String>, f32) {
    let ln = sp.child("spPr").and_then(|p| p.child("ln"));
    let ref_ = sp.child("style").and_then(|s| s.child("lnRef")).filter(|r| r.attr("idx") != Some("0"));
    // Larguras do tema Office (lnStyleLst): 6350, 12700, 19050 EMU.
    let do_tema = ref_.and_then(|r| r.attr("idx")).and_then(|i| i.parse::<usize>().ok())
        .map(|i| [6350.0, 12700.0, 19050.0][i.clamp(1, 3) - 1]);
    let largura = attr_f(ln, "w").or(do_tema).map(|w| (w / EMU_POR_PX) as f32).unwrap_or(1.0).max(0.5);
    if let Some(l) = ln {
        if l.child("noFill").is_some() { return (None, 0.0); }
        if let Some(s) = l.child("solidFill") { return (cor_alfa(s), largura); }
        if let Some(g) = l.child("gradFill").and_then(|g| g.child("gsLst")).and_then(|g| g.children_named("gs").next()) {
            return (cor_alfa(g), largura);
        }
    }
    match ref_ {
        Some(r) => (cor_alfa(r), largura),
        None => (None, 0.0),
    }
}

/// Sombra externa do `effectLst`.
fn sombra(sp: &XmlNode) -> Option<Sombra> {
    let s = sp.child("spPr")?.child("effectLst")?.child("outerShdw")?;
    let px = |n: &str| (attr_f(Some(s), n).unwrap_or(0.0) / EMU_POR_PX) as f32;
    let dist = px("dist");
    let dir = (attr_f(Some(s), "dir").unwrap_or(0.0) / 60_000.0).to_radians() as f32;
    let cor = cor_alfa(s).unwrap_or_else(|| "#00000066".into());
    Some(Sombra { cor, desfoque: px("blurRad"), dx: dist * dir.cos(), dy: dist * dir.sin() })
}

/// Geometria: o id gravado e os ajustes no sentido do `feather-shapes`.
fn geometria(pr: Option<&XmlNode>) -> (String, Vec<f32>) {
    let Some(g) = pr.and_then(|p| p.child("prstGeom")) else { return ("rect".into(), Vec::new()) };
    let prst = g.attr("prst").unwrap_or("rect");
    let adj: HashMap<&str, f32> = g.child("avLst").into_iter().flat_map(|l| l.children_named("gd"))
        .filter_map(|gd| {
            let v = gd.attr("fmla")?.strip_prefix("val ")?.trim().parse::<f32>().ok()?;
            Some((gd.attr("name")?, v / 100_000.0))
        })
        .collect();
    let a = |n: &str, padrao: f32| adj.get(n).copied().unwrap_or(padrao).clamp(0.0, 0.5);
    match prst {
        // Raios por canto: sup. esq., sup. dir., inf. dir., inf. esq.
        "round2SameRect" => { let (c, b) = (a("adj1", 0.16667), a("adj2", 0.0)); ("retangulo".into(), vec![c, c, b, b]) }
        "round1Rect" => { let r = a("adj", 0.16667); ("retangulo".into(), vec![0.0, r, 0.0, 0.0]) }
        "round2DiagRect" => { let (p, s) = (a("adj1", 0.16667), a("adj2", 0.0)); ("retangulo".into(), vec![p, s, p, s]) }
        "roundRect" => match adj.get("adj") {
            Some(&v) => { let r = v.clamp(0.0, 0.5); ("roundRect".into(), vec![r, r]) }
            None => ("roundRect".into(), Vec::new()),
        },
        outro => (outro.to_string(), Vec::new()),
    }
}

/// Texto, alinhamento e letra do `txBody`.
fn texto(sp: &XmlNode) -> (Option<String>, AlinharTexto, Option<FonteTexto>) {
    let Some(corpo) = sp.child("txBody") else { return (None, AlinharTexto::default(), None) };
    let corpo_pr = corpo.child("bodyPr");
    let v = match corpo_pr.and_then(|b| b.attr("anchor")) {
        Some("t") => AlinharV::Topo,
        Some("b") => AlinharV::Baixo,
        _ => AlinharV::Meio,
    };
    let mut linhas = Vec::new();
    let mut h = None;
    let mut primeiro_rpr: Option<&XmlNode> = None;
    let mut fim_rpr: Option<&XmlNode> = None;
    for p in corpo.children_named("p") {
        let mut linha = String::new();
        if h.is_none() {
            h = p.child("pPr").and_then(|pp| pp.attr("algn")).map(|a| match a {
                "ctr" => AlinharH::Centro,
                "r" => AlinharH::Direita,
                _ => AlinharH::Esquerda,
            });
        }
        for r in p.children.iter().filter(|c| matches!(c.name.as_str(), "r" | "fld" | "br")) {
            if r.name == "br" { linha.push('\n'); continue; }
            if let Some(t) = r.child("t") {
                linha.push_str(&t.text);
                if primeiro_rpr.is_none() && !t.text.trim().is_empty() { primeiro_rpr = r.child("rPr"); }
            }
        }
        if fim_rpr.is_none() { fim_rpr = p.child("endParaRPr"); }
        linhas.push(linha);
    }
    // Sem `algn` o parágrafo do Office fica à esquerda.
    let alinhar = AlinharTexto { h: h.unwrap_or(AlinharH::Esquerda), v };
    // Espaços nas pontas de cada linha somem; as linhas vazias do fim ficam
    // (sobem o texto centrado, como no Office).
    let t = linhas.iter().map(|l| l.trim_matches(' ')).collect::<Vec<_>>().join("\n");
    if t.trim().is_empty() { return (None, alinhar, None); }
    let rpr = primeiro_rpr.or(fim_rpr);
    let mut fonte = FonteTexto {
        tamanho: attr_f(rpr, "sz").map(|s| (s / 100.0) as f32),
        negrito: verdade(rpr.and_then(|r| r.attr("b"))),
        italico: verdade(rpr.and_then(|r| r.attr("i"))),
        cor: rpr.and_then(|r| r.child("solidFill")).and_then(cor_alfa),
        familia: rpr.and_then(|r| r.child("latin")).and_then(|l| l.attr("typeface"))
            .filter(|f| !f.starts_with('+')).map(str::to_string),
        sem_quebra: corpo_pr.and_then(|b| b.attr("wrap")) == Some("none"),
    };
    if fonte.cor.is_none() {
        fonte.cor = sp.child("style").and_then(|s| s.child("fontRef")).and_then(cor_alfa);
    }
    (Some(t), alinhar, Some(fonte))
}

/// Retângulo de `xfrm` (EMU) → `(x, y, w, h)` em EMU.
fn caixa(xfrm: Option<&XmlNode>, off: &str, ext: &str) -> Option<(f64, f64, f64, f64)> {
    let x = xfrm?;
    let o = x.child(off)?;
    let e = x.child(ext)?;
    Some((attr_f(Some(o), "x")?, attr_f(Some(o), "y")?, attr_f(Some(e), "cx")?, attr_f(Some(e), "cy")?))
}

/// Uma figura do zip como `data:` URI.
fn figura(zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, caminho: &str) -> Option<String> {
    let mime = match caminho.rsplit('.').next()?.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        // EMF/WMF/TIFF/WDP: a GUI não decodifica; fica de fora.
        _ => return None,
    };
    let mut f = zip.by_name(caminho).ok()?;
    if f.size() as usize > MAX_FIGURA { return None; }
    let mut bytes = Vec::with_capacity(f.size() as usize);
    f.read_to_end(&mut bytes).ok()?;
    Some(format!("data:{mime};base64,{}", base64(&bytes)))
}

pub(super) fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for b in bytes.chunks(3) {
        let n = (u32::from(b[0]) << 16) | (u32::from(*b.get(1).unwrap_or(&0)) << 8) | u32::from(*b.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= b.len() { s.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char) } else { s.push('=') }
        }
    }
    s
}

/// Contexto da leitura de um desenho.
struct Leitura<'a> {
    regua: &'a ReguaLida,
    rels: HashMap<String, String>,
    desenho: String,
    pular_com_texto: bool,
    saida: Vec<ObjetoSolto>,
    /// Índice da âncora em leitura (ordem de empilhamento, como o `z` dos gráficos).
    z: usize,
}

fn base(tipo: TipoObjeto, forma: String, ret: Retangulo, regua: &ReguaLida, z: usize) -> ObjetoSolto {
    let mut o = ObjetoSolto::nova_forma(0, &forma, ret, regua);
    o.tipo = tipo;
    // Ordem no desenho do Excel: a GUI põe acima do gráfico o objeto que
    // vem depois dele (o `z` do gráfico mora em `_excel.z`).
    o.extras.insert("z".into(), serde_json::json!(z));
    o
}

fn transformar(o: &mut ObjetoSolto, xfrm: Option<&XmlNode>) {
    let Some(x) = xfrm else { return };
    o.espelhar_h = verdade(x.attr("flipH"));
    o.espelhar_v = verdade(x.attr("flipV"));
    if let Some(r) = x.attr("rot").and_then(|r| r.parse::<f32>().ok()) {
        o.definir_giro(r / 60_000.0);
    }
}

impl Leitura<'_> {
    /// Um elemento do desenho no retângulo `ret` (px absolutos).
    fn elemento(&mut self, zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, no: &XmlNode, ret: Retangulo, herdado: &Herdado) {
        match no.name.as_str() {
            "sp" | "cxnSp" => self.forma(no, ret, herdado),
            "pic" => self.imagem(zip, no, ret),
            "grpSp" => self.grupo(zip, no, ret, herdado),
            "AlternateContent" => {
                // Segmentação de dados e afins: a escolha moderna é um
                // `graphicFrame` (não desenhado aqui); o substituto é um aviso.
                let escolha = no.child("Choice");
                let tem_quadro = escolha.is_some_and(|c| c.children.iter().any(|f| f.name == "graphicFrame"));
                if !tem_quadro {
                    if let Some(c) = escolha.and_then(|c| c.children.iter().find(|f| matches!(f.name.as_str(), "sp" | "grpSp" | "cxnSp" | "pic"))) {
                        self.elemento(zip, c, ret, herdado);
                    }
                }
            }
            _ => {}
        }
    }

    fn forma(&mut self, sp: &XmlNode, ret: Retangulo, herdado: &Herdado) {
        let (t, alinhar, fonte) = texto(sp);
        if self.pular_com_texto && t.is_some() && sp.attr("macro").is_some_and(|m| !m.is_empty()) { return; }
        let pr = sp.child("spPr");
        let (forma, ajustes) = geometria(pr);
        let mut o = base(TipoObjeto::Forma, forma.clone(), ret, self.regua, self.z);
        let (cor, gradiente) = preenchimento(sp, herdado);
        let (linha_cor, espessura) = contorno(sp);
        let linha = sp.name == "cxnSp" || crate::objetos_aba::e_linha(&forma);
        o.ajustes = ajustes;
        o.aparencia = Aparencia {
            preenchimento: if linha { None } else { cor },
            contorno: linha_cor,
            espessura,
            transparencia: 0.0,
        };
        o.gradiente = if linha { None } else { gradiente };
        o.sombra = sombra(sp);
        o.texto = t;
        o.alinhar = alinhar;
        o.fonte = fonte;
        o.macro_nome = sp.attr("macro").map(|m| m.rsplit(['!', '.']).next().unwrap_or(m).to_string()).filter(|m| !m.is_empty());
        if let Some(v) = sp.attr("textlink").filter(|v| !v.is_empty()) {
            o.extras.insert("vinculo".into(), serde_json::Value::String(v.to_string()));
        }
        transformar(&mut o, pr.and_then(|p| p.child("xfrm")));
        self.saida.push(o);
    }

    fn imagem(&mut self, zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, pic: &XmlNode, ret: Retangulo) {
        let Some(blip_fill) = pic.child("blipFill") else { return };
        let Some(blip) = blip_fill.child("blip") else { return };
        let Some(alvo) = blip.attr("r:embed").and_then(|id| self.rels.get(id)) else { return };
        let caminho = caminho_relativo(&self.desenho, alvo);
        let Some(src) = figura(zip, &caminho) else { return };
        let duotom = blip.child("duotone").and_then(|d| {
            let cores: Vec<String> = d.children.iter().filter_map(cor_do_elemento).collect();
            (cores.len() == 2).then(|| [cores[0].clone(), cores[1].clone()])
        });
        let recorte = blip_fill.child("srcRect").map(|r| {
            let f = |n: &str| (attr_f(Some(r), n).unwrap_or(0.0) / 100_000.0) as f32;
            [f("l"), f("t"), f("r"), f("b")]
        }).filter(|r| r.iter().any(|v| *v != 0.0));
        let pr = pic.child("spPr");
        let mut o = base(TipoObjeto::Imagem, "rect".into(), ret, self.regua, self.z);
        o.aparencia = Aparencia { preenchimento: None, contorno: None, espessura: 0.0, transparencia: 0.0 };
        if let Some((c, e)) = pr.and_then(|p| p.child("ln")).filter(|l| l.child("noFill").is_none())
            .and_then(|l| Some((l.child("solidFill").and_then(cor_alfa)?, attr_f(Some(l), "w").unwrap_or(9525.0))))
        {
            o.aparencia.contorno = Some(c);
            o.aparencia.espessura = (e / EMU_POR_PX) as f32;
        }
        o.sombra = sombra(pic);
        o.imagem = Some(ImagemObjeto { src, duotom, recorte });
        transformar(&mut o, pr.and_then(|p| p.child("xfrm")));
        self.saida.push(o);
    }

    /// Grupo: os filhos estão no espaço `chOff`/`chExt`, levado ao `ret`.
    fn grupo(&mut self, zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, g: &XmlNode, ret: Retangulo, herdado: &Herdado) {
        let pr = g.child("grpSpPr");
        let xfrm = pr.and_then(|p| p.child("xfrm"));
        let filhos = caixa(xfrm, "chOff", "chExt").filter(|c| c.2 > 0.0 && c.3 > 0.0);
        // O preenchimento do grupo vale para os filhos com `grpFill`.
        let proprio = pr.map(|p| {
            let falso = XmlNode { name: "sp".into(), attrs: HashMap::new(), children: vec![XmlNode { name: "spPr".into(), ..p.clone() }], text: String::new() };
            let (cor, gradiente) = preenchimento(&falso, herdado);
            Herdado { cor, gradiente }
        }).unwrap_or_else(|| herdado.clone());
        for c in &g.children {
            if !matches!(c.name.as_str(), "sp" | "cxnSp" | "pic" | "grpSp" | "AlternateContent") { continue; }
            let pr_filho = c.child(if c.name == "grpSp" { "grpSpPr" } else { "spPr" });
            let caixa_filho = caixa(pr_filho.and_then(|p| p.child("xfrm")), "off", "ext");
            let r = match (filhos, caixa_filho) {
                (Some((ox, oy, cw, ch)), Some((x, y, w, h))) => {
                    let (sx, sy) = (f64::from(ret.largura) / cw, f64::from(ret.altura) / ch);
                    Retangulo::novo(
                        ret.x + ((x - ox) * sx) as f32,
                        ret.y + ((y - oy) * sy) as f32,
                        (w * sx) as f32,
                        (h * sy) as f32,
                    )
                }
                _ => ret,
            };
            self.elemento(zip, c, r, &proprio);
        }
    }
}

/// Retângulo (px absolutos) de uma âncora do desenho.
fn retangulo_da_ancora(a: &XmlNode, regua: &ReguaLida) -> Option<Retangulo> {
    let canto = |n: &XmlNode| Ancora {
        coluna: numero(n.child("col")).max(0.0) as u32,
        linha: numero(n.child("row")).max(0.0) as u32,
        dx: (numero(n.child("colOff")) / EMU_POR_PX) as f32,
        dy: (numero(n.child("rowOff")) / EMU_POR_PX) as f32,
    };
    let ext = a.child("ext");
    let tamanho = || ((attr_f(ext, "cx")? / EMU_POR_PX) as f32, (attr_f(ext, "cy")? / EMU_POR_PX) as f32).into();
    let tamanho: Option<(f32, f32)> = tamanho();
    match a.name.as_str() {
        "twoCellAnchor" => {
            let (x0, y0) = canto(a.child("from")?).ponto(regua);
            let (x1, y1) = canto(a.child("to")?).ponto(regua);
            Some(Retangulo::entre(x0, y0, x1, y1))
        }
        "oneCellAnchor" => {
            let (x, y) = canto(a.child("from")?).ponto(regua);
            let (w, h) = tamanho?;
            Some(Retangulo::novo(x, y, w, h))
        }
        "absoluteAnchor" => {
            let p = a.child("pos")?;
            let (w, h) = tamanho?;
            Some(Retangulo::novo((attr_f(Some(p), "x")? / EMU_POR_PX) as f32, (attr_f(Some(p), "y")? / EMU_POR_PX) as f32, w, h))
        }
        _ => None,
    }
}

/// Lê um `drawingN.xml` (texto) com as relações dele.
fn ler_desenho(
    zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, caminho: &str, xml: &XmlNode, regua: &ReguaLida, pular_com_texto: bool,
) -> Vec<ObjetoSolto> {
    let (dir, arq) = caminho.rsplit_once('/').unwrap_or(("", caminho));
    let rels = entry_text(zip, &format!("{dir}/_rels/{arq}.rels")).map(|t| read_rels(&t)).unwrap_or_default();
    let mut l = Leitura { regua, rels, desenho: caminho.to_string(), pular_com_texto, saida: Vec::new(), z: 0 };
    for (z, a) in xml.children.iter().filter(|c| c.name.ends_with("Anchor")).enumerate() {
        l.z = z;
        let Some(ret) = retangulo_da_ancora(a, regua) else { continue };
        for c in &a.children {
            l.elemento(zip, c, ret, &Herdado::default());
        }
    }
    l.saida
}

/// As formas, conectores, grupos e figuras de cada aba, na ordem do
/// arquivo. `reguas[i]` é a régua da aba `i` (larguras/alturas já lidas).
/// `pular_com_texto`: no `.xlsm` a forma com macro e texto vira botão
/// (`importar_vba`); fica de fora para não aparecer duas vezes.
pub(super) fn objetos_dos_desenhos(
    zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, reguas: &[ReguaLida], pular_com_texto: bool,
) -> Vec<(usize, Vec<ObjetoSolto>)> {
    let mut saida = Vec::new();
    for (aba, d) in desenhos_por_aba(zip) {
        let Some(regua) = reguas.get(aba) else { continue };
        let Ok(xml) = entry_text(zip, &d).and_then(|t| parse_xml(&t)) else { continue };
        let objetos = ler_desenho(zip, &d, &xml, regua, pular_com_texto);
        if !objetos.is_empty() { saida.push((aba, objetos)); }
    }
    saida
}

/// Teto de linhas que ganham a altura do arquivo numa aba com desenho.
const MAX_LINHAS_PAINEL: u32 = 5_000;

/// `defaultRowHeight` (pt) do `<sheetFormatPr>`, lido do começo da aba.
fn altura_padrao_pt(zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, folha: &str) -> Option<f64> {
    let mut f = zip.by_name(folha).ok()?;
    let mut buf = vec![0u8; 64 * 1024];
    let mut n = 0;
    while n < buf.len() {
        let lido = f.read(&mut buf[n..]).ok()?;
        if lido == 0 { break; }
        n += lido;
    }
    let texto = String::from_utf8_lossy(&buf[..n]);
    let i = texto.find("<sheetFormatPr")?;
    let resto = &texto[i..];
    let fim = resto.find('>')?;
    let tag = &resto[..fim];
    let j = tag.find("defaultRowHeight=\"")? + "defaultRowHeight=\"".len();
    tag[j..].split('"').next()?.parse::<f64>().ok().filter(|v| *v > 0.0)
}

/// Última linha (base 0) citada pelas âncoras de um desenho.
fn ultima_linha_do_desenho(zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, desenho: &str) -> Option<u32> {
    let xml = entry_text(zip, desenho).and_then(|t| parse_xml(&t)).ok()?;
    xml.children.iter().filter(|c| c.name.ends_with("Anchor"))
        .flat_map(|a| [a.child("from"), a.child("to")])
        .flatten()
        .map(|c| numero(c.child("row")).max(0.0) as u32)
        .max()
}

/// Numa aba com desenho (painel, dashboard), as linhas sem altura própria
/// recebem a altura padrão do arquivo (`defaultRowHeight`, 15 pt = 20 px):
/// a régua da grade usa 28 px e esticaria o desenho ancorado em ~33 %.
/// Só até a última linha usada (desenho ou célula), e só em aba pequena.
/// `ultima_celula[i]` = última linha com dado da aba `i`.
pub(super) fn alturas_das_abas_com_desenho(
    zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>, folhas: &[String], ultima_celula: &[u32],
) -> Vec<(usize, f32, u32)> {
    let mut saida: Vec<(usize, f32, u32)> = Vec::new();
    for (aba, d) in desenhos_por_aba(zip) {
        let Some(folha) = folhas.get(aba) else { continue };
        let Some(ultima) = ultima_linha_do_desenho(zip, &d) else { continue };
        let ultima = ultima.max(ultima_celula.get(aba).copied().unwrap_or(0));
        if ultima > MAX_LINHAS_PAINEL { continue; }
        let pt = altura_padrao_pt(zip, folha).unwrap_or(15.0);
        let px = (pt / 0.75).round() as f32;
        match saida.iter_mut().find(|s| s.0 == aba) {
            Some(s) => s.2 = s.2.max(ultima),
            None => saida.push((aba, px, ultima)),
        }
    }
    saida
}

/// `(índice da aba, caminho do desenho)` de cada desenho ligado a uma aba.
pub(super) fn desenhos_por_aba(zip: &mut ZipArchive<Cursor<std::sync::Arc<[u8]>>>) -> Vec<(usize, String)> {
    let mut saida = Vec::new();
    let Ok(book) = entry_text(zip, "xl/workbook.xml").and_then(|t| parse_xml(&t)) else { return saida };
    let rels_book = entry_text(zip, "xl/_rels/workbook.xml.rels").map(|t| read_rels(&t)).unwrap_or_default();
    let Some(abas) = book.child("sheets") else { return saida };
    for (i, aba) in abas.children_named("sheet").enumerate() {
        let Some(alvo) = aba.attr("r:id").and_then(|id| rels_book.get(id)) else { continue };
        let folha = caminho_relativo("xl/workbook.xml", alvo);
        let (dir, arq) = folha.rsplit_once('/').unwrap_or(("", &folha));
        let Ok(rels) = entry_text(zip, &format!("{dir}/_rels/{arq}.rels")) else { continue };
        let Ok(rels_xml) = parse_xml(&rels) else { continue };
        for r in rels_xml.children_named("Relationship") {
            if r.attr("Type").is_some_and(|t| t.ends_with("/drawing")) {
                if let Some(t) = r.attr("Target") { saida.push((i, caminho_relativo(&folha, t))); }
            }
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;

    fn zip_de(arquivos: &[(&str, &[u8])]) -> ZipArchive<Cursor<std::sync::Arc<[u8]>>> {
        let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (nome, dados) in arquivos {
            w.start_file(*nome, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(dados).unwrap();
        }
        ZipArchive::new(Cursor::new(w.finish().unwrap().into_inner().into())).unwrap()
    }

    fn regua() -> ReguaLida {
        ReguaLida { largura_padrao: 64.0, altura_padrao: 20.0, ..Default::default() }
    }

    const PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0x0D, 0x49, 0x48, 0x44, 0x52, 0, 0, 0, 1, 0, 0, 0, 1,
        8, 6, 0, 0, 0, 0x1F, 0x15, 0xC4, 0x89, 0, 0, 0, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0xF8, 0xCF,
        0xC0, 0xF0, 0x1F, 0, 5, 0, 1, 0xFF, 0x89, 0x99, 0x3D, 0x1D, 0, 0, 0, 0, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    const DESENHO: &str = r#"<xdr:wsDr xmlns:xdr="x" xmlns:a="a" xmlns:r="r">
      <xdr:twoCellAnchor>
        <xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
        <xdr:to><xdr:col>10</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>10</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
        <xdr:sp><xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>
          <a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"><a:shade val="30000"/></a:schemeClr></a:gs>
          <a:gs pos="100000"><a:srgbClr val="1572B9"><a:alpha val="50000"/></a:srgbClr></a:gs></a:gsLst><a:lin ang="16200000" scaled="1"/></a:gradFill>
          <a:effectLst><a:outerShdw blurRad="63500" dist="38100" dir="5400000"><a:prstClr val="black"><a:alpha val="40000"/></a:prstClr></a:outerShdw></a:effectLst>
          </xdr:spPr>
          <xdr:style><a:lnRef idx="2"><a:schemeClr val="accent1"/></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent1"/></a:fillRef>
          <a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></xdr:style>
          <xdr:txBody><a:bodyPr anchor="ctr"/><a:p><a:pPr algn="ctr"/><a:r><a:rPr sz="2000" b="1"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:rPr><a:t> R$ 10 </a:t></a:r></a:p><a:p><a:endParaRPr sz="2000"/></a:p></xdr:txBody>
        </xdr:sp><xdr:clientData/></xdr:twoCellAnchor>
      <xdr:twoCellAnchor>
        <xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
        <xdr:to><xdr:col>5</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>5</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
        <xdr:grpSp><xdr:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="100" cy="100"/><a:chOff x="1000" y="1000"/><a:chExt cx="200" cy="400"/></a:xfrm></xdr:grpSpPr>
          <xdr:sp><xdr:spPr><a:xfrm><a:off x="1000" y="1000"/><a:ext cx="200" cy="100"/></a:xfrm><a:prstGeom prst="round2SameRect"><a:avLst><a:gd name="adj1" fmla="val 50000"/><a:gd name="adj2" fmla="val 0"/></a:avLst></a:prstGeom>
            <a:solidFill><a:srgbClr val="002060"/></a:solidFill><a:ln><a:noFill/></a:ln></xdr:spPr></xdr:sp>
          <xdr:pic><xdr:blipFill><a:blip r:embed="rId1"><a:duotone><a:schemeClr val="accent4"/><a:prstClr val="white"/></a:duotone></a:blip></xdr:blipFill>
            <xdr:spPr><a:xfrm><a:off x="1100" y="1200"/><a:ext cx="100" cy="200"/></a:xfrm></xdr:spPr></xdr:pic>
        </xdr:grpSp><xdr:clientData/></xdr:twoCellAnchor>
      <xdr:twoCellAnchor>
        <xdr:from><xdr:col>0</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>0</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
        <xdr:to><xdr:col>3</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>3</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
        <xdr:graphicFrame><a:graphic><a:graphicData><c:chart xmlns:c="c" r:id="rId9"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>
      <xdr:twoCellAnchor>
        <xdr:from><xdr:col>2</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>
        <xdr:to><xdr:col>6</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>
        <xdr:cxnSp><xdr:spPr><a:prstGeom prst="line"/><a:ln w="19050"><a:solidFill><a:schemeClr val="bg1"/></a:solidFill></a:ln></xdr:spPr></xdr:cxnSp><xdr:clientData/></xdr:twoCellAnchor>
    </xdr:wsDr>"#;

    #[test]
    fn formas_grupo_conector_e_figura_na_ordem_do_desenho() {
        let rels = r#"<Relationships><Relationship Id="rId1" Type="t/image" Target="../media/image1.png"/></Relationships>"#;
        let mut zip = zip_de(&[
            ("xl/drawings/drawing1.xml", DESENHO.as_bytes()),
            ("xl/drawings/_rels/drawing1.xml.rels", rels.as_bytes()),
            ("xl/media/image1.png", PNG_1X1),
        ]);
        let xml = parse_xml(DESENHO).unwrap();
        let objs = ler_desenho(&mut zip, "xl/drawings/drawing1.xml", &xml, &regua(), false);
        assert_eq!(objs.len(), 4, "fundo, barra e figura do grupo, conector; o gráfico fica de fora");

        let fundo = &objs[0];
        assert_eq!(fundo.forma, "rect");
        let g = fundo.gradiente.as_ref().unwrap();
        assert_eq!(g.angulo, 270.0);
        assert_eq!(g.paradas.len(), 2);
        // accent1 (4472C4) com shade 30 % (em RGB linear, cores.rs); a segunda com alfa 50 %.
        assert_eq!(g.paradas[0].1, "#14223b");
        assert_eq!(g.paradas[1].1, "#1572b980");
        assert_eq!(fundo.aparencia.contorno.as_deref(), Some("#4472c4"));
        assert!((fundo.aparencia.espessura - 12700.0 / 9525.0).abs() < 1e-3);
        let s = fundo.sombra.as_ref().unwrap();
        assert_eq!(s.cor, "#00000066");
        assert!((s.dy - 4.0).abs() < 1e-3 && s.dx.abs() < 1e-3);
        assert_eq!(fundo.texto.as_deref(), Some("R$ 10\n"));
        assert_eq!(fundo.alinhar, AlinharTexto { h: AlinharH::Centro, v: AlinharV::Meio });
        let f = fundo.fonte.as_ref().unwrap();
        assert_eq!((f.tamanho, f.negrito, f.cor.as_deref()), (Some(20.0), true, Some("#ff0000")));
        assert_eq!((fundo.de, fundo.ate.coluna, fundo.ate.linha), (Ancora::default(), 10, 10));

        // Grupo: âncora B2:F6 = (64, 20) 256 x 80 px; filhos em 200 x 400 EMU.
        let barra = &objs[1];
        assert_eq!((barra.forma.as_str(), barra.ajustes.clone()), ("retangulo", vec![0.5, 0.5, 0.0, 0.0]));
        assert_eq!(barra.aparencia.preenchimento.as_deref(), Some("#002060"));
        assert_eq!(barra.aparencia.contorno, None);
        let r = barra.retangulo(&regua());
        assert_eq!((r.x, r.y, r.largura, r.altura), (64.0, 20.0, 256.0, 20.0));
        let fig = &objs[2];
        assert_eq!(fig.tipo, TipoObjeto::Imagem);
        let im = fig.imagem.as_ref().unwrap();
        assert!(im.src.starts_with("data:image/png;base64,iVBORw0KGgo"));
        assert_eq!(im.duotom.as_ref().map(|d| d[1].as_str()), Some("#ffffff"));
        let r = fig.retangulo(&regua());
        assert_eq!((r.x, r.y, r.largura, r.altura), (192.0, 60.0, 128.0, 40.0));

        let linha = &objs[3];
        assert_eq!((linha.forma.as_str(), linha.aparencia.preenchimento.clone()), ("line", None));
        assert_eq!(linha.aparencia.contorno.as_deref(), Some("#ffffff"));
        assert!((linha.aparencia.espessura - 2.0).abs() < 1e-3);
    }

    #[test]
    fn regua_lida_acha_coluna_e_linha() {
        let mut r = regua();
        r.larguras.insert(1, 100.0);
        r.alturas.insert(0, 200.0);
        assert_eq!(r.inicio_coluna(2), 164.0);
        assert_eq!(r.coluna_em(163.9), 1);
        assert_eq!(r.linha_em(219.0), 1);
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
    }
}
