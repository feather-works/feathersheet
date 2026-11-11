//! DIN-6: itens calculados (o "Item calculado" do Excel), no painel ▸ Origem e campos
//! calculados. Escolhe-se um campo de Linhas ou Colunas, um nome e a fórmula sobre os
//! outros itens do campo (`Norte - Sul`); o motor faz a conta em cada cruzamento
//! (`Extras::itens_calculados`).
use super::*;
use feathersheet_core::dinamica::campo_calculado;
use feathersheet_core::dinamica::exibicao::ItemCalculado;
use pelicano::column;

/// O campo da fórmula do item: volta o foco a ele depois de completar um nome.
pub(super) const ID_FORMULA_ITEM: &str = "dinamica-formula-item";

impl App {
    /// Os itens do campo da `coluna` como a dinâmica os mostra (agrupamento e grupos à mão), na
    /// ordem natural (meses e trimestres em ordem; o resto pelo texto). Também a lista do
    /// "item base" de "% de"/"Diferença de".
    pub(super) fn itens_do_campo(&self, e: &Estado, coluna: usize) -> Vec<String> {
        let extras = avancado::extras_vigentes(e);
        let extra = match (e.linhas.iter().position(|&c| c == coluna), e.colunas.iter().position(|&c| c == coluna)) {
            (Some(p), _) => extras.campo(EixoExtra::Linhas, p),
            (None, Some(p)) => extras.campo(EixoExtra::Colunas, p),
            _ => return Vec::new(),
        };
        let ultima = e.origem.r1.min(e.origem.r0.saturating_add(config_campo::LINHAS_DOS_ITENS));
        let mut vistos: std::collections::HashMap<String, Option<f64>> = std::collections::HashMap::new();
        for r in (e.origem.r0 + 1)..=ultima {
            if vistos.len() >= config_campo::MAX_ITENS { break; }
            let v = self.workbook.valor(e.folha_origem, r, coluna as u32);
            let (rotulo, ordem) = feathersheet_core::dinamica::dinamica_layout::rotulo_do_item(&v, &extra, e.agrupar_data);
            let juntado = extras.juntar(coluna, rotulo.clone());
            // Um grupo feito à mão não tem ordem natural: vai pelo nome.
            let ordem = if juntado == rotulo { ordem } else { None };
            vistos.entry(juntado).or_insert(ordem);
        }
        let mut itens: Vec<(String, Option<f64>)> = vistos.into_iter().collect();
        itens.sort_by(|(a, oa), (b, ob)| match (oa, ob) {
            (Some(x), Some(y)) => x.total_cmp(y),
            _ => feathersheet_core::dinamica::exibicao::comparar_natural(a, b),
        });
        itens.into_iter().map(|(n, _)| n).collect()
    }

