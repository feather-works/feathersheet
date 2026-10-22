//! Congelar painéis sem duplicar células e sem alterar o arquivo.
//! Origem rolável e geometria do Canvas usam a mesma quantidade fixada.
use super::*;

impl App {
    pub(super) fn congelamento_atual(&self) -> (u32,u32) {
        self.workbook.aba(self.folha).and_then(|a| self.congelados.get(&a.id))
            .copied().unwrap_or((0,0))
    }

    /// 0 seleção; 1/2 primeira linha/coluna; 3 descongelar;
    /// 4/5 nenhuma ou duas linhas; 6/7 nenhuma ou duas colunas;
    /// 8/9 até linha ou coluna atual, preservando o outro eixo;
    /// 10/11 linha superior / primeira coluna sozinhas (como no Excel).
    /// O limite impede que uma
    /// seleção de milhão de linhas vire um viewport irrecuperável.
    pub(super) fn aplicar_congelamento(&mut self, acao: u8) -> Task<Msg> {
        self.congelar_menu_aberto = false;
        if !self.confirmar() { return Task::none(); }
        let (antes_linhas, antes_colunas) = self.congelamento_atual();
        let (linhas, colunas) = match acao {
            0 => (self.linha, self.coluna),
            1 => (1, antes_colunas),
            2 => (antes_linhas, 1),
            3 => (0, 0),
            4 => (0, antes_colunas),
            5 => (2, antes_colunas),
            6 => (antes_linhas, 0),
            7 => (antes_linhas, 2),
            8 => (self.linha, antes_colunas),
            9 => (antes_linhas, self.coluna),
            10 => (1, 0),
            11 => (0, 1),
            _ => return Task::none(),
        };
        if linhas > 30 || colunas > 16 {
            self.status = tr!("Congelamento muito grande: escolha até 30 linhas e 16 colunas.").into();
            return Task::none();
        }
        let Some(aba) = self.workbook.aba(self.folha) else { return Task::none(); };
        if linhas == 0 && colunas == 0 {
            self.congelados.remove(&aba.id);
            self.status = tr!("Painéis descongelados").into();
        } else {
            self.congelados.insert(aba.id, (linhas, colunas));
            self.status = trf!("Congeladas {linhas} linha(s) e {colunas} coluna(s)");
        }
        self.linha_inicio = linhas;
        self.coluna_inicio = colunas;
        self.ancorar_rodape = false;
        // Visualização não altera células, versão da aba nem flag de salvar.
        Task::none()
    }

