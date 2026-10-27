use super::*;

#[test]
fn ocultar_e_reexibir_linhas_preserva_dados_e_desfaz_em_um_passo() {
    let mut s = Sheet::new("Dados");
    s.set_cell_raw(5, 2, "123".into(), None);
    s.clear_history();
    assert_eq!(s.alterar_visibilidade_dimensoes(false, true, 4, 6), Ok(3));
    for r in 4..=6 { assert_eq!(s.row_heights.get(&r), Some(&0)); }
    assert_eq!(s.get_raw(5, 2), "123");
    assert_eq!(s.profundidade_do_historico(), 1);
    assert!(s.undo());
    assert!(s.row_heights.is_empty());
    assert_eq!(s.get_raw(5, 2), "123");
    assert!(s.redo());
    assert_eq!(s.row_heights.get(&5), Some(&0));
    // A faixa encostada à seleção também volta a ficar visível.
    assert_eq!(s.alterar_visibilidade_dimensoes(false, false, 7, 7), Ok(3));
    assert!(s.row_heights.is_empty());
    assert!(s.undo());
    assert_eq!(s.row_heights.get(&4), Some(&0));
}

#[test]
fn ocultar_coluna_preserva_largura_personalizada_no_undo() {
    let mut s = Sheet::new("Dados");
    s.definir_dimensao(true, 3, 145);
    s.clear_history();
    assert_eq!(s.alterar_visibilidade_dimensoes(true, true, 3, 5), Ok(3));
    assert_eq!(s.col_widths.get(&3), Some(&0));
    assert!(s.undo());
    assert_eq!(s.col_widths.get(&3), Some(&145));
    assert!(s.col_widths.get(&4).is_none());
    assert!(s.col_widths.get(&5).is_none());
    assert_eq!(s.alterar_visibilidade_dimensoes(true, true, MAX_COLS, MAX_COLS).is_err(), true);
}

#[test]
fn reexibir_no_limite_final_e_intervalo_enorme_sao_seguros() {
    let mut s = Sheet::new("Dados");
    s.alterar_visibilidade_dimensoes(true, true, MAX_COLS - 1, MAX_COLS - 1).unwrap();
    assert_eq!(s.alterar_visibilidade_dimensoes(true, false, MAX_COLS - 1, MAX_COLS - 1), Ok(1));
    assert!(s.col_widths.is_empty());
    assert!(s.alterar_visibilidade_dimensoes(false, true, 0, MAX_ROWS - 1).is_err());
    assert!(s.row_heights.is_empty());
}

fn negrito() -> EstiloParcial {
    let mut e = EstiloParcial::new();
    e.insert("bold".into(), Campo::Logico(true));
    e
}

#[test]
fn spill_para_disco_preserva_undo_redo_e_remove_arquivo() {
    let mut sheet = Sheet::new("A");
    sheet.set_cell(0, 0, "Antes;Depois");
    sheet.clear_history();
    sheet.abrir_transacao();
    // Simula um histórico já próximo do teto, sem criar 24 MiB no teste.
    sheet.hist.memoria_divisao = LIMITE_HISTORIA_DIVISAO_RAM - 128;
    for r in 0..40 {
        sheet.set_cell_divisao(r, 0, &format!("novo-{r}"));
        sheet.set_cell_divisao(r, 1, &r.to_string());
    }
    sheet.compactar_lote_divisao();
    let pendente = sheet.hist.pendente.as_ref().unwrap();
    assert!(matches!(pendente.first(), Some(Passo::CompactoNoDisco(_))));
    let caminho = if let Passo::CompactoNoDisco(b) = &pendente[0] {
        assert!(b.origem.caminho.exists());
        b.origem.caminho.clone()
    } else { panic!("arquivo de histórico não foi criado") };
    sheet.fechar_transacao();
    assert_eq!(sheet.profundidade_do_historico(), 1);
    assert_eq!(sheet.celulas_do_proximo(false), vec![[0, 0, 39, 1]]);
    assert!(sheet.undo());
    assert_eq!(sheet.get_raw(0, 0), "Antes;Depois");
    assert_eq!(sheet.get_raw(39, 1), "");
    assert!(sheet.redo());
    assert_eq!(sheet.get_raw(0, 0), "novo-0");
    assert_eq!(sheet.get_raw(39, 1), "39");
    sheet.clear_history();
    assert!(!caminho.exists(), "arquivo de histórico deve ser eliminado");
}

