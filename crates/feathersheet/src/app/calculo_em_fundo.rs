//! Fórmula nova em milhões de linhas (Ctrl+D, alça, MCP) calcula em paralelo noutra thread,
//! como a abertura do arquivo. Sem isso as células calculavam uma a uma ao serem lidas: o
//! índice do `MÍNIMOSES` de 13,7 milhões de linhas travava a janela 16 s, e a dinâmica que lia
//! a coluna passava de 9 GB (28/10/2026). A pasta vai para a thread e volta no fim, como na
//! macro em fundo; enquanto isso a tela mostra o espelho e o aviso.
use super::*;
use std::sync::mpsc;

/// Abaixo disto o cálculo preguiçoso das células visíveis dá conta.
pub(super) const CELULAS_EM_FUNDO: usize = 200_000;

pub(in crate::app) struct CalculoRodando {
    inicio: std::time::Instant,
    recebe: mpsc::Receiver<Workbook>,
    parando: bool,
}

impl App {
    /// Depois de escrever `celulas` fórmulas de uma vez: muitas, calcula em fundo.
    pub(in crate::app) fn calcular_se_grande(&mut self, celulas: usize) {
        if celulas >= CELULAS_EM_FUNDO { self.calcular_em_fundo(); }
    }

    pub(in crate::app) fn calcular_em_fundo(&mut self) {
        if self.macro_rodando.is_some() || self.calculo_rodando.is_some() { return; }
        let espelho = super::macro_dialogo::em_fundo::espelho_da_pasta(&self.workbook);
        let mut pasta = std::mem::replace(&mut self.workbook, espelho);
        let (envio, recebe) = mpsc::channel();
        std::thread::spawn(move || {
            // Pânico no cálculo não pode levar a pasta embora: ela volta, e o que faltou
            // fica sujo para o cálculo preguiçoso.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| pasta.calcular_em_paralelo()));
            let _ = envio.send(pasta);
        });
        self.status = tr!("Calculando as fórmulas…").into();
        self.calculo_rodando = Some(CalculoRodando { inicio: std::time::Instant::now(), recebe, parando: false });
        #[cfg(test)]
        self.conferir_calculo_em_fundo(true);
    }

    /// `Msg::CalculoPulso`: o cálculo terminou? `esperar` bloqueia até terminar (testes).
    pub(in crate::app) fn conferir_calculo_em_fundo(&mut self, esperar: bool) {
        let Some(c) = self.calculo_rodando.as_ref() else { return };
        let pasta = if esperar { c.recebe.recv().ok() } else { c.recebe.try_recv().ok() };
        let Some(pasta) = pasta else { return };
        let segundos = c.inicio.elapsed().as_secs_f32();
        let parou = c.parando;
        self.calculo_rodando = None;
        self.workbook = pasta;
        self.cache_grade.desenho.clear();
        self.status = if parou {
            trf!("Cálculo parado em {segundos:.1} s: o que faltou calcula quando a célula aparecer; Ctrl+Z desfaz o preenchimento.")
        } else { trf!("Fórmulas calculadas em {segundos:.1} s.") };
        self.trocar_selecao(self.linha, self.coluna);
    }

    /// `Msg::CalculoParar`: termina a fatia em curso e devolve a pasta.
    pub(in crate::app) fn parar_calculo_em_fundo(&mut self) {
        if let Some(c) = self.calculo_rodando.as_mut() {
            c.parando = true;
            feathersheet_core::paralelismo::andamento::pedir_parar();
            self.status = tr!("Parando o cálculo…").into();
        }
    }

    pub(in crate::app) fn sobrepor_calculo_rodando<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(c) = &self.calculo_rodando else { return base };
        let (feito, total) = feathersheet_core::paralelismo::andamento::agora();
        let milhoes = |n: u64| format!("{:.1}", n as f64 / 1e6).replace('.', ",");
        let corpo = pelicano::column![
            text(trf!("{} s", c.inicio.elapsed().as_secs())).size(28),
            text(trf!("{} de {} milhões de células · {} núcleos", milhoes(feito), milhoes(total),
                feathersheet_core::paralelismo::nucleos())).size(13),
            text(if c.parando { tr!("Parando… o que faltou calcula quando a célula aparecer.") }
                else { tr!("A planilha volta assim que as contas terminarem. Menos núcleos: Configurações → Desempenho.") })
                .size(12).color(COR_TEXTO_MUTED()),
        ].spacing(6);
        self.dialogo(tr!("Calculando as fórmulas"), corpo)
            .icone("math-function")
            .largura(420.0)
            .clique_fora_fecha(false)
            .primario(tr!("Parar"), (!c.parando).then_some(Msg::CalculoParar))
            .sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn preencher_muitas_formulas_calcula_em_fundo_e_devolve_a_pasta() {
        let mut app = App::default();
        // De 1 a 7: critério 0 não usa o índice do MÍNIMOSES e varreria a coluna por linha.
        let n = CELULAS_EM_FUNDO as u32 + 10;
        app.workbook.aba_mut(0).unwrap().sheet.gravar_bloco_literais(0, 0,
            (0..n).map(|r| vec![(r % 7 + 1).to_string()]).collect());
        app.workbook.escrever_celula(0, 0, 1, "=MÍNIMOSES(A:A;A:A;A1)+1".into());
        app.linha = 0; app.coluna = 1; app.fim_selecao = (n - 1, 1);
        let _ = app.update(Msg::Preencher(DirecaoPreenchimento::Baixo));
        assert!(app.calculo_rodando.is_none(), "o teste espera a pasta voltar");
        assert!(app.status.contains("calculadas"), "{}", app.status);
        assert!(!app.workbook.calculo_pendente(0, n - 1, 1), "a última linha já veio calculada");
        assert_eq!(app.workbook.valor(0, n - 1, 1), feathersheet_core::Value::Number(((n - 1) % 7 + 2) as f64));
    }
}
