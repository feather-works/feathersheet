//! Testes de integração da API JavaScript de macros do FeatherSheet.
//! Extraídos de lib.rs sem mudar sua execução nem as expectativas.
use super::*;
#[test]
fn macro_special_cells_usa_enumeracao_do_core_sem_marcar_arquivo() {
    let mut wb = Workbook::new();
    wb.escrever_celula(0, 0, 0, "10".into());
    wb.escrever_celula(0, 0, 1, "=A1*2".into());
    wb.escrever_celula(0, 1, 0, "=1/0".into());
    let resultado=executar(&mut wb,0,
        "log(sheet.specialCells('A1:C2','formulas')); \
         log(sheet.range('A1:C2').specialCells('constants')); \
         log(sheet.specialCells('A1:C2','blanks'));");
    assert!(resultado.erro.is_none(), "{:?}",resultado.erro);
    assert!(!resultado.alterou);
    assert_eq!(resultado.saida.len(),3);
    assert!(resultado.saida[0].contains("B1"));
    assert!(resultado.saida[0].contains("A2"));
    assert!(resultado.saida[1].contains("A1"));
    assert!(resultado.saida[2].contains("C2"));
}

#[test]
fn macro_intersect_e_union_reusam_operacoes_de_retangulos() {
    let mut wb=Workbook::new();
    let resultado=executar(&mut wb,0,
        "log(sheet.intersect('A1:C5','B3:D7').address); \
         log(sheet.union('A1:B2','D4:E5').address); \
         log(sheet.intersect('A1','Z90')===null);");
    assert!(resultado.erro.is_none(), "{:?}",resultado.erro);
    assert!(!resultado.alterou);
    assert_eq!(resultado.saida,vec![
        "B3:C5".to_string(), "A1:E5".to_string(), "true".to_string()
    ]);
}

#[test]
fn macro_get_region_conecta_motor_rust_e_preserva_bloco_separado() {
    let mut wb=Workbook::new();
    for (row,col,raw) in [
        (0,0,"Primeiro"), (1,1,"Diagonal"), (2,1,"=1+2"),
        (5,6,"Fora"),
    ] { wb.escrever_celula(0,row,col,raw.into()); }
    let versao=wb.aba(0).unwrap().versao;
    let res=executar(&mut wb,0,concat!(
        "log(sheet.getRegion('B2').address);",
        "log(sheet.getRegion('A1:B3').address);",
        "log(sheet.getRegion('G6').address);",
        "log(sheet.getRegion('B2').rows);",
        "log(sheet.getRegion('B2').columns);"
    ));
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["A1:B3","A1:B3","G6","3","2"]);
    assert!(!res.alterou);
    assert_eq!(wb.aba(0).unwrap().versao,versao);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2,1),"=1+2");
}
#[test]
fn macro_get_region_aba_vazia_e_endereco_invalido() {
    let mut wb=Workbook::new();
    let vazio=executar(&mut wb,0,
        "log(sheet.getRegion('D9').address);log(sheet.getRegion('B3:C5').address);");
    assert!(vazio.erro.is_none(),"{:?}",vazio.erro);
    assert_eq!(vazio.saida,vec!["D9","B3:C5"]);
    assert!(!vazio.alterou);
    let erro=executar(&mut wb,0,
        "sheet.set('A1',42);sheet.getRegion('XFE1');");
    assert!(erro.erro.is_some());
    assert!(!erro.alterou);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"");
}
#[test]
fn macro_get_region_coluna_inteira_e_abas_nomeadas() {
    let mut wb=Workbook::new();
    wb.criar_aba("Dados");
    for (r,c,raw) in [
        (0,0,"Nome"), (0,1,"Número"), (1,0,"A"), (1,1,"9"), (2,0,"B"), (2,1,"10"),
    ] { wb.escrever_celula(1,r,c,raw.into()); }
    let res=executar(&mut wb,0,concat!(
        "log(wb.sheet('Dados').getRegion('A:A').address);",
        "log(wb.sheet('Dados').getRegion('B2').address);"
    ));
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["A1:B3","A1:B3"]);
    assert!(!res.alterou);
}
#[test]
fn macro_geometria_de_intervalos_quatro_operacoes() {
    let mut wb=Workbook::new();
    let out=executar(&mut wb,0,concat!(
        "log(sheet.offset('B2:C3',2,-1).address);",
        "log(sheet.resize('C7:D9',2,3).address);",
        "log(sheet.columnOf('B2:D5',2).address);",
        "log(sheet.rowOf('B2:D5',3).address);",
        "log(sheet.offset('C:C',0,1).rows);"
    ));
    assert!(out.erro.is_none(),"{:?}",out.erro);
    assert_eq!(out.saida,vec!["A4:B5","C7:E8","C2:C5","B4:D4","100000000"]);
    assert!(!out.alterou);
    assert_eq!(wb.aba(0).unwrap().sheet.len(),0);
}

