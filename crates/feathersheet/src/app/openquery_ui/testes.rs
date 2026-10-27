//! Testes do editor nativo OpenQuery — separados para manter o modulo UI pequeno.
use super::*;
    #[test]
    fn ler_intervalo_com_cabecalho_sem_alterar_dados() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "Cidade".into());
        wb.escrever_celula(0, 0, 1, "MWh".into());
        wb.escrever_celula(0, 1, 0, "Vitória".into());
        wb.escrever_celula(0, 1, 1, "42".into());
        let tabela = ler_tabela(&wb, 0, Rect::new(0, 0, 1, 1)).unwrap();
        assert_eq!(tabela.cabecalho, ["Cidade", "MWh"]);
        assert_eq!(tabela.linhas[0], [Value::Text("Vitória".into()), Value::Number(42.0)]);
    }

    #[test]
    fn selecao_enorme_recusada_sem_materializar() {
        // A leitura corta na área usada: os dados precisam ocupar a seleção.
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "Cabeçalho".into());
        wb.escrever_celula(0, 1_000_000, 12, "fim".into());
        let erro = ler_tabela(&wb, 0, Rect::new(0, 0, 1_000_000, 12)).unwrap_err();
        assert!(erro.contains("Reduza a seleção"));
    }

    #[test]
    fn ribbon_e_menu_acionam_openquery_nativo() {
        let app = App::default();
        let acao = menu_nativo::Acao { id: "openOpenQuery".into(), args: Vec::new() };
        assert!(matches!(app.evento_menu(&acao), Some(Msg::OpenQuery(Evento::Abrir))));
    }

    #[test]
    fn etapas_ordenacao_renomeacao_e_advanced_json() {
        let fonte = Tabela {
            cabecalho: vec!["Num".into()],
            linhas: vec![vec![Value::Number(3.0)], vec![Value::Number(1.0)]],
        };
        let mut estado = Estado::novo(fonte, "Aba".into(), Rect::new(0, 0, 2, 0));
        estado.adicionar("ordenar");
        assert_eq!(estado.resultado.linhas[0][0], Value::Number(1.0));
        estado.renomear = "Novo".into();
        estado.adicionar("renomear");
        assert_eq!(estado.resultado.cabecalho[0], "Novo");
        assert!(estado.sincronizar().is_ok());
    }

    #[test]
    fn juntar_e_acrescentar_pedem_fonte_e_respeitam_preflight() {
        let fonte = Tabela {
            cabecalho: vec!["Id".into()],
            linhas: vec![vec![Value::Number(1.0)]],
        };
        let mut estado = Estado::novo(fonte.clone(), "Origem".into(), Rect::new(0, 0, 1, 0));
        estado.adicionar("juntar");
        assert!(estado.erro.contains("fonte adicional"));
        estado.fontes.insert("Destino".into(), fonte);
        estado.adicionar("juntar");
        assert!(estado.erro.is_empty());
        assert_eq!(estado.resultado.cabecalho.len(), 2);
        estado.adicionar("acrescentar");
        assert!(estado.erro.is_empty());
        assert_eq!(estado.resultado.linhas.len(), 2);
    }

    #[test]
    fn dividir_linhas_e_mesclar_colunas_com_separador_personalizado() {
        let fonte = Tabela {
            cabecalho: vec!["Nome".into(), "Unidade".into()],
            linhas: vec![
                vec![Value::Text("A;B".into()), Value::Text("MW".into())],
            ],
        };
        let mut estado = Estado::novo(fonte, "Origem".into(), Rect::new(0, 0, 1, 1));
        estado.adicionar("dividir");
        assert_eq!(estado.resultado.linhas.len(), 2);
        estado.adicionar("mesclar");
        assert_eq!(estado.resultado.cabecalho.last().unwrap(), "Mesclado");
    }

    #[test]
    fn gerenciar_consultas_cancela_e_desvincula_sem_apagar_celulas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Consumo".into());
        app.workbook.escrever_celula(0, 1, 0, "12".into());
        abrir_aba_para_teste(&mut app);
        app.concluir_openquery();
        let id = registro::listar(&app.workbook).unwrap()[0].dados["id"]
            .as_str().unwrap().to_string();
        app.evento_openquery(Evento::Abrir);
        app.evento_openquery(Evento::PedirExclusao(id.clone()));
        app.evento_openquery(Evento::CancelarExclusao);
        assert_eq!(registro::listar(&app.workbook).unwrap().len(), 1);
        app.evento_openquery(Evento::PedirExclusao(id));
        app.evento_openquery(Evento::ConfirmarExclusao);
        assert!(registro::listar(&app.workbook).unwrap().is_empty());
        assert_eq!(app.workbook.valor(1, 1, 0), Value::Number(12.0));
        assert_eq!(app.workbook.len(), 2);
    }

    #[test]
    fn etapa_previa_nao_altera_resultado_de_carga() {
        let fonte = Tabela {
            cabecalho: vec!["Id".into()],
            linhas: vec![vec![Value::Number(3.)], vec![Value::Number(1.)]],
        };
        let mut editor = Estado::novo(fonte, "Origem".into(), Rect::new(0,0,2,0));
        editor.adicionar("ordenar");
        editor.adicionar("limite");
        // A prévia de etapa 1 ainda mostra duas linhas, mas o resultado final
        // continua com a limitação aplicada.
        editor.prever(Some(0));
        assert_eq!(editor.tabela_visual().linhas.len(),2);
        assert_eq!(editor.resultado.linhas.len(),2);
        editor.quantidade = "1".into();
        editor.passos.pop();
        editor.adicionar("limite");
        editor.prever(Some(0));
        assert_eq!(editor.tabela_visual().linhas.len(),2);
        assert_eq!(editor.resultado.linhas.len(),1);
    }

    #[test]
    fn importacao_assincrona_antiga_nao_substitui_editor_atual() {
        let mut app = App::default();
        app.evento_openquery(Evento::Abrir);
        let ticket = 1000;
        app.openquery.as_mut().unwrap().importacao_pendente = Some(ticket + 1);
        let fonte = Tabela {
            cabecalho: vec!["Irrelevante".into()],
            linhas: vec![vec![Value::Number(999.)]],
        };
        app.evento_openquery(Evento::ArquivoLido(
            ticket, Ok((fonte, "Arquivo: antigo.csv".into())),
        ));
        assert_ne!(app.openquery.as_ref().unwrap().origem_aba, "Arquivo: antigo.csv");
    }

    #[test]
    fn escolher_coluna_fora_da_previa_mantem_indice_valido() {
        let mut editor = Estado::novo(Tabela {
            cabecalho: (0..20).map(|i| format!("C{i}")).collect(),
            linhas: vec![vec![Value::Number(1.);20]],
        }, "Fonte".into(), Rect::new(0,0,1,19));
        editor.coluna_numero = "20".into();
        if let Ok(i) = editor.coluna_numero.parse::<usize>() {
            editor.coluna = i - 1;
        }
        editor.adicionar("ordenar");
        assert!(editor.erro.is_empty());
        assert_eq!(editor.coluna,19);
    }

    #[test]
    fn carregar_cria_aba_e_preserva_valor_tipado() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Número".into());
        app.workbook.escrever_celula(0, 1, 0, "12".into());
        abrir_aba_para_teste(&mut app);
        assert!(app.openquery.is_some());
        app.evento_openquery(Evento::Carregar);
        assert!(app.openquery.is_none());
        assert_eq!(app.workbook.len(), 2);
        assert_eq!(app.workbook.valor(1, 1, 0), Value::Number(12.0));
        assert!(app.alterado);
    }

