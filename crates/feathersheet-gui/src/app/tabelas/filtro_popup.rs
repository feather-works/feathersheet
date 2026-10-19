//! Popup nativo de filtro: hierarquia e medidas equivalentes ao filter-popup.ts.
//! A lista tem rolagem própria; o painel nunca reserva um grande espaço vazio.
use super::{area_de, sob_id, Acao};
use crate::app::{App, Msg};
use feather_ui::tema::{
    estilo_botao_primario, estilo_botao_secundario, estilo_entrada_suspensa,
    estilo_item_menu, estilo_linha, estilo_popup_menu, LARGURA_SUBMENU, COR_AZUL_ACENTO,
    COR_BORDA_SUAVE, COR_PAINEL, COR_TEXTO, COR_TEXTO_MUTED,
};
use feather_ui::icones::icone;
use iced::{Alignment, Background, Border, Element, Length};
use iced::widget::{button, column, container, row, scrollable, text, text_input, Space};

/// Comportamento compatível com o popup TypeScript e com o menu de colunas:
/// larguras fixas, rolagem apenas na lista e ações sempre visíveis.
pub(in crate::app) const LARGURA: f32 = 300.0;
pub(super) const ALTURA_ITEM: f32 = 28.0;
pub(super) const ALTURA_LISTA: f32 = 148.0;

fn divisor() -> Element<'static, Msg> {
    container(container(Space::with_height(1))
        .width(Length::Fill).style(estilo_linha))
        .padding([6, 0]).width(Length::Fill).into()
}

fn acao(rotulo: impl Into<String>, msg: Option<Acao>) -> Element<'static, Msg> {
    button(text(rotulo.into()).size(13))
        .width(Length::Fill)
        .height(Length::Fixed(ALTURA_ITEM))
        .padding([4, 10])
        .style(estilo_item_menu(false, msg.is_some()))
        .on_press_maybe(msg.map(Msg::TabelaAcao))
        .into()
}

/// Linha de ação com ícone Tabler à esquerda (sem caracteres Unicode, que
/// a fonte do app pode não ter e apareceriam como quadrados vazios).
fn acao_com_icone(
    nome_icone: &str,
    rotulo: impl Into<String>,
    msg: Option<Acao>,
) -> Element<'static, Msg> {
    button(
        row![icone(nome_icone, 15.0), text(rotulo.into()).size(13)]
            .spacing(8).align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ALTURA_ITEM))
    .padding([4, 10])
    .style(estilo_item_menu(false, msg.is_some()))
    .on_press_maybe(msg.map(Msg::TabelaAcao))
    .into()
}

fn caixa_marcacao(valor: String, rotulo: String, marcado: bool) -> Element<'static, Msg> {
    let quadrado = container(if marcado {
        // Ícone Tabler "checks" tingido de branco sobre o acento.
        Element::from(icone("checks", 12.0).style(|_, _| iced::widget::svg::Style {
            color: Some(iced::Color::WHITE),
        }))
    } else {
        Space::with_width(0).into()
    })
    .width(Length::Fixed(16.0))
    .height(Length::Fixed(16.0))
    .align_x(iced::alignment::Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_| container::Style {
        background: Some(Background::Color(
            if marcado { COR_AZUL_ACENTO() } else { COR_PAINEL() }
        )),
        border: Border {
            color: if marcado { COR_AZUL_ACENTO() } else { COR_BORDA_SUAVE() },
            width: 1.0, radius: 3.0.into(),
        },
        ..Default::default()
    });
    button(row![quadrado, text(rotulo).size(12.5)]
        .spacing(9).align_y(Alignment::Center))
        .width(Length::Fill)
        .height(Length::Fixed(26.0))
        .padding([3, 7])
        .style(estilo_item_menu(false, true))
        .on_press(Msg::TabelaAcao(Acao::MarcarValor(valor)))
        .into()
}