#[test]
fn macro_geometria_invalida_reverte_uma_edicao_anterior() {
    for operacao in [
        "sheet.offset('A1',-1,0)",
        "sheet.offset('XFD1',0,1)",
        "sheet.resize('A1',0,4)",
        "sheet.resize('XFD100000000',2,1)",
        "sheet.columnOf('B2:D5',4)",
        "sheet.rowOf('A1:C3',0)",
    ] {
        let mut wb=Workbook::new();
        let res=executar(&mut wb,0,&format!("sheet.set('A1',123);{operacao};"));
        assert!(res.erro.is_some(),"operacao: {operacao}");
        assert!(!res.alterou);
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"");
    }
}

#[test]
fn limpeza_sem_alteracao_nao_marca_pasta_modificada() {
    let mut wb=Workbook::new();
    let result=executar(&mut wb,0,
        "sheet.clearContents('A1:A3'); sheet.clearFormats('B1:B3'); log(sheet.replace('C1:C3','x','y'));");
    assert!(result.erro.is_none(),"{:?}",result.erro);
    assert!(!result.alterou);
    assert_eq!(wb.aba(0).unwrap().sheet.len(),0);
}

#[test]
fn macro_clear_contents_preserva_cor_e_formula_desfeita() {
    use feathersheet_core::celulas::estilo_parcial::{EstiloParcial, Campo};
    use feathersheet_core::celulas::refs::Rect;
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"=4+5".into());
    let mut patch = EstiloParcial::new();
    patch.insert("bg".into(), Campo::Texto("#00ffaa".into()));
    wb.editar(0,|sheet| sheet.set_style(&Rect::cell(0,0),&patch));
    let res=executar(&mut wb,0,"sheet.range('A1').clearContents();");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"");
    assert_eq!(
        wb.aba(0).unwrap().sheet.estilo_resolvido(0,0).unwrap().bg.as_deref(),
        Some("#00ffaa")
    );
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"=4+5");
}
#[test]
fn macro_clear_formats_preserva_valor_e_heranca() {
    use feathersheet_core::celulas::estilo_parcial::{EstiloParcial, Campo};
    use feathersheet_core::celulas::refs::Rect;
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"42".into());
    let mut patch = EstiloParcial::new();
    patch.insert("bold".into(), Campo::Logico(true));
    wb.editar(0,|sheet| sheet.set_style(&Rect::cell(0,0),&patch));
    let res=executar(&mut wb,0,"sheet.clearFormats('A1');");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"42");
    assert_eq!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,0),None);
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,0).unwrap().bold);
}
#[test]
fn macro_find_considera_formula_e_faixa_sem_alterar_documento() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"=1+2".into());
    wb.escrever_celula(0,4,3,"Olá Vitória".into());
    let res=executar(&mut wb,0,
        "log(sheet.find('vitória')); log(sheet.find('=1+2',{inFormulas:true})); log(sheet.range('D1:D10').find('Olá',{wholeCell:false}));");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert!(res.saida.iter().any(|msg|msg.contains("D5")));
    assert!(res.saida.iter().any(|msg|msg.contains("A1")));
    assert_eq!(wb.valor(0,0,0),Value::Number(3.));
    assert!(!res.alterou, "find deve ser somente leitura");
}

#[test]
fn macro_replace_muda_multiplas_celulas_e_devolve_quantidade() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"ABRIL e abril".into());
    wb.escrever_celula(0,1,0,"outro".into());
    let res=executar(&mut wb,0,
        "const n=sheet.replace('A1:A2','abril','maio'); log(n);");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"maio e maio");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,0),"outro");
    assert!(res.saida.iter().any(|s|s.contains('1')));
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"ABRIL e abril");
}

