//! Testes da referência viva entre arquivos, sem socket de verdade: o canal
//! fica desligado nos testes (`canal::disponivel`), e o que chegaria por ele
//! é entregue direto ao `receber_vinculo`.
use super::tela::Situacao;
use super::*;

/// Destino com duas fórmulas que leem Vendas.fshz, uma que depende delas e
/// uma segunda aba que não lê nada de fora.
fn destino() -> App {
    let mut app = App::default();
    app.caminho = Some(PathBuf::from("/tmp/fs-vinculos-teste/Destino.fshz"));
    app.workbook.escrever_celula(0, 0, 0, "=[Vendas.fshz]Dados!A1*2".into());
    app.workbook.escrever_celula(0, 1, 0, "=SOMA([Vendas.fshz]Dados!B1:B3)".into());
    app.workbook.escrever_celula(0, 2, 0, "=A1+1".into());
    app.workbook.criar_aba("Outra");
    app.workbook.escrever_celula(1, 0, 0, "=10*2".into());
    let dados = motor_vinculo::aba_de_valores("Dados", &[
        (0, 0, Value::Number(10.0)),
        (0, 1, Value::Number(1.0)),
        (1, 1, Value::Number(2.0)),
    ]);
    app.workbook.instalar_externo_com("Vendas.fshz", vec![dados], Some("/tmp/fs-vinculos-teste/Vendas.fshz".into()), Some(1_760_630_000));
    for (aba, linha) in [(0, 0), (0, 1), (0, 2), (1, 0)] {
        let _ = app.workbook.valor(aba, linha, 0);
    }
    app
}

/// A origem que está aberta em outra janela (conexão de mentira 7).
fn com_origem_conectada(app: &mut App) {
    let _ = app.pulso_vinculos();
    assert_eq!(app.vinculos.origens.len(), 1, "{:?}", app.vinculos.origens);
    let o = &mut app.vinculos.origens[0];
    assert_eq!(o.nomes, vec!["Vendas.fshz".to_string()]);
    assert_eq!(o.caminho.as_deref(), Some(Path::new("/tmp/fs-vinculos-teste/Vendas.fshz")));
    o.conexao = Some(7);
}

#[test]
fn mudanca_que_chega_recalcula_so_as_dependentes() {
    let mut app = destino();
    com_origem_conectada(&mut app);
    let (versao_1, versao_2) = (app.workbook.versao(0), app.workbook.versao(1));
    // A origem mandou A1 (lido por A1*2 e, por cascata, por =A1+1).
    app.receber_vinculo(Chegada::Valores { conexao: 7, aba: "Dados".into(), celulas: vec![(0, 0, Value::Number(50.0))] });
    assert_ne!(app.workbook.versao(0), versao_1, "a aba que lê de fora mudou");
    assert_eq!(app.workbook.versao(1), versao_2, "a outra aba não recalcula");
    assert!(app.workbook.calculo_pendente(0, 0, 0), "A1*2 lê a célula mudada");
    assert!(app.workbook.calculo_pendente(0, 2, 0), "=A1+1 depende dela");
    assert!(!app.workbook.calculo_pendente(0, 1, 0), "a SOMA não lê A1");
    assert_eq!(app.workbook.valor(0, 0, 0), Value::Number(100.0));
    assert_eq!(app.workbook.valor(0, 2, 0), Value::Number(101.0));
    // Uma conexão que não é desta origem é ignorada.
    app.receber_vinculo(Chegada::Valores { conexao: 99, aba: "Dados".into(), celulas: vec![(0, 0, Value::Number(1.0))] });
    assert_eq!(app.workbook.valor(0, 0, 0), Value::Number(100.0));
}

