//! O `Canvas` dos objetos soltos por cima da grade.
//!
//! **Desempenho do arraste:** o gesto inteiro (mover, alças, giro, ajuste,
//! criar) vive no estado do Canvas; nada vai ao `Sheet`, ao JSON ou ao
//! desfazer durante o arraste — só a mensagem final ao soltar. Enquanto o
//! gesto dura, [`arrastando`] faz o app ignorar o `CursorMoved` global
//! (que reconstruía a tela inteira a cada movimento); só o Canvas redesenha.
//! Os objetos parados não têm cache próprio: o pelicano só repinta o que
//! mudou na tela (o `canvas::Cache` do Iced saiu).
//! A forma em movimento sai inteira (preenchimento, contorno, cantos).
use super::geometria::{self, Desenho};
use super::{Evento, Medida};
use crate::app::desenhar::Vista;
use crate::app::{Msg, COR_AZUL_ACENTO, COR_PAINEL};
use feather_draw::{Caminho, Comando, Ponto, Retangulo as RetDraw, Transformacao as TransfDraw};
use feathersheet_io::objetos_aba::{self as modelo, ObjetoSolto, Regua, Retangulo};
use crate::compat::canvas as cv; use crate::compat::canvas::Frame; use crate::compat::canvas::Path; use crate::compat::canvas::Program; use crate::compat::canvas::Stroke;
use crate::compat::{Distancia as _, Retangulo as _};
use crate::compat::alignment; use crate::compat::mouse; use pelicano::{Background, Captured, Color, Event, MouseButton}; use pelicano::Point; use crate::compat::Rectangle; use pelicano::Renderer; use pelicano::Theme; use crate::compat::Vector;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

pub(super) const CAB_X: f32 = crate::grade::CABECALHO_LINHA;
pub(super) const CAB_Y: f32 = crate::grade::CABECALHO_COLUNA;
/// Barras de rolagem desenhadas pela grade (não escalam com o zoom).
const BARRA: f32 = 13.0;
/// Lado da alça, em px de tela.
const ALCA: f32 = 8.0;
/// Distância da alça de giro acima da moldura, em px de tela.
pub(super) const DIST_GIRO: f32 = 22.0;
/// Folga do clique, em px de tela (linha fina também pega).
const FOLGA: f32 = 4.0;
/// Tamanho da forma criada por clique simples, em px lógicos.
pub(super) const TAMANHO_PADRAO: (f32, f32) = (120.0, 80.0);
/// Arraste menor que isto (px de tela) conta como clique.
const ARRASTE_MINIMO: f32 = 3.0;
/// Dois cliques na mesma forma dentro deste tempo = duplo clique.
const DUPLO_CLIQUE_MS: u128 = 450;

static ARRASTANDO: AtomicBool = AtomicBool::new(false);

/// Há um gesto da camada de objetos em andamento? O app deixa de
/// transformar o `CursorMoved` global em mensagem enquanto for `true`.
pub(in crate::app) fn arrastando() -> bool {
    ARRASTANDO.load(Ordering::Relaxed)
}

pub(super) fn soltar_arraste() {
    ARRASTANDO.store(false, Ordering::Relaxed);
}

/// Gesto do mouse sobre a camada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum TipoGesto {
    Mover,
    Alca(Alca),
    Girar,
    /// Alça de ajuste (índice da `geometria::alcas`).
    Ajuste(usize),
    Criar(&'static str),
    /// Clique numa forma com macro: executa ao soltar sem arrastar.
    Macro,
}

/// As oito alças de redimensionar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Alca { Nw, N, Ne, E, Se, S, Sw, W }

impl Alca {
    const TODAS: [Alca; 8] = [Alca::Nw, Alca::N, Alca::Ne, Alca::E, Alca::Se, Alca::S, Alca::Sw, Alca::W];
    fn norte(self) -> bool { matches!(self, Alca::Nw | Alca::N | Alca::Ne) }
    fn sul(self) -> bool { matches!(self, Alca::Sw | Alca::S | Alca::Se) }
    fn leste(self) -> bool { matches!(self, Alca::Ne | Alca::E | Alca::Se) }
    fn oeste(self) -> bool { matches!(self, Alca::Nw | Alca::W | Alca::Sw) }
    fn canto(self) -> bool { matches!(self, Alca::Nw | Alca::Ne | Alca::Se | Alca::Sw) }
    fn fracao(self) -> (f32, f32) {
        let fx = if self.oeste() { 0.0 } else if self.leste() { 1.0 } else { 0.5 };
        let fy = if self.norte() { 0.0 } else if self.sul() { 1.0 } else { 0.5 };
        (fx, fy)
    }
    fn oposta(self) -> Alca {
        match self {
            Alca::Nw => Alca::Se, Alca::N => Alca::S, Alca::Ne => Alca::Sw, Alca::E => Alca::W,
            Alca::Se => Alca::Nw, Alca::S => Alca::N, Alca::Sw => Alca::Ne, Alca::W => Alca::E,
        }
    }
    fn cursor(self) -> mouse::Interaction {
        match self {
            Alca::Nw | Alca::Se => mouse::Interaction::NwseResize,
            Alca::Ne | Alca::Sw => mouse::Interaction::NeswResize,
            Alca::N | Alca::S => mouse::Interaction::NsResize,
            Alca::E | Alca::W => mouse::Interaction::EwResize,
        }
    }
}

// ------------------------------------------------------------ geometria pura

pub(super) fn girar_em_volta(p: (f32, f32), c: (f32, f32), graus: f32) -> (f32, f32) {
    let (s, co) = graus.to_radians().sin_cos();
    let (x, y) = (p.0 - c.0, p.1 - c.1);
    (c.0 + x * co - y * s, c.1 + x * s + y * co)
}

/// Mover pelo deslocamento `d` (px lógicos); não passa de A1.
pub(super) fn mover(base: Retangulo, d: (f32, f32)) -> Retangulo {
    Retangulo { x: (base.x + d.0).max(0.0), y: (base.y + d.1).max(0.0), ..base }
}

/// Redimensionar pela alça com o objeto girado `giro` graus: o
/// deslocamento entra no sistema da forma, o lado oposto fica parado na
/// tela e `manter` (Shift ou "Travar proporção") preserva a proporção nos cantos.
pub(super) fn redimensionar(base: Retangulo, giro: f32, alca: Alca, d: (f32, f32), manter: bool, minimo: f32) -> Retangulo {
    let (s, c) = giro.to_radians().sin_cos();
    let (lx, ly) = (d.0 * c + d.1 * s, -d.0 * s + d.1 * c);
    let (mut x0, mut y0, mut x1, mut y1) = (base.x, base.y, base.x + base.largura, base.y + base.altura);
    if alca.leste() { x1 = (x1 + lx).max(x0 + minimo); }
    if alca.oeste() { x0 = (x0 + lx).min(x1 - minimo); }
    if alca.sul() { y1 = (y1 + ly).max(y0 + minimo); }
    if alca.norte() { y0 = (y0 + ly).min(y1 - minimo); }
    if manter && alca.canto() && base.largura > 0.0 && base.altura > 0.0 {
        let proporcao = base.largura / base.altura;
        let (mut w, mut h) = (x1 - x0, y1 - y0);
        if w / base.largura >= h / base.altura { h = w / proporcao } else { w = h * proporcao }
        if alca.oeste() { x0 = x1 - w } else { x1 = x0 + w }
        if alca.norte() { y0 = y1 - h } else { y1 = y0 + h }
    }
    let novo = Retangulo::novo(x0, y0, x1 - x0, y1 - y0);
    // O ponto da alça oposta não anda na tela, mesmo girado.
    let (fx, fy) = alca.oposta().fracao();
    let fixo = (base.x + base.largura * fx, base.y + base.altura * fy);
    let antes = girar_em_volta(fixo, base.centro(), giro);
    let depois = girar_em_volta(fixo, novo.centro(), giro);
    let r = Retangulo { x: novo.x + antes.0 - depois.0, y: novo.y + antes.1 - depois.1, ..novo };
    Retangulo { x: r.x.max(0.0), y: r.y.max(0.0), ..r }
}

