//! Testes de colagem especial, pincel, formatação condicional e viewport.
use super::*;

#[path = "condicional_e_viewport.rs"]
mod condicional_e_viewport;
    #[test]
    fn colar_uma_celula_em_area_repete_conteudo_e_estilo_com_um_desfazer() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=2+3".into());
        let mut cor = EstiloParcial::new();
        cor.insert("bg".into(), Campo::Texto("#113355".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(0, 0), &cor));
        let copiado = app.copiar_selecao().unwrap();

        app.update(Msg::IrPara("C3:E4".into()));
        app.update(Msg::Colar(copiado));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        for r in 2..=3 {
            for c in 2..=4 {
                assert_eq!(sheet.get_raw(r, c), "=2+3");
                assert_eq!(sheet.estilo_resolvido(r, c), sheet.estilo_resolvido(0, 0));
            }
        }
        assert_eq!(app.area_selecionada(), Rect::new(2, 2, 3, 4));
        assert!(app.status.contains("6 células coladas"));
        app.update(Msg::Desfazer);
        for r in 2..=3 {
            for c in 2..=4 {
                assert_eq!(app.workbook.aba(0).unwrap().sheet.snapshot(r, c), None);
            }
        }
        app.update(Msg::Refazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 4), "=2+3");
    }

    #[test]
    fn colar_bloco_em_area_multipla_preserva_posicoes_e_vazios() {
        let mut app = App::default();
        app.update(Msg::Colar("A\tB\nC\t".into()));
        app.update(Msg::IrPara("A1:B2".into()));
        let texto = app.copiar_selecao().unwrap();
        app.update(Msg::IrPara("D4:G7".into()));
        app.update(Msg::Colar(texto));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        for dr in 0..4 {
            for dc in 0..4 {
                let esperado = match (dr % 2, dc % 2) {
                    (0, 0) => "A",
                    (0, 1) => "B",
                    (1, 0) => "C",
                    _ => "",
                };
                assert_eq!(sheet.get_raw(3 + dr, 3 + dc), esperado);
            }
        }
        assert_eq!(app.area_selecionada(), Rect::new(3, 3, 6, 6));
        assert!(app.status.contains("16 células coladas"));
    }

    #[test]
    fn texto_externo_preenche_area_sem_herdar_estilo_da_copia_interna() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "anterior".into());
        let mut estilo = EstiloParcial::new();
        estilo.insert("color".into(), Campo::Texto("#FF0000".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(0, 0), &estilo));
        app.copiar_selecao().unwrap();
        app.update(Msg::IrPara("C3:D4".into()));
        app.update(Msg::Colar("novo".into()));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        for r in 2..=3 {
            for c in 2..=3 {
                assert_eq!(sheet.get_raw(r, c), "novo");
                assert!(sheet.estilo_resolvido(r, c).is_none());
            }
        }
    }

    #[test]
    fn colagem_de_area_grande_aceita_volume_e_respeita_formato_e_limites() {
        let mut app = App::default();
        app.update(Msg::IrPara("C3:E4".into()));
        app.update(Msg::Colar("1\t2\n3\t4".into()));
        assert!(app.status.contains("múltiplas"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.snapshot(2, 2), None);
        app.update(Msg::IrPara("A1:B60000".into()));
        app.update(Msg::Colar("X".into()));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(59999, 1), "X");
        assert!(!app.status.contains("100.000"));
        app.update(Msg::Selecionar(MAX_ROWS - 1, MAX_COLS - 1));
        app.update(Msg::Colar("1\t2\n3\t4".into()));
        assert!(app.status.contains("limite de linhas ou colunas"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.snapshot(MAX_ROWS - 1, MAX_COLS - 1), None);
    }

    #[test]
    fn copiar_e_limpar_mais_de_cem_mil_linhas_preserva_final_e_desfazer() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "inicio".into());
        app.workbook.escrever_celula(0, 100_000, 0, "final".into());
        app.update(Msg::IrPara("A1:A100001".into()));
        let texto = app.copiar_selecao().unwrap();
        assert_eq!(texto.lines().count(), 100_001);
        assert!(texto.ends_with("final"));
        app.update(Msg::Limpar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(100_000, 0), "");
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(100_000, 0), "final");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "inicio");
    }

    #[test]
    fn copiar_colar_estilo_sem_conteudo() {
        let mut app = App::default();
        let mut cor = EstiloParcial::new();
        cor.insert("bg".into(), Campo::Texto("#00FF00".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::new(0, 0, 0, 0), &cor));
        let dados = app.copiar_selecao().unwrap();
        assert!(dados.is_empty());
        let _ = app.update(Msg::Selecionar(1, 1));
        let _ = app.update(Msg::Colar(dados));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(1, 1), "");
        assert_eq!(sheet.estilo_resolvido(1, 1), sheet.estilo_resolvido(0, 0));
    }

    #[test]
    fn salvar_inclui_edicao_na_barra_fx() {
        let mut app = App::default();
        let dir = std::env::temp_dir().join(format!("feathersheet-gui-salvar-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("teste.fshz");
        let _ = app.update(Msg::Editar("42".into()));
        let _ = app.gravar(path.clone());
        app.concluir_gravacao_teste(path.clone(), false);
        assert!(!app.alterado);
        let bytes = std::fs::read(&path).unwrap();
        app.carregar(path.clone(),bytes).unwrap();
        assert_eq!(app.workbook.valor(0,0,0), feathersheet_core::Value::Number(42.));
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn alt_enter_cria_quebra_na_mesma_celula_e_expande_altura() {
        let mut app = App::default();
        app.update(Msg::Editar("Primeira".into()));
        let selecionada = (app.folha, app.linha, app.coluna);
        app.update(Msg::InserirQuebraLinha);
        assert_eq!(app.edicao, "Primeira\n");
        assert_eq!((app.folha, app.linha, app.coluna), selecionada);
        assert!(app.multilinha.is_some());
        // O widget edita o `Content` e só então avisa (sem `Action` no pelicano).
        app.multilinha.as_ref().unwrap().insert("S");
        app.update(Msg::EditarMultilinha);
        assert_eq!(app.edicao, "Primeira\nS");
        app.update(Msg::Confirmar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "Primeira\nS");
        assert!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&0).copied().unwrap_or(28) >= 46);
        assert!(app.multilinha.is_none());
    }

    #[test]
    fn cancelar_edicao_multilinha_mantem_valor_original() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Original".into());
        app.update(Msg::Editar("Texto".into()));
        app.update(Msg::InserirQuebraLinha);
        app.update(Msg::Cancelar);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "Original");
        assert_eq!(app.edicao, "Original");
        assert!(app.multilinha.is_none());
    }

    #[test]
    fn unico_botao_mesclar_alterna_e_undo_refaz_sem_apagar_celulas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Título".into());
        app.workbook.escrever_celula(0, 0, 1, "Valor preservado".into());
        app.update(Msg::Estender(0, 2));
        let area = app.area_selecionada();
        assert_eq!(area, Rect::new(0, 0, 0, 2));
        assert!(!mesclas::exatamente_mesclada(&app.workbook.aba(0).unwrap().sheet, &area));
        app.update(Msg::Mesclar);
        assert!(mesclas::exatamente_mesclada(&app.workbook.aba(0).unwrap().sheet, &area));
        app.update(Msg::Mesclar);
        assert!(!mesclas::exatamente_mesclada(&app.workbook.aba(0).unwrap().sheet, &area));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 1), "Valor preservado");
        app.update(Msg::Desfazer);
        assert!(mesclas::exatamente_mesclada(&app.workbook.aba(0).unwrap().sheet, &area));
        app.update(Msg::Refazer);
        assert!(!mesclas::exatamente_mesclada(&app.workbook.aba(0).unwrap().sheet, &area));
    }

    #[test]
    fn inclinar_texto_alterna_rotacao_e_preserva_conteudo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Texto".into());
        app.update(Msg::InclinarTexto);
        assert_eq!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0, 0).and_then(|estilo| estilo.rotation), Some(45));
        app.update(Msg::InclinarTexto);
        assert_eq!(app.workbook.aba(0).unwrap().sheet
            .estilo_resolvido(0, 0).and_then(|estilo| estilo.rotation), None);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "Texto");
    }

    #[test]
    fn motor_edita_formula_e_cria_abas() {
        let mut app = App::default();
        let _ = app.update(Msg::Editar("10".into()));
        let _ = app.update(Msg::Confirmar);
        assert_eq!(app.workbook.valor(0,0,0), feathersheet_core::Value::Number(10.));
        let _ = app.update(Msg::Selecionar(0,1));
        let _ = app.update(Msg::Editar("=A1*3".into()));
        let _ = app.update(Msg::Confirmar);
        assert_eq!(app.workbook.valor(0,0,1), feathersheet_core::Value::Number(30.));
        let _ = app.update(Msg::AdicionarAba);
        assert_eq!(app.workbook.len(), 2);
        assert_eq!(app.folha, 1);
    }

    #[test]
    fn colar_intervalo_e_desfazer_uma_vez() {
        let mut app = App::default();
        let _=app.update(Msg::Colar("2\t=A1*4\n3\t7".into()));
        assert_eq!(app.workbook.valor(0,0,0), feathersheet_core::Value::Number(2.));
        assert_eq!(app.workbook.valor(0,0,1), feathersheet_core::Value::Number(8.));
        assert_eq!(app.workbook.valor(0,1,0), feathersheet_core::Value::Number(3.));
        let _=app.update(Msg::Desfazer);
        assert_eq!(app.workbook.valor(0,0,0), feathersheet_core::Value::Empty);
        assert_eq!(app.workbook.valor(0,0,1), feathersheet_core::Value::Empty);
        assert_eq!(app.workbook.valor(0,1,0), feathersheet_core::Value::Empty);
        let _=app.update(Msg::Refazer);
        assert_eq!(app.workbook.valor(0,0,1), feathersheet_core::Value::Number(8.));
    }
    #[test]
    fn estilo_e_estrutura_usam_motor_rust() {
        let mut app=App::default();
        let _=app.update(Msg::Editar("12".into()));
        let _=app.update(Msg::Confirmar);
        let _=app.update(Msg::Formatar(Formatacao::Moeda));
        assert_eq!(
            app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0,0).unwrap().format,
            NumberFormat::Currency
        );
        let _=app.update(Msg::Estrutura(Estrutura::InserirLinha));
        assert_eq!(app.workbook.valor(0,1,0),feathersheet_core::Value::Number(12.));
        let _=app.update(Msg::EditarNomeAba);
        let _=app.update(Msg::NomearAba("Resultados".into()));
        let _=app.update(Msg::RenomearAba);
        assert_eq!(app.workbook.nomes(),vec!["Resultados".to_string()]);
    }

    #[test]
    fn xlsx_rust_exporta_e_reabre_sem_typescript() {
        let mut app=App::default();
        app.workbook.escrever_celula(0,0,0,"5".into());
        app.workbook.escrever_celula(0,0,1,"=A1*4".into());
        let bytes=feathersheet_io::xlsx_write::gerar_xlsx(&app.workbook).unwrap();
        assert!(bytes.starts_with(b"PK"));
        app.carregar(PathBuf::from("arquivo.xlsx"),bytes).unwrap();
        assert_eq!(app.workbook.valor(0,0,0),feathersheet_core::Value::Number(5.));
        assert_eq!(app.workbook.valor(0,0,1),feathersheet_core::Value::Number(20.));
    }

    #[test]
    fn fshz_nativo_volta_sem_perder_formula() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "7".into());
        app.workbook.escrever_celula(0, 0, 1, "=A1+5".into());
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&app.workbook, 0).unwrap();
        let path = PathBuf::from("arquivo.fshz");
        app.carregar(path, bytes).unwrap();
        assert_eq!(app.workbook.valor(0,0,1), feathersheet_core::Value::Number(12.));
    }

    #[test]
    fn colar_valores_de_formula_copia_resultado_sem_formato_e_undo_unico() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=2+3".into());
        let mut estilo_origem = EstiloParcial::new();
        estilo_origem.insert("bg".into(), Campo::Texto("#FFCC00".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(0, 0), &estilo_origem));
        app.workbook.escrever_celula(0, 2, 1, "99".into());
        let mut estilo_destino = EstiloParcial::new();
        estilo_destino.insert("color".into(), Campo::Texto("#123456".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(2, 1), &estilo_destino));
        let original = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 1);
        let copia = app.copiar_selecao().unwrap();
        app.update(Msg::Selecionar(2, 1));
        app.update(Msg::ColarValores(copia));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 1), "5");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 1), original);
        assert!(app.status.contains("somente valores"));
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 1), "99");
        app.update(Msg::Refazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 1), "5");
    }

    #[test]
    fn colar_transposta_inverte_dimensoes_preserva_estilos_e_desfaz_uma_vez() {
        let mut app = App::default();
        app.update(Msg::Colar("A\tB\tC\nD\tE\tF".into()));
        let mut cor = EstiloParcial::new();
        cor.insert("bg".into(), Campo::Texto("#123456".into()));
        app.workbook.editar(0, |s| s.set_style(&Rect::cell(0, 1), &cor));
        app.update(Msg::IrPara("A1:C2".into()));
        let dados = app.copiar_selecao().unwrap();
        app.update(Msg::Selecionar(3, 4));
        app.update(Msg::ColarEspecial(dados, ModoColagem::Transposta));
        let s = &app.workbook.aba(0).unwrap().sheet;
        for (r, c, valor) in [
            (3, 4, "A"), (4, 4, "B"), (5, 4, "C"),
            (3, 5, "D"), (4, 5, "E"), (5, 5, "F"),
        ] {
            assert_eq!(s.get_raw(r, c), valor);
        }
        assert_eq!(s.estilo_resolvido(4, 4), s.estilo_resolvido(0, 1));
        assert_eq!(app.area_selecionada(), Rect::new(3, 4, 5, 5));
        app.update(Msg::Desfazer);
        assert!(app.workbook.aba(0).unwrap().sheet.snapshot(4, 4).is_none());
        assert!(app.workbook.aba(0).unwrap().sheet.snapshot(5, 5).is_none());
    }

    #[test]
    fn colar_transposta_texto_externo_e_mosaico() {
        let mut app = App::default();
        app.update(Msg::IrPara("C3:D6".into()));
        app.update(Msg::ColarEspecial("1\t2\n3\t4".into(), ModoColagem::Transposta));
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(2, 2), "1");
        assert_eq!(s.get_raw(2, 3), "3");
        assert_eq!(s.get_raw(3, 2), "2");
        assert_eq!(s.get_raw(3, 3), "4");
        assert_eq!(s.get_raw(4, 2), "1");
        assert_eq!(s.get_raw(5, 3), "4");
    }

    #[test]
    fn colar_valores_com_formato_congela_formula_e_copia_estilo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=2+3".into());
        let mut origem = EstiloParcial::new();
        origem.insert("bg".into(), Campo::Texto("#112233".into()));
        app.workbook.editar(0, |s| s.set_style(&Rect::cell(0, 0), &origem));
        let dados = app.copiar_selecao().unwrap();
        app.workbook.escrever_celula(0, 2, 2, "anterior".into());
        app.update(Msg::Selecionar(2, 2));
        app.update(Msg::ColarEspecial(dados, ModoColagem::ValoresComFormato));
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(2, 2), "5");
        assert_eq!(s.estilo_resolvido(2, 2), s.estilo_resolvido(0, 0));
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 2), "anterior");
    }

    #[test]
    fn colar_formulas_sem_formato_preserva_destino_e_padrao_copia_formato() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=2+3".into());
        let mut origem = EstiloParcial::new();
        origem.insert("bg".into(), Campo::Texto("#123456".into()));
        app.workbook.editar(0, |s| s.set_style(&Rect::cell(0, 0), &origem));
        let dados = app.copiar_selecao().unwrap();
        let mut destino = EstiloParcial::new();
        destino.insert("color".into(), Campo::Texto("#654321".into()));
        app.workbook.editar(0, |s| s.set_style(&Rect::cell(2, 2), &destino));
        let estilo_original = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 2);
        app.update(Msg::Selecionar(2, 2));
        app.update(Msg::ColarEspecial(dados.clone(), ModoColagem::FormulasSemFormato));
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(2, 2), "=2+3");
        assert_eq!(s.estilo_resolvido(2, 2), estilo_original);
        app.update(Msg::Selecionar(3, 3));
        app.update(Msg::ColarEspecial(dados, ModoColagem::Completa));
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(3, 3), "=2+3");
        assert_eq!(s.estilo_resolvido(3, 3), s.estilo_resolvido(0, 0));
    }

    #[test]
    fn colagem_valores_com_formato_rejeita_clipboard_externo_sem_tocar_destino() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "origem".into());
        app.copiar_selecao().unwrap();
        app.update(Msg::Selecionar(2, 2));
        app.update(Msg::ColarEspecial("externo".into(), ModoColagem::ValoresComFormato));
        assert!(app.status.contains("requer células copiadas"));
        assert!(app.workbook.aba(0).unwrap().sheet.snapshot(2, 2).is_none());
    }

    #[test]
    fn pincel_aplica_estilo_sem_tocar_em_formula_e_desfaz_em_um_passo() {
        let mut app = App::default();
        let mut estilo = EstiloParcial::new();
        estilo.insert("bold".into(), Campo::Logico(true));
        estilo.insert("bg".into(), Campo::Texto("#AABBCC".into()));
        estilo.insert("format".into(), Campo::Texto("currency".into()));
        app.workbook.editar(0, |s| s.set_style(&Rect::cell(0, 0), &estilo));
        app.workbook.escrever_celula(0, 3, 4, "=2+3".into());
        let original = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(3, 4);
        app.update(Msg::Pincel);
        assert!(app.pincel_formatacao.is_some());
        app.update(Msg::Selecionar(3, 4));
        app.update(Msg::AplicarPincel);
        assert!(app.pincel_formatacao.is_none());
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(3, 4), "=2+3");
        assert_eq!(s.estilo_resolvido(3, 4), s.estilo_resolvido(0, 0));
        app.update(Msg::Desfazer);
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(3, 4), "=2+3");
        assert_eq!(s.estilo_resolvido(3, 4), original);
        app.update(Msg::Refazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(3, 4),
            app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0));
    }

    #[test]
    fn pincel_reproduz_padrao_de_estilos_com_arrasto_e_sem_copiar_valores() {
        let mut app = App::default();
        for dr in 0..2_u32 {
            for dc in 0..2_u32 {
                app.workbook.escrever_celula(0, dr, dc, format!("src-{dr}-{dc}"));
                let mut estilo = EstiloParcial::new();
                estilo.insert("color".into(), Campo::Texto(
                    format!("#{:06X}", dr * 2 + dc + 0x220000)
                ));
                app.workbook.editar(0, |s|
                    s.set_style(&Rect::cell(dr, dc), &estilo));
            }
        }
        app.update(Msg::IrPara("A1:B2".into()));
        app.update(Msg::Pincel);
        app.workbook.escrever_celula(0, 6, 6, "destino".into());
        app.update(Msg::Selecionar(6, 6));
        app.update(Msg::Estender(9, 9));
        app.update(Msg::AplicarPincel);
        for dr in 0..4_u32 {
            for dc in 0..4_u32 {
                let s = &app.workbook.aba(0).unwrap().sheet;
                assert_eq!(s.estilo_resolvido(6 + dr, 6 + dc),
                    s.estilo_resolvido(dr % 2, dc % 2));
                assert_eq!(s.get_raw(6 + dr, 6 + dc),
                    if dr == 0 && dc == 0 { "destino" } else { "" });
            }
        }
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(6, 6), "destino");
        assert!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(6, 6).is_none());
        assert!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(9, 9).is_none());
    }

    #[test]
    fn qa009_cancelar_abertura_e_arquivo_corrompido_nao_apagam_planilha() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 5, 2, "valor original".into());
        app.alterado = true;
        let antes = app.workbook.aba(0).unwrap().sheet.get_raw(5, 2).to_owned();
        let _ = app.update(Msg::Aberto(Ok(None)));
        assert!(app.status.contains("cancelada"));
        assert!(app.alterado);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(5, 2), antes);
        let _ = app.update(Msg::Aberto(Err("Planilha corrompida".into())));
        assert!(app.status.contains("corrompida"));
        assert!(app.alterado);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(5, 2), antes);
    }

    #[test]
    fn pincel_esc_ou_segundo_clique_cancela_sem_alterar_planilha() {
        let mut app = App::default();
        let mut e = EstiloParcial::new();
        e.insert("bold".into(), Campo::Logico(true));
        app.workbook.editar(0, |s| s.set_style(&Rect::cell(0, 0), &e));
        app.update(Msg::Pincel);
        assert!(app.pincel_formatacao.is_some());
        app.update(Msg::Cancelar);
        assert!(app.pincel_formatacao.is_none());
        app.update(Msg::Selecionar(1, 1));
        app.update(Msg::AplicarPincel);
        assert!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(1, 1).is_none());
        app.update(Msg::Pincel);
        app.update(Msg::Pincel);
        assert!(app.pincel_formatacao.is_none());
    }

    #[test]
    fn pincel_valida_bloco_antes_de_aplicar_e_preserva_estado_para_tentar_novamente() {
        let mut app = App::default();
        app.update(Msg::IrPara("A1:B2".into()));
        app.update(Msg::Pincel);
        app.update(Msg::IrPara("D4:F5".into()));
        app.update(Msg::AplicarPincel);
        assert!(app.status.contains("múltiplas"));
        assert!(app.pincel_formatacao.is_some());
        assert!(app.workbook.aba(0).unwrap().sheet.snapshot(3, 3).is_none());
        app.update(Msg::IrPara("D4:E5".into()));
        app.update(Msg::AplicarPincel);
        assert!(app.pincel_formatacao.is_none());
    }

    #[test]
    fn pincel_aceita_mais_de_cem_mil_celulas_e_respeita_limites() {
        let mut app = App::default();
        app.update(Msg::IrPara("A1:Z5000".into()));
        app.update(Msg::Pincel);
        assert!(app.pincel_formatacao.is_some());
        assert!(!app.status.contains("100.000"));
        app.update(Msg::Pincel); // desativa antes de capturar a próxima seleção
        app.update(Msg::Selecionar(0, 0));
        app.update(Msg::Pincel);
        assert!(app.pincel_formatacao.is_some());
        app.update(Msg::Selecionar(MAX_ROWS - 1, MAX_COLS - 1));
        app.update(Msg::AplicarPincel);
        assert!(app.pincel_formatacao.is_none());
    }

    #[test]
    fn colar_dados_expande_tabela_mas_somente_formato_nao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Produto".into());
        app.workbook.escrever_celula(0, 1, 0, "Caneta".into());
        tabelas::criar(&mut app.workbook, 0, Rect::new(0, 0, 1, 0), "TabelaColagem", true, false).unwrap();

        app.update(Msg::Selecionar(2, 0));
        app.update(Msg::Colar("Caderno\nBorracha".into()));
        let tabela = tabelas::sob_id(&app.workbook, 0, "tab_TabelaColagem").unwrap();
        assert_eq!(tabela["area"]["r1"], 3);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 0), "Borracha");

        app.update(Msg::Selecionar(0, 0));
        let copiado = app.copiar_selecao().unwrap();
        app.update(Msg::Selecionar(4, 0));
        app.update(Msg::ColarFormato(copiado));
        let tabela = tabelas::sob_id(&app.workbook, 0, "tab_TabelaColagem").unwrap();
        assert_eq!(tabela["area"]["r1"], 3);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(4, 0), "");
    }

    #[test]
    fn colar_somente_formatacao_nao_altera_valores_nem_formulas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "origem".into());
        let mut fonte = EstiloParcial::new();
        fonte.insert("bold".into(), Campo::Logico(true));
        fonte.insert("bg".into(), Campo::Texto("#00FF00".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(0, 0), &fonte));
        let estilo = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0);
        app.workbook.escrever_celula(0, 2, 2, "=2*4".into());
        let copia = app.copiar_selecao().unwrap();
        app.update(Msg::Selecionar(2, 2));
        app.update(Msg::ColarFormato(copia));
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(2, 2), "=2*4");
        assert_eq!(sheet.estilo_resolvido(2, 2), estilo);
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(2, 2), "=2*4");
        assert!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 2).is_none());
        app.update(Msg::Refazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 2), estilo);
    }

    #[test]
    fn ctrl_shift_v_preenche_area_repetindo_valores_preservando_estilos_do_destino() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=10+5".into());
        let copia = app.copiar_selecao().unwrap();
        let mut cor = EstiloParcial::new();
        cor.insert("color".into(), Campo::Texto("#0088BB".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(2, 3), &cor));
        let estilo_d4 = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 3);
        app.update(Msg::IrPara("C3:E4".into()));
        app.update(Msg::ColarValores(copia));
        for row in 2..=3 {
            for col in 2..=4 {
                assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(row, col), "15");
            }
        }
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 3), estilo_d4);
        app.update(Msg::Desfazer);
        for row in 2..=3 {
            for col in 2..=4 {
                assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(row, col), "");
            }
        }
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 3), estilo_d4);
    }

    #[test]
    fn colar_valores_texto_externo_respeita_selecao_e_formato_preexistente() {
        let mut app = App::default();
        let mut cor = EstiloParcial::new();
        cor.insert("color".into(), Campo::Texto("#ABCDEF".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(2, 3), &cor));
        let antes = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 3);
        app.update(Msg::IrPara("C3:D4".into()));
        app.update(Msg::ColarValores("1\t2\n3\t4".into()));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 3), "4");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 3), antes);
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 3), "");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(2, 3), antes);
    }

    #[test]
    fn formato_sem_copia_interna_recusa_sem_alterar_destino() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "original".into());
        app.update(Msg::ColarFormato("texto externo".into()));
        assert!(app.status.contains("requer células copiadas"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "original");
    }

    #[test]
    fn colagem_especial_nao_aceita_area_incompativel() {
        let mut app = App::default();
        app.update(Msg::IrPara("C3:E4".into()));
        app.update(Msg::ColarValores("1\t2\n3\t4".into()));
        assert!(app.status.contains("múltiplas"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.snapshot(2, 2), None);
    }

#[cfg(test)]
mod testes_nao_disp {
    use super::*;
    use feathersheet_core::celulas::values::{ErrorCode, Value};

    /// O caminho de quem digita: Editar + Confirmar, como na barra de fórmulas.
    fn digitar(app: &mut App, endereco: &str, texto: &str) -> Value {
        app.update(Msg::IrPara(endereco.into()));
        app.update(Msg::Editar(texto.into()));
        app.update(Msg::Confirmar);
        app.update(Msg::IrPara(endereco.into()));
        let (r, c) = (app.linha, app.coluna);
        app.workbook.aba(0).unwrap().sheet.get_value(r, c)
    }

    #[test]
    fn nao_disp_e_n_d_em_portugues() {
        let mut app = App::default();
        // A1 vazio: o SE escolhe o terceiro argumento.
        assert_eq!(digitar(&mut app, "B1", "=SE(A1;#N/A;0)"), Value::Number(0.0));
        assert!(matches!(digitar(&mut app, "B2", "=SE(1;#N/A;0)"), Value::Error(e) if e.code == ErrorCode::Na));
        assert!(matches!(digitar(&mut app, "B3", "=SE(1;#N/D;0)"), Value::Error(e) if e.code == ErrorCode::Na));
        assert!(matches!(digitar(&mut app, "B4", "=NÃO.DISP()"), Value::Error(e) if e.code == ErrorCode::Na));
        assert_eq!(digitar(&mut app, "B5", "=É.NÃO.DISP(B4)"), Value::Bool(true));
        assert_eq!(digitar(&mut app, "B6", "=ÉNÃO.DISP(#N/D)"), Value::Bool(true));
    }
}

#[cfg(test)]
mod testes_matriz_dinamica {
    use super::*;

    #[test]
    fn copiar_pedaco_da_matriz_leva_valores_e_inteira_leva_a_formula() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "=SEQUÊNCIA(3)".into());
        // Só A2:A3 (sem a âncora): valores.
        app.update(Msg::IrPara("A2:A3".into()));
        assert_eq!(app.copiar_selecao().as_deref(), Some("2\n3"));
        // A1:A3 inteira: a fórmula na âncora, vizinhas vazias (derramam de novo).
        app.update(Msg::IrPara("A1:A3".into()));
        assert_eq!(app.copiar_selecao().as_deref(), Some("=SEQUÊNCIA(3)\n\n"));
    }

    #[test]
    fn digitar_na_area_derramada_da_spill() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "={1;2;3}".into());
        app.update(Msg::IrPara("A2".into()));
        app.update(Msg::Editar("x".into()));
        app.update(Msg::Confirmar);
        let v = app.workbook.valor(0, 0, 0);
        assert!(matches!(v, feathersheet_core::Value::Error(e) if e.code == feathersheet_core::celulas::values::ErrorCode::Spill));
    }

    #[test]
    fn delete_apaga_so_o_conteudo_e_mantem_a_formatacao() {
        let mut app = App::default();
        let _ = app.update(Msg::Editar("12".into()));
        let _ = app.update(Msg::Confirmar);
        let _ = app.update(Msg::Formatar(Formatacao::Moeda));
        let mut est = EstiloParcial::new();
        est.insert("bg".into(), Campo::Texto("#113355".into()));
        est.insert("borderTop".into(), Campo::Texto("thin #000000".into()));
        app.workbook.editar(0, |sheet| sheet.set_style(&Rect::cell(0, 0), &est));
        let estilo = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0);
        let direto = app.workbook.aba(0).unwrap().sheet.snapshot(0, 0).unwrap().style;

        let _ = app.update(Msg::Limpar);
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(0, 0), "", "valor some");
        assert_eq!(sheet.estilo_resolvido(0, 0), estilo, "formatação fica");
        assert_eq!(sheet.snapshot(0, 0).unwrap().style, direto);

        // Um Ctrl+Z devolve o valor (com a formatação).
        app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0, 0), "12");
        assert_eq!(app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0), estilo);

        // Limpar tudo continua limpando tudo.
        let _ = app.update(Msg::LimparTudo);
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.get_raw(0, 0), "");
        assert_eq!(sheet.snapshot(0, 0), None);
    }
}
