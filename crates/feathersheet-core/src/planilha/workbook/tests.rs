use super::*;

/// Uma pasta com duas abas para os testes do mover.
fn pasta_para_mover() -> Workbook {
    let mut w = Workbook::new();
    w.inserir_aba("Outra", 1);
    let e = |w: &mut Workbook, i: usize, r: u32, c: u32, t: &str| {
        w.escrever_celula(i, r, c, t.to_string());
    };
    e(&mut w, 0, 0, 0, "10"); // A1
    e(&mut w, 0, 1, 0, "=A1*2"); // A2
    e(&mut w, 0, 0, 2, "=A1+A2"); // C1, fora do bloco
    e(&mut w, 0, 5, 3, "velho"); // D6, será sobrescrito
    e(&mut w, 0, 6, 3, "=D6"); // D7, lê o sobrescrito
    e(&mut w, 1, 0, 0, "=Planilha1!A1+Planilha1!A2"); // Outra!A1
    w
}

#[test]
fn mover_leva_conteudo_e_as_referencias_acompanham() {
    let mut w = pasta_para_mover();
    // A1:A2 para D5:D6.
    w.mover_intervalo(0, Rect::new(0, 0, 1, 0), 4, 3).unwrap();
    let s = &w.aba(0).unwrap().sheet;
    assert_eq!(s.get_raw(0, 0), "");
    assert_eq!(s.get_raw(1, 0), "");
    assert_eq!(s.get_raw(4, 3), "10");
    // A fórmula movida lia a vizinha que foi junto: acompanha.
    assert_eq!(s.get_raw(5, 3), "=D5*2");
    assert_eq!(s.get_raw(0, 2), "=D5+D6");
    // D7 lia D6, que foi sobrescrito.
    assert_eq!(s.get_raw(6, 3), "=#REF!");
    assert_eq!(w.aba(1).unwrap().sheet.get_raw(0, 0), "=Planilha1!D5+Planilha1!D6");
    assert_eq!(w.valor(0, 0, 2), Value::Number(30.0));
    assert_eq!(w.valor(1, 0, 0), Value::Number(30.0));
}

#[test]
fn mover_se_desfaz_num_ctrl_z() {
    let mut w = pasta_para_mover();
    w.mover_intervalo(0, Rect::new(0, 0, 1, 0), 4, 3).unwrap();
    let s = &mut w.aba_mut(0).unwrap().sheet;
    assert!(s.undo());
    assert_eq!(s.get_raw(0, 0), "10");
    assert_eq!(s.get_raw(1, 0), "=A1*2");
    assert_eq!(s.get_raw(0, 2), "=A1+A2");
    assert_eq!(s.get_raw(5, 3), "velho");
    assert_eq!(s.get_raw(6, 3), "=D6");
    assert_eq!(s.get_raw(4, 3), "");
}

#[test]
fn mover_leva_o_estilo_e_a_formula_de_fora_continua_lendo_o_mesmo() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 1, "5".into()); // B1
    w.escrever_celula(0, 0, 0, "=B1+1".into()); // A1 lê B1, que fica
    let mut negrito = crate::celulas::estilo_parcial::EstiloParcial::new();
    negrito.insert("bold".into(), crate::celulas::estilo_parcial::Campo::Logico(true));
    w.aba_mut(0).unwrap().sheet.set_style(&Rect::new(0, 0, 0, 0), &negrito);
    w.mover_intervalo(0, Rect::new(0, 0, 0, 0), 2, 0).unwrap(); // A1 -> A3
    let s = &w.aba(0).unwrap().sheet;
    assert_eq!(s.get_raw(2, 0), "=B1+1");
    assert_eq!(w.valor(0, 2, 0), Value::Number(6.0));
    assert!(s.snapshot(2, 0).unwrap().style.is_some_and(|e| e.contains_key("bold")));
    assert!(s.snapshot(0, 0).is_none());
}

#[test]
fn mover_sobreposto_uma_linha_para_baixo() {
    let mut w = Workbook::new();
    for (r, t) in ["1", "2", "3"].iter().enumerate() {
        w.escrever_celula(0, r as u32, 0, t.to_string());
    }
    w.escrever_celula(0, 0, 1, "=SOMA(A1:A3)".into());
    w.mover_intervalo(0, Rect::new(0, 0, 2, 0), 1, 0).unwrap();
    let s = &w.aba(0).unwrap().sheet;
    assert_eq!((s.get_raw(0, 0), s.get_raw(1, 0), s.get_raw(3, 0)), ("".into(), "1".into(), "3".into()));
    assert_eq!(s.get_raw(0, 1), "=SOMA(A2:A4)");
    assert!(w.mover_intervalo(0, Rect::new(0, 0, 0, 0), -1, 0).is_none());
}

