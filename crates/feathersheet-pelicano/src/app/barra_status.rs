//! "Personalizar barra de status", como o do Excel: botão direito no rodapé
//! abre a lista do que aparece nele, cada item com a marca e o valor atual
//! ao lado. Além das seis contas do Excel (média, contagem, contagem
//! numérica, mínimo, máximo e soma), oferece desvio padrão, células vazias
//! e erros. A escolha fica nas preferências (`Configuracao::barra_status`).
use super::*;
use feather_ui::icones::icone;
use feather_ui::tema::{estilo_item_menu, estilo_popup_menu};
use feathersheet_core::planilha::sheet::ContagemSelecao;

/// Os itens, na ordem do menu e do rodapé: chave gravada e rótulo.
/// Grupos separados por linha: posição, contas do Excel, contas a mais e
/// os controles da direita.
pub const ITENS: &[&[(&str, &str)]] = &[
    &[("celula", "Célula ativa e abas")],
    &[
        ("media", "Média"),
        ("contagem", "Contagem"),
        ("contagem_numerica", "Contagem numérica"),
        ("minimo", "Mínimo"),
        ("maximo", "Máximo"),
        ("soma", "Soma"),
    ],
    &[
        ("desvio", "Desvio padrão"),
        ("vazias", "Células vazias"),
        ("erros", "Erros"),
    ],
    &[("aviso_erro", "Aviso de erro da célula"), ("zoom", "Zoom")],
];


/// Ordem em que as contas somem quando o rodapé fica estreito: a primeira
/// da lista sai primeiro; a contagem é a última a sair.
const PRIORIDADE_SAIDA: [&str; 9] = [
    "desvio", "vazias", "erros", "contagem_numerica", "minimo", "maximo", "media", "soma", "contagem",
];

pub(super) const LARGURA: f32 = 300.0;

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Fechar,
    Alternar(&'static str),
}

/// Formatação pt-BR (milhar com ponto, até duas casas), a mesma do rodapé.
pub(super) fn formatar_numero(v: f64) -> String {
    if !v.is_finite() {
        return v.to_string();
    }
    // O `{:.2}` do Rust arredonda meio para o par; arredondar antes evita
    // 0,125 virar "0,12".
    let arredondado = (v.abs() * 100.0).round() / 100.0;
    if arredondado == 0.0 {
        return "0".into();
    }
    let positivo = format!("{:.2}", arredondado);
    let (inteiro, dec) = positivo.split_once('.').unwrap_or((&positivo, "00"));
    let decimais = dec.trim_end_matches('0');
    let prefixo = if v < 0. { "-" } else { "" };
    if decimais.is_empty() {
        format!("{prefixo}{}", agrupar(inteiro))
    } else {
        format!("{prefixo}{},{decimais}", agrupar(inteiro))
    }
}

pub(super) fn formatar_inteiro(v: u64) -> String {
    agrupar(&v.to_string())
}

