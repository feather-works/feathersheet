//! Minigráficos do `.xlsx`: `<extLst><ext><x14:sparklineGroups>` da aba (o
//! leitor de XML tira os prefixos `x14:`/`xm:`). Cada `sparklineGroup` vira um
//! `Grupo`; a faixa de dados da própria aba perde o nome da aba, para andar
//! junto ao inserir/excluir linhas.
use feathersheet_core::celulas::refs::parse_cell_ref;
use feathersheet_core::operacoes::minigraficos::{partir_dados, Grupo, Minigrafico, Tipo};

use super::xml::XmlNode;

/// `FF376092` → `#376092`. shortcut: cor por tema (`theme`/`tint`) fica com a cor padrão; ler o tema se aparecer arquivo assim.
fn cor(no: &XmlNode, nome: &str) -> Option<String> {
    let rgb = no.child(nome)?.attr("rgb")?;
    let rgb = if rgb.len() == 8 { &rgb[2..] } else { rgb };
    (rgb.len() == 6 && rgb.bytes().all(|b| b.is_ascii_hexdigit())).then(|| format!("#{}", rgb.to_ascii_uppercase()))
}

pub(super) fn ler(root: &XmlNode, aba: &str) -> Vec<Grupo> {
    let grupos = root.child("extLst").into_iter()
        .flat_map(|e| e.children_named("ext"))
        .flat_map(|e| e.children_named("sparklineGroups"))
        .flat_map(|g| g.children_named("sparklineGroup"));
    let mut saida = Vec::new();
    for g in grupos {
        let tipo = match g.attr("type") {
            Some("column") => Tipo::Coluna,
            Some("stacked") => Tipo::GanhosPerdas,
            _ => Tipo::Linha,
        };
        let itens: Vec<Minigrafico> = g.child("sparklines").into_iter()
            .flat_map(|s| s.children_named("sparkline"))
            .filter_map(|s| {
                let (linha, coluna) = parse_cell_ref(&s.child("sqref")?.text.replace('$', ""))?;
                let texto = s.child("f")?.text.replace('$', "");
                let dados = match partir_dados(texto.trim())? {
                    (Some(nome), r) if nome == aba => feathersheet_core::celulas::refs::format_ref(&r),
                    _ => texto.trim().to_owned(),
                };
                Some(Minigrafico { linha, coluna, dados })
            })
            .collect();
        if itens.is_empty() { continue; }
        let sim = |nome| matches!(g.attr(nome), Some("1") | Some("true"));
        let mut grupo = Grupo::novo(tipo, itens);
        grupo.marcadores = sim("markers");
        grupo.alto = sim("high");
        grupo.baixo = sim("low");
        grupo.primeiro = sim("first");
        grupo.ultimo = sim("last");
        grupo.negativos = sim("negative");
        grupo.eixo_igual = g.attr("minAxisType") == Some("group") || g.attr("maxAxisType") == Some("group");
        for (campo, nome) in [
            (&mut grupo.cor, "colorSeries"), (&mut grupo.cor_negativo, "colorNegative"),
            (&mut grupo.cor_marcadores, "colorMarkers"), (&mut grupo.cor_alto, "colorHigh"),
            (&mut grupo.cor_baixo, "colorLow"), (&mut grupo.cor_primeiro, "colorFirst"),
            (&mut grupo.cor_ultimo, "colorLast"),
        ] {
            if let Some(c) = cor(g, nome) { *campo = c; }
        }
        saida.push(grupo);
    }
    saida
}
