//! Variações do botão dividido Mesclar ▾ da Página Inicial, como no Excel:
//! Mesclar e centralizar, Mesclar através, Mesclar e Desmesclar células.
//! Cada uma é uma única transação: um Ctrl+Z desfaz a mescla e o estilo.
use super::super::*;
use feathersheet_core::celulas::refs::col_name;

/// Mesclar através cria uma região por linha; acima disto vira trabalho
/// demais para um clique só (e um passo de desfazer gigante).
const TETO_LINHAS_ATRAVES: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModoMescla { Centralizar, Atraves, Mesclar, Desmesclar }

impl App {
    pub(in crate::app) fn mesclar_modo(&mut self, modo: ModoMescla) {
        self.confirmar();
        let area = self.area_selecionada();
        let mut mudou = false;
        let status: String = match modo {
            ModoMescla::Desmesclar => {
                self.workbook.editar(self.folha, |sheet| {
                    mudou = mesclas::desmesclar(sheet, area.clone());
                });
                if mudou { tr!("Mesclagem desfeita").into() }
                else { tr!("Não há células mescladas na seleção").into() }
            }
            _ if area.area() < 2 => tr!("Selecione duas ou mais células para mesclar").into(),
            ModoMescla::Atraves if area.c0 == area.c1 =>
                tr!("Selecione mais de uma coluna para mesclar através").into(),
            ModoMescla::Atraves if area.r1 - area.r0 + 1 > TETO_LINHAS_ATRAVES =>
                trf!("Mesclar através aceita até {TETO_LINHAS_ATRAVES} linhas"),
            ModoMescla::Atraves => {
                self.workbook.editar(self.folha, |sheet| {
                    sheet.abrir_transacao();
                    // Mescla antiga de várias linhas engoliria as novas:
                    // desfaz primeiro, como o Excel.
                    mudou |= mesclas::desmesclar(sheet, area.clone());
                    for r in area.r0..=area.r1 {
                        mudou |= mesclas::mesclar(sheet, Rect::new(r, area.c0, r, area.c1));
                    }
                    sheet.fechar_transacao();
                });
                let linhas = area.r1 - area.r0 + 1;
                trf!("{linhas} {} mesclada{} de {} a {}",
                    if linhas == 1 { "linha" } else { "linhas" },
                    if linhas == 1 { "" } else { "s" },
                    col_name(area.c0), col_name(area.c1))
            }
            ModoMescla::Mesclar | ModoMescla::Centralizar => {
                let centralizar = modo == ModoMescla::Centralizar;
                self.workbook.editar(self.folha, |sheet| {
                    sheet.abrir_transacao();
                    mudou = mesclas::mesclar(sheet, area.clone());
                    if centralizar {
                        let mut patch = EstiloParcial::new();
                        patch.insert("align".into(), Campo::Texto("center".into()));
                        sheet.set_style(&area, &patch);
                        mudou = true;
                    }
                    sheet.fechar_transacao();
                });
                trf!("Células {}:{} mescladas{}",
                    cell_name(area.r0, area.c0), cell_name(area.r1, area.c1),
                    if centralizar { tr!(" e centralizadas") } else { "" })
            }
        };
        self.alterado |= mudou;
        self.status = status;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn regioes(app: &App) -> Vec<Rect> {
        mesclas::ler(&app.workbook.aba(0).unwrap().sheet)
    }

    fn selecionar(app: &mut App, r0: u32, c0: u32, r1: u32, c1: u32) {
        let _ = app.update(Msg::Selecionar(r0, c0));
        let _ = app.update(Msg::Estender(r1, c1));
    }

    #[test]
    fn mesclar_e_centralizar_e_um_desfazer_so() {
        let mut app = App::default();
        selecionar(&mut app, 0, 0, 1, 2);
        let _ = app.update(Msg::MesclarModo(ModoMescla::Centralizar));
        assert_eq!(regioes(&app), vec![Rect::new(0, 0, 1, 2)]);
        let estilo = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0).unwrap();
        assert_eq!(estilo.align, feathersheet_core::celulas::style::Align::Center);
        let _ = app.update(Msg::Desfazer);
        assert!(regioes(&app).is_empty());
        let estilo = app.workbook.aba(0).unwrap().sheet.estilo_resolvido(0, 0);
        assert!(estilo.is_none_or(|e| e.align != feathersheet_core::celulas::style::Align::Center));
    }

    #[test]
    fn mesclar_atraves_cria_uma_regiao_por_linha() {
        let mut app = App::default();
        selecionar(&mut app, 2, 1, 4, 3);
        let _ = app.update(Msg::MesclarModo(ModoMescla::Atraves));
        assert_eq!(regioes(&app), vec![
            Rect::new(2, 1, 2, 3), Rect::new(3, 1, 3, 3), Rect::new(4, 1, 4, 3),
        ]);
        let _ = app.update(Msg::Desfazer);
        assert!(regioes(&app).is_empty());
    }

    #[test]
    fn mesclar_atraves_desfaz_mescla_antiga_de_varias_linhas() {
        let mut app = App::default();
        selecionar(&mut app, 0, 0, 1, 1);
        let _ = app.update(Msg::MesclarModo(ModoMescla::Mesclar));
        let _ = app.update(Msg::MesclarModo(ModoMescla::Atraves));
        assert_eq!(regioes(&app), vec![Rect::new(0, 0, 0, 1), Rect::new(1, 0, 1, 1)]);
    }

    #[test]
    fn desmesclar_e_selecao_de_uma_celula() {
        let mut app = App::default();
        let _ = app.update(Msg::MesclarModo(ModoMescla::Mesclar));
        assert!(regioes(&app).is_empty());
        selecionar(&mut app, 0, 0, 0, 3);
        let _ = app.update(Msg::MesclarModo(ModoMescla::Mesclar));
        assert_eq!(regioes(&app).len(), 1);
        let _ = app.update(Msg::MesclarModo(ModoMescla::Desmesclar));
        assert!(regioes(&app).is_empty());
    }
}
