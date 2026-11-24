//! `Sheet::remapear_estilos`: trocar o tema da pasta troca a cor do texto, do fundo e da borda, e a fonte, em
//! todos os estilos da aba; o que não bate fica.
use feathersheet_core::celulas::estilo_parcial::Campo;
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::planilha::sheet::Sheet;

#[test]
fn troca_cores_e_fontes_de_todos_os_estilos() {
    let mut s = Sheet::new("A");
    let borda: std::collections::BTreeMap<String, Campo> =
        [("top".to_string(), Campo::Logico(true)), ("color".to_string(), Campo::Texto("#4472c4".into()))].into();
    s.set_style(&Rect::new(0, 0, 0, 0), &[("bg".to_string(), Campo::Texto("#4472c4".into())),
        ("color".to_string(), Campo::Texto("#ffffff".into())), ("fontFamily".to_string(), Campo::Texto("Georgia".into()))].into());
    s.set_style(&Rect::new(1, 0, 1, 0), &[("borders".to_string(), Campo::Objeto(borda))].into());
    s.set_style(&Rect::new(2, 0, 2, 0), &[("bg".to_string(), Campo::Texto("#123456".into()))].into());
    let geracao = s.geracao();
    let n = s.remapear_estilos(
        &|c: &str| (c.eq_ignore_ascii_case("#4472c4")).then(|| "#2e7d32".to_string()),
        &|f: &str| (f == "Georgia").then(|| "Verdana".to_string()),
    );
    assert_eq!(n, 2, "a terceira não bate com nada");
    let a1 = s.get_style(0, 0).unwrap();
    assert_eq!((a1.bg.as_deref(), a1.color.as_deref(), a1.font_family.as_deref()), (Some("#2e7d32"), Some("#ffffff"), Some("Verdana")));
    assert_eq!(s.get_style(1, 0).unwrap().borders.as_ref().unwrap().color.as_deref(), Some("#2e7d32"));
    assert_eq!(s.get_style(2, 0).unwrap().bg.as_deref(), Some("#123456"));
    assert_ne!(s.geracao(), geracao);
    assert_eq!(s.remapear_estilos(&|_: &str| None, &|_: &str| None), 0);
}