fn botoes_rodape(acao_principal: Acao, texto_principal: &str) -> Element<'static, Msg> {
    row![
        Space::with_width(Length::Fill),
        button(text(tr!("Cancelar")).size(12.5))
            .padding([7, 14])
            .style(estilo_botao_secundario)
            .on_press(Msg::TabelaAcao(Acao::Fechar)),
        button(text(texto_principal.to_owned()).size(12.5))
            .padding([7, 18])
            .style(estilo_botao_primario)
            .on_press(Msg::TabelaAcao(acao_principal)),
    ].spacing(8).align_y(Alignment::Center).into()
}

impl App {
    pub(in crate::app) fn menu_filtro_tabela(&self) -> Element<'_, Msg> {
        let Some(id) = self.tabela_editando.as_ref() else {
            return Space::with_height(0).into();
        };
        let Ok(t) = sob_id(&self.workbook, self.folha, id) else {
            return Space::with_height(0).into();
        };
        let Some(area) = area_de(&t) else {
            return Space::with_height(0).into();
        };
        let col = self.tabela_col_filtro.clamp(area.c0, area.c1);
        let nome_coluna = feathersheet_core::celulas::refs::col_name(col);
        let filtro_ativo = t["filtros"].as_array().is_some_and(|lista| {
            lista.iter().any(|f| f["col"].as_u64() == Some((col - area.c0) as u64))
        });
        let tem_filtros = t["filtros"].as_array().is_some_and(|v|!v.is_empty());
        // O menu principal nunca é substituído por um filtro avançado.
        // Os operadores e o critério são desenhados em outro popup (overlay).
        let cabecalho = row![
            text(trf!("Menu da coluna {nome_coluna}")).size(13.5).color(COR_TEXTO()),
            Space::with_width(Length::Fill),
            button(icone("x", 14.0))
                .padding([2, 9])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::TabelaAcao(Acao::Fechar)),
        ].spacing(8).align_y(Alignment::Center);
        // Cabe na janela: a lista encolhe até 3 linhas e, se ainda faltar
        // espaço, o meio rola; cabeçalho e OK/Cancelar ficam sempre à vista.
        let disponivel = (self.tamanho_principal.height - 8.0 - self.topo_menu_principal()).max(240.0);
        let max_meio = disponivel - 45.0 - 57.0;
        let n_itens = 7 + self.n_itens_filtro_extra();
        let pe_lista = if self.tabela_filtro_truncado { 25.0 } else { 14.0 };
        let sem_lista = n_itens as f32 * 28.0 + 26.0 + 22.0 + 32.0 + 10.0 + 14.0 + pe_lista
            + 3.0 * (n_itens + 7) as f32;
        let teto_lista = (max_meio - sem_lista).clamp(3.0 * 26.0 + 8.0, ALTURA_LISTA);
        let mut conteudo = column![].spacing(3);
        {
            // 1. Classificação/limpeza em linhas discretas, sem botões azuis.
            conteudo = conteudo
                .push(acao_com_icone("sort-ascending", tr!("Classificar de A a Z"),
                    Some(Acao::Ordenar(true))))
                .push(acao_com_icone("sort-descending", tr!("Classificar de Z a A"),
                    Some(Acao::Ordenar(false))));
            for item in self.itens_classificar_extra() { conteudo = conteudo.push(item); }
            conteudo = conteudo
                .push(divisor())
                .push(acao(
                    trf!("Limpar filtro de {nome_coluna}"),
                    filtro_ativo.then_some(Acao::LimparFiltro),
                ))
                .push(acao(
                    tr!("Limpar todos os filtros"),
                    tem_filtros.then_some(Acao::LimparTodosFiltros),
                ));
            for item in self.itens_filtro_extra() { conteudo = conteudo.push(item); }
            conteudo = conteudo
                .push(button(
                    row![
                        text(tr!("Filtrar por condição")).size(13),
                        Space::with_width(Length::Fill),
                        icone("chevron-right", 14.0),
                    ].align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .height(Length::Fixed(ALTURA_ITEM))
                .padding([4, 10])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::TabelaAcao(Acao::AbrirCondicoes)))
                .push(divisor());

            // Índice filtrado e viewport independentes: mesmo depois de
            // descobrir 1 milhão de valores, só ~18 widgets são construídos.
            let total = self.tabela_filtro_indices.len();
            let inicio = self.tabela_filtro_inicio_visivel.min(total);
            const JANELA: usize = 18;
            const ALTURA_LINHA: f32 = 26.0;
            let fim = inicio.saturating_add(JANELA).min(total);
            conteudo = conteudo.push(
                row![
                    button(text(tr!("Selecionar tudo")).size(11.5).color(COR_AZUL_ACENTO()))
                        .padding([3, 3])
                        .style(estilo_item_menu(false, true))
                        .on_press(Msg::TabelaAcao(Acao::MarcarTodos(true))),
                    text("·").size(12).color(COR_TEXTO_MUTED()),
                    button(text(tr!("Limpar")).size(11.5).color(COR_AZUL_ACENTO()))
                        .padding([3, 3])
                        .style(estilo_item_menu(false, true))
                        .on_press(Msg::TabelaAcao(Acao::MarcarTodos(false))),
                    Space::with_width(Length::Fill),
                    text(trf!("{} / {} marcados",
                        self.tabela_filtro_marcados.len(),
                        self.tabela_filtro_lista.len())).size(11)
                        .color(COR_TEXTO_MUTED()),
                ].spacing(2).align_y(Alignment::Center)
            );

            conteudo = conteudo.push(
                text_input(tr!("Pesquisar valores carregados…"), &self.tabela_filtro_busca)
                    .on_input(|v| Msg::TabelaAcao(Acao::BuscaLista(v)))
                    .style(estilo_entrada_suspensa)
                    .size(12.5)
                    .padding([7, 10])
                    .width(Length::Fill)
            );

            // Virtualização real: a altura dos espaços simula as linhas
            // omitidas e preserva a barra de rolagem e a posição original.
            let mut itens = column![].spacing(0);
            if inicio > 0 {
                itens = itens.push(Space::with_height(Length::Fixed(inicio as f32 * ALTURA_LINHA)));
            }
            for &indice in &self.tabela_filtro_indices[inicio..fim] {
                let valor = &self.tabela_filtro_lista[indice];
                itens = itens.push(caixa_marcacao(
                    valor.clone(), self.rotulo_valor_lista(valor),
                    self.tabela_filtro_marcados.contains(valor)
                ));
            }
            if fim < total {
                itens = itens.push(Space::with_height(Length::Fixed((total - fim) as f32 * ALTURA_LINHA)));
            }
            if total == 0 {
                itens = itens.push(text(tr!("Nenhum valor encontrado entre os já carregados")).size(12)
                    .color(COR_TEXTO_MUTED()));
            }
            let altura = (total as f32 * ALTURA_LINHA + 8.0)
                .clamp(36.0, teto_lista);
            conteudo = conteudo.push(
                container(scrollable(itens)
                    .id(iced::widget::scrollable::Id::new("feathersheet-filtro-valores"))
                    .on_scroll(|viewport| Msg::TabelaAcao(
                        Acao::ScrollLista(viewport.absolute_offset().y)))
                    .height(Length::Fixed(altura)))
                    .padding(4)
                    .width(Length::Fill)
                    .style(|_| container::Style {
                        background: Some(Background::Color(COR_PAINEL())),
                        border: Border { color: COR_BORDA_SUAVE(),
                            width: 1.0, radius: 5.0.into() },
                        ..Default::default()
                    })
            );
            let inicio_dados = area.r0.saturating_add(u32::from(t["temCabecalho"] == true));
            let total_linhas = if self.tabela_filtro_ultima_linha < inicio_dados { 0 }
                else { self.tabela_filtro_ultima_linha - inicio_dados + 1 };
            let lidas = self.tabela_filtro_proxima_linha
                .saturating_sub(inicio_dados).min(total_linhas);
            conteudo = conteudo.push(
                text(trf!("{} valores encontrados · {} / {} linhas lidas · {} na pesquisa",
                    self.tabela_filtro_lista.len(), lidas, total_linhas, total))
                    .size(10.5).color(COR_TEXTO_MUTED())
            );
            if self.tabela_filtro_truncado {
                let carregando = self.tabela_filtro_automatico;
                conteudo = conteudo.push(
                    row![
                        button(text(if carregando { tr!("Lendo…") } else { tr!("Carregar +8 mil linhas") }).size(11))
                            .padding([5, 7])
                            .style(estilo_botao_secundario)
                            .on_press_maybe((!carregando).then_some(Msg::TabelaAcao(Acao::CarregarMais))),
                        Space::with_width(Length::Fill),
                        button(text(if carregando { tr!("Parar") } else { tr!("Carregar tudo") }).size(11))
                            .padding([5, 7])
                            .style(estilo_botao_secundario)
                            .on_press(Msg::TabelaAcao(if carregando { Acao::PararCarga }
                                else { Acao::CarregarTodos })),
                    ].spacing(5).align_y(Alignment::Center)
                );
            } else {
                conteudo = conteudo.push(
                    text(tr!("Todos os dados da coluna foram examinados."))
                        .size(10.5).color(COR_TEXTO_MUTED())
                );
            }
        }
        let altura_lista = (self.tabela_filtro_indices.len() as f32 * 26.0 + 8.0).clamp(36.0, teto_lista);
        let meio: Element<'_, Msg> = if sem_lista + altura_lista + 10.0 > max_meio {
            use iced::widget::scrollable::{Direction, Scrollbar};
            scrollable(conteudo)
                .direction(Direction::Vertical(Scrollbar::new().width(5).scroller_width(5).spacing(3)))
                .height(Length::Fixed(max_meio.max(120.0))).into()
        } else {
            conteudo.into()
        };
        let conteudo = column![
            cabecalho, divisor(), meio, divisor(),
            botoes_rodape(Acao::AplicarLista, "OK"),
        ].spacing(3).padding(8);
        // O popup segue o modo claro/escuro do FeatherWorks e mantém
        // bordas de 8 px, sem superfície lateral ou altura fixa ociosa.
        container(conteudo)
            .width(Length::Fixed(LARGURA))
            .style(estilo_popup_menu)
            .into()
    }

    /// Flyout secundário flutuante, que NÃO substitui o menu de valores.
    /// Página 1 lista operadores e página 2 edita o critério.
    pub(in crate::app) fn menu_condicoes_tabela(&self) -> Element<'_, Msg> {
        let pagina = self.tabela_filtro_pagina;
        if pagina == 0 {
            return Space::with_height(0).into();
        }
        if pagina >= super::filtro_extra::PAG_COR {
            return self.submenu_extra(pagina);
        }
        let titulo = if pagina == 1 { tr!("Filtrar por condição") }
            else { tr!("Configurar condição") };
        let cabecalho = row![
            button(icone("chevron-left", 14.0))
                .padding([3, 5])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::TabelaAcao(Acao::VoltarCondicoes)),
            text(titulo).size(13).color(COR_TEXTO()),
            Space::with_width(Length::Fill),
            button(icone("x", 14.0))
                .padding([1, 7])
                .style(estilo_item_menu(false, true))
                .on_press(Msg::TabelaAcao(Acao::FecharCondicoes)),
        ].spacing(4).align_y(Alignment::Center);

        let mut corpo = column![cabecalho, divisor()].spacing(6).padding(7);
        if pagina == 1 {
            corpo = corpo.push(text(tr!("Escolha uma condição")).size(11.5)
                .color(COR_TEXTO_MUTED()));
            let mut operadores = column![].spacing(1);
            for &(rotulo, op) in super::painel::OPERADORES {
                operadores = operadores.push(
                    acao(tr!(rotulo), Some(Acao::Operador(op)))
                );
            }
            // Só a lista de operadores rola, sem esticar a janelinha.
            corpo = corpo.push(scrollable(operadores)
                .height(Length::Fixed(210.0)));
        } else {
            let rotulo = super::painel::OPERADORES.iter()
                .find(|(_, op)| *op == self.tabela_operador)
                .map(|(r, _)| tr!(*r)).unwrap_or(tr!("Condição"));
            corpo = corpo.push(text(rotulo).size(12.5));
            let sem_parametro = matches!(self.tabela_operador,
                "is_true" | "is_false" | "is_blank" | "is_not_blank");
            if !sem_parametro {
                corpo = corpo.push(
                    text_input(tr!("Valor da condição"), &self.tabela_criterio)
                        .on_input(|v| Msg::TabelaAcao(Acao::Criterio(v)))
                        .on_submit(Msg::TabelaAcao(Acao::AplicarFiltro))
                        .style(estilo_entrada_suspensa)
                        .size(12)
                        .padding([7, 8])
                        .width(Length::Fill),
                );
            }
            corpo = corpo.push(divisor());
            corpo = corpo.push(row![
                button(text(tr!("Cancelar")).size(12))
                    .padding([6, 9])
                    .style(estilo_botao_secundario)
                    .on_press(Msg::TabelaAcao(Acao::FecharCondicoes)),
                Space::with_width(Length::Fill),
                button(text(tr!("Aplicar")).size(12))
                    .padding([6, 12])
                    .style(estilo_botao_primario)
                    .on_press(Msg::TabelaAcao(Acao::AplicarFiltro)),
            ].spacing(5).align_y(Alignment::Center));
        }
        let altura = if pagina == 1 { 270.0 } else { 150.0 };
        iced::widget::column![
            Space::with_height(Length::Fixed(self.recuo_flyout(altura))),
            container(corpo)
                .width(Length::Fixed(LARGURA_SUBMENU))
                .style(estilo_popup_menu),
        ].into()
    }
}

