//! Barra de abertura no rodapé: quanto do arquivo já foi lido.
//!
//! Os leitores do `feathersheet-io` somam os bytes em
//! [`feathersheet_io::progresso`]; aqui só se mostra, a cada 100 ms.
use super::*;
use feathersheet_io::progresso::{self, Fase};
use std::sync::atomic::{AtomicU32, Ordering};

/// O percentual que a barra mostra (bits de f32). Persegue o real a cada
/// pulso, para a barra deslizar em vez de pular.
static EXIBIDO: AtomicU32 = AtomicU32::new(0);

/// A porcentagem da abertura inteira: ler o arquivo, montar na memória e
/// calcular, cada etapa com o seu trecho da barra. O `.xlsx` não conta a
/// leitura em bytes (a montagem lê o XML): a montagem vai de 0 a 80%.
fn porcentagem() -> f64 {
    let (lidos, total, fase, _) = progresso::agora();
    let fracao = |feito: u64, total: u64| if total > 0 { (feito as f64 / total as f64).min(1.0) } else { 0.0 };
    // Onde a montagem começa: sem leitura contada (xlsx), no zero.
    let inicio_montar = if lidos > 0 { 40.0 } else { 0.0 };
    match fase {
        Fase::Lendo => fracao(lidos, total) * 40.0,
        Fase::Montando => {
            let (feito, total) = progresso::etapa();
            inicio_montar + fracao(feito, total) * (80.0 - inicio_montar)
        }
        Fase::Calculando => {
            let (feito, total) = feathersheet_core::paralelismo::andamento::agora();
            80.0 + fracao(feito, total) * 20.0
        }
    }
}

/// Um passo da animação; chamado pelo `Msg::ProgressoPulso`.
pub(super) fn animar() {
    let alvo = porcentagem() as f32;
    let atual = f32::from_bits(EXIBIDO.load(Ordering::Relaxed));
    // Arquivo novo (o real voltou para trás): recomeça do zero, sem deslizar para trás.
    let novo = if alvo + 1.0 < atual { 0.0 } else { atual + (alvo - atual) * 0.2 };
    EXIBIDO.store(novo.to_bits(), Ordering::Relaxed);
}

impl App {
    pub(super) fn view_progresso_abertura(&self) -> Element<'_, Msg> {
        if !self.carregando {
            return Space::new(0.0, 0.0).into();
        }
        let (lidos, total, fase, nome) = progresso::agora();
        let mb = |b: u64| b as f64 / 1_048_576.0;
        let linhas = |n: u64| match n {
            0..=9_999 => n.to_string(),
            10_000..=999_999 => trf!("{} mil", n / 1_000),
            _ => trf!("{:.1} mi", n as f64 / 1e6).replace('.', ","),
        };
        let (pct, texto) = match fase {
            Fase::Lendo if total > 0 => {
                let pct = porcentagem();
                let quanto = if progresso::em_linhas() {
                    trf!("{} de {} linhas", linhas(lidos), linhas(total))
                } else {
                    trf!("{:.0} de {:.0} MB", mb(lidos), mb(total))
                };
                (pct, trf!("Abrindo {nome} · {pct:.0}% · {quanto}"))
            }
            Fase::Lendo => (0.0, trf!("Abrindo {nome}…")),
            Fase::Montando => (0.0, trf!("Montando {nome} na memória · {:.0}%", porcentagem())),
            Fase::Calculando => (0.0, trf!("Calculando as fórmulas de {nome} em {} núcleos · {:.0}%",
                feathersheet_core::paralelismo::nucleos(), porcentagem())),
        };
        let _ = pct;
        let exibido = f32::from_bits(EXIBIDO.load(Ordering::Relaxed));
        row![
            pelicano::progress_bar(0.0..=100.0, exibido)
                // shortcut: o trilho é o do pelicano (texto do tema a 12%), não COR_BORDA; trocar quando a ProgressBar aceitar cor de fundo.
                .width(Length::Fixed(150.0)).height(6.0)
                .color(COR_AZUL_ACENTO()),
            text(texto).size(11).color(COR_TEXTO_MUTED()),
        ].spacing(8).align(pelicano::Align::Center).into()
    }
}

