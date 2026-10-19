use super::*;

#[test]
fn guardar_calcular_e_ler_de_volta() {
    let mut t = FormulaTable::new();
    let mut pool = StringPool::new();
    assert!(t.set(1, 3, "=B2+C2", |_| true)); // D2
    assert_eq!(t.size(), 1);
    assert!(!t.is_clean(1, 3));
    t.store(1, 3, &Value::Number(10.0), &mut pool);
    assert!(t.is_clean(1, 3));
    assert_eq!(t.value_at(1, 3, &pool), Value::Number(10.0));
    assert_eq!(t.raw_at(1, 3), Some("=B2+C2".to_string()));
}

#[test]
fn duas_celulas_do_mesmo_padrao_compartilham_o_id() {
    let mut t = FormulaTable::new();
    assert!(t.set(1, 3, "=B2+C2", |_| true)); // D2
    assert!(t.set(2, 3, "=B3+C3", |_| true)); // D3
    assert_eq!(t.pattern_count(), 1);
    assert_eq!(t.size(), 2);
}

#[test]
fn formula_que_le_intervalo_nao_entra() {
    let mut t = FormulaTable::new();
    assert!(!t.set(0, 0, "=SOMA(A1:A9)", |_| true));
    assert_eq!(t.size(), 0);
}

#[test]
fn formula_para_outra_aba_entra_com_dependencias_por_padrao() {
    let mut t = FormulaTable::new();
    assert!(t.set(0, 5, "=Dados!A1+B1", |_| true));
    assert_eq!(t.size(), 1);
    assert!(t.tem_externas());
}

#[test]
fn check_reprovando_nao_deixa_entrar_e_padrao_fica_lembrado() {
    let mut t = FormulaTable::new();
    let mut chamadas = 0;
    assert!(!t.set(1, 3, "=B2+C2", |_| {
        chamadas += 1;
        false
    }));
    assert!(!t.set(2, 3, "=B3+C3", |_| {
        chamadas += 1;
        false
    }));
    // A segunda célula tem o mesmo padrão da primeira, já reprovado —
    // não deveria compilar/checar de novo.
    assert_eq!(chamadas, 1);
}

#[test]
fn invalidar_todas_sem_criar_novas_celulas() {
    let mut t=FormulaTable::new();
    let mut pool=StringPool::new();
    assert!(t.set(0,0,"=B1+1", |_|true));
    assert!(t.set(50,0,"=B51+1", |_|true));
    t.store(0,0,&Value::Number(5.0),&mut pool);
    t.store(50,0,&Value::Number(10.0),&mut pool);
    assert!(t.is_clean(0,0));
    assert!(t.is_clean(50,0));
    let antes=t.size();
    t.marcar_todas_sujas();
    assert!(!t.is_clean(0,0));
    assert!(!t.is_clean(50,0));
    assert_eq!(t.size(),antes);
    assert!(!t.has(10,0),"não deve preencher lacunas esparsas");
    t.marcar_todas_sujas(); // idempotente
    assert_eq!(t.size(),antes);
}

#[test]
fn mark_dirty_e_clear_cell() {
    let mut t = FormulaTable::new();
    let mut pool = StringPool::new();
    t.set(0, 0, "=A2", |_| true);
    t.store(0, 0, &Value::Number(1.0), &mut pool);
    assert!(t.is_clean(0, 0));
    assert!(t.mark_dirty(0, 0));
    assert!(!t.is_clean(0, 0));
    assert!(!t.mark_dirty(0, 0)); // já estava suja
    t.clear_cell(0, 0);
    assert!(!t.has(0, 0));
    assert_eq!(t.size(), 0);
}

#[test]
fn columns_in_use_lista_os_padroes_por_coluna() {
    let mut t = FormulaTable::new();
    t.set(1, 3, "=B2+C2", |_| true);
    t.set(2, 3, "=B3+C3", |_| true);
    let usos = t.columns_in_use();
    assert_eq!(usos.len(), 1);
    assert_eq!(usos[0].col, 3);
    assert_eq!(usos[0].usos.len(), 1);
    assert_eq!(usos[0].usos[0].refs.len(), 2); // B e C
}