/// Giro pelo ângulo entre o início e o fim do arraste em volta do centro.
pub(super) fn girar(giro: f32, centro: Point, inicio: Point, atual: Point, passo_15: bool) -> f32 {
    let a0 = (inicio.y - centro.y).atan2(inicio.x - centro.x);
    let a1 = (atual.y - centro.y).atan2(atual.x - centro.x);
    let mut g = giro + (a1 - a0).to_degrees();
    if passo_15 {
        g = (g / 15.0).round() * 15.0;
    }
    g.rem_euclid(360.0)
}

fn para_path(c: &Caminho) -> Path {
    let pt = |p: Ponto| Point::new(p.x, p.y);
    Path::new(|b| {
        for cmd in &c.comandos {
            match *cmd {
                Comando::Mover(p) => b.move_to(pt(p)),
                Comando::Linha(p) => b.line_to(pt(p)),
                Comando::Quadratica { controle, fim } => b.quadratic_curve_to(pt(controle), pt(fim)),
                Comando::Cubica { controle1, controle2, fim } => b.bezier_curve_to(pt(controle1), pt(controle2), pt(fim)),
                Comando::Fechar => b.close(),
            }
        }
    })
}

/// Os caminhos da forma centrados na origem, com espelho (o giro fica no Frame).
pub(super) fn caminhos_centrados(forma: &str, ajustes: &[f32], largura: f32, altura: f32, espelhar: (bool, bool)) -> (Path, Path) {
    let Desenho { preenchida, tracos } = geometria::desenho(forma, ajustes, RetDraw::novo(-largura / 2.0, -altura / 2.0, largura, altura));
    let espelho = |c: Caminho| {
        if espelhar.0 || espelhar.1 {
            c.transformar(TransfDraw::escalar(if espelhar.0 { -1.0 } else { 1.0 }, if espelhar.1 { -1.0 } else { 1.0 }))
        } else {
            c
        }
    };
    (para_path(&espelho(preenchida)), para_path(&espelho(tracos)))
}

/// Ícone Tabler centrado na origem, no menor lado da caixa; o traço é o do
/// Tabler (2 em 24) vezes `espessura`. Também desenha a miniatura da galeria.
pub(super) fn desenhar_icone(f: &mut Frame, nome: &str, largura: f32, altura: f32, espelhar: (bool, bool), espessura: f32, cor: Color) {
    let Some(ic) = feather_shapes::icones::buscar_por_id(nome) else { return };
    let (linhas, escala) = feather_shapes::icones::tracos(ic, -largura / 2.0, -altura / 2.0, largura, altura);
    let (sx, sy) = (if espelhar.0 { -1.0 } else { 1.0 }, if espelhar.1 { -1.0 } else { 1.0 });
    let caminho = Path::new(|b| {
        for l in linhas.iter().filter(|l| l.len() > 1) {
            for (i, p) in l.iter().enumerate() {
                let pt = Point::new(p[0] * sx, p[1] * sy);
                if i == 0 { b.move_to(pt) } else { b.line_to(pt) }
            }
        }
    });
    let largura_traco = (2.0 * escala * if espessura > 0.0 { espessura } else { 1.0 }).max(0.5);
    f.stroke(&caminho, Stroke::default().with_color(cor).with_width(largura_traco)
        .with_line_join(cv::LineJoin::Round).with_line_cap(cv::LineCap::Round));
}

/// Cor do texto da forma: branco sobre preenchimento escuro, cinza-escuro
/// sem preenchimento ou sobre fundo claro (a caixa de texto é branca).
pub(super) fn cor_texto(it: &ItemTela) -> Color {
    match it.preenchimento {
        Some(c) if 0.299 * c.r + 0.587 * c.g + 0.114 * c.b < 0.7 => Color::WHITE,
        _ => Color::rgb8(32, 33, 36),
    }
}

/// Tamanho da letra do texto da forma na tela.
pub(super) fn tamanho_texto(z: f32) -> f32 {
    (11.0 * z).max(6.0)
}

/// Margem interna do texto da forma, em px lógicos (como o "inset" do Office).
const MARGEM_TEXTO: f32 = 7.0;

/// Âncora e alinhamento do texto dentro da forma `r` (tela), pelo
/// alinhamento escolhido. A edição usa o mesmo ponto para não pular.
pub(super) fn posicao_texto(r: Rectangle, a: modelo::AlinharTexto, z: f32) -> (Point, alignment::Horizontal, alignment::Vertical) {
    let m = MARGEM_TEXTO * z;
    let (x, h) = match a.h {
        modelo::AlinharH::Esquerda => (r.x + m, pelicano::Align::Start),
        modelo::AlinharH::Centro => (r.center_x(), pelicano::Align::Center),
        modelo::AlinharH::Direita => (r.x + r.width - m, pelicano::Align::End),
    };
    let (y, v) = match a.v {
        modelo::AlinharV::Topo => (r.y + m, pelicano::Align::Start),
        modelo::AlinharV::Meio => (r.center_y(), pelicano::Align::Center),
        modelo::AlinharV::Baixo => (r.y + r.height - m, pelicano::Align::End),
    };
    (Point::new(x, y), h, v)
}

pub(super) fn cor_hex(hex: &str, transparencia: f32) -> Color {
    let [r, g, b, a] = feather_ui::seletor_cor::rgba_de(hex);
    Color::rgb8(r, g, b).alpha(f32::from(a) / 255.0 * (1.0 - transparencia).clamp(0.0, 1.0))
}

/// O degradê do Office na caixa da forma. `angulo` no sentido do Office
/// (0 = esquerda→direita, 90 = cima→baixo); o linear do pelicano já cobre a
/// caixa inteira (meia projeção nos dois lados). O radial sai do centro para o
/// canto (só há linear: aproxima pela diagonal, com as paradas na metade de lá).
fn gradiente_canvas(g: &modelo::Gradiente, w: f32, h: f32) -> Background {
    let (angle, posicao): (f32, fn(f32) -> f32) = if g.radial {
        (h.atan2(w), |p| 0.5 + p / 2.0)
    } else {
        (g.angulo.to_radians(), |p| p)
    };
    // O canvas do Iced aceitava até 8 paradas; fica o mesmo teto.
    let stops = g.paradas.iter().take(8).map(|(pos, cor)| (posicao(pos.clamp(0.0, 1.0)), cor_hex(cor, 0.0))).collect();
    Background::Linear { angle, stops }
}

/// Sombra externa com desfoque aproximado: a forma deslocada e três
/// contornos cada vez mais largos e transparentes.
fn desenhar_sombra(f: &mut Frame, caminho: &Path, s: &modelo::Sombra, z: f32, so_traco: bool) {
    let cor = cor_hex(&s.cor, 0.0);
    f.with_save(|f| {
        f.translate(Vector::new(s.dx * z, s.dy * z));
        let blur = (s.desfoque * z).max(0.0);
        if !so_traco {
            f.fill(caminho, Color { a: cor.a * 0.7, ..cor });
        }
        if blur > 0.5 {
            for k in 1..=3 {
                let largura = blur * k as f32 / 1.5;
                f.stroke(caminho, Stroke::default().with_color(Color { a: cor.a / 4.0, ..cor }).with_width(largura)
                    .with_line_join(cv::LineJoin::Round));
            }
        }
    });
}

