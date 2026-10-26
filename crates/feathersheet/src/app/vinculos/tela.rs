//! O que a pessoa vê das referências a outros arquivos: a faixa fina acima
//! da grade ("Valores de [Vendas.fshz] de 16/10 14:32: a origem está
//! fechada." + Atualizar valores) e o diálogo Editar links (Dados → Editar
//! links), no padrão dos diálogos internos (`feather_ui::dialogo`).
use super::*;
use pelicano::column;

/// Como uma origem está agora.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Situacao {
    /// Aberta em outra janela: os valores chegam sozinhos.
    AoVivo,
    /// Lendo do disco agora.
    Lendo,
    /// Fechada; valem os valores guardados (de quando, se se sabe).
    Guardado(Option<i64>),
    /// O arquivo existe, mas ainda não há valor nenhum.
    SemValores,
    /// Não há arquivo no caminho.
    NaoEncontrado,
}

impl Situacao {
    fn texto(&self) -> String {
        match self {
            Situacao::AoVivo => tr!("Ao vivo: aberto em outra janela").into(),
            Situacao::Lendo => tr!("Lendo do disco…").into(),
            Situacao::Guardado(Some(t)) => trf!("Valores guardados de {}", data_hora_local(*t)),
            Situacao::Guardado(None) => tr!("Valores guardados").into(),
            Situacao::SemValores => tr!("Ainda sem valores").into(),
            Situacao::NaoEncontrado => tr!("Arquivo não encontrado").into(),
        }
    }

    fn alerta(&self) -> bool {
        matches!(self, Situacao::SemValores | Situacao::NaoEncontrado)
    }
}

/// Corta no fim com reticências (nome de arquivo).
fn cortar_fim(texto: &str, maximo: usize) -> String {
    if texto.chars().count() <= maximo {
        return texto.to_string();
    }
    format!("{}…", texto.chars().take(maximo.saturating_sub(1)).collect::<String>())
}

/// Corta no começo com reticências (caminho de pasta: o fim é o que importa).
fn cortar_inicio(texto: &str, maximo: usize) -> String {
    let n = texto.chars().count();
    if n <= maximo {
        return texto.to_string();
    }
    format!("…{}", texto.chars().skip(n - maximo.saturating_sub(1)).collect::<String>())
}

fn semibold() -> pelicano::Font {
    pelicano::Font { weight: 600, ..pelicano::Font::DEFAULT }
}

impl App {
    pub(crate) fn situacao_origem(&self, o: &Origem) -> Situacao {
        if o.ao_vivo {
            return Situacao::AoVivo;
        }
        if self.vinculos.lendo.contains(&o.chave) {
            return Situacao::Lendo;
        }
        let livros: Vec<_> = o.nomes.iter().filter_map(|n| self.workbook.livro_externo(n)).collect();
        if !livros.is_empty() {
            return Situacao::Guardado(livros.iter().filter_map(|l| l.atualizado_em).max());
        }
        if o.caminho.as_ref().is_some_and(|c| c.is_file()) {
            Situacao::SemValores
        } else {
            Situacao::NaoEncontrado
        }
    }

    /// A frase da faixa e se é alerta. `None`: nada a avisar (todas vivas).
    pub(crate) fn aviso_vinculos(&self) -> Option<(String, bool)> {
        if self.vinculos.faixa_fechada {
            return None;
        }
        let fechadas: Vec<(&Origem, Situacao)> = self
            .vinculos
            .origens
            .iter()
            .map(|o| (o, self.situacao_origem(o)))
            .filter(|(_, s)| *s != Situacao::AoVivo)
            .collect();
        match fechadas.as_slice() {
            [] => None,
            [(o, s)] => {
                let arquivo = o.arquivo();
                Some(match s {
                    Situacao::Guardado(Some(t)) => {
                        (trf!("Valores de [{arquivo}] de {}: a origem está fechada.", data_hora_local(*t)), false)
                    }
                    Situacao::Guardado(None) => (trf!("Valores guardados de [{arquivo}]: a origem está fechada."), false),
                    Situacao::Lendo => (trf!("Lendo os valores de [{arquivo}]…"), false),
                    Situacao::SemValores => (trf!("Ainda sem valores de [{arquivo}]."), true),
                    _ => (trf!("A origem [{arquivo}] não foi encontrada."), true),
                })
            }
            varias => Some((
                trf!("{} arquivos de origem fechados: valem os últimos valores guardados.", varias.len()),
                varias.iter().any(|(_, s)| s.alerta()),
            )),
        }
    }

