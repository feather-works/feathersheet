//! Controles avançados da tabela dinâmica nativa (filtros, layout e calculados).
use super::*;
use pelicano::column;
use feathersheet_core::dinamica::campo_calculado;

impl App {
    pub(super) fn adicionar_campo_calculado(&mut self) {
        let Some(e) = self.dinamica.as_mut() else { return };
        let nome = e.novo_nome.trim().to_owned();
        let formula = e.nova_formula.trim().to_owned();
        if nome.is_empty() || formula.is_empty() {
            self.status = tr!("Informe nome e fórmula para o campo calculado.").into();
            return;
        }
        if e.nomes.iter().any(|existente| existente.eq_ignore_ascii_case(&nome)) {
            self.status = trf!("O campo '{nome}' já existe na tabela dinâmica.");
            return;
        }
        let real = e.nomes.len().saturating_sub(e.calculados.len());
        if (e.origem.c1 as usize) + e.calculados.len() + 1 >= MAX_COLS as usize {
            self.status = tr!("Não existem mais colunas disponíveis para campos calculados.").into();
            return;
        }
        if let Err(msg) = campo_calculado::compilar(&formula, &e.nomes[..real]) {
            self.status = trf!("Fórmula inválida: {msg}");
            return;
        }
        let col = e.origem.c1 as usize + 1 + e.calculados.len();
        e.calculados.push(Calculado { nome: nome.clone(), formula });
        e.nomes.push(nome);
        e.valores.push(MedidaDinamica { coluna: col, op: AgregacaoOp::Soma });
        e.novo_nome.clear();
        e.nova_formula.clear();
        e.previa_calculado = None;
        self.status = tr!("Campo calculado adicionado. Clique em Criar/Aplicar para atualizar o resultado.").into();
    }

    pub(super) fn remover_campo_calculado(&mut self, indice: usize) {
        let Some(e) = self.dinamica.as_mut() else { return };
        if indice >= e.calculados.len() { return; }
        let col = e.origem.c1 as usize + 1 + indice;
        let real = e.nomes.len().saturating_sub(e.calculados.len());
        e.calculados.remove(indice);
        e.nomes.remove(real + indice);
        let atualizar = |a: &mut Vec<usize>| {
            a.retain(|&c| c != col);
            for c in a { if *c > col { *c -= 1; } }
        };
        atualizar(&mut e.linhas);
        atualizar(&mut e.colunas);
        e.valores.retain(|v| v.coluna != col);
        for v in &mut e.valores { if v.coluna > col { v.coluna -= 1; } }
        e.filtros.retain(|f| f.coluna != col);
        for f in &mut e.filtros { if f.coluna > col { f.coluna -= 1; } }
        e.campo_filtro = None;
        e.valores_filtro.clear();
    }

    /// O que fica recolhido no painel: a origem dos dados e os campos
    /// calculados. Layout, totais, agrupar, cores e recolher foram para a
    /// aba "Tabela Dinâmica" da faixa (17/10/2026).
    pub(super) fn controles_avancados<'a>(&'a self, e: &'a Estado) -> Element<'a, Msg> {
        let mut calculados = column(Vec::<pelicano::Element<_>>::new()).spacing(3);
        for (i, calculado) in e.calculados.iter().enumerate() {
            calculados = calculados.push(row![
                text(format!("{} = {}", calculado.nome, calculado.formula)).size(11).width(Length::FILL),
                button(feather_ui::icones::icone("trash", 13.0)).padding([3, 5])
                    .style(estilo_botao_pequeno(false))
                    .on_press(Msg::Dinamica(Evento::RemoverCalculado(i))),
            ].spacing(6).align(pelicano::Align::Center));
        }
        let previa: Element<'a, Msg> = if let Some(valores) = &e.previa_calculado {
            text(trf!("Prévia: {}", valores.iter().map(|v| v.to_string())
                .collect::<Vec<_>>().join(" · "))).size(11).color(COR_TEXTO_MUTED()).into()
        } else { Space::new(0.0, 0).into() };
        let titulo = |t: &'static str| text(t).size(10.5).color(COR_TEXTO_MUTED());
        column![
            titulo(tr!("ORIGEM DOS DADOS")),
            row![
                text_input("A1:D100", &e.origem_texto)
                    .on_input(|v| Msg::Dinamica(Evento::OrigemTexto(v)))
                    .on_submit(Msg::Dinamica(Evento::OrigemAplicar)).padding(5).size(12),
                button(text(tr!("Aplicar")).size(11.5)).style(estilo_botao_secundario)
                    .on_press(Msg::Dinamica(Evento::OrigemAplicar)),
            ].spacing(6).align(pelicano::Align::Center),
            titulo(tr!("CAMPOS CALCULADOS")),
            calculados,
            text_input(tr!("Nome do campo"), &e.novo_nome)
                .on_input(|s| Msg::Dinamica(Evento::NomeCalculado(s))).padding(6).size(12),
            text_input(tr!("Fórmula: [Preço] * Quantidade"), &e.nova_formula)
                .on_input(|s| Msg::Dinamica(Evento::FormulaCalculado(s))).padding(6).size(12),
            row![
                Space::new(Length::FILL, 0.0),
                button(text(tr!("Prévia")).size(11.5)).style(estilo_botao_secundario)
                    .on_press(Msg::Dinamica(Evento::PreviaCalculado)),
                button(text(tr!("Adicionar")).size(11.5)).style(estilo_botao_primario)
                    .on_press(Msg::Dinamica(Evento::AdicionarCalculado)),
            ].spacing(6),
            previa,
        ].spacing(9).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn previa_de_campo_calculado_nao_altera_a_origem() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0,0,"Preço"); s.set_cell(0,1,"Qtd");
            s.set_cell(1,0,"10"); s.set_cell(1,1,"2");
            s.set_cell(2,0,"20"); s.set_cell(2,1,"3");
        });
        app.fim_selecao = (2,1);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::FormulaCalculado("Preço * Qtd".into()));
        app.evento_dinamica(Evento::PreviaCalculado);
        assert_eq!(app.dinamica.as_ref().unwrap().previa_calculado,Some(vec![20.,60.]));
        assert_eq!(app.workbook.valor(0,2,1),Value::Number(3.));
        assert_eq!(app.workbook.len(),1);
    }

    #[test]
    fn campo_calculado_e_removido_sem_indices_pendentes() {
        let mut app = App::default();
        app.workbook.editar(0, |s| {
            s.set_cell(0,0,"Região"); s.set_cell(0,1,"Preço"); s.set_cell(0,2,"Quantidade");
            s.set_cell(1,0,"Sul"); s.set_cell(1,1,"10"); s.set_cell(1,2,"2");
        });
        app.fim_selecao = (1,2);
        app.evento_dinamica(Evento::Abrir);
        crate::app::dinamica_ui::campos_de_antes(&mut app);
        app.evento_dinamica(Evento::NomeCalculado("Receita".into()));
        app.evento_dinamica(Evento::FormulaCalculado("[Preço] * Quantidade".into()));
        app.evento_dinamica(Evento::AdicionarCalculado);
        let e = app.dinamica.as_ref().expect("janela aberta");
        assert_eq!(e.calculados.len(), 1);
        assert_eq!(e.valores.last().unwrap().coluna, 3);
        app.evento_dinamica(Evento::RemoverCalculado(0));
        let e = app.dinamica.as_ref().unwrap();
        assert!(e.calculados.is_empty());
        assert!(!e.valores.iter().any(|v| v.coluna == 3));
    }
}