#[test]
fn macro_setbold_cores_tamanho_e_alinhamento_funcionam_com_undo() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"42".into());
    let res=executar(&mut wb,0,
        "sheet.setBold('A1'); sheet.setBackground('A1','#0f8'); sheet.range('A1').setFontColor(255).setFontSize(16).setAlignHorizontal('right').setWrapText(true);");
    assert!(res.erro.is_none(), "{:?}", res.erro);
    let style=wb.aba(0).unwrap().sheet.estilo_resolvido(0,0).unwrap();
    assert!(style.bold);
    assert_eq!(style.bg, Some("#00ff88".into()));
    assert_eq!(style.color, Some("#ff0000".into()));
    assert_eq!(style.font_size, Some(16.0));
    assert!(style.wrap);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"42");
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,0).is_none());
}

#[test]
fn macro_hyperlink_preserva_outros_valores_e_erro_reverte_tudo() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,1,"valor existente".into());
    let ok=executar(&mut wb,0,
        "sheet.setHyperlink('A1:B1','https://example.org','Visitar');");
    assert!(ok.erro.is_none(), "{:?}",ok.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"Visitar");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,1),"valor existente");
    let failed=executar(&mut wb,0,
        "sheet.range('A1').setBold(false); sheet.setFontSize('A1',1000);");
    assert!(failed.erro.is_some());
    assert!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,0).is_some());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"Visitar");
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"");
}

#[test]
fn macro_autofill_preenche_serie_numerica_e_undo_unico() {
    let mut wb = Workbook::new();
    wb.escrever_celula(0,0,0,"10".into());
    wb.escrever_celula(0,1,0,"20".into());
    let res = executar(&mut wb,0,
        "const total=sheet.autoFill('A1:A2','A1:A6'); log(total);");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert!(res.alterou);
    let sheet=&wb.aba(0).unwrap().sheet;
    assert_eq!(sheet.get_raw(2,0),"30");
    assert_eq!(sheet.get_raw(5,0),"60");
    assert!(res.saida.iter().any(|m|m.contains('4')));
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(5,0),"");
}

#[test]
fn macro_autofill_formula_e_erro_revertem_tudo() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"2".into());
    wb.escrever_celula(0,0,1,"=A1*2".into());
    let res=executar(&mut wb,0,
        "sheet.range('B1').autoFill('B1:B3'); throw new Error('cancelar');");
    assert!(res.erro.is_some());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,1),"");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2,1),"");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,1),"=A1*2");
}

#[test]
fn macro_autofill_rejeita_destinos_ambiguos_e_nulos_sem_alterar() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"1".into());
    let igual=executar(&mut wb,0,
        "log(sheet.autoFill('A1','A1'));");
    assert!(igual.erro.is_none(), "{:?}",igual.erro);
    assert!(!igual.alterou);
    let falha=executar(&mut wb,0,
        "sheet.autoFill('A1','A1:B9999999');");
    assert!(falha.erro.is_some());
    assert_eq!(wb.aba(0).unwrap().sheet.len(),1);
}

#[test]
fn move_de_macro_reescreve_formulas_da_mesma_aba_e_desfaz() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"10".into());
    wb.escrever_celula(0,1,0,"20".into());
    wb.escrever_celula(0,0,2,"=A1+A2".into());
    let res=executar(&mut wb,0,"sheet.move('A1:A2','D5');");
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert!(res.alterou);
    let aba=&wb.aba(0).unwrap().sheet;
    assert_eq!(aba.get_raw(0,0),"");
    assert_eq!(aba.get_raw(4,3),"10");
    assert_eq!(aba.get_raw(5,3),"20");
    assert_eq!(aba.get_raw(0,2),"=D5+D6");
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    let aba=&wb.aba(0).unwrap().sheet;
    assert_eq!(aba.get_raw(0,0),"10");
    assert_eq!(aba.get_raw(4,3),"");
    assert_eq!(aba.get_raw(0,2),"=A1+A2");
}

#[test]
fn move_macro_rollback_reverte_referencias_de_outra_aba() {
    let mut wb=Workbook::new();
    wb.criar_aba("Relatorio");
    let nome=wb.aba(0).unwrap().sheet.name.clone();
    wb.escrever_celula(0,0,0,"8".into());
    wb.escrever_celula(1,0,0,format!("={nome}!A1"));
    let res=executar(&mut wb,0,
        "sheet.range('A1').move('D5'); throw new Error('cancelar');");
    assert!(res.erro.is_some());
    assert!(!res.alterou);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"8");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(4,3),"");
    assert_eq!(wb.aba(1).unwrap().sheet.get_raw(0,0),
        format!("={nome}!A1"));
}

