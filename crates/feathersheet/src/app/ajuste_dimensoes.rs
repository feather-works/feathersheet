//! Autoajuste de largura de colunas e altura de linhas ao conteúdo, em lote.
//! A medida mora no motor (`operacoes::autoajuste`); aqui grava tudo numa
//! única transação: um Ctrl+Z desfaz o lote inteiro.
use super::*;
use crate::grade::{ALTURA_PADRAO, LARGURA_PADRAO};
use feathersheet_core::operacoes::autoajuste;

fn larguras_ideais(w: &Workbook, folha: usize, c0: u32, c1: u32) -> Vec<(u32, u32)> {
    let m = w.aba(folha).map(|a| mesclas::ler(&a.sheet)).unwrap_or_default();
    let padrao = w.aba(folha).map_or(LARGURA_PADRAO, |a| crate::grade::largura_padrao_da(&a.sheet));
    autoajuste::larguras_ideais(w, folha, c0, c1, padrao as u32, &m)
}

pub(super) fn alturas_ideais(w: &Workbook, folha: usize, r0: u32, r1: u32) -> Vec<(u32, u32)> {
    let m = w.aba(folha).map(|a| mesclas::ler(&a.sheet)).unwrap_or_default();
    autoajuste::alturas_ideais(w, folha, r0, r1, ALTURA_PADRAO as u32, &m)
}

impl App {
    /// Colunas da seleção atual (a ativa, se não houver faixa de colunas).
    fn colunas_da_selecao(&self) -> (u32, u32) {
        let a = self.area_selecionada();
        (a.c0, a.c1)
    }

    pub(super) fn selecao_e_colunas_inteiras(&self) -> bool {
        self.linha == 0 && self.fim_selecao.0 >= MAX_ROWS - 1
    }
    pub(super) fn selecao_e_linhas_inteiras(&self) -> bool {
        self.coluna == 0 && self.fim_selecao.1 >= MAX_COLS - 1
    }

