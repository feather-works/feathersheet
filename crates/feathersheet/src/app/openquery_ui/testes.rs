//! Testes do editor nativo FeatherQuery — separados para manter o modulo UI pequeno.
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
    fn selecao_enorme_sem_limite_pula_linhas_vazias() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "Cabeçalho".into());
        wb.escrever_celula(0, 1_000_000, 12, "fim".into());
        let tabela = ler_tabela(&wb, 0, Rect::new(0, 0, 1_000_000, 12)).unwrap();
        assert_eq!(tabela.cabecalho.len(), 13);
        assert_eq!(tabela.linhas, [[vec![Value::Empty; 12], vec![Value::Text("fim".into())]].concat()]);
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
fn editor_avancado_aplica_script_escrito_a_mao() {
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
            "let fonte = dadosDigitados();\nfonte.manterPrimeiras(1);",
        ));
    let _ = app.view_openquery();
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.editor_avancado.is_none());
    assert_eq!(editor.passos.len(), 1);
    assert_eq!(editor.resultado.linhas.len(), 1);
    assert!(editor.codigo.clone().contains("fonte.manterPrimeiras(1);"), "{}", editor.codigo.clone());
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
    let original = app.openquery.as_ref().unwrap().codigo.clone();
    app.evento_openquery(Evento::AbrirEditorAvancado);
    app.openquery.as_mut().unwrap().editor_avancado =
        Some(pelicano::Content::with_text("console.log('não executa');"));
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.editor_avancado.is_some());
    assert!(!editor.erro_avancado.is_empty());
    assert_eq!(editor.codigo.clone(), original);
    assert_eq!(editor.resultado.linhas.len(), 2);
    assert!(app.fechar_assistente_openquery()); // Esc descarta o rascunho
    assert!(app.openquery.as_ref().unwrap().editor_avancado.is_none());
    assert_eq!(app.openquery.as_ref().unwrap().codigo.clone(), original);
}

    #[test]
    fn de_tabela_intervalo_abre_com_a_regiao_da_celula() {
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Cidade\tMWh\nVitória\t42\nSerra\t7".into()));
        let _ = app.update(Msg::IrPara("B2".into()));
        let _ = app.update(Msg::OpenQuery(Evento::AbrirCom(Box::new(Evento::CarregarSelecao))));
        let e = app.openquery.as_ref().expect("FeatherQuery aberto");
        assert_eq!(e.fonte.cabecalho, ["Cidade", "MWh"]);
        assert_eq!(e.fonte.linhas.len(), 2);
    }

    #[test]
    fn data_da_fonte_aparece_formatada_e_volta_formatada_na_aba() {
        use feathersheet_core::celulas::style::NumberFormat;
        let mut app = App::default();
        let _ = app.update(Msg::Colar("Data\tN\n27/10/2026\t5".into()));
        let _ = app.update(Msg::IrPara("A2".into()));
        let _ = app.update(Msg::OpenQuery(Evento::AbrirCom(Box::new(Evento::CarregarSelecao))));
        let e = app.openquery.as_ref().unwrap();
        assert_eq!(e.formatos.get("Data").map(|s| s.format), Some(NumberFormat::Date));
        app.evento_openquery(Evento::Carregar);
        let estilo = app.workbook.aba(1).unwrap().sheet.get_style(1, 0).cloned();
        assert_eq!(estilo.map(|s| s.format), Some(NumberFormat::Date));
    }

    #[test]
    fn fontes_recentes_sem_repetir_e_a_mais_nova_primeiro() {
        let mut app = App::default();
        app.lembrar_fonte("/tmp/a.csv".into());
        app.lembrar_fonte("https://x.org/t".into());
        app.lembrar_fonte("/tmp/a.csv".into());
        assert_eq!(app.config.fontes_recentes, ["/tmp/a.csv", "https://x.org/t"]);
    }


    #[test]
    fn ctrl_z_desfaz_e_refaz_as_etapas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Valor".into());
        for (r, v) in ["3", "1", "2"].iter().enumerate() { app.workbook.escrever_celula(0, r as u32 + 1, 0, (*v).into()); }
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Adicionar("ordenar"));
        app.evento_openquery(Evento::Adicionar("indice"));
        assert_eq!(app.openquery.as_ref().unwrap().passos.len(), 2);
        app.evento_openquery(Evento::Desfazer(true));
        let e = app.openquery.as_ref().unwrap();
        assert_eq!(e.passos.len(), 1);
        assert_eq!(e.resultado.cabecalho, ["Valor"], "o resultado volta junto");
        app.evento_openquery(Evento::Desfazer(true));
        assert!(app.openquery.as_ref().unwrap().passos.is_empty());
        app.evento_openquery(Evento::Desfazer(true));
        assert_eq!(app.status, "FeatherQuery: nada a desfazer");
        app.evento_openquery(Evento::Desfazer(false));
        app.evento_openquery(Evento::Desfazer(false));
        assert_eq!(app.openquery.as_ref().unwrap().passos.len(), 2, "Ctrl+Y refaz as duas");
        // Etapa nova depois de desfazer apaga o refazer.
        app.evento_openquery(Evento::Desfazer(true));
        app.evento_openquery(Evento::Adicionar("inverter"));
        app.evento_openquery(Evento::Desfazer(false));
        assert_eq!(app.status, "FeatherQuery: nada a refazer");
    }

    #[test]
    fn duplicar_e_referenciar_consulta_gravada() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Valor".into());
        for (r, v) in ["3", "1"].iter().enumerate() { app.workbook.escrever_celula(0, r as u32 + 1, 0, (*v).into()); }
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Nome("Base".into()));
        app.evento_openquery(Evento::Adicionar("ordenar"));
        app.concluir_openquery();
        let id = registro::listar(&app.workbook).unwrap()[0].dados["id"].as_str().unwrap().to_owned();
        assert!(app.abrir_consulta_salva(&id));
        app.evento_openquery(Evento::Duplicar);
        let salvas = registro::listar(&app.workbook).unwrap();
        assert_eq!(salvas.len(), 2);
        assert_eq!(salvas[1].dados["nome"], "Base (2)");
        assert_eq!(salvas[1].dados["destino"]["modo"], "apenas_conexao");
        // Referenciar: a fonte é o resultado da "Base" (já ordenado).
        app.evento_openquery(Evento::Referenciar);
        let e = app.openquery.as_ref().unwrap();
        assert_eq!(e.origem_aba, "Consulta: Base");
        assert_eq!(e.fonte.linhas[0][0], Value::Number(1.0));
        app.evento_openquery(Evento::Adicionar("inverter"));
        app.concluir_openquery();
        let destino = app.folha;
        assert_eq!(app.workbook.valor(destino, 1, 0), Value::Number(3.0));
        // A origem muda: atualizar a referência recalcula a "Base" antes.
        app.workbook.escrever_celula(0, 1, 0, "9".into());
        let ref_id = registro::listar(&app.workbook).unwrap().into_iter()
            .find(|s| s.dados["nome"] == "Base (referência)").unwrap().dados["id"].as_str().unwrap().to_owned();
        app.atualizar_consulta_salva(&ref_id);
        assert_eq!(app.workbook.valor(destino, 1, 0), Value::Number(9.0));
    }

    #[test]
    fn aviso_de_tipo_conta_o_que_nao_converteu_e_remove_so_essas_linhas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Valor".into());
        app.workbook.escrever_celula(0, 0, 1, "Nome".into());
        for (r, v) in ["10", "N/D", "", "2"].iter().enumerate() {
            app.workbook.escrever_celula(0, r as u32 + 1, 0, (*v).into());
            app.workbook.escrever_celula(0, r as u32 + 1, 1, "x".into());
        }
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Tipar(0, "numero"));
        let aviso = app.openquery.as_ref().unwrap().aviso_tipo.clone().expect("aviso");
        assert_eq!((aviso.falhas, aviso.exemplos.as_slice()), (1, &["N/D".to_string()][..]), "a vazia não conta");
        assert!(matches!(app.openquery.as_ref().unwrap().passos[..], [Passo::Tipar { erros: true, .. }]),
            "o que não converte vira erro já na etapa de tipo");
        app.evento_openquery(Evento::FalhasDeTipo(0));
        let e = app.openquery.as_ref().unwrap();
        assert!(e.aviso_tipo.is_none());
        assert!(matches!(e.passos[..], [Passo::Tipar { erros: true, .. }, Passo::RemoverErros { .. }]),
            "um botão, uma etapa: a de tipo não muda");
        assert_eq!(e.resultado.linhas.len(), 3, "sai só a linha do N/D; a vazia fica");
        app.evento_openquery(Evento::Desfazer(true));
        app.evento_openquery(Evento::FalhasDeTipo(1));
        let e = app.openquery.as_ref().unwrap();
        assert_eq!(e.resultado.linhas[1][0], Value::Number(0.0));
        assert_eq!(e.resultado.linhas[2][0], Value::Empty);
    }

    #[test]
    fn dependencias_poem_a_referencia_depois_da_consulta_que_ela_le_e_esc_fecha() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Valor".into());
        app.workbook.escrever_celula(0, 1, 0, "1".into());
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Nome("Base".into()));
        app.concluir_openquery();
        let id = registro::listar(&app.workbook).unwrap()[0].dados["id"].as_str().unwrap().to_owned();
        assert!(app.abrir_consulta_salva(&id));
        app.evento_openquery(Evento::Referenciar);
        app.concluir_openquery();
        let mapa = registro::dependencias(&app.workbook);
        assert_eq!(mapa.len(), 2);
        assert_eq!(mapa[0].0, "Base");
        assert_eq!(mapa[1].1, ["Consulta: Base"]);
        assert!(mapa[1].2.starts_with("aba "));
        abrir_aba_para_teste(&mut app);
        app.evento_openquery(Evento::Dependencias(true));
        assert_eq!(app.openquery.as_ref().unwrap().dependencias.as_ref().map(Vec::len), Some(2));
        assert!(app.fechar_assistente_openquery(), "Esc fecha o mapa, não a janela");
        app.evento_openquery(Evento::Adicionar("filtrar"));
        assert!(app.fechar_assistente_openquery(), "Esc fecha o modal da etapa");
        assert!(app.openquery.as_ref().unwrap().modal.is_none());
    }