#[test]
fn entrada_manual_vira_consulta_reabrivel_sem_virar_formula() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    app.openquery.as_mut().unwrap().entrada_manual =
        pelicano::Content::with_text("Nome;Expressao\nCidade;=1+2");
    app.evento_openquery(Evento::AplicarManual);
    assert_eq!(app.openquery.as_ref().unwrap().origem_aba, "Entrada manual");
    app.concluir_openquery();
    let id = registro::listar(&app.workbook).unwrap()[0].dados["id"]
        .as_str().unwrap().to_string();
    assert_eq!(app.workbook.valor(1,1,1),Value::Text("=1+2".into()));
    assert!(app.abrir_consulta_salva(&id));
    assert_eq!(app.openquery.as_ref().unwrap().origem_aba,"Entrada manual");
}
#[test]
fn reimportacao_recusa_cabecalhos_diferentes_sem_alterar_passos() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    let editor = app.openquery.as_mut().unwrap();
    editor.fonte = Tabela {
        cabecalho: vec!["Original".into()],
        linhas: vec![vec![Value::Text("valor".into())]],
    };
    editor.origem_aba = "Arquivo: teste.csv".into();
    editor.importacao_pendente = Some(42);
    editor.importacao_preservar = true;
    editor.adicionar("limite");
    let antes = editor.passos.len();
    app.evento_openquery(Evento::ArquivoLido(
        42, Ok((Tabela {
            cabecalho: vec!["Mudou".into()],
            linhas: vec![vec![Value::Text("x".into())]],
        },"Arquivo: atualizado.csv".into())),
    ));
    assert_eq!(app.openquery.as_ref().unwrap().passos.len(),antes);
    assert_eq!(app.openquery.as_ref().unwrap().fonte.cabecalho[0],"Original");
    assert!(!app.openquery.as_ref().unwrap().erro.is_empty());
}