/// Largura média de um caractere, em frações do tamanho da letra.
const LARGURA_CARACTERE: f32 = 0.52;

/// Quebra o texto em linhas que caibam em `largura` (px de tela).
pub(super) fn quebrar_texto(t: &str, largura: f32, tamanho: f32, quebrar: bool) -> Vec<String> {
    let cabe = ((largura / (tamanho * LARGURA_CARACTERE)).floor() as usize).max(1);
    let mut linhas = Vec::new();
    for par in t.split('\n') {
        if !quebrar || par.chars().count() <= cabe {
            linhas.push(par.to_string());
            continue;
        }
        let mut atual = String::new();
        for palavra in par.split(' ') {
            let n = atual.chars().count();
            if n > 0 && n + 1 + palavra.chars().count() > cabe {
                linhas.push(std::mem::take(&mut atual));
            }
            if !atual.is_empty() { atual.push(' '); }
            atual.push_str(palavra);
        }
        linhas.push(atual);
    }
    linhas
}

/// Texto com a letra do Office (tamanho em pt, negrito, cor), quebrado na
/// largura da forma e alinhado como o `bodyPr`/`pPr` pedem.
fn desenhar_texto_office(f: &mut Frame, t: &str, fonte: &modelo::FonteTexto, r: Rectangle, it: &ItemTela, z: f32) {
    // pt -> px (96 dpi) e zoom.
    let tamanho = (fonte.tamanho.unwrap_or(11.0) * 4.0 / 3.0 * z).max(4.0);
    let (mx, my) = (9.6 * z, 4.8 * z);
    let util = (r.width - 2.0 * mx).max(tamanho);
    let linhas = quebrar_texto(t, util, tamanho, !fonte.sem_quebra);
    let alto = tamanho * 1.2;
    let bloco = alto * linhas.len() as f32;
    let topo = match it.alinhar.v {
        modelo::AlinharV::Topo => r.y + my,
        modelo::AlinharV::Meio => r.center_y() - bloco / 2.0,
        modelo::AlinharV::Baixo => r.y + r.height - my - bloco,
    };
    let (x, h) = match it.alinhar.h {
        modelo::AlinharH::Esquerda => (r.x + mx, pelicano::Align::Start),
        modelo::AlinharH::Centro => (r.center_x(), pelicano::Align::Center),
        modelo::AlinharH::Direita => (r.x + r.width - mx, pelicano::Align::End),
    };
    let cor = fonte.cor.as_deref().map(|c| cor_hex(c, 0.0)).unwrap_or_else(|| cor_texto(it));
    let mut letra = pelicano::Font::DEFAULT;
    if fonte.negrito { letra.weight = 700; }
    if fonte.italico { letra.italic = true; }
    for (i, l) in linhas.into_iter().enumerate() {
        f.fill_text(cv::Text {
            content: l,
            position: Point::new(x, topo + alto * i as f32 + alto / 2.0),
            color: cor,
            size: crate::compat::Pixels(tamanho),
            font: letra,
            horizontal_alignment: h,
            vertical_alignment: pelicano::Align::Center,
            ..Default::default()
        });
    }
}

// ------------------------------------------------------------ camada

/// Um objeto pronto para a tela (px lógicos absolutos).
#[derive(Clone)]
pub(super) struct ItemTela {
    pub id: u64,
    pub forma: String,
    pub ajustes: Vec<f32>,
    pub ret: Retangulo,
    pub giro: f32,
    pub espelhar: (bool, bool),
    pub preenchimento: Option<Color>,
    pub contorno: Option<Color>,
    pub espessura: f32,
    pub texto: Option<String>,
    pub alinhar: modelo::AlinharTexto,
    pub linha: bool,
    pub tem_macro: bool,
    /// Ícone Tabler (`forma` = nome do ícone): só traço, sem alças de ajuste.
    pub icone: bool,
    /// WordArt: o texto vira contorno deformado (`forma` = deformação), sem alças de ajuste.
    pub wordart: Option<super::wordart::WordArtTela>,
    /// Degradê do preenchimento (vem do `.xlsx`).
    pub gradiente: Option<modelo::Gradiente>,
    pub sombra: Option<modelo::Sombra>,
    pub fonte: Option<modelo::FonteTexto>,
    /// Figura decodificada (chave do cache, imagem).
    pub imagem: Option<(u64, pelicano::Image)>,
    /// Ordem no desenho do `.xlsx` (`extras.z`), comparada à dos gráficos.
    pub z: Option<u64>,
}

impl ItemTela {
    pub(super) fn de(o: &ObjetoSolto, regua: &impl Regua) -> Self {
        let a = &o.aparencia;
        let icone = o.tipo == modelo::TipoObjeto::Icone;
        let wordart = (o.tipo == modelo::TipoObjeto::Wordart).then(|| super::wordart::WordArtTela::de(o));
        let linha = o.tipo == modelo::TipoObjeto::Forma && (geometria::so_traco(&o.forma) || modelo::e_linha(&o.forma));
        Self {
            id: o.id,
            forma: o.forma.clone(),
            ajustes: o.ajustes.clone(),
            ret: o.retangulo(regua),
            giro: o.giro,
            espelhar: (o.espelhar_h, o.espelhar_v),
            preenchimento: a.preenchimento.as_deref().filter(|_| !linha && !icone).map(|c| cor_hex(c, a.transparencia)),
            contorno: a.contorno.as_deref().map(|c| cor_hex(c, a.transparencia)),
            espessura: a.espessura,
            texto: o.texto.clone().filter(|t| !t.is_empty()),
            alinhar: o.alinhar,
            linha,
            tem_macro: o.macro_nome.is_some(),
            icone,
            wordart,
            gradiente: o.gradiente.clone().filter(|g| !g.paradas.is_empty()),
            sombra: o.sombra.clone(),
            fonte: o.fonte.clone(),
            imagem: o.imagem.as_ref().and_then(super::figura::handle),
            z: o.extras.get("z").and_then(serde_json::Value::as_u64),
        }
    }
}

/// Gráfico do `.xlsx` desenhado na camada, na ordem do desenho do Excel.
#[derive(Debug, Clone)]
pub(super) struct GraficoTela {
    pub z: u64,
    pub ret: Retangulo,
    pub svg: std::rc::Rc<String>,
    /// Tamanho do SVG (`width`/`height` da raiz), encaixado em `ret` sem
    /// deformar (como o `ContentFit::Contain` do widget).
    pub tamanho_svg: (f32, f32),
    chave: u64,
}

impl GraficoTela {
    pub(super) fn novo(z: u64, ret: Retangulo, svg: String) -> Self {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        svg.hash(&mut h);
        let raiz = svg.find("<svg").and_then(|i| svg[i..].find('>').map(|f| &svg[i..i + f])).unwrap_or("");
        let medida = |n: &str| {
            let chave = format!(" {n}=\"");
            raiz.find(&chave).and_then(|i| raiz[i + chave.len()..].split('"').next())
                .and_then(|v| v.trim_end_matches("px").parse::<f32>().ok()).filter(|v| *v > 0.0)
        };
        let tamanho_svg = (medida("width").unwrap_or(ret.largura.max(1.0)), medida("height").unwrap_or(ret.altura.max(1.0)));
        Self { z, ret, svg: std::rc::Rc::new(svg), tamanho_svg, chave: h.finish() }
    }
}