#[test]
fn mover_conserta_a_formula_colunar() {
    let mut w = Workbook::new();
    for r in 0..50u32 {
        w.escrever_celula(0, r, 0, format!("{r}"));
        w.escrever_celula(0, r, 1, format!("=A{}*2", r + 1));
    }
    // A10 vai para E10: só a fórmula B10 muda.
    w.mover_intervalo(0, Rect::new(9, 0, 9, 0), 0, 4).unwrap();
    let s = &w.aba(0).unwrap().sheet;
    assert_eq!(s.get_raw(9, 1), "=E10*2");
    assert_eq!(s.get_raw(10, 1), "=A11*2");
    assert_eq!(w.valor(0, 9, 1), Value::Number(18.0));
}

#[test]
fn a_pasta_nasce_com_uma_aba() {
    let w = Workbook::new();
    assert_eq!(w.len(), 1);
    assert_eq!(w.nomes(), vec!["Planilha1"]);
    assert_eq!(w.versao(0), 1);
}

#[test]
fn escrever_sobe_a_versao_da_aba() {
    let mut w = Workbook::new();
    let antes = w.versao(0);
    let m = w.escrever_celula(0, 0, 0, "10".into()).unwrap();
    assert_eq!(m.versao, antes + 1);
    assert_eq!(w.versao(0), m.versao);
}

#[test]
fn mexer_numa_aba_nao_invalida_o_quadro_da_outra() {
    let mut w = Workbook::new();
    w.criar_aba("Dados");
    let versao_da_primeira = w.versao(0);
    w.escrever_celula(1, 0, 0, "x".into());
    assert_eq!(
        w.versao(0),
        versao_da_primeira,
        "editar a Plan2 invalidou o quadro de quem está olhando a Plan1",
    );
}

#[test]
fn a_aba_e_achada_pelo_nome_sem_ligar_para_a_caixa() {
    let mut w = Workbook::new();
    w.criar_aba("Meu Plano");
    assert_eq!(w.indice_por_nome("meu plano"), Some(1));
    assert_eq!(w.indice_por_nome("MEU PLANO"), Some(1));
    assert_eq!(w.indice_por_nome("não existe"), None);
}

#[test]
fn o_valor_digitado_em_pt_br_vira_numero() {
    // O caminho inteiro: intenção do TypeScript -> `input.rs` -> `data`.
    // Era aqui que o motor guardava "199,9" como TEXTO, e a soma da
    // coluna dava zero.
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "199,9".into());
    w.escrever_celula(0, 1, 0, "1250".into());
    w.escrever_celula(0, 2, 0, "=SOMA(A1:A2)".into());
    let aba = w.aba(0).unwrap();
    assert_eq!(aba.sheet.get_value(2, 0), crate::celulas::values::Value::Number(1449.9));
}

#[test]
fn excluir_coluna_sobe_a_versao_e_move_o_conteudo() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "A".into());
    w.escrever_celula(0, 0, 1, "B".into());
    w.escrever_celula(0, 0, 2, "C".into());
    let antes = w.versao(0);

    let m = w.excluir_colunas(0, 1, 1).unwrap();
    assert!(m.versao > antes, "excluir coluna não subiu a versão da aba");

    let aba = w.aba(0).unwrap();
    assert_eq!(aba.sheet.get_value(0, 0), crate::celulas::values::Value::Text("A".into()));
    assert_eq!(aba.sheet.get_value(0, 1), crate::celulas::values::Value::Text("C".into()));
    assert_eq!(aba.sheet.get_value(0, 2), crate::celulas::values::Value::Empty);
}

#[test]
fn o_comando_devolve_onde_mexeu() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 5, 3, "x".into());
    let m = w.escrever_celula(0, 0, 0, "y".into()).unwrap();
    assert_eq!(m.sujos.len(), 1);
    // Grosseiro de propósito: o retângulo usado inteiro. Ver a nota do topo.
    assert!(m.sujos[0].r1 >= 5);
}

#[test]
fn aba_que_nao_existe_devolve_nada_em_vez_de_quebrar() {
    let mut w = Workbook::new();
    assert!(w.escrever_celula(9, 0, 0, "x".into()).is_none());
    assert!(w.excluir_colunas(9, 0, 1).is_none());
    assert_eq!(w.versao(9), 0);
}

