//! Aba Desenhar (caneta) do FeatherSheet. O que é comum mora fora daqui:
//! o motor dos traços em `feather-draw::tinta`, a camada e os
//! controles em `feather_ui::tinta`, as âncoras e o `.fshz`/`.xlsx` em
//! `feathersheet_io::tinta_aba`. Aqui fica só a ligação com a planilha:
//! a régua das células (rolagem, zoom, larguras/alturas, painéis
//! congelados) e o desfazer — cada traço, apagar ou forma é UM Ctrl+Z
//! (`Sheet::alterar_tinta`). Em modo desenho o clique não seleciona célula;
//! Esc sai.
//!
//! **Pressão da caneta:** a leitura é do `feather_ui::caneta` (XInput2 no
//! Linux, WM_POINTER no Windows), ligada em [`App::ligar_caneta`]. Ao
//! gravar, a pressão crua passa pela curva da pessoa (gama das
//! Configurações) e vira a "pressão de resposta" do arquivo; tela, PDF,
//! SVG e `.xlsx` desenham com `CurvaPressao::PADRAO` (o mesmo contorno do
//! motor). Sem mesa, a espessura vem da velocidade (pressão sintética,
//! marcada no traço).
use super::*;
use feather_ui::caneta::{self as ui_caneta, FaseToque, FontePressao, ToqueCaneta};
use feather_ui::tinta::{self as ui_tinta, Camada, EventoCamada, Ferramenta, GestoTinta, TracoTela, Transformacao};
use feather_draw::tinta::{
    CurvaPressao, Ferramenta as FerramentaMotor, PontoTinta, Tinta, Traco, VELOCIDADE_FINA,
};
use std::cell::RefCell;
use std::collections::HashMap;
use feather_draw::{Cor, Ponto};
use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_io::tinta_aba::{TintaAba, MAX_TRACOS};

/// Simplificação do traço, em pixels de tela (dividida pelo zoom).
const TOLERANCIA_TELA: f32 = 0.6;
/// Cabeçalhos da grade em px lógicos (escalam com o zoom), iguais aos da grade.
const CABECALHO_X: f32 = crate::grade::CABECALHO_LINHA;
const CABECALHO_Y: f32 = crate::grade::CABECALHO_COLUNA;
/// Barras de rolagem desenhadas no próprio Canvas (não escalam com o zoom).
const BARRA: f32 = 13.0;

#[derive(Debug, Default)]
pub(super) struct EstadoTinta {
    pub ui: ui_tinta::Estado,
    /// Apagar em andamento: aba, tinta absoluta já apagada, se mudou algo
    /// e o último ponto da borracha (o trecho entre dois eventos também apaga).
    rascunho: Option<(u32, Tinta, bool, Option<Ponto>)>,
    /// Seleção do laço: aba e ids dos traços.
    pub selecao: Option<(u32, Vec<u64>)>,
    /// Onde o menu de cor/espessura abre (posição do clique na faixa).
    pub menu_pos: (f32, f32),
    /// Pressão da mesa (None = leitura desligada ou indisponível).
    pub fonte_pressao: Option<FontePressao>,
    /// Contornos de largura variável já calculados (o `view` roda a cada quadro).
    contornos: CacheContorno,
    /// Traço da caneta lido pelo XInput2 (o servidor X nem sempre entrega
    /// a ponta ao winit; ver [`App::toques_caneta`]).
    gesto_caneta: Option<GestoTinta>,
    /// O modo desenho foi ligado pela caneta (sai com o mouse ou Esc).
    pub auto_caneta: bool,
    /// Última caneta escolhida (é a que a caneta liga sozinha).
    ultima_pena: Option<Ferramenta>,
}

/// Ajustes da caneta (Configurações > Planilha).
#[derive(Debug, Clone, PartialEq)]
pub enum AjusteCaneta {
    /// Ler a pressão da mesa (vale na próxima abertura).
    Pressao(bool),
    /// Linux/Wayland: `auto`, `x11` ou `wayland` (vale na próxima abertura).
    Janela(&'static str),
    /// Gama da curva de resposta.
    Curva(f32),
    /// Sem mesa, espessura pela velocidade.
    Velocidade(bool),
    /// Indicador "pressão 0,63" durante o traço.
    MostrarPressao(bool),
    /// Resultado de ligar a leitura no Windows (texto vazio = deu certo).
    Falha(String),
    /// Toques da ponta lidos por fora do Iced (XInput2), em ordem.
    Toques(Vec<ToqueCaneta>),
    /// A caneta liga o modo desenhar sozinha.
    AutoDesenhar(bool),
    /// O Iced recebeu um clique (modo ligado pela caneta: se for o mouse, sai).
    CliqueIced,
}

/// `FEATHERSHEET_CANETA_DIAG=1`: lista os dispositivos XInput, anota os
/// eventos crus da camada e liga o indicador da pressão.
pub(super) fn diagnostico_no_terminal() -> bool {
    std::env::var_os("FEATHERSHEET_CANETA_DIAG").is_some_and(|v| v == "1")
}

/// Contorno de cada traço relativo ao primeiro ponto, pela forma do traço.
/// Por id do traço: (chave da forma, polígonos relativos ao primeiro ponto).
type ContornosPorTraco = HashMap<u64, (u64, Vec<Vec<Ponto>>)>;

#[derive(Debug, Default)]
struct CacheContorno(RefCell<ContornosPorTraco>);

impl CacheContorno {
    fn chave(t: &Traco) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        let base = t.pontos.first().map(|p| (p.x, p.y)).unwrap_or_default();
        for p in &t.pontos {
            (p.x - base.0).to_bits().hash(&mut h);
            (p.y - base.1).to_bits().hash(&mut h);
            p.pressao.map(f32::to_bits).hash(&mut h);
        }
        t.espessura.to_bits().hash(&mut h);
        t.reto.hash(&mut h);
        h.finish()
    }

    /// Polígonos do motor (`CurvaPressao::PADRAO`) já deslocados como o traço na vista.
    fn contorno(&self, t: &Traco, dx: f32, dy: f32) -> Vec<Vec<pelicano::Point>> {
        let Some(base) = t.pontos.first().map(|p| (p.x, p.y)) else { return Vec::new() };
        let chave = Self::chave(t);
        let mut mapa = self.0.borrow_mut();
        if mapa.len() > 8000 {
            mapa.clear();
        }
        let entrada = mapa.entry(t.id).or_insert_with(|| (0, Vec::new()));
        if entrada.0 != chave || entrada.1.is_empty() {
            let pol = t.contorno(CurvaPressao::PADRAO);
            *entrada = (chave, pol.into_iter().map(|v| v.into_iter().map(|p| Ponto::novo(p.x - base.0, p.y - base.1)).collect()).collect());
        }
        let (ox, oy) = (base.0 - dx, base.1 - dy);
        entrada.1.iter().map(|v| v.iter().map(|p| pelicano::Point::new(p.x + ox, p.y + oy)).collect()).collect()
    }
}

/// Resumo da pressão de um traço para o Console F12 e a barra de status.
fn resumo_pressao(traco: &Traco, real: bool) -> String {
    let valores: Vec<f32> = traco.pontos.iter().filter_map(|p| p.pressao).collect();
    if valores.is_empty() {
        return tr!("sem pressão").into();
    }
    let min = valores.iter().copied().fold(f32::INFINITY, f32::min);
    let max = valores.iter().copied().fold(0.0f32, f32::max);
    let med = valores.iter().sum::<f32>() / valores.len() as f32;
    let br = |v: f32| format!("{v:.2}").replace('.', ",");
    trf!(
        "{} {}–{} (média {})",
        if real { tr!("pressão da mesa") } else { tr!("pressão pela velocidade (sem mesa)") },
        br(min),
        br(max),
        br(med)
    )
}

/// Régua da aba: deslocamentos de rolagem e painel congelado, em px lógicos.
/// Reaproveitada pela camada de objetos soltos (`objetos.rs`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Vista {
    pub(super) zoom: f32,
    pub(super) desloc_x: f32,
    pub(super) desloc_y: f32,
    pub(super) fixa_x: f32,
    pub(super) fixa_y: f32,
}