    pub(super) fn evento_item_calculado(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::ItemCampo(coluna) => {
                let itens = self.dinamica.as_ref().map(|e| self.itens_do_campo(e, coluna)).unwrap_or_default();
                if let Some(e) = &mut self.dinamica {
                    e.ui.item_campo = Some(coluna);
                    e.ui.itens_do_campo = itens;
                }
            }
            Evento::ItemNome(nome) => if let Some(e) = &mut self.dinamica { e.ui.item_nome = nome },
            Evento::ItemFormula(f) => if let Some(e) = &mut self.dinamica { e.ui.item_formula = f },
            Evento::CompletarItem(nome) => if let Some(e) = &mut self.dinamica {
                e.ui.item_formula = campo_calculado::completar(&e.ui.item_formula, &nome);
                return Task::focus(ID_FORMULA_ITEM);
            },
            Evento::AdicionarItem => self.adicionar_item_calculado(),
            Evento::RemoverItem(i) => if let Some(e) = &mut self.dinamica {
                let itens = &mut config_campo::extras_editaveis(e).itens_calculados;
                if i < itens.len() { itens.remove(i); }
            },
            _ => {}
        }
        Task::none()
    }

    fn adicionar_item_calculado(&mut self) {
        let Some(e) = &mut self.dinamica else { return };
        let (nome, formula) = (e.ui.item_nome.trim().to_owned(), e.ui.item_formula.trim().to_owned());
        let Some(coluna) = e.ui.item_campo.filter(|c| e.linhas.contains(c) || e.colunas.contains(c)) else {
            self.status = tr!("Escolha o campo de Linhas ou Colunas do item calculado.").into();
            return;
        };
        if nome.is_empty() || formula.is_empty() {
            self.status = tr!("Informe nome e fórmula para o item calculado.").into();
            return;
        }
        if e.ui.itens_do_campo.contains(&nome) || avancado::extras_vigentes(e).itens_calculados.iter().any(|i| i.coluna == coluna && i.nome == nome) {
            self.status = trf!("O campo já tem um item chamado {nome}.");
            return;
        }
        if let Err(msg) = campo_calculado::compilar(&formula, &e.ui.itens_do_campo) {
            self.status = msg;
            return;
        }
        config_campo::extras_editaveis(e).itens_calculados.push(ItemCalculado { coluna, nome: nome.clone(), formula });
        e.ui.item_nome.clear();
        e.ui.item_formula.clear();
        self.status = trf!("Item calculado {nome} adicionado.");
    }

    /// A seção "ITENS CALCULADOS" do painel avançado.
    pub(super) fn secao_itens_calculados<'a>(&'a self, e: &'a Estado) -> Element<'a, Msg> {
        let titulo = text(tr!("ITENS CALCULADOS")).size(10.5).color(COR_TEXTO_MUTED());
        let mut lista = column(Vec::<Element<'a, Msg>>::new()).spacing(3);
        for (i, item) in avancado::extras_vigentes(e).itens_calculados.iter().enumerate() {
            lista = lista.push(row![
                text(format!("{}: {} = {}", e.nome_da_coluna(item.coluna), item.nome, item.formula)).size(11).width(Length::FILL),
                button(feather_ui::icones::icone("trash", 13.0)).padding([3, 5])
                    .style(estilo_botao_pequeno(false))
                    .on_press(Msg::Dinamica(Evento::RemoverItem(i))),
            ].spacing(6).align(pelicano::Align::Center));
        }
        let campos = pelicano::row(e.linhas.iter().chain(&e.colunas).map(|&c| {
            button(text(e.nome_da_coluna(c)).size(11)).padding([2, 8])
                .style(if e.ui.item_campo == Some(c) { estilo_botao_primario } else { estilo_botao_secundario })
                .on_press(Msg::Dinamica(Evento::ItemCampo(c))).into()
        }).collect::<Vec<Element<'a, Msg>>>()).spacing(4).wrap();
        let sugestoes = pelicano::row(
            campo_calculado::sugestoes(&e.ui.item_formula, &e.ui.itens_do_campo).into_iter().take(12).map(|nome| {
                button(text(nome).size(11)).padding([2, 8]).style(estilo_botao_secundario)
                    .on_press(Msg::Dinamica(Evento::CompletarItem(nome.to_string()))).into()
            }).collect::<Vec<Element<'a, Msg>>>(),
        ).spacing(4).wrap();
        let corpo: Element<'a, Msg> = if e.ui.item_campo.is_some() {
            column![
                text_input(tr!("Nome do item"), &e.ui.item_nome)
                    .on_input(|s| Msg::Dinamica(Evento::ItemNome(s))).padding(6).size(12),
                text_input(tr!("Fórmula: Norte - Sul"), &e.ui.item_formula)
                    .id(ID_FORMULA_ITEM)
                    .on_input(|s| Msg::Dinamica(Evento::ItemFormula(s)))
                    .on_submit(Msg::Dinamica(Evento::AdicionarItem)).padding(6).size(12),
                sugestoes,
                row![
                    Space::new(Length::FILL, 0.0),
                    button(text(tr!("Adicionar")).size(11.5)).style(estilo_botao_primario)
                        .on_press(Msg::Dinamica(Evento::AdicionarItem)),
                ],
            ].spacing(9).into()
        } else {
            text(tr!("Escolha o campo do item:")).size(11).color(COR_TEXTO_MUTED()).into()
        };
        column![titulo, lista, campos, corpo].spacing(9).into()
    }
}