#[test]
fn web_multiplas_tabelas_escolhe_a_segunda_sem_substituir_fonte_antes() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    let ticket = 312;
    let primeira = Tabela {
        cabecalho: vec!["Produto".into()],
        linhas: vec![vec![Value::Text("A".into())]],
    };
    let segunda = Tabela {
        cabecalho: vec!["Cidade".into(), "Ativo".into()],
        linhas: vec![vec![Value::Text("Vitória".into()), Value::Bool(true)]],
    };
    let editor = app.openquery.as_mut().unwrap();
    editor.importacao_pendente = Some(ticket);
    editor.mostrar_web = true;
    app.evento_openquery(Evento::TabelasWebLidas(
        ticket, Ok((vec![primeira, segunda], "Web: https://exemplo.com/".into())),
    ));
    let editor = app.openquery.as_ref().unwrap();
    assert_eq!(editor.tabelas_web.len(), 2);
    assert!(editor.fonte.cabecalho.is_empty()); // nenhuma carga antes de confirmar
    assert_eq!(editor.indice_tabela_web, 0);
    app.evento_openquery(Evento::SelecionarTabelaWeb(1));
    assert_eq!(app.openquery.as_ref().unwrap().indice_tabela_web, 1);
    app.evento_openquery(Evento::ConfirmarTabelaWeb);
    let editor = app.openquery.as_ref().unwrap();
    assert_eq!(editor.fonte.cabecalho, ["Cidade", "Ativo"]);
    assert_eq!(editor.fonte.linhas[0][1], Value::Bool(true));
    assert_eq!(editor.origem_aba, "Web: https://exemplo.com/");
    assert!(editor.tabelas_web.is_empty());
    assert_eq!(editor.importacao_pendente, None);
}

#[test]
fn web_cancelamento_e_resposta_antiga_nao_mudam_a_fonte() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    let dados = Tabela {
        cabecalho: vec!["Valor".into()],
        linhas: vec![vec![Value::Number(50.)]],
    };
    app.openquery.as_mut().unwrap().importacao_pendente = Some(413);
    app.evento_openquery(Evento::TabelasWebLidas(
        412, Ok((vec![dados.clone()], "Web: https://antigo.test/".into())),
    ));
    assert!(app.openquery.as_ref().unwrap().fonte.cabecalho.is_empty());
    app.evento_openquery(Evento::TabelasWebLidas(
        413, Ok((vec![dados.clone(), dados.clone()], "Web: https://atual.test/".into())),
    ));
    app.evento_openquery(Evento::CancelarSelecaoWeb);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.tabelas_web.is_empty());
    assert_eq!(editor.importacao_pendente, None);
    assert!(editor.fonte.cabecalho.is_empty());
    app.evento_openquery(Evento::ConfirmarTabelaWeb);
    assert!(app.openquery.as_ref().unwrap().fonte.cabecalho.is_empty());
}

#[test]
fn web_uma_tabela_importa_diretamente_sem_seletor() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    app.openquery.as_mut().unwrap().importacao_pendente = Some(99);
    let fonte = Tabela {
        cabecalho: vec!["Nome".into()],
        linhas: vec![vec![Value::Text("Ana".into())]],
    };
    app.evento_openquery(Evento::TabelasWebLidas(
        99, Ok((vec![fonte], "Web: https://api.exemplo.com/dados".into())),
    ));
    let editor = app.openquery.as_ref().unwrap();
    assert_eq!(editor.fonte.cabecalho, ["Nome"]);
    assert!(editor.tabelas_web.is_empty());
    assert!(!editor.mostrar_web);
}

#[test]
fn web_reimportacao_preserva_passos_ao_escolher_tabela_compativel() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    let editor = app.openquery.as_mut().unwrap();
    editor.fonte = Tabela {
        cabecalho: vec!["Valor".into()],
        linhas: vec![vec![Value::Number(1.)]],
    };
    editor.origem_aba = "Web: https://exemplo.com/".into();
    editor.url_web = "https://exemplo.com".into();
    editor.adicionar("limite");
    let passos_originais = editor.passos.len();
    // Não executa o download do Task retornado.
    app.evento_openquery(Evento::ImportarWeb);
    let ticket = app.openquery.as_ref().unwrap().importacao_pendente.unwrap();
    assert!(app.openquery.as_ref().unwrap().importacao_preservar);
    app.evento_openquery(Evento::TabelasWebLidas(ticket, Ok((
        vec![
            Tabela { cabecalho: vec!["Outra".into()],
                linhas: vec![vec![Value::Number(99.)]] },
            Tabela { cabecalho: vec!["Valor".into()],
                linhas: vec![vec![Value::Number(4.)]] },
        ],
        "Web: https://exemplo.com/".into(),
    ))));
    assert_eq!(app.openquery.as_ref().unwrap().passos.len(), passos_originais);
    app.evento_openquery(Evento::SelecionarTabelaWeb(1));
    app.evento_openquery(Evento::ConfirmarTabelaWeb);
    let editor = app.openquery.as_ref().unwrap();
    assert_eq!(editor.passos.len(), passos_originais);
    assert_eq!(editor.fonte.linhas[0][0], Value::Number(4.));
    assert!(editor.erro.is_empty());
}

