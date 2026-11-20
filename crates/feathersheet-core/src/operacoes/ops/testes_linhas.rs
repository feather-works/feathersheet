//! Ordenar, dividir, aparar, bordas e casas decimais — os casos do
//! `engine.test.ts`, mais o Ctrl+Z, que as versões antigas não tinham.
use super::*;
use crate::celulas::estilo_parcial::formato;

pub(super) fn planilha(linhas: &[&[&str]]) -> Sheet {
    let mut s = Sheet::new("Planilha1");
    for (r, linha) in linhas.iter().enumerate() {
        for (c, texto) in linha.iter().enumerate() {
            if !texto.is_empty() {
                s.set_cell(r as u32, c as u32, texto);
            }
        }
    }
    s.clear_history();
    s
}

pub(super) fn coluna(s: &Sheet, c: u32, ate: u32) -> Vec<String> {
    (0..=ate).map(|r| s.get_raw(r, c)).collect()
}

pub(super) fn negrito() -> EstiloParcial {
    let mut e = EstiloParcial::new();
    e.insert("bold".into(), Campo::Logico(true));
    e
}

#[test]
pub(super) fn classificar_por_varias_chaves() {
    let mut s = planilha(&[&["Maçã", "5"], &["Banana", "3"], &["Maçã", "2"], &["Banana", "8"]]);
    let chaves = [SortKey { col: 0, ascending: true }, SortKey { col: 1, ascending: false }];
    sort_range(&mut s, Rect::new(0, 0, 3, 1), &chaves, false);
    assert_eq!(coluna(&s, 0, 3), ["Banana", "Banana", "Maçã", "Maçã"]);
    assert_eq!(coluna(&s, 1, 3), ["8", "3", "5", "2"]);
}

#[test]
pub(super) fn ordenar_leva_estilo_e_formula_e_desfaz_de_uma_vez() {
    let mut s = planilha(&[&["Nome", "Dobro"], &["c", "=B2*0"], &["a", "=A3"], &["b", ""]]);
    s.set_style(&Rect::new(2, 0, 2, 0), &negrito());
    s.set_style(&Rect::new(5, 0, 5, 0), &negrito());
    s.clear_history();
    let chaves = [SortKey { col: 0, ascending: true }];
    sort_range(&mut s, Rect::new(0, 0, 3, 1), &chaves, true);
    assert_eq!(coluna(&s, 0, 3), ["Nome", "a", "b", "c"]);
    // A fórmula anda com a linha: estava na 3 e lia A3, foi para a 2.
    assert_eq!(s.get_raw(1, 1), "=A2");
    assert_eq!(s.get_raw(3, 1), "=B4*0");
    assert!(s.estilo_mesclado(1, 0).is_some_and(|e| e.contains_key("bold")), "o negrito de \"a\" foi junto");
    assert!(s.estilo_mesclado(2, 0).is_none_or(|e| !e.contains_key("bold")));
    assert!(s.estilo_mesclado(5, 0).is_some_and(|e| e.contains_key("bold")), "fora da área o estilo fica");
    s.undo();
    assert_eq!(coluna(&s, 0, 3), ["Nome", "c", "a", "b"]);
    assert_eq!(s.get_raw(2, 1), "=A3");
    assert!(s.estilo_mesclado(2, 0).is_some_and(|e| e.contains_key("bold")), "o Ctrl+Z devolve o negrito");
    assert!(s.estilo_mesclado(1, 0).is_none_or(|e| !e.contains_key("bold")));
}

#[test]
pub(super) fn ja_em_ordem_nao_mexe() {
    let mut s = planilha(&[&["a"], &["b"]]);
    sort_range(&mut s, Rect::new(0, 0, 1, 0), &[SortKey { col: 0, ascending: true }], false);
    assert!(!s.can_undo());
}

#[test]
pub(super) fn repetidas_entram_no_desfazer() {
    let mut s = planilha(&[&["a", "1"], &["a", "1"], &["b", "2"]]);
    assert_eq!(remove_duplicates(&mut s, Rect::new(0, 0, 2, 1), false, None), 1);
    assert_eq!(coluna(&s, 0, 2), ["a", "b", ""]);
    s.undo();
    assert_eq!(coluna(&s, 0, 2), ["a", "a", "b"]);
}