#[test]
fn cancelar_spill_para_disco_restabelece_dados_sem_historico() {
    let mut sheet = Sheet::new("A");
    sheet.set_cell(0, 0, "antes;novo");
    sheet.clear_history();
    sheet.abrir_transacao();
    sheet.hist.memoria_divisao = LIMITE_HISTORIA_DIVISAO_RAM - 1;
    sheet.set_cell_divisao(0, 0, "antes");
    sheet.set_cell_divisao(0, 1, "novo");
    sheet.compactar_lote_divisao();
    let origem = sheet.hist.arquivo_divisao.as_ref().unwrap().caminho.clone();
    assert!(origem.exists());
    sheet.reverter_transacao();
    assert_eq!(sheet.get_raw(0, 0), "antes;novo");
    assert_eq!(sheet.get_raw(0, 1), "");
    assert!(!sheet.can_undo());
    assert!(!origem.exists(), "cancelamento deve remover arquivo temporário");
}

#[test]
fn blocos_compactos_de_divisao_nao_retêm_milhares_de_snaps_individuais() {
    let mut s = Sheet::new("A");
    for r in 0..5_000u32 {
        s.set_cell_raw(r, 0, format!("Nome;{r};Vitória"), None);
    }
    s.clear_history();
    s.abrir_transacao();
    for (inicio, fim) in [(0, 999), (1_000, 1_999), (2_000, 3_999), (4_000, 4_999)] {
        for r in inicio..=fim {
            s.set_cell_divisao(r, 0, "Nome");
            s.set_cell_divisao(r, 1, &r.to_string());
            s.set_cell_divisao(r, 2, "Vitória");
        }
        s.compactar_lote_divisao();
    }
    let pendente = s.hist.pendente.as_ref().unwrap();
    assert_eq!(pendente.len(), 4, "um registro compacto por lote");
    assert!(pendente.iter().all(|p| matches!(p, Passo::Compacto(_))));
    let mut alteradas=0;
    let mut bytes=0;
    for passo in pendente {
        if let Passo::Compacto(b) = passo {
            alteradas += b.alteracoes.len();
            bytes += b.textos.len();
        }
    }
    assert_eq!(alteradas, 15_000);
    assert!(bytes < 300_000, "buffers brutos são limitados às strings convertidas");
    s.fechar_transacao();
    assert_eq!(s.profundidade_do_historico(), 1);
    assert_eq!(s.celulas_do_proximo(false).len(), 4);
    assert!(s.undo());
    assert_eq!(s.get_raw(0, 0), "Nome;0;Vitória");
    assert_eq!(s.get_raw(4_999, 0), "Nome;4999;Vitória");
    assert_eq!(s.get_raw(0, 1), "");
    assert_eq!(s.get_raw(4_999, 2), "");
    assert!(s.redo());
    assert_eq!(s.get_raw(0, 0), "Nome");
    assert_eq!(s.get_raw(4_999, 1), "4999");
    assert_eq!(s.get_raw(4_999, 2), "Vitória");
}

#[test]
fn lote_compacto_cancelado_restaura_texto_e_estilo() {
    let mut s = Sheet::new("A");
    let mut negrito = EstiloParcial::new();
    negrito.insert("bold".into(), Campo::Logico(true));
    s.apply(0, 0, Some(Snap { raw: "ABC;1".into(), style: Some(negrito.clone()) }), false);
    s.clear_history();
    s.abrir_transacao();
    s.set_cell_divisao(0, 0, "ABC");
    s.set_cell_divisao(0, 1, "1");
    s.compactar_lote_divisao();
    s.reverter_transacao();
    assert_eq!(s.get_raw(0, 0), "ABC;1");
    assert_eq!(s.snapshot(0, 0).unwrap().style, Some(negrito));
    assert_eq!(s.get_raw(0, 1), "");
    assert!(!s.can_undo());
}

