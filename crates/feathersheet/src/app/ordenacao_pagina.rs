//! Dados → Classificar página (migração do sort('sheet') do Ribbon TS).
//! A primeira linha permanece como cabeçalho; a linha inteira se move,
//! preservando valores, fórmulas, estilos e o histórico do motor Rust.
use super::*;
use feathersheet_core::operacoes::ops;

impl App {
    /// Ordena a área realmente utilizada, nunca a grade virtual inteira.
    pub(super) fn ordenar_pagina(&mut self, crescente: bool) {
        if !self.confirmar() { return; }
        let Some(aba) = self.workbook.aba(self.folha) else { return; };
        if aba.sheet.is_empty() {
            self.status = tr!("A página está vazia; nada para classificar.").into();
            return;
        }
        let usada = aba.sheet.used_rect();
        if self.coluna < usada.c0 || self.coluna > usada.c1 {
            self.status = tr!("Selecione uma coluna com dados para classificar a página.").into();
            return;
        }
        let linhas = u64::from(usada.r1) + 1;
        let colunas = u64::from(usada.c1) + 1;
        // A operação do motor ainda é síncrona: limites explícitos evitam
        // congelar a janela ao tentar ordenar milhões de linhas de uma vez.
        const MAX_LINHAS: u64 = 250_000;
        const MAX_CELULAS: u64 = 3_000_000;
        if linhas > MAX_LINHAS || linhas.saturating_mul(colunas) > MAX_CELULAS {
            self.status = trf!(
                "Classificação interrompida: {linhas} linhas × {colunas} colunas excedem o limite seguro (250.000 linhas / 3 milhões de células)."
            );
            return;
        }

        let area = Rect::new(0, 0, usada.r1, usada.c1);
        let chave = [ops::SortKey { col: self.coluna, ascending: crescente }];
        let plano = ops::planejar_ordenacao(area.clone(), &chave, true, usada.r1,
            |r, c| self.workbook.valor(self.folha, r, c));
        if let Some(plano) = plano {
            self.workbook.editar(self.folha,
                |sheet| ops::aplicar_ordenacao(sheet, area.c0, area.c1, &plano));
            self.alterado = true;
            self.edicao = self.workbook.aba(self.folha)
                .map(|a| a.sheet.get_raw(self.linha, self.coluna))
                .unwrap_or_default();
            self.status = trf!(
                "Página classificada {} pela coluna {} (cabeçalho preservado).",
                if crescente { tr!("A → Z") } else { tr!("Z → A") },
                feathersheet_core::celulas::refs::col_name(self.coluna)
            );
        } else {
            self.status = tr!("A página já está classificada ou possui apenas o cabeçalho.").into();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn classificar_pagina_preserva_cabecalho_colunas_e_desfazer() {
        let mut app = App::default();
        for (r, nome, qtd) in [(0, "Nome", "Qtd"), (1, "Z", "10"),
                              (2, "A", "20"), (3, "M", "30")] {
            app.workbook.escrever_celula(0, r, 0, nome.into());
            app.workbook.escrever_celula(0, r, 1, qtd.into());
        }
        app.trocar_selecao(2, 0);
        // Mesmo com apenas uma célula selecionada, ordena a aba usada.
        app.update(Msg::FaixaOpcao("ordenar", "Página A → Z"));
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(0, 0), "Nome");
        assert_eq!(s.get_raw(1, 0), "A");
        assert_eq!(s.get_raw(1, 1), "20");
        assert_eq!(s.get_raw(2, 0), "M");
        assert_eq!(s.get_raw(2, 1), "30");
        assert_eq!(s.get_raw(3, 0), "Z");
        assert_eq!(s.get_raw(3, 1), "10");
        assert!(app.alterado);
        app.update(Msg::Desfazer);
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(1, 0), "Z");
        assert_eq!(s.get_raw(2, 0), "A");
    }

    #[test]
    fn classificar_pagina_usa_coluna_ativa_no_sentido_descendente() {
        let mut app = App::default();
        for (r, nome, qtd) in [(0, "Nome", "Qtd"), (1, "Z", "10"),
                              (2, "A", "30"), (3, "M", "20")] {
            app.workbook.escrever_celula(0, r, 0, nome.into());
            app.workbook.escrever_celula(0, r, 1, qtd.into());
        }
        app.trocar_selecao(1, 1);
        app.update(Msg::FaixaOpcao("ordenar", "Página Z → A"));
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(s.get_raw(0, 1), "Qtd");
        assert_eq!(s.get_raw(1, 0), "A");
        assert_eq!(s.get_raw(1, 1), "30");
        assert_eq!(s.get_raw(3, 0), "Z");
        assert_eq!(s.get_raw(3, 1), "10");
    }

    #[test]
    fn classificar_pagina_muito_grande_recusa_sem_alterar() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "Nome".into());
        app.workbook.escrever_celula(0, 250_000, 0, "Muito longe".into());
        app.alterado = false;
        app.update(Msg::OrdenarPagina(true));
        assert!(app.status.contains("limite seguro"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(250_000, 0), "Muito longe");
        assert!(!app.alterado);
    }
}
