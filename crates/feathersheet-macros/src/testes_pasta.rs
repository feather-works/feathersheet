//! Operações de pasta das macros convertidas do VBA: abas, nomes, seleção,
//! arquivo, mensagens e a pergunta Sim/Não.
use super::*;
use feathersheet_core::Value;

fn pasta_com_dados() -> Workbook {
    let mut wb = Workbook::new();
    wb.inserir_aba("Dados", 1);
    wb
}

#[test]
fn ativa_aba_seleciona_e_cria_nome() {
    let mut wb = pasta_com_dados();
    let r = executar(&mut wb, 0, r#"
        app.setEventsEnabled(false);
        wb.activate("Dados");
        const d = wb.sheet("Dados");
        d.select("B2:B4");
        d.selection().set(7);
        d.activeCell().set(1);
        wb.addName("Lista", d.selection().address);
        if (wb.activeSheet.name !== "Dados") throw new Error(wb.activeSheet.name);
    "#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.aba_ativa, Some(1));
    assert_eq!(wb.valor(1, 1, 1), Value::Number(1.));
    assert_eq!(wb.valor(1, 3, 1), Value::Number(7.));
    let n = wb.nomeados();
    assert_eq!(n[0].nome, "Lista");
    assert_eq!((n[0].area.r0, n[0].area.r1, n[0].area.sheet.as_deref()), (1, 3, Some("Dados")));
}

#[test]
fn nome_aceita_aba_no_endereco() {
    let mut wb = pasta_com_dados();
    let r = executar(&mut wb, 0, r#"wb.addName("Fab", "'Dados'!$C$2:$C$9");"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let n = &wb.nomeados()[0];
    assert_eq!((n.area.r0, n.area.c0, n.area.r1, n.area.sheet.as_deref()), (1, 2, 8, Some("Dados")));
}

#[test]
fn selecao_inicial_e_arquivo_vem_do_contexto() {
    let mut wb = Workbook::new();
    let ctx = Contexto { arquivo: Some("/tmp/proj/Planilha.xlsm".into()),
        selecao: Some("C5".into()), respostas: vec![] };
    let r = executar_com(&mut wb, 0, r#"
        sheet.activeCell().set(wb.name + "|" + wb.path + "|" + sheet.value("C5"));
    "#, ctx);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(wb.valor(0, 4, 2), Value::Text("Planilha.xlsm|/tmp/proj|null".into()));
}

#[test]
fn pergunta_sem_resposta_desfaz_e_devolve_a_pergunta() {
    let mut wb = Workbook::new();
    let codigo = r#"
        sheet.set("A1", 1);
        alert("antes");
        if (confirm("Gerar?")) sheet.set("A2", 2);
        wb.addName("X", "A1");
    "#;
    let r = executar(&mut wb, 0, codigo);
    assert_eq!(r.pergunta.as_deref(), Some("Gerar?"));
    assert!(!r.alterou && r.mensagens.is_empty());
    assert_eq!(wb.valor(0, 0, 0), Value::Empty);
    assert!(wb.nomeados().is_empty());

    let r = executar_com(&mut wb, 0, codigo, Contexto { respostas: vec![true], ..Default::default() });
    assert!(r.pergunta.is_none() && r.erro.is_none());
    assert_eq!(r.mensagens, vec!["antes".to_string()]);
    assert_eq!(wb.valor(0, 1, 0), Value::Number(2.));
    assert_eq!(wb.nomeados().len(), 1);
}

#[test]
fn nome_invalido_e_erro_desfazem_os_nomes() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, r#"wb.addName("A1", "B2");"#);
    assert!(r.erro.unwrap().contains("Nome inválido"));
    let r = executar(&mut wb, 0, r#"wb.addName("Bom", "B2"); throw new Error("x");"#);
    assert!(r.erro.is_some());
    assert!(wb.nomeados().is_empty());
}

#[test]
fn pedidos_de_arquivo_ficam_anotados_com_caminho_resolvido() {
    let mut wb = pasta_com_dados();
    let ids = wb.ids();
    let ctx = Contexto { arquivo: Some("/tmp/proj/Planilha.xlsm".into()), ..Default::default() };
    let r = executar_com(&mut wb, 0, r#"
        sheet.exportPdf("saida\\01-FORMULARIO");
        wb.sheet("Dados").exportPdf("/tmp/proj/fora/../saida/./a.PDF");
        wb.saveSheetsAs(["dados", null, 1, "Dados"], wb.path + "\\09-UNIDADES.xlsx");
        wb.sheet("Dados").saveAs("copia.fshz");
    "#, ctx);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.arquivos, vec![
        PedidoArquivo::Pdf { aba: ids[0], caminho: "/tmp/proj/saida/01-FORMULARIO.pdf".into() },
        PedidoArquivo::Pdf { aba: ids[1], caminho: "/tmp/proj/saida/a.PDF".into() },
        PedidoArquivo::Abas { abas: vec![ids[1], ids[0]], caminho: "/tmp/proj/09-UNIDADES.xlsx".into() },
        PedidoArquivo::Abas { abas: vec![ids[1]], caminho: "/tmp/proj/copia.fshz".into() },
    ]);
}

#[test]
fn pedido_de_arquivo_fora_da_pasta_do_documento_e_recusado() {
    let mut wb = pasta_com_dados();
    let ctx = || Contexto { arquivo: Some("/tmp/proj/P.xlsm".into()), ..Default::default() };
    for alvo in ["/tmp/fora/a.pdf", "/home/x/.bashrc.pdf", "../a.pdf", "saida/../../a.pdf", "..\\a.pdf", "/tmp/proj"] {
        let r = executar_com(&mut wb, 0, &format!("sheet.exportPdf({alvo:?});"), ctx());
        assert!(r.erro.as_deref().is_some_and(|e| e.contains("só grava dentro")), "{alvo}: {:?}", r.erro);
        assert!(r.arquivos.is_empty());
    }
}

#[test]
fn pedido_de_arquivo_some_com_erro_e_recusa_o_que_nao_sabe_gravar() {
    let mut wb = pasta_com_dados();
    let ctx = || Contexto { arquivo: Some("/tmp/proj/P.xlsm".into()), ..Default::default() };
    let r = executar_com(&mut wb, 0, r#"sheet.exportPdf("a.pdf"); throw new Error("x");"#, ctx());
    assert!(r.erro.is_some() && r.arquivos.is_empty());
    let r = executar_com(&mut wb, 0, r#"wb.saveSheetsAs(["Dados"], "a.xlsm");"#, ctx());
    assert!(r.erro.unwrap().contains(".xlsm"));
    let r = executar_com(&mut wb, 0, r#"wb.saveSheetsAs(["Nenhuma"], "a.xlsx");"#, ctx());
    assert!(r.erro.unwrap().contains("Aba inexistente"));
    // Pasta nunca salva: `wb.path` é vazio, o caminho cairia na raiz.
    let r = executar(&mut wb, 0, r#"sheet.exportPdf(wb.path + "\\x.pdf");"#);
    assert!(r.erro.unwrap().contains("Salve a pasta"));
    let r = executar(&mut wb, 0, r#"sheet.exportPdf("x.pdf");"#);
    assert!(r.erro.unwrap().contains("Salve a pasta"));
}

#[test]
fn formula_r1c1_com_colunas_inteiras_faz_procv_na_outra_aba() {
    let mut wb = Workbook::new();
    wb.inserir_aba("MAIN", 1);
    wb.inserir_aba("inversores", 2);
    let r = executar(&mut wb, 1, r#"
        wb.sheet("MAIN").set("E29", "K1");
        const inv = wb.sheet("inversores");
        inv.set("C4", "K0"); inv.set("L4", "800");
        inv.set("C5", "K1"); inv.set("L5", "840");
        wb.sheet("MAIN").range("A1").setFormulaR1C1(
            '=CONCATENATE("MPPT RANGE:",VLOOKUP(MAIN!R29C5,inversores!C3:C13,10,0)," V")');
    "#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let f = wb.aba(1).unwrap().sheet.get_raw(0, 0);
    assert!(f.contains("VLOOKUP(MAIN!$E$29,inversores!$C:$M,10,0)"), "{f:?}");
    assert_eq!(wb.valor(1, 0, 0), Value::Text("MPPT RANGE:840 V".into()));
}
