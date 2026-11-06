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
        selecao: Some("C5".into()), respostas: vec![], valores: None, ..Default::default() };
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

#[test]
fn field_muda_o_formulario_aberto_e_hide_fecha() {
    let mut wb = pasta_com_dados();
    let contexto = Contexto { valores: Some(serde_json::json!({ "nome": "Ana" }).as_object().unwrap().clone()), ..Default::default() };
    let r = executar_com(&mut wb, 0, r#"
        const n = sheet.field("nome");
        if (n.value !== "Ana") throw new Error(n.value);
        n.value = n.value.toUpperCase();
        sheet.field("idade").enabled = false;
        sheet.field("obs").visible = 0;
        sheet.field("ok").backColor = '#ff0000';
        sheet.hideComponent();
    "#, contexto);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.campos, vec![("nome".into(), "value".into(), "ANA".into()), ("idade".into(), "enabled".into(), false.into()),
        ("obs".into(), "visible".into(), false.into()), ("ok".into(), "backColor".into(), "#ff0000".into())]);
    assert!(r.fechar_formulario);
    // Com erro, nada do formulário é aplicado.
    let r = executar(&mut wb, 0, r#"sheet.field("a").value = 1; throw new Error("x");"#);
    assert!(r.campos.is_empty() && !r.fechar_formulario);
}

#[test]
fn macro_cria_renomeia_e_apaga_aba_e_as_referencias_seguem_a_aba() {
    let mut wb = pasta_com_dados();
    wb.inserir_aba("Velha", 0);
    let r = executar(&mut wb, 1, r#"
        const dados = wb.sheet('Dados');
        const rel = wb.addSheet(null, { after: sheet });
        wb.renameSheet(rel, 'Fechamento');
        wb.deleteSheet('Velha');
        rel.set('A1', 'ok');
        dados.set('A1', rel.name);
        const fim = wb.addSheet('Fim', { after: wb.sheet(wb.sheetCount) });
        fim.set('B2', 7);
    "#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert!(r.alterou);
    assert_eq!(wb.nomes(), ["Planilha1", "Fechamento", "Dados", "Fim"]);
    assert_eq!(wb.valor(1, 0, 0), Value::Text("ok".into()));
    assert_eq!(wb.valor(2, 0, 0), Value::Text("Fechamento".into()));
    assert_eq!(wb.valor(3, 1, 1), Value::Number(7.0));
    assert_eq!(r.aba_ativa, Some(3));
}

#[test]
fn erro_na_macro_desfaz_as_abas_criadas_apagadas_e_renomeadas() {
    let mut wb = pasta_com_dados();
    wb.inserir_aba("Velha", 0);
    wb.aba_mut(0).unwrap().sheet.set_cell(0, 0, "guardado");
    let r = executar(&mut wb, 1, r#"
        wb.sheet('Velha').set('A1', 'mudado');
        const nova = wb.addSheet('Nova');
        nova.set('A1', 1);
        wb.renameSheet('Dados', 'Outra');
        wb.deleteSheet('Velha');
        throw new Error('falhou');
    "#);
    assert!(r.erro.is_some());
    assert_eq!(wb.nomes(), ["Velha", "Planilha1", "Dados"]);
    assert_eq!(wb.valor(0, 0, 0), Value::Text("guardado".into()));
}

#[test]
fn nao_apaga_a_ultima_aba_nem_cria_nome_repetido() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, "wb.deleteSheet(sheet);");
    assert!(r.erro.as_deref().is_some_and(|e| e.contains("pelo menos uma aba")), "{:?}", r.erro);
    let r = executar(&mut wb, 0, "wb.addSheet(sheet.name);");
    assert!(r.erro.as_deref().is_some_and(|e| e.contains("Já existe")), "{:?}", r.erro);
    assert_eq!(wb.len(), 1);
}