#[test]
fn move_macro_com_outra_edicao_reverte_toda_transacao() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"6".into());
    let res=executar(&mut wb,0,
        "sheet.set('B1',99); sheet.move('A1','C1'); throw new Error('falha');");
    assert!(res.erro.is_some());
    let aba=&wb.aba(0).unwrap().sheet;
    assert_eq!(aba.get_raw(0,0),"6");
    assert_eq!(aba.get_raw(0,1),"");
    assert_eq!(aba.get_raw(0,2),"");
}

#[test]
fn move_macro_destino_invalido_nunca_escreve() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"24".into());
    let res=executar(&mut wb,0,
        "sheet.move('A1:B2','XFD10000000');");
    assert!(res.erro.is_some());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"24");
    assert_eq!(wb.aba(0).unwrap().sheet.len(),1);
}

#[test]
fn copy_modo_formulas_atualiza_referencias_com_host() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"7".into());
    wb.escrever_celula(0,0,1,"=A1+1".into());
    let out=executar(&mut wb,0,
        "sheet.copy('B1','B2',{paste:'formulas'});");
    assert!(out.erro.is_none(), "{:?}",out.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,1),"=A2+1");
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,1),"");
}

#[test]
fn copy_modos_values_e_formats_com_api_js() {
    use feathersheet_core::{
        celulas::estilo_parcial::{Campo,EstiloParcial},
        celulas::refs::Rect,
    };
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"8".into());
    wb.escrever_celula(0,0,1,"=A1*2".into());
    wb.escrever_celula(0,0,2,"destino".into());
    let mut estilo=EstiloParcial::new();
    estilo.insert("bold".into(), Campo::Logico(true));
    wb.editar(0,|sheet|sheet.set_style(&Rect::cell(0,1),&estilo));
    let out=executar(&mut wb,0,
        "sheet.range('B1').copy('C1',{paste:'formats'}); sheet.copy('B1','C2',{paste:'values'});");
    assert!(out.erro.is_none(), "{:?}",out.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,2),"destino");
    assert!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,2).unwrap().bold);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,2),"16");
    assert!(!wb.aba(0).unwrap().sheet.estilo_resolvido(1,2)
        .is_some_and(|e|e.bold));
}

#[test]
fn copy_invalido_e_limite_de_destino_preservam_tudo() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"123".into());
    let erro=executar(&mut wb,0,
        "sheet.copy('A1:B2','XFD10000000',{paste:'all'});");
    assert!(erro.erro.is_some(), "limites validados antes de gravar");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"123");
    assert!(!erro.alterou);
    assert_eq!(wb.aba(0).unwrap().sheet.len(),1,
        "erro não deve ampliar a planilha");
}

#[test]
fn macro_r1c1_e_formula_relativa_preenchem_intervalo() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"10".into());
    wb.escrever_celula(0,0,1,"20".into());
    wb.escrever_celula(0,1,0,"30".into());
    wb.escrever_celula(0,1,1,"40".into());
    let res=executar(&mut wb,0,
        "sheet.range('C1:C2').setFormulaR1C1('=RC[-2]+RC[-1]'); sheet.setFormula('D1:D2','=A1+B1');");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,2),"=A1+B1");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,2),"=A2+B2");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,3),"=A1+B1");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1,3),"=A2+B2");
    assert_eq!(wb.valor(0,1,2),Value::Number(70.));
    assert!(wb.aba_mut(0).unwrap().sheet.undo(), "uma transação por macro");
    assert!(wb.aba(0).unwrap().sheet.get_raw(1,2).is_empty());
    assert!(wb.aba(0).unwrap().sheet.get_raw(0,3).is_empty());
}

#[test]
fn macro_formata_por_mascara_excel_e_desfaz_junto_com_valor() {
    use feathersheet_core::celulas::style::NumberFormat;
    let mut wb=Workbook::new();
    let res=executar(&mut wb,0,
        "sheet.set('A1',123.45); sheet.range('A1:B1').setNumberFormat('R$ #,##0.00');");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,0).unwrap().format,NumberFormat::Currency);
    assert_eq!(wb.aba(0).unwrap().sheet.estilo_resolvido(0,1).unwrap().decimals,Some(2));
    wb.aba_mut(0).unwrap().sheet.undo();
    assert_eq!(wb.valor(0,0,0),Value::Empty);
}