/// Calcula a posição do flyout com inversão à esquerda na borda direita.
/// Caso não exista espaço lateral, sobrepõe parcialmente o menu principal
/// sem sair do viewport. Coordenadas relativas à janela em pixels Iced.
pub(in crate::app) fn posicao_condicoes(
    principal_x: f32,
    principal_y: f32,
    janela_largura: f32,
    janela_altura: f32,
    altura_submenu: f32,
) -> (f32, f32) {
    let margem = 8.0;
    let largura = LARGURA_SUBMENU;
    let gap = 6.0;
    let esquerda = principal_x - largura - gap;
    let direita = principal_x + LARGURA + gap;
    let limite_x = (janela_largura - largura - margem).max(0.0);
    let x = if direita + largura + margem <= janela_largura {
        direita
    } else if esquerda >= margem {
        esquerda
    } else {
        limite_x
    }.clamp(0.0, limite_x);
    let limite_y = (janela_altura - altura_submenu - margem).max(0.0);
    let y = (principal_y + 115.0).clamp(0.0, limite_y);
    (x, y)
}

#[cfg(test)]
mod testes_submenu {
    use super::*;

    #[test]
    fn flyout_abre_ao_lado_e_inverte_na_borda_direita() {
        let (x, y) = posicao_condicoes(100.0, 80.0, 1200.0, 800.0, 320.0);
        assert!(x >= 400.0 && y > 80.0);
        let (x2, _) = posicao_condicoes(900.0, 80.0, 1200.0, 800.0, 320.0);
        assert!(x2 < 900.0, "perto da borda deve abrir à esquerda");
    }

    #[test]
    fn flyout_pequena_janela_nao_ultrapassa_os_limites() {
        for (w, h) in [(560.0, 420.0), (360.0, 300.0), (800.0, 600.0)] {
            let (x, y) = posicao_condicoes(w - 40.0, h - 80.0, w, h, 210.0);
            assert!(x >= 0.0 && x + LARGURA_SUBMENU <= w);
            assert!(y >= 0.0 && y + 210.0 <= h);
        }
    }
}
