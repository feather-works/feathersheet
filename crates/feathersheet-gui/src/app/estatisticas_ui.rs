//! Diálogo de Estatísticas portado de app-dialogs.ts para a GUI Iced.
//! Reutiliza o agregador do core; nenhuma segunda implementação de cálculos.
use super::*;
use feathersheet_core::{operacoes::ops::{self, ColumnStats}, Value};

/// Evita copiar milhões de células e criar HashSets gigantes no event loop.
const LIMITE_CELULAS: u64 = 100_000;
const LIMIAR_ASSINCRONO: u64 = 4_096;

#[derive(Debug, Clone)]
pub struct Visao {
    pub(super) area: Rect,
    pub(super) dados: ColumnStats,
}
#[derive(Debug, Clone)]
pub(super) enum Estado {
    Calculando { area: Rect },
    Prontas(Visao),
}
fn vazio() -> ColumnStats {
    ColumnStats {
        count: 0, numbers: 0, empty: 0, distinct: 0,
        sum: 0.0, average: 0.0, min: 0.0, max: 0.0,
    }
}
/// Formatação de números do diálogo TS em português brasileiro.
fn agrupar(inteiro: &str) -> String {
    let (sinal, digitos) = if let Some(resto) = inteiro.strip_prefix('-') {
        ("-", resto)
    } else { ("", inteiro) };
    let mut inverso = String::new();
    for (i, c) in digitos.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { inverso.push('.'); }
        inverso.push(c);
    }
    format!("{sinal}{}", inverso.chars().rev().collect::<String>())
}
fn inteiro_pt(numero: usize) -> String { agrupar(&numero.to_string()) }
fn numero_pt(numero: f64) -> String {
    if !numero.is_finite() { return numero.to_string(); }
    let texto = format!("{numero:.3}");
    let texto = texto.trim_end_matches('0').trim_end_matches('.');
    match texto.split_once('.') {
        Some((parte_inteira, decimal)) if !decimal.is_empty() =>
            format!("{},{}", agrupar(parte_inteira), decimal),
        _ => agrupar(texto),
    }
}
/// Os pares do diálogo, em dois grupos. "Vazias" conta toda a seleção (o
/// motor para na última linha usada, de propósito, para não varrer um milhão
/// de linhas vazias); média, mínimo e máximo sem números viram "—".
/// Pares rótulo/valor de um grupo do diálogo.
type Pares = Vec<(String, String)>;

fn itens(area: &Rect, dados: &ColumnStats) -> (Pares, Pares) {
    let n = area.normalized();
    let total = u64::from(n.r1 - n.r0 + 1) * u64::from(n.c1 - n.c0 + 1);
    let vazias = total.saturating_sub(dados.count as u64);
    let sem_numero = dados.numbers == 0;
    let ou_traco = |v: f64| if sem_numero { "—".to_string() } else { numero_pt(v) };
    let contagens = vec![
        (tr!("Células preenchidas").to_string(), inteiro_pt(dados.count)),
        (tr!("Vazias").to_string(), agrupar(&vazias.to_string())),
        (tr!("Números").to_string(), inteiro_pt(dados.numbers)),
        (tr!("Valores distintos").to_string(), inteiro_pt(dados.distinct)),
    ];
    let numeros = vec![
        (tr!("Soma").to_string(), numero_pt(dados.sum)),
        (tr!("Média").to_string(), ou_traco(dados.average)),
        (tr!("Mínimo").to_string(), ou_traco(dados.min)),
        (tr!("Máximo").to_string(), ou_traco(dados.max)),
    ];
    (contagens, numeros)
}