    /// Faixa fina acima da grade, no mesmo desenho da faixa de salvamento.
    pub(in crate::app) fn view_faixa_vinculos(&self) -> Element<'_, Msg> {
        let Some((mensagem, alerta)) = self.aviso_vinculos() else { return Space::new(0.0, 0.0).into() };
        let lendo_tudo = self.vinculos.origens.iter().all(|o| matches!(self.situacao_origem(o), Situacao::Lendo | Situacao::AoVivo));
        container(
            row![
                feather_ui::icones::icone_traco(if alerta { "alert-triangle" } else { "link" }, 16.),
                text(mensagem).size(12.5).width(Length::FILL),
                button(text(tr!("Editar links…")).size(12))
                    .padding([5, 12])
                    .style(estilo_botao_secundario)
                    .on_press(Msg::Vinculos(Evento::EditarAbrir)),
                button(text(tr!("Atualizar valores")).size(12))
                    .padding([5, 12])
                    .style(estilo_botao_primario)
                    .on_press_maybe((!lendo_tudo).then_some(Msg::Vinculos(Evento::AtualizarValores(None)))),
                feather_ui::formatacao::dica(
                    button(feather_ui::icones::icone_traco("x", 14.))
                        .padding([5, 7])
                        .style(estilo_botao_pequeno(false))
                        .on_press(Msg::Vinculos(Evento::FaixaFechar)),
                    tr!("Fechar aviso"),
                ),
            ]
            .spacing(10)
            .align(pelicano::Align::Center),
        )
        .padding([6, 12])
        .width(Length::FILL)
        .style(|_: &pelicano::Theme| pelicano::Style {
            background: Some(pelicano::Background::Color(COR_AZUL_FUNDO())),
            border: pelicano::Border { color: COR_BORDA_SUAVE(), width: 1., ..Default::default() },
            ..Default::default()
        })
        .into()
    }

    /// Editar links: as origens da pasta, cada uma com Atualizar e Trocar.
    pub(in crate::app) fn sobrepor_editar_links<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        use crate::app::dialogo_interno::dlg;
        if !self.vinculos.editar_aberto {
            return base;
        }
        let corpo: Element<'a, Msg> = if self.vinculos.origens.is_empty() {
            dlg::aviso_vazio(tr!("Esta pasta não tem fórmulas que leem outro arquivo. Exemplo: =[Vendas.fshz]Dados!A1"))
        } else {
            let mut lista = column(Vec::new()).spacing(6);
            for o in &self.vinculos.origens {
                let situacao = self.situacao_origem(o);
                let caminho = o
                    .caminho
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| tr!("Salve esta pasta para achar a origem pelo nome").into());
                let pode_atualizar = situacao != Situacao::Lendo && (o.conexao.is_some() || o.caminho.as_ref().is_some_and(|c| c.is_file()));
                let icone = match situacao {
                    Situacao::NaoEncontrado => "link-off",
                    Situacao::AoVivo => "bolt",
                    _ => "link",
                };
                let linha = row![
                    feather_ui::icones::icone_traco(icone, 18.),
                    column![
                        text(cortar_fim(&o.arquivo(), 48)).size(13).font(semibold()),
                        text(cortar_inicio(&caminho, 66)).size(11.5).color(COR_TEXTO_MUTED()),
                        text(situacao.texto()).size(11.5).color(if situacao.alerta() { COR_TEXTO() } else { COR_TEXTO_MUTED() }),
                    ]
                    .spacing(2)
                    .width(Length::FILL),
                    feather_ui::formatacao::dica(
                        button(feather_ui::icones::icone("refresh", 15.))
                            .padding(5)
                            .style(estilo_botao_pequeno(false))
                            .on_press_maybe(pode_atualizar.then(|| Msg::Vinculos(Evento::AtualizarValores(Some(o.chave.clone()))))),
                        tr!("Atualizar valores desta origem"),
                    ),
                    feather_ui::formatacao::dica(
                        button(feather_ui::icones::icone("folder-open", 15.))
                            .padding(5)
                            .style(estilo_botao_pequeno(false))
                            .on_press(Msg::Vinculos(Evento::TrocarOrigem(o.chave.clone()))),
                        tr!("Trocar origem…"),
                    ),
                ]
                .spacing(10)
                .align(pelicano::Align::Center);
                lista = lista.push(container(linha).padding([8, 12]).width(Length::FILL).style(dlg::estilo_bloco));
            }
            column![
                text(tr!("Fórmulas desta pasta que leem outros arquivos. Com a origem aberta em outra janela do FeatherSheet, os valores chegam sozinhos; com ela fechada, valem os últimos valores guardados neste arquivo."))
                    .size(12.5)
                    .line_height(1.4),
                dlg::secao(tr!("Origens")),
                lista,
            ]
            .spacing(12)
            .into()
        };
        let tem_origens = !self.vinculos.origens.is_empty();
        self.dialogo(tr!("Editar links"), corpo)
            .icone("link")
            .subtitulo(tr!("Referências a outros arquivos"))
            .largura(560.0)
            .botao(dlg::botao_secundario(tr!("Atualizar todos"), tem_origens.then_some(Msg::Vinculos(Evento::AtualizarValores(None)))))
            .botao(dlg::botao_primario(tr!("Concluir"), Some(Msg::Vinculos(Evento::EditarFechar))))
            .ao_fechar(Msg::Vinculos(Evento::EditarFechar))
            .sobre(base)
    }
}
