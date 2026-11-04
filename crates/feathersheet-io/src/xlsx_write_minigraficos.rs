//! Minigráficos no `.xlsx`: o `<extLst>` da aba com `x14:sparklineGroups`,
//! o caminho de volta do `xlsx_read/minigraficos.rs`.
use std::fmt::Write;

use feathersheet_core::celulas::refs::{cell_name, quote_sheet};
use feathersheet_core::operacoes::minigraficos::{partir_dados, Tipo};
use feathersheet_core::planilha::sheet::Sheet;

use crate::xlsx_write::escape_xml;

const EXT_MINIGRAFICOS: &str = "{05C60535-1F16-4fd2-B633-F4F36F0B64E0}";
const NS_X14: &str = "http://schemas.microsoft.com/office/spreadsheetml/2009/9/main";
const NS_XM: &str = "http://schemas.microsoft.com/office/excel/2006/main";

/// `#376092` → `FF376092` (o Excel guarda ARGB).
fn argb(cor: &str) -> String {
    let hex = cor.trim_start_matches('#');
    if hex.len() == 6 { format!("FF{}", hex.to_ascii_uppercase()) } else { "FF376092".into() }
}

/// O `<extLst>` com os minigráficos da aba, ou `None` se ela não tem.
pub(crate) fn ext_da_aba(sheet: &Sheet) -> Option<String> {
    let grupos = crate::minigraficos_aba::ler(sheet.mg.as_deref());
    if grupos.is_empty() { return None; }
    let propria = quote_sheet(&sheet.name);
    let mut x = format!("<extLst><ext uri=\"{EXT_MINIGRAFICOS}\" xmlns:x14=\"{NS_X14}\"><x14:sparklineGroups xmlns:xm=\"{NS_XM}\">");
    for g in &grupos {
        let tipo = match g.tipo { Tipo::Linha => "", Tipo::Coluna => " type=\"column\"", Tipo::GanhosPerdas => " type=\"stacked\"" };
        let sim = |v: bool, nome: &str| if v { format!(" {nome}=\"1\"") } else { String::new() };
        let eixo = if g.eixo_igual { " minAxisType=\"group\" maxAxisType=\"group\"" } else { "" };
        let _ = write!(x, "<x14:sparklineGroup displayEmptyCellsAs=\"gap\"{tipo}{}{}{}{}{}{}{eixo}>",
            sim(g.marcadores, "markers"), sim(g.alto, "high"), sim(g.baixo, "low"),
            sim(g.primeiro, "first"), sim(g.ultimo, "last"), sim(g.negativos, "negative"));
        // A ordem dos filhos é a do esquema (CT_SparklineGroup).
        for (nome, cor) in [
            ("colorSeries", &g.cor), ("colorNegative", &g.cor_negativo), ("colorAxis", &"#000000".to_string()),
            ("colorMarkers", &g.cor_marcadores), ("colorFirst", &g.cor_primeiro), ("colorLast", &g.cor_ultimo),
            ("colorHigh", &g.cor_alto), ("colorLow", &g.cor_baixo),
        ] {
            let _ = write!(x, "<x14:{nome} rgb=\"{}\"/>", argb(cor));
        }
        x.push_str("<x14:sparklines>");
        for m in &g.itens {
            // O Excel sempre escreve a aba na faixa de dados.
            let dados = match partir_dados(&m.dados) {
                Some((None, _)) => format!("{propria}!{}", m.dados),
                _ => m.dados.clone(),
            };
            let _ = write!(x, "<x14:sparkline><xm:f>{}</xm:f><xm:sqref>{}</xm:sqref></x14:sparkline>",
                escape_xml(&dados), cell_name(m.linha, m.coluna));
        }
        x.push_str("</x14:sparklines></x14:sparklineGroup>");
    }
    x.push_str("</x14:sparklineGroups></ext></extLst>");
    Some(x)
}