#[test]
fn o_quadro_acompanha_a_versao() {
    use crate::planilha::frame::{build_frame, FRAME_SEM_MUDANCA};
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "10".into());

    let versao = w.versao(0);
    let sheet = &w.aba(0).unwrap().sheet;
    let cheio = build_frame(sheet, versao, 0, 0, 10, 10, None);
    assert!(cheio.len() > FRAME_SEM_MUDANCA);

    // Pedindo de novo com a versão que já tenho: nada mudou.
    let curto = build_frame(sheet, versao, 0, 0, 10, 10, Some(versao));
    assert_eq!(curto.len(), FRAME_SEM_MUDANCA);

    // Uma edição, e o quadro volta a vir inteiro.
    w.escrever_celula(0, 0, 1, "20".into());
    let nova = w.versao(0);
    let sheet = &w.aba(0).unwrap().sheet;
    assert!(build_frame(sheet, nova, 0, 0, 10, 10, Some(versao)).len() > FRAME_SEM_MUDANCA);
}

#[test]
fn formula_le_outra_aba_e_acompanha_a_mudanca_de_la() {
    let mut w = Workbook::new();
    let dados = w.criar_aba("Meu Plano");
    w.escrever_celula(dados, 0, 0, "10".into());
    w.escrever_celula(0, 0, 0, "='Meu Plano'!A1*2".into());
    assert_eq!(w.valor(0, 0, 0), crate::celulas::values::Value::Number(20.0));

    let antes = w.versao(0);
    w.escrever_celula(dados, 0, 0, "50".into());
    assert_eq!(w.valor(0, 0, 0), crate::celulas::values::Value::Number(100.0));
    assert!(w.versao(0) > antes, "a aba que lê a outra não soube que mudou");
}

#[test]
fn intervalo_nomeado_resolve_na_pasta() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "3".into());
    w.escrever_celula(0, 1, 0, "4".into());
    w.escrever_celula(0, 0, 1, "=SOMA(Precos)".into());
    w.definir_nome("Precos", Rect { r0: 0, c0: 0, r1: 1, c1: 0, sheet: Some("Planilha1".into()) });
    assert_eq!(w.valor(0, 0, 1), crate::celulas::values::Value::Number(7.0));
}

#[test]
fn somarproduto_avalia_expressao_de_matriz() {
    // #6: davam 0 e 1 (só o primeiro elemento).
    use crate::celulas::values::Value;
    let mut w = Workbook::new();
    for (i, (data, qtd, total)) in [("15/01/2026", "3", "10"), ("10/02/2026", "8", "20"), ("20/02/2026", "9", "5")].iter().enumerate() {
        w.escrever_celula(0, i as u32, 0, data.to_string());
        w.escrever_celula(0, i as u32, 1, qtd.to_string());
        w.escrever_celula(0, i as u32, 2, total.to_string());
    }
    w.escrever_celula(0, 0, 4, "=SOMARPRODUTO((B1:B3>5)*1)".into());
    w.escrever_celula(0, 0, 5, "=SOMARPRODUTO(MÊS(A1:A3))".into());
    w.escrever_celula(0, 0, 6, "=SOMARPRODUTO((MÊS(A1:A3)=2)*C1:C3)".into());
    w.escrever_celula(0, 0, 7, "=SOMARPRODUTO(B1:B3;C1:C3)".into());
    w.escrever_celula(0, 0, 8, "=SOMARPRODUTO(B1:B3;{1;2})".into());
    assert_eq!(w.valor(0, 0, 4), Value::Number(2.0));
    assert_eq!(w.valor(0, 0, 5), Value::Number(5.0));
    assert_eq!(w.valor(0, 0, 6), Value::Number(25.0));
    assert_eq!(w.valor(0, 0, 7), Value::Number(3.0 * 10.0 + 8.0 * 20.0 + 9.0 * 5.0));
    assert!(matches!(w.valor(0, 0, 8), Value::Error(_)), "tamanhos diferentes");
}

