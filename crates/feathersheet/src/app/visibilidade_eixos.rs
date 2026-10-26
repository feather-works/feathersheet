//! Ocultar/Reexibir linhas e colunas migrado do legado `ui/app/estrutura.ts`.
//! Tamanho zero é o mesmo estado usado pelo filtro e pela grade virtualizada.
use super::*;

impl App {
    /// Rola apenas pelas colunas que aparecem no Canvas, sem parar em
    /// colunas ocultas. Nao refaz o indice a cada roda: EstadoEixos reutiliza
    /// o cache esparso do desenho (invalidado pela versao da aba).
    pub(super) fn rolar_colunas_visiveis(&mut self, delta: i32) {
        if delta == 0 { return; }
        let (_, congeladas) = self.congelamento_atual();
        self.coluna_inicio = self.eixos_grade.borrow_mut().coluna_apos_rolar(
            &self.workbook, self.folha, self.coluna_inicio, congeladas, delta,
        );
    }

    /// `coluna = true` atua na largura; `false` na altura.
    pub(super) fn alterar_visibilidade_eixos(&mut self, coluna: bool, ocultar: bool) {
        if !self.confirmar() { return; }
        let area = self.area_selecionada().normalized();
        let (a, b) = if coluna { (area.c0, area.c1) } else { (area.r0, area.r1) };
        let mut resultado = Ok(0usize);
        if self.workbook.editar(self.folha, |sheet| {
            resultado = sheet.alterar_visibilidade_dimensoes(coluna, ocultar, a, b);
        }).is_none() {
            self.status = tr!("A planilha não está disponível.").into();
            return;
        }
        match resultado {
            Ok(0) => self.status = if ocultar {
                tr!("As dimensões selecionadas já estão ocultas.").into()
            } else {
                tr!("Não há linhas ou colunas ocultas nessa região.").into()
            },
            Ok(quantidade) => {
                self.alterado = true;
                if ocultar {
                    // A célula ativa pode ter acabado de ficar invisível.
                    // Selecionar o próximo eixo visível, sem alterar conteúdo.
                    let atual = if coluna { self.coluna } else { self.linha };
                    if (a..=b).contains(&atual) {
                        let proxima = self.eixos_grade.borrow_mut()
                            .visivel_apos_ocultar(
                                &self.workbook, self.folha, coluna, a, b,
                            );
                        if let Some(indice) = proxima {
                            if coluna {
                                self.trocar_selecao(self.linha, indice);
                            } else {
                                self.trocar_selecao(indice, self.coluna);
                            }
                        }
                    }
                }
                let eixo = if coluna { "coluna" } else { "linha" };
                let plural = if quantidade == 1 { "" } else { "s" };
                let verbo = if ocultar { "ocultada" } else { "reexibida" };
                self.status = format!("{quantidade} {eixo}{plural} {verbo}{plural}. Ctrl+Z desfaz.");
            },
            Err(erro) => self.status = erro,
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    #[test]
    fn app_rola_horizontalmente_sem_giros_desperdicados_em_colunas_ocultas() {
        let mut app = App::default();
        app.workbook.editar(0, |aba| {
            aba.col_widths.insert(2, 0);
            aba.col_widths.insert(3, 0);
            aba.col_widths.insert(4, 0);
        });
        app.coluna_inicio = 1;
        app.rolar_colunas_visiveis(1);
        assert_eq!(app.coluna_inicio, 5);
        app.rolar_colunas_visiveis(-1);
        assert_eq!(app.coluna_inicio, 1);
        app.rolar_colunas_visiveis(2);
        assert_eq!(app.coluna_inicio, 6);
    }

    #[test]
    fn mensagens_reais_de_roda_e_rolagem_usam_colunas_visiveis() {
        let mut app = App::default();
        app.workbook.editar(0, |aba| {
            for c in 2..=5 { aba.col_widths.insert(c, 0); }
        });
        app.coluna_inicio = 1;
        app.update(Msg::RolarNaGrade(0, app.coluna_inicio, 1, false));
        assert_eq!(app.coluna_inicio, 6);
        app.update(Msg::Rolar(0, 2));
        assert_eq!(app.coluna_inicio, 8);
        app.update(Msg::RolarNaGrade(0, app.coluna_inicio, -1, false));
        assert_eq!(app.coluna_inicio, 7);
        app.update(Msg::Rolar(0, -2));
        assert_eq!(app.coluna_inicio, 1);
    }

    #[test]
    fn menu_nativo_oculta_linhas_e_reexibe_sem_perder_valores() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 4, 1, "42".into());
        app.linha = 4;
        app.coluna = 0;
        app.fim_selecao = (6, MAX_COLS - 1);
        app.alterar_visibilidade_eixos(false, true);
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.row_heights.get(&4), Some(&0));
        assert_eq!(sheet.row_heights.get(&6), Some(&0));
        assert_eq!(app.workbook.valor(0, 4, 1), Value::Number(42.0));

        // Selecionar a linha seguinte reexibe também o bloco oculto adjacente.
        app.linha = 7;
        app.coluna = 0;
        app.fim_selecao = (7, MAX_COLS - 1);
        app.alterar_visibilidade_eixos(false, false);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&4), None);
        assert_eq!(app.workbook.valor(0, 4, 1), Value::Number(42.0));
    }

    #[test]
    fn ocultar_colunas_por_contexto_e_desfazer_em_um_passo() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 2, "texto".into());
        app.linha = 0;
        app.coluna = 2;
        app.fim_selecao = (MAX_ROWS - 1, 3);
        let opcoes = app.opcoes_contexto(Contexto::Coluna(2));
        assert!(opcoes.iter().any(|v| v.as_ref()
            .is_some_and(|(nome, _)| nome == "Ocultar colunas")));
        app.alterar_visibilidade_eixos(true, true);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.col_widths.get(&3), Some(&0));
        app.historico(false);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.col_widths.get(&2), None);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.col_widths.get(&3), None);
        assert_eq!(app.workbook.valor(0, 0, 2), Value::Text("texto".into()));
    }
}
