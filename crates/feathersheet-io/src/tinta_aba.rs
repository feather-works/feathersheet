//! Tinta da aba Desenhar no FeatherSheet: traços presos às células.
//!
//! Campo `tn` da aba no `.fshz` (JSON, opaco para o motor):
//!
//! ```json
//! {"v":1,"a":{"7":[2,1]},"t":"FTINTA 1\nT 7 c 202124FF 2 1 s 3,4 10.5,12\n"}
//! ```
//!
//! `a` = âncora de cada traço (id → `[linha, coluna]`, base 0); `t` = os
//! traços no formato de texto do `feather-draw::tinta`. Os pontos
//! são pixels lógicos (zoom 100 %) a partir do canto superior esquerdo da
//! célula âncora: o traço anda junto quando colunas/linhas antes dele
//! mudam de tamanho, como as imagens do Excel ("mover com as células").
use std::collections::BTreeMap;

use feather_draw::tinta::{traco_para_drawingml_com_pressao, CurvaPressao, Tinta, Traco, EMU_POR_PIXEL};
use serde_json::{json, Value};

/// Teto de traços por aba (o arquivo não cresce sem limite).
pub const MAX_TRACOS: usize = 4000;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TintaAba {
    pub tinta: Tinta,
    pub ancoras: BTreeMap<u64, (u32, u32)>,
}

impl TintaAba {
    /// Lê o campo `tn`. Vazio/ausente = sem tinta.
    pub fn ler(tn: Option<&str>) -> Result<Self, String> {
        let Some(texto) = tn.filter(|t| !t.trim().is_empty()) else { return Ok(Self::default()) };
        let v: Value = serde_json::from_str(texto).map_err(|e| feathersheet_core::trf!("Tinta da aba inválida: {e}"))?;
        if v["v"].as_u64() != Some(1) {
            return Err(feathersheet_core::tr!("Versão da tinta da aba desconhecida.").into());
        }
        let tinta = Tinta::de_texto(v["t"].as_str().unwrap_or("FTINTA 1\n"))
            .map_err(|e| feathersheet_core::trf!("Tinta da aba inválida: {e}"))?;
        let mut ancoras = BTreeMap::new();
        if let Some(mapa) = v["a"].as_object() {
            for (id, rc) in mapa {
                let (Ok(id), Some(r), Some(c)) = (id.parse::<u64>(), rc[0].as_u64(), rc[1].as_u64()) else {
                    continue;
                };
                if tinta.obter(id).is_some() {
                    ancoras.insert(id, (r.min(u64::from(u32::MAX)) as u32, c.min(u64::from(u32::MAX)) as u32));
                }
            }
        }
        Ok(Self { tinta, ancoras })
    }

    /// Texto do campo `tn`; `None` se não houver traço.
    pub fn gravar(&self) -> Option<String> {
        if self.tinta.is_empty() {
            return None;
        }
        let a: serde_json::Map<String, Value> = self
            .tinta
            .tracos()
            .iter()
            .map(|t| {
                let (r, c) = self.ancora(t.id);
                (t.id.to_string(), json!([r, c]))
            })
            .collect();
        Some(json!({"v": 1, "a": a, "t": self.tinta.para_texto()}).to_string())
    }

    pub fn ancora(&self, id: u64) -> (u32, u32) {
        self.ancoras.get(&id).copied().unwrap_or((0, 0))
    }

    /// Tinta em coordenadas absolutas da aba (pixels lógicos desde A1).
    /// `x_da(coluna)`/`y_da(linha)` dão o início da coluna/linha.
    pub fn para_absoluta(&self, x_da: impl Fn(u32) -> f32, y_da: impl Fn(u32) -> f32) -> Tinta {
        let mut abs = self.tinta.clone();
        for traco in self.tinta.tracos() {
            let (r, c) = self.ancora(traco.id);
            if let Some(t) = abs.obter_mut(traco.id) {
                t.transladar(x_da(c), y_da(r));
            }
        }
        abs
    }

    /// Volta da tinta absoluta: cada traço é preso à célula onde fica o
    /// canto superior esquerdo da sua caixa. `celula_em(x, y)` devolve
    /// `(linha, coluna, x_inicio, y_inicio)` dessa célula.
    pub fn de_absoluta(mut tinta: Tinta, celula_em: impl Fn(f32, f32) -> (u32, u32, f32, f32)) -> Self {
        let mut ancoras = BTreeMap::new();
        let ids: Vec<u64> = tinta.tracos().iter().map(|t| t.id).collect();
        for id in ids {
            let Some(t) = tinta.obter_mut(id) else { continue };
            let Some(caixa) = t.limites() else { continue };
            let (r, c, x0, y0) = celula_em(caixa.x.max(0.0), caixa.y.max(0.0));
            t.transladar(-x0, -y0);
            ancoras.insert(id, (r, c));
        }
        Self { tinta, ancoras }
    }
}