#[test]
fn nome_de_outra_aba_vale_onde_a_funcao_exige_intervalo() {
    // #5: SOMASES/CORRESP davam #VALUE! com nome; SOMA funcionava.
    use crate::celulas::values::Value;
    let mut w = Workbook::new();
    let vendas = w.criar_aba("Vendas");
    for (i, (regiao, total)) in [("Sul", "10"), ("Norte", "20"), ("Sul", "5")].iter().enumerate() {
        w.escrever_celula(vendas, i as u32, 0, regiao.to_string());
        w.escrever_celula(vendas, i as u32, 1, total.to_string());
    }
    w.definir_nome("Regioes", Rect::na_aba(0, 0, 2, 0, "Vendas"));
    w.definir_nome("TotalVendas", Rect::na_aba(0, 1, 2, 1, "Vendas"));
    w.escrever_celula(0, 0, 0, "=SOMASES(TotalVendas;Regioes;\"Sul\")".into());
    w.escrever_celula(0, 0, 1, "=CORRESP(MÁXIMO(TotalVendas);TotalVendas;0)".into());
    w.escrever_celula(0, 0, 2, "=SOMASE(Regioes;\"Sul\";TotalVendas)".into());
    w.escrever_celula(0, 0, 3, "=CONT.SES(Regioes;\"Sul\")".into());
    w.escrever_celula(0, 0, 4, "=PROCV(\"Norte\";Vendas!A1:B3;2;0)".into());
    assert_eq!(w.valor(0, 0, 0), Value::Number(15.0));
    assert_eq!(w.valor(0, 0, 1), Value::Number(2.0));
    assert_eq!(w.valor(0, 0, 2), Value::Number(15.0));
    assert_eq!(w.valor(0, 0, 3), Value::Number(2.0));
    assert_eq!(w.valor(0, 0, 4), Value::Number(20.0));
}

#[test]
fn renomear_aba_preserva_intervalos_nomeados_e_formula_de_outra_aba() {
    let mut w = Workbook::new();
    let dados = w.criar_aba("Dados");
    w.escrever_celula(dados, 0, 0, "10".into());
    w.escrever_celula(dados, 1, 0, "20".into());
    w.definir_nome("Receitas", Rect::na_aba(0, 0, 1, 0, "Dados"));
    w.escrever_celula(0, 0, 0, "=SOMA(Receitas)".into());
    assert_eq!(w.valor(0, 0, 0), crate::celulas::values::Value::Number(30.0));
    assert!(w.renomear_aba(dados, "Relatório"));
    assert_eq!(w.nomeados()[0].area.sheet.as_deref(), Some("Relatório"));
    assert_eq!(w.valor(0, 0, 0), crate::celulas::values::Value::Number(30.0));
}

#[test]
fn renomear_reescreve_as_formulas_que_citam_a_aba() {
    let mut w = Workbook::new();
    let d = w.criar_aba("Dados");
    w.escrever_celula(d, 0, 0, "5".into());
    w.escrever_celula(0, 0, 0, "=Dados!A1+dados!A1".into());
    assert!(w.renomear_aba(d, "Meu Plano"));
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(0, 0), "='Meu Plano'!A1+'Meu Plano'!A1");
    assert_eq!(w.valor(0, 0, 0), crate::celulas::values::Value::Number(10.0));
    // E de volta, tirando as aspas.
    assert!(w.renomear_aba(d, "Base"));
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(0, 0), "=Base!A1+Base!A1");
}

#[test]
fn reescrever_nao_pega_pedaco_de_outro_nome() {
    assert_eq!(reescrever_aba("=XDados!A1+Dados!B2", "Dados", "D"), "=XDados!A1+D!B2");
}

#[test]
fn o_id_sobrevive_a_apagar_e_mover() {
    let mut w = Workbook::new();
    w.criar_aba("B");
    w.criar_aba("C");
    let id_c = w.aba(2).unwrap().id;
    assert!(w.remover_aba(0));
    assert_eq!(w.indice_do_id(id_c), Some(1));
    w.mover_aba(1, 0);
    assert_eq!(w.indice_do_id(id_c), Some(0));
    assert!(w.remover_aba(0));
    assert!(!w.remover_aba(0), "apagou a última aba");
}

#[test]
fn excluir_coluna_conserta_o_texto_das_formulas() {
    use crate::celulas::values::Value;
    let mut w = Workbook::new();
    w.criar_aba("Outra");
    for (c, v) in ["1", "2", "3"].iter().enumerate() {
        w.escrever_celula(0, 0, c as u32, v.to_string());
    }
    w.escrever_celula(0, 0, 3, "=C1*10".into()); // colunar
    w.escrever_celula(0, 0, 4, "=SOMA(A1:C1)".into()); // avulsa
    w.escrever_celula(0, 0, 5, "=B1".into()); // aponta para o que sai
    w.escrever_celula(1, 0, 0, "=Planilha1!C1".into()); // de outra aba
    w.excluir_colunas(0, 1, 1);
    let s = &w.aba(0).unwrap().sheet;
    assert_eq!(s.get_raw(0, 2), "=B1*10");
    assert_eq!(w.valor(0, 0, 2), Value::Number(30.0));
    assert_eq!(s.get_raw(0, 3), "=SOMA(A1:B1)");
    assert_eq!(w.valor(0, 0, 3), Value::Number(4.0));
    assert!(matches!(w.valor(0, 0, 4), Value::Error(_)), "a referência excluída não virou erro");
    assert_eq!(w.aba(1).unwrap().sheet.get_raw(0, 0), "=Planilha1!B1");
    assert_eq!(w.valor(1, 0, 0), Value::Number(3.0));
}