#[test]
fn retrato_deixa_ao_vivo_e_origem_fechada_volta_a_faixa() {
    let mut app = destino();
    com_origem_conectada(&mut app);
    app.receber_vinculo(Chegada::Retrato {
        conexao: 7,
        abas: vec!["Dados".into(), "Resumo".into()],
        celulas: vec![("Dados".into(), vec![(0, 0, Value::Number(3.0)), (0, 1, Value::Number(4.0))])],
    });
    assert_eq!(app.workbook.valor(0, 0, 0), Value::Number(6.0));
    assert_eq!(app.workbook.valor(0, 1, 0), Value::Number(4.0));
    let o = app.vinculos.origens[0].clone();
    assert_eq!(app.situacao_origem(&o), Situacao::AoVivo);
    assert!(app.aviso_vinculos().is_none(), "tudo ao vivo: sem faixa");
    app.receber_vinculo(Chegada::OrigemFechou(7));
    let o = app.vinculos.origens[0].clone();
    assert!(matches!(app.situacao_origem(&o), Situacao::Guardado(Some(_))));
    let (texto, alerta) = app.aviso_vinculos().expect("faixa");
    assert!(texto.contains("[Vendas.fshz]"), "{texto}");
    assert!(!alerta);
    assert_eq!(app.workbook.valor(0, 0, 0), Value::Number(6.0), "fica o último valor");
}

#[test]
fn faixa_mostra_a_data_dos_valores_guardados() {
    let mut app = destino();
    let _ = app.pulso_vinculos();
    let (texto, _) = app.aviso_vinculos().expect("origem fechada");
    assert!(texto.contains(&data_hora_local(1_760_630_000)), "{texto}");
    let _ = app.update(Msg::Vinculos(Evento::FaixaFechar));
    assert!(app.aviso_vinculos().is_none());
    for escuro in [false, true] {
        feather_ui::tema::definir_tema(escuro, 0);
        let _: Element<'_, Msg> = app.view_faixa_vinculos();
        app.vinculos.editar_aberto = true;
        let _: Element<'_, Msg> = app.sobrepor_editar_links(Space::new(0.0, 0).into());
    }
    feather_ui::tema::definir_tema(false, 0);
}

#[test]
fn origem_publica_so_o_que_mudou() {
    let mut origem = Workbook::new();
    origem.renomear_aba(0, "Dados");
    origem.escrever_celula(0, 0, 0, "1".into());
    origem.escrever_celula(0, 1, 0, "=A1*10".into());
    origem.escrever_celula(0, 0, 5, "fora da área".into());
    let areas = vec![AreaPedida { aba: "Dados".into(), area: Rect::new(0, 0, 9, 0) }];
    let (retrato, assinante) = retrato_para(&origem, &areas);
    assert_eq!(retrato["m"], "retrato");
    assert_eq!(retrato["celulas"][0]["c"].as_array().map(Vec::len), Some(2));
    let mut assinantes = HashMap::from([(1u64, assinante)]);
    assert!(publicar(&origem, &mut assinantes).is_empty(), "nada mudou");
    // Mudar fora da área assinada não manda nada.
    origem.escrever_celula(0, 0, 5, "outro".into());
    assert!(publicar(&origem, &mut assinantes).is_empty());
    // Mudar A1: vai A1 e A2 (recalculada), e só elas.
    origem.escrever_celula(0, 0, 0, "2".into());
    let saidas = publicar(&origem, &mut assinantes);
    assert_eq!(saidas.len(), 1);
    let (id, msg) = &saidas[0];
    assert_eq!(*id, 1);
    assert_eq!(msg["m"], "valores");
    assert_eq!(msg["c"], serde_json::json!([[0, 0, 2.0], [1, 0, 20.0]]));
    // Apagar A1: vai vazia.
    origem.escrever_celula(0, 0, 0, String::new());
    let saidas = publicar(&origem, &mut assinantes);
    assert_eq!(saidas[0].1["c"][0], serde_json::json!([0, 0, null]));
}

#[test]
fn caminho_relativo_resolve_pela_pasta_do_destino() {
    let app = destino();
    assert_eq!(app.resolver_origem("/abs/Vendas.fshz"), Some(PathBuf::from("/abs/Vendas.fshz")));
    // Não existe no disco: fica o relativo à pasta do destino.
    assert_eq!(app.resolver_origem("Outro.fshz"), Some(PathBuf::from("/tmp/fs-vinculos-teste/Outro.fshz")));
    let sem_caminho = App::default();
    assert_eq!(sem_caminho.resolver_origem("Outro.fshz"), None);
}