#[test]
fn trocar_planilha_pelo_script_rele_o_arquivo_e_recusa_a_que_nao_existe() {
    let tabela = |cab: &str, v: f64| Tabela { cabecalho: vec![cab.into()], linhas: vec![vec![Value::Number(v)]] };
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    {
        let e = app.openquery.as_mut().unwrap();
        e.fonte = tabela("Valor", 1.);
        e.origem_aba = "Arquivo: vendas.fshz ▸ Planilha Jan".into();
        e.registrar_passos();
        assert!(e.codigo.contains("fonte.planilha(\"Jan\");"), "{}", e.codigo);
        e.editor_avancado = Some(pelicano::Content::with_text(&e.codigo.replace("Jan", "Fev")));
    }
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let e = app.openquery.as_mut().unwrap();
    assert_eq!(e.item_pedido.as_deref(), Some("Planilha Fev"));
    let ticket = e.importacao_pendente.expect("relendo o arquivo");
    let itens = || vec![importador::Item::pronto("Planilha Jan".into(), tabela("Valor", 1.), Formatos::new()),
        importador::Item::pronto("Planilha Fev".into(), tabela("Valor", 2.), Formatos::new())];
    app.evento_openquery(Evento::ItensLidos(ticket, itens(), "Arquivo: vendas.fshz".into()));
    let e = app.openquery.as_ref().unwrap();
    assert!(e.erro.is_empty() && e.item_pedido.is_none(), "{}", e.erro);
    // A leitura em segundo plano devolve a Fev.
    app.evento_openquery(Evento::ItemLido(ticket, Ok((tabela("Valor", 2.), Formatos::new())), "Arquivo: vendas.fshz ▸ Planilha Fev".into()));
    let e = app.openquery.as_mut().unwrap();
    assert_eq!((e.origem_aba.as_str(), &e.fonte.linhas[0][0]), ("Arquivo: vendas.fshz ▸ Planilha Fev", &Value::Number(2.)));
    // Uma planilha que o arquivo não tem: erro, e a fonte fica.
    e.item_pedido = Some("Planilha Mar".into());
    e.importacao_preservar = true;
    e.importacao_pendente = Some(ticket + 1);
    app.evento_openquery(Evento::ItensLidos(ticket + 1, itens(), "Arquivo: vendas.fshz".into()));
    let e = app.openquery.as_ref().unwrap();
    assert!(e.erro.contains("Mar"), "{}", e.erro);
    assert_eq!(e.origem_aba, "Arquivo: vendas.fshz ▸ Planilha Fev");
}

