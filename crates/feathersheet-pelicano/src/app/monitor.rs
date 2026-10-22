//! Monitor F12. A janela inteira (memória, CPU, console, gráfico) é do
//! `feather_ui::monitor`; aqui o FeatherSheet só entrega o que é dele: o
//! diário do motor no console e as métricas do documento.
use super::{App, Msg};
use feather_ui::monitor::{Metricas, Nivel};
use pelicano::Element;

impl App {
    /// Métricas que o motor anotou (abrir, desfazer…) vão para o Console.
    pub(super) fn recolher_diario_motor(&mut self) {
        for linha in crate::vigia::recolher() {
            self.monitor.registrar(Nivel::Aviso, "Tela", &linha);
        }
        for (modelo, linha) in feathersheet_core::diario::recolher_com_modelo() {
            let linha = match modelo {
                Some(modelo) => crate::idioma::traduzir_formatado(modelo, linha),
                None => linha,
            };
            self.monitor.registrar(Nivel::Info, tr!("Motor"), &linha);
        }
        // Diagnóstico da caneta (FEATHERSHEET_CANETA_DIAG=1): eventos crus
        // da camada e do XInput2. Também no terminal, para o usuário mandar.
        for linha in feather_ui::caneta::recolher_diagnostico() {
            if super::desenhar::diagnostico_no_terminal() {
                println!("{linha}");
            }
            self.monitor.registrar(Nivel::Info, tr!("Caneta"), &linha);
        }
    }

    pub(super) fn view_monitor(&self) -> Element<'_, Msg> {
        let celulas: usize = (0..self.workbook.len())
            .filter_map(|i| self.workbook.aba(i))
            .map(|a| a.sheet.len())
            .sum();
        let metricas = Metricas {
            cartoes: vec![
                (tr!("Abas").into(), self.workbook.len().to_string(), tr!("No documento atual").into()),
                (tr!("Células ocupadas").into(), celulas.to_string(), tr!("Dados e fórmulas").into()),
            ],
            linhas: vec![
                (tr!("Documento").into(), self.nome_documento()),
                (tr!("Estado").into(), if self.alterado { tr!("Alterações não salvas") } else { tr!("Salvo") }.into()),
                (tr!("Operação").into(), if self.carregando { tr!("Importando planilha") } else { tr!("Pronto") }.into()),
            ],
            ocupado: self.carregando.then(|| tr!("Importação em andamento").into()),
        };
        self.monitor.view(metricas, Msg::MonitorEvento)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::EventoMonitor;

    #[test]
    fn monitor_renderiza_as_tres_abas() {
        let mut app = App::default();
        for aba in 0..3 {
            let _ = app.update(Msg::MonitorEvento(EventoMonitor::Aba(aba)));
            let _view = app.view_monitor();
        }
    }

    #[test]
    fn periodo_20_40_80_e_alterado_sem_modificar_a_planilha() {
        let mut app = App::default();
        let nomes = app.workbook.nomes();
        for n in [20, 40, 80] {
            let _ = app.update(Msg::MonitorEvento(EventoMonitor::AlternarAmostras));
            assert!(app.monitor.menu_amostras_aberto());
            drop(app.view_monitor());
            let _ = app.update(Msg::MonitorEvento(EventoMonitor::Amostras(n)));
            assert_eq!(app.monitor.amostras(), n);
        }
        assert_eq!(app.workbook.nomes(), nomes);
        assert!(!app.alterado);
    }

    #[test]
    fn console_recebe_log_real_de_macro_e_repeticao() {
        let mut app = App::default();
        app.macro_visual.documento = "modulo:teste".into();
        app.macro_codigo =
            pelicano::Content::with_text("log('mensagem de teste');");
        let _ = app.update(Msg::MacroExecutar);
        let _ = app.update(Msg::MacroExecutar);
        assert_eq!(
            app.monitor.console.registros()
                .filter(|r| r.origem == "Macros" && r.mensagem.contains("mensagem de teste"))
                .count(),
            2
        );
        let _ = app.update(Msg::Aberto(Err("Falha após pausar rolagem".into())));
        assert!(app.monitor.console.registros().last().unwrap().mensagem.contains("pausar rolagem"));
    }

    #[test]
    fn console_recebe_erros_repetidos_sem_f12_aberto() {
        let mut app = App::default();
        let erro = "Arquivo ZIP corrompido".to_owned();
        let _ = app.update(Msg::Aberto(Err(erro.clone())));
        let _ = app.update(Msg::Aberto(Err(erro)));
        assert_eq!(app.monitor.console.erros(), 2);
        let _ = app.update(Msg::MonitorEvento(EventoMonitor::Limpar));
        assert_eq!(app.monitor.console.registros().count(), 0);
    }
}