/// O gráfico rasterizado uma vez por tamanho (px inteiros evitam borrão).
/// Antes cada quadro da rolagem montava um SVG novo (recorte no `viewBox`) e
/// o Iced analisava e rasterizava o gráfico inteiro de novo: com 2.400 pontos
/// por série, a rolagem travava. O pelicano recorta o `draw_image` pela
/// camada, então a imagem inteira vai sempre (sem recortar os pixels à mão).
struct Raster {
    /// A imagem inteira, com id fixo: o pelicano não a escala de novo.
    inteira: pelicano::Image,
}

thread_local! {
    static RASTERES: RefCell<(usize, std::collections::HashMap<(u64, u32, u32), Option<std::rc::Rc<Raster>>>)> = RefCell::default();
}

/// Teto das imagens guardadas; passou, recomeça (as visíveis voltam no próximo quadro).
const TETO_RASTERES: usize = 128 * 1024 * 1024;

/// As fontes do sistema: o `Svg` do pelicano analisa sem elas e o texto do gráfico sumiria.
fn fontes_svg() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static FONTES: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    FONTES.get_or_init(|| {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        std::sync::Arc::new(db)
    }).clone()
}

/// Os pixels (RGBA, alfa à parte) do SVG em `w` x `h`.
fn rgba_do_svg(svg: &str, w: u32, h: u32) -> Option<Vec<u8>> {
    let opcoes = resvg::usvg::Options { fontdb: fontes_svg(), ..Default::default() };
    let arvore = resvg::usvg::Tree::from_str(svg, &opcoes).ok()?;
    let mut img = resvg::tiny_skia::Pixmap::new(w, h)?;
    let t = arvore.size();
    resvg::render(&arvore, resvg::tiny_skia::Transform::from_scale(w as f32 / t.width(), h as f32 / t.height()), &mut img.as_mut());
    // O tiny-skia guarda pré-multiplicado; a `Image` quer o alfa à parte.
    Some(img.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect())
}

/// Pixels feitos fora da thread da janela ([`adiantar_rasteres`]), à espera
/// do primeiro quadro que os mostre. Rasterizar na troca de aba custava até
/// 75 ms por aba no arquivo do usuário (28/10/2026).
static ADIANTADOS: std::sync::LazyLock<std::sync::Mutex<(usize, std::collections::HashMap<(u64, u32, u32), Vec<u8>>)>> =
    std::sync::LazyLock::new(Default::default);

/// O tamanho em px do gráfico na tela: o quadro no zoom, encaixado sem deformar.
fn tamanho_raster(ret: Retangulo, tamanho_svg: (f32, f32), zoom: f32) -> (u32, u32) {
    let (sw, sh) = tamanho_svg;
    let escala = (ret.largura * zoom / sw).min(ret.altura * zoom / sh);
    ((sw * escala).round().max(1.0) as u32, (sh * escala).round().max(1.0) as u32)
}

/// Rasteriza numa thread à parte os gráficos (de qualquer aba) que ainda não
/// têm imagem, no tamanho que terão com este zoom: trocar de aba já os acha prontos.
pub(in crate::app) fn adiantar_rasteres(graficos: Vec<(Retangulo, String)>, zoom: f32) {
    if graficos.is_empty() { return; }
    std::thread::spawn(move || {
        for (ret, svg) in graficos {
            let g = GraficoTela::novo(0, ret, svg);
            let (w, h) = tamanho_raster(ret, g.tamanho_svg, zoom);
            let chave = (g.chave, w, h);
            let ja = RASTERES_FEITOS.lock().is_ok_and(|f| f.contains(&chave))
                || ADIANTADOS.lock().is_ok_and(|a| a.1.contains_key(&chave) || a.0 > TETO_RASTERES / 2);
            if ja { continue; }
            if let Some(rgba) = rgba_do_svg(&g.svg, w, h) {
                if let Ok(mut a) = ADIANTADOS.lock() {
                    a.0 += rgba.len();
                    a.1.insert(chave, rgba);
                }
            }
        }
    });
}

/// As chaves já rasterizadas pela janela (a thread não refaz o que está na tela).
static RASTERES_FEITOS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<(u64, u32, u32)>>> =
    std::sync::LazyLock::new(Default::default);

// shortcut: rasteriza em px lógicos; numa tela HiDPI (escala > 1) sai borrado. Ler o fator de escala da janela se aparecer.
fn raster(g: &GraficoTela, w: u32, h: u32) -> Option<std::rc::Rc<Raster>> {
    RASTERES.with(|r| {
        let (bytes, mapa) = &mut *r.borrow_mut();
        if let Some(pronto) = mapa.get(&(g.chave, w, h)) {
            return pronto.clone();
        }
        if *bytes > TETO_RASTERES {
            mapa.clear();
            *bytes = 0;
            if let Ok(mut f) = RASTERES_FEITOS.lock() { f.clear(); }
        }
        let adiantado = ADIANTADOS.lock().ok().and_then(|mut a| {
            let rgba = a.1.remove(&(g.chave, w, h))?;
            a.0 -= rgba.len();
            Some(rgba)
        });
        let pronto = adiantado.or_else(|| rgba_do_svg(&g.svg, w, h))
            .and_then(|rgba| pelicano::Image::from_rgba(w, h, rgba))
            .map(|inteira| std::rc::Rc::new(Raster { inteira }));
        if let Ok(mut f) = RASTERES_FEITOS.lock() { f.insert((g.chave, w, h)); }
        *bytes += (w * h * 4) as usize;
        mapa.insert((g.chave, w, h), pronto.clone());
        pronto
    })
}

pub(super) struct Camada {
    pub itens: Vec<ItemTela>,
    /// Gráficos do Excel intercalados com os objetos pelo `z`.
    pub graficos: Vec<GraficoTela>,
    pub selecionado: Option<u64>,
    pub inserir: Option<&'static str>,
    pub vista: Vista,
    /// "Travar proporção" ligado na aba Editar forma.
    pub travar: bool,
    /// Id do objeto com o texto em edição (o texto vai no campo, não aqui).
    pub editando: Option<u64>,
}

#[derive(Debug, Clone)]
struct Gesto {
    tipo: TipoGesto,
    id: Option<u64>,
    inicio: Point,
    atual: Point,
    base: Retangulo,
    giro: f32,
    /// Ctrl no início (ajuste por canto).
    ctrl: bool,
    comeco: Instant,
}

#[derive(Default)]
pub(in crate::app) struct EstadoCanvas {
    gesto: Option<Gesto>,
    shift: bool,
    ctrl: bool,
    /// Último clique: (forma, instante), para o duplo clique.
    ultimo_clique: Option<(u64, Instant)>,
    medida: RefCell<Medida>,
}

impl Camada {
    fn z(&self) -> f32 {
        self.vista.zoom
    }

    /// Retângulo do objeto na tela (relativo ao Canvas), sem giro.
    pub(super) fn ret_tela(&self, r: Retangulo) -> Rectangle {
        let (dx, dy) = self.vista.deslocamento(Ponto::novo(r.x, r.y));
        let z = self.z();
        Rectangle { x: (r.x - dx + CAB_X) * z, y: (r.y - dy + CAB_Y) * z, width: r.largura * z, height: r.altura * z }
    }

    fn corpo(&self, tamanho: pelicano::Size) -> Rectangle {
        let (x, y) = (CAB_X * self.z(), CAB_Y * self.z());
        Rectangle { x, y, width: (tamanho.width - BARRA - x).max(0.0), height: (tamanho.height - BARRA - y).max(0.0) }
    }