#[test]
fn inserir_linha_move_o_texto_digitado_e_o_estilo_de_linha() {
    use crate::celulas::estilo_parcial::{Campo, EstiloParcial};
    let mut w = Workbook::new();
    w.escrever_celula(0, 3, 0, "007".into());
    w.escrever_celula(0, 4, 0, "=A4".into());
    let mut negrito = EstiloParcial::new();
    negrito.insert("bold".into(), Campo::Logico(true));
    w.editar(0, |s| s.set_style(&Rect::new(3, 0, 3, crate::celulas::refs::MAX_COLS - 1), &negrito));
    w.inserir_linhas(0, 1, 2);
    let s = &w.aba(0).unwrap().sheet;
    assert_eq!(s.get_raw(5, 0), "007");
    assert_eq!(s.get_raw(6, 0), "=A6");
    assert!(s.row_styles.contains_key(&5));
}

#[test]
fn referencia_estruturada_com_tabelas_lado_a_lado() {
    let mut w = Workbook::new();
    // Tabela 1 em col B: "Mãos com o Futuro"
    w.escrever_celula(0, 0, 1, "Mãos com o Futuro".into());
    w.escrever_celula(0, 1, 1, "Valor Total (R$)".into());
    w.escrever_celula(0, 2, 1, "1000".into());
    w.escrever_celula(0, 3, 1, "2000".into());

    // Tabela 2 em col E: "Projeto Olubajé"
    w.escrever_celula(0, 0, 4, "Projeto Olubajé".into());
    w.escrever_celula(0, 1, 4, "Valor Total (R$)".into());
    w.escrever_celula(0, 2, 4, "300".into());
    w.escrever_celula(0, 3, 4, "400".into());

    // Fórmula calculando a soma da segunda tabela
    w.escrever_celula(0, 5, 4, "=SOMA(Projeto_Olubajé[Valor Total (R$)])".into());

    assert_eq!(w.valor(0, 5, 4), crate::celulas::values::Value::Number(700.0));
}
#[test]
fn copiar_intervalo_acima_de_cem_mil_preserva_final_e_desfazer() {
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "inicio".into());
    w.escrever_celula(0, 100_000, 0, "=A1".into());
    assert!(w.copiar_intervalo(0, Rect::new(0, 0, 100_000, 0), 0, 1).is_some());
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(100_000, 1), "=B1");
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(0, 1), "inicio");
    assert!(w.aba_mut(0).unwrap().sheet.undo());
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(100_000, 1), "");
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(100_000, 0), "=A1");
}


// ------------------------------------------------------ duplicar aba

fn pasta_para_duplicar() -> Workbook {
    let mut w = Workbook::new();
    w.inserir_aba("Outra", 1);
    w.escrever_celula(0, 0, 0, "10".into()); // A1
    w.escrever_celula(0, 1, 0, "=A1*2".into()); // A2
    w.escrever_celula(0, 2, 0, "007".into()); // texto cru
    w.escrever_celula(1, 0, 0, "=Planilha1!A2+1".into());
    {
        let s = &mut w.aba_mut(0).unwrap().sheet;
        s.col_widths.insert(0, 150);
        s.row_heights.insert(1, 40);
        s.mc = Some("[[0,0,0,1]]".into());
        s.r = Some("[{\"regra\":1}]".into());
        s.tab_color = Some("#ff0000".into());
        let mut patch = crate::celulas::estilo_parcial::EstiloParcial::new();
        patch.insert("bold".into(), crate::celulas::estilo_parcial::Campo::Logico(true));
        s.set_style(&Rect { r0: 0, c0: 0, r1: 0, c1: 0, sheet: None }, &patch);
    }
    w
}

