//! Testes de formatação condicional, viewport e exportação.
use super::*;
use feathersheet_core::Value;
    #[test]
    fn condicional_editar_regras_ts_preserva_limites_numericos_e_percentuais() {
        use feathersheet_core::operacoes::rules::{
            Condition, ConditionalRule, ColorScaleCondition, DataBarCondition,
            Faixa, ScalePoint, ScalePointType,
        };
        let area = vec![Faixa {r0:0,c0:0,r1:3,c1:1}];
        let escala = ConditionalRule {
            id: "typescript-escala".into(),
            ranges: area.clone(),
            condition: Condition::ColorScale(ColorScaleCondition {
                min: ScalePoint {
                    point_type: ScalePointType::Number,
                    value: Some(12.0),color:"#111111".into(),
                },
                mid: Some(ScalePoint {
                    point_type: ScalePointType::Percent,
                    value: Some(40.0),color:"#888888".into(),
                }),
                max: ScalePoint {
                    point_type: ScalePointType::Percentile,
                    value: Some(95.0),color:"#eeeeee".into(),
                },
            }),
        };
        let barras = ConditionalRule {
            id: "typescript-barra".into(),ranges:area,
            condition: Condition::DataBar(DataBarCondition {
                color:"#22aa55".into(),negative_color:None,hide_value:false,
                min:Some(ScalePoint {
                    point_type:ScalePointType::Number,value:Some(-20.0),
                    color:"#222222".into(),
                }),
                max:Some(ScalePoint {
                    point_type:ScalePointType::Number,value:Some(100.0),
                    color:"#eeeeee".into(),
                }),
            }),
        };
        let mut app=App::default();
        condicional::gravar(&mut app.workbook,0,vec![escala.clone(),barras.clone()]).unwrap();
        app.update(Msg::CondicionalEditar(0));
        app.update(Msg::CondicionalCor("#abcdef".into()));
        app.update(Msg::CondicionalAdicionar);
        let regras=condicional::regras(&app.workbook,0);
        assert_eq!(regras.len(),2);
        if let Condition::ColorScale(ref e)=regras[0].condition {
            assert_eq!(e.min.point_type,ScalePointType::Number);
            assert_eq!(e.min.value,Some(12.0));
            assert_eq!(e.mid.as_ref().unwrap().point_type,ScalePointType::Percent);
            assert_eq!(e.mid.as_ref().unwrap().value,Some(40.0));
            assert_eq!(e.max.point_type,ScalePointType::Percentile);
            assert_eq!(e.max.value,Some(95.0));
            assert_eq!(e.max.color,"#abcdef");
        } else {panic!("escala original foi substituída por outro tipo")}
        app.update(Msg::CondicionalEditar(1));
        app.update(Msg::CondicionalOcultarValor(true));
        app.update(Msg::CondicionalAdicionar);
        let regras=condicional::regras(&app.workbook,0);
        if let Condition::DataBar(ref b)=regras[1].condition {
            assert_eq!(b.min.as_ref().and_then(|p|p.value),Some(-20.0));
            assert_eq!(b.max.as_ref().and_then(|p|p.value),Some(100.0));
            assert!(b.hide_value);
        } else {panic!("barra original foi substituída por outro tipo")}
        assert_eq!(regras[0].id,escala.id);
        assert_eq!(regras[1].id,barras.id);
    }

    #[test]
    fn cores_alternadas_nao_sobrescrevem_regra_sendo_editada() {
        let mut app=App::default();
        app.update(Msg::CondicionalAdicionar);
        let primeira=condicional::regras(&app.workbook,0);
        app.update(Msg::CondicionalEditar(0));
        app.update(Msg::CondicionalCoresAlternadas);
        let novas=condicional::regras(&app.workbook,0);
        assert_eq!(novas.len(),2);
        assert_eq!(novas[0],primeira[0]);
        assert!(app.condicional_editando.is_none());
    }

    #[test]
    fn condicional_editar_mantem_identidade_intervalos_e_prioridade() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "15".into());
        app.update(Msg::IrPara("A1:B2".into()));
        app.update(Msg::CondicionalValor("10".into()));
        app.update(Msg::CondicionalCor("#ff0000".into()));
        app.update(Msg::CondicionalAdicionar);
        let primeira = condicional::regras(&app.workbook, 0);
        assert_eq!(primeira.len(), 1);
        let id = primeira[0].id.clone();
        let intervalos = primeira[0].ranges.clone();

        // Mudar seleção durante a edição não troca o alvo já salvo.
        app.update(Msg::Selecionar(7, 8));
        app.update(Msg::CondicionalEditar(0));
        assert_eq!(app.condicional_editando, Some(0));
        assert_eq!(app.condicional_operador, TipoCondicional::Maior);
        assert_eq!(app.condicional_valor, "10");
        app.update(Msg::CondicionalValor("12".into()));
        app.update(Msg::CondicionalCor("#00ff00".into()));
        app.update(Msg::CondicionalAdicionar);
        assert!(app.condicional_editando.is_none());

        let depois = condicional::regras(&app.workbook, 0);
        assert_eq!(depois.len(), 1);
        assert_eq!(depois[0].id, id);
        assert_eq!(depois[0].ranges, intervalos);
        assert_eq!(
            app.condicional.borrow_mut()
                .formato(&app.workbook, 0, 0, 0)
                .unwrap().style.unwrap().bg.as_deref(),
            Some("#00ff00")
        );
        assert!(app.condicional.borrow_mut().formato(&app.workbook, 0, 7, 8).is_none());
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "15");
    }

    #[test]
    fn condicional_editar_escala_e_barra_carrega_opcoes_visuais() {
        let mut app = App::default();
        app.update(Msg::CondicionalTipo(TipoCondicional::Escala));
        app.update(Msg::CondicionalCorMin("#123456".into()));
        app.update(Msg::CondicionalCorMeio("#abcdef".into()));
        app.update(Msg::CondicionalPontoMeio(true));
        app.update(Msg::CondicionalCor("#fedcba".into()));
        app.update(Msg::CondicionalAdicionar);

        app.update(Msg::CondicionalPontoMeio(false));
        app.update(Msg::CondicionalEditar(0));
        assert_eq!(app.condicional_operador, TipoCondicional::Escala);
        assert!(app.condicional_ponto_meio);
        assert_eq!(app.condicional_cor_min, "#123456");
        assert_eq!(app.condicional_cor_meio, "#abcdef");
        assert_eq!(app.condicional_cor, "#fedcba");

        app.update(Msg::CondicionalTipo(TipoCondicional::Barra));
        app.update(Msg::CondicionalCor("#3377aa".into()));
        app.update(Msg::CondicionalCorNegativa("#bb4400".into()));
        app.update(Msg::CondicionalOcultarValor(true));
        app.update(Msg::CondicionalAdicionar);
        assert_eq!(condicional::regras(&app.workbook, 0).len(), 1);
        app.update(Msg::CondicionalOcultarValor(false));
        app.update(Msg::CondicionalEditar(0));
        assert_eq!(app.condicional_operador, TipoCondicional::Barra);
        assert_eq!(app.condicional_cor, "#3377aa");
        assert_eq!(app.condicional_cor_negativa, "#bb4400");
        assert!(app.condicional_ocultar_valor);
    }

    #[test]
    fn condicional_maior_que_altera_canvas_sem_alterar_estilo_salve() {
        let mut app = App::default();
        app.workbook.escrever_celula(0,0,0,"15".into());
        let estilo_original=app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0,0);
        // O botão abre o menu; "Nova regra" abre o painel da regra.
        app.update(Msg::CondicionalAbrir);
        app.update(Msg::CondicionalNovaRegra);
        assert!(app.condicional_aberto);
        app.update(Msg::CondicionalTipo(TipoCondicional::Maior));
        app.update(Msg::CondicionalValor("10".into()));
        app.update(Msg::CondicionalCor("#ff0000".into()));
        app.update(Msg::CondicionalAdicionar);
        let cond=app.condicional.borrow_mut().formato(&app.workbook,0,0,0).unwrap();
        assert_eq!(cond.style.unwrap().bg.as_deref(),Some("#ff0000"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0,0),estilo_original);
        app.workbook.escrever_celula(0,0,0,"5".into());
        assert!(app.condicional.borrow_mut().formato(&app.workbook,0,0,0).is_none());
        assert!(app.alterado);
    }

    #[test]
    fn condicional_escala_e_barras_reagem_a_edicao_de_valores() {
        let mut app=App::default();
        for (r,s) in ["0","50","100"].iter().enumerate() {
            app.workbook.escrever_celula(0,r as u32,0,(*s).into());
        }
        app.update(Msg::IrPara("A1:A3".into()));
        app.update(Msg::CondicionalTipo(TipoCondicional::Escala));
        app.update(Msg::CondicionalCor("#ff0000".into()));
        app.update(Msg::CondicionalAdicionar);
        let primeiro=app.condicional.borrow_mut().formato(&app.workbook,0,0,0).unwrap();
        assert_eq!(primeiro.style.unwrap().bg.as_deref(),Some("#fff2b2"));
        let ultimo=app.condicional.borrow_mut().formato(&app.workbook,0,2,0).unwrap();
        assert_eq!(ultimo.style.unwrap().bg.as_deref(),Some("#ff0000"));
        app.update(Msg::CondicionalTipo(TipoCondicional::Barra));
        app.update(Msg::CondicionalAdicionar);
        let barra=app.condicional.borrow_mut().formato(&app.workbook,0,1,0).unwrap().bar.unwrap();
        assert!(barra.to > barra.from);
        app.workbook.escrever_celula(0,1,0,"25".into());
        let atual=app.condicional.borrow_mut().formato(&app.workbook,0,1,0).unwrap().bar.unwrap();
        assert!(atual.to < barra.to);
    }

    #[test]
    fn condicional_reordenar_apaga_e_recortar_selecao() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"30".into());
        app.workbook.escrever_celula(0,1,0,"30".into());
        app.update(Msg::IrPara("A1:A2".into()));
        app.update(Msg::CondicionalValor("10".into()));
        app.update(Msg::CondicionalCor("#ff0000".into()));
        app.update(Msg::CondicionalAdicionar);
        app.update(Msg::CondicionalCor("#00ff00".into()));
        app.update(Msg::CondicionalAdicionar);
        assert_eq!(app.condicional.borrow_mut().formato(&app.workbook,0,0,0)
            .unwrap().style.unwrap().bg.as_deref(),Some("#ff0000"));
        app.update(Msg::CondicionalMover(0,1));
        assert_eq!(app.condicional.borrow_mut().formato(&app.workbook,0,0,0)
            .unwrap().style.unwrap().bg.as_deref(),Some("#00ff00"));
        app.update(Msg::CondicionalRemover(1));
        assert_eq!(condicional::regras(&app.workbook,0).len(),1);
        app.update(Msg::Selecionar(0,0));
        app.update(Msg::CondicionalLimparSelecao);
        assert!(app.condicional.borrow_mut().formato(&app.workbook,0,0,0).is_none());
        assert!(app.condicional.borrow_mut().formato(&app.workbook,0,1,0).is_some());
        app.update(Msg::CondicionalLimparTudo);
        assert!(condicional::regras(&app.workbook,0).is_empty());
    }

    #[test]
    fn cores_alternadas_e_persistencia_fshz() {
        let mut app=App::default();
        for r in 0..4 {
            app.workbook.escrever_celula(0,r,0,format!("Linha {r}"));
        }
        app.update(Msg::IrPara("A1:A4".into()));
        app.update(Msg::CondicionalCoresAlternadas);
        assert!(app.condicional.borrow_mut().formato(&app.workbook,0,0,0).is_none());
        assert!(app.condicional.borrow_mut().formato(&app.workbook,0,1,0).is_some());
        let data=feathersheet_io::da_workbook::pasta_gravar_bytes(&app.workbook,0).unwrap();
        let decoded=feathersheet_io::fshz_read::ler_fshz(&data).unwrap();
        let wb=feathersheet_io::para_workbook::para_workbook(&decoded);
        assert_eq!(condicional::regras(&wb,0),condicional::regras(&app.workbook,0));
        let mut estado=condicional::Estado::default();
        assert!(estado.formato(&wb,0,1,0).is_some());
    }

    #[test]
    fn atalho_ctrl_shift_v_e_distinto_de_ctrl_v() {
        use crate::compat::keyboard::Key; use crate::compat::keyboard::Modifiers;
        assert_eq!(
            feather_ui::atalhos::combo_de(&Key::Character("v".into()), Modifiers::CTRL | Modifiers::SHIFT).as_deref(),
            Some("Ctrl+Shift+V"),
        );
        assert_eq!(
            feather_ui::atalhos::combo_de(&Key::Character("v".into()), Modifiers::CTRL).as_deref(),
            Some("Ctrl+V"),
        );
    }

    #[test]
    fn realce_da_linha_e_coluna_segue_a_celula_sem_sujar_o_documento() {
        let mut app = App::default();
        assert!(app.config.realce_eixos, "pintar eixos ligado inicialmente");
        app.update(Msg::Selecionar(3, 3)); // D4
        assert_eq!((app.linha, app.coluna), (3, 3));
        assert_eq!(app.endereco, "D4");
        let estilo = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(3, 3);
        let texto = app.workbook.aba(0).unwrap().sheet.get_raw(3, 3);
        let alterado = app.alterado;

        app.update(Msg::RealceEixos(false));
        assert!(!app.config.realce_eixos);
        app.update(Msg::RealceEixos(true));
        assert!(app.config.realce_eixos);
        assert_eq!(app.alterado, alterado, "alternar realce não altera a planilha");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(3, 3), estilo);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 3), texto);

        app.update(Msg::Selecionar(8, 5)); // F9
        assert_eq!(app.endereco, "F9");
        assert_eq!((app.linha, app.coluna), (8, 5));
        assert!(app.config.realce_eixos);
    }

    #[test]
    fn zoom_pela_roda_respeita_ctrl_limites_e_nao_altera_celulas() {
        let mut app = App::default();
        let original = app.workbook.aba(0).unwrap().sheet.get_raw(0, 0).to_owned();
        assert!(feather_ui::icones::existe("zoom-out"));
        assert!(feather_ui::icones::existe("zoom-in"));
        app.update(Msg::Roda(1));
        assert_eq!(app.config.zoom, 100);
        app.update(Msg::ControleZoom(true));
        app.update(Msg::Roda(1));
        // Degraus do rodapé (zoom::PASSOS): 100 → 125.
        assert_eq!(app.config.zoom, 125);
        app.update(Msg::Roda(-1));
        assert_eq!(app.config.zoom, 100);
        app.update(Msg::Zoom(200));
        app.update(Msg::Roda(1));
        assert_eq!(app.config.zoom, 200);
        app.update(Msg::Zoom(50));
        app.update(Msg::Roda(-1));
        assert_eq!(app.config.zoom, 50);
        app.update(Msg::ControleZoom(false));
        app.update(Msg::Roda(1));
        assert_eq!(app.config.zoom, 50);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), original);
    }

    #[test]
    fn zoom_mantem_ultima_linha_no_rodape_e_roda_libera_ancoragem() {
        let mut app = App::default();
        app.update(Msg::Selecionar(MAX_ROWS - 1, 0));
        assert_eq!(app.endereco, "A100000000");
        assert!(app.ancorar_rodape);
        // Sem acesso à janela real no teste unitário, o Canvas recalcula
        // o limite pelo seu tamanho; a App apenas força nova ancoragem.
        app.update(Msg::Zoom(200));
        assert_eq!(app.linha_inicio, MAX_ROWS - 1);
        assert!(app.ancorar_rodape);
        app.update(Msg::RolarNaGrade(MAX_ROWS - 21, 0, 0, false));
        assert!(!app.ancorar_rodape);
        app.update(Msg::Zoom(50));
        assert_eq!(app.linha_inicio, MAX_ROWS - 21);
        app.update(Msg::RolarNaGrade(MAX_ROWS - 20, 0, 0, true));
        assert!(app.ancorar_rodape);
        app.update(Msg::Zoom(100));
        assert_eq!(app.linha_inicio, MAX_ROWS - 1);
        // Ir para o início não deve fazer o zoom regressar ao fim.
        app.update(Msg::Selecionar(0, 0));
        assert!(!app.ancorar_rodape);
        app.update(Msg::Zoom(125));
        assert_eq!(app.linha_inicio, 0);
    }
    #[test]
    fn exportacao_cancelada_nao_altera_estado_da_pasta() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"Nome".into());
        let antes=app.workbook.versao(0);
        let _=app.update(Msg::DestinoDelimitado(None,false));
        assert_eq!(app.workbook.versao(0),antes);
        assert_eq!(app.workbook.valor(0,0,0),Value::Text("Nome".into()));
    }
