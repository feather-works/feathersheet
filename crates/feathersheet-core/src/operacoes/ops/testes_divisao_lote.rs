use super::*;
use std::sync::Arc;

const LINHAS: [&str; 6] = [
    "nome,valor,data,taxa,codigo",
    "Ana,1.5,29/08/2019,10%,007",
    "Bia,2022,2019-08-29,,=1+1",
    "Caio,\"3,5\",abc,VERDADEIRO,R$ 10",
    "",
    "Duda, 7 ,x",
];

pub(super) fn fonte() -> Sheet {
    let mut s = Sheet::new("A");
    for (r, l) in LINHAS.iter().enumerate() {
        if !l.is_empty() { s.set_cell(r as u32, 0, l); }
    }
    s.set_cell(1, 6, "fica");
    s.clear_history();
    s
}

#[test]
fn estimativa_de_textos_novos_conta_distintos_e_ignora_numero() {
    // Coluna 0: id único por linha; 1: número; 2: sempre "Vitória".
    let amostra: Vec<String> = (0..5_000u32).step_by(10).map(|r| format!("id{r};{r};Vitória")).collect();
    let p = PlanoDivisao { r1: 4_999, colunas: 3,
        opcoes: OpcoesDivisao { delimitadores: vec![";".into()], consecutivos_como_um: false, qualificador_texto: None },
        ..plano() };
    let n = estimar_textos_novos(&p, &amostra, 5_000);
    // ~5.000 ids + "Vitória" (categoria: 1 distinto, dobrado) — o número não conta.
    assert!((4_900..=5_200).contains(&n), "{n}");
}

pub(super) fn plano() -> PlanoDivisao {
    PlanoDivisao {
        col_origem: 0, r0: 0, r1: LINHAS.len() as u32 - 1, dst_row: 0, dst_col: 0,
        colunas: 5, largura: None,
        opcoes: OpcoesDivisao { delimitadores: vec![",".into()], consecutivos_como_um: false,
            qualificador_texto: Some('"') },
        tipos: Vec::new(), formulas: HashMap::new(),
    }
}

pub(super) fn foto(s: &Sheet) -> Vec<(String, Option<usize>, String)> {
    let mut v = Vec::new();
    for r in 0..LINHAS.len() as u32 {
        for c in 0..7 {
            v.push((s.get_raw(r, c), s.get_style_idx(r, c).map(|i| i), format!("{:?}", s.get_value(r, c))));
        }
    }
    v
}

#[test]
pub(super) fn lote_grava_igual_ao_caminho_antigo_e_desfaz_e_refaz() {
    // Caminho antigo: aplicar_divisao célula por célula.
    let mut antigo = fonte();
    let p = plano();
    let linhas: Vec<(u32, Vec<String>)> = (0..LINHAS.len() as u32)
        .map(|r| (r, p.texto_origem(&antigo, r)))
        .filter(|(_, t)| !t.is_empty())
        .map(|(r, t)| (r, dividir_com_opcoes(&t, &p.opcoes))).collect();
    antigo.abrir_transacao();
    aplicar_divisao(&mut antigo, 0, &linhas, None);
    antigo.fechar_transacao();

    let mut novo = fonte();
    let antes = foto(&novo);
    novo.abrir_transacao();
    let plano = Arc::new(p);
    let mut g = iniciar_divisao(&mut novo, Arc::clone(&plano));
    let textos = (0..LINHAS.len() as u32).map(|r| (r, plano.texto_origem(&novo, r))).collect();
    let preparadas = plano.preparar_linhas(textos);
    novo.gravar_divisao(&mut g, 0, &preparadas);
    novo.fechar_transacao();

    let depois = foto(&novo);
    // Índices de estilo podem diferir na ordem de registro: comparar o formato.
    let fmt = |s: &Sheet| -> Vec<Option<String>> {
        (0..LINHAS.len() as u32).flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| s.get_style(r, c).map(|st| format!("{:?}", st))).collect()
    };
    let sem_idx = |v: &[(String, Option<usize>, String)]| -> Vec<(String, String)> {
        v.iter().map(|(a, _, b)| (a.clone(), b.clone())).collect()
    };
    assert_eq!(sem_idx(&depois), sem_idx(&foto(&antigo)));
    assert_eq!(fmt(&novo), fmt(&antigo));
    assert_eq!(novo.get_raw(1, 6), "fica");

    assert!(novo.undo());
    assert_eq!(foto(&novo), antes, "desfazer volta a coluna original");
    assert!(novo.redo());
    assert_eq!(sem_idx(&foto(&novo)), sem_idx(&depois), "refazer refaz igual");
    assert!(novo.undo());
    assert_eq!(foto(&novo), antes);
}

#[test]
pub(super) fn substituir_dados_existentes_volta_no_desfazer() {
    let mut s = fonte();
    s.set_cell(1, 2, "antigo");
    s.set_cell(2, 3, "=A1");
    s.clear_history();
    let antes = foto(&s);
    s.abrir_transacao();
    let plano = Arc::new(plano());
    let mut g = iniciar_divisao(&mut s, Arc::clone(&plano));
    let textos = (0..LINHAS.len() as u32).map(|r| (r, plano.texto_origem(&s, r))).collect();
    let linhas = plano.preparar_linhas(textos);
    s.gravar_divisao(&mut g, 0, &linhas);
    s.fechar_transacao();
    assert_eq!(s.get_raw(1, 2), "29/08/2019");
    assert!(s.undo());
    assert_eq!(foto(&s), antes);
}