#[test]
fn trocar_origem_reescreve_as_formulas() {
    let mut app = destino();
    com_origem_conectada(&mut app);
    let chave = app.vinculos.origens[0].chave.clone();
    let _ = app.update(Msg::Vinculos(Evento::OrigemEscolhida(chave, Some(PathBuf::from("/tmp/fs-vinculos-teste/Vendas2026.fshz")))));
    let aba = &app.workbook.aba(0).unwrap().sheet;
    assert_eq!(aba.get_raw(0, 0), "=[Vendas2026.fshz]Dados!A1*2", "mesma pasta: só o nome");
    assert_eq!(aba.get_raw(1, 0), "=SOMA([Vendas2026.fshz]Dados!B1:B3)");
    assert!(app.alterado);
    let _ = app.update(Msg::Vinculos(Evento::OrigemEscolhida(
        app.vinculos.origens[0].chave.clone(),
        Some(PathBuf::from("/srv/dados/Vendas.fshz")),
    )));
    assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "='/srv/dados/[Vendas.fshz]Dados'!A1*2", "outra pasta: caminho inteiro");
}

#[test]
fn data_curta_tem_dia_mes_hora_e_minuto() {
    let texto = data_hora_local(1_760_630_000);
    assert!(texto.len() >= 11, "{texto}");
    assert_eq!(&texto[2..3], "/");
    assert_eq!(&texto[5..6], " ");
}

#[test]
fn referencia_clicada_noutra_janela_sai_como_no_excel() {
    use super::texto_referencia_externa as t;
    let meu = Path::new("/x/Resumo.fshz");
    assert_eq!(t(Some(meu), Path::new("/x/Vendas.fshz"), "Vendas.fshz", "Dados", "A1"), "[Vendas.fshz]Dados!A1");
    assert_eq!(t(Some(meu), Path::new("/x/Vendas 2.fshz"), "Vendas 2.fshz", "Dados", "A1:B3"), "'[Vendas 2.fshz]Dados'!A1:B3");
    assert_eq!(t(Some(meu), Path::new("/y/Vendas.fshz"), "Vendas.fshz", "Minha aba", "C2"), "'/y/[Vendas.fshz]Minha aba'!C2");
    assert_eq!(t(None, Path::new("/y/V.fshz"), "V.fshz", "D'a", "A1"), "'/y/[V.fshz]D''a'!A1");
}

#[test]
fn mensagens_de_apontar_vao_e_voltam() {
    let m = canal::mensagem_apontar("/x/Vendas.fshz", "Vendas.fshz", "Dados", "A1:B2");
    match canal::interpretar_do_assinante(1, &m) {
        Some(Chegada::Apontar { caminho, arquivo, aba, referencia, .. }) =>
            assert_eq!((caminho.as_str(), arquivo.as_str(), aba.as_str(), referencia.as_str()), ("/x/Vendas.fshz", "Vendas.fshz", "Dados", "A1:B2")),
        outra => panic!("{outra:?}"),
    }
    assert!(matches!(canal::interpretar_do_assinante(1, &canal::mensagem_apontar_fim(true)), Some(Chegada::ApontarFim { confirmar: true })));
}

#[test]
fn referencia_pronta_troca_a_apontada_antes() {
    let mut app = App::default();
    let _ = app.update(Msg::Digitar("=".into()));
    assert!(app.editor_formula.ativo, "editando a fórmula");
    assert!(app.editor_formula.referencia_pronta(&mut app.edicao, "[Vendas.fshz]Dados!A1"));
    assert!(app.editor_formula.referencia_pronta(&mut app.edicao, "[Vendas.fshz]Dados!B2"));
    assert_eq!(app.edicao, "=[Vendas.fshz]Dados!B2");
}