impl Vista {
    /// Quanto subtrair de um ponto absoluto para chegar à vista. Objetos
    /// que começam no painel congelado não rolam (como as imagens).
    pub(super) fn deslocamento(self, inicio: Ponto) -> (f32, f32) {
        (
            if inicio.x < self.fixa_x { 0.0 } else { self.desloc_x - self.fixa_x },
            if inicio.y < self.fixa_y { 0.0 } else { self.desloc_y - self.fixa_y },
        )
    }
    /// Ponto da vista (o que a camada devolve) para absoluto.
    pub(super) fn absoluto(self, p: Ponto) -> Ponto {
        let (dx, dy) = self.deslocamento(p);
        Ponto::novo(p.x + dx, p.y + dy)
    }
    fn transformacao(self) -> Transformacao {
        Transformacao { escala: self.zoom, dx: CABECALHO_X * self.zoom, dy: CABECALHO_Y * self.zoom }
    }
}

/// Célula onde fica o ponto absoluto: `(linha, coluna, x_inicio, y_inicio)`.
pub(super) fn celula_em(sheet: &Sheet, x: f32, y: f32) -> (u32, u32, f32, f32) {
    use crate::grade::modelo::{inicio_x_da, inicio_y_da};
    fn buscar(limite: u32, alvo: f32, inicio: impl Fn(u32) -> f32) -> u32 {
        // Maior índice com início <= alvo (o início cresce com o índice).
        let (mut lo, mut hi) = (0u32, limite.saturating_sub(1));
        while lo < hi {
            let meio = lo + (hi - lo).div_ceil(2);
            if inicio(meio) <= alvo { lo = meio } else { hi = meio - 1 }
        }
        lo
    }
    let c = buscar(feathersheet_core::MAX_COLS, x, |c| inicio_x_da(sheet, c));
    let r = buscar(feathersheet_core::MAX_ROWS, y, |r| inicio_y_da(sheet, r));
    (r, c, inicio_x_da(sheet, c), inicio_y_da(sheet, r))
}

fn tinta_absoluta(sheet: &Sheet) -> Tinta {
    use crate::grade::modelo::{inicio_x_da, inicio_y_da};
    TintaAba::ler(sheet.tn.as_deref())
        .map(|aba| aba.para_absoluta(|c| inicio_x_da(sheet, c), |r| inicio_y_da(sheet, r)))
        .unwrap_or_default()
}

fn cor_motor(c: [u8; 4]) -> Cor {
    Cor::rgba(c[0], c[1], c[2], c[3])
}

impl App {
    pub(super) fn vista_tinta(&self) -> Option<Vista> {
        use crate::grade::modelo::{inicio_x_da, inicio_y_da};
        let aba = self.workbook.aba(self.folha)?;
        let (linhas_fixas, colunas_fixas) = self.congelamento_atual();
        Some(Vista {
            zoom: (f32::from(self.config.zoom) / 100.0).max(0.05),
            desloc_x: inicio_x_da(&aba.sheet, self.coluna_inicio),
            desloc_y: inicio_y_da(&aba.sheet, self.linha_inicio),
            fixa_x: inicio_x_da(&aba.sheet, colunas_fixas),
            fixa_y: inicio_y_da(&aba.sheet, linhas_fixas),
        })
    }

    /// Tinta da aba ativa em coordenadas absolutas (o apagar em andamento
    /// aparece já apagado).
    fn tinta_atual(&self) -> Option<(u32, Tinta)> {
        let aba = self.workbook.aba(self.folha)?;
        if let Some((id, tinta, _, _)) = &self.tinta.rascunho {
            if *id == aba.id {
                return Some((aba.id, tinta.clone()));
            }
        }
        Some((aba.id, tinta_absoluta(&aba.sheet)))
    }

    /// Grava a tinta absoluta no `tn` da aba: uma etapa de desfazer.
    fn gravar_tinta(&mut self, tinta: Tinta, status: String) {
        let Some(aba) = self.workbook.aba_mut(self.folha) else { return };
        let sheet = &aba.sheet;
        let texto = TintaAba::de_absoluta(tinta, |x, y| celula_em(sheet, x, y)).gravar();
        if aba.sheet.alterar_tinta(texto) {
            self.alterado = true;
            self.status = status;
        }
    }

    /// Traços na vista (rolagem e painel congelado já descontados) e a
    /// transformação vista → tela. É o que a camada desenha.
    pub(super) fn tracos_tela(&self) -> (Vec<TracoTela>, Transformacao) {
        let Some(vista) = self.vista_tinta() else { return (Vec::new(), Transformacao::default()) };
        let Some((_, tinta)) = self.tinta_atual() else { return (Vec::new(), vista.transformacao()) };
        let tracos = tinta
            .tracos()
            .iter()
            .filter_map(|t| {
                let inicio = t.pontos.first()?.ponto();
                let (dx, dy) = vista.deslocamento(inicio);
                let alfa = f32::from(t.cor.a) / 255.0 * t.opacidade;
                Some(TracoTela {
                    id: t.id,
                    pontos: t.pontos.iter().map(|p| pelicano::Point::new(p.x - dx, p.y - dy)).collect(),
                    cor: pelicano::Color::rgb8(t.cor.r, t.cor.g, t.cor.b).alpha(alfa.clamp(0.0, 1.0)),
                    espessura: t.espessura,
                    reto: t.reto,
                    marca_texto: t.ferramenta == FerramentaMotor::MarcaTexto,
                    contorno: if t.largura_variavel() { self.tinta.contornos.contorno(t, dx, dy) } else { Vec::new() },
                })
            })
            .collect();
        (tracos, vista.transformacao())
    }