#[test]
fn duplicar_aba_copia_tudo_e_nomeia_a_copia() {
    let mut w = pasta_para_duplicar();
    let i = w.duplicar_aba(0).unwrap();
    assert_eq!(i, 1);
    assert_eq!(w.nomes(), vec!["Planilha1", "Planilha1 (2)", "Outra"]);
    let c = &w.aba(1).unwrap().sheet;
    assert_eq!(c.get_raw(0, 0), "10");
    assert_eq!(c.get_raw(1, 0), "=A1*2");
    assert_eq!(c.get_raw(2, 0), "007");
    assert_eq!(c.col_widths.get(&0), Some(&150));
    assert_eq!(c.row_heights.get(&1), Some(&40));
    assert_eq!(c.mc.as_deref(), Some("[[0,0,0,1]]"));
    assert!(c.r.is_some());
    assert_eq!(c.tab_color.as_deref(), Some("#ff0000"));
    assert_eq!(c.snapshot(0, 0).unwrap().style,
        w.aba(0).unwrap().sheet.snapshot(0, 0).unwrap().style);
    assert!(c.snapshot(0, 0).unwrap().style.is_some());
    assert_eq!(w.valor(1, 1, 0), Value::Number(20.0));
    // A cópia é independente da original.
    w.escrever_celula(1, 0, 0, "5".into());
    assert_eq!(w.valor(1, 1, 0), Value::Number(10.0));
    assert_eq!(w.valor(0, 1, 0), Value::Number(20.0));
    // Duplicar a cópia dá (3); duplicar de novo a original, também.
    assert_eq!(w.nome_da_copia("Planilha1 (2)"), "Planilha1 (3)");
    assert_eq!(w.nome_da_copia("Planilha1"), "Planilha1 (3)");
}

#[test]
fn ocultar_aba_preserva_dados_referencias_e_ultima_visivel() {
    let mut pasta = Workbook::new();
    pasta.escrever_celula(0, 0, 0, "42".into());
    let segunda = pasta.criar_aba("Resumo");
    pasta.escrever_celula(segunda, 0, 0, "=Planilha1!A1".into());
    assert_eq!(pasta.ocultar_aba(0), Ok(1));
    assert!(pasta.aba(0).unwrap().sheet.oculta);
    assert_eq!(pasta.valor(1, 0, 0), crate::Value::Number(42.0));
    assert_eq!(pasta.abas_visiveis().collect::<Vec<_>>(), vec![1]);
    assert!(pasta.ocultar_aba(1).is_err());
    assert!(!pasta.remover_aba(1), "não pode remover a última visível");
    assert!(pasta.reexibir_aba(0));
    assert_eq!(pasta.abas_visiveis().collect::<Vec<_>>(), vec![0, 1]);
    assert!(!pasta.reexibir_aba(0), "reexibir uma visível é operação nula");
}