#[test]
fn salvar_e_carregar_roda_ida_e_volta_com_numero_texto_e_erro() {
    let mut t = FormulaTable::new();
    let mut pool = StringPool::new();
    t.set(0, 0, "=A2", |_| true);
    t.set(1, 0, "=A3", |_| true);
    t.set(2, 0, "=A4", |_| true);
    t.store(0, 0, &Value::Number(42.0), &mut pool);
    t.store(1, 0, &Value::Text("olá".to_string()), &mut pool);
    t.store(2, 0, &Value::Error(CellError::new(ErrorCode::Div0)), &mut pool);

    let salva = t.save_column(0, 3, &pool).unwrap();
    assert_eq!(salva.dict, vec!["olá".to_string()]);
    assert_eq!(salva.erros.len(), 1);
    assert_eq!(salva.erros[0].code, "#DIV/0!");

    let mut t2 = FormulaTable::new();
    let mut pool2 = StringPool::new();
    t2.load_patterns(t.pattern_list());
    t2.load_column(0, &salva, 3, &mut pool2);

    assert_eq!(t2.value_at(0, 0, &pool2), Value::Number(42.0));
    assert_eq!(t2.value_at(1, 0, &pool2), Value::Text("olá".to_string()));
    assert_eq!(t2.value_at(2, 0, &pool2), Value::Error(CellError::new(ErrorCode::Div0)));
    assert_eq!(t2.size(), 3);
}

#[test]
fn for_each_in_range_so_visita_o_retangulo() {
    let mut t = FormulaTable::new();
    t.set(0, 0, "=A2", |_| true);
    t.set(5, 0, "=A7", |_| true);
    t.set(5, 1, "=B7", |_| true);
    let mut vistos = Vec::new();
    t.for_each_in_range(0, 5, 0, 0, |r, c| vistos.push((r, c)));
    vistos.sort();
    assert_eq!(vistos, vec![(0, 0), (5, 0)]);
}

#[test]
fn entries_reconstroi_o_texto_de_cada_formula() {
    let mut t = FormulaTable::new();
    t.set(1, 3, "=B2+C2", |_| true);
    t.set(2, 3, "=B3+C3", |_| true);
    let mut es: Vec<String> = t.entries().into_iter().map(|e| e.raw).collect();
    es.sort();
    assert_eq!(es, vec!["=B2+C2".to_string(), "=B3+C3".to_string()]);
}

#[test]
fn bounds_vazio_e_preenchido() {
    let t = FormulaTable::new();
    let b = t.bounds();
    assert_eq!(b.max_row, -1);
    assert_eq!(b.max_col, -1);

    let mut t2 = FormulaTable::new();
    t2.set(9, 4, "=A1", |_| true);
    let b2 = t2.bounds();
    assert_eq!(b2.max_row, 9);
    assert_eq!(b2.max_col, 4);
}

#[test]
fn shift_columns_e_shift_rows_em_formula_table() {
    let mut t = FormulaTable::new();
    t.set(1, 3, "=B2+C2", |_| true); // D2
    t.set(2, 3, "=B3+C3", |_| true); // D3
    assert_eq!(t.size(), 2);

    // Shift rows: insert 1 row at row 1
    t.shift_rows(1, 1, false);
    assert!(!t.has(1, 3));
    assert!(t.has(2, 3)); // D2 -> D3
    assert!(t.has(3, 3)); // D3 -> D4
    assert_eq!(t.raw_at(2, 3), Some("=B3+C3".to_string()));

    // Shift columns: insert 1 column at col 1 (B)
    t.shift_columns(1, 1, false);
    assert!(!t.has(2, 3));
    assert!(t.has(2, 4)); // D3 -> E3
}

#[test]
fn salvar_guarda_o_erro_repetido_uma_vez_e_volta_igual() {
    use crate::celulas::values::{CellError, ErrorCode};
    let mut pool = StringPool::default();
    let mut t = FormulaTable::new();
    let refe = Value::Error(CellError::new(ErrorCode::Ref));
    let div = Value::Error(CellError::new(ErrorCode::Div0));
    for r in 0..4 {
        assert!(t.set(r, 1, &format!("=A{}", r + 1), |_| true));
    }
    t.store(0, 1, &refe, &mut pool);
    t.store(1, 1, &div, &mut pool);
    t.store(2, 1, &refe, &mut pool);
    t.store(3, 1, &refe, &mut pool);
    let salva = t.save_column(1, 4, &pool).unwrap();
    assert_eq!(salva.erros.len(), 2, "três #REF! viram uma entrada");
    let mut lida = FormulaTable::new();
    lida.load_patterns(&t.patterns);
    lida.load_column(1, &salva, 4, &mut pool);
    for (r, v) in [(0, &refe), (1, &div), (2, &refe), (3, &refe)] {
        assert_eq!(&lida.value_at(r, 1, &pool), v, "linha {r}");
    }
}
