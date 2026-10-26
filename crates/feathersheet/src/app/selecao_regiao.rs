//! Seleção de região atual: conecta a operação Rust já portada do TS.
use super::*;
use feathersheet_core::operacoes::ops;

impl App {
    /// Uma ação só altera âncora, intervalo e viewport; nunca o Workbook.
    pub(super) fn selecionar_regiao_atual(&mut self) {
        self.edicao_menu = None;
        if !self.confirmar() { return; }
        let selecao = self.area_selecionada();
        let Some(aba) = self.workbook.aba(self.folha) else { return; };
        if aba.sheet.is_empty() {
            self.status = tr!("Não há dados para selecionar nesta aba.").into();
            return;
        }
        let usada = aba.sheet.used_rect();
        // Sem essa guarda, o motor trunca a posição de uma célula vazia
        // abaixo/à direita da região usada e salta para outro endereço.
        if selecao.r0 > usada.r1 || selecao.c0 > usada.c1 {
            self.status = tr!("Não há região de dados na seleção atual.").into();
            return;
        }
        let regiao = ops::regiao_atual(&aba.sheet, selecao);
        let regiao = mesclas::expandir(&mesclas::ler(&aba.sheet), regiao);
        let endereco = endereco_area(&regiao);
        self.imagem_selecionada = None;
        self.arraste_imagem = None;
        self.trocar_selecao(regiao.r0, regiao.c0);
        self.fim_selecao = (regiao.r1, regiao.c1);
        self.endereco = endereco.clone();
        self.status = trf!("Região atual selecionada: {endereco}");
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn seleciona_tabela_conectada_sem_incluir_coluna_isolada() {
        let mut app = App::default();
        for (r,c,v) in [
            (0,0,"Nome"), (0,1,"Valor"), (1,0,"A"), (1,1,"2"),
            (2,0,"B"), (2,1,"3"), (1,6,"fora"),
        ] { app.workbook.escrever_celula(0, r, c, v.into()); }
        let versao = app.workbook.aba(0).unwrap().versao;
        app.trocar_selecao(1, 1);
        app.update(Msg::SelecionarRegiaoAtual);
        assert_eq!(app.area_selecionada(), Rect::new(0, 0, 2, 1));
        assert_eq!(app.endereco, "A1:B3");
        assert_eq!(app.workbook.aba(0).unwrap().versao, versao);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 6), "fora");
        assert!(!app.alterado);
    }

    #[test]
    fn coluna_inteira_cortada_no_limite_usado_sem_varrer_grade_virtual() {
        let mut app = App::default();
        for (r,c,v) in [(0,0,"x"),(0,1,"y"),(1,0,"a"),(1,1,"b"),(2,1,"c")] {
            app.workbook.escrever_celula(0, r, c, v.into());
        }
        app.update(Msg::SelecionarColuna(1));
        app.update(Msg::SelecionarRegiaoAtual);
        assert_eq!(app.area_selecionada(), Rect::new(0, 0, 2, 1));
        assert!(!app.alterado);
    }

    #[test]
    fn nao_salta_para_dados_se_o_cursor_esta_fora_da_area_usada() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "dado".into());
        app.trocar_selecao(100, 12);
        app.update(Msg::SelecionarRegiaoAtual);
        assert_eq!((app.linha, app.coluna, app.fim_selecao), (100, 12, (100, 12)));
        assert!(app.status.contains("Não há região"));
        assert!(!app.alterado);
    }

    #[test]
    fn aba_vazia_preserva_a_selecao_sem_alterar_o_documento() {
        let mut app = App::default();
        app.trocar_selecao(14, 7);
        app.update(Msg::SelecionarRegiaoAtual);
        assert_eq!(app.area_selecionada(), Rect::new(14, 7, 14, 7));
        assert!(app.status.contains("Não há dados"));
        assert!(!app.alterado);
    }
}