#[test]
fn protect_e_unprotect_ficam_anotados_na_ordem() {
    use feathersheet_core::planilha::sheet::protecao::Permissao;
    let mut wb = pasta_com_dados();
    let r = executar(&mut wb, 0, r#"
        wb.sheet("Dados").unprotect("abc");
        wb.sheet("Dados").set("A1", 5);
        wb.sheet("Dados").protect("abc", { formatCells: true, sort: false });
        sheet.protect();
    "#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let p = &r.protecoes;
    assert_eq!(p.len(), 3);
    assert!(!p[0].proteger && p[0].senha.as_deref() == Some("abc"));
    assert!(p[1].proteger && p[1].permitidas == [Permissao::FormatarCelulas]);
    assert!(p[2].proteger && p[2].senha.is_none() && p[2].aba != p[1].aba);
    let r = executar(&mut wb, 0, r#"sheet.protect("x", { voar: true });"#);
    assert!(r.erro.is_some() && r.protecoes.is_empty());
}

#[test]
fn show_gridlines_vale_o_ultimo_por_aba() {
    let mut wb = pasta_com_dados();
    let r = executar(&mut wb, 0, r#"sheet.showGridlines(false); wb.sheet("Dados").showGridlines(0); sheet.showGridlines(true);"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.grades.len(), 2);
    assert!(r.grades[0].1 == false && r.grades[1].1 == true && r.grades[0].0 != r.grades[1].0);
}

#[test]
fn open_url_so_anota_o_link() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, r#"if (app.openUrl("https://example.com")) sheet.set("A1", 1);"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.links, ["https://example.com"]);
    assert_eq!(wb.valor(0, 0, 0), Value::Number(1.));
}

#[test]
fn shape_muda_a_letra_e_o_texto_da_caixa_pelo_nome() {
    let mut wb = Workbook::new();
    // O Excel em português grava "CaixaDeTexto 39"; a macro gravada pede "TextBox 39".
    wb.editar(0, |s| { s.alterar_objetos(Some(r##"{"v":1,"objetos":[{"id":1,"forma":"rect","de":{},"ate":{},"nome":"CaixaDeTexto 39","texto":"Saldo","fonte":{"tamanho":11}}]}"##.into())); });
    let r = executar(&mut wb, 0, r#"
        if (sheet.shape("TextBox 3").exists || sheet.shape("Retângulo 39").exists) throw new Error("número ou tipo diferente");
        const t = sheet.shape("textbox 39");
        if (!t.exists || sheet.shape("Nada").exists) throw new Error("exists");
        t.font({ bold: true, size: 14, color: app.rgb(0, 176, 80) });
        t.text = t.text + " ok";
        sheet.set("A1", t.font().color + "|" + t.font().size);
    "#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(wb.valor(0, 0, 0), Value::Text("#00b050|14".into()));
    let ob: serde_json::Value = serde_json::from_str(wb.aba(0).unwrap().sheet.ob.as_deref().unwrap()).unwrap();
    let o = &ob["objetos"][0];
    assert_eq!(o["texto"], "Saldo ok");
    assert_eq!(o["fonte"]["negrito"], true);
    let r = executar(&mut wb, 0, r#"sheet.shape("TextBox 39").font({ bold: false }); sheet.shape("Outra").font({ bold: true });"#);
    assert!(r.erro.is_some(), "forma que não existe é erro");
    let ob: serde_json::Value = serde_json::from_str(wb.aba(0).unwrap().sheet.ob.as_deref().unwrap()).unwrap();
    assert_eq!(ob["objetos"][0]["fonte"]["negrito"], true, "o erro desfaz a mudança anterior");
}

#[test]
fn shell_so_anota_o_programa_e_some_se_a_macro_falha() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, r#"const id = app.shell("  echo oi  "); sheet.set("A1", id);"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.programas, vec!["echo oi".to_string()]);
    assert_eq!(wb.valor(0, 0, 0), Value::Number(0.));
    let r = executar(&mut wb, 0, r#"app.shell("echo oi"); throw new Error("x");"#);
    assert!(r.erro.is_some() && r.programas.is_empty());
    let r = executar(&mut wb, 0, r#"app.shell("");"#);
    assert!(r.erro.is_some());
    let r = executar(&mut wb, 0, r#"for (let i = 0; i < 30; i++) app.shell("x");"#);
    assert!(r.erro.is_some() && r.programas.is_empty(), "teto de 20 por execução");
}

#[test]
fn autofilter_so_anota_o_pedido_e_confere_a_coluna() {
    use feathersheet_core::celulas::refs::Rect;
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, r#"sheet.autoFilter("A1:C10", 2, ">100").autoFilter("A1:C10", 1, ["SP", "RJ"]).autoFilter("A1:C10", 3).showAllData().autoFilterOff();"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let area = Rect::new(0, 0, 9, 2);
    let acoes: Vec<_> = r.filtros.iter().map(|p| p.acao.clone()).collect();
    assert_eq!(acoes, vec![
        AcaoFiltro::Aplicar { area: area.clone(), coluna: Some(2), criterio: Some(serde_json::json!(">100")) },
        AcaoFiltro::Aplicar { area: area.clone(), coluna: Some(1), criterio: Some(serde_json::json!(["SP", "RJ"])) },
        AcaoFiltro::Aplicar { area, coluna: Some(3), criterio: None },
        AcaoFiltro::MostrarTudo, AcaoFiltro::Remover]);
    assert!(executar(&mut wb, 0, r#"sheet.autoFilter("A1:C10", 4, "x");"#).erro.is_some(), "coluna fora da área");
    let r = executar(&mut wb, 0, r#"sheet.autoFilter("A1:C10", 1, "x"); throw new Error("x");"#);
    assert!(r.erro.is_some() && r.filtros.is_empty(), "macro com erro não filtra");
}

#[test]
fn visiveis_seguem_o_filtro_pedido_na_mesma_macro() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, r#"sheet.set("A1:B6", [["Nome","Valor"],["Bia",50],["Ana",150],["Caio",250],["Davi",90],["Eva",120]]);
        sheet.autoFilter("A1:B6", 2, {op: "e", val: [">100", "<200"]});
        log(sheet.specialCells("A2:A6", "visible").join(","));
        sheet.showAllData();
        log(sheet.specialCells("A2:A6", "visible").length);
        sheet.autoFilter("A1", 1, "Bia");
        log(sheet.specialCells("A1:B6", "visible").join(","));"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.saida, vec!["A3,A6".to_string(), "5".into(), "A1,B1,A2,B2".into()]);
    // Linha e coluna ocultas à mão também ficam de fora, sem filtro nenhum.
    wb.editar(0, |s| { s.row_heights.insert(2, 0); s.col_widths.insert(1, 0); });
    let r = executar(&mut wb, 0, r#"log(sheet.specialCells("A2:B4", "visible").join(","));"#);
    assert_eq!(r.saida, vec!["A2,A4".to_string()]);
}

#[test]
fn varias_areas_copiam_juntas_e_escrevem_em_cada_uma() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, r#"sheet.set("A1:B6", [["Nome","UF"],["Bia","SP"],["Ana","RJ"],["Caio","SP"],["Davi","MG"],["Eva","SP"]]);
        sheet.autoFilter("A1:B6", 2, "SP");
        const vis = sheet.specialAreas("A1:B6", "visible");
        log(vis);
        sheet.copy(vis, "D1");
        log(sheet.range(vis).address, sheet.range(vis).value, sheet.range(vis).rows);
        let n = 0; sheet.range(vis).forEach(() => n++); log(n);
        sheet.range("A2:A2,A4:A4").set("x");
        try { sheet.range("A1,B2").sort([]); } catch (e) { log("sort recusado"); }
        log(sheet.specialAreas("A1:B6", "formulas") === "");"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.saida[0], "A1:B2,A4:B4,A6:B6");
    assert_eq!(r.saida[1], "A1:B2,A4:B4,A6:B6 Nome 2");
    assert_eq!(r.saida[2..], ["8".to_string(), "sort recusado".into(), "true".into()]);
    let col = |c: u32| (0..5).map(|l| wb.valor(0, l, c)).collect::<Vec<_>>();
    let t = |x: &str| Value::Text(x.into());
    assert_eq!(col(3), vec![t("Nome"), t("Bia"), t("Caio"), t("Eva"), Value::Empty], "as visíveis coladas juntas");
    assert_eq!(col(4)[..4], [t("UF"), t("SP"), t("SP"), t("SP")]);
    assert_eq!((wb.valor(0, 1, 0), wb.valor(0, 2, 0), wb.valor(0, 3, 0)), (t("x"), t("Ana"), t("x")));
}

#[test]
fn shape_esconde_troca_a_macro_e_pede_posicao() {
    let mut wb = Workbook::new();
    wb.editar(0, |s| { s.alterar_objetos(Some(r##"{"v":1,"objetos":[{"id":1,"forma":"rect","de":{},"ate":{},"nome":"Botão 2","macro":"Velha"}]}"##.into())); });
    let r = executar(&mut wb, 0, r#"
        const b = sheet.shape("Button 2");
        if (!b.visible || b.onAction !== "Velha") throw new Error("leitura");
        b.visible = false; b.onAction = "Modulo1.Gravar";
        b.left = 72; b.top = 36; b.move({ width: 90, height: 30 });
        if (b.visible || b.onAction !== "Modulo1.Gravar") throw new Error("releitura");"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let ob: serde_json::Value = serde_json::from_str(wb.aba(0).unwrap().sheet.ob.as_deref().unwrap()).unwrap();
    assert_eq!(ob["objetos"][0]["oculto"], true);
    assert_eq!(ob["objetos"][0]["macro"], "Modulo1.Gravar");
    let p: Vec<_> = r.formas.iter().map(|p| (p.nome.as_str(), p.esquerda, p.topo, p.largura, p.altura)).collect();
    assert_eq!(p, vec![("Button 2", Some(72.), None, None, None), ("Button 2", None, Some(36.), None, None), ("Button 2", None, None, Some(90.), Some(30.))]);
    let r = executar(&mut wb, 0, r#"sheet.shape("Button 2").visible = true; sheet.shape("Button 2").onAction = "";"#);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let ob: serde_json::Value = serde_json::from_str(wb.aba(0).unwrap().sheet.ob.as_deref().unwrap()).unwrap();
    assert!(ob["objetos"][0].get("oculto").is_none() && ob["objetos"][0].get("macro").is_none());
    assert!(executar(&mut wb, 0, r#"sheet.shape("Nada").visible = false;"#).erro.is_some());
    assert!(executar(&mut wb, 0, r#"sheet.shape("Button 2").width = -5;"#).erro.is_some());
}

#[test]
fn choose_espera_o_botao_e_roda_de_novo_com_a_escolha() {
    let mut wb = Workbook::new();
    let codigo = r#"const i = app.choose("Arquivo em uso", [3, 4, 5], 16, 1); sheet.set("A1", i);"#;
    let r = executar(&mut wb, 0, codigo);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.pergunta.as_deref(), Some("Arquivo em uso"));
    assert_eq!(r.escolha, Some(Escolha { botoes: vec![3, 4, 5], icone: 16, padrao: 1 }));
    assert_eq!(wb.valor(0, 0, 0), Value::Empty, "nada gravado enquanto espera");
    let r = executar_com(&mut wb, 0, codigo, Contexto { escolhas: vec![2], ..Default::default() });
    assert!(r.erro.is_none() && r.pergunta.is_none() && r.escolha.is_none(), "{r:?}");
    assert_eq!(wb.valor(0, 0, 0), Value::Number(2.));
    assert!(executar(&mut wb, 0, r#"app.choose("x", [9]);"#).erro.is_some());
}
