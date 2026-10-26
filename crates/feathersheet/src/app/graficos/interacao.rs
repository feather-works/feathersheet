//! O gráfico parado na planilha responde ao mouse: dica com o valor do ponto
//! sob o cursor e clique na legenda liga/desliga a série (na pizza, a fatia),
//! com o item hachurado e uma transição curta em vez de sumir de uma vez.
//!
//! Quem diz o que está sob o mouse é o mapa do próprio desenho
//! (`feather_charts::formatado::MapaGrafico`), em unidades do SVG.
use super::editor::{desenhar_grafico_salvo, Desenhado};
use super::*;
use std::time::{Duration, Instant};

/// Duração da transição da legenda: rápida, mas sem ser seca.
const DURACAO: Duration = Duration::from_millis(260);

/// Campo do gráfico salvo com os itens desligados pela legenda, quando ele
/// ainda não tem estilo próprio (com estilo, vai no `legenda` do estilo).
pub(super) const CAMPO_DESLIGADOS: &str = "_featherLegendaDesligada";

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Dica {
    /// Categoria (vazio = sem título).
    pub titulo: String,
    pub cor: pelicano::Color,
    /// Uma linha por valor: "Série: valor" (ou só o valor). O Gantt traz
    /// início, término e duração; as velas, os quatro preços…
    pub linhas: Vec<String>,
    pub fundo: Option<pelicano::Color>,
    pub texto: Option<pelicano::Color>,
    pub tamanho: f32,
}

#[derive(Debug, Clone)]
pub(super) struct Pairar {
    pub id: String,
    /// Posição do mouse na tela, relativa ao canto do gráfico.
    pub tela: pelicano::Point,
    pub dica: Option<Dica>,
    /// O mouse está sobre um item da legenda (índice).
    pub legenda: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) struct TransicaoLegenda {
    pub id: String,
    pub indice: usize,
    pub inicio: Instant,
}
impl TransicaoLegenda {
    /// Quanto do desenho novo já aparece por cima do antigo (0 a 1), suave.
    fn progresso(&self) -> f32 {
        let t = (self.inicio.elapsed().as_secs_f32() / DURACAO.as_secs_f32()).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }
    fn terminou(&self) -> bool {
        self.inicio.elapsed() >= DURACAO
    }
}

/// O mapa e os dados do último gráfico sob o mouse: sem redesenhar a cada
/// movimento enquanto nada muda.
struct Cache {
    chave: String,
    desenho: Desenhado,
}

#[derive(Default)]
pub(super) struct Estado {
    pub pairar: Option<Pairar>,
    pub transicao: Option<TransicaoLegenda>,
    cache: Option<Cache>,
    /// Modo de captura: a dica montada para o print não segue o mouse real.
    pub fixa: bool,
}

/// O gráfico salvo com o item `i` da legenda trocado (ligado ↔ desligado):
/// no estilo, se ele tem um; senão, na lista própria. Devolve também o
/// campo que mudou (para gravar só ele) e se o item ficou desligado.
pub(super) fn com_item_alternado(v: &serde_json::Value, i: usize) -> (serde_json::Value, &'static str, bool) {
    let mut novo = v.clone();
    let mut aparencia: Aparencia = serde_json::from_value(v["_featherNative"].clone()).unwrap_or_default();
    if let Some(e) = aparencia.estilo.as_mut() {
        let ocultando = !e.legenda.desligado(i);
        e.legenda.alternar(i);
        novo["_featherNative"] = serde_json::to_value(&aparencia).unwrap_or_default();
        return (novo, "_featherNative", ocultando);
    }
    let mut lista: Vec<usize> = serde_json::from_value(v[CAMPO_DESLIGADOS].clone()).unwrap_or_default();
    let ocultando = !lista.contains(&i);
    if ocultando { lista.push(i) } else { lista.retain(|x| *x != i) }
    lista.sort_unstable();
    novo[CAMPO_DESLIGADOS] = serde_json::json!(lista);
    (novo, CAMPO_DESLIGADOS, ocultando)
}

fn cor_iced(c: feather_draw::Cor) -> pelicano::Color {
    pelicano::Color::rgb8(c.r, c.g, c.b).alpha(f32::from(c.a) / 255.0)
}

impl App {
    /// O gráfico salvo `id` da aba ativa, a aba de onde vêm os dados e o
    /// índice dela.
    fn grafico_salvo(&self, id: &str) -> Option<(serde_json::Value, &feathersheet_core::Sheet, usize)> {
        let aba = self.workbook.aba(self.folha)?;
        let lista = feathersheet_io::graficos_salvos::listar(&aba.sheet).ok()?;
        let v = lista.into_iter().find(|v| v["id"].as_str() == Some(id))?;
        let indice = v["sourceSheet"].as_str().and_then(|n| self.workbook.indice_por_nome(n)).unwrap_or(self.folha);
        let dados = self.workbook.aba(indice).map_or(&aba.sheet, |a| &a.sheet);
        Some((v, dados, indice))
    }

