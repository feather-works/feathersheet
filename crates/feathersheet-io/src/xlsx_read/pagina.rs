//! `printOptions`, `pageMargins`, `pageSetup`, `headerFooter` e `sheetPr/pageSetUpPr` da aba
//! para o [`crate::pagina_aba::PaginaAba`]. As áreas e os títulos vêm dos nomes definidos.
use super::xml::XmlNode;
use crate::pagina_aba::{texto_simples, PaginaAba};

const PAPEIS: [u16; 9] = [9, 1, 5, 8, 11, 12, 13, 7, 3];

/// `None` quando a aba não tem nada de página (arquivo de outro programa).
pub(super) fn ler(root: &XmlNode) -> Option<PaginaAba> {
    let (opcoes, margens, setup, cab) = (root.child("printOptions"), root.child("pageMargins"),
        root.child("pageSetup"), root.child("headerFooter"));
    if opcoes.is_none() && margens.is_none() && setup.is_none() && cab.is_none() { return None; }
    let sim = |n: Option<&XmlNode>, a: &str| n.and_then(|n| n.attr(a)).is_some_and(|v| v == "1" || v == "true");
    let num = |n: Option<&XmlNode>, a: &str| n.and_then(|n| n.attr(a)).and_then(|v| v.parse::<f64>().ok());
    let mut p = PaginaAba {
        // Sem `printOptions`, o Excel não imprime grade nem títulos.
        grade: Some(sim(opcoes, "gridLines")),
        cabecalhos: Some(sim(opcoes, "headings")),
        ..PaginaAba::default()
    };
    if margens.is_some() {
        let mm = |a: &str, padrao: f64| (num(margens, a).unwrap_or(padrao).clamp(0.0, 20.0) * 25.4) as f32;
        p.margens = Some([mm("left", 0.7), mm("right", 0.7), mm("top", 0.75), mm("bottom", 0.75),
            mm("header", 0.3), mm("footer", 0.3)]);
    }
    if let Some(papel) = num(setup, "paperSize").map(|v| v as u16).filter(|v| PAPEIS.contains(v)) { p.papel = papel; }
    p.paisagem = setup.and_then(|n| n.attr("orientation")) == Some("landscape");
    if let Some(e) = num(setup, "scale") { p.escala = (e as u16).clamp(10, 400); }
    let ajustar = root.child("sheetPr").and_then(|n| n.child("pageSetUpPr"));
    if sim(ajustar, "fitToPage") {
        let pag = |a: &str| num(setup, a).map_or(1, |v| v.clamp(0.0, 32767.0) as u16);
        p.ajustar = Some([pag("fitToWidth"), pag("fitToHeight")]);
    }
    if let Some(cab) = cab {
        let texto = |nome: &str| cab.child(nome).map(|n| n.text.clone()).filter(|t| !t.is_empty());
        p.xlsx_cabecalho = texto("oddHeader");
        p.xlsx_rodape = texto("oddFooter");
        p.cabecalho = p.xlsx_cabecalho.as_deref().map(texto_simples).unwrap_or_default();
        p.rodape = p.xlsx_rodape.as_deref().map(texto_simples).unwrap_or_default();
    }
    Some(p)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn le_a_pagina_do_excel() {
        let raiz = super::super::xml::parse_xml(r#"<worksheet><sheetPr><pageSetUpPr fitToPage="1"/></sheetPr><sheetData/>
            <printOptions gridLines="1"/><pageMargins left="0.25" right="0.25" top="0.75" bottom="0.75" header="0.3" footer="0.3"/>
            <pageSetup paperSize="8" orientation="landscape" fitToHeight="0"/>
            <headerFooter><oddHeader>&amp;CVendas</oddHeader></headerFooter></worksheet>"#).unwrap();
        let p = ler(&raiz).unwrap();
        assert_eq!((p.papel, p.paisagem, p.ajustar, p.grade, p.cabecalhos), (8, true, Some([1, 0]), Some(true), Some(false)));
        assert_eq!(p.margens.unwrap()[0], 6.35);
        assert_eq!((p.cabecalho.as_str(), p.xlsx_cabecalho.as_deref()), ("Vendas", Some("&CVendas")));
        assert!(ler(&super::super::xml::parse_xml("<worksheet><sheetData/></worksheet>").unwrap()).is_none());
    }
}