fn traco_xlsx(traco: &Traco, ancora: (u32, u32), n: usize) -> Option<String> {
    let caixa = traco.limites()?;
    // A forma fica presa à célula âncora, deslocada pelo canto da caixa.
    let mut local = traco.clone();
    local.transladar(-caixa.x, -caixa.y);
    // Caneta com pressão sai como o contorno de largura variável (o mesmo da tela).
    let d = traco_para_drawingml_com_pressao(&local, EMU_POR_PIXEL, CurvaPressao::PADRAO)?;
    // O contorno passa um pouco dos pontos (meia espessura): `d.x`/`d.y` ≤ 0.
    let emu = |v: f32| (f64::from(v.max(0.0)) * EMU_POR_PIXEL).round() as i64;
    let nome = format!("Tinta {n}");
    Some(format!(
        "<xdr:oneCellAnchor><xdr:from><xdr:col>{}</xdr:col><xdr:colOff>{}</xdr:colOff><xdr:row>{}</xdr:row><xdr:rowOff>{}</xdr:rowOff></xdr:from>\
         <xdr:ext cx=\"{}\" cy=\"{}\"/><xdr:sp macro=\"\" textlink=\"\"><xdr:nvSpPr><xdr:cNvPr id=\"{}\" name=\"{nome}\"/><xdr:cNvSpPr/></xdr:nvSpPr>\
         <xdr:spPr>{}</xdr:spPr></xdr:sp><xdr:clientData/></xdr:oneCellAnchor>",
        ancora.1,
        (emu(caixa.x) + d.x).max(0),
        ancora.0,
        (emu(caixa.y) + d.y).max(0),
        d.cx,
        d.cy,
        n + 1,
        d.geometria
    ))
}

/// A parte `xl/drawings/drawingN.xml` com os traços como formas livres
/// (`a:custGeom`), que o Excel abre e deixa editar. `None` sem tinta.
pub fn desenho_xlsx(tn: Option<&str>) -> Option<String> {
    let aba = TintaAba::ler(tn).ok()?;
    let formas: String = aba
        .tinta
        .tracos()
        .iter()
        .enumerate()
        .filter_map(|(i, t)| traco_xlsx(t, aba.ancora(t.id), i + 1))
        .collect();
    if formas.is_empty() {
        return None;
    }
    Some(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<xdr:wsDr xmlns:xdr=\"http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing\" xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">{formas}</xdr:wsDr>"
    ))
}

#[cfg(test)]
mod testes {
    use super::*;
    use feather_draw::tinta::{Ferramenta, PontoTinta};
    use feather_draw::Cor;

    fn traco(pts: &[(f32, f32)]) -> Traco {
        Traco::novo(
            pts.iter().map(|&(x, y)| PontoTinta::novo(x, y)).collect(),
            Cor::rgb(0x20, 0x21, 0x24),
            2.0,
            Ferramenta::Caneta,
        )
    }

    /// Colunas de 100 px e linhas de 20 px.
    fn celula(x: f32, y: f32) -> (u32, u32, f32, f32) {
        let (c, r) = ((x / 100.0).floor() as u32, (y / 20.0).floor() as u32);
        (r, c, c as f32 * 100.0, r as f32 * 20.0)
    }

    #[test]
    fn ancora_na_celula_e_volta_igual() {
        let mut abs = Tinta::nova();
        abs.adicionar(traco(&[(250.0, 45.0), (300.0, 90.0)]));
        let aba = TintaAba::de_absoluta(abs.clone(), celula);
        let id = aba.tinta.tracos()[0].id;
        assert_eq!(aba.ancora(id), (2, 2));
        assert_eq!(aba.tinta.tracos()[0].pontos[0].ponto(), feather_draw::Ponto::novo(50.0, 5.0));
        let texto = aba.gravar().unwrap();
        let lida = TintaAba::ler(Some(&texto)).unwrap();
        assert_eq!(lida, aba);
        // Coluna B mais larga (+30 px): o traço anda junto.
        let movida = lida.para_absoluta(|c| c as f32 * 100.0 + if c >= 2 { 30.0 } else { 0.0 }, |r| r as f32 * 20.0);
        assert_eq!(movida.tracos()[0].pontos[0].ponto(), feather_draw::Ponto::novo(280.0, 45.0));
        assert_eq!(TintaAba::ler(None).unwrap(), TintaAba::default());
        assert!(TintaAba::default().gravar().is_none());
        assert!(TintaAba::ler(Some("{\"v\":9}")).is_err());
    }