    /// Mapa e dados do gráfico `id`, do cache quando nada mudou.
    fn desenho_para_mouse(&mut self, id: &str) -> Option<&Desenhado> {
        let (v, dados, indice) = self.grafico_salvo(id)?;
        let chave = format!("{}|{v}", self.workbook.versao(indice));
        if self.graficos.interacao.cache.as_ref().map(|c| &c.chave) != Some(&chave) {
            let desenho = desenhar_grafico_salvo(dados, &v, self.fonte_dados(indice))?;
            self.graficos.interacao.cache = Some(Cache { chave, desenho });
        }
        self.graficos.interacao.cache.as_ref().map(|c| &c.desenho)
    }

    pub(super) fn pairar_grafico(&mut self, id: String, tela: pelicano::Point) {
        if self.graficos.interacao.fixa {
            return;
        }
        let zoom = (self.config.zoom as f32 / 100.).max(0.01);
        let (x, y) = (tela.x / zoom, tela.y / zoom);
        let mut dica = None;
        let mut legenda = None;
        if let Some(d) = self.desenho_para_mouse(&id) {
            if let (Some(mapa), Some(g)) = (&d.mapa, &d.grafico) {
                legenda = mapa.item_legenda_em(x, y);
                if legenda.is_none() {
                    let padrao = feather_charts::estilo::EstiloGrafico::default();
                    let e = d.estilo.as_ref().unwrap_or(&padrao);
                    let o = &e.dica;
                    dica = mapa.ponto_em(x, y).filter(|_| o.visivel != Some(false)).and_then(|alvo| {
                        let conteudo = mapa.conteudo_dica(g, &alvo)?;
                        let pizza = g.tipo == feather_charts::Tipo::Pizza;
                        let formato = o.formato.or(e.rotulos.formato).or(e.eixo_y.formato).unwrap_or_default();
                        let escrever = |l: &feather_charts::formatado::LinhaDica| if l.data {
                            feather_charts::formatado::data_de_serie(l.valor)
                        } else {
                            format!("{}{}{}", o.prefixo, formato.formatar(l.valor), o.sufixo)
                        };
                        let serie = g.series.get(alvo.serie)?;
                        // Tipo com dica própria (Gantt, velas, cascata…): uma
                        // linha por valor, com o nome de cada um.
                        let propria = mapa.dica.is_some();
                        let v = conteudo.linhas[0].valor;
                        let mut valor = escrever(&conteudo.linhas[0]);
                        if o.percentual && !propria {
                            // Pizza: do total; senão, da categoria (séries ligadas).
                            let total: f64 = if pizza {
                                serie.valores.iter().enumerate().filter(|(i, v)| **v > 0.0 && !e.legenda.desligado(*i)).map(|(_, v)| *v).sum()
                            } else {
                                g.series.iter().enumerate().filter(|(i, _)| !e.legenda.desligado(*i))
                                    .filter_map(|(_, s)| s.valores.get(alvo.ponto)).map(|v| v.abs()).sum()
                            };
                            if total > 0.0 {
                                valor.push_str(&format!(" ({})", feather_charts::estilo::FormatoNum::Percentual { casas: 1 }.formatar(v / total)));
                            }
                        }
                        let cores: Vec<_> = g.series.iter().map(|s| s.cor).collect();
                        let cor = if pizza {
                            e.serie(0).and_then(|s| s.pontos.get(&alvo.ponto)).and_then(|p| p.cor).map_or(serie.cor, |c| c.0)
                        } else {
                            super::painel::cor_resolvida_serie(e, &cores, alvo.serie).0
                        };
                        let categoria = conteudo.titulo.clone();
                        let com_nome = |nome: &str, valor: String| if nome.is_empty() { valor } else { format!("{nome}: {valor}") };
                        let linhas = if propria {
                            conteudo.linhas.iter().map(|l| com_nome(&l.rotulo, escrever(l))).collect()
                        } else {
                            let nome = if pizza { categoria.clone() } else if o.serie.unwrap_or(true) { serie.nome.clone() } else { String::new() };
                            vec![com_nome(&nome, valor)]
                        };
                        Some(Dica {
                            titulo: if o.categoria.unwrap_or(true) && !pizza { categoria } else { String::new() },
                            cor: cor_iced(cor),
                            linhas,
                            fundo: o.fundo.map(|c| cor_iced(c.0)),
                            texto: o.texto.map(|c| cor_iced(c.0)),
                            tamanho: o.tamanho.unwrap_or(12.0).clamp(8.0, 28.0),
                        })
                    });
                }
            }
        }
        self.graficos.interacao.pairar = Some(Pairar { id, tela, dica, legenda });
    }

    pub(super) fn sair_do_grafico(&mut self, id: &str) {
        if self.graficos.interacao.fixa {
            return;
        }
        if self.graficos.interacao.pairar.as_ref().is_some_and(|p| p.id == id) {
            self.graficos.interacao.pairar = None;
        }
    }