fn agrupar(digitos: &str) -> String {
    let mut out = String::with_capacity(digitos.len() + digitos.len() / 3);
    for (i, c) in digitos.chars().enumerate() {
        if i > 0 && (digitos.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// O valor de uma conta para a seleção; `None` quando não se aplica (média
/// sem números, desvio com um número só, nenhum erro).
pub(super) fn valor(chave: &str, c: &ContagemSelecao, celulas_area: u64) -> Option<String> {
    let com_numeros = c.numbers > 0;
    match chave {
        "media" => c.media().map(formatar_numero),
        "contagem" => Some(formatar_inteiro(c.count)),
        "contagem_numerica" => Some(formatar_inteiro(c.numbers)),
        "minimo" => com_numeros.then(|| formatar_numero(c.min)),
        "maximo" => com_numeros.then(|| formatar_numero(c.max)),
        "soma" => com_numeros.then(|| formatar_numero(c.sum)),
        "desvio" => c.desvio_padrao().map(formatar_numero),
        "vazias" => Some(formatar_inteiro(celulas_area.saturating_sub(c.count + c.erros))),
        "erros" => (c.erros > 0).then(|| formatar_inteiro(c.erros)),
        _ => None,
    }
}

/// "Média: 10" no rodapé (rótulo curto onde o do menu é longo).
fn rotulo_curto(chave: &str, valor: &str) -> String {
    match chave {
        "media" => trf!("Média: {}", valor),
        "contagem" => trf!("Contagem: {}", valor),
        "contagem_numerica" => trf!("Contagem numérica: {}", valor),
        "minimo" => trf!("Mín: {}", valor),
        "maximo" => trf!("Máx: {}", valor),
        "soma" => trf!("Soma: {}", valor),
        "desvio" => trf!("Desvio padrão: {}", valor),
        "vazias" => trf!("Vazias: {}", valor),
        "erros" => trf!("Erros: {}", valor),
        _ => String::new(),
    }
}

/// Texto das contas no rodapé, na ordem do menu. Quando não cabe na
/// largura (~6,5 px por caractere em 11 pt), as contas saem na ordem de
/// [`PRIORIDADE_SAIDA`].
pub(super) fn texto_contas(
    c: &ContagemSelecao, ligados: &[String], celulas_area: u64, largura: f32,
) -> String {
    let mut pecas: Vec<(&str, String)> = ITENS.iter().flat_map(|g| g.iter())
        .filter(|(chave, _)| ligados.iter().any(|l| l == chave))
        .filter_map(|(chave, _)| valor(chave, c, celulas_area).map(|v| (*chave, rotulo_curto(chave, &v))))
        .collect();
    let orcamento = ((largura - 700.) / 6.5).max(14.) as usize;
    let tamanho = |p: &[(&str, String)]| p.iter().map(|(_, t)| t.chars().count() + 5).sum::<usize>();
    for sai in PRIORIDADE_SAIDA {
        if tamanho(&pecas) <= orcamento || pecas.len() <= 1 {
            break;
        }
        pecas.retain(|(chave, _)| *chave != sai);
    }
    pecas.into_iter().map(|(_, t)| t).collect::<Vec<_>>().join("     ")
}

impl App {
    /// O item está ligado na barra de status?
    pub(super) fn barra_status_mostra(&self, chave: &str) -> bool {
        self.config.barra_status.iter().any(|c| c == chave)
    }

    pub(super) fn evento_barra_status(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => self.barra_status_menu = true,
            Evento::Fechar => self.barra_status_menu = false,
            // Como no Excel, o menu continua aberto para marcar vários.
            Evento::Alternar(chave) => {
                if let Some(i) = self.config.barra_status.iter().position(|c| c == chave) {
                    self.config.barra_status.remove(i);
                } else {
                    self.config.barra_status.push(chave.to_string());
                }
                self.config.gravar();
            }
        }
        Task::none()
    }

    /// Texto "A1 · 2 abas" do rodapé.
    pub(super) fn texto_celula_abas(&self) -> String {
        if self.workbook.len() == 1 {
            trf!("{}  ·  {} aba", cell_name(self.linha, self.coluna), 1)
        } else {
            trf!("{}  ·  {} abas", cell_name(self.linha, self.coluna), self.workbook.len())
        }
    }

    fn menu_barra_status(&self) -> Element<'_, Msg> {
        let (conta, celulas_area) = self.contas_da_selecao();
        let mut itens = pelicano::column![
            container(text(tr!("Personalizar barra de status")).size(12).color(COR_TEXTO_MUTED()))
                .padding([4, 10]),
        ].spacing(2).padding(8);
        for (g, grupo) in ITENS.iter().enumerate() {
            if g > 0 {
                itens = itens.push(container(Space::new(Length::FILL, Length::Fixed(1.)))
                    .padding([3, 6])
                    .style(|_| pelicano::Style {
                        background: Some(pelicano::Background::Color(COR_BORDA())),
                        ..Default::default()
                    }).height(Length::Fixed(1.)));
            }
            for (chave, rotulo) in grupo.iter() {
                let marcado = self.barra_status_mostra(chave);
                let marca: Element<'_, Msg> = if marcado {
                    icone("check", 13.0).into()
                } else {
                    Space::new(Length::Fixed(13.0), 0.0).into()
                };
                let atual = match *chave {
                    "celula" => self.texto_celula_abas(),
                    "zoom" => format!("{}%", self.config.zoom),
                    "aviso_erro" => String::new(),
                    _ => valor(chave, &conta, celulas_area).unwrap_or_default(),
                };
                itens = itens.push(
                    button(row![
                        marca,
                        text(tr!(*rotulo)).size(13).width(Length::FILL)
                            .no_wrap(),
                        text(atual).size(12).color(COR_TEXTO_MUTED())
                            .no_wrap(),
                    ].spacing(8).align(pelicano::Align::Center))
                        .width(Length::FILL)
                        .height(Length::Fixed(28.0))
                        .padding([4, 10])
                        .style(estilo_item_menu(false, true))
                        .on_press(Msg::BarraStatus(Evento::Alternar(chave))),
                );
            }
        }
        container(itens).width(Length::Fixed(LARGURA)).style(estilo_popup_menu).into()
    }

    /// O menu flutua no canto direito, logo acima do rodapé; clicar fora fecha.
    pub(super) fn sobrepor_barra_status<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if !self.barra_status_menu {
            return base;
        }
        let fora = pelicano::mouse_area(Space::new(Length::FILL, Length::FILL))
            .on_press(Msg::BarraStatus(Evento::Fechar))
            .on_right_press(Msg::BarraStatus(Evento::Fechar));
        let menu = pelicano::column![
            Space::new(0.0, Length::FILL),
            row![Space::new(Length::FILL, 0.0), pelicano::opaque(self.menu_barra_status())],
        ].padding(pelicano::Padding { top: 8., right: 8., bottom: 40., left: 8. });
        stack![base, fora, menu].height(Length::FILL).into()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn conta() -> ContagemSelecao {
        let mut c = ContagemSelecao { count: 4, numbers: 3, sum: 30., min: 5., max: 15., ..Default::default() };
        c.erros = 1;
        c
    }
    fn ligados(chaves: &[&str]) -> Vec<String> {
        chaves.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn numeros_ptbr() {
        assert_eq!(formatar_numero(1234.5), "1.234,5");
        assert_eq!(formatar_numero(-10000.25), "-10.000,25");
        assert_eq!(formatar_numero(0.125), "0,13");
        assert_eq!(formatar_numero(0.001), "0");
        assert_eq!(formatar_inteiro(1002003), "1.002.003");
    }

    #[test]
    fn mostra_so_o_que_esta_ligado_na_ordem_do_menu() {
        let c = conta();
        assert_eq!(texto_contas(&c, &ligados(&["soma", "media", "contagem"]), 6, 1600.),
            "Média: 10     Contagem: 4     Soma: 30");
        assert_eq!(texto_contas(&c, &ligados(&["minimo", "maximo", "contagem_numerica"]), 6, 1600.),
            "Contagem numérica: 3     Mín: 5     Máx: 15");
        // 6 células na área: 4 com valor, 1 erro, 1 vazia.
        assert_eq!(texto_contas(&c, &ligados(&["vazias", "erros"]), 6, 1600.),
            "Vazias: 1     Erros: 1");
        assert_eq!(texto_contas(&c, &[], 6, 1600.), "");
    }

    #[test]
    fn sem_numeros_as_contas_numericas_somem() {
        let c = ContagemSelecao { count: 2, ..Default::default() };
        assert_eq!(texto_contas(&c, &crate::configuracao::barra_status_padrao(), 2, 1600.), "Contagem: 2");
    }

    #[test]
    fn rodape_estreito_tira_primeiro_as_contas_extras_e_deixa_a_contagem() {
        let todos: Vec<String> = ITENS.iter().flat_map(|g| g.iter()).map(|(k, _)| k.to_string()).collect();
        let c = conta();
        let largo = texto_contas(&c, &todos, 6, 3000.);
        assert!(largo.contains("Erros: 1"));
        let estreito = texto_contas(&c, &todos, 6, 700.);
        assert_eq!(estreito, "Contagem: 4");
        let medio = texto_contas(&c, &todos, 6, 1100.);
        assert!(medio.contains("Soma: 30") && !medio.contains("Vazias"));
    }

    #[test]
    fn alternar_liga_desliga_e_menu_fica_aberto() {
        let mut app = App::default();
        let _ = app.evento_barra_status(Evento::Abrir);
        assert!(app.barra_status_mostra("soma"));
        let _ = app.evento_barra_status(Evento::Alternar("soma"));
        assert!(!app.barra_status_mostra("soma"));
        let _ = app.evento_barra_status(Evento::Alternar("desvio"));
        assert!(app.barra_status_mostra("desvio"));
        assert!(app.barra_status_menu);
        { let _: Element<'_, Msg> = app.menu_barra_status(); }
        let _ = app.evento_barra_status(Evento::Fechar);
        assert!(!app.barra_status_menu);
    }
}