#[test]
fn conversao_parse_unico_preserva_formatos_implicitos_e_textos_crus() {
    let mut s=Sheet::new("Teste");
    s.abrir_transacao();
    for (col, raw) in ["15%", "R$ 25,50", "0012",
        "TRUE", "São Paulo", "  texto  ", "123.0"].iter().enumerate() {
        s.set_cell_divisao(0, col as u32, raw);
    }
    s.compactar_lote_divisao();
    s.fechar_transacao();
    for (col, raw) in ["15%", "R$ 25,50", "0012",
        "TRUE", "São Paulo", "  texto  ", "123.0"].iter().enumerate() {
        assert_eq!(s.get_raw(0,col as u32),*raw);
    }
    assert!(s.undo());
    for col in 0..7 {
        assert_eq!(s.get_raw(0,col),"");
    }
    assert!(s.redo());
    assert_eq!(s.get_raw(0,0),"15%");
    assert_eq!(s.get_raw(0,1),"R$ 25,50");
    assert_eq!(s.get_raw(0,3),"TRUE");
}

#[test]
fn escrita_divisao_em_lote_preserva_formato_raw_e_undo_unico() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "antes;valor");
    s.clear_history();
    s.abrir_transacao();
    s.set_cell_divisao(0, 0, "123");
    s.set_cell_divisao(0, 1, "0012");
    s.set_cell_divisao(0, 2, "São Paulo");
    s.fechar_transacao();
    assert_eq!(s.get_value(0, 0), Value::Number(123.0));
    assert_eq!(s.get_raw(0, 1), "0012");
    assert_eq!(s.get_value(0, 2), Value::Text("São Paulo".into()));
    assert_eq!(s.profundidade_do_historico(), 1);
    assert!(s.undo());
    assert_eq!(s.get_raw(0, 0), "antes;valor");
    assert_eq!(s.get_raw(0, 1), "");
    assert!(s.redo());
    assert_eq!(s.get_raw(0, 1), "0012");
}

#[test]
fn escrita_divisao_invalida_formula_dependente() {
    let mut s=Sheet::new("A");
    s.set_cell(0, 0, "10");
    s.set_cell(0, 1, "=A1*2");
    assert_eq!(s.get_value(0, 1), Value::Number(20.0));
    s.abrir_transacao();
    s.set_cell_divisao(0, 0, "50");
    s.fechar_transacao();
    assert_eq!(s.get_value(0, 1), Value::Number(100.0));
    assert!(s.undo());
    assert_eq!(s.get_value(0, 1), Value::Number(20.0));
}

#[test]
fn o_texto_digitado_que_nao_volta_do_valor_fica_guardado() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "007");
    s.set_cell(1, 0, "1.50");
    assert_eq!(s.get_raw(0, 0), "007");
    assert_eq!(s.get_raw(1, 0), "1.50");
    assert_eq!(s.get_value(0, 0), Value::Number(7.0));
}

/// O `TextosCrus` guarda o texto digitado; desfazer e refazer têm que
/// devolver o mesmo raw, sem perder a célula nem trocar a forma.
#[test]
fn texto_digitado_sobrevive_a_set_cell_e_a_desfazer_refazer() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "15.0");
    s.set_cell(1, 0, "True");
    s.set_cell(2, 0, "15.0");
    assert_eq!(s.get_raw(0, 0), "15.0");
    assert_eq!(s.get_raw(1, 0), "True");
    assert_eq!(s.textos.len(), 3, "as três células guardam o texto digitado");
    assert_eq!(s.textos.formas_distintas(), 2, "\"15.0\" repetido não cria forma nova");

    s.set_cell(0, 0, "x");
    assert_eq!(s.get_raw(0, 0), "x");
    assert!(s.undo());
    assert_eq!(s.get_raw(0, 0), "15.0");
    assert!(s.redo());
    assert_eq!(s.get_raw(0, 0), "x");
    assert!(s.undo());
    assert_eq!(s.get_raw(1, 0), "True");
    assert_eq!(s.get_raw(2, 0), "15.0");
}