impl App {
    pub(super) fn abrir_estatisticas(&mut self) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        let area = self.area_selecionada().normalized();
        let Some(aba) = self.workbook.aba(self.folha) else {
            self.status = tr!("Nenhuma planilha disponível para analisar.").into();
            return Task::none();
        };
        let vazia = aba.sheet.is_empty();
        let ultima = aba.sheet.used_rect().r1;
        let r1 = area.r1.min(ultima);
        let linhas = if vazia || area.r0 > r1 { 0 } else { u64::from(r1 - area.r0) + 1 };
        let colunas = u64::from(area.c1 - area.c0) + 1;
        let celulas = linhas.saturating_mul(colunas);
        if celulas > LIMITE_CELULAS {
            self.status = trf!(
                "Seleção com {celulas} células: limite de {} para Estatísticas. Reduza a seleção.",
                inteiro_pt(LIMITE_CELULAS as usize)
            );
            return Task::none();
        }
        // Não manter simultaneamente dois painéis analíticos sobre a grade.
        self.perfil_geracao = self.perfil_geracao.wrapping_add(1);
        self.perfil_coluna = None;
        self.estatisticas_geracao = self.estatisticas_geracao.wrapping_add(1);
        let geracao = self.estatisticas_geracao;
        self.menu_aberto = false;
        self.contexto = None;
        if celulas == 0 {
            self.estatisticas = Some(Estado::Prontas(Visao { area, dados: vazio() }));
            return Task::none();
        }

        // Retângulo efetivo, cortado na última linha utilizada do motor.
        let medida = Rect::new(area.r0, area.c0, r1, area.c1);
        if celulas <= LIMIAR_ASSINCRONO {
            let dados = ops::column_stats_com(medida, ultima,
                |r, c| self.workbook.valor(self.folha, r, c));
            self.estatisticas = Some(Estado::Prontas(Visao { area, dados }));
            self.status = tr!("Estatísticas calculadas sobre a seleção.").into();
            return Task::none();
        }

