//! Lista de cálculos da célula da linha de totais (a seta que aparece na
//! célula ativa, como no Excel). Escolher grava a fórmula na célula, numa
//! única ação de desfazer — ver `totais.rs`.
use super::totais::{conta_da_coluna, definir_total, tabela_da_linha_de_totais, CONTAS};
use super::{sob_id, Acao};
use crate::app::{App, Msg};
use feather_ui::icones::icone;
use feather_ui::tema::{estilo_item_menu, estilo_popup_menu};
use pelicano::button; use pelicano::column; use pelicano::container; use pelicano::row; use pelicano::text; use pelicano::Space;
use pelicano::Element; use pelicano::Length; use pelicano::Task;

pub(in crate::app) const LARGURA: f32 = 190.0;
pub(in crate::app) const ALTURA: f32 = CONTAS.len() as f32 * 28.0 + 3.0 * (CONTAS.len() as f32 - 1.0) + 16.0;

impl App {
    /// Trata as duas ações da seta de totais; `None` deixa o resto para o painel.
    pub(in crate::app) fn acao_totais(&mut self, acao: &Acao) -> Option<Task<Msg>> {
        match acao {
            Acao::Fechar => {
                self.tabela_popup_totais = false;
                None
            }
            Acao::AbrirTotais(r, c, x, y) => {
                self.trocar_selecao(*r, *c);
                let Some((id, _)) = tabela_da_linha_de_totais(&self.workbook, self.folha, *r, *c)
                else { return Some(Task::none()) };
                self.tabela_editando = Some(id);
                self.tabela_col_filtro = *c;
                self.tabela_popup_x = (*x - 18.0).max(0.0);
                self.tabela_popup_y = (*y).max(0.0);
                self.tabela_popup_filtro = false;
                self.tabela_popup_totais = true;
                Some(Task::none())
            }
            Acao::EscolherTotal(conta) => {
                self.tabela_popup_totais = false;
                let Some(id) = self.tabela_editando.clone() else { return Some(Task::none()) };
                let c0 = sob_id(&self.workbook, self.folha, &id).ok()
                    .and_then(|t| super::area_de(&t)).map_or(0, |a| a.c0);
                let rel = self.tabela_col_filtro.saturating_sub(c0);
                let r = definir_total(&mut self.workbook, self.folha, &id, rel, conta);
                match r {
                    Ok(()) => {
                        self.status = tr!("Cálculo da linha de totais atualizado.").into();
                        self.alterado = true;
                    }
                    Err(e) => self.status = e,
                }
                Some(Task::none())
            }
            _ => None,
        }
    }

    pub(in crate::app) fn menu_totais_tabela(&self) -> Element<'_, Msg> {
        let atual = self.tabela_editando.as_deref()
            .and_then(|id| sob_id(&self.workbook, self.folha, id).ok())
            .and_then(|t| {
                let rel = self.tabela_col_filtro.checked_sub(super::area_de(&t)?.c0)?;
                Some(conta_da_coluna(&t, rel))
            })
            .unwrap_or_else(|| "nenhum".into());
        let mut itens = column(Vec::<pelicano::Element<_>>::new()).spacing(3).padding(8);
        for (chave, rotulo, _) in CONTAS {
            let marcado = *chave == atual;
            let marca: Element<'_, Msg> = if marcado {
                icone("check", 13.0).into()
            } else {
                Space::new(Length::Fixed(13.0), 0.0).into()
            };
            itens = itens.push(
                button(row![marca, text(tr!(*rotulo)).size(13)]
                    .spacing(8).align(pelicano::Align::Center))
                    .width(Length::FILL)
                    .height(Length::Fixed(28.0))
                    .padding([4, 10])
                    .style(estilo_item_menu(marcado, true))
                    .on_press(Msg::TabelaAcao(Acao::EscolherTotal(chave))),
            );
        }
        container(itens).width(Length::Fixed(LARGURA)).style(estilo_popup_menu).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::refs::Rect;

    fn app_com_tabela() -> App {
        let mut app = App::default();
        for (r, (a, b)) in [("a", "b"), ("x", "10"), ("y", "20")].iter().enumerate() {
            app.workbook.escrever_celula(0, r as u32, 0, (*a).into());
            app.workbook.escrever_celula(0, r as u32, 1, (*b).into());
        }
        super::super::criar(&mut app.workbook, 0, Rect::new(0, 0, 2, 1), "Tabela1", true, false)
            .unwrap();
        app
    }

    #[test]
    fn seta_da_celula_de_total_escolhe_conta_com_um_ctrl_z() {
        let mut app = app_com_tabela();
        app.trocar_selecao(0, 0);
        let _ = app.acao_tabela(Acao::Totais);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 1), "=SUBTOTAL(109;Tabela1[b])");
        let _ = app.acao_tabela(Acao::AbrirTotais(3, 1, 300.0, 100.0));
        assert!(app.tabela_popup_totais);
        assert_eq!((app.linha, app.coluna), (3, 1));
        { let _menu: Element<'_, Msg> = app.menu_totais_tabela(); }
        let _ = app.acao_tabela(Acao::EscolherTotal("media"));
        assert!(!app.tabela_popup_totais);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 1), "=SUBTOTAL(101;Tabela1[b])");
        let _ = app.acao_tabela(Acao::AbrirTotais(3, 1, 300.0, 100.0));
        let _ = app.acao_tabela(Acao::EscolherTotal("nenhum"));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 1), "");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 1), "=SUBTOTAL(101;Tabela1[b])");
        let _ = app.update(Msg::Desfazer);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(3, 1), "=SUBTOTAL(109;Tabela1[b])");
    }
}