/// Achado em 23/09/2026 (teste do Ctrl+; no motor Rust): a data digitada
/// voltava como o número de série (46288) — a barra de fórmulas do app
/// mostraria o número. No TypeScript o texto digitado fica guardado.
#[test]
fn a_data_digitada_fica_como_foi_digitada() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "23/09/2026");
    s.set_cell(1, 0, "08:30");
    assert_eq!(s.get_raw(0, 0), "23/09/2026");
    assert_eq!(s.get_value(0, 0), Value::Number(46288.0));
    assert_eq!(s.get_raw(1, 0), "08:30");
}

#[test]
fn a_data_colada_em_lote_tambem_fica_como_foi_digitada() {
    let mut s = Sheet::new("A");
    s.paste(0, 0, &[vec!["23/09/2026".to_string(), "1,5".to_string()]], false);
    assert_eq!(s.get_raw(0, 0), "23/09/2026", "o CSV com data não pode virar 46288");
    assert_eq!(s.get_value(0, 0), Value::Number(46288.0));
    assert_eq!(s.get_value(0, 1), Value::Number(1.5));
}

#[test]
fn mesclagem_metadado_tem_um_desfazer_e_um_refazer() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "Título");
    s.set_cell(0, 1, "Dado preservado");
    let intervalo = crate::celulas::refs::Rect::new(0, 0, 0, 2);
    let json = r#"[{"r0":0,"c0":0,"r1":0,"c1":2}]"#.to_owned();
    assert!(s.alterar_mesclas(Some(json.clone()), &intervalo));
    assert!(!s.alterar_mesclas(Some(json.clone()), &intervalo));
    assert_eq!(s.celulas_do_proximo(false), vec![[0, 0, 0, 2]]);
    assert!(s.undo());
    assert_eq!(s.mc, None);
    assert_eq!(s.get_raw(0, 1), "Dado preservado");
    assert!(s.redo());
    assert_eq!(s.mc.as_deref(), Some(json.as_str()));
}

#[test]
fn dinamica_desfaz_celulas_e_metadados_na_mesma_transacao() {
    let mut s = Sheet::new("Dados");
    let area = Rect::new(0, 1, 2, 2);
    let definicao = r#"[{"id":"dinamica1","areaResultado":{"r0":0,"c0":1,"r1":2,"c1":2}}]"#.to_owned();
    s.abrir_transacao();
    s.set_cell(0, 1, "Categoria");
    s.set_cell(1, 1, "Sul");
    s.set_cell(1, 2, "125");
    assert!(s.alterar_dinamicas(Some(definicao.clone()), &area));
    s.fechar_transacao();

    assert_eq!(s.dn.as_deref(), Some(definicao.as_str()));
    assert_eq!(s.get_raw(1, 2), "125");
    assert!(s.undo());
    assert!(s.dn.is_none());
    assert!(s.get_raw(1, 2).is_empty());
    assert!(!s.can_undo(), "a dinâmica inteira é um único passo");
    assert!(s.redo());
    assert_eq!(s.dn.as_deref(), Some(definicao.as_str()));
    assert_eq!(s.get_raw(1, 2), "125");
}

#[test]
fn dinamica_grande_compactada_continua_com_undo_atomico() {
    let mut s = Sheet::new("Dados");
    let rect = Rect::new(0,0,16_399,1);
    s.abrir_transacao();
    for row in 0..16_400_u32 {
        s.set_cell(row,1,"7");
        if (row + 1) % 4096 == 0 { s.compactar_lote_divisao(); }
    }
    s.compactar_lote_divisao();
    s.alterar_dinamicas(Some("definicao-v1".into()),&rect);
    s.fechar_transacao();

    assert_eq!(s.dn.as_deref(),Some("definicao-v1"));
    assert_eq!(s.get_raw(16_399,1),"7");
    assert!(s.undo());
    assert!(s.dn.is_none());
    assert!(s.get_raw(16_399,1).is_empty());
    assert!(!s.can_undo());
    assert!(s.redo());
    assert_eq!(s.dn.as_deref(),Some("definicao-v1"));
    assert_eq!(s.get_raw(16_399,1),"7");
}