#[test]
fn renomear_e_mover_seguidos_viram_uma_etapa_e_filtro_por_lista() {
    let mut app = App::default();
    for (r, l) in [["UF", "Valor", "Nome"], ["SP", "1", "a"], ["RJ", "2", "b"], ["SP", "3", "c"]].iter().enumerate() {
        for (c, v) in l.iter().enumerate() { app.workbook.escrever_celula(0, r as u32, c as u32, (*v).into()); }
    }
    abrir_aba_para_teste(&mut app);
    let renomear = |app: &mut App, col: usize, nome: &str| {
        app.openquery.as_mut().unwrap().coluna = col;
        app.evento_openquery(Evento::Adicionar("renomear"));
        app.evento_openquery(Evento::Etapa(etapa::Evento::Texto(nome.into())));
        app.evento_openquery(Evento::Etapa(etapa::Evento::Aplicar));
    };
    renomear(&mut app, 0, "Estado");
    renomear(&mut app, 1, "Total");
    renomear(&mut app, 0, "UF2");
    let e = app.openquery.as_ref().unwrap();
    assert!(matches!(&e.passos[..], [Passo::RenomearColunas { nomes }] if nomes.len() == 2), "{:?}", e.passos);
    assert_eq!(e.resultado.cabecalho, ["UF2", "Total", "Nome"]);
    // Mover a coluna 0 duas vezes para a direita: uma etapa, e a seleção vai junto.
    app.openquery.as_mut().unwrap().coluna = 0;
    app.evento_openquery(Evento::Adicionar("mover_direita"));
    app.evento_openquery(Evento::Adicionar("mover_direita"));
    let e = app.openquery.as_ref().unwrap();
    assert_eq!(e.passos.len(), 2);
    assert_eq!(e.resultado.cabecalho, ["Total", "Nome", "UF2"]);
    // Filtro pela lista: só SP marcado.
    app.openquery.as_mut().unwrap().coluna = 2;
    app.evento_openquery(Evento::Adicionar("filtrar"));
    app.evento_openquery(Evento::Etapa(etapa::Evento::FiltroLista(true)));
    // Valores ordenados: RJ, SP. Desmarca o RJ.
    app.evento_openquery(Evento::Etapa(etapa::Evento::Marcar(0)));
    app.evento_openquery(Evento::Etapa(etapa::Evento::Aplicar));
    let e = app.openquery.as_ref().unwrap();
    assert!(e.modal.is_none(), "{}", e.erro_modal);
    assert_eq!(e.resultado.linhas.len(), 2);
    // Duplicar a coluna.
    app.evento_openquery(Evento::Adicionar("duplicar"));
    assert_eq!(app.openquery.as_ref().unwrap().resultado.cabecalho.last().map(String::as_str), Some("UF2 - Cópia"));
}