    /// Onde o objeto pode aparecer: o painel congelado não mostra o que
    /// rolou para baixo dele.
    fn recorte(&self, r: Retangulo, tamanho: pelicano::Size) -> Rectangle {
        let corpo = self.corpo(tamanho);
        let z = self.z();
        let x0 = if r.x < self.vista.fixa_x { corpo.x } else { (CAB_X + self.vista.fixa_x) * z };
        let y0 = if r.y < self.vista.fixa_y { corpo.y } else { (CAB_Y + self.vista.fixa_y) * z };
        let (x1, y1) = (corpo.x + corpo.width, corpo.y + corpo.height);
        Rectangle { x: x0, y: y0, width: (x1 - x0).max(0.0), height: (y1 - y0).max(0.0) }
    }

    /// Ponto da tela → px lógicos absolutos (como a tinta).
    pub(super) fn absoluto(&self, p: Point) -> Ponto {
        let z = self.z().max(0.01);
        self.vista.absoluto(Ponto::novo(p.x / z - CAB_X, p.y / z - CAB_Y))
    }

    /// Ponto da tela no sistema sem giro do objeto.
    fn local(p: Point, r: Rectangle, giro: f32) -> Point {
        let c = r.center();
        let (x, y) = girar_em_volta((p.x, p.y), (c.x, c.y), -giro);
        Point::new(x, y)
    }

    fn acerta(&self, it: &ItemTela, p: Point) -> bool {
        let r = self.ret_tela(it.ret);
        let q = Self::local(p, r, it.giro);
        q.x >= r.x - FOLGA && q.x <= r.x + r.width + FOLGA && q.y >= r.y - FOLGA && q.y <= r.y + r.height + FOLGA
    }

    /// Alças visíveis: a linha só tem as duas pontas e não gira.
    fn alcas_de(it: &ItemTela) -> Vec<Alca> {
        if !it.linha {
            return Alca::TODAS.to_vec();
        }
        if it.espelhar.0 != it.espelhar.1 { vec![Alca::Ne, Alca::Sw] } else { vec![Alca::Nw, Alca::Se] }
    }

    fn ponto_alca(r: Rectangle, alca: Alca) -> Point {
        let (fx, fy) = alca.fracao();
        Point::new(r.x + r.width * fx, r.y + r.height * fy)
    }

    fn ponto_giro(r: Rectangle) -> Point {
        Point::new(r.center_x(), r.y - DIST_GIRO)
    }

    /// Alças de ajuste do objeto (sistema sem giro, tela).
    fn alcas_ajuste(it: &ItemTela, r: Rectangle) -> Vec<(usize, Point)> {
        if it.espelhar != (false, false) || it.icone || it.wordart.is_some() {
            return Vec::new();
        }
        geometria::alcas(&it.forma, &it.ajustes, (r.x, r.y, r.width, r.height))
            .into_iter()
            .map(|(i, (x, y))| (i, Point::new(x, y)))
            .collect()
    }

    /// O que está sob o ponto: alças do selecionado (ajuste, giro, tamanho),
    /// depois o objeto mais de cima. Com macro e sem Ctrl, o clique executa.
    pub(super) fn alvo(&self, p: Point, ctrl: bool) -> Option<(u64, TipoGesto)> {
        if let Some(it) = self.selecionado.and_then(|id| self.itens.iter().find(|i| i.id == id)) {
            let r = self.ret_tela(it.ret);
            let q = Self::local(p, r, it.giro);
            let raio = ALCA / 2.0 + 2.0;
            if let Some((i, _)) = Self::alcas_ajuste(it, r).into_iter().find(|(_, a)| q.distance(*a) <= raio) {
                return Some((it.id, TipoGesto::Ajuste(i)));
            }
            if !it.linha && q.distance(Self::ponto_giro(r)) <= raio + 1.0 {
                return Some((it.id, TipoGesto::Girar));
            }
            if let Some(a) = Self::alcas_de(it).into_iter().find(|&a| q.distance(Self::ponto_alca(r, a)) <= raio) {
                return Some((it.id, TipoGesto::Alca(a)));
            }
        }
        let it = self.itens.iter().rev().find(|it| self.acerta(it, p))?;
        let macro_ativa = it.tem_macro && !ctrl && self.selecionado != Some(it.id);
        Some((it.id, if macro_ativa { TipoGesto::Macro } else { TipoGesto::Mover }))
    }