#[test]
fn macro_aceita_mascara_personalizada_e_limpa_ao_voltar_para_conhecida() {
    use crate::operacoes_valores::texto_exibido;
    let mut wb=Workbook::new();
    let res=executar(&mut wb,0,"sheet.set('A1',45000); sheet.set('A2',45000); sheet.set('A3',1234.5); sheet.setNumberFormat('A1','dd'); sheet.setNumberFormat('A2','mmmm/yyyy'); sheet.setNumberFormat('A3','#.##0,00 \"kWh\"');");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert_eq!(texto_exibido(&wb,0,0,0),"15");
    assert_eq!(texto_exibido(&wb,0,1,0),"março/2023");
    let res=executar(&mut wb,0,"sheet.setNumberFormat('A1','0.00');");
    assert!(res.erro.is_none(), "{:?}",res.erro);
    assert_eq!(texto_exibido(&wb,0,0,0),"45.000,00");
    let res=executar(&mut wb,0,"sheet.setNumberFormat('A1','zzz qqq');");
    assert!(res.erro.is_some());
}

#[test]
fn erro_na_formula_reverte_todas_as_celulas_da_macro() {
    let mut wb=Workbook::new();
    let res=executar(&mut wb,0,
        "sheet.set('A1',1); sheet.range('B1:B2').setFormulaR1C1('abc');");
    assert!(res.erro.is_some());
    assert_eq!(wb.valor(0,0,0),Value::Empty);
    assert_eq!(wb.valor(0,0,1),Value::Empty);
}

#[test]
fn macro_le_escreve_formula_e_desfaz_em_uma_operacao() {
    let mut wb = Workbook::new();
    let r = executar(&mut wb,0,"range('A1:B2').set([[2,'=A1*4'],[3,5]]); range('A2').map(v=>v+1); log(sheet.get('B1'));");
    assert!(r.erro.is_none(),"{:?}",r.erro);
    assert_eq!(wb.valor(0,0,1),Value::Number(8.));
    assert_eq!(wb.valor(0,1,0),Value::Number(4.));
    assert_eq!(r.saida,vec!["8"]);
    wb.editar(0,|s|{s.undo();});
    assert_eq!(wb.valor(0,0,0),Value::Empty);
    assert_eq!(wb.valor(0,1,0),Value::Empty);
}
#[test]
fn erro_reverte_escritas_e_preserva_edicoes_anteriores() {
    let mut wb = Workbook::new();
    wb.escrever_celula(0,0,0,"9".into());
    let r = executar(&mut wb,0,"sheet.set('A1',20); throw new Error('falhou');");
    assert!(r.erro.unwrap().contains("falhou"));
    assert_eq!(wb.valor(0,0,0),Value::Number(9.));
    assert!(!wb.aba(0).unwrap().sheet.can_redo());
    let r = executar(&mut wb,0,"sheet.set('A1',9); throw new Error('sem mudança');");
    assert!(r.erro.is_some());
    assert_eq!(wb.valor(0,0,0),Value::Number(9.));
    assert!(executar(&mut wb,0,"while(true) {}").erro.is_some());
    assert_eq!(wb.valor(0,0,0),Value::Number(9.));
}
#[test]
fn laco_acima_de_cem_mil_voltas_roda() {
    // #23: parava com RuntimeLimitError em 100 mil voltas.
    let mut wb = Workbook::new();
    let r = executar(&mut wb, 0, "let s = 0; for (let i = 0; i < 200000; i++) { s += i; } sheet.set('A1', s);");
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(wb.valor(0, 0, 0), Value::Number(19_999_900_000.0));
}
#[test]
fn get_text_retorna_valor_formatado_e_formula_calculada() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"12.5".into());
    wb.escrever_celula(0,0,1,"=A1*2".into());
    let res=executar(&mut wb,0,
        "sheet.setNumberFormat('A1','0.00'); log(sheet.getText('A1')); log(sheet.getText('B1')); log(sheet.range('A1').getText()); log(sheet.range('A1').text);");
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida.len(),4);
    assert!(res.saida[0].contains("12,50"),"{:?}",res.saida);
    assert_eq!(res.saida[1],"25");
    assert_eq!(res.saida[2],res.saida[0]);
    assert_eq!(res.saida[3],"12.5");
}
#[test]
fn get_text_booleano_erro_e_vazio_nao_altera_planilha() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"=1=1".into());
    wb.escrever_celula(0,0,1,"=1/0".into());
    let res=executar(&mut wb,0,
        "log(sheet.getText('A1')); log(sheet.getText('B1')); log('['+sheet.getText('C1')+']');");
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["VERDADEIRO","#DIV/0!","[]"]);
    assert!(!res.alterou);
}
#[test]
fn get_address_normaliza_intervalos_e_aceita_range_e_rect() {
    let mut wb=Workbook::new();
    let res=executar(&mut wb,0,
        "log(sheet.getAddress('D8:B2')); log(sheet.getAddress(sheet.range('A5:C9'))); log(sheet.range('B3:C3').getAddress()); log(sheet.getAddress({r0:1,c0:0,r1:3,c1:1})); log(sheet.getAddress('C:C')); log(sheet.getAddress({address:'EXTERNO!A1'}));");
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["B2:D8","A5:C9","B3:C3","A2:B4","C1:C100000000","EXTERNO!A1"]);
    assert!(!res.alterou);
    assert_eq!(wb.aba(0).unwrap().sheet.len(),0);
}
#[test]
fn get_text_coluna_inteira_nao_materializa_matriz() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,2,"Texto grande".into());
    let res=executar(&mut wb,0,
        "log(sheet.getText('C:C')); log(sheet.getAddress('A1:XFD10000000'));");
    assert!(res.erro.is_none(),"{:?}",res.erro);
    assert_eq!(res.saida,vec!["Texto grande","A1:XFD10000000"]);
    assert!(!res.alterou);
}
#[test]
fn leitura_invalida_reverte_outras_escritas_da_macro() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"123".into());
    let res=executar(&mut wb,0,"sheet.set('B1',99); sheet.getText('XFE1');");
    assert!(res.erro.is_some());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"123");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,1),"");
    assert!(!res.alterou);
}