#[test]
pub(super) fn dividir_usa_o_valor_e_conta_as_linhas_com_texto() {
    let mut s = planilha(&[&["a; b"], &["=\"x;y\""], &["só"], &[""]]);
    assert_eq!(split_text_to_columns(&mut s, Rect::new(0, 0, 3, 0), ";"), 3);
    assert_eq!((s.get_raw(0, 0), s.get_raw(0, 1)), ("a".into(), "b".into()));
    // A fórmula é dividida pelo resultado, e a célula vira o pedaço.
    assert_eq!((s.get_raw(1, 0), s.get_raw(1, 1)), ("x".into(), "y".into()));
    assert_eq!(s.get_raw(2, 0), "só");
    s.undo();
    assert_eq!(s.get_raw(1, 0), "=\"x;y\"");
    assert_eq!(s.get_raw(0, 1), "");
}

#[test]
pub(super) fn dividir_sem_separador_achado_nao_mexe() {
    let mut s = planilha(&[&["a b"]]);
    assert_eq!(split_text_to_columns(&mut s, Rect::new(0, 0, 0, 0), ";"), 0);
    assert_eq!(split_text_to_columns(&mut s, Rect::new(0, 0, 0, 0), ""), 0);
    assert!(!s.can_undo());
}

#[test]
pub(super) fn dividir_com_tipo_forca_texto_em_numero_que_pareceria_numero() {
    let mut s = planilha(&[&["0012;a"]]);
    let opcoes = OpcoesDivisao { delimitadores: vec![";".into()], ..Default::default() };
    let linhas = planejar_divisao(Rect::new(0, 0, 0, 0), &opcoes, s.max_row_ts(), |r, c| s.get_value(r, c)).unwrap();
    let tipos = [TipoColunaDivisao::Texto, TipoColunaDivisao::Geral];
    assert_eq!(aplicar_divisao(&mut s, 0, &linhas, Some(&tipos)), 1);
    assert_eq!(s.get_value(0, 0), Value::Text("0012".into()));
}

#[test]
pub(super) fn dividir_com_tipo_data_le_a_ordem_escolhida() {
    // "03/04/2005" seria 3 de abril pela ordem Dia/Mês/Ano; pedindo
    // Mês/Dia/Ano vira 4 de março — é exatamente a ambiguidade que o
    // assistente resolve perguntando, em vez de adivinhar.
    let mut s = planilha(&[&["a"]]);
    let linhas = vec![(0u32, vec!["03/04/2005".to_string()])];
    let tipos = [TipoColunaDivisao::Data(FormatoDeData::MesDiaAno)];
    assert_eq!(aplicar_divisao(&mut s, 0, &linhas, Some(&tipos)), 1);
    assert_eq!(s.get_value(0, 0), Value::Number(date_to_serial(2005, 3, 4)));
    assert_eq!(s.estilo_mesclado(0, 0).and_then(|e| e.get("format").cloned()), Some(Campo::Texto("date".into())));
}

#[test]
pub(super) fn dividir_com_tipo_porcentagem_e_pular() {
    let mut s = planilha(&[&["x", "a"]]);
    let linhas = vec![(0u32, vec!["15,5%".to_string(), "descartado".to_string()])];
    let tipos = [TipoColunaDivisao::Porcentagem, TipoColunaDivisao::Pular];
    assert_eq!(aplicar_divisao(&mut s, 0, &linhas, Some(&tipos)), 1);
    assert_eq!(s.get_value(0, 0), Value::Number(0.155));
    assert_eq!(s.get_raw(0, 1), "a", "a coluna Pular não escreve nem apaga");
}

#[test]
pub(super) fn dividir_com_varios_delimitadores_ao_mesmo_tempo() {
    let opcoes = OpcoesDivisao { delimitadores: vec![",".into(), ";".into()], ..Default::default() };
    assert_eq!(dividir_com_opcoes("a,b;c", &opcoes), vec!["a", "b", "c"]);
}

#[test]
pub(super) fn dividir_sem_consecutivos_como_um_conta_o_campo_vazio() {
    let opcoes = OpcoesDivisao { delimitadores: vec![",".into()], ..Default::default() };
    assert_eq!(dividir_com_opcoes("a,,b", &opcoes), vec!["a", "", "b"]);
}

