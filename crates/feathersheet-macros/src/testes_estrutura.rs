//! Contratos TS de dimensão e contagem: testes via JavaScript Boa.
use super::*;


#[test]
fn macro_mesclagem_rust_liga_merge_unmerge_is_merged() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"Título".into());
    wb.escrever_celula(0,0,1,"Subordinada".into());
    wb.escrever_celula(0,1,1,"=2+3".into());
    let resultado=executar(&mut wb,0,concat!(
        "log(sheet.isMerged('B1'));",
        "sheet.merge('A1:C2');",
        "log(sheet.isMerged('B1'));",
        "log(sheet.isMerged('D1'));",
        "log(sheet.get('A1'));",
        "log(sheet.getRaw('B1'));",
        "log(sheet.getRaw('B2'));"
    ));
    assert!(resultado.erro.is_none(),"{:?}",resultado.erro);
    assert_eq!(resultado.saida,vec![
        "false","true","false","Título","",""
    ]);
    assert!(resultado.alterou);
    assert!(wb.aba(0).unwrap().sheet.mc.is_some());
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,1),"Subordinada");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,1),"=2+3");
    assert!(wb.aba(0).unwrap().sheet.mc.is_none());
}

#[test]
fn macro_mesclagem_desmescla_por_celula_interna_e_undo() {
    let mut wb=Workbook::new();
    let res=executar(&mut wb,0,concat!(
        "sheet.merge('B2:D4');",
        "log(sheet.isMerged('C3'));",
        "sheet.unmerge('C3');",
        "log(sheet.isMerged('B2'));"
    ));
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["true","false"]);
    // Mesclar e desmesclar dentro da mesma macro são uma transação
    // (o conteúdo original permanece sem modificações).
    assert!(wb.aba(0).unwrap().sheet.mc.is_none());
}

#[test]
fn macro_mesclagem_rollback_de_conteudo_e_metadados() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"Âncora".into());
    wb.escrever_celula(0,0,1,"Restaurar".into());
    let erro=executar(&mut wb,0,
        "sheet.merge('A1:B1');throw new Error('cancelar');");
    assert!(erro.erro.is_some());
    assert!(!erro.alterou);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,1),"Restaurar");
    assert!(wb.aba(0).unwrap().sheet.mc.is_none());
}

#[test]
fn macro_mesclagem_erro_e_noop_nao_mudam_planilha() {
    let mut wb=Workbook::new();
    for codigo in [
        "sheet.merge('A1')",
        "sheet.merge('A1:A500002')",
        "sheet.merge('XFE1:XFE2')",
        "sheet.isMerged('XFE1')",
    ] {
        let res=executar(&mut wb,0,codigo);
        assert!(res.erro.is_some(),"deveria falhar: {codigo}");
        assert!(!res.alterou);
    }
    let sem=executar(&mut wb,0,
        "sheet.unmerge('A1:XFD10000000');log(sheet.isMerged('A1'));");
    assert!(sem.erro.is_none(),"{:?}",sem.erro);
    assert!(!sem.alterou);
    assert_eq!(sem.saida,vec!["false"]);
    assert!(wb.aba(0).unwrap().sheet.mc.is_none());
}

#[test]
fn macro_mesclagem_aceita_aba_alternativa_e_encadeia() {
    let mut wb=Workbook::new();
    wb.criar_aba("Dados");
    wb.escrever_celula(1,0,0,"Cabeçalho".into());
    let res=executar(&mut wb,0,concat!(
        "wb.sheet('Dados').merge('A1:C1').set('D1',7);",
        "log(wb.sheet('Dados').isMerged('B1'));",
        "log(sheet.isMerged('B1'));"
    ));
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["true","false"]);
    assert!(res.alterou);
    assert_eq!(wb.aba(1).unwrap().sheet.get_raw(0,3),"7");
    assert!(wb.aba_mut(1).unwrap().sheet.undo());
    assert_eq!(wb.aba(1).unwrap().sheet.get_raw(0,3),"");
    assert!(wb.aba(1).unwrap().sheet.mc.is_none());
}

#[test]
fn macro_altura_e_largura_reusam_undo_do_core() {
    let mut wb = Workbook::new();
    wb.escrever_celula(0,0,0,"conteudo".into());
    let resposta = executar(&mut wb,0,
        "sheet.setRowHeight(2,45);sheet.setColumnWidth('C',144);");
    assert!(resposta.erro.is_none(),"{:?}",resposta.erro);
    assert!(resposta.alterou);
    let aba = &wb.aba(0).unwrap().sheet;
    assert_eq!(aba.row_heights.get(&1),Some(&45));
    assert_eq!(aba.col_widths.get(&2),Some(&144));
    assert_eq!(aba.get_raw(0,0),"conteudo");
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    let aba = &wb.aba(0).unwrap().sheet;
    assert_eq!(aba.row_heights.get(&1),None);
    assert_eq!(aba.col_widths.get(&2),None);
    assert_eq!(aba.get_raw(0,0),"conteudo");
}

#[test]
fn macro_oculta_linha_e_coluna_desfaz_junto() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"permanece".into());
    let resposta=executar(&mut wb,0,
        "sheet.hideRow(3);sheet.hideColumn(2);");
    assert!(resposta.erro.is_none(),"{:?}",resposta.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.row_heights.get(&2),Some(&0));
    assert_eq!(wb.aba(0).unwrap().sheet.col_widths.get(&1),Some(&0));
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert!(wb.aba(0).unwrap().sheet.row_heights.get(&2).is_none());
    assert!(wb.aba(0).unwrap().sheet.col_widths.get(&1).is_none());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"permanece");
}