    #[test]
    fn desenho_xlsx_tem_uma_forma_por_traco() {
        let mut abs = Tinta::nova();
        abs.adicionar(traco(&[(250.0, 45.0), (300.0, 90.0), (320.0, 60.0)]));
        abs.adicionar(traco(&[(10.0, 10.0), (60.0, 10.0)]));
        let tn = TintaAba::de_absoluta(abs, celula).gravar();
        let xml = desenho_xlsx(tn.as_deref()).unwrap();
        assert_eq!(xml.matches("<xdr:oneCellAnchor>").count(), 2);
        assert!(xml.contains("<xdr:col>2</xdr:col><xdr:colOff>476250</xdr:colOff><xdr:row>2</xdr:row><xdr:rowOff>47625</xdr:rowOff>"));
        assert!(xml.contains("<a:custGeom>"));
        assert!(desenho_xlsx(None).is_none());
    }

    #[test]
    fn caneta_com_pressao_vai_ao_xlsx_como_contorno_e_guarda_a_pressao() {
        let mut t = traco(&[(250.0, 45.0), (300.0, 50.0), (350.0, 45.0)]);
        for (p, pr) in t.pontos.iter_mut().zip([0.2, 0.9, 0.4]) {
            p.pressao = Some(pr);
        }
        t.espessura = 6.0;
        let mut abs = Tinta::nova();
        abs.adicionar(t);
        let tn = TintaAba::de_absoluta(abs, celula).gravar();
        // A pressão sobrevive ao campo `tn` (e portanto ao .fshz).
        let lida = TintaAba::ler(tn.as_deref()).unwrap();
        assert_eq!(lida.tinta.tracos()[0].pontos[1].pressao, Some(0.9));
        let xml = desenho_xlsx(tn.as_deref()).unwrap();
        assert!(xml.contains("stroke=\"0\"") && xml.contains("<a:close/>") && xml.contains("<a:ln><a:noFill/></a:ln>"), "{xml}");
        // O contorno começa meia espessura antes do primeiro ponto (50 px na coluna C).
        let off: i64 = xml.split("<xdr:colOff>").nth(1).unwrap().split('<').next().unwrap().parse().unwrap();
        assert!(off < 476_250 && off > 476_250 - 4 * 9525, "{off}");
    }

    #[test]
    fn tinta_sobrevive_ao_fshz_e_vai_para_o_xlsx() {
        let mut abs = Tinta::nova();
        abs.adicionar(traco(&[(250.0, 45.0), (300.0, 90.0)]));
        let tn = TintaAba::de_absoluta(abs, celula).gravar();
        let mut wb = feathersheet_core::planilha::workbook::Workbook::new();
        wb.aba_mut(0).unwrap().sheet.alterar_tinta(tn.clone());
        let bytes = crate::da_workbook::pasta_gravar_bytes(&wb, 0).unwrap();
        let aberta = crate::para_workbook::para_workbook(&crate::fshz_read::ler_fshz(&bytes).unwrap());
        let lida = aberta.aba(0).unwrap().sheet.tn.clone();
        assert_eq!(TintaAba::ler(lida.as_deref()).unwrap(), TintaAba::ler(tn.as_deref()).unwrap());
        let partes = crate::xlsx_write::montar_partes(&wb);
        let achar = |nome: &str| partes.iter().find(|(n, _)| n == nome).map(|(_, b)| String::from_utf8_lossy(b).into_owned());
        assert!(achar("xl/worksheets/sheet1.xml").unwrap().ends_with("<drawing r:id=\"rIdTinta\"/></worksheet>"));
        assert!(achar("xl/worksheets/_rels/sheet1.xml.rels").unwrap().contains("../drawings/drawing1.xml"));
        assert!(achar("[Content_Types].xml").unwrap().contains("/xl/drawings/drawing1.xml"));
        assert!(achar("xl/drawings/drawing1.xml").unwrap().contains("<a:custGeom>"));
        // O .xlsx com desenho continua abrindo no leitor.
        let xlsx = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        assert!(crate::xlsx_read::ler_xlsx_workbook(xlsx).is_ok());
    }
}