#[test]
fn historico_de_estilo_tabela_nao_copia_as_celulas() {
    let mut s=Sheet::new("Dados");
    s.set_cell(0,0,"Produto");
    s.set_cell(1,0,"Caneta");
    let anterior=r#"[{"nome":"Tabela1","estilo":{"tema":"azul"}}]"#.to_string();
    let posterior=r#"[{"nome":"Tabela1","estilo":{"tema":"verde"}}]"#.to_string();
    s.tb=Some(anterior.clone());
    assert!(s.alterar_tabelas(Some(posterior.clone()),&Rect::new(0,0,1000000,8)));
    assert_eq!(s.celulas_do_proximo(false),vec![[0,0,1000000,8]]);
    assert!(s.undo());
    assert_eq!(s.tb.as_deref(),Some(anterior.as_str()));
    assert_eq!(s.get_raw(1,0),"Caneta");
    assert!(s.redo());
    assert_eq!(s.tb.as_deref(),Some(posterior.as_str()));
    assert_eq!(s.get_raw(0,0),"Produto");
    assert!(!s.alterar_tabelas(Some(posterior),&Rect::cell(0,0)));
}

#[test]
fn metadados_de_tabela_agrupam_multiplas_mudancas_em_um_undo() {
    let mut s=Sheet::new("Dados");
    s.tb=Some("estado0".into());
    s.abrir_transacao();
    s.alterar_tabelas(Some("estado1".into()),&Rect::new(0,0,9,3));
    s.alterar_tabelas(Some("estado2".into()),&Rect::new(0,0,9,3));
    s.fechar_transacao();
    assert!(s.undo());
    assert_eq!(s.tb.as_deref(),Some("estado0"));
    assert!(!s.can_undo());
    assert!(s.redo());
    assert_eq!(s.tb.as_deref(),Some("estado2"));
}

/// O colar rápido (abrir CSV) deixou de sujar cada literal; quem lê as
/// células coladas ainda tem de recalcular — pelo grafo e pelo padrão.
#[test]
fn colar_rapido_ainda_recalcula_quem_le() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 2, "=SOMA(A1:A3)");
    s.set_cell(0, 3, "=B1*2");
    assert_eq!(s.get_value(0, 2), Value::Number(0.0));
    assert_eq!(s.get_value(0, 3), Value::Number(0.0));
    s.paste(0, 0, &[vec!["1".into(), "5".into()], vec!["2".into()], vec!["3".into()]], false);
    assert_eq!(s.get_value(0, 2), Value::Number(6.0), "o intervalo recalcula");
    assert_eq!(s.get_value(0, 3), Value::Number(10.0), "a referência simples recalcula");
    // Fórmula por cima de fórmula já calculada: o literal novo apaga o valor velho.
    s.paste(0, 3, &[vec!["7".into()]], false);
    assert_eq!(s.get_value(0, 3), Value::Number(7.0));
    assert_eq!(s.get_raw(0, 3), "7");
}

/// Achado ao abrir um CSV do Spotify (23/09/2026): o dicionário de texto
/// tinha teto de 100 mil textos diferentes, e passado dele a célula de
/// texto simplesmente não era guardada — sumia calada. Coluna de ID ou de
/// nome de faixa passa disso em poucas dezenas de milhares de linhas. O
/// `pool.ts` do TypeScript tem o mesmo teto (bug do motor antigo).
#[test]
fn texto_nao_some_depois_de_cem_mil_diferentes() {
    let mut s = Sheet::new("A");
    let linhas: Vec<Vec<String>> = (0..150_000).map(|i| vec![format!("id-{i}")]).collect();
    s.paste(0, 0, &linhas, false);
    assert_eq!(s.get_raw(149_999, 0), "id-149999");
    assert_eq!(s.get_value(120_000, 0), Value::Text("id-120000".into()));
    s.set_cell(150_000, 0, "=A150000");
    assert_eq!(s.get_value(150_000, 0), Value::Text("id-149999".into()), "nem o texto calculado por fórmula");
}