    /// Item grande do menu, como no Excel: ícone, título em negrito e descrição.
    fn item_congelar<'a>(&self, icone: &'a str, titulo: &'a str, descricao: &'a str, acao: u8)
        -> Element<'a, Msg> {
        button(row![
            feather_ui::icones::icone(icone, 24.0),
            pelicano::column![
                text(titulo).size(13).font(pelicano::Font { weight: 700, ..pelicano::Font::DEFAULT }),
                text(descricao).size(11.5).color(COR_TEXTO_MUTED()),
            ].spacing(2).width(Length::FILL),
        ].spacing(12).align(pelicano::Align::Center))
            .width(Length::FILL).padding([9, 10])
            .style(estilo_item_menu(false, true))
            .on_press(Msg::CongelarAplicar(acao))
            .into()
    }

    /// Escolhas finas (N linhas / N colunas): uma linha compacta por eixo.
    fn escolhas_finas<'a>(&self, rotulo: &'a str, atual: u32, opcoes: [(&'a str, u8, Option<u32>); 4])
        -> Element<'a, Msg> {
        let mut linha = row![text(rotulo).size(12).color(COR_TEXTO_MUTED()).width(Length::Fixed(58.))]
            .spacing(4).align(pelicano::Align::Center);
        for (nome, codigo, valor) in opcoes {
            linha = linha.push(button(text(nome).size(12).no_wrap())
                .padding([4, 8])
                .style(estilo_item_menu(valor == Some(atual), true))
                .on_press(Msg::CongelarAplicar(codigo)));
        }
        container(linha).padding([2, 10]).into()
    }

    pub(super) fn sobrepor_congelar<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a,Msg> {
        if !self.congelar_menu_aberto { return base; }
        const LARGURA: f32 = 372.;
        const ALTURA: f32 = 330.;
        let (linhas, colunas) = self.congelamento_atual();
        let congelado = linhas > 0 || colunas > 0;
        let (titulo, descricao) = if congelado {
            (tr!("Descongelar painéis"), tr!("Libera todas as linhas e colunas para rolar com o restante da planilha."))
        } else {
            (tr!("Congelar painéis"),
             tr!("Mantém linhas e colunas visíveis enquanto rola o restante, com base na seleção atual."))
        };
        let mut itens = pelicano::column![
            self.item_congelar("table", titulo, descricao, if congelado { 3 } else { 0 }),
            self.item_congelar("header-footer", tr!("Congelar linha superior"),
                tr!("Mantém a linha superior visível ao rolar o restante da planilha."), 10),
            self.item_congelar("layout-sidebar", tr!("Congelar primeira coluna"),
                tr!("Mantém a primeira coluna visível ao rolar o restante da planilha."), 11),
            container(Space::new(Length::FILL, Length::Fixed(1.)))
                .style(|_| pelicano::Style {
                    background: Some(COR_BORDA().into()), ..Default::default() })
                .padding([0, 6]),
            container(text(tr!("Mais opções")).size(11.5).color(COR_TEXTO_MUTED())).padding([4, 10]),
        ].spacing(1);
        itens = itens.push(self.escolhas_finas(tr!("Linhas"), linhas, [
            ("0", 4, Some(0)), ("1", 1, Some(1)), ("2", 5, Some(2)), (tr!("Até a atual"), 8, None)]));
        itens = itens.push(self.escolhas_finas(tr!("Colunas"), colunas, [
            ("0", 6, Some(0)), ("1", 2, Some(1)), ("2", 7, Some(2)), (tr!("Até a atual"), 9, None)]));
        let caixa = container(itens).padding(6).width(Length::Fixed(LARGURA))
            .style(estilo_popup_menu);
        let (px, py) = self.congelar_popup;
        let x = px.clamp(8., (self.tamanho_principal.width - LARGURA - 8.).max(8.));
        let y = py.clamp(8., (self.tamanho_principal.height - ALTURA - 8.).max(8.));
        let menu = pelicano::column![
            Space::new(0.0, Length::Fixed(y)),
            row![Space::new(Length::Fixed(x), 0.0), caixa],
        ].width(Length::FILL).height(Length::FILL);
        let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
            .on_press(Msg::CongelarFechar);
        stack![base, fora, menu].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn congela_sem_alterar_documento_e_guarda_por_id_da_aba() {
        let mut app=App::default();
        app.linha=2; app.coluna=3;
        let versao=app.workbook.versao(0);
        app.aplicar_congelamento(0);
        assert_eq!(app.congelamento_atual(),(2,3));
        assert!(!app.alterado);
        assert_eq!(app.workbook.versao(0),versao);
        app.workbook.criar_aba("Outra");
        app.folha=1;
        assert_eq!(app.congelamento_atual(),(0,0));
        app.folha=0;
        assert_eq!(app.congelamento_atual(),(2,3));
        app.aplicar_congelamento(3);
        assert_eq!(app.congelamento_atual(),(0,0));
    }
    #[test]
    fn menu_legado_congelar_mapeia_ids_e_preserva_o_outro_eixo() {
        let mut app=App::default();
        let evento = |id: &str, args: Vec<serde_json::Value>| app.evento_menu(
            &menu_nativo::Acao { id: id.into(), args }
        );
        assert!(matches!(evento("freeze", vec![serde_json::json!(2), serde_json::Value::Null]),
            Some(Msg::CongelarAplicar(5))));
        assert!(matches!(evento("freeze", vec![serde_json::Value::Null, serde_json::json!(1)]),
            Some(Msg::CongelarAplicar(2))));
        assert!(matches!(evento("freezeCurrentRow", vec![]),
            Some(Msg::CongelarAplicar(8))));
        app.aplicar_congelamento(5);
        app.aplicar_congelamento(2);
        assert_eq!(app.congelamento_atual(),(2,1));
        app.aplicar_congelamento(6);
        assert_eq!(app.congelamento_atual(),(2,0));
    }

    #[test]
    fn evita_congelar_milhao_de_linhas_ao_navegar() {
        let mut app=App::default();
        app.linha=1_000_000;
        app.aplicar_congelamento(0);
        assert_eq!(app.congelamento_atual(),(0,0));
        assert!(app.status.contains("muito grande"));
    }

    #[test]
    fn tres_itens_do_menu_congelam_como_no_excel() {
        let mut app=App::default();
        app.linha=4; app.coluna=2;
        app.aplicar_congelamento(10);
        assert_eq!(app.congelamento_atual(),(1,0));
        app.aplicar_congelamento(11);
        assert_eq!(app.congelamento_atual(),(0,1));
        app.aplicar_congelamento(0);
        assert_eq!(app.congelamento_atual(),(4,2));
        // Com congelamento ativo o primeiro item vira Descongelar.
        app.congelar_menu_aberto = true;
        let _ = app.sobrepor_congelar(pelicano::text("x").into());
        app.aplicar_congelamento(3);
        assert_eq!(app.congelamento_atual(),(0,0));
        assert!(!app.congelar_menu_aberto);
    }
}