#[test]
fn web_trocando_url_descarta_previa_e_invalida_download() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    let editor = app.openquery.as_mut().unwrap();
    editor.url_web = "https://origem.test".into();
    editor.importacao_pendente = Some(5);
    editor.tabelas_web.push(Tabela {
        cabecalho: vec!["Teste".into()],
        linhas: vec![vec![Value::Number(5.)]],
    });
    app.evento_openquery(Evento::UrlWeb("https://outra.test".into()));
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.tabelas_web.is_empty());
    assert!(editor.importacao_pendente.is_none());
}

#[test]
fn editor_avancado_aplica_rascunho_valido_e_mostra_script_completo() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    {
        let editor = app.openquery.as_mut().unwrap();
        editor.fonte = Tabela {
            cabecalho: vec!["Valor".into()],
            linhas: vec![vec![Value::Number(10.)], vec![Value::Number(20.)]],
        };
        editor.registrar_passos();
    }
    app.evento_openquery(Evento::AbrirEditorAvancado);
    assert!(app.openquery.as_ref().unwrap().editor_avancado.is_some());
    app.openquery.as_mut().unwrap().editor_avancado =
        Some(pelicano::Content::with_text(
            r#"const etapas = [{"tipo":"limite","quantas":1}];"#,
        ));
    app.evento_openquery(Evento::MostrarScriptAvancado);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.script_avancado.as_ref().unwrap().text().contains("manterPrimeirasLinhas"));
    let _ = app.view_openquery();
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.editor_avancado.is_none());
    assert_eq!(editor.passos.len(), 1);
    assert_eq!(editor.resultado.linhas.len(), 1);
    assert!(editor.codigo.text().contains("limite"));
}

#[test]
fn editor_avancado_cancela_e_recusa_codigo_invalido_sem_alterar_consulta() {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    {
        let editor = app.openquery.as_mut().unwrap();
        editor.fonte = Tabela {
            cabecalho: vec!["Valor".into()],
            linhas: vec![vec![Value::Number(10.)], vec![Value::Number(20.)]],
        };
        editor.registrar_passos();
    }
    let original = app.openquery.as_ref().unwrap().codigo.text();
    app.evento_openquery(Evento::AbrirEditorAvancado);
    app.openquery.as_mut().unwrap().editor_avancado =
        Some(pelicano::Content::with_text("console.log('não executa');"));
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.editor_avancado.is_some());
    assert!(!editor.erro_avancado.is_empty());
    assert_eq!(editor.codigo.text(), original);
    assert_eq!(editor.resultado.linhas.len(), 2);
    assert!(app.fechar_assistente_openquery()); // Esc descarta o rascunho
    assert!(app.openquery.as_ref().unwrap().editor_avancado.is_none());
    assert_eq!(app.openquery.as_ref().unwrap().codigo.text(), original);
}

    #[test]
    fn de_tabela_intervalo_abre_com_a_regiao_da_celula() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Cidade\tMWh\nVitória\t42\nSerra\t7".into()));
        let _ = app.update(Msg::IrPara("B2".into()));
        let _ = app.update(Msg::OpenQuery(Evento::AbrirCom(Box::new(Evento::CarregarSelecao))));
        let e = app.openquery.as_ref().expect("OpenQuery aberto");
        assert_eq!(e.fonte.cabecalho, ["Cidade", "MWh"]);
        assert_eq!(e.fonte.linhas.len(), 2);
    }

    #[test]
    fn fontes_recentes_sem_repetir_e_a_mais_nova_primeiro() {
        let mut app = App::default();
        app.lembrar_fonte("/tmp/a.csv".into());
        app.lembrar_fonte("https://x.org/t".into());
        app.lembrar_fonte("/tmp/a.csv".into());
        assert_eq!(app.config.fontes_recentes, ["/tmp/a.csv", "https://x.org/t"]);
    }