/// Os casos do `test/core/table.test.ts`.
fn com_bloco() -> Sheet {
    let mut s = Sheet::new("A");
    let dados = [["Categoria", "Vendedor", "Valor"], ["Eletrônicos", "Ana", "1000"], ["Móveis", "Bruno", "500"], ["Móveis", "Ana", "300"]];
    for (r, linha) in dados.iter().enumerate() {
        for (c, v) in linha.iter().enumerate() {
            s.set_cell(2 + r as u32, 2 + c as u32, v);
        }
    }
    s.set_cell(20, 20, "ilha");
    s
}

#[test]
fn regiao_da_tabela_como_no_typescript() {
    let mut s = com_bloco();
    for (r, c) in [(2, 2), (3, 3), (5, 4), (4, 2)] {
        assert_eq!(s.regiao_da_tabela(r, c), Rect::new(2, 2, 5, 4), "a partir de {r},{c}");
    }
    assert_eq!(s.regiao_da_tabela(20, 20), Rect::new(20, 20, 20, 20), "não atravessa a linha vazia");
    assert_eq!(s.regiao_da_tabela(10, 10), Rect::new(10, 10, 10, 10), "célula vazia é só ela");
    let mut l = Sheet::new("L");
    for (r, c, v) in [(0, 0, "a"), (1, 0, "b"), (1, 1, "c"), (2, 1, "d")] {
        l.set_cell(r, c, v);
    }
    assert_eq!(l.regiao_da_tabela(0, 0), Rect::new(0, 0, 2, 1), "cresce em L");
}

#[test]
fn desfazer_e_refazer_voltam_o_texto_e_o_estilo() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "10");
    s.set_style(&Rect::new(0, 0, 0, 0), &negrito());
    s.set_cell(0, 0, "20");
    assert!(s.undo());
    assert_eq!(s.get_raw(0, 0), "10");
    assert!(s.estilo_mesclado(0, 0).unwrap().contains_key("bold"));
    assert!(s.undo());
    assert!(s.estilo_mesclado(0, 0).is_none());
    assert!(s.undo());
    assert_eq!(s.get_raw(0, 0), "");
    assert!(!s.undo());
    assert!(s.redo());
    assert_eq!(s.get_raw(0, 0), "10");
}

#[test]
fn uma_transacao_desfaz_de_uma_vez() {
    let mut s = Sheet::new("A");
    s.abrir_transacao();
    s.set_cell(0, 0, "1");
    s.set_cell(1, 0, "2");
    s.fechar_transacao();
    assert!(s.undo());
    assert!(!s.has_content(0, 0) && !s.has_content(1, 0));
}

#[test]
fn reverter_a_transacao_volta_o_que_ja_foi_escrito() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "antes");
    s.abrir_transacao();
    s.set_cell(0, 0, "depois");
    s.reverter_transacao();
    assert_eq!(s.get_raw(0, 0), "antes");
    // E não empilhou nada além da primeira escrita.
    assert!(s.undo());
    assert!(!s.can_undo());
}

#[test]
fn celula_so_de_estilo_existe_sem_conteudo() {
    let mut s = Sheet::new("A");
    s.set_style(&Rect::new(3, 3, 3, 3), &negrito());
    assert!(!s.has_content(3, 3));
    assert_eq!(s.snapshot(3, 3).unwrap().raw, "");
    assert_eq!(s.max_row_ts(), 3);
    s.clear(&Rect::new(3, 3, 3, 3));
    assert!(s.snapshot(3, 3).is_none());
    assert_eq!(s.max_row_ts(), 0);
}

#[test]
fn estilo_de_coluna_inteira_nao_mexe_em_celula() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "x");
    s.set_style(&Rect::new(0, 1, MAX_ROWS - 1, 1), &negrito());
    assert!(s.col_styles.contains_key(&1));
    assert!(s.estilo_mesclado(500, 1).unwrap().contains_key("bold"));
    assert!(s.estilo_mesclado(0, 0).is_none());
}