#[test]
fn duplicar_aba_da_nome_novo_as_tabelas_da_copia() {
    let mut w = pasta_para_duplicar();
    w.aba_mut(0).unwrap().sheet.tb = Some(
        r#"[{"id":"tab_Tabela1","nome":"Tabela1","area":{"r0":0,"c0":0,"r1":2,"c1":0},"temCabecalho":true}]"#.into());
    w.escrever_celula(0, 3, 0, "=SOMA(Tabela1[x])+\"Tabela1[y]\"+Outra!Tabela1[z]".into());
    w.duplicar_aba(0).unwrap();
    let orig = w.aba(0).unwrap().sheet.tb.clone().unwrap();
    assert!(orig.contains("\"Tabela1\""));
    let v: serde_json::Value = serde_json::from_str(w.aba(1).unwrap().sheet.tb.as_deref().unwrap()).unwrap();
    assert_eq!(v[0]["nome"], "Tabela2");
    assert_eq!(v[0]["id"], "tab_Tabela2");
    assert_eq!(w.aba(0).unwrap().sheet.get_raw(3, 0), "=SOMA(Tabela1[x])+\"Tabela1[y]\"+Outra!Tabela1[z]");
    assert_eq!(w.aba(1).unwrap().sheet.get_raw(3, 0), "=SOMA(Tabela2[x])+\"Tabela1[y]\"+Outra!Tabela1[z]");
    // Duplicar de novo: terceiro nome.
    w.duplicar_aba(0).unwrap();
    let v: serde_json::Value = serde_json::from_str(w.aba(1).unwrap().sheet.tb.as_deref().unwrap()).unwrap();
    assert_eq!(v[0]["nome"], "Tabela3");
}

#[test]
fn duplicar_aba_nao_muda_formulas_de_outras_abas() {
    let mut w = pasta_para_duplicar();
    w.duplicar_aba(0).unwrap();
    // "Outra" agora é a terceira aba e ainda lê a original.
    assert_eq!(w.aba(2).unwrap().sheet.get_raw(0, 0), "=Planilha1!A2+1");
    assert_eq!(w.valor(2, 0, 0), Value::Number(21.0));
    w.escrever_celula(1, 0, 0, "999".into());
    assert_eq!(w.valor(2, 0, 0), Value::Number(21.0));
}

#[test]
fn duplicar_aba_desfaz_e_refaz() {
    let mut w = pasta_para_duplicar();
    w.duplicar_aba(0).unwrap();
    let id = w.aba(1).unwrap().id;
    assert!(w.pode_desfazer_aba());
    assert_eq!(w.desfazer_aba(), Some(0));
    assert_eq!(w.nomes(), vec!["Planilha1", "Outra"]);
    assert_eq!(w.valor(1, 0, 0), Value::Number(21.0));
    assert!(!w.pode_desfazer_aba());
    assert!(w.pode_refazer_aba());
    assert_eq!(w.refazer_aba(), Some(1));
    assert_eq!(w.nomes(), vec!["Planilha1", "Planilha1 (2)", "Outra"]);
    assert_eq!(w.aba(1).unwrap().id, id);
    assert_eq!(w.aba(1).unwrap().sheet.get_raw(1, 0), "=A1*2");
    assert_eq!(w.valor(1, 1, 0), Value::Number(20.0));
    assert!(w.pode_desfazer_aba());
}

#[test]
fn duplicar_aba_nao_desfaz_depois_de_outra_edicao() {
    let mut w = pasta_para_duplicar();
    w.duplicar_aba(0).unwrap();
    w.escrever_celula(0, 5, 5, "x".into());
    assert!(!w.pode_desfazer_aba());
    assert_eq!(w.desfazer_aba(), None);
    assert_eq!(w.len(), 3);
    // Nova duplicação invalida o refazer.
    let mut w = pasta_para_duplicar();
    w.duplicar_aba(0).unwrap();
    w.desfazer_aba();
    w.duplicar_aba(1).unwrap();
    assert!(!w.pode_refazer_aba());
}

#[test]
fn dia_da_semana_segue_o_tipo() {
    // #22: o tipo era ignorado (sempre domingo = 1). 01/03/2026 é domingo.
    use crate::celulas::values::Value;
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "01/03/2026".into());
    w.escrever_celula(0, 1, 0, "02/03/2026".into());
    let casos = [("A1", 1, 1.0), ("A1", 2, 7.0), ("A1", 3, 6.0), ("A1", 11, 7.0), ("A1", 12, 6.0),
        ("A1", 16, 2.0), ("A1", 17, 1.0), ("A2", 2, 1.0), ("A2", 3, 0.0)];
    for (i, (celula, tipo, _)) in casos.iter().enumerate() {
        w.escrever_celula(0, i as u32, 2, format!("=DIA.DA.SEMANA({celula};{tipo})"));
    }
    for (i, (celula, tipo, esperado)) in casos.iter().enumerate() {
        assert_eq!(w.valor(0, i as u32, 2), Value::Number(*esperado), "{celula} tipo {tipo}");
    }
    w.escrever_celula(0, 0, 3, "=DIA.DA.SEMANA(A1)".into());
    w.escrever_celula(0, 1, 3, "=DIA.DA.SEMANA(A1;4)".into());
    assert_eq!(w.valor(0, 0, 3), Value::Number(1.0));
    assert!(matches!(w.valor(0, 1, 3), Value::Error(_)), "tipo inválido");
}

#[test]
fn outra_aba_le_a_area_derramada_e_acompanha_a_ancora() {
    use crate::celulas::values::Value;
    let mut w = Workbook::new();
    let dados = w.criar_aba("Dados");
    w.escrever_celula(0, 0, 0, "=Dados!A2+1".into());
    w.escrever_celula(dados, 0, 0, "=SEQUÊNCIA(3)".into());
    assert_eq!(w.valor(0, 0, 0), Value::Number(3.0));
    w.escrever_celula(0, 0, 1, "=SOMA(Dados!A1:A3)".into());
    assert_eq!(w.valor(0, 0, 1), Value::Number(6.0));
    w.escrever_celula(dados, 0, 0, "=SEQUÊNCIA(3;1;10)".into());
    assert_eq!(w.valor(0, 0, 0), Value::Number(12.0));
    assert_eq!(w.valor(0, 0, 1), Value::Number(33.0));
}