    /// Retângulo, giro e ajustes do objeto com o gesto aplicado.
    fn aplicar(&self, g: &Gesto, it: &ItemTela, shift: bool) -> (Retangulo, f32, Option<(&'static str, Vec<f32>)>) {
        let z = self.z().max(0.01);
        let d = ((g.atual.x - g.inicio.x) / z, (g.atual.y - g.inicio.y) / z);
        match g.tipo {
            TipoGesto::Mover => (mover(g.base, d), g.giro, None),
            TipoGesto::Alca(a) => {
                let minimo = if it.linha { 0.0 } else { modelo::LADO_MINIMO };
                (redimensionar(g.base, g.giro, a, d, shift || self.travar, minimo), g.giro, None)
            }
            TipoGesto::Girar => (g.base, girar(g.giro, self.ret_tela(g.base).center(), g.inicio, g.atual, shift), None),
            TipoGesto::Ajuste(i) => {
                let r = self.ret_tela(g.base);
                let q = Self::local(g.atual, r, g.giro);
                let novo = geometria::ajustar(&it.forma, &it.ajustes, i, (q.x, q.y), (r.x, r.y, r.width, r.height), g.ctrl);
                (g.base, g.giro, novo)
            }
            TipoGesto::Criar(_) | TipoGesto::Macro => (g.base, g.giro, None),
        }
    }

    /// O retângulo da forma em criação (e se a linha sai espelhada).
    fn ret_criacao(&self, g: &Gesto, shift: bool) -> (Retangulo, bool) {
        let a = self.absoluto(g.inicio);
        if g.inicio.distance(g.atual) < ARRASTE_MINIMO {
            return (Retangulo::novo(a.x, a.y, TAMANHO_PADRAO.0, TAMANHO_PADRAO.1), false);
        }
        let b = self.absoluto(g.atual);
        let (mut bx, mut by) = (b.x, b.y);
        if shift {
            let lado = (bx - a.x).abs().max((by - a.y).abs());
            bx = a.x + lado * (bx - a.x).signum();
            by = a.y + lado * (by - a.y).signum();
        }
        let espelhar = (bx - a.x) * (by - a.y) < 0.0;
        (Retangulo::entre(a.x, a.y, bx.max(0.0), by.max(0.0)), espelhar)
    }

    /// `visivel`: a parte da tela onde o objeto pode aparecer (o desenho é
    /// recortado nela pelo `with_clip`).
    fn desenhar_forma(frame: &mut Frame, it: &ItemTela, ajustes: &[f32], forma: &str, r: Rectangle, giro: f32, z: f32, com_texto: bool, visivel: Rectangle) {
        let c = r.center();
        if let Some(wa) = &it.wordart {
            // O texto é o próprio desenho (não sai centrado por cima, como o da forma).
            let texto = it.texto.as_deref().unwrap_or(tr!(super::wordart::TEXTO_PADRAO));
            frame.with_save(|f| {
                f.translate(Vector::new(c.x, c.y));
                f.rotate(giro.to_radians());
                super::wordart::desenhar(f, texto, wa, r.width, r.height, it.espelhar, it.preenchimento, it.contorno, it.espessura, z);
            });
            return;
        }
        frame.with_save(|f| {
            f.translate(Vector::new(c.x, c.y));
            f.rotate(giro.to_radians());
            if it.icone {
                if let Some(cor) = it.contorno {
                    desenhar_icone(f, forma, r.width, r.height, it.espelhar, it.espessura, cor);
                }
                return;
            }
            let (cheia, tracos) = caminhos_centrados(forma, ajustes, r.width, r.height, it.espelhar);
            let traco = |cor: Color| {
                Stroke::default().with_color(cor).with_width((it.espessura * z).max(0.5))
                    .with_line_join(cv::LineJoin::Round).with_line_cap(cv::LineCap::Round)
            };
            if let Some(s) = &it.sombra {
                desenhar_sombra(f, if it.linha { &tracos } else { &cheia }, s, z, it.linha);
            }
            if let Some((_, h)) = &it.imagem {
                // O pelicano recorta a figura pelo `with_clip` de quem chama
                // (no Iced ela era recortada à mão e, girada e cortada, sumia).
                let local = Rectangle { x: -r.width / 2.0, y: -r.height / 2.0, width: r.width, height: r.height };
                if r.intersects(&visivel) {
                    f.draw_image(local, h);
                }
            } else if let Some(g) = &it.gradiente {
                f.fill(&cheia, gradiente_canvas(g, r.width, r.height));
            } else if let Some(cor) = it.preenchimento {
                f.fill(&cheia, cor);
            }
            if let Some(cor) = it.contorno {
                f.stroke(&cheia, traco(cor));
                f.stroke(&tracos, traco(cor));
            }
        });
        if let Some(t) = it.texto.as_ref().filter(|_| com_texto) {
            if let Some(fonte) = &it.fonte {
                frame.with_save(|f| {
                    f.translate(Vector::new(c.x, c.y));
                    f.rotate(giro.to_radians());
                    let local = Rectangle { x: -r.width / 2.0, y: -r.height / 2.0, width: r.width, height: r.height };
                    desenhar_texto_office(f, t, fonte, local, it, z);
                });
                return;
            }
            let (position, horizontal_alignment, vertical_alignment) = posicao_texto(r, it.alinhar, z);
            frame.fill_text(cv::Text {
                content: t.clone(),
                position,
                color: cor_texto(it),
                size: crate::compat::Pixels(tamanho_texto(z)),
                horizontal_alignment,
                vertical_alignment,
                ..Default::default()
            });
        }
    }

    fn desenhar_selecao(frame: &mut Frame, it: &ItemTela, ajustes: &[f32], forma: &str, r: Rectangle, giro: f32) {
        let acento = COR_AZUL_ACENTO();
        let c = r.center();
        frame.with_save(|f| {
            f.translate(Vector::new(c.x, c.y));
            f.rotate(giro.to_radians());
            let local = Rectangle { x: -r.width / 2.0, y: -r.height / 2.0, width: r.width, height: r.height };
            let fino = Stroke::default().with_color(acento).with_width(1.0);
            if !it.linha {
                f.stroke(&Path::rectangle(local.position(), local.size()), fino.clone());
                let giro_p = Self::ponto_giro(local);
                f.stroke(&Path::line(Point::new(0.0, local.y), giro_p), fino.clone());
                let circulo = Path::circle(giro_p, ALCA / 2.0 + 1.0);
                f.fill(&circulo, COR_PAINEL());
                f.stroke(&circulo, fino.clone().with_width(1.5));
            }
            for a in Self::alcas_de(it) {
                let p = Self::ponto_alca(local, a);
                let quadrado = Path::new(|b| {
                    b.rounded_rectangle(Point::new(p.x - ALCA / 2.0, p.y - ALCA / 2.0), pelicano::Size::new(ALCA, ALCA), 2.0);
                });
                f.fill(&quadrado, COR_PAINEL());
                f.stroke(&quadrado, fino.clone().with_width(1.5));
            }
            // Alças de ajuste: losango cheio na cor de destaque (as de tamanho são brancas).
            let ajustado = ItemTela { ajustes: ajustes.to_vec(), forma: forma.to_owned(), ..it.clone() };
            for (_, p) in Self::alcas_ajuste(&ajustado, local) {
                let l = ALCA / 2.0 + 1.0;
                let losango = Path::new(|b| {
                    b.move_to(Point::new(p.x, p.y - l));
                    b.line_to(Point::new(p.x + l, p.y));
                    b.line_to(Point::new(p.x, p.y + l));
                    b.line_to(Point::new(p.x - l, p.y));
                    b.close();
                });
                f.fill(&losango, acento);
                f.stroke(&losango, Stroke::default().with_color(COR_PAINEL()).with_width(1.0));
            }
        });
    }

    fn visivel(r: Rectangle, corpo: Rectangle) -> bool {
        let raio = r.width.hypot(r.height) / 2.0 + 8.0;
        let c = r.center();
        !(c.x + raio < corpo.x || c.y + raio < corpo.y || c.x - raio > corpo.x + corpo.width || c.y - raio > corpo.y + corpo.height)
    }

    fn desenhar_grafico(&self, frame: &mut Frame, g: &GraficoTela, tamanho: pelicano::Size) {
        let quadro = self.ret_tela(g.ret);
        if !Self::visivel(quadro, self.corpo(tamanho)) || quadro.width < 1.0 || quadro.height < 1.0 {
            return;
        }
        // Encaixe sem deformar, centrado no quadro (o mesmo tamanho que a thread adianta).
        let (w, h) = tamanho_raster(g.ret, g.tamanho_svg, self.z());
        let r = Rectangle {
            x: quadro.x + (quadro.width - w as f32) / 2.0,
            y: quadro.y + (quadro.height - h as f32) / 2.0,
            width: w as f32,
            height: h as f32,
        };
        let recorte = self.recorte(g.ret, tamanho);
        if !r.intersects(&recorte) { return; }
        let (x, y) = (r.x.round(), r.y.round());
        let Some(img) = raster(g, w, h) else { return };
        let destino = Rectangle { x, y, width: w as f32, height: h as f32 };
        frame.with_clip(recorte, |f| {
            f.translate(Vector::new(-recorte.x, -recorte.y));
            f.draw_image(destino, &img.inteira);
        });
    }

    fn desenhar_recortado(&self, frame: &mut Frame, it: &ItemTela, ret: Retangulo, giro: f32, ajustes: &[f32], forma: &str, tamanho: pelicano::Size) {
        let r = self.ret_tela(ret);
        if !Self::visivel(r, self.corpo(tamanho)) {
            return;
        }
        let recorte = self.recorte(ret, tamanho);
        let com_texto = self.editando != Some(it.id);
        frame.with_clip(recorte, |f| {
            f.translate(Vector::new(-recorte.x, -recorte.y));
            Self::desenhar_forma(f, it, ajustes, forma, r, giro, self.z(), com_texto, recorte);
        });
    }
}

impl Program<Msg> for Camada {
    type State = EstadoCanvas;

    fn update(&self, estado: &mut EstadoCanvas, evento: &Event, bounds: Rectangle, cursor: Option<Point>, publish: &mut dyn FnMut(Msg)) -> Captured {
        let (capturou, msg) = self.atualizar(estado, evento, bounds, cursor);
        msg.into_iter().for_each(publish);
        capturou
    }

    fn draw(&self, estado: &EstadoCanvas, renderer: &mut Renderer, bounds: Rectangle, _tema: &Theme, _cursor: Option<Point>) {
        let mut frame = Frame::new(renderer, bounds);
        self.desenhar(estado, &mut frame, bounds);
        frame.fim();
    }