/// Bug achado em 23/09/2026 (o teste antigo do `engine.test.ts`, portado):
/// com dados só na coluna A, formatar a coluna A inteira pintava a aba
/// INTEIRA. A seleção de coluna inteira "cobria a área usada" e caía na
/// regra do Ctrl+A. O motor TypeScript tem o mesmo defeito.
#[test]
fn coluna_inteira_nao_vira_aba_inteira_quando_os_dados_moram_nela() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "Cabeçalho");
    s.set_cell(1, 0, "100");
    s.set_style(&Rect::new(0, 0, MAX_ROWS - 1, 0), &negrito());
    assert!(s.col_styles.contains_key(&0));
    assert!(s.sheet_style.is_none(), "a aba não ganha estilo");
    assert!(s.estilo_mesclado(100, 1).is_none(), "a coluna B continua sem negrito");

    // A linha inteira, com dados só na linha 1: o mesmo.
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "x");
    s.set_cell(0, 3, "y");
    s.set_style(&Rect::new(0, 0, 0, MAX_COLS - 1), &negrito());
    assert!(s.row_styles.contains_key(&0));
    assert!(s.estilo_mesclado(1, 0).is_none(), "a linha 2 continua sem negrito");

    // Limpar a coluna A inteira não apaga o estilo das outras.
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "x");
    s.set_style(&Rect::new(0, 1, MAX_ROWS - 1, 1), &negrito());
    s.clear(&Rect::new(0, 0, MAX_ROWS - 1, 0));
    assert!(s.col_styles.contains_key(&1), "o negrito da coluna B fica");
}

/// O Ctrl+A seleciona exatamente a área usada: continua valendo como aba
/// inteira (é o que evita estilo célula a célula em milhões de linhas).
#[test]
fn ctrl_a_na_area_usada_continua_sendo_a_aba_inteira() {
    let mut s = Sheet::new("A");
    for r in 0..60 {
        s.set_cell(r, 0, "1");
        s.set_cell(r, 2, "2");
    }
    s.set_style(&Rect::new(0, 0, 59, 2), &negrito());
    assert!(s.sheet_style.is_some());
}

#[test]
fn digitar_porcentagem_pede_o_formato() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "10%");
    assert_eq!(formato(s.estilo_mesclado(0, 0).as_ref()).as_deref(), Some("percent"));
    assert_eq!(s.get_value(0, 0), Value::Number(0.1));
}

#[test]
fn colar_sem_desfazer_perde_o_zero_como_no_typescript() {
    // Regra 12: traduzir, não melhorar.
    let mut s = Sheet::new("A");
    s.paste(0, 0, &[vec!["007".into(), "abc".into(), "=A1+1".into()]], false);
    assert_eq!(s.get_raw(0, 0), "7");
    assert_eq!(s.get_value(0, 2), Value::Number(8.0));
    assert!(!s.can_undo());
}