#[test]
fn navegacao_macro_ultima_linha_e_coluna_ignorando_lacunas() {
    let mut wb=Workbook::new();
    for (r,c,v) in [
        (0,0,"Nome"),(0,1,"Nota"),(1,0,"Maria"),(1,1,"10"),
        (3,0,"Ana"),(3,2,"=B2*2"),
    ] { wb.escrever_celula(0,r,c,v.into()); }
    let out=executar(&mut wb,0,
        "log(sheet.getLastRow('A')); log(sheet.getLastRow(2)); log(sheet.getLastRow('D')); log(sheet.getLastColumn(4)); log(sheet.getLastColumn(3));");
    assert!(out.erro.is_none(),"{:?}",out.erro);
    assert_eq!(out.saida,vec!["4","2","0","3","0"]);
    assert!(!out.alterou);
}
#[test]
fn navegacao_macro_data_range_e_endereco_sao_somente_leitura() {
    let mut wb=Workbook::new();
    let vazio=executar(&mut wb,0,"log(sheet.getDataRange()===null);");
    assert!(vazio.erro.is_none(),"{:?}",vazio.erro);
    assert_eq!(vazio.saida,vec!["true"]);
    wb.escrever_celula(0,9,3,"dado".into());
    let ok=executar(&mut wb,0,
        "const r=sheet.getDataRange(); log(r.address); log(r.rows); log(r.columns); log(sheet.lastRow); log(sheet.lastColumn); log(sheet.address(10,4));");
    assert!(ok.erro.is_none(),"{:?}",ok.erro);
    assert_eq!(ok.saida,vec!["A1:D10","10","4","10","4","D10"]);
    assert!(!ok.alterou);
}
#[test]
fn navegacao_macro_validacao_invalida_preserva_rollback() {
    for code in [
        "sheet.set('A1',99); sheet.getLastRow(0);",
        "sheet.set('A1',99); sheet.getLastColumn(100000001);",
        "sheet.set('A1',99); sheet.address(0,1);",
        "sheet.set('A1',99); sheet.getLastRow('XFE');",
    ] {
        let mut wb=Workbook::new();
        let out=executar(&mut wb,0,code);
        assert!(out.erro.is_some(),"{:?}",out);
        assert!(!out.alterou);
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"");
    }
}