    fn mouse_interaction(&self, estado: &EstadoCanvas, bounds: Rectangle, cursor: Option<Point>) -> mouse::Interaction {
        if let Some(g) = &estado.gesto {
            return match g.tipo {
                TipoGesto::Mover | TipoGesto::Girar | TipoGesto::Ajuste(_) => mouse::Interaction::Grabbing,
                TipoGesto::Alca(a) => a.cursor(),
                TipoGesto::Criar(_) => mouse::Interaction::Crosshair,
                TipoGesto::Macro => mouse::Interaction::Pointer,
            };
        }
        let Some(p) = em(cursor, bounds).filter(|p| self.corpo(bounds.size()).contains(*p)) else {
            return mouse::Interaction::Default;
        };
        if self.inserir.is_some() {
            return mouse::Interaction::Crosshair;
        }
        match self.alvo(p, estado.ctrl) {
            Some((_, TipoGesto::Alca(a))) => a.cursor(),
            Some((_, TipoGesto::Girar | TipoGesto::Ajuste(_))) => mouse::Interaction::Grab,
            Some((_, TipoGesto::Macro)) => mouse::Interaction::Pointer,
            Some(_) => mouse::Interaction::Move,
            None => mouse::Interaction::Default,
        }
    }
}

/// A camada com o ícone da grade de baixo: no pelicano o canvas de cima decide
/// o ícone, e fora dos objetos ele repete o da grade (o `Interaction::None` do Iced).
pub(super) struct CamadaNaGrade<'a> {
    pub camada: Camada,
    pub cursor_grade: &'a std::cell::Cell<mouse::Interaction>,
}

impl Program<Msg> for CamadaNaGrade<'_> {
    type State = EstadoCanvas;

    fn update(&self, estado: &mut EstadoCanvas, evento: &Event, bounds: Rectangle, cursor: Option<Point>, publish: &mut dyn FnMut(Msg)) -> Captured {
        self.camada.update(estado, evento, bounds, cursor, publish)
    }

    fn draw(&self, estado: &EstadoCanvas, renderer: &mut Renderer, bounds: Rectangle, tema: &Theme, cursor: Option<Point>) {
        self.camada.draw(estado, renderer, bounds, tema, cursor)
    }

    fn mouse_interaction(&self, estado: &EstadoCanvas, bounds: Rectangle, cursor: Option<Point>) -> mouse::Interaction {
        match self.camada.mouse_interaction(estado, bounds, cursor) {
            mouse::Interaction::Default => self.cursor_grade.get(),
            i => i,
        }
    }
}

/// O cursor relativo ao canto do Canvas, só se estiver dentro dele.
fn em(cursor: Option<Point>, bounds: Rectangle) -> Option<Point> {
    cursor.filter(|p| bounds.contains(*p)).map(|p| Point::new(p.x - bounds.x, p.y - bounds.y))
}

impl Camada {
    fn atualizar(&self, estado: &mut EstadoCanvas, evento: &Event, bounds: Rectangle, cursor: Option<Point>) -> (Captured, Option<Msg>) {
        let t0 = Instant::now();
        let pos = cursor.map(|p| Point::new(p.x - bounds.x, p.y - bounds.y));
        let dentro = em(cursor, bounds).is_some_and(|p| self.corpo(bounds.size()).contains(p));
        let msg = |e: Evento| Some(Msg::Objetos(e));
        match evento {
            Event::ModifiersChanged(m) => {
                estado.shift = m.shift_key();
                estado.ctrl = m.control_key();
                (Captured::No, None)
            }
            Event::MousePressed(MouseButton::Right) if dentro => {
                let Some(p) = pos else { return (Captured::No, None) };
                match self.itens.iter().rev().find(|it| self.acerta(it, p)) {
                    Some(it) => (Captured::Yes, msg(Evento::Contexto(it.id))),
                    None => (Captured::No, None),
                }
            }
            Event::MousePressed(MouseButton::Left) if dentro => {
                let Some(p) = pos else { return (Captured::No, None) };
                let novo = |tipo, id, base, giro, ctrl| Gesto { tipo, id, inicio: p, atual: p, base, giro, ctrl, comeco: Instant::now() };
                *estado.medida.borrow_mut() = Medida::default();
                if let Some(forma) = self.inserir {
                    estado.gesto = Some(novo(TipoGesto::Criar(forma), None, Retangulo::default(), 0.0, false));
                    ARRASTANDO.store(true, Ordering::Relaxed);
                    return (Captured::Yes, None);
                }
                match self.alvo(p, estado.ctrl) {
                    Some((id, tipo)) => {
                        let it = self.itens.iter().find(|i| i.id == id).expect("alvo existe");
                        // Duplo clique numa forma sem macro: editar o texto.
                        let duplo = tipo == TipoGesto::Mover
                            && estado.ultimo_clique.is_some_and(|(u, t)| u == id && t.elapsed().as_millis() < DUPLO_CLIQUE_MS);
                        estado.ultimo_clique = Some((id, Instant::now()));
                        if duplo {
                            estado.gesto = None;
                            return (Captured::Yes, msg(Evento::EditarTexto(id)));
                        }
                        estado.gesto = Some(novo(tipo, Some(id), it.ret, it.giro, estado.ctrl));
                        ARRASTANDO.store(true, Ordering::Relaxed);
                        let selecionar = tipo != TipoGesto::Macro && self.selecionado != Some(id);
                        (Captured::Yes, if selecionar { msg(Evento::Selecionar(id)) } else { None })
                    }
                    // Clique fora: tira a seleção e deixa a grade receber o clique.
                    None => (Captured::No, self.selecionado.and_then(|_| msg(Evento::Desselecionar))),
                }
            }
            Event::CursorMoved(_) => match (&mut estado.gesto, pos) {
                (Some(g), Some(p)) => {
                    g.atual = p;
                    let mut m = estado.medida.borrow_mut();
                    m.eventos += 1;
                    m.us_eventos += t0.elapsed().as_micros() as u64;
                    (Captured::Yes, None)
                }
                _ => (Captured::No, None),
            },
            Event::MouseReleased(MouseButton::Left) => {
                let Some(mut g) = estado.gesto.take() else { return (Captured::No, None) };
                ARRASTANDO.store(false, Ordering::Relaxed);
                if let Some(p) = pos {
                    g.atual = p;
                }
                let mut medida = *estado.medida.borrow();
                medida.ms_total = g.comeco.elapsed().as_secs_f32() * 1000.0;
                if let TipoGesto::Criar(forma) = g.tipo {
                    let (ret, espelhar) = self.ret_criacao(&g, estado.shift);
                    return (Captured::Yes, msg(Evento::Criar { forma, ret, espelhar }));
                }
                let parado = g.inicio.distance(g.atual) < if g.tipo == TipoGesto::Macro { 4.0 } else { 1.0 };
                let Some(it) = g.id.and_then(|id| self.itens.iter().find(|i| i.id == id)) else {
                    return (Captured::Yes, None);
                };
                if g.tipo == TipoGesto::Macro {
                    return (Captured::Yes, if parado { msg(Evento::ExecutarMacro(it.id)) } else { None });
                }
                if parado {
                    return (Captured::Yes, None);
                }
                let (ret, giro, ajuste) = self.aplicar(&g, it, estado.shift);
                let e = match ajuste {
                    Some((forma, ajustes)) => Evento::Ajustar { id: it.id, forma, ajustes, medida },
                    None => Evento::Transformar { id: it.id, ret, giro, medida },
                };
                (Captured::Yes, msg(e))
            }
            &Event::Wheel { dx, dy, modifiers } if dentro
                && (self.inserir.is_some() || pos.is_some_and(|p| self.alvo(p, estado.ctrl).is_some())) =>
            {
                // Sobre o objeto a grade não recebe o cursor: rola por aqui.
                (Captured::Yes, Some(crate::app::botoes_macro::rolar_pela_roda(dx, dy, modifiers)))
            }
            _ => (Captured::No, None),
        }
    }