#[test]
fn macro_erro_apos_redimensionar_reverte_ambas_as_dimensoes() {
    let mut wb=Workbook::new();
    let resultado=executar(&mut wb,0,
        "sheet.setRowHeight(2,37);sheet.hideColumn('AA');throw new Error('falha');");
    assert!(resultado.erro.is_some());
    assert!(!resultado.alterou);
    let sheet=&wb.aba(0).unwrap().sheet;
    assert_eq!(sheet.row_heights.get(&1),None);
    assert_eq!(sheet.col_widths.get(&26),None);
}

#[test]
fn macro_limites_invalidos_nao_aplicam_mudancas_parciais() {
    let mut wb=Workbook::new();
    let erros=[
        "sheet.setRowHeight(0,50);",
        "sheet.setRowHeight(1,-3);",
        "sheet.setColumnWidth('XFE',90);",
        "sheet.hideRow(100000001);",
        "sheet.hideColumn(0);",
    ];
    for codigo in erros {
        let resposta=executar(&mut wb,0,codigo);
        assert!(resposta.erro.is_some(),"esperado erro: {codigo}");
        assert!(!resposta.alterou);
    }
    assert!(wb.aba(0).unwrap().sheet.row_heights.is_empty());
    assert!(wb.aba(0).unwrap().sheet.col_widths.is_empty());
}

#[test]
fn contagem_linha_coluna_nao_aloca_planilha_gigante() {
    let mut wb=Workbook::new();
    let resposta=executar(&mut wb,0,
        "log(sheet.rowCount('B2:D9'));log(sheet.columnCount('B2:D9'));         log(sheet.rowCount('C:C'));log(sheet.columnCount('1:1'));");
    assert!(resposta.erro.is_none(),"{:?}",resposta.erro);
    assert!(!resposta.alterou);
    assert_eq!(resposta.saida,vec!["8","3","100000000","16384"]);
    assert_eq!(wb.aba(0).unwrap().sheet.len(),0);
}

#[test]
fn dimensoes_funcionam_em_outras_abas_pelo_objeto_wb() {
    let mut wb=Workbook::new();
    wb.criar_aba("Dados");
    let resultado=executar(&mut wb,0,
        "wb.sheet('Dados').setRowHeight(10,35).hideColumn('B');");
    assert!(resultado.erro.is_none(),"{:?}",resultado.erro);
    assert_eq!(wb.aba(1).unwrap().sheet.row_heights.get(&9),Some(&35));
    assert_eq!(wb.aba(1).unwrap().sheet.col_widths.get(&1),Some(&0));
    assert!(wb.aba_mut(1).unwrap().sheet.undo());
    assert!(wb.aba(1).unwrap().sheet.row_heights.is_empty());
    assert!(wb.aba(1).unwrap().sheet.col_widths.is_empty());
}

#[test]
fn macro_exclui_celulas_deslocando_e_linhas_inteiras() {
    let mut wb=Workbook::new();
    for (r,t) in ["a","b","c","d"].iter().enumerate() {
        wb.escrever_celula(0,r as u32,0,(*t).into());
        wb.escrever_celula(0,r as u32,1,format!("x{t}"));
    }
    let res=executar(&mut wb,0,concat!(
        "sheet.range('A2').deleteCells('up');",
        "log(sheet.get('A2'), sheet.get('B2'));",
        "sheet.insertCells('B1', 'down');",
        "log(sheet.get('B1') === null || sheet.get('B1') === '', sheet.get('B2'));",
        "sheet.deleteRow(1);",
        "log(sheet.get('A1'));",
        "sheet.insertColumn('A', 2);",
        "log(sheet.get('C1'));"
    ));
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["c xb","true xa","c","c"]);
    assert!(executar(&mut wb,0,"sheet.range('A1').deleteCells('down');").erro.is_some());
}

#[test]
fn macro_copia_para_outra_aba() {
    let mut wb=Workbook::new();
    wb.criar_aba("Destino");
    wb.escrever_celula(0,0,0,"7".into());
    wb.escrever_celula(0,0,1,"=A1*2".into());
    let res=executar(&mut wb,0,concat!(
        "sheet.range('A1:B1').copy('C3', {sheet: 'Destino'});",
        "sheet.range('A1').copy('Destino!A9', {paste: 'values'});"
    ));
    assert!(res.erro.is_none(),"{:?}",res.erro);
    let d=&wb.aba(1).unwrap().sheet;
    assert_eq!(d.get_raw(2,2),"7");
    assert_eq!(d.get_raw(2,3),"=C3*2");
    assert_eq!(d.get_raw(8,0),"7");
    assert!(executar(&mut wb,0,"sheet.range('A1').copy('B1', {sheet: 'Nada'});").erro.is_some());
}

#[test]
fn macro_ajusta_a_largura_pelo_conteudo_e_aceita_inteiro_vindo_como_fracao() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, "sheet.set('B1', 'um texto bem comprido para a coluna B ficar larga'); sheet.set('C1', 'x'); sheet.autoFitColumns('B:C'); sheet.set('D1', sheet.resize('A1', 0.5 * 4, 1).address);");
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let s = &wb.aba(0).unwrap().sheet;
    assert!(s.col_widths.get(&1).is_some_and(|&l| l > 200), "{:?}", s.col_widths);
    assert!(s.col_widths.get(&2).is_some_and(|&l| l < 122), "{:?}", s.col_widths);
    assert_eq!(wb.valor(0, 0, 3), feathersheet_core::Value::Text("A1:A2".into()));
}