    /// Ajusta todas as colunas de `c0..=c1` numa única operação desfazível.
    pub(super) fn ajustar_colunas(&mut self, c0: u32, c1: u32) {
        self.confirmar();
        let (c0, c1) = (c0.min(c1), c0.max(c1));
        let valores = larguras_ideais(&self.workbook, self.folha, c0, c1);
        let mut mudou_algo = false;
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for &(c, px) in &valores {
                mudou_algo |= sheet.definir_dimensao(true, c, px);
            }
            sheet.fechar_transacao();
        });
        if mudou_algo { self.alterado = true; }
        self.status = match valores.as_slice() {
            [(_, px)] => trf!("Largura ajustada: {px} px"),
            v => trf!("{} colunas ajustadas ao conteúdo", v.len()),
        };
    }

    pub(super) fn ajustar_linhas(&mut self, r0: u32, r1: u32) {
        self.confirmar();
        let (r0, r1) = (r0.min(r1), r0.max(r1));
        let valores = alturas_ideais(&self.workbook, self.folha, r0, r1);
        let mut mudou_algo = false;
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for &(r, px) in &valores {
                mudou_algo |= sheet.definir_dimensao(false, r, px);
            }
            sheet.fechar_transacao();
        });
        if mudou_algo { self.alterado = true; }
        self.status = match valores.as_slice() {
            [(_, px)] => trf!("Altura ajustada: {px} px"),
            v => trf!("{} linhas ajustadas ao conteúdo", v.len()),
        };
    }

    /// Comando "Ajustar largura": todas as colunas da seleção.
    pub(super) fn ajustar_largura_da_selecao(&mut self) {
        let (c0, c1) = self.colunas_da_selecao();
        self.ajustar_colunas(c0, c1);
    }

    /// Divisória arrastada até `px`: vale para todas as colunas (linhas)
    /// inteiras selecionadas quando a arrastada está entre elas, como no
    /// Excel. Abaixo de 4 px oculta. Um Ctrl+Z desfaz o lote.
    pub(super) fn aplicar_dimensao_arrastada(&mut self, coluna: bool, indice: u32, px: u32) {
        self.confirmar();
        let a = self.area_selecionada();
        let (inteiras, inicio, fim) = if coluna {
            (self.selecao_e_colunas_inteiras(), a.c0, a.c1)
        } else { (self.selecao_e_linhas_inteiras(), a.r0, a.r1) };
        let (de, ate) = if inteiras && (inicio..=fim).contains(&indice) && fim - inicio < 16_384 {
            (inicio, fim)
        } else { (indice, indice) };
        let nome = |i: u32| if coluna { feathersheet_core::celulas::refs::col_name(i) } else { (i + 1).to_string() };
        if px < 4 {
            let mut r = Ok(0);
            self.workbook.editar(self.folha, |s| { r = s.alterar_visibilidade_dimensoes(coluna, true, de, ate); });
            self.status = match r {
                Ok(n) if n > 0 => { self.alterado = true; trf!("{n} {} ocultada(s)", if coluna { "coluna(s)" } else { "linha(s)" }) }
                Ok(_) => tr!("Nada a ocultar").into(),
                Err(e) => e,
            };
            return;
        }
        let mut mudou = false;
        self.workbook.editar(self.folha, |s| {
            s.abrir_transacao();
            for i in de..=ate { mudou |= s.definir_dimensao(coluna, i, px); }
            s.fechar_transacao();
        });
        self.alterado |= mudou;
        let alvo = if de == ate { nome(de) } else { trf!("{} a {}", nome(de), nome(ate)) };
        self.status = trf!("{} de {alvo}: {px} px", if coluna { tr!("Largura") } else { tr!("Altura") });
    }

    /// Duplo clique numa divisória: lote se o cabeçalho está na seleção.
    pub(super) fn ajustar_pela_divisoria(&mut self, coluna: bool, indice: u32) {
        if coluna {
            let (c0, c1) = self.colunas_da_selecao();
            if self.selecao_e_colunas_inteiras() && (c0..=c1).contains(&indice) {
                self.ajustar_colunas(c0, c1);
            } else { self.ajustar_colunas(indice, indice); }
        } else {
            let a = self.area_selecionada();
            if self.selecao_e_linhas_inteiras() && (a.r0..=a.r1).contains(&indice) {
                self.ajustar_linhas(a.r0, a.r1);
            } else { self.ajustar_linhas(indice, indice); }
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app_com_dados() -> App {
        let mut app = App::default();
        for (c, t) in [(0u32, "abc"), (1, "uma frase bem mais longa aqui"), (2, "x")] {
            app.workbook.escrever_celula(0, 0, c, t.into());
        }
        app
    }
    fn larg(app: &App, c: u32) -> u32 {
        app.workbook.aba(0).unwrap().sheet.col_widths.get(&c).copied()
            .unwrap_or(LARGURA_PADRAO as u32)
    }

    #[test]
    fn varias_colunas_ajustam_e_um_desfazer_volta_todas() {
        let mut app = app_com_dados();
        let _ = app.update(Msg::SelecionarColuna(0));
        let _ = app.update(Msg::EstenderColuna(2));
        let _ = app.update(Msg::AjustarLargura);
        assert_eq!(larg(&app, 1), (29.0f32 * 7.3 + 20.0).ceil() as u32);
        assert_eq!(larg(&app, 0), 48);
        assert_eq!(larg(&app, 2), 48);
        let _ = app.update(Msg::Desfazer);
        for c in 0..3 { assert_eq!(larg(&app, c), LARGURA_PADRAO as u32, "coluna {c}"); }
    }

    #[test]
    fn arrastar_coluna_inteira_nao_leva_a_vizinha_mesclada() {
        // A1:B1 mesclada (como o título da v6.0a): escolher a coluna B e
        // arrastar a divisória dela muda só a B.
        let mut app = app_com_dados();
        let _ = app.update(Msg::IrPara("A1:B1".into()));
        let _ = app.update(Msg::Mesclar);
        let _ = app.update(Msg::SelecionarColuna(1));
        assert_eq!((app.area_selecionada().c0, app.area_selecionada().c1), (1, 1));
        let _ = app.update(Msg::DimensaoArrastada(true, 1, 300));
        assert_eq!(larg(&app, 1), 300);
        assert_eq!(larg(&app, 0), LARGURA_PADRAO as u32);
    }

    #[test]
    fn planilha_inteira_ajusta_so_colunas_usadas() {
        let mut app = app_com_dados();
        let _ = app.update(Msg::SelecionarTudo);
        let _ = app.update(Msg::AjustarLargura);
        let sheet = &app.workbook.aba(0).unwrap().sheet;
        assert_eq!(sheet.col_widths.len(), 3);
        assert!(sheet.col_widths.keys().all(|&c| c <= 2));
    }

    #[test]
    fn duplo_clique_na_divisoria_respeita_a_selecao() {
        let mut app = app_com_dados();
        let _ = app.update(Msg::SelecionarColuna(0));
        let _ = app.update(Msg::EstenderColuna(1));
        // fora da seleção: só a coluna 2
        let _ = app.update(Msg::AjustarDivisoria(true, 2));
        assert_eq!(larg(&app, 2), 48);
        assert_eq!(larg(&app, 0), LARGURA_PADRAO as u32);
        // dentro da seleção: as duas
        let _ = app.update(Msg::AjustarDivisoria(true, 0));
        assert_eq!(larg(&app, 0), 48);
        assert!(larg(&app, 1) > 200);
    }

    #[test]
    fn arrastar_divisoria_vale_para_a_selecao_e_um_desfazer_volta() {
        let mut app = app_com_dados();
        let _ = app.update(Msg::DimensaoArrastada(true, 4, 200));
        assert_eq!(larg(&app, 4), 200, "fora da seleção: só a arrastada");
        let _ = app.update(Msg::SelecionarColuna(0));
        let _ = app.update(Msg::EstenderColuna(2));
        let _ = app.update(Msg::DimensaoArrastada(true, 1, 90));
        for c in 0..3 { assert_eq!(larg(&app, c), 90, "coluna {c}"); }
        let _ = app.update(Msg::Desfazer);
        for c in 0..3 { assert_eq!(larg(&app, c), LARGURA_PADRAO as u32, "coluna {c}"); }
        let _ = app.update(Msg::DimensaoArrastada(false, 3, 2));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.row_heights.get(&3), Some(&0), "arrastar até quase zero oculta");
    }

    #[test]
    fn linhas_com_quebra_crescem_em_lote() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "a\nb\nc".into());
        app.workbook.escrever_celula(0, 1, 0, "d\ne".into());
        let _ = app.update(Msg::SelecionarLinha(0));
        let _ = app.update(Msg::EstenderLinha(1));
        let _ = app.update(Msg::AjustarDivisoria(false, 1));
        let h = |app: &App, r| app.workbook.aba(0).unwrap().sheet.row_heights.get(&r).copied();
        assert_eq!(h(&app, 0), Some(64));
        assert_eq!(h(&app, 1), Some(46));
    }
}