    fn desenhar(&self, estado: &EstadoCanvas, frame: &mut Frame, bounds: Rectangle) {
        let t0 = Instant::now();
        let tamanho = bounds.size();
        let corpo = self.corpo(tamanho);
        let gesto = estado.gesto.as_ref();
        let em_gesto = gesto.and_then(|g| g.id).filter(|_| gesto.is_some_and(|g| g.tipo != TipoGesto::Macro));
        // 1) Os parados (o pelicano só repinta o que mudou; sem `Cache`).
        {
            // Gráfico antes do primeiro objeto que vem depois dele no desenho.
            let mut graficos: Vec<&GraficoTela> = self.graficos.iter().collect();
            graficos.sort_by_key(|g| g.z);
            let mut proximo = graficos.into_iter().peekable();
            for it in self.itens.iter().filter(|i| Some(i.id) != em_gesto) {
                while let Some(g) = proximo.next_if(|g| it.z.is_some_and(|z| g.z < z)) {
                    self.desenhar_grafico(frame, g, tamanho);
                }
                self.desenhar_recortado(frame, it, it.ret, it.giro, &it.ajustes, &it.forma, tamanho);
            }
            for g in proximo {
                self.desenhar_grafico(frame, g, tamanho);
            }
        }
        // 2) O que muda por quadro: o objeto em gesto (inteiro), a seleção e a prévia de criação.
        for it in &self.itens {
            let (ret, giro, ajuste) = match gesto {
                Some(g) if g.id == Some(it.id) && g.tipo != TipoGesto::Macro => self.aplicar(g, it, estado.shift),
                _ => (it.ret, it.giro, None),
            };
            let (forma, ajustes) = ajuste.as_ref().map_or((it.forma.as_str(), it.ajustes.as_slice()), |(f, a)| (*f, a.as_slice()));
            if Some(it.id) == em_gesto {
                self.desenhar_recortado(frame, it, ret, giro, ajustes, forma, tamanho);
            }
            if self.selecionado == Some(it.id) {
                frame.with_clip(corpo, |f| {
                    f.translate(Vector::new(-corpo.x, -corpo.y));
                    Self::desenhar_selecao(f, it, ajustes, forma, self.ret_tela(ret), giro);
                });
            }
        }
        if let Some(g @ Gesto { tipo: TipoGesto::Criar(forma), .. }) = gesto {
            if g.inicio.distance(g.atual) >= ARRASTE_MINIMO {
                let (ret, espelhar) = self.ret_criacao(g, estado.shift);
                let mut previa = ItemTela::de(
                    &ObjetoSolto::nova_forma(0, forma, Retangulo::default(), &ReguaZero),
                    &ReguaZero,
                );
                previa.espelhar = (espelhar, false);
                frame.with_clip(corpo, |f| {
                    f.translate(Vector::new(-corpo.x, -corpo.y));
                    Self::desenhar_forma(f, &previa, &[], forma, self.ret_tela(ret), 0.0, self.z(), false, corpo);
                });
            }
        }
        if gesto.is_some() {
            let mut m = estado.medida.borrow_mut();
            m.quadros += 1;
            let us = t0.elapsed().as_micros() as u64;
            m.us_desenho += us;
            m.us_max_desenho = m.us_max_desenho.max(us);
        }
    }

}

/// Régua de colunas/linhas de 1 px (só para montar a prévia da criação).
struct ReguaZero;
impl Regua for ReguaZero {
    fn inicio_coluna(&self, c: u32) -> f32 { c as f32 }
    fn inicio_linha(&self, r: u32) -> f32 { r as f32 }
    fn coluna_em(&self, x: f32) -> u32 { x.max(0.0) as u32 }
    fn linha_em(&self, y: f32) -> u32 { y.max(0.0) as u32 }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn grafico_rasterizado_na_thread_chega_pronto_ao_desenho() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100"><rect width="200" height="100" fill="#ff0000"/><!-- teste-adiantar --></svg>"##;
        let ret = Retangulo::novo(0.0, 0.0, 200.0, 100.0);
        adiantar_rasteres(vec![(ret, svg.to_string())], 1.5);
        let g = GraficoTela::novo(0, ret, svg.to_string());
        let (w, h) = tamanho_raster(ret, g.tamanho_svg, 1.5);
        assert_eq!((w, h), (300, 150));
        let chave = (g.chave, w, h);
        let inicio = std::time::Instant::now();
        while !ADIANTADOS.lock().unwrap().1.contains_key(&chave) {
            assert!(inicio.elapsed().as_secs() < 10, "a thread não rasterizou");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        // O desenho consome o adiantado (sai do depósito) em vez de rasterizar.
        assert!(raster(&g, w, h).is_some());
        assert!(!ADIANTADOS.lock().unwrap().1.contains_key(&chave));
    }

    #[test]
    fn redimensionar_sem_giro_segura_o_lado_oposto_e_shift_mantem_proporcao() {
        let base = Retangulo::novo(100.0, 80.0, 200.0, 100.0);
        assert_eq!(redimensionar(base, 0.0, Alca::Se, (50.0, 30.0), false, 4.0), Retangulo::novo(100.0, 80.0, 250.0, 130.0));
        assert_eq!(redimensionar(base, 0.0, Alca::Nw, (-20.0, -10.0), false, 4.0), Retangulo::novo(80.0, 70.0, 220.0, 110.0));
        let prop = redimensionar(base, 0.0, Alca::Se, (50.0, 0.0), true, 4.0);
        assert_eq!((prop.largura, prop.altura), (250.0, 125.0));
        let minimo = redimensionar(base, 0.0, Alca::W, (9999.0, 0.0), false, 4.0);
        assert_eq!((minimo.x, minimo.largura), (296.0, 4.0));
        assert_eq!(mover(base, (-500.0, 10.0)), Retangulo::novo(0.0, 90.0, 200.0, 100.0));
    }

    #[test]
    fn redimensionar_girado_mantem_a_alca_oposta_parada_na_tela() {
        let base = Retangulo::novo(100.0, 100.0, 200.0, 100.0);
        let giro = 90.0;
        let fixo = |r: Retangulo| girar_em_volta((r.x + r.largura, r.y + r.altura), r.centro(), giro);
        let novo = redimensionar(base, giro, Alca::Nw, (15.0, -40.0), false, 4.0);
        let (a, b) = (fixo(base), fixo(novo));
        assert!((a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01, "{a:?} {b:?}");
        // Arrastar (15, −40) na tela com 90 graus = (−40, −15) no sistema da forma.
        assert!((novo.largura - 240.0).abs() < 0.01 && (novo.altura - 115.0).abs() < 0.01, "{novo:?}");
    }

    #[test]
    fn girar_pelo_arraste_e_shift_de_15_em_15() {
        let c = Point::new(0.0, 0.0);
        assert!((girar(0.0, c, Point::new(0.0, -10.0), Point::new(10.0, 0.0), false) - 90.0).abs() < 0.01);
        let g = girar(350.0, c, Point::new(0.0, -10.0), Point::new(1.0, -10.0), true);
        assert!(g.abs() < 0.01 || (g - 360.0).abs() < 0.01, "{g}");
    }
}

