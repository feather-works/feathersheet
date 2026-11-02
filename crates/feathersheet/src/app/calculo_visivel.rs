//! O cálculo das células visíveis fica fora do desenho: a grade mostra "…"
//! no que ainda espera conta, e o app calcula aqui, um pouco por vez, entre
//! um evento e outro. Antes a grade calculava dentro do `draw`, e uma aba
//! com `SOMASES` sobre 1 milhão de linhas deixava a janela "não está
//! respondendo" (13/10/2026).
use std::time::{Duration, Instant};

use super::App;

/// Quanto cada rodada pode gastar antes de devolver a janela aos eventos.
/// Uma célula começada vai até o fim, então uma conta pesada passa disso.
pub(super) const ORCAMENTO: Duration = Duration::from_millis(30);

impl App {
    /// Calcula as células visíveis que esperam conta até gastar `orcamento`;
    /// sobrou alguma, liga o pulso que chama de novo no quadro seguinte.
    pub(super) fn calcular_visiveis(&mut self, orcamento: Duration) {
        if self.carregando || self.mostrar_formulas {
            self.calculo_pendente = false;
            return;
        }
        let aba = self.folha;
        let Some(usado) = self.workbook.aba(aba).map(|a| a.sheet.used_rect()) else {
            self.calculo_pendente = false;
            return;
        };
        let (fixas_l, fixas_c) = self.congelamento_atual();
        // Generoso de propósito: com zoom pequeno cabem mais células, e
        // calcular algumas além da borda não custa quase nada.
        let zoom = (self.config.zoom as f32 / 100.0).max(0.25);
        let (linhas, colunas) = ((90.0 / zoom) as u32, (40.0 / zoom) as u32);
        // Linha/coluna oculta (filtro, ocultar) não conta: com filtro, as 90
        // linhas visíveis podem estar espalhadas por milhares de índices.
        let mut eixos = self.eixos_grade.borrow_mut();
        let (fim_l, fim_c) = (usado.r1.saturating_add(1), usado.c1.saturating_add(1));
        let mut rs = eixos.visiveis_desde(&self.workbook, aba, false, 0, fixas_l.min(fim_l), fixas_l);
        rs.extend(eixos.visiveis_desde(&self.workbook, aba, false, self.linha_inicio, fim_l, linhas));
        let mut cs = eixos.visiveis_desde(&self.workbook, aba, true, 0, fixas_c.min(fim_c), fixas_c);
        cs.extend(eixos.visiveis_desde(&self.workbook, aba, true, self.coluna_inicio, fim_c, colunas));
        drop(eixos);
        let inicio = Instant::now();
        let mut resta = false;
        'fora: for &r in &rs {
            for &c in &cs {
                if !self.workbook.calculo_pendente(aba, r, c) {
                    continue;
                }
                if inicio.elapsed() >= orcamento {
                    resta = true;
                    break 'fora;
                }
                let _ = self.workbook.valor(aba, r, c);
            }
        }
        self.calculo_pendente = resta;
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::app::Msg;

    #[test]
    fn mensagem_calcula_o_visivel_e_desliga_o_pulso() {
        let mut app = App::default();
        for r in 0..50u32 {
            app.workbook.escrever_celula(0, r, 0, format!("{}", r + 1));
            app.workbook.escrever_celula(0, r, 1, format!("=A{}*2", r + 1));
        }
        assert!(app.workbook.calculo_pendente(0, 49, 1));
        let _ = app.update(Msg::CalcularPulso);
        assert!(!app.workbook.calculo_pendente(0, 49, 1), "a célula visível foi calculada");
        assert!(!app.calculo_pendente, "nada sobrou: o pulso desliga");
    }

    #[test]
    fn orcamento_esgotado_deixa_o_pulso_ligado() {
        let mut app = App::default();
        for r in 0..50u32 {
            app.workbook.escrever_celula(0, r, 1, format!("=A{}*2", r + 1));
        }
        app.calcular_visiveis(Duration::ZERO);
        assert!(app.calculo_pendente, "sem tempo para calcular, o pulso continua");
    }

    #[test]
    fn com_linhas_ocultas_calcula_as_visiveis_la_embaixo() {
        let mut app = App::default();
        for r in 0..3000u32 {
            app.workbook.escrever_celula(0, r, 1, format!("=A{}*2", r + 1));
        }
        // Como um filtro: só 0 e 2500 à mostra.
        if let Some(a) = app.workbook.aba_mut(0) {
            for r in 1..2500u32 { a.sheet.row_heights.insert(r, 0); }
        }
        app.calcular_visiveis(Duration::from_secs(5));
        assert!(!app.workbook.calculo_pendente(0, 2500, 1), "a linha visível depois das ocultas foi calculada");
    }
}