#[test]
fn macro_nomeada_ts_so_executa_quando_chamada() {
    let json = r#"[{"name":"Preencher","code":"sheet.range('D4').value=42; log('ok')","auto":true}]"#;
    let catalogo = feather_macros::Catalogo::importar_json(json).unwrap();
    let mut wb = Workbook::new();
    assert_eq!(wb.valor(0, 3, 3), Value::Empty);
    // 'auto' é apenas metadado; carregar a biblioteca não altera a pasta.
    let modulo = catalogo.obter("Preencher").unwrap();
    assert!(modulo.auto);
    assert_eq!(wb.valor(0, 3, 3), Value::Empty);
    let resultado = executar(&mut wb, 0, &modulo.code);
    assert_eq!(resultado.erro, None, "{:?}", resultado);
    assert!(resultado.alterou);
    assert_eq!(resultado.saida, vec!["ok"]);
    assert_eq!(wb.valor(0, 3, 3), Value::Number(42.0));
    wb.editar(0, |s| { s.undo(); });
    assert_eq!(wb.valor(0, 3, 3), Value::Empty);
}

#[test]
fn abas_e_ambiente_local() {
    let mut wb = Workbook::new();
    wb.criar_aba("Dados");
    let r = executar(&mut wb,0,"wb.sheet('Dados').range('A1').value=7; sheet.set('A1','=Dados!A1'); log(typeof fetch,typeof document,typeof process);");
    assert!(r.erro.is_none(),"{:?}",r.erro);
    assert_eq!(wb.valor(0,0,0),Value::Number(7.));
    assert_eq!(r.saida,vec!["undefined undefined undefined"]);
}

#[test]
fn macro_ordena_e_remove_duplicatas_uma_unica_vez_no_desfazer() {
    let mut wb = Workbook::new();
    for (linha, nome, valor) in [
        (1, "Joao", "1"), (2, "Ana", "3"), (3, "Joao", "2"),
    ] {
        wb.escrever_celula(0, linha, 0, nome.into());
        wb.escrever_celula(0, linha, 1, valor.into());
    }
    wb.escrever_celula(0, 0, 0, "Nome".into());
    wb.escrever_celula(0, 0, 1, "Valor".into());
    let r = executar(&mut wb, 0,
        "sheet.sort('A1:B4',[{col:'B',order:'desc'}],{header:true}); const n=sheet.removeDuplicates('A1:B4',['A'],{header:true}); log(n);");
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(r.saida, vec!["1"]);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1, 0), "Ana");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2, 1), "2");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(3, 0), "");
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(1, 0), "Joao");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2, 0), "Ana");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(3, 1), "2");
}

#[test]
fn macro_range_ordena_e_erro_de_coluna_reverte_toda_execucao() {
    let mut wb = Workbook::new();
    wb.escrever_celula(0, 0, 0, "3".into());
    wb.escrever_celula(0, 1, 0, "1".into());
    let ok = executar(&mut wb, 0, "sheet.range('A1:A2').sort([{col:1}]);");
    assert!(ok.erro.is_none(), "{:?}", ok.erro);
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0, 0), "1");
    assert!(wb.aba_mut(0).unwrap().sheet.undo());
    let erro = executar(&mut wb, 0,
        "sheet.set('C1',99); sheet.sort('A1:A2',[{col:'B'}]);");
    assert!(erro.erro.is_some());
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0, 0), "3");
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0, 2), "");
}

#[test]
fn copiar_a_planilha_inteira_copia_so_a_area_usada() {
    // `Cells.Copy` + `Range("A1").PasteSpecial` em outra aba (VBA convertido).
    let mut wb=Workbook::new();
    wb.inserir_aba("Destino", 1);
    wb.escrever_celula(0,0,0,"a".into());
    wb.escrever_celula(0,2,1,"b".into());
    let out=executar(&mut wb,0,
        "sheet.range('A1:XFD1048576').copy('A1',{sheet:'Destino'}); sheet.range('A:C').copy('E1');");
    assert!(out.erro.is_none(), "{:?}",out.erro);
    let destino=&wb.aba(1).unwrap().sheet;
    assert_eq!((destino.get_raw(0,0).as_str(), destino.get_raw(2,1).as_str()), ("a","b"));
    assert_eq!(wb.aba(0).unwrap().sheet.get_raw(2,5),"b", "colunas inteiras também");
}