#[test]
fn excluir_linha_leva_junto_a_formula_que_le_nome() {
    use crate::celulas::values::Value;
    let mut w = Workbook::new();
    w.escrever_celula(0, 0, 0, "1".into());
    w.escrever_celula(0, 1, 0, "2".into());
    w.definir_nome("Nm", Rect::na_aba(0, 0, 1, 0, "Planilha1"));
    w.escrever_celula(0, 6, 2, "=SOMA(Nm)".into());
    assert_eq!(w.valor(0, 6, 2), Value::Number(3.0));
    w.excluir_linhas(0, 3, 1);
    assert_eq!(w.valor(0, 5, 2), Value::Number(3.0));
    w.escrever_celula(0, 0, 0, "10".into());
    assert_eq!(w.valor(0, 5, 2), Value::Number(12.0));
}

#[test]
fn nome_acompanha_inserir_e_excluir_linha_e_coluna() {
    let mut w = Workbook::new();
    let dados = w.criar_aba("Dados");
    w.definir_nome("Nm", Rect::na_aba(0, 0, 3, 0, "Dados"));
    w.definir_nome("Outro", Rect::na_aba(5, 1, 5, 1, "Dados"));
    let area = |w: &Workbook, n: &str| w.nomeados().into_iter().find(|x| x.nome == n).map(|x| (x.area.r0, x.area.c0, x.area.r1, x.area.c1));
    w.inserir_linhas(dados, 0, 1);
    assert_eq!(area(&w, "Nm"), Some((1, 0, 4, 0)));
    w.excluir_linhas(dados, 2, 2);
    assert_eq!(area(&w, "Nm"), Some((1, 0, 2, 0)));
    w.inserir_colunas(dados, 0, 2);
    assert_eq!(area(&w, "Nm"), Some((1, 2, 2, 2)));
    w.excluir_colunas(dados, 3, 1);
    assert_eq!(area(&w, "Outro"), None, "área toda excluída");
    // Nome de outra aba não anda.
    w.inserir_linhas(0, 0, 5);
    assert_eq!(area(&w, "Nm"), Some((1, 2, 2, 2)));
}

#[test]
fn somarproduto_eleva_matriz_por_dentro() {
    let mut w = Workbook::new();
    for (i, v) in [3, 7, 9, 1].iter().enumerate() {
        w.escrever_celula(0, i as u32, 0, v.to_string());
    }
    w.escrever_celula(0, 0, 1, "=SOMARPRODUTO((A1:A4>5)*1)".into());
    w.escrever_celula(0, 1, 1, "=SOMARPRODUTO(A1:A4*2)".into());
    w.escrever_celula(0, 2, 1, "=SOMARPRODUTO(--(A1:A4>2))".into());
    assert_eq!(w.valor(0, 0, 1), Value::Number(2.0));
    assert_eq!(w.valor(0, 1, 1), Value::Number(40.0));
    assert_eq!(w.valor(0, 2, 1), Value::Number(3.0));
    // Datas de janeiro e março: MÊS também precisa virar matriz por dentro.
    w.escrever_celula(0, 0, 2, "=DATA(2026;1;5)".into());
    w.escrever_celula(0, 1, 2, "=DATA(2026;3;9)".into());
    w.escrever_celula(0, 3, 1, "=SOMARPRODUTO(MÊS(C1:C2))".into());
    assert_eq!(w.valor(0, 3, 1), Value::Number(4.0));
}

#[test]
fn nome_de_outra_aba_serve_de_intervalo_em_somases_e_corresp() {
    let mut w = Workbook::new();
    w.inserir_aba("Vendas", 1);
    for (i, (k, v)) in [("a", 10), ("b", 20), ("a", 5)].iter().enumerate() {
        w.escrever_celula(1, i as u32, 0, k.to_string());
        w.escrever_celula(1, i as u32, 1, v.to_string());
    }
    let mut chaves = Rect::new(0, 0, 2, 0); chaves.sheet = Some("Vendas".into());
    let mut valores = Rect::new(0, 1, 2, 1); valores.sheet = Some("Vendas".into());
    w.definir_nome("Chaves", chaves);
    w.definir_nome("Valores", valores);
    w.escrever_celula(0, 0, 0, "=SOMASES(Valores;Chaves;\"a\")".into());
    w.escrever_celula(0, 1, 0, "=CORRESP(\"b\";Chaves;0)".into());
    w.escrever_celula(0, 2, 0, "=SOMA(Valores)".into());
    assert_eq!(w.valor(0, 2, 0), Value::Number(35.0));
    assert_eq!(w.valor(0, 0, 0), Value::Number(15.0));
    assert_eq!(w.valor(0, 1, 0), Value::Number(2.0));
}