#[test]
pub(super) fn contar_sem_alocar_equivale_dividir_em_todas_as_variantes() {
    for opcoes in [
        OpcoesDivisao {
            delimitadores: vec![",".into()],
            consecutivos_como_um: true,
            qualificador_texto: Some('"'),
        },
        OpcoesDivisao {
            delimitadores: vec!["||".into()],
            consecutivos_como_um: false,
            qualificador_texto: None,
        },
        OpcoesDivisao {
            delimitadores: vec![",".into(), ";".into()],
            consecutivos_como_um: true,
            qualificador_texto: Some('"'),
        },
    ] {
        for texto in ["", "A", ",,", "Ana,,26,Vitória",
            r#""Ana, Maria",26,ES"#, "a||b||", "São;Vitória"] {
            assert_eq!(contar_colunas_divisao(texto,&opcoes),
                dividir_com_opcoes(texto,&opcoes).len(), "{texto:?}");
        }
    }
}

#[test]
pub(super) fn caminho_rapido_com_qualificador_padrao_e_consecutivos_ativos() {
    let opcoes = OpcoesDivisao {
        delimitadores: vec![";".into()],
        consecutivos_como_um: true,
        qualificador_texto: Some('"'),
    };
    assert_eq!(dividir_com_opcoes("Ana;;25;;;Vitória", &opcoes),
        ["Ana", "25", "Vitória"]);
    assert_eq!(dividir_com_opcoes(";;", &opcoes), [""]);
    assert_eq!(dividir_com_opcoes("A;", &opcoes), ["A"]);
    assert_eq!(dividir_com_opcoes(r#""Ana; Maria";;25"#, &opcoes),
        ["Ana; Maria", "25"]);
}

#[test]
pub(super) fn dividir_com_consecutivos_como_um_ignora_o_campo_vazio() {
    let opcoes = OpcoesDivisao { delimitadores: vec![",".into()], consecutivos_como_um: true, ..Default::default() };
    assert_eq!(dividir_com_opcoes("a,,,b", &opcoes), vec!["a", "b"]);
    assert_eq!(dividir_com_opcoes(",,", &opcoes), vec![""], "tudo vazio não pode sumir também");
}

#[test]
pub(super) fn dividir_com_qualificador_de_texto_protege_o_delimitador_de_dentro() {
    let opcoes = OpcoesDivisao {
        delimitadores: vec![",".into()],
        qualificador_texto: Some('"'),
        ..Default::default()
    };
    assert_eq!(dividir_com_opcoes(r#""a,b",c"#, &opcoes), vec!["a,b", "c"]);
}

#[test]
pub(super) fn dividir_com_qualificador_duplicado_vira_aspas_literal() {
    let opcoes = OpcoesDivisao { delimitadores: vec![",".into()], qualificador_texto: Some('"'), ..Default::default() };
    assert_eq!(dividir_com_opcoes(r#""ele disse ""oi""",b"#, &opcoes), vec![r#"ele disse "oi""#, "b"]);
}

#[test]
pub(super) fn separar_rapido_mantem_unicode_campos_vazios_e_token_de_varios_caracteres() {
    for (texto, separador, esperado) in [
        ("São;Vitória;;", ";", vec!["São", "Vitória", "", ""]),
        (";início;fim", ";", vec!["", "início", "fim"]),
        ("a||b||c", "||", vec!["a", "b", "c"]),
        ("a🐦b🐦", "🐦", vec!["a", "b", ""]),
        ("sem separador", ";", vec!["sem separador"]),
    ] {
        let opcoes = OpcoesDivisao {
            delimitadores: vec![separador.to_owned()],
            ..Default::default()
        };
        assert_eq!(dividir_com_opcoes(texto, &opcoes), esperado);
    }
}

#[test]
pub(super) fn delimitador_com_qualificador_ativo_e_sem_aspas_usa_resultado_rapido() {
    let opcoes = OpcoesDivisao {
        delimitadores: vec![";".into()],
        qualificador_texto: Some('"'),
        ..Default::default()
    };
    assert_eq!(dividir_com_opcoes("João;25;Vitória", &opcoes),
        ["João", "25", "Vitória"]);
    assert_eq!(dividir_com_opcoes(r#""João; Filho";25"#, &opcoes),
        ["João; Filho", "25"]);
}

#[test]
pub(super) fn separar_com_aspas_e_multiplos_delimitadores_nao_cai_no_caminho_simples() {
    let opcoes = OpcoesDivisao {
        delimitadores: vec![";".into()],
        qualificador_texto: Some('"'),
        ..Default::default()
    };
    assert_eq!(dividir_com_opcoes(r#""São;Paulo";SP"#, &opcoes), vec!["São;Paulo", "SP"]);
    let opcoes = OpcoesDivisao {
        delimitadores: vec![",".into(), ";".into()],
        ..Default::default()
    };
    assert_eq!(dividir_com_opcoes("A,B;C", &opcoes), vec!["A", "B", "C"]);
}

#[test]
pub(super) fn dividir_com_delimitador_de_mais_de_um_caractere() {
    // "Outros:" pode ser um texto, não só um caractere.
    let opcoes = OpcoesDivisao { delimitadores: vec!["::".into()], ..Default::default() };
    assert_eq!(dividir_com_opcoes("a::b::c", &opcoes), vec!["a", "b", "c"]);
}

#[test]
pub(super) fn aparar_espacos_preserva_formula_e_estilo() {
    let mut s = planilha(&[&["  a   b  "], &["=CONCAT(\"  a   b  \")"], &["  texto   com    muito   espaço  "]]);
    s.set_style(&Rect::new(0, 0, 0, 0), &negrito());
    s.clear_history();
    assert_eq!(trim_spaces(&mut s, Rect::new(0, 0, 2, 0)), 2);
    assert_eq!(coluna(&s, 0, 2), ["a b", "=CONCAT(\"  a   b  \")", "texto com muito espaço"]);
    assert!(s.estilo_mesclado(0, 0).is_some_and(|e| e.contains_key("bold")), "o negrito ficou");
    s.undo();
    assert_eq!(s.get_raw(0, 0), "  a   b  ");
}

#[test]
pub(super) fn aparar_segue_o_espaco_do_javascript() {
    // U+FEFF é espaço para o `trim()` do JavaScript; U+0085 não é.
    let mut s = planilha(&[&["\u{feff}a\u{85}"]]);
    trim_spaces(&mut s, Rect::new(0, 0, 0, 0));
    assert_eq!(s.get_raw(0, 0), "a\u{85}");
}

pub(super) fn bordas(s: &Sheet, r: u32, c: u32) -> Vec<String> {
    match s.estilo_mesclado(r, c).and_then(|e| e.get("borders").cloned()) {
        Some(Campo::Objeto(b)) => b.keys().cloned().collect(),
        _ => vec![],
    }
}

#[test]
pub(super) fn bordas_externas_marcam_so_o_lado_de_fora() {
    let mut s = planilha(&[&["1", "2"], &["3", "4"]]);
    apply_borders(&mut s, Rect::new(0, 0, 1, 1), "outer", None);
    assert_eq!(bordas(&s, 0, 0), ["left", "top"]);
    assert_eq!(bordas(&s, 1, 1), ["bottom", "right"]);
    apply_borders(&mut s, Rect::new(0, 0, 1, 1), "none", None);
    assert!(bordas(&s, 0, 0).is_empty());
    s.undo();
    assert_eq!(bordas(&s, 0, 0), ["left", "top"]);
}

#[test]
pub(super) fn casas_decimais_partem_da_primeira_celula() {
    // Sem formato, o `decimalsOf` parte de zero casas.
    let mut s = planilha(&[&["1,5"]]);
    step_decimals(&mut s, Rect::new(0, 0, 0, 0), 1);
    let e = s.estilo_mesclado(0, 0).unwrap();
    assert_eq!(e.get("decimals"), Some(&Campo::Numero(1.0)));
    assert_eq!(formato(Some(&e)).as_deref(), Some("number"));
    // Com moeda, de duas, e o formato fica.
    let mut s = planilha(&[&["1,5"]]);
    let mut moeda = EstiloParcial::new();
    moeda.insert("format".into(), Campo::Texto("currency".into()));
    s.set_style(&Rect::new(0, 0, 0, 0), &moeda);
    step_decimals(&mut s, Rect::new(0, 0, 0, 0), 1);
    let e = s.estilo_mesclado(0, 0).unwrap();
    assert_eq!(e.get("decimals"), Some(&Campo::Numero(3.0)));
    assert_eq!(formato(Some(&e)).as_deref(), Some("currency"));
    // E não desce de zero.
    let mut s = planilha(&[&["1,5"]]);
    step_decimals(&mut s, Rect::new(0, 0, 0, 0), -5);
    assert_eq!(s.estilo_mesclado(0, 0).unwrap().get("decimals"), Some(&Campo::Numero(0.0)));
}
