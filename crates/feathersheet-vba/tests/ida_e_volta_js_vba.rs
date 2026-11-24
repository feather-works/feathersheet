//! Ida e volta sem Excel: JS → VBA (o tradutor) → JS (o conversor) e os dois JS rodados no motor das macros
//! dão as mesmas células. Pega VBA inválido e diferença de sentido (dinheiro, decimal, arredondamento, laços).
use feathersheet_core::planilha::workbook::Workbook;

fn roda(js: &str) -> Workbook {
    let mut wb = Workbook::new();
    wb.escrever_celula(0, 0, 1, "1234,5".into()); // B1 = 1234,5 (pt-BR)
    wb.escrever_celula(0, 1, 1, "7".into());
    let r = feathersheet_macros::executar(&mut wb, 0, js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    wb
}

fn ida_e_volta(js: &str, celulas: &[(u32, u32)]) {
    let t = feathersheet_vba::js_para_vba::traduzir(js, "Teste", &|f: &str| feathersheet_io::xlsx_write::formula_to_excel(f));
    assert!(t.pendencias.is_empty(), "{:?}\n{}", t.pendencias, t.vba);
    let c = feathersheet_vba::converter_texto(&[("Modulo1", &t.vba)], &[]);
    assert!(c.pendencias.iter().all(|p| p.aviso), "o VBA gerado não converteu de volta: {:#?}\n{}", c.pendencias, t.vba);
    let volta = &c.macros.iter().find(|m| m.nome == "Teste").expect("Sub Teste").codigo;
    let (a, b) = (roda(js), roda(volta));
    for &(r, col) in celulas {
        assert_eq!(a.valor(0, r, col), b.valor(0, r, col), "célula ({r},{col}) difere\nVBA:\n{}\nJS de volta:\n{volta}", t.vba);
    }
}

#[test]
fn contas_com_dinheiro_e_arredondamento_batem() {
    ida_e_volta(r#"
        const preco = sheet.get("B1");
        const qtd = sheet.get("B2");
        sheet.set("C1", preco * qtd);
        sheet.set("C2", Math.round(preco * 1.1));
        sheet.set("C3", Math.floor(-2.5));
        sheet.set("C4", "Total: " + (preco * qtd));
        let soma = 0;
        for (let i = 1; i <= 5; i++) { soma += i * 0.1; }
        sheet.set("C5", soma);
        if (preco > 1000) { sheet.set("C6", "caro"); } else { sheet.set("C6", "barato"); }
    "#, &[(0, 2), (1, 2), (2, 2), (3, 2), (4, 2), (5, 2)]);
}

#[test]
fn texto_e_funcoes_batem() {
    ida_e_volta(r#"
        function Dobro(x) { return x * 2; }
        const nome = "  Ana Maria  ";
        sheet.set("D1", nome.trim().toUpperCase());
        sheet.set("D2", Dobro(21));
        sheet.set("D3", nome.includes("Maria"));
        sheet.set("D4", "abc".replace("b", "X"));
        let n = 0;
        while (n < 3) { n++; }
        sheet.set("D5", n);
    "#, &[(0, 3), (1, 3), (2, 3), (3, 3), (4, 3)]);
}