    /// A camada de tinta por cima da grade (e das imagens/botões).
    pub(super) fn grade_com_tinta<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let estado = &self.tinta.ui;
        let automatica = self.caneta_automatica();
        if let Some(fonte) = &self.tinta.fonte_pressao {
            // A leitura da caneta converte pixel físico em lógico e decide
            // entre traço (zona da camada) e clique.
            fonte.definir_tamanho_logico(self.tamanho_principal);
            if !estado.ativo && !automatica {
                fonte.definir_zona_tinta(None);
            }
        }
        let tem_tinta = self.workbook.aba(self.folha).is_some_and(|a| a.sheet.tn.is_some());
        if !estado.ativo && !tem_tinta && !automatica {
            return base;
        }
        let (tracos, transformacao) = self.tracos_tela();
        let zoom = transformacao.escala;
        let mut camada = Camada::nova(tracos, transformacao, Msg::Tinta);
        camada.margens = pelicano::Padding { top: CABECALHO_Y * zoom, right: BARRA, bottom: BARRA, left: CABECALHO_X * zoom };
        camada.ferramenta = estado.ferramenta_ativa();
        camada.pena = estado.pena().map(|p| {
            let opacidade = if estado.ferramenta == Ferramenta::MarcaTexto {
                FerramentaMotor::MarcaTexto.opacidade_padrao()
            } else {
                1.0
            };
            let [r, g, b, a] = p.cor;
            (pelicano::Color::rgb8(r, g, b).alpha(f32::from(a) / 255.0 * opacidade), p.espessura)
        });
        camada.fonte_pressao = self.tinta.fonte_pressao.clone();
        camada.curva_previa = (CurvaPressao::PADRAO.minimo, self.config.caneta_gama);
        camada.velocidade_previa = self.config.caneta_velocidade.then_some(VELOCIDADE_FINA);
        camada.mostrar_pressao = self.config.caneta_mostrar_pressao || diagnostico_no_terminal();
        camada.gesto_externo = self.tinta.gesto_caneta.clone();
        camada.capturar_caneta = estado.menu.is_none() && !self.algo_sobre_a_grade();
        camada.caneta_automatica = automatica;
        camada.mouse_sai_do_modo = self.tinta.auto_caneta;
        if let (Some((aba_id, ids)), Some(aba)) = (&self.tinta.selecao, self.workbook.aba(self.folha)) {
            if *aba_id == aba.id {
                camada.selecionados = ids.clone();
            }
        }
        stack![base, ui_tinta::camada(camada)].width(Length::FILL).height(Length::FILL).into()
    }

    /// O menu flutuante de cor e espessura das canetas.
    pub(super) fn sobrepor_menu_tinta<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        let (x, y) = self.tinta.menu_pos;
        ui_tinta::sobrepor_menu(base, &self.tinta.ui, x, y, self.tamanho_principal, Msg::Desenhar)
    }

    /// Botões da aba Desenhar.
    pub(super) fn evento_desenhar(&mut self, evento: ui_tinta::Evento) -> Task<Msg> {
        let menu_antes = self.tinta.ui.menu;
        let parar = evento == ui_tinta::Evento::Parar;
        // Escolha à mão (mouse, faixa): o modo deixa de ser "da caneta".
        self.tinta.auto_caneta = false;
        if let ui_tinta::Evento::Escolher(f) = evento {
            if f != Ferramenta::Laco {
                self.tinta.selecao = None;
            }
            if f.desenha() {
                self.tinta.ultima_pena = Some(f);
            }
        }
        self.tinta.ui.atualizar(evento);
        if self.tinta.ui.menu.is_some() && self.tinta.ui.menu != menu_antes {
            // Abaixo do botão clicado.
            self.tinta.menu_pos = ((self.ponteiro_x - 30.0).max(8.0), self.ponteiro_y + 26.0);
        }
        if parar {
            self.tinta.selecao = None;
            self.tinta.rascunho = None;
            self.status = tr!("Modo de seleção: clique nas células normalmente.").into();
        } else if self.tinta.ui.ativo && self.tinta.ui.menu.is_none() {
            self.status = match self.tinta.ui.ferramenta {
                f if f.desenha() => trf!("{}: desenhe sobre a planilha (Esc para parar).", f.rotulo()),
                Ferramenta::Laco => tr!("Laço: circule os traços para selecionar (Delete apaga).").into(),
                f => trf!("{}: passe sobre os traços (Esc para parar).", f.rotulo()),
            };
        }
        Task::none()
    }

    /// O que a camada de tinta capturou.
    pub(super) fn evento_tinta(&mut self, evento: EventoCamada) -> Task<Msg> {
        let Some(vista) = self.vista_tinta() else { return Task::none() };
        match evento {
            EventoCamada::TracoTerminado { ferramenta, pontos } => {
                let Some(pena) = self.tinta.ui.pena_de(ferramenta) else { return Task::none() };
                let Some(primeiro) = pontos.first() else { return Task::none() };
                // O traço inteiro rola junto: a decisão é pelo primeiro ponto.
                let base = vista.absoluto(Ponto::novo(primeiro.x, primeiro.y));
                let (dx, dy) = (base.x - primeiro.x, base.y - primeiro.y);
                let motor = if ferramenta == Ferramenta::MarcaTexto { FerramentaMotor::MarcaTexto } else { FerramentaMotor::Caneta };
                // A pressão crua passa pela curva da pessoa; o marca-texto não usa pressão.
                let curva = CurvaPressao::nova(CurvaPressao::PADRAO.minimo, self.config.caneta_gama);
                let usa_pressao = motor == FerramentaMotor::Caneta;
                let mut traco = Traco::novo(
                    pontos
                        .iter()
                        .map(|p| {
                            let ponto = PontoTinta::novo(p.x + dx, p.y + dy).com_tempo(p.tempo_ms);
                            match p.pressao.filter(|_| usa_pressao) {
                                Some(pr) => ponto.com_pressao(curva.resposta(pr)),
                                None => ponto,
                            }
                        })
                        .collect(),
                    cor_motor(pena.cor),
                    pena.espessura,
                    motor,
                );
                let real = traco.tem_pressao();
                if usa_pressao && !real && self.config.caneta_velocidade {
                    traco.aplicar_pressao_por_velocidade(VELOCIDADE_FINA);
                }
                traco.preparar(TOLERANCIA_TELA / vista.zoom);
                let resumo = resumo_pressao(&traco, real);
                let origem = self.tinta.fonte_pressao.as_ref().map(FontePressao::descricao).unwrap_or_else(|| tr!("leitura da mesa desligada").into());
                self.monitor.registrar(
                    feather_ui::monitor::Nivel::Info,
                    tr!("Caneta"),
                    &format!("[caneta] {} com {} pontos: {resumo}; {origem}", ferramenta.rotulo(), traco.pontos.len()),
                );
                let mut status = if usa_pressao && traco.tem_pressao() {
                    trf!("Traço de {} desenhado, {resumo} (Ctrl+Z desfaz).", ferramenta.rotulo().to_lowercase())
                } else {
                    trf!("Traço de {} desenhado (Ctrl+Z desfaz).", ferramenta.rotulo().to_lowercase())
                };
                if self.tinta.ui.tinta_para_forma {
                    if let Some(forma) = traco.reconhecer_forma() {
                        traco = traco.com_forma(&forma);
                        status = trf!("Tinta convertida em {} (Ctrl+Z desfaz).", forma.nome());
                    }
                }
                let Some((_, mut tinta)) = self.tinta_atual() else { return Task::none() };
                if tinta.len() >= MAX_TRACOS {
                    self.status = trf!("Limite de {MAX_TRACOS} traços por aba.");
                    return Task::none();
                }
                if tinta.adicionar(traco).is_some() {
                    self.gravar_tinta(tinta, status);
                }
            }
            EventoCamada::Apagar { ferramenta, ponto, raio } => {
                let Some((aba_id, tinta)) = self.tinta_atual() else { return Task::none() };
                let (_, mut tinta, mut mudou, ultimo) = self
                    .tinta
                    .rascunho
                    .take()
                    .filter(|(id, ..)| *id == aba_id)
                    .unwrap_or((aba_id, tinta, false, None));
                let p = vista.absoluto(Ponto::novo(ponto.x, ponto.y));
                let caminho: Vec<Ponto> = ultimo.into_iter().chain([p]).collect();
                mudou |= match ferramenta {
                    Ferramenta::BorrachaPonto => tinta.apagar_pontos_no_caminho(&caminho, raio),
                    _ => !tinta.apagar_tracos_no_caminho(&caminho, raio).is_empty(),
                };
                self.tinta.rascunho = Some((aba_id, tinta, mudou, Some(p)));
            }
            EventoCamada::ApagarFim => {
                if let Some((aba_id, tinta, true, _)) = self.tinta.rascunho.take() {
                    if self.workbook.aba(self.folha).is_some_and(|a| a.id == aba_id) {
                        self.tinta.selecao = None;
                        self.gravar_tinta(tinta, tr!("Tinta apagada (Ctrl+Z desfaz).").into());
                    }
                }
            }
            EventoCamada::Laco(poligono) => {
                let laco: Vec<Ponto> = poligono.iter().map(|p| vista.absoluto(Ponto::novo(p.x, p.y))).collect();
                let Some((aba_id, tinta)) = self.tinta_atual() else { return Task::none() };
                let ids = tinta.selecionar_laco(&laco);
                self.status = match ids.len() {
                    0 => tr!("Nenhum traço dentro do laço.").into(),
                    1 => tr!("1 traço selecionado (Delete apaga).").into(),
                    n => trf!("{n} traços selecionados (Delete apaga)."),
                };
                self.tinta.selecao = (!ids.is_empty()).then_some((aba_id, ids));
            }
        }
        Task::none()
    }

    /// Assinaturas da caneta: os toques lidos pelo XInput2 (também acordam
    /// o invólucro `clique_caneta`, que entrega a ponta como clique).
    pub(super) fn assinatura_caneta(&self) -> Subscription<Msg> {
        // Modo ligado pela caneta: um clique do mouse volta à seleção — sem
        // `listen_with` no pelicano, o clique chega por [`Self::clique_mouse_caneta`].
        match self.tinta.fonte_pressao.as_ref().filter(|f| f.entrega_toques()) {
            Some(fonte) => ui_caneta::assinatura_toques(fonte).map(|lote| Msg::Caneta(AjusteCaneta::Toques(lote))),
            None => Subscription::none(),
        }
    }

    /// O `MousePressed` da janela principal (`App::evento_janela`): com o modo
    /// ligado pela caneta, um clique do mouse volta à seleção.
    pub(super) fn clique_mouse_caneta(&self) -> Option<Msg> {
        (self.tinta.auto_caneta && self.tinta.fonte_pressao.as_ref().is_some_and(FontePressao::entrega_toques))
            .then_some(Msg::Caneta(AjusteCaneta::CliqueIced))
    }

    /// A ponta da caneta pode ligar o modo desenhar sozinha agora?
    fn caneta_automatica(&self) -> bool {
        self.config.caneta_auto_desenhar
            && !self.tinta.ui.ativo
            && self.tinta.fonte_pressao.as_ref().is_some_and(FontePressao::entrega_toques)
    }

    /// Janela, menu ou caixa aberta por cima da grade: ali o toque da
    /// caneta é clique, não traço.
    fn algo_sobre_a_grade(&self) -> bool {
        self.contexto.is_some()
            || self.menu_aberto
            || self.nomes_modal.is_some()
            || self.link_modal.is_some()
            || self.dimensao_modal.is_some()
            || self.celulas_modal.is_some()
            || self.macro_pergunta.is_some()
            || self.protecao_modal.is_some()
            || self.congelar_menu_aberto
            || self.condicional_menu_aberto
            || self.tabelas_menu_aberto
            || self.estilos_menu_aberto
            || self.orientacao_menu_aberto
            || self.faixa_grupo_aberto.is_some()
            || self.faixa_extras_aberto
            || self.bordas_menu_aberto
            || self.colar_menu_aberto
            || self.estatisticas.is_some()
            || self.perfil_coluna.is_some()
            || self.subtotal.is_some()
            || self.novo_estilo.is_some()
            || self.colar_especial.is_some()
            || self.ir_para.is_some()
            || self.classificar.is_some()
            || self.atingir_meta.is_some()
            || self.tabela_dados.is_some()
            || self.cenarios.is_some()
            || self.avaliar.is_some()
            || self.criar_nomes.is_some()
            || self.formulas_menu.is_some()
    }

    /// Clique que o Iced recebeu com o modo ligado pela caneta: se não veio
    /// dela (nenhum toque recente), é o mouse — volta à
    /// seleção normal (o clique segue para a grade).
    fn clique_iced_no_modo_da_caneta(&mut self) -> Task<Msg> {
        if !self.tinta.auto_caneta {
            return Task::none();
        }
        let da_caneta = self.tinta.fonte_pressao.as_ref().is_some_and(|f| f.caneta_em_uso(std::time::Duration::from_millis(500)));
        if da_caneta || self.tinta.gesto_caneta.is_some() {
            return Task::none();
        }
        ui_caneta::anotar(|| "[caneta] mouse de volta: sai do modo desenhar".into());
        let tarefa = self.evento_desenhar(ui_tinta::Evento::Parar);
        self.status = tr!("Mouse: de volta à seleção de células.").into();
        tarefa
    }

    /// A ponta da caneta pela leitura X11: dentro da camada de tinta (modo
    /// desenho), o traço inteiro vem daqui, com a pressão exata de cada
    /// ponto; o clique do Iced, se vier, a camada descarta. Fora dela o
    /// toque é clique (do Iced ou do invólucro `feather_ui::clique_caneta`).
    pub(super) fn toques_caneta(&mut self, lote: Vec<ToqueCaneta>) -> Task<Msg> {
        let mut tarefas = Vec::new();
        for toque in lote {
            if let Some(evento) = self.toque_caneta(toque) {
                tarefas.push(self.evento_tinta(evento));
            }
        }
        Task::batch(tarefas)
    }

    fn toque_caneta(&mut self, toque: ToqueCaneta) -> Option<EventoCamada> {
        let fonte = self.tinta.fonte_pressao.clone()?;
        let transformacao = self.vista_tinta()?.transformacao();
        let zona = fonte.zona_tinta();
        let p_tela = |z: ui_caneta::ZonaTinta| pelicano::Point::new(toque.posicao.x - z.origem.x, toque.posicao.y - z.origem.y);
        match toque.fase {
            FaseToque::Encostou => {
                if !toque.na_tinta || self.tinta.ui.menu.is_some() {
                    return None;
                }
                let z = zona?;
                if !self.tinta.ui.ativo {
                    // A caneta na planilha liga o modo desenhar sozinha, com a
                    // última caneta usada; o primeiro toque já risca.
                    if !self.config.caneta_auto_desenhar {
                        return None;
                    }
                    let pena = self.tinta.ultima_pena.unwrap_or(Ferramenta::Caneta(0));
                    let _ = self.evento_desenhar(ui_tinta::Evento::Escolher(pena));
                    self.tinta.auto_caneta = true;
                    self.status = trf!("Caneta: {} (o mouse ou Esc voltam à seleção).", pena.rotulo());
                    ui_caneta::anotar(|| format!("[caneta] modo desenhar ligado pela caneta ({})", pena.rotulo()));
                }
                // A ponta de borracha apaga traços inteiros, seja qual for a ferramenta.
                let ferramenta = if toque.borracha { Ferramenta::BorrachaTraco } else { self.tinta.ui.ferramenta_ativa()? };
                let (gesto, evento) = GestoTinta::comecar_externo(ferramenta, p_tela(z), transformacao, toque.instante, toque.pressao);
                ui_caneta::anotar(|| format!("[caneta] traço pela leitura X11 começou ({})", ferramenta.rotulo()));
                self.tinta.gesto_caneta = Some(gesto);
                evento
            }
            FaseToque::Moveu => {
                let z = zona?;
                let gesto = self.tinta.gesto_caneta.as_mut()?;
                gesto.mover(p_tela(z), transformacao, toque.instante, toque.pressao)
            }
            FaseToque::Levantou => {
                let gesto = self.tinta.gesto_caneta.take()?;
                Some(gesto.terminar(Some(&fonte)))
            }
        }
    }

    /// Liga a leitura da pressão da mesa, se pedida nas Configurações (ou
    /// por `FEATHERSHEET_CANETA_X11=1`), e o diagnóstico. `janela` é a
    /// janela principal (no Windows a leitura é por subclasse dela).
    pub(super) fn ligar_caneta(&mut self, janela: window::Id) -> Task<Msg> {
        let diag = diagnostico_no_terminal();
        ui_caneta::definir_diagnostico(diag);
        #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
        if diag {
            match ui_caneta::x11::listar_dispositivos() {
                Ok(linhas) => {
                    for l in linhas {
                        ui_caneta::anotar(|| format!("[dispositivo] {l}"));
                    }
                }
                Err(e) => ui_caneta::anotar(|| format!("[dispositivo] XInput2 indisponível: {e}")),
            }
        }
        self.registrar_janela_e_mesa();
        if !crate::caneta_pressao_pedida(&self.config) {
            return Task::none();
        }
        self.iniciar_leitura_caneta(FontePressao::nova(), janela)
    }

    /// Console F12: onde a janela abriu e se há mesa (o clique da caneta
    /// só funciona pelo XWayland no Linux/Wayland).
    fn registrar_janela_e_mesa(&mut self) {
        use feather_ui::monitor::Nivel;
        if !cfg!(target_os = "linux") {
            return;
        }
        let mesas = crate::MESAS_NA_ABERTURA.get().cloned().unwrap_or_default();
        let quais = if mesas.is_empty() { tr!("nenhuma mesa detectada").to_string() } else { trf!("mesa: {}", mesas.join(", ")) };
        let forcado = crate::CANETA_X11_FORCADO.load(std::sync::atomic::Ordering::Relaxed);
        let wayland = std::env::var("WAYLAND_DISPLAY").is_ok_and(|v| !v.is_empty());
        let preferencia = crate::preferencia_janela(&self.config).texto();
        let (nivel, linha) = if forcado {
            (Nivel::Info, trf!("Janela pelo XWayland (preferência {preferencia}); {quais}."))
        } else if wayland && !mesas.is_empty() {
            (Nivel::Aviso, trf!("Janela Wayland nativa com {quais}: a ponta da caneta não clica aqui (o winit 0.30 não recebe a mesa no Wayland). Configurações > Planilha > Janela: Automático ou X11."))
        } else if wayland {
            (Nivel::Info, trf!("Janela Wayland nativa (preferência {preferencia}); {quais}."))
        } else {
            (Nivel::Info, trf!("Janela X11; {quais}."))
        };
        self.monitor.registrar(nivel, tr!("Caneta"), &linha);
        ui_caneta::anotar(|| format!("[janela] {linha}"));
    }

    /// Linux: XInput2 numa conexão X própria (a janela precisa ser X11).
    #[cfg(all(unix, not(target_os = "macos"), not(target_os = "android")))]
    fn iniciar_leitura_caneta(&mut self, fonte: FontePressao, janela: window::Id) -> Task<Msg> {
        use feather_ui::monitor::Nivel;
        let wayland = std::env::var("WAYLAND_DISPLAY").ok().filter(|v| !v.is_empty());
        if wayland.is_some() {
            // Janela nativa do Wayland: o XWayland não recebe a mesa.
            self.monitor.registrar(Nivel::Aviso, tr!("Caneta"),
                tr!("Pressão pedida, mas a janela abriu no Wayland (sem DISPLAY/XWayland): sem pressão."));
            return Task::none();
        }
        let forcado = crate::CANETA_X11_FORCADO.load(std::sync::atomic::Ordering::Relaxed);
        match ui_caneta::x11::iniciar(fonte.clone()) {
            Ok(descricao) => {
                let modo = if forcado { tr!("janela pelo XWayland") } else { tr!("sessão X11") };
                self.monitor.registrar(Nivel::Info, tr!("Caneta"), &trf!("Pressão ligada ({modo}). {descricao}"));
                self.tinta.fonte_pressao = Some(fonte.clone());
                // A leitura precisa da janela X do winit para saber onde a
                // ponta encostou (traço na tinta ou clique na faixa/células).
                Task::window_handle(janela, move |h| {
                    use crate::compat::window::raw_window_handle::RawWindowHandle;
                    let id = match h {
                        RawWindowHandle::Xlib(x) => u32::try_from(x.window).ok(),
                        RawWindowHandle::Xcb(x) => Some(x.window.get()),
                        _ => None,
                    };
                    match id {
                        Some(id) => {
                            fonte.definir_janela_x11(id);
                            ui_caneta::anotar(|| format!("[caneta-x11] janela do app: {id:#x}"));
                            String::new()
                        }
                        None => {
                            ui_caneta::anotar(|| "[caneta-x11] a janela não é X11: a ponta fica só com o Iced".into());
                            String::new()
                        }
                    }
                })
                .map(|erro| Msg::Caneta(AjusteCaneta::Falha(erro)))
            }
            Err(e) => {
                self.monitor.registrar(Nivel::Aviso, tr!("Caneta"), &trf!("Pressão indisponível: {e}"));
                Task::none()
            }
        }
    }

    /// Windows: WM_POINTER por subclasse da janela principal.
    #[cfg(windows)]
    fn iniciar_leitura_caneta(&mut self, fonte: FontePressao, janela: window::Id) -> Task<Msg> {
        self.tinta.fonte_pressao = Some(fonte.clone());
        Task::window_handle(janela, move |h| ui_caneta::windows::ligar_na_janela(h, fonte).err())
            .map(|erro| Msg::Caneta(AjusteCaneta::Falha(erro.unwrap_or_default())))
    }

    #[cfg(not(any(windows, all(unix, not(target_os = "macos"), not(target_os = "android")))))]
    fn iniciar_leitura_caneta(&mut self, _fonte: FontePressao, _janela: window::Id) -> Task<Msg> {
        Task::none()
    }

    pub(super) fn ajustar_caneta(&mut self, ajuste: AjusteCaneta) -> Task<Msg> {
        match ajuste {
            AjusteCaneta::Pressao(v) => {
                self.config.caneta_pressao = v;
                self.status = if v {
                    tr!("Pressão da caneta ligada: vale na próxima vez que o FeatherSheet abrir.").into()
                } else {
                    tr!("Pressão da caneta desligada (vale na próxima abertura).").into()
                };
            }
            AjusteCaneta::Janela(valor) => {
                if let Some(&(rotulo, v)) = crate::configuracao::JANELAS_LINUX.iter().find(|&&(_, v)| v == valor) {
                    self.config.janela_linux = v.into();
                    self.status = trf!("Janela: {rotulo} — vale na próxima vez que o FeatherSheet abrir.");
                }
            }
            AjusteCaneta::Curva(g) => {
                if let Some(&(nome, gama)) = crate::configuracao::CURVAS_CANETA.iter().find(|&&(_, x)| x == g) {
                    self.config.caneta_gama = gama;
                    self.status = trf!("Curva da pressão: {nome}.");
                }
            }
            AjusteCaneta::Velocidade(v) => self.config.caneta_velocidade = v,
            AjusteCaneta::MostrarPressao(v) => self.config.caneta_mostrar_pressao = v,
            AjusteCaneta::Toques(lote) => return self.toques_caneta(lote),
            AjusteCaneta::CliqueIced => return self.clique_iced_no_modo_da_caneta(),
            AjusteCaneta::AutoDesenhar(v) => {
                self.config.caneta_auto_desenhar = v;
                self.status = if v {
                    tr!("A caneta liga o modo desenhar ao tocar na planilha.").into()
                } else {
                    tr!("A caneta só desenha com Desenhar ligado (sem modo automático).").into()
                };
            }
            AjusteCaneta::Falha(erro) => {
                if !erro.is_empty() {
                    self.tinta.fonte_pressao = None;
                    self.monitor.registrar(feather_ui::monitor::Nivel::Aviso, tr!("Caneta"), &trf!("Pressão indisponível: {erro}"));
                }
                return Task::none();
            }
        }
        self.config.gravar();
        Task::none()
    }

    /// Delete com traços escolhidos pelo laço: apaga todos num Ctrl+Z.
    /// Devolve se havia seleção (a tecla não vai para as células).
    pub(super) fn apagar_selecao_tinta(&mut self) -> bool {
        let Some((aba_id, ids)) = self.tinta.selecao.take() else { return false };
        let Some((atual, mut tinta)) = self.tinta_atual() else { return false };
        if atual != aba_id || !self.tinta.ui.ativo {
            return false;
        }
        for id in &ids {
            tinta.remover(*id);
        }
        self.gravar_tinta(tinta, trf!("{} traço(s) apagado(s) (Ctrl+Z desfaz).", ids.len()));
        true
    }

    /// Esc no modo desenho: fecha o menu da caneta ou sai do modo.
    pub(super) fn esc_desenhar(&mut self) -> Option<Task<Msg>> {
        if self.tinta.ui.menu.is_some() {
            return Some(self.evento_desenhar(ui_tinta::Evento::FecharMenu));
        }
        self.tinta.ui.ativo.then(|| self.evento_desenhar(ui_tinta::Evento::Parar))
    }

    /// `FEATHERSHEET_CAPTURAR_TINTA=1` (ou `menu`): abre a aba Desenhar com
    /// a caneta ativa e alguns traços de exemplo (à mão, marca-texto e
    /// formas reconhecidas); `menu` deixa aberto o menu da Caneta 2.
    pub(super) fn preparar_captura_tinta(&mut self, modo: &str) {
        self.faixa_ativa = AbaFaixa::Desenhar;
        let traco = |app: &mut App, f: Ferramenta, pts: Vec<(f32, f32)>| {
            let pontos = pts.into_iter().enumerate()
                .map(|(i, (x, y))| ui_tinta::PontoCaptura::novo(x, y, i as u32 * 8))
                .collect();
            let _ = app.evento_tinta(EventoCamada::TracoTerminado { ferramenta: f, pontos });
        };
        let onda: Vec<(f32, f32)> = (0..120).map(|i| {
            let x = 40.0 + i as f32 * 3.0;
            (x, 70.0 + 18.0 * (i as f32 / 9.0).sin() + (i as f32 * 1.7).sin())
        }).collect();
        let _ = self.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(0)));
        traco(self, Ferramenta::Caneta(0), onda);
        let _ = self.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::MarcaTexto));
        traco(self, Ferramenta::MarcaTexto, (0..60).map(|i| (60.0 + i as f32 * 5.0, 140.0 + (i as f32 * 0.9).sin())).collect());
        // Formas desenhadas tortas, convertidas pela "Tinta para forma".
        let _ = self.evento_desenhar(ui_tinta::Evento::AlternarFormas);
        let _ = self.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(1)));
        let mut ret = Vec::new();
        for (a, b) in [((520.0, 40.0), (760.0, 44.0)), ((760.0, 44.0), (757.0, 170.0)), ((757.0, 170.0), (522.0, 166.0)), ((522.0, 166.0), (521.0, 42.0))] {
            for k in 0..30 {
                let t = k as f32 / 30.0;
                ret.push((a.0 + (b.0 - a.0) * t + (k as f32 * 2.1).sin() * 1.5, a.1 + (b.1 - a.1) * t + (k as f32 * 1.3).cos() * 1.5));
            }
        }
        traco(self, Ferramenta::Caneta(1), ret);
        let _ = self.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(2)));
        traco(self, Ferramenta::Caneta(2), (0..=104).map(|i| {
            let a = std::f32::consts::TAU * i as f32 / 100.0;
            (900.0 + 110.0 * a.cos() + (i as f32 * 1.9).sin() * 2.0, 120.0 + 70.0 * a.sin() + (i as f32 * 2.3).cos() * 2.0)
        }).collect());
        let mut seta: Vec<(f32, f32)> = (0..50).map(|i| (80.0 + i as f32 * 6.0, 260.0 + (i as f32 * 1.1).sin())).collect();
        seta.extend((0..12).map(|i| (380.0 - i as f32 * 2.5, 260.0 - i as f32 * 1.6)));
        seta.extend((0..12).map(|i| (350.0 + i as f32 * 2.5, 240.8 + i as f32 * 1.6)));
        seta.extend((0..12).map(|i| (380.0 - i as f32 * 2.5, 260.0 + i as f32 * 1.6)));
        traco(self, Ferramenta::Caneta(2), seta);
        let _ = self.evento_desenhar(ui_tinta::Evento::AlternarFormas);
        // Assinatura com pressão de mesa (fina → grossa → fina), como a XP-Pen entrega.
        let _ = self.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(0)));
        let assinatura = (0..160)
            .map(|i| {
                let t = i as f32 / 159.0;
                let mut p = ui_tinta::PontoCaptura::novo(
                    80.0 + t * 380.0,
                    360.0 + 22.0 * (t * 18.0).sin() * (1.0 - t * 0.4),
                    i * 6,
                );
                p.pressao = Some((t * std::f32::consts::PI).sin().powf(0.8).max(0.05));
                p
            })
            .collect();
        let espessura_antes = self.tinta.ui.canetas[0].espessura;
        self.tinta.ui.canetas[0].espessura = 6.0;
        let _ = self.evento_tinta(EventoCamada::TracoTerminado { ferramenta: Ferramenta::Caneta(0), pontos: assinatura });
        self.tinta.ui.canetas[0].espessura = espessura_antes;
        if modo == "menu" {
            self.ponteiro_x = 110.0;
            self.ponteiro_y = 150.0;
            let _ = self.evento_desenhar(ui_tinta::Evento::AlternarMenu(Ferramenta::Caneta(1)));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn app() -> App {
        let mut app = App::default();
        app.config.zoom = 100;
        app
    }

    fn rabisco(app: &mut App, f: Ferramenta, pts: &[(f32, f32)]) {
        let pontos = pts.iter().map(|&(x, y)| ui_tinta::PontoCaptura::novo(x, y, 0)).collect();
        let _ = app.evento_tinta(EventoCamada::TracoTerminado { ferramenta: f, pontos });
    }

    fn tn(app: &App) -> Option<String> {
        app.workbook.aba(0).unwrap().sheet.tn.clone()
    }

    #[test]
    fn traco_vira_um_desfazer_e_sobrevive_ao_fshz() {
        let mut app = app();
        let _ = app.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(0)));
        assert!(app.tinta.ui.ativo);
        rabisco(&mut app, Ferramenta::Caneta(0), &[(10.0, 10.0), (60.0, 40.0), (130.0, 20.0)]);
        let um = tn(&app).expect("o traço vai para o tn da aba");
        rabisco(&mut app, Ferramenta::Caneta(0), &[(10.0, 100.0), (60.0, 140.0)]);
        assert_eq!(TintaAba::ler(tn(&app).as_deref()).unwrap().tinta.len(), 2);
        // Um Ctrl+Z tira só o último traço; outro, o primeiro; Ctrl+Y devolve.
        app.historico(false);
        assert_eq!(tn(&app).as_deref(), Some(um.as_str()));
        app.historico(false);
        assert!(tn(&app).is_none());
        app.historico(true);
        assert_eq!(tn(&app).as_deref(), Some(um.as_str()));
        // Salvar e abrir o .fshz.
        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(&app.workbook, 0).unwrap();
        let aberta = feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&bytes).unwrap());
        let lida = TintaAba::ler(aberta.aba(0).unwrap().sheet.tn.as_deref()).unwrap();
        assert_eq!(lida, TintaAba::ler(Some(&um)).unwrap());
        assert_eq!(lida.ancora(lida.tinta.tracos()[0].id), (0, 0));
    }

    #[test]
    fn apagar_arrastando_e_um_desfazer_e_laco_seleciona_para_delete() {
        let mut app = app();
        rabisco(&mut app, Ferramenta::Caneta(0), &[(10.0, 10.0), (200.0, 10.0)]);
        rabisco(&mut app, Ferramenta::Caneta(1), &[(10.0, 300.0), (200.0, 300.0)]);
        let antes = tn(&app);
        let _ = app.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::BorrachaPonto));
        for x in [90.0, 100.0, 110.0] {
            let _ = app.evento_tinta(EventoCamada::Apagar { ferramenta: Ferramenta::BorrachaPonto, ponto: pelicano::Point::new(x, 10.0), raio: 4.0 });
        }
        // Durante o arrasto nada vai para o histórico; a tela já mostra o corte
        // (um buraco só: o trecho entre dois eventos também é apagado).
        assert_eq!(tn(&app), antes);
        assert_eq!(app.tracos_tela().0.len(), 3);
        let _ = app.evento_tinta(EventoCamada::ApagarFim);
        assert_eq!(TintaAba::ler(tn(&app).as_deref()).unwrap().tinta.len(), 3);
        app.historico(false);
        assert_eq!(tn(&app), antes, "o apagar inteiro desfaz num Ctrl+Z só");
        // Laço em volta do traço de baixo + Delete.
        let _ = app.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Laco));
        let _ = app.evento_tinta(EventoCamada::Laco(vec![
            pelicano::Point::new(0.0, 280.0), pelicano::Point::new(260.0, 280.0),
            pelicano::Point::new(260.0, 330.0), pelicano::Point::new(0.0, 330.0),
        ]));
        assert_eq!(app.tinta.selecao.as_ref().map(|s| s.1.len()), Some(1));
        assert!(app.apagar_selecao_tinta());
        assert_eq!(TintaAba::ler(tn(&app).as_deref()).unwrap().tinta.len(), 1);
        assert!(app.esc_desenhar().is_some());
        assert!(!app.tinta.ui.ativo);
    }

    #[test]
    fn tinta_para_forma_troca_o_traco_pela_forma() {
        let mut app = app();
        let _ = app.evento_desenhar(ui_tinta::Evento::AlternarFormas);
        let torta: Vec<(f32, f32)> = (0..40).map(|i| (20.0 + i as f32 * 5.0, 50.0 + (i as f32 * 1.7).sin())).collect();
        rabisco(&mut app, Ferramenta::Caneta(0), &torta);
        let aba = TintaAba::ler(tn(&app).as_deref()).unwrap();
        let t = &aba.tinta.tracos()[0];
        assert!(t.reto && t.pontos.len() == 2, "{t:?}");
    }

    #[test]
    fn tinta_acompanha_rolagem_zoom_e_largura_de_coluna() {
        let mut app = app();
        rabisco(&mut app, Ferramenta::Caneta(0), &[(300.0, 100.0), (340.0, 120.0)]);
        let aba = TintaAba::ler(tn(&app).as_deref()).unwrap();
        // 300 px com colunas de 122: começa na coluna C (índice 2), linha 3 (28 px).
        let id = aba.tinta.tracos()[0].id;
        assert_eq!(aba.ancora(id), (3, 2));
        let (tracos, t) = app.tracos_tela();
        assert_eq!(tracos[0].pontos[0], pelicano::Point::new(300.0, 100.0));
        assert_eq!(t.para_tela(tracos[0].pontos[0]), pelicano::Point::new(400.0, 130.0));
        // Zoom 200 %: a mesma vista, a tela dobra.
        app.config.zoom = 200;
        let (tracos, t) = app.tracos_tela();
        assert_eq!(t.para_tela(tracos[0].pontos[0]), pelicano::Point::new(800.0, 260.0));
        app.config.zoom = 100;
        // Rolar uma coluna e uma linha para a direita/baixo.
        app.coluna_inicio = 1;
        app.linha_inicio = 1;
        let (tracos, _) = app.tracos_tela();
        assert_eq!(tracos[0].pontos[0], pelicano::Point::new(300.0 - 122.0, 100.0 - 28.0));
        // A coluna A fica 50 px mais larga: o traço (preso em C) anda 50 px.
        app.coluna_inicio = 0;
        app.linha_inicio = 0;
        app.workbook.editar(0, |s| { s.col_widths.insert(0, 172); });
        let (tracos, _) = app.tracos_tela();
        assert_eq!(tracos[0].pontos[0], pelicano::Point::new(350.0, 100.0));
        // E desenhar já rolado grava no lugar certo da planilha.
        app.coluna_inicio = 2;
        rabisco(&mut app, Ferramenta::Caneta(0), &[(10.0, 10.0), (20.0, 20.0)]);
        let aba = TintaAba::ler(tn(&app).as_deref()).unwrap();
        let novo = aba.tinta.tracos()[1].id;
        // 10 px depois da coluna C (que começa em 172 + 122 = 294).
        assert_eq!(aba.ancora(novo), (0, 2));
        assert_eq!(aba.tinta.tracos()[1].pontos[0].ponto(), Ponto::novo(10.0, 10.0));
    }

    #[test]
    fn faixa_desenhar_monta_e_menu_abre_flutuando() {
        let mut app = app();
        app.faixa_ativa = AbaFaixa::Desenhar;
        assert_eq!(app.montar_grupos_faixa().len(), app.faixa_larguras().len());
        let _ = app.evento_desenhar(ui_tinta::Evento::AlternarMenu(Ferramenta::Caneta(2)));
        assert_eq!(app.tinta.ui.menu, Some(Ferramenta::Caneta(2)));
        let _: Element<'_, Msg> = app.sobrepor_menu_tinta(text("base").into());
        let _: Element<'_, Msg> = app.grade_com_tinta(text("grade").into());
        assert!(app.esc_desenhar().is_some());
        assert!(app.tinta.ui.menu.is_none());
    }

    fn ponto(x: f32, y: f32, t: u32, p: Option<f32>) -> ui_tinta::PontoCaptura {
        let mut c = ui_tinta::PontoCaptura::novo(x, y, t);
        c.pressao = p;
        c
    }

    fn primeiro_traco(app: &App) -> Traco {
        TintaAba::ler(tn(app).as_deref()).unwrap().tinta.tracos()[0].clone()
    }

    #[test]
    fn pressao_da_mesa_passa_pela_curva_e_vira_contorno_na_tela() {
        let mut app = app();
        app.config.caneta_gama = 0.5;
        let pontos = (0..30).map(|i| ponto(10.0 + i as f32 * 8.0, 40.0, i * 10, Some(0.25))).collect();
        let _ = app.evento_tinta(EventoCamada::TracoTerminado { ferramenta: Ferramenta::Caneta(0), pontos });
        let t = primeiro_traco(&app);
        assert!(!t.pressao_sintetica);
        // 0,25 com gama 0,5 = 0,5 gravado (pressão de resposta).
        assert!(t.pontos.iter().all(|p| (p.pressao.unwrap() - 0.5).abs() < 1e-3), "{t:?}");
        let (tracos, _) = app.tracos_tela();
        assert!(!tracos[0].contorno.is_empty(), "caneta com pressão desenha o contorno");
        // O cache devolve o mesmo contorno no quadro seguinte, e acompanha a rolagem.
        assert_eq!(app.tracos_tela().0[0].contorno, tracos[0].contorno);
        app.linha_inicio = 1;
        let rolado = app.tracos_tela().0[0].contorno[0][0];
        assert!((rolado.y - (tracos[0].contorno[0][0].y - 28.0)).abs() < 1e-3);
        // E o Console F12 ganhou a linha do traço.
        assert!(app.monitor.console().registros().any(|r| r.origem == "Caneta" && r.mensagem.contains("pressão da mesa 0,50")));
    }

    #[test]
    fn sem_mesa_espessura_pela_velocidade_marcada_como_sintetica() {
        let mut app = app();
        app.config.caneta_velocidade = true;
        let devagar_rapido: Vec<_> = (0..20).map(|i| ponto(i as f32, 0.0, i * 10, None))
            .chain((0..20).map(|i| ponto(20.0 + i as f32 * 10.0, 0.0, 200 + i, None)))
            .collect();
        let _ = app.evento_tinta(EventoCamada::TracoTerminado { ferramenta: Ferramenta::Caneta(0), pontos: devagar_rapido.clone() });
        let t = primeiro_traco(&app);
        assert!(t.pressao_sintetica && t.largura_variavel());
        let pr: Vec<f32> = t.pontos.iter().filter_map(|p| p.pressao).collect();
        assert!(pr.first().unwrap() > pr.last().unwrap(), "rápido = mais fino: {pr:?}");
        // Desligado: largura fixa, sem pressão nenhuma.
        let mut app = self::app();
        app.config.caneta_velocidade = false;
        let _ = app.evento_tinta(EventoCamada::TracoTerminado { ferramenta: Ferramenta::Caneta(0), pontos: devagar_rapido });
        let t = primeiro_traco(&app);
        assert!(!t.tem_pressao() && !t.pressao_sintetica);
        assert!(app.tracos_tela().0[0].contorno.is_empty());
    }

    #[test]
    fn marca_texto_ignora_a_pressao() {
        let mut app = app();
        let pontos = (0..10).map(|i| ponto(i as f32 * 10.0, 5.0, i * 10, Some(0.3))).collect();
        let _ = app.evento_tinta(EventoCamada::TracoTerminado { ferramenta: Ferramenta::MarcaTexto, pontos });
        let t = primeiro_traco(&app);
        assert!(!t.tem_pressao());
        assert!(app.tracos_tela().0[0].contorno.is_empty());
    }

    #[test]
    fn previa_da_velocidade_no_feather_ui_e_igual_ao_motor() {
        for v in [0.0, 0.3, 1.0, 1.99, 2.0, 5.0, f32::NAN] {
            for vf in [2.0, 0.5, 0.0] {
                assert_eq!(
                    ui_caneta::previa_pressao_da_velocidade(v, vf),
                    feather_draw::tinta::pressao_da_velocidade(v, vf),
                    "v={v} vf={vf}"
                );
            }
        }
    }

    #[test]
    fn ajustes_da_caneta_vao_para_a_configuracao() {
        let mut app = app();
        assert!(app.config.caneta_pressao, "ligado por padrão");
        assert_eq!(app.config.janela_linux, "auto");
        let _ = app.update(Msg::Caneta(AjusteCaneta::Pressao(false)));
        assert!(!app.config.caneta_pressao && app.status.contains("próxima abertura"));
        let _ = app.update(Msg::Caneta(AjusteCaneta::Janela("x11")));
        assert_eq!(app.config.janela_linux, "x11");
        assert!(app.status.contains("XWayland"));
        let _ = app.update(Msg::Caneta(AjusteCaneta::Janela("outra")));
        assert_eq!(app.config.janela_linux, "x11", "valor desconhecido é ignorado");
        // O Console registra onde a janela abriu (sem panicar em teste, sem mesa conhecida).
        app.registrar_janela_e_mesa();
        let _ = app.update(Msg::Caneta(AjusteCaneta::Curva(1.5)));
        assert_eq!(app.config.caneta_gama, 1.5);
        let _ = app.update(Msg::Caneta(AjusteCaneta::Curva(3.3)));
        assert_eq!(app.config.caneta_gama, 1.5, "curva fora da lista é ignorada");
        let _ = app.update(Msg::Caneta(AjusteCaneta::MostrarPressao(true)));
        let _ = app.update(Msg::Caneta(AjusteCaneta::Velocidade(false)));
        assert!(app.config.caneta_mostrar_pressao && !app.config.caneta_velocidade);
        // A camada recebe a curva e o indicador.
        app.tinta.ui.ativo = true;
        let _: Element<'_, Msg> = app.grade_com_tinta(text("grade").into());
        let _ = app.update(Msg::Caneta(AjusteCaneta::Falha("x".into())));
        assert!(app.tinta.fonte_pressao.is_none());
    }

    /// A ponta da caneta pela leitura X11 (o servidor não entrega o botão ao
    /// winit): o traço sai inteiro dos toques, com a pressão de cada ponto,
    /// convertido da janela para o documento, e é um Ctrl+Z.
    #[test]
    fn toques_da_caneta_lidos_por_fora_viram_traco_com_pressao() {
        use feather_ui::caneta::ZonaTinta;
        use std::time::{Duration, Instant};
        let mut app = app();
        let fonte = FontePressao::nova();
        fonte.definir_janela_x11(0x40_0001);
        app.tinta.fonte_pressao = Some(fonte.clone());
        let _ = app.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(0)));
        // A camada começa em (0, 100) da janela; a área útil fica depois dos cabeçalhos.
        let z = 1.0;
        let origem = pelicano::Point::new(0.0, 100.0);
        let area = crate::compat::Rectangle::new(
            pelicano::Point::new(origem.x + CABECALHO_X * z, origem.y + CABECALHO_Y * z),
            pelicano::Size::new(800.0, 500.0),
        );
        fonte.definir_zona_tinta(Some(ZonaTinta { area, origem }));
        let t0 = Instant::now();
        let toque = |fase, x: f32, y: f32, ms: u64, pressao: Option<f32>| ToqueCaneta {
            fase,
            posicao: pelicano::Point::new(x, y),
            pressao,
            instante: t0 + Duration::from_millis(ms),
            na_tinta: true,
            borracha: false,
        };
        let lote = vec![
            toque(FaseToque::Encostou, 200.0, 200.0, 0, None),
            toque(FaseToque::Moveu, 200.5, 200.0, 4, Some(0.25)),
            toque(FaseToque::Moveu, 240.0, 210.0, 8, Some(0.9)),
        ];
        let _ = app.update(Msg::Caneta(AjusteCaneta::Toques(lote)));
        assert!(app.tinta.gesto_caneta.is_some(), "traço em andamento (prévia na camada)");
        assert!(tn(&app).is_none());
        let _ = app.update(Msg::Caneta(AjusteCaneta::Toques(vec![
            toque(FaseToque::Moveu, 280.0, 230.0, 12, Some(0.5)),
            toque(FaseToque::Levantou, 280.0, 230.0, 16, None),
        ])));
        assert!(app.tinta.gesto_caneta.is_none());
        let aba = TintaAba::ler(tn(&app).as_deref()).expect("o traço foi gravado");
        let tracos = aba.tinta.tracos();
        assert_eq!(tracos.len(), 1);
        assert!(tracos[0].tem_pressao(), "pressão real da mesa, não a da velocidade");
        // Um toque fora da camada (faixa) não risca: é clique.
        let _ = app.update(Msg::Caneta(AjusteCaneta::Toques(vec![
            ToqueCaneta { na_tinta: false, ..toque(FaseToque::Encostou, 50.0, 20.0, 100, Some(0.5)) },
            ToqueCaneta { na_tinta: false, ..toque(FaseToque::Levantou, 50.0, 20.0, 120, None) },
        ])));
        assert_eq!(TintaAba::ler(tn(&app).as_deref()).unwrap().tinta.len(), 1);
        // Fora do modo desenho, sem o modo automático, a zona some (o toque
        // vira clique na grade).
        app.config.caneta_auto_desenhar = false;
        let _ = app.evento_desenhar(ui_tinta::Evento::Parar);
        let _ = app.grade_com_tinta(pelicano::text("grade").into());
        assert!(fonte.zona_tinta().is_none());
    }

    /// Prepara a leitura da caneta "por fora" com a camada em (0, 100).
    fn caneta_ligada(app: &mut App) -> FontePressao {
        use feather_ui::caneta::ZonaTinta;
        let fonte = FontePressao::nova();
        fonte.definir_janela_x11(0x40_0001);
        app.tinta.fonte_pressao = Some(fonte.clone());
        let origem = pelicano::Point::new(0.0, 100.0);
        let area = crate::compat::Rectangle::new(pelicano::Point::new(CABECALHO_X, 100.0 + CABECALHO_Y), pelicano::Size::new(800.0, 500.0));
        fonte.definir_zona_tinta(Some(ZonaTinta { area, origem }));
        fonte
    }

    /// Como na leitura real: o toque passa pela fonte (que marca o sinal da
    /// caneta) e chega ao app em lote.
    fn entregar(app: &mut App, toques: Vec<ToqueCaneta>) {
        let fonte = app.tinta.fonte_pressao.clone().unwrap();
        for t in toques {
            fonte.enviar_toque(t);
        }
        let _ = app.update(Msg::Caneta(AjusteCaneta::Toques(fonte.recolher_toques())));
    }

    fn toque_em(fase: FaseToque, x: f32, y: f32, pressao: Option<f32>, borracha: bool) -> ToqueCaneta {
        ToqueCaneta { fase, posicao: pelicano::Point::new(x, y), pressao, instante: std::time::Instant::now(), na_tinta: true, borracha }
    }

    /// Caneta na planilha com o modo desenhar desligado: liga sozinho com a
    /// última caneta, o primeiro toque já risca; o mouse ou Esc saem.
    #[test]
    fn caneta_liga_o_modo_desenhar_sozinha_e_o_mouse_ou_esc_saem() {
        let mut app = app();
        assert!(app.config.caneta_auto_desenhar, "ligado por padrão");
        let fonte = caneta_ligada(&mut app);
        // A última caneta escolhida à mão foi a 2; depois parou.
        let _ = app.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(1)));
        let _ = app.evento_desenhar(ui_tinta::Evento::Parar);
        assert!(!app.tinta.ui.ativo && app.caneta_automatica());
        entregar(&mut app, vec![
            toque_em(FaseToque::Encostou, 200.0, 200.0, Some(0.5), false),
            toque_em(FaseToque::Moveu, 240.0, 220.0, Some(0.7), false),
            toque_em(FaseToque::Levantou, 240.0, 220.0, None, false),
        ]);
        assert!(app.tinta.ui.ativo && app.tinta.auto_caneta);
        assert_eq!(app.tinta.ui.ferramenta, Ferramenta::Caneta(1));
        let aba = TintaAba::ler(tn(&app).as_deref()).expect("o primeiro toque já riscou");
        assert_eq!(aba.tinta.len(), 1);
        // Clique que o Iced recebe logo depois do toque é da caneta: fica.
        let _ = app.update(Msg::Caneta(AjusteCaneta::CliqueIced));
        assert!(app.tinta.ui.ativo);
        // Mouse (sem sinal da caneta há mais de 0,5 s): volta à seleção.
        let fonte_mouse = FontePressao::nova();
        fonte_mouse.definir_janela_x11(0x40_0001);
        app.tinta.fonte_pressao = Some(fonte_mouse);
        let _ = app.update(Msg::Caneta(AjusteCaneta::CliqueIced));
        assert!(!app.tinta.ui.ativo && !app.tinta.auto_caneta);
        // Esc também sai.
        app.tinta.fonte_pressao = Some(fonte.clone());
        entregar(&mut app, vec![toque_em(FaseToque::Encostou, 300.0, 300.0, Some(0.5), false)]);
        assert!(app.tinta.ui.ativo && app.tinta.auto_caneta);
        entregar(&mut app, vec![toque_em(FaseToque::Levantou, 300.0, 300.0, None, false)]);
        assert!(app.esc_desenhar().is_some());
        assert!(!app.tinta.ui.ativo && !app.tinta.auto_caneta);
        // Modo escolhido à mão não sai com o mouse.
        let _ = app.evento_desenhar(ui_tinta::Evento::Escolher(Ferramenta::Caneta(0)));
        app.tinta.fonte_pressao = Some(FontePressao::nova());
        let _ = app.update(Msg::Caneta(AjusteCaneta::CliqueIced));
        assert!(app.tinta.ui.ativo);
    }

    /// Opção desligada: a caneta fora do modo desenhar não risca. Toque fora
    /// da planilha (faixa, abas) nunca liga o modo. A borracha da caneta apaga.
    #[test]
    fn opcao_desligada_toque_fora_da_planilha_e_borracha_da_caneta() {
        let mut app = app();
        let _ = caneta_ligada(&mut app);
        let _ = app.update(Msg::Caneta(AjusteCaneta::AutoDesenhar(false)));
        assert!(!app.config.caneta_auto_desenhar && !app.caneta_automatica());
        entregar(&mut app, vec![toque_em(FaseToque::Encostou, 200.0, 200.0, Some(0.5), false)]);
        assert!(!app.tinta.ui.ativo && app.tinta.gesto_caneta.is_none());
        entregar(&mut app, vec![toque_em(FaseToque::Levantou, 200.0, 200.0, None, false)]);
        app.config.caneta_auto_desenhar = true;
        let fora = ToqueCaneta { na_tinta: false, ..toque_em(FaseToque::Encostou, 50.0, 20.0, Some(0.5), false) };
        entregar(&mut app, vec![fora]);
        assert!(!app.tinta.ui.ativo, "faixa: é clique");
        // Janela aberta por cima da grade: a camada não anuncia zona.
        app.menu_aberto = true;
        assert!(app.algo_sobre_a_grade());
        app.menu_aberto = false;
        // Um traço e depois a ponta de borracha passando por cima dele.
        entregar(&mut app, vec![
            toque_em(FaseToque::Encostou, 200.0, 250.0, Some(0.5), false),
            toque_em(FaseToque::Moveu, 300.0, 250.0, Some(0.5), false),
            toque_em(FaseToque::Levantou, 300.0, 250.0, None, false),
        ]);
        assert_eq!(TintaAba::ler(tn(&app).as_deref()).unwrap().tinta.len(), 1);
        entregar(&mut app, vec![
            toque_em(FaseToque::Encostou, 250.0, 230.0, Some(0.5), true),
            toque_em(FaseToque::Moveu, 250.0, 270.0, Some(0.5), true),
            toque_em(FaseToque::Levantou, 250.0, 270.0, None, true),
        ]);
        assert!(TintaAba::ler(tn(&app).as_deref()).map_or(true, |a| a.tinta.len() == 0), "a borracha da caneta apagou");
        assert_eq!(app.tinta.ui.ferramenta, Ferramenta::Caneta(0), "a ferramenta escolhida continua a caneta");
    }
}