    /// O clique caiu num item da legenda: liga/desliga e devolve `true`
    /// (o clique não vira arraste nem seleção).
    pub(super) fn clique_na_legenda(&mut self, id: &str) -> bool {
        let Some(indice) = self.graficos.interacao.pairar.as_ref().filter(|p| p.id == id).and_then(|p| p.legenda) else {
            return false;
        };
        if self.graficos.aberto && self.graficos.id.as_deref() == Some(id) {
            // Editor aberto: a mudança passa pelo painel (ele grava ao vivo).
            self.mudar_estilo(|e| { e.legenda.alternar(indice); true }, None);
        } else {
            let Some((v, _, _)) = self.grafico_salvo(id) else { return false };
            let (novo, campo, _) = com_item_alternado(&v, indice);
            let atualizacao = serde_json::json!({ campo: novo[campo] });
            let Some(aba) = self.workbook.aba_mut(self.folha) else { return false };
            match feathersheet_io::graficos_salvos::mesclado(&aba.sheet, id, atualizacao) {
                Ok(novo) => {
                    if aba.sheet.alterar_graficos(Some(novo), false) {
                        self.alterado = true;
                    }
                }
                Err(_) => return false,
            }
        }
        self.graficos.interacao.transicao = Some(TransicaoLegenda {
            id: id.to_owned(), indice, inicio: Instant::now(),
        });
        true
    }

    /// Um quadro da animação; termina sozinha.
    pub(super) fn quadro_transicao_grafico(&mut self) {
        if self.graficos.interacao.transicao.as_ref().is_some_and(TransicaoLegenda::terminou) {
            self.graficos.interacao.transicao = None;
        }
    }

    pub(in crate::app) fn grafico_em_transicao(&self) -> bool {
        self.graficos.interacao.transicao.is_some()
    }

    /// Transição em curso no gráfico `id`: o item trocado e quanto do
    /// desenho novo já aparece (0 a 1).
    pub(super) fn transicao_do_grafico(&self, id: &str) -> Option<(usize, f32)> {
        self.graficos.interacao.transicao.as_ref()
            .filter(|t| t.id == id)
            .map(|t| (t.indice, t.progresso()))
    }

    /// A dica flutuante do ponto sob o mouse, já na posição dentro do gráfico.
    pub(super) fn dica_do_grafico<'a>(&self, id: &str, largura: f32, altura: f32) -> Option<Element<'a, Msg>> {
        let p = self.graficos.interacao.pairar.as_ref().filter(|p| p.id == id)?;
        let d = p.dica.clone()?;
        let amostra = container(Space::new(Length::Fixed(10.), Length::Fixed(10.)))
            .style(move |_| pelicano::Style {
                background: Some(pelicano::Background::Color(d.cor)),
                border: pelicano::Border { radius: [2.0; 4], ..Default::default() },
                ..Default::default()
            });
        let tam = d.tamanho;
        let mut corpo = pelicano::column(Vec::new()).spacing(3);
        if !d.titulo.is_empty() {
            corpo = corpo.push(text(d.titulo.clone()).size(tam).font(pelicano::Font { weight: 700, ..Default::default() }));
        }
        let mut amostra = Some(amostra);
        for linha in &d.linhas {
            // A amostra de cor só na primeira linha; as outras alinham com ela.
            let marca: Element<'a, Msg> = match amostra.take() {
                Some(a) => a.into(),
                None => Space::new(Length::Fixed(10.), Length::Fixed(10.)).into(),
            };
            corpo = corpo.push(
                pelicano::row![marca, text(linha.clone()).size(tam)]
                    .spacing(6).align(pelicano::Align::Center),
            );
        }
        let (fundo, cor_texto) = (d.fundo, d.texto);
        let caixa = container(corpo).padding([6, 9]).style(move |_| pelicano::Style {
            background: Some(pelicano::Background::Color(fundo.unwrap_or_else(feather_ui::tema::COR_PAINEL))),
            text_color: Some(cor_texto.unwrap_or_else(feather_ui::tema::COR_TEXTO)),
            border: pelicano::Border { color: feather_ui::tema::COR_BORDA(), width: 1.0, radius: [6.0; 4] },
            shadow: Some(pelicano::Shadow {
                color: pelicano::Color::rgba(0.0, 0.0, 0.0, 0.18),
                offset: pelicano::Point::new(0.0, 2.0),
                blur: 8.0,
            }),
            ..Default::default()
        });
        // À direita e abaixo do cursor; vira para o outro lado perto da borda.
        let (lw, lh) = (210.0f32, 34.0f32 + 18.0 * d.linhas.len() as f32);
        let x = if p.tela.x + 14.0 + lw > largura { (p.tela.x - 14.0 - lw).max(0.0) } else { p.tela.x + 14.0 };
        let y = if p.tela.y + 16.0 + lh > altura { (p.tela.y - 10.0 - lh).max(0.0) } else { p.tela.y + 16.0 };
        Some(
            pelicano::column![
                Space::new(0.0, Length::Fixed(y)),
                pelicano::row![Space::new(Length::Fixed(x), 0.0), caixa],
            ]
            .width(Length::Fixed(largura)).height(Length::Fixed(altura))
            .into(),
        )
    }
}