#[test]
fn texto_para_colunas_com_delimitadores_destino_e_coluna_inteira() {
    let mut wb=Workbook::new();
    wb.escrever_celula(0,0,0,"Growatt-MIN-5000".into());
    wb.escrever_celula(0,1,0,"Potência:5 kW".into());
    wb.escrever_celula(0,2,0,"sem separador".into());
    let out=executar(&mut wb,0,
        "const n = sheet.range('A:A').textToColumns({delimiters:['-', ':']});\
         sheet.set('E1', n);");
    assert!(out.erro.is_none(), "{:?}",out.erro);
    let s=&wb.aba(0).unwrap().sheet;
    assert_eq!((s.get_raw(0,0).as_str(), s.get_raw(0,1).as_str(), s.get_raw(0,2).as_str()), ("Growatt","MIN","5000"));
    assert_eq!((s.get_raw(1,0).as_str(), s.get_raw(1,1).as_str()), ("Potência","5 kW"));
    assert_eq!(s.get_raw(2,0),"sem separador");
    assert_eq!(s.get_raw(0,4),"3", "linhas com texto (como o assistente da faixa)");
    // Destino em outro lugar: a origem fica.
    let mut wb=Workbook::new();
    wb.escrever_celula(0,3,0,"a;b".into());
    let out=executar(&mut wb,0,"sheet.textToColumns('A4',{delimiters:';',destination:'C10'});");
    assert!(out.erro.is_none(), "{:?}",out.erro);
    let s=&wb.aba(0).unwrap().sheet;
    assert_eq!((s.get_raw(3,0).as_str(), s.get_raw(9,2).as_str(), s.get_raw(9,3).as_str()), ("a;b","a","b"));
    assert!(executar(&mut wb,0,"sheet.textToColumns('A1:B2',{delimiters:';'});").erro.is_some(), "uma coluna só");
}

/// Como no VBA: `cell`/`range`/`cells` sem aba valem para a aba ativa
/// (onde a macro roda); a outra aba só quando escrita (`sheets("Nome")`).
#[test]
fn cell_e_range_sem_aba_usam_a_aba_ativa() {
    let mut wb = Workbook::new();
    let outra = wb.inserir_aba("Vendas", 1);
    let r = executar(&mut wb, outra,
        "cell('A1').value = 5; cells(2, 1).value = 6; range('B1:B2').set([[1],[2]]); \
         sheets(1).cell('A1').value = 7; sheets('Vendas').cell('C1').value = 8;");
    assert!(r.erro.is_none(), "{:?}", r.erro);
    assert_eq!(wb.valor(outra, 0, 0), Value::Number(5.));
    assert_eq!(wb.valor(outra, 1, 0), Value::Number(6.));
    assert_eq!(wb.valor(outra, 1, 1), Value::Number(2.));
    assert_eq!(wb.valor(outra, 0, 2), Value::Number(8.));
    assert_eq!(wb.valor(0, 0, 0), Value::Number(7.), "a primeira aba só pelo nome/número");
}

#[test]
fn to_locale_string_segue_a_lingua_pedida() {
    // #7: o Boa sem `intl` devolvia "Mon Oct 19 2026 08:41:50 GMT-0300".
    let mut wb = Workbook::new();
    let resultado = executar(&mut wb, 0,
        "const d = new Date(2026, 9, 19, 8, 5, 9); \
         log(d.toLocaleString('pt-BR')); log(d.toLocaleDateString()); log(d.toLocaleTimeString('en-US')); \
         log((833402).toLocaleString('pt-BR', {minimumFractionDigits: 2})); \
         log((833402.5).toLocaleString('pt-BR', {style: 'currency', currency: 'BRL'})); \
         log((-1234.5678).toLocaleString('en-US')); \
         log((0.358).toLocaleString('pt-BR', {style: 'percent', maximumFractionDigits: 1})); \
         log((1234.5).toLocaleString('es-ES', {style: 'currency', currency: 'EUR'}));");
    assert!(resultado.erro.is_none(), "{:?}", resultado.erro);
    assert_eq!(resultado.saida, vec![
        "19/10/2026, 08:05:09", "19/10/2026", "8:05:09 AM",
        "833.402,00", "R$ 833.402,50", "-1,234.568", "35,8 %", "1.234,50 €",
    ]);
}