/// Um `.xlsx` só com o DataMashup (o que o Excel grava quando a pasta tem Power Query).
fn xlsx_com_power_query(secao: &str) -> std::path::PathBuf {
    use std::io::Write;
    let zip_com = |partes: &[(&str, &[u8])]| {
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (nome, bytes) in partes {
            w.start_file(*nome, zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap().into_inner()
    };
    let pacote = zip_com(&[("Formulas/Section1.m", secao.as_bytes())]);
    let mut bin = 0u32.to_le_bytes().to_vec();
    bin.extend((pacote.len() as u32).to_le_bytes());
    bin.extend(&pacote);
    let xml = format!("<DataMashup xmlns=\"http://schemas.microsoft.com/DataMashup\">{}</DataMashup>",
        feathersheet_io::base64::codificar(&bin));
    let caminho = std::env::temp_dir().join(format!("fs-pq-{}-{}.xlsx", std::process::id(), secao.len()));
    std::fs::write(&caminho, zip_com(&[("customXml/item1.xml", xml.as_bytes())])).unwrap();
    caminho
}

fn app_com_valores() -> App {
    let mut app = App::default();
    app.evento_openquery(Evento::Abrir);
    let editor = app.openquery.as_mut().unwrap();
    editor.fonte = Tabela {
        cabecalho: vec!["UF".into(), "Valor".into()],
        linhas: vec![
            vec![Value::Text("SP".into()), Value::Number(10.)],
            vec![Value::Text("RJ".into()), Value::Number(20.)],
            vec![Value::Text("SP".into()), Value::Number(30.)],
        ],
    };
    editor.registrar_passos();
    app
}

#[test]
fn consultas_do_power_query_do_xlsx_abrem_no_editor_avancado() {
    let secao = "section Section1;\r\n\r\nshared Vendas = let\r\n    Fonte = Excel.CurrentWorkbook(){[Name=\"Vendas\"]}[Content],\r\n    #\"Linhas Filtradas\" = Table.SelectRows(Fonte, each [UF] = \"SP\"),\r\n    #\"Linhas Agrupadas\" = Table.Group(#\"Linhas Filtradas\", {\"UF\"}, {{\"Total\", each List.Sum([Valor]), type number}})\r\nin\r\n    #\"Linhas Agrupadas\";\r\n\r\nshared Outra = 1;";
    let caminho = xlsx_com_power_query(secao);
    let mut app = app_com_valores();
    // Sem .xlsx aberto, o item nem aparece.
    assert!(app.arquivo_xlsx_aberto().is_none());
    app.caminho = Some(caminho.clone());
    app.evento_openquery(Evento::ConsultasPowerQuery);
    let lista = app.openquery.as_ref().unwrap().consultas_pq.clone().expect("lista aberta");
    assert_eq!(lista.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["Vendas", "Outra"]);
    let _ = app.view_openquery();
    app.evento_openquery(Evento::AbrirConsultaPowerQuery(0));
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.consultas_pq.is_none());
    assert_eq!(editor.nome, "Vendas");
    let texto = editor.editor_avancado.as_ref().expect("editor avançado aberto").text();
    assert!(texto.contains("fonte.filtrar(\"UF\", \"SP\");"), "{texto}");
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.erro_avancado.is_empty(), "{}", editor.erro_avancado);
    assert_eq!(editor.resultado.cabecalho, ["UF", "Total"]);
    assert_eq!(editor.resultado.linhas, [vec![Value::Text("SP".into()), Value::Number(40.)]]);
    let _ = std::fs::remove_file(caminho);

    // Arquivo sem Power Query: avisa e não abre lista.
    let vazio = xlsx_com_power_query("section Section1;");
    app.caminho = Some(vazio.clone());
    app.evento_openquery(Evento::ConsultasPowerQuery);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.consultas_pq.is_none() && !editor.erro.is_empty());
    let _ = std::fs::remove_file(vazio);
}