#[test]
fn formula_que_le_outra_aba_fica_marcada() {
    let mut s = Sheet::new("A");
    s.set_cell(0, 0, "=Dados!A1");
    s.set_cell(0, 1, "=A1");
    assert!(s.tem_formulas_externas());
    assert!(s.formulas.le_de_fora_em(0, 0));
    assert!(!s.externas.contains(&key(0, 1)));
    // Sem a pasta em volta, a outra aba não existe.
    assert!(matches!(s.get_value(0, 0), Value::Error(_)));
}
#[test]
fn ctrl_seta_salta_ate_a_borda_dos_dados() {
    let mut s = Sheet::new("A");
    for r in (0..10).chain(20..30) {
        s.set_cell_raw(r, 0, (r + 1).to_string(), None);
    }
    let (lr, lc) = (9_999_999, 16_383);
    assert_eq!(s.borda_dos_dados(0, 0, 1, 0, lr, lc), (9, 0), "desce até o fim do bloco");
    assert_eq!(s.borda_dos_dados(9, 0, 1, 0, lr, lc), (20, 0), "atravessa o vazio até o próximo");
    assert_eq!(s.borda_dos_dados(29, 0, 1, 0, lr, lc), (lr, 0), "depois do último, vai ao fim da planilha");
    assert_eq!(s.borda_dos_dados(lr, 0, -1, 0, lr, lc), (29, 0), "do fim sobe direto até o último");
    assert_eq!(s.borda_dos_dados(25, 0, -1, 0, lr, lc), (20, 0));
    assert_eq!(s.borda_dos_dados(0, 0, -1, 0, lr, lc), (0, 0), "na borda, fica");
    assert_eq!(s.borda_dos_dados(0, 0, 0, 1, lr, lc), (0, lc), "à direita, vazio até o fim");
}
#[test]
fn validacoes_json_entram_no_mesmo_undo_e_rollback_das_celulas() {
    let mut s=Sheet::new("Dados");
    let a=crate::celulas::refs::Rect::new(0,0,9,0);
    s.abrir_transacao();
    s.set_cell(0,0,"123");
    assert!(s.alterar_validacoes(Some("[{\"regra\":1}]".into()),&a));
    s.fechar_transacao();
    assert!(s.va.as_ref().is_some_and(|v|v.contains("regra")));
    assert!(s.undo());
    assert!(s.va.is_none());
    assert_eq!(s.get_raw(0,0),"");
    assert!(s.redo());
    assert_eq!(s.get_raw(0,0),"123");
    s.abrir_transacao();
    assert!(s.alterar_validacoes(Some("[]".into()),&a));
    s.reverter_transacao();
    assert!(s.va.as_ref().is_some_and(|v|v.contains("regra")));
}


#[test]
fn regiao_da_tabela_junta_colunas_que_acabam_em_linhas_diferentes() {
    let mut s = Sheet::new("Escada");
    // A: linhas 0–5; B: linhas 3–70.000 (passa da divisa de bloco); C: 60.000–70.010.
    for r in 0..=5 { s.set_cell(r, 0, "a"); }
    for r in 3..=70_000 { s.set_cell(r, 1, "1"); }
    for r in 60_000..=70_010 { s.set_cell(r, 2, "2"); }
    assert_eq!(s.regiao_da_tabela(0, 0), Rect::new(0, 0, 70_010, 2));
    assert_eq!(s.regiao_da_tabela(70_010, 2), Rect::new(0, 0, 70_010, 2), "sobe até a linha 0");
}

#[test]
fn desfazer_varias_celulas_de_uma_vez_recalcula_quem_depende() {
    use crate::celulas::values::Value;
    let mut s = Sheet::new("Lote");
    for r in 0..3 { s.set_cell(r, 0, "1"); }
    s.set_cell(0, 1, "=SOMA(A1:A3)");
    s.set_cell(1, 1, "=B1*2");
    assert_eq!(s.get_value(1, 1), Value::Number(6.0));
    s.abrir_transacao();
    for r in 0..3 { s.set_cell(r, 0, "10"); }
    s.fechar_transacao();
    assert_eq!(s.get_value(1, 1), Value::Number(60.0));
    assert!(s.undo());
    assert_eq!(s.get_value(1, 1), Value::Number(6.0), "Ctrl+Z em lote suja a cadeia toda");
    assert!(s.redo());
    assert_eq!(s.get_value(1, 1), Value::Number(60.0));
}

#[test]
fn desfazer_ordenacao_devolve_formula_que_virou_ref() {
    let mut s = Sheet::new("Plan1");
    for (r, v) in ["3", "1", "2"].iter().enumerate() {
        s.set_cell_raw(r as u32, 0, (*v).into(), None);
    }
    s.set_cell_raw(0, 3, "100".into(), None);
    s.set_cell_raw(2, 1, "=D1".into(), None);
    s.set_cell_raw(2, 2, "=SOMA(D1:D2)".into(), None);
    s.clear_history();
    s.permutar_linhas(0, 0, 2, &[2, 0, 1]);
    assert!(s.undo());
    assert_eq!(s.get_raw(2, 1), "=D1");
    assert_eq!(s.get_raw(2, 2), "=SOMA(D1:D2)");
    assert_eq!(s.get_raw(0, 0), "3");
    assert!(s.redo());
    assert!(s.undo());
    assert_eq!(s.get_raw(2, 1), "=D1");
}
