//! Nomes definidos com valor ou fórmula (`Taxa` = `=0,1`) e de escopo de aba (o `localSheetId` do Excel).

use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_core::Value;

fn num(wb: &Workbook, aba: usize, r: u32, c: u32) -> f64 {
    match wb.valor(aba, r, c) {
        Value::Number(n) => n,
        outro => panic!("esperava número em {aba}:{r},{c}, veio {outro:?}"),
    }
}

#[test]
fn nome_com_valor_e_formula_calcula_e_acompanha_a_origem() {
    let mut wb = Workbook::new();
    let vendas = wb.criar_aba("Vendas");
    wb.escrever_celula(vendas, 0, 0, "200".into());
    wb.definir_nome_formula("Taxa", "=0,1", None);
    wb.definir_nome_formula("Bonus", "=Vendas!$A$1*2", None);
    wb.escrever_celula(0, 0, 0, "1000".into());
    wb.escrever_celula(0, 0, 1, "=A1*Taxa".into());
    wb.escrever_celula(0, 0, 2, "=Bonus+Taxa".into());
    assert!((num(&wb, 0, 0, 1) - 100.0).abs() < 1e-9);
    assert!((num(&wb, 0, 0, 2) - 400.1).abs() < 1e-9);
    // Mudar a origem do nome recalcula quem usa o nome.
    wb.escrever_celula(vendas, 0, 0, "300".into());
    assert!((num(&wb, 0, 0, 2) - 600.1).abs() < 1e-9);
    // Redefinir o nome também.
    wb.definir_nome_formula("Taxa", "=0,25", None);
    assert!((num(&wb, 0, 0, 1) - 250.0).abs() < 1e-9);
}

#[test]
fn nome_da_aba_vence_o_da_pasta() {
    let mut wb = Workbook::new();
    let plan2 = wb.criar_aba("Plan2");
    wb.definir_nome_formula("Meta", "=10", None);
    wb.definir_nome_formula("Meta", "=20", Some("Plan2"));
    wb.escrever_celula(0, 0, 0, "=Meta".into());
    wb.escrever_celula(plan2, 0, 0, "=Meta".into());
    assert_eq!(num(&wb, 0, 0, 0), 10.0);
    assert_eq!(num(&wb, plan2, 0, 0), 20.0);
    assert_eq!(wb.nomes_formula().len(), 2);
}

#[test]
fn nome_de_aba_que_e_intervalo_serve_como_referencia() {
    let mut wb = Workbook::new();
    let plan2 = wb.criar_aba("Plan2");
    for (r, v) in ["5", "7", "11"].iter().enumerate() { wb.escrever_celula(plan2, r as u32, 0, (*v).into()); }
    wb.definir_nome_formula("Itens", "=Plan2!$A$1:$A$3", Some("Plan2"));
    wb.escrever_celula(plan2, 0, 2, "=SOMA(Itens)".into());
    wb.escrever_celula(plan2, 1, 2, "=ÍNDICE(Itens;2)".into());
    assert_eq!(num(&wb, plan2, 0, 2), 23.0);
    assert_eq!(num(&wb, plan2, 1, 2), 7.0);
    // Fora da aba o nome não existe.
    wb.escrever_celula(0, 0, 0, "=SOMA(Itens)".into());
    assert!(matches!(wb.valor(0, 0, 0), Value::Error(_)), "{:?}", wb.valor(0, 0, 0));
}

#[test]
fn nome_que_depende_dele_mesmo_da_erro_sem_travar() {
    let mut wb = Workbook::new();
    wb.definir_nome_formula("A_", "=B_+1", None);
    wb.definir_nome_formula("B_", "=A_+1", None);
    wb.escrever_celula(0, 0, 0, "=A_".into());
    assert!(matches!(wb.valor(0, 0, 0), Value::Error(_)));
}

#[test]
fn renomear_aba_e_inserir_linha_acompanham_o_nome() {
    let mut wb = Workbook::new();
    let vendas = wb.criar_aba("Vendas");
    wb.escrever_celula(vendas, 0, 0, "7".into());
    wb.definir_nome_formula("Base", "=Vendas!$A$1*3", None);
    wb.definir_nome_formula("Local", "=1", Some("Vendas"));
    assert!(wb.renomear_aba(vendas, "Receitas"));
    let n = wb.nomes_formula();
    assert!(n.iter().any(|n| n.nome == "Base" && n.formula.contains("Receitas!")), "{n:?}");
    assert!(n.iter().any(|n| n.nome == "Local" && n.aba.as_deref() == Some("Receitas")));
    wb.inserir_linhas(vendas, 0, 1);
    wb.escrever_celula(0, 0, 0, "=Base".into());
    assert_eq!(num(&wb, 0, 0, 0), 21.0, "{:?}", wb.nomes_formula());
}

#[test]
fn intervalo_e_formula_da_pasta_com_o_mesmo_nome_nao_convivem() {
    let mut wb = Workbook::new();
    wb.definir_nome_formula("Taxa", "=0,1", None);
    wb.definir_nome("Taxa", feathersheet_core::celulas::refs::Rect::na_aba(0, 0, 0, 0, "Planilha1"));
    assert!(wb.nomes_formula().is_empty(), "o intervalo substitui a fórmula");
    wb.definir_nome_formula("Taxa", "=0,2", None);
    assert!(wb.nomeados().is_empty(), "e a fórmula substitui o intervalo");
    wb.definir_nome_formula("Taxa", "=0,3", Some("Planilha1"));
    assert!(wb.remover_nome("taxa"), "remover tira o da pasta");
    assert_eq!(wb.nomes_formula().len(), 1, "o da aba fica");
}