        // A leitura limitada acontece uma vez só, antes da tarefa em background.
        // O cálculo mais caro (HashSet de valores distintos) roda fora da UI.
        let mut valores = Vec::with_capacity(celulas as usize);
        for r in medida.r0..=medida.r1 {
            for c in medida.c0..=medida.c1 {
                valores.push(self.workbook.valor(self.folha, r, c));
            }
        }
        let largura = colunas as usize;
        let linha_inicial = medida.r0;
        let coluna_inicial = medida.c0;
        self.estatisticas = Some(Estado::Calculando { area: area.clone() });
        self.status = trf!("Calculando estatísticas de {} células…", inteiro_pt(celulas as usize));
        Task::perform(async move {
            tokio::task::spawn_blocking(move || {
                let dados = ops::column_stats_com(medida, ultima, |r, c| {
                    let offset = ((r - linha_inicial) as usize)
                        .saturating_mul(largura)
                        .saturating_add((c - coluna_inicial) as usize);
                    valores.get(offset).cloned().unwrap_or(Value::Empty)
                });
                Visao { area, dados }
            }).await.map_err(|erro| erro.to_string())
        }, move |resultado| Msg::EstatisticasProntas(geracao, resultado))
    }

    fn view_estatisticas(&self, estado: &Estado) -> super::dialogo_interno::Dialogo<'_, Msg> {
        use super::dialogo_interno::dlg;
        let area = match estado {
            Estado::Calculando { area } => area,
            Estado::Prontas(visao) => &visao.area,
        };
        let titulo_area = endereco_area(&area);
        let corpo: Element<'_, Msg> = match estado {
            Estado::Calculando { .. } => dlg::aviso_vazio(tr!("Calculando contagens e valores distintos…")),
            Estado::Prontas(visao) => {
                let (contagens, numeros) = itens(&visao.area, &visao.dados);
                iced::widget::column![
                    dlg::secao(tr!("Contagens")),
                    dlg::grade_rotulo_valor(contagens, 2),
                    dlg::secao(tr!("Números")),
                    dlg::grade_rotulo_valor(numeros, 2),
                ].spacing(8).into()
            }
        };
        self.dialogo(tr!("Estatísticas da seleção"), corpo)
            .icone("chart-bar")
            .subtitulo(trf!("Intervalo {titulo_area} · somente leitura, não altera valores nem fórmulas"))
            .largura(460.0)
            .ao_fechar(Msg::EstatisticasFechar)
    }

    pub(super) fn sobrepor_estatisticas<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let Some(estado) = &self.estatisticas else { return base };
        self.view_estatisticas(estado).sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn estatisticas_da_selecao_multicoluna_usa_core_rust() {
        let mut app = App::default();
        for (r,c,v) in [(0,0,"10"),(0,1,"2"),(1,0,"20"),(2,0,"20"),(2,1,"texto")] {
            app.workbook.escrever_celula(0,r,c,v.into());
        }
        app.update(Msg::IrPara("A1:B3".into()));
        app.update(Msg::EstatisticasAbrir);
        let Some(Estado::Prontas(visao)) = &app.estatisticas else { panic!("diálogo não abriu") };
        assert_eq!((visao.dados.count, visao.dados.numbers, visao.dados.empty), (5, 4, 1));
        assert_eq!(visao.dados.distinct, 4);
        assert_eq!((visao.dados.sum, visao.dados.average), (52.0, 13.0));
        assert_eq!((visao.dados.min, visao.dados.max), (2.0, 20.0));
        let _: Element<'_, Msg> = app.view_estatisticas(app.estatisticas.as_ref().unwrap()).painel();
        assert!(!app.alterado);
    }

    #[test]
    fn estatisticas_e_perfil_sao_acoes_independentes() {
        let app = App::default();
        let legado = menu_nativo::Acao { id: "columnStats".into(), args: vec![] };
        assert!(matches!(app.evento_menu(&legado), Some(Msg::EstatisticasAbrir)));
        let mut app = app;
        app.workbook.escrever_celula(0, 0, 0, "1".into());
        app.update(Msg::EstatisticasAbrir);
        assert!(app.estatisticas.is_some());
        assert!(app.perfil_coluna.is_none());
        app.update(Msg::EstatisticasFechar);
        assert!(app.estatisticas.is_none());
        app.update(Msg::PerfilColunaAbrir);
        assert!(app.perfil_coluna.is_some());
        assert!(app.estatisticas.is_none());
    }

    #[test]
    fn estatisticas_limitam_selecao_sem_truncamento_silencioso() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 100_001, 0, "2".into());
        app.update(Msg::IrPara("A1:A100002".into()));
        app.update(Msg::EstatisticasAbrir);
        assert!(app.estatisticas.is_none());
        assert!(app.status.contains("limite"));
        assert!(!app.alterado);
    }

    #[test]
    fn celula_vazia_conta_como_vazia_e_sem_numeros_mostra_traco() {
        let mut app = App::default();
        app.update(Msg::IrPara("B7".into()));
        app.update(Msg::EstatisticasAbrir);
        let Some(Estado::Prontas(visao)) = &app.estatisticas else { panic!("diálogo não abriu") };
        let (contagens, numeros) = itens(&visao.area, &visao.dados);
        assert_eq!(contagens[1], ("Vazias".to_string(), "1".to_string()));
        assert_eq!(numeros[0].1, "0", "soma vazia é zero");
        assert!(numeros[1..].iter().all(|(_, v)| v == "—"), "{numeros:?}");
        // Seleção maior que a área usada: as linhas abaixo também são vazias.
        app.workbook.escrever_celula(0, 0, 0, "4".into());
        app.update(Msg::IrPara("A1:A10".into()));
        app.update(Msg::EstatisticasAbrir);
        let Some(Estado::Prontas(visao)) = &app.estatisticas else { panic!() };
        let (contagens, numeros) = itens(&visao.area, &visao.dados);
        assert_eq!(contagens[1].1, "9");
        assert_eq!(numeros[1].1, "4");
    }

    #[test]
    fn numeros_portugueses_preservam_grupos_e_casas() {
        assert_eq!(inteiro_pt(1234567), "1.234.567");
        assert_eq!(numero_pt(1234.5), "1.234,5");
        assert_eq!(numero_pt(0.0), "0");
    }

    #[test]
    fn tarefas_antigas_nao_reabrem_a_janela_apos_fechar() {
        let mut app = App::default();
        app.estatisticas = Some(Estado::Calculando { area: Rect::new(0, 0, 0, 0) });
        let geracao = app.estatisticas_geracao;
        app.update(Msg::EstatisticasFechar);
        app.update(Msg::EstatisticasProntas(geracao,
            Ok(Visao { area: Rect::new(0, 0, 0, 0), dados: vazio() })));
        assert!(app.estatisticas.is_none());
    }
}