#[test]
fn m_colado_no_editor_avancado_aplica_ou_para_no_que_nao_converte() {
    let mut app = app_com_valores();
    app.evento_openquery(Evento::AbrirEditorAvancado);
    app.openquery.as_mut().unwrap().editor_avancado = Some(pelicano::Content::with_text(
        "let\n    Fonte = Excel.CurrentWorkbook(){[Name=\"T\"]}[Content],\n    Filtradas = Table.SelectRows(Fonte, each [Valor] > 15)\nin\n    Filtradas"));
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let editor = app.openquery.as_ref().unwrap();
    assert!(editor.editor_avancado.is_none(), "{}", editor.erro_avancado);
    assert_eq!(editor.resultado.linhas.len(), 2);

    app.evento_openquery(Evento::AbrirEditorAvancado);
    app.openquery.as_mut().unwrap().editor_avancado = Some(pelicano::Content::with_text(
        "let\n    Fonte = Excel.CurrentWorkbook(){[Name=\"T\"]}[Content],\n    P = Table.Profile(Fonte)\nin\n    P"));
    app.evento_openquery(Evento::AplicarEditorAvancado);
    let editor = app.openquery.as_ref().unwrap();
    let texto = editor.editor_avancado.as_ref().expect("fica aberto para conferir").text();
    assert!(texto.contains("Table.Profile") && texto.starts_with("let fonte = "), "{texto}");
    assert!(!editor.aviso_avancado.is_empty());
    assert_eq!(editor.passos.len(), 1, "a consulta anterior não mudou");
}
