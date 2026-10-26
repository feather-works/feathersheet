//! PNG/JPEG incorporados na planilha com metadados compatíveis com FSHZ.
//! A importação mantém a origem intacta; o editor de efeitos atualiza
//! somente o campo `src` quando acionado explicitamente na Ribbon.
//! Usa o bloco opaco `im` do FSHZ / ImageConfig TypeScript,
//! sem converter imagens em valores de células.
use super::{App, Msg};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use crate::compat::mouse; use pelicano::column; use pelicano::container; use pelicano::image as iced_image; use pelicano::Image; use pelicano::mouse_area; use pelicano::row; use pelicano::stack; use pelicano::Space; use pelicano::Border; use pelicano::Color; use pelicano::Element; use pelicano::Length; use pelicano::Point;
use feathersheet_core::planilha::sheet::Sheet;
use serde_json::{json, Value};
use std::{collections::HashMap, io::Cursor, sync::{Arc, atomic::{AtomicU64, Ordering}}};

pub(super) const MAX_BYTES: usize = 16 * 1024 * 1024;
const MAX_PIXELS: u64 = 4_000_000;
const LIMITE_IMAGENS: usize = 64;
const MAX_LARGURA: f64 = 320.;
const MAX_ALTURA: f64 = 220.;
const CABECALHO_X: f64 = 90.;
const CABECALHO_Y: f64 = 30.;
/// Quanto a imagem inserida fica para dentro do canto da célula.
pub(super) const RECUO_X: f64 = 30.;
pub(super) const RECUO_Y: f64 = 24.;
/// A régua das imagens começa em `CABECALHO_X/Y`; a da grade, nos
/// cabeçalhos dela. Somar isto leva um ponto da imagem para a régua da grade.
pub(super) const DESLOC_X: f64 = CABECALHO_X - crate::grade::CABECALHO_LINHA as f64;
pub(super) const DESLOC_Y: f64 = CABECALHO_Y - crate::grade::CABECALHO_COLUNA as f64;

/// "Propriedades" do Formatar imagem: como a imagem acompanha as células
/// (chave `placement` do item; o padrão é o do Excel para imagens).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Posicionamento {
    /// Mover e dimensionar com células (`twoCell`).
    MoverEDimensionar,
    /// Mover mas não dimensionar com células (`oneCell`).
    #[default]
    Mover,
    /// Não mover nem dimensionar com células (`absolute`).
    Fixa,
}
impl Posicionamento {
    pub(super) fn de(item: &Value) -> Self {
        match item["placement"].as_str() {
            Some("twoCell") => Self::MoverEDimensionar,
            Some("absolute") => Self::Fixa,
            _ => Self::Mover,
        }
    }
    pub(super) fn chave(self) -> &'static str {
        match self {
            Self::MoverEDimensionar => "twoCell",
            Self::Mover => "oneCell",
            Self::Fixa => "absolute",
        }
    }
}
static PROXIMO_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub(super) struct Cache {
    pub(super) handles: std::cell::RefCell<HashMap<String, Image>>,
    // A posição muda na prévia do arraste sem reanalisar vários MiB de base64
    // a cada CursorMoved. A chave usa ponteiro e tamanho de Sheet.im.
    objetos: std::cell::RefCell<HashMap<u32, (usize, usize, Arc<Vec<Value>>)>>,
    /// Shift segurado: redimensionar pelos cantos mantém a proporção (como no TS).
    pub(super) shift: std::cell::Cell<bool>,
    /// Alt segurado: soltar a imagem arrastada a põe dentro da célula.
    pub(super) alt: std::cell::Cell<bool>,
}

/// Interação da imagem selecionada com o mouse. Sem editor de efeitos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesto {
    Mover,
    /// Canto inferior direito mantendo a proporção (gesto antigo).
    Redimensionar,
    /// Uma das oito alças; livre, e com Shift nos cantos mantém a proporção.
    Alca(Alca),
}

/// As oito alças de redimensionamento (mesmas do `image-overlay.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alca { Nw, N, Ne, E, Se, S, Sw, W }

impl Alca {
    pub const TODAS: [Alca; 8] = [Alca::Nw, Alca::N, Alca::Ne, Alca::E, Alca::Se, Alca::S, Alca::Sw, Alca::W];
    fn norte(self) -> bool { matches!(self, Alca::Nw | Alca::N | Alca::Ne) }
    fn sul(self) -> bool { matches!(self, Alca::Sw | Alca::S | Alca::Se) }
    fn leste(self) -> bool { matches!(self, Alca::Ne | Alca::E | Alca::Se) }
    fn oeste(self) -> bool { matches!(self, Alca::Nw | Alca::W | Alca::Sw) }
    fn canto(self) -> bool { matches!(self, Alca::Nw | Alca::Ne | Alca::Se | Alca::Sw) }

    pub(super) fn cursor(self) -> mouse::Interaction {
        match self {
            Alca::Nw | Alca::Se => mouse::Interaction::NwseResize,
            Alca::Ne | Alca::Sw => mouse::Interaction::NeswResize,
            Alca::N | Alca::S => mouse::Interaction::NsResize,
            Alca::E | Alca::W => mouse::Interaction::EwResize,
        }
    }

    /// Canto superior esquerdo da alça, em px da tela, dentro de uma imagem
    /// `largura` x `altura` (a alça fica encostada nas bordas, por dentro).
    pub(super) fn origem(self, largura: f64, altura: f64) -> (f64, f64) {
        let x = if self.oeste() { 0. } else if self.leste() { largura - TAM_ALCA } else { (largura - TAM_ALCA) / 2. };
        let y = if self.norte() { 0. } else if self.sul() { altura - TAM_ALCA } else { (altura - TAM_ALCA) / 2. };
        (x.max(0.), y.max(0.))
    }

    /// Qual alça está sob o ponto (coordenadas locais da imagem, px da tela)?
    pub fn em(ponto: (f64, f64), largura: f64, altura: f64) -> Option<Alca> {
        Alca::TODAS.into_iter().find(|a| {
            let (x, y) = a.origem(largura, altura);
            ponto.0 >= x && ponto.0 <= x + TAM_ALCA && ponto.1 >= y && ponto.1 <= y + TAM_ALCA
        })
    }
}

const TAM_ALCA: f64 = 10.;
const LADO_MINIMO: f64 = 20.;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Posicao {
    pub(super) x: f64,
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}
impl Posicao {
    /// Onde a imagem está agora: presa às células pela âncora (`de`/`ate`)
    /// conforme o [`Posicionamento`]; sem âncora, a `position` gravada.
    pub(super) fn efetiva(item: &Value, sheet: &Sheet) -> Option<Self> {
        use feathersheet_io::objetos_aba::Ancora;
        let pos = Self::ler(item)?;
        let regua = super::objetos::ReguaAba(sheet);
        let ancora = |k: &str| serde_json::from_value::<Ancora>(item[k].clone()).ok();
        let ponto = |a: Ancora| {
            let (x, y) = a.ponto(&regua);
            (f64::from(x) - DESLOC_X, f64::from(y) - DESLOC_Y)
        };
        Some(match (Posicionamento::de(item), ancora("de"), ancora("ate")) {
            (Posicionamento::Fixa, ..) | (_, None, _) => pos,
            (Posicionamento::MoverEDimensionar, Some(de), Some(ate)) => {
                let ((x0, y0), (x1, y1)) = (ponto(de), ponto(ate));
                Self { x: x0, y: y0, width: (x1 - x0).clamp(1., 4096.), height: (y1 - y0).clamp(1., 4096.) }
            }
            (_, Some(de), _) => {
                let (x, y) = ponto(de);
                Self { x, y, ..pos }
            }
        })
    }

    /// Grava a posição e a âncora nas células (as duas pontas).
    pub(super) fn gravar(self, item: &mut Value, sheet: &Sheet) {
        use feathersheet_io::objetos_aba::Ancora;
        for (campo, numero) in [("x", self.x), ("y", self.y), ("width", self.width), ("height", self.height)] {
            item["position"][campo] = json!(numero);
        }
        let regua = super::objetos::ReguaAba(sheet);
        let ancora = |x: f64, y: f64| serde_json::to_value(
            Ancora::em((x + DESLOC_X) as f32, (y + DESLOC_Y) as f32, &regua)).unwrap_or(Value::Null);
        item["de"] = ancora(self.x, self.y);
        item["ate"] = ancora(self.x + self.width, self.y + self.height);
    }

    fn ler(item: &Value) -> Option<Self> {
        let p = &item["position"];
        let pos = Self {
            x: p["x"].as_f64()?,
            y: p["y"].as_f64()?,
            width: p["width"].as_f64()?,
            height: p["height"].as_f64()?,
        };
        ([pos.x, pos.y, pos.width, pos.height].iter().all(|v| v.is_finite())
            && pos.width > 0. && pos.height > 0.
            && pos.width <= 4096. && pos.height <= 4096.).then_some(pos)
    }

    /// Redimensiona pela alça. Livre por padrão; `manter` (Shift) preserva a
    /// proporção só nos cantos. O lado oposto à alça fica parado.
    fn redimensionar(self, inicio: Point, atual: Point, zoom: f64, alca: Alca, manter: bool) -> Self {
        if !zoom.is_finite() || zoom <= 0. {
            return self;
        }
        let dx = f64::from(atual.x - inicio.x) / zoom;
        let dy = f64::from(atual.y - inicio.y) / zoom;
        if !dx.is_finite() || !dy.is_finite() {
            return self;
        }
        let (mut x, mut y, mut w, mut h) = (self.x, self.y, self.width, self.height);
        if alca.leste() { w = (self.width + dx).clamp(LADO_MINIMO, 4096.); }
        if alca.oeste() {
            w = (self.width - dx).clamp(LADO_MINIMO, 4096.);
            x = self.x + self.width - w;
        }
        if alca.sul() { h = (self.height + dy).clamp(LADO_MINIMO, 4096.); }
        if alca.norte() {
            h = (self.height - dy).clamp(LADO_MINIMO, 4096.);
            y = self.y + self.height - h;
        }
        if manter && alca.canto() {
            let ratio = self.width / self.height;
            let por_largura = (w / self.width - 1.).abs() >= (h / self.height - 1.).abs();
            let largura = if por_largura { w } else { h * ratio };
            let largura = largura.clamp(LADO_MINIMO.max(LADO_MINIMO * ratio), 4096.0_f64.min(4096. * ratio));
            w = largura;
            h = largura / ratio;
            x = if alca.oeste() { self.x + self.width - w } else { self.x };
            y = if alca.norte() { self.y + self.height - h } else { self.y };
        }
        Self {
            x: x.max(0.).round(),
            y: y.max(0.).round(),
            width: w.round().max(1.),
            height: h.round().max(1.),
        }
    }

    /// Converte o movimento do cursor no tamanho/posição em pixels lógicos
    /// da planilha, não em pixels de tela. O redimensionamento preserva AR.
    fn transformar(self, inicio: Point, atual: Point, zoom: f64, gesto: Gesto) -> Self {
        if !zoom.is_finite() || zoom <= 0. {
            return self;
        }
        let dx = f64::from(atual.x - inicio.x) / zoom;
        let dy = f64::from(atual.y - inicio.y) / zoom;
        if !dx.is_finite() || !dy.is_finite() || (dx == 0. && dy == 0.) {
            return self;
        }
        match gesto {
            Gesto::Mover => Self {
                x: (self.x + dx).clamp(0., 1_000_000_000.),
                y: (self.y + dy).clamp(0., 1_000_000_000.),
                ..self
            },
            Gesto::Alca(alca) => self.redimensionar(inicio, atual, zoom, alca, false),
            Gesto::Redimensionar => {
                let ratio = self.width / self.height;
                let dw = if dx.abs() >= (dy * ratio).abs() { dx } else { dy * ratio };
                let minimo = 24.0_f64.max(24. * ratio);
                let maximo = 4096.0_f64.min(4096. * ratio);
                let largura = (self.width + dw).clamp(minimo.min(maximo), maximo);
                Self { width: largura, height: largura / ratio, ..self }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Arraste {
    id: String,
    aba_id: u32,
    inicio: Point,
    base: Posicao,
    atual: Posicao,
    gesto: Gesto,
    /// "Fixar taxa de proporção": os cantos mantêm a proporção (Shift também).
    travar: bool,
}

/// Atualiza somente a posição do objeto em Sheet.im. Todos os pixels e
/// metadados que vieram do FSHZ/TypeScript permanecem intactos.
fn salvar_posicao(sheet: &mut Sheet, id: &str, nova: Posicao) -> Result<bool, String> {
    let mut lista = itens(sheet)?;
    let item = lista.iter_mut().find(|v| v["id"].as_str() == Some(id))
        .ok_or(tr!("A imagem selecionada não existe mais."))?;
    let antiga = Posicao::efetiva(item, sheet).ok_or(tr!("A posição da imagem é inválida."))?;
    if antiga == nova {
        return Ok(false);
    }
    nova.gravar(item, sheet);
    let gravado = serde_json::to_string(&lista)
        .map_err(|e| trf!("Não consegui atualizar a imagem: {e}"))?;
    // Um único passo de desfazer, junto com o resto do histórico da aba.
    sheet.alterar_imagens(Some(gravado));
    Ok(true)
}

/// Muda campos de uma imagem solta (um passo de desfazer). `f` devolve
/// `false` quando não mudou nada.
pub(super) fn alterar_item(sheet: &mut Sheet, id: &str, f: impl FnOnce(&mut Value, &Sheet) -> bool) -> Result<bool, String> {
    let mut lista = itens(sheet)?;
    let item = lista.iter_mut().find(|v| v["id"].as_str() == Some(id))
        .ok_or(tr!("A imagem selecionada não existe mais."))?;
    if !f(item, sheet) {
        return Ok(false);
    }
    let gravado = serde_json::to_string(&lista)
        .map_err(|e| trf!("Não consegui atualizar a imagem: {e}"))?;
    sheet.alterar_imagens(Some(gravado));
    Ok(true)
}

/// Largura e altura em pixels da figura do item.
pub(super) fn tamanho_original(item: &Value) -> Option<(u32, u32)> {
    let bytes = decodificar_src(item["src"].as_str()?)?;
    image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?.into_dimensions().ok()
}

/// O `src` e o canto (x, y) da imagem solta.
pub(super) fn src_e_posicao(sheet: &Sheet, id: &str) -> Result<(String, f64, f64), String> {
    let lista = itens(sheet)?;
    let item = lista.iter().find(|v| v["id"].as_str() == Some(id))
        .ok_or(tr!("A imagem selecionada não existe mais."))?;
    let pos = Posicao::efetiva(item, sheet).ok_or(tr!("A posição da imagem é inválida."))?;
    let src = item["src"].as_str().ok_or(tr!("Não consegui ler a imagem."))?;
    Ok((src.to_owned(), pos.x, pos.y))
}

/// Remove a imagem do bloco `im` (um passo de desfazer).
pub(super) fn remover_imagem(sheet: &mut Sheet, id: &str) -> Result<bool, String> {
    let mut lista = itens(sheet)?;
    let antes = lista.len();
    lista.retain(|v| v["id"].as_str() != Some(id));
    if lista.len() == antes {
        return Ok(false);
    }
    let gravado = if lista.is_empty() { None } else {
        Some(serde_json::to_string(&lista)
            .map_err(|e| trf!("Não consegui atualizar as imagens: {e}"))?)
    };
    sheet.alterar_imagens(gravado);
    Ok(true)
}

fn itens(sheet: &Sheet) -> Result<Vec<Value>, String> {
    let Some(data) = &sheet.im else { return Ok(Vec::new()) };
    let lista: Vec<Value> = serde_json::from_str(data)
        .map_err(|_| tr!("Não foi possível ler as imagens desta aba.").to_string())?;
    if lista.len() > LIMITE_IMAGENS {
        return Err(tr!("O limite de 64 imagens por aba foi excedido.").into());
    }
    Ok(lista)
}

fn decodificar_src(src: &str) -> Option<Vec<u8>> {
    if src.len() > MAX_BYTES * 4 / 3 + 100 {
        return None;
    }
    let conteudo = src.strip_prefix("data:image/png;base64,")
        .or_else(|| src.strip_prefix("data:image/jpeg;base64,"))?;
    let dados = STANDARD.decode(conteudo).ok()?;
    (dados.len() <= MAX_BYTES && !dados.is_empty()).then_some(dados)
}

/// Adiciona uma imagem real, com dimensões de exibição contidas e dados
/// originais no campo `im`. Sem efeitos, reamostragem ou cópias em células.
pub(super) fn importar(
    sheet: &mut Sheet, bytes: &[u8], nome: &str, linha: u32, coluna: u32,
) -> Result<String, String> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err(tr!("Selecione uma imagem PNG/JPEG de até 16 MiB.").into());
    }
    let formato = image::guess_format(bytes).map_err(|_| tr!("Imagem inválida.").to_string())?;
    let mime = match formato {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        _ => return Err(tr!("Importe uma imagem PNG ou JPEG.").into()),
    };
    let (w, h) = image::ImageReader::with_format(Cursor::new(bytes), formato)
        .into_dimensions().map_err(|e| trf!("Cabeçalho de imagem inválido: {e}"))?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(tr!("A imagem excede 4 milhões de pixels.").into());
    }
    // Confirma que o arquivo inteiro pode ser decodificado antes de modificar a aba.
    image::load_from_memory(bytes).map_err(|e| trf!("Imagem corrompida: {e}"))?;
    let mut anteriores = itens(sheet)?;
    if anteriores.len() >= LIMITE_IMAGENS {
        return Err(tr!("Limite de 64 imagens por aba atingido.").into());
    }
    let escala = (MAX_LARGURA / f64::from(w)).min(MAX_ALTURA / f64::from(h)).min(1.);
    let numero = PROXIMO_ID.fetch_add(1, Ordering::Relaxed);
    let id = format!("rust-imagem-{numero}-{}", std::process::id());
    let (largura, altura) = ((f64::from(w) * escala).round().max(1.), (f64::from(h) * escala).round().max(1.));
    let mut novo = json!({
        "id": id,
        "src": format!("data:{mime};base64,{}", STANDARD.encode(bytes)),
        "name": nome,
        "aspectRatio": f64::from(w) / f64::from(h),
        // Tamanho ao inserir: a escala "não relativa ao original" parte dele.
        "base": {"width": largura, "height": altura}
    });
    // Régua real da aba (larguras e alturas próprias), a da grade.
    Posicao {
        x: f64::from(crate::grade::modelo::inicio_x_da(sheet, coluna)) + RECUO_X,
        y: f64::from(crate::grade::modelo::inicio_y_da(sheet, linha)) + RECUO_Y,
        width: largura,
        height: altura,
    }.gravar(&mut novo, sheet);
    anteriores.push(novo);
    let json = serde_json::to_string(&anteriores)
        .map_err(|e| trf!("Não consegui guardar a imagem: {e}"))?;
    sheet.im = Some(json);
    Ok(id)
}

/// Converte pixels RGBA 8 bits (como o `arboard` entrega) em PNG. Reduz a
/// imagem quando passa do limite de pixels da importação (ex.: print 4K).
pub(super) fn rgba_para_png(largura: u32, altura: u32, rgba: Vec<u8>) -> Result<Vec<u8>, String> {
    let img = image::RgbaImage::from_raw(largura, altura, rgba)
        .ok_or(tr!("Pixels da área de transferência inconsistentes."))?;
    if largura == 0 || altura == 0 {
        return Err(tr!("Imagem vazia na área de transferência.").into());
    }
    let pixels = u64::from(largura) * u64::from(altura);
    let img = if pixels > MAX_PIXELS {
        let f = (MAX_PIXELS as f64 / pixels as f64).sqrt();
        let (w, h) = (((f64::from(largura) * f) as u32).max(1), ((f64::from(altura) * f) as u32).max(1));
        image::imageops::resize(&img, w, h, image::imageops::FilterType::Triangle)
    } else { img };
    let mut saida = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut Cursor::new(&mut saida), image::ImageFormat::Png)
        .map_err(|e| trf!("Falha ao codificar PNG: {e}"))?;
    Ok(saida)
}

/// Bloqueante: chamar fora do loop da interface. `None` se não há imagem.
pub(super) fn ler_png_da_area_de_transferencia() -> Option<Vec<u8>> {
    let dados = arboard::Clipboard::new().ok()?.get_image().ok()?;
    rgba_para_png(u32::try_from(dados.width).ok()?, u32::try_from(dados.height).ok()?,
        dados.bytes.into_owned()).ok()
}

/// Recupera a última imagem importada na aba para edição manual.
/// O editor não muda a seleção ou o posicionamento de imagens na grade.
pub(super) fn ultima_imagem(sheet: &Sheet) -> Result<Option<(String, Vec<u8>, String)>, String> {
    for item in itens(sheet)?.into_iter().rev() {
        let (Some(id), Some(src)) = (item["id"].as_str(), item["src"].as_str()) else {
            continue;
        };
        if let Some(bytes) = decodificar_src(src) {
            let nome = item["name"].as_str().unwrap_or(tr!("Imagem")).to_owned();
            return Ok(Some((id.to_owned(), bytes, nome)));
        }
    }
    Ok(None)
}

/// Substitui somente o raster de um objeto existente, preservando o ID,
/// posição, dimensões e demais metadados de compatibilidade FSHZ/TypeScript.
/// Valida e serializa ANTES de atribuir, para evitar corrupção em caso de falha.
pub(super) fn substituir_png(sheet: &mut Sheet, id: &str, png: &[u8]) -> Result<(), String> {
    if png.is_empty() || png.len() > MAX_BYTES {
        return Err(tr!("PNG de efeito vazio ou maior que 16 MiB.").into());
    }
    let formato = image::guess_format(png)
        .map_err(|_| tr!("O efeito não gerou um PNG válido.").to_string())?;
    if formato != image::ImageFormat::Png {
        return Err(tr!("Somente PNG pode substituir imagem após efeitos.").into());
    }
    let (w, h) = image::ImageReader::with_format(Cursor::new(png), formato)
        .into_dimensions().map_err(|e| trf!("PNG inválido: {e}"))?;
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_PIXELS {
        return Err(tr!("PNG excede 4 milhões de pixels.").into());
    }
    let mut lista = itens(sheet)?;
    let alvo = lista.iter_mut().find(|v| v["id"].as_str() == Some(id))
        .ok_or(tr!("Imagem não encontrada nesta aba."))?;
    // A imagem original pode ser JPEG: o efeito salvo tem MIME PNG.
    alvo["src"] = Value::String(format!("data:image/png;base64,{}", STANDARD.encode(png)));
    let nova = serde_json::to_string(&lista)
        .map_err(|e| trf!("Não foi possível salvar a imagem: {e}"))?;
    sheet.im = Some(nova);
    Ok(())
}

impl App {
    /// Um clique seleciona, arrastar no corpo move, e arrastar a alça ↘
    /// redimensiona preservando proporções. Não desloca Ribbon ou barra fx.
    pub(super) fn grade_com_imagens(&self) -> Element<'_, Msg> {
        let grade = self.view_grade();
        let Some(aba) = self.workbook.aba(self.folha) else { return grade };
        let Ok(lista) = self.itens_cached(aba.id, &aba.sheet) else { return grade };
        if lista.is_empty() {
            return grade;
        }
        let zoom = f64::from(self.config.zoom) / 100.;
        // A primeira camada enche: a pilha das imagens ocupa a grade toda para o recorte abaixo.
        let mut camadas: Vec<Element<'_, Msg>> = vec![Space::new(Length::FILL, Length::FILL).into()];
        use crate::grade::modelo::{inicio_x_da, inicio_y_da};
        let x_desloc = f64::from(inicio_x_da(&aba.sheet, self.coluna_inicio));
        let y_desloc = f64::from(inicio_y_da(&aba.sheet, self.linha_inicio));
        let (linhas_fixas, colunas_fixas) = self.congelamento_atual();
        let faixa_fixa_x = f64::from(inicio_x_da(&aba.sheet, colunas_fixas));
        let faixa_fixa_y = f64::from(inicio_y_da(&aba.sheet, linhas_fixas));

        // Ref<&Value> evita copiar os dados base64 de cada item durante repaint.
        // A imagem ativa permanece por último (acima das demais).
        let mut ordem: Vec<&Value> = lista.iter().collect();
        if let Some((aba_id, selecionado)) = &self.imagem_selecionada {
            if *aba_id == aba.id {
                ordem.sort_by_key(|v| v["id"].as_str() == Some(selecionado.as_str()));
            }
        }

        for item in ordem {
            let (Some(id), Some(src)) = (item["id"].as_str(), item["src"].as_str()) else {
                continue;
            };
            let Some(mut pos) = Posicao::efetiva(item, &aba.sheet) else { continue };
            let selecionada = self.imagem_selecionada.as_ref()
                .is_some_and(|(aba_id, escolhido)| *aba_id == aba.id && escolhido == id);
            if let Some(arraste) = &self.arraste_imagem {
                if arraste.aba_id == aba.id && arraste.id == id {
                    pos = arraste.atual;
                }
            }
            // Objetos ancorados dentro do painel fixo permanecem estáticos.
            // Os demais acompanham somente a origem da região rolável.
            let x = (if pos.x < faixa_fixa_x { pos.x }
                else { pos.x - x_desloc + faixa_fixa_x }) * zoom + CABECALHO_X * zoom;
            let y = (if pos.y < faixa_fixa_y { pos.y }
                else { pos.y - y_desloc + faixa_fixa_y }) * zoom + CABECALHO_Y * zoom;
            let width = pos.width * zoom;
            let height = pos.height * zoom;
            // A área de interação é apenas o próprio retângulo da imagem.
            // Não desenha por cima dos cabeçalhos de linha e coluna.
            if x < CABECALHO_X * zoom || y < CABECALHO_Y * zoom || x > 10000. || y > 10000. ||
                width > 8192. || height > 8192. {
                continue;
            }
            let handle = {
                let cache = self.imagens_importadas.handles.borrow();
                cache.get(id).cloned()
            }.or_else(|| {
                let bytes = decodificar_src(src)?;
                let handle = crate::compat::imagem_de_bytes(&bytes)?;
                self.imagens_importadas.handles.borrow_mut()
                    .insert(id.to_owned(), handle.clone());
                Some(handle)
            });
            let Some(handle) = handle else { continue };

            let giro = item["rotation"].as_f64().unwrap_or(0.);
            // O giro não mexe no layout (o `Rotation::Floating` do Iced); sem transição.
            let imagem = mouse_area(
                pelicano::animate(
                    iced_image(handle)
                        .width(Length::Fixed(width as f32))
                        .height(Length::Fixed(height as f32))
                        // Sem "fixar proporção" a figura estica no tamanho pedido.
                        .content_fit(pelicano::ContentFit::Fill),
                ).rotate(giro as f32).transition(pelicano::Transition::new(0)),
            ).on_press(Msg::ImagemPressionar(id.to_owned(), Gesto::Mover))
            .on_right_press(Msg::ImagemCelula(super::imagem_na_celula::Evento::Contexto(id.to_owned())))
            .on_scroll(crate::app::botoes_macro::rolar_pela_roda)
            .cursor_icon(mouse::Interaction::Grab);
            let conteudo: Element<'_, Msg> = if selecionada {
                let moldura = container(imagem).style(|_tema| pelicano::Style {
                    border: Border {
                        color: Color::rgb8(57, 115, 225),
                        width: 2.0,
                        radius: [2.0; 4],
                    },
                    ..Default::default()
                });
                // As oito alças ficam por cima da moldura, cada uma com hitbox
                // e cursor próprios: não iniciam o gesto de mover.
                let mut camadas_alcas: Vec<Element<'_, Msg>> = vec![moldura.into()];
                for alca in Alca::TODAS {
                    let (ax, ay) = alca.origem(width, height);
                    let quadrado = container(Space::new(Length::Fixed(TAM_ALCA as f32), Length::Fixed(TAM_ALCA as f32)))
                        .style(|_tema| pelicano::Style {
                            background: Some(Color::WHITE.into()),
                            border: Border { color: Color::rgb8(37, 99, 235), width: 1.0, radius: [2.0; 4] },
                            ..Default::default()
                        });
                    let alca_widget = mouse_area(quadrado)
                        .on_press(Msg::ImagemPressionar(id.to_owned(), Gesto::Alca(alca)))
                        .cursor_icon(alca.cursor());
                    camadas_alcas.push(column![
                        Space::new(0.0, Length::Fixed(ay as f32)),
                        row![Space::new(Length::Fixed(ax as f32), 0.0), alca_widget],
                    ].into());
                }
                stack(camadas_alcas).into()
            } else {
                imagem.into()
            };
            camadas.push(column![
                Space::new(0.0, Length::Fixed(y as f32)),
                row![
                    Space::new(Length::Fixed(x as f32), 0.0),
                    conteudo,
                ],
            ].into());
        }
        // Recortada pela grade sem as barras de rolagem (como os gráficos): o pelicano
        // não limita a imagem ao espaço do pai, e ela cobria a barra e o rodapé.
        let barra = crate::grade::barras::ESPESSURA;
        let imagens = column![
            row![
                container(stack(camadas).height(Length::FILL).width(Length::FILL))
                    .width(Length::FILL).height(Length::FILL).clip(true),
                Space::new(Length::Fixed(barra), 0.0),
            ].height(Length::FILL),
            Space::new(0.0, Length::Fixed(barra)),
        ].width(Length::FILL).height(Length::FILL);
        stack([grade, imagens.into()]).height(Length::FILL).width(Length::FILL).into()
    }

    /// O widget mostra o estado local enquanto o botão está pressionado.
    /// Somente mouse-up serializa JSON, evitando reserializar PNGs de MiBs
    /// sessenta vezes por segundo durante um arraste.
    pub(super) fn pressionar_imagem(&mut self, id: String, gesto: Gesto) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let Ok(lista) = itens(&aba.sheet) else { return };
        let Some(item) = lista.iter().find(|v| v["id"].as_str() == Some(&id)) else {
            return;
        };
        let Some(pos) = Posicao::efetiva(item, &aba.sheet) else { return };
        self.imagem_selecionada = Some((aba.id, id.clone()));
        // Bloqueada (Formatar imagem › Propriedades) com a aba protegida: só seleciona.
        if aba.sheet.protegida() && item["locked"].as_bool().unwrap_or(true) {
            self.status = tr!("A imagem está bloqueada: desproteja a planilha para mover.").into();
            return;
        }
        let travar = item["lockAspect"].as_bool().unwrap_or(true);
        self.arraste_imagem = Some(Arraste {
            id,
            aba_id: aba.id,
            inicio: self.graficos.cursor,
            base: pos,
            atual: pos,
            gesto,
            travar,
        });
    }

    pub(super) fn mover_cursor_imagem(&mut self, cursor: Point) {
        if let Some(drag) = &mut self.arraste_imagem {
            let zoom = f64::from(self.config.zoom) / 100.;
            drag.atual = match drag.gesto {
                Gesto::Alca(alca) => drag.base.redimensionar(
                    drag.inicio, cursor, zoom, alca, drag.travar || self.imagens_importadas.shift.get()),
                outro => drag.base.transformar(drag.inicio, cursor, zoom, outro),
            };
        }
    }

    pub(super) fn soltar_imagem(&mut self) {
        let Some(drag) = self.arraste_imagem.take() else { return };
        // Alt + arrastar: a imagem vai para dentro da célula onde o canto dela caiu.
        if self.imagens_importadas.alt.get() && drag.gesto == Gesto::Mover && drag.atual != drag.base {
            self.imagem_selecionada = Some((drag.aba_id, drag.id));
            if let Err(e) = self.colocar_na_celula_em(Some((drag.atual.x, drag.atual.y))) {
                self.status = e;
            }
            return;
        }
        let Some(aba) = self.workbook.aba_mut(self.folha)
            .filter(|a| a.id == drag.aba_id) else { return };
        match salvar_posicao(&mut aba.sheet, &drag.id, drag.atual) {
            Ok(true) => {
                self.imagens_importadas.objetos.borrow_mut().remove(&drag.aba_id);
                self.alterado = true;
                self.status = match drag.gesto {
                    Gesto::Mover => tr!("Imagem movida na planilha."),
                    Gesto::Redimensionar | Gesto::Alca(_) => tr!("Imagem redimensionada."),
                }.into();
            }
            Ok(false) => {}
            Err(e) => self.status = e,
        }
    }

    /// Delete/Backspace com a imagem selecionada apaga a imagem (desfazível).
    pub(super) fn apagar_imagem_selecionada(&mut self) {
        let Some((aba_id, id)) = self.imagem_selecionada.take() else { return };
        self.arraste_imagem = None;
        let Some(aba) = self.workbook.aba_mut(self.folha).filter(|a| a.id == aba_id) else { return };
        match remover_imagem(&mut aba.sheet, &id) {
            Ok(true) => {
                self.imagens_importadas.objetos.borrow_mut().remove(&aba_id);
                self.imagens_importadas.handles.borrow_mut().remove(&id);
                self.alterado = true;
                self.status = tr!("Imagem excluída (Ctrl+Z desfaz).").into();
            }
            Ok(false) => {}
            Err(e) => self.status = e,
        }
    }

    /// Cache da lista de metadados; invalida quando troca a String de Sheet.im.
    /// Com isso, arrastar a imagem não custa O(tamanho do PNG base64) por frame.
    pub(super) fn itens_cached(&self, aba_id: u32, sheet: &Sheet) -> Result<Arc<Vec<Value>>, String> {
        let Some(json) = &sheet.im else { return Ok(Arc::new(Vec::new())) };
        let chave = (json.as_ptr() as usize, json.len());
        if let Some((ptr, len, lista)) = self.imagens_importadas.objetos.borrow().get(&aba_id) {
            if (*ptr, *len) == chave {
                return Ok(Arc::clone(lista));
            }
        }
        let valores = Arc::new(itens(sheet)?);
        self.imagens_importadas.objetos.borrow_mut()
            .insert(aba_id, (chave.0, chave.1, Arc::clone(&valores)));
        Ok(valores)
    }

    pub(super) fn invalidar_cache_objetos(&self) {
        self.imagens_importadas.objetos.borrow_mut().clear();
    }

    pub(super) fn limpar_cache_imagens(&self) {
        self.imagens_importadas.handles.borrow_mut().clear();
        self.imagens_importadas.objetos.borrow_mut().clear();
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::planilha::workbook::Workbook;

    fn png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(2, 3, image::Rgba([18, 40, 80, 175]));
        let mut data = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut data), image::ImageFormat::Png).unwrap();
        data
    }

    #[test]
    fn rgba_vira_png_e_insere_como_imagem() {
        let png = rgba_para_png(2, 2, vec![255, 0, 0, 255].repeat(4)).unwrap();
        assert_eq!(image::guess_format(&png).unwrap(), image::ImageFormat::Png);
        let mut sheet = Sheet::default();
        let id = importar(&mut sheet, &png, "Captura de tela.png", 1, 1).unwrap();
        assert!(sheet.im.unwrap().contains(&id));
        assert!(rgba_para_png(3, 3, vec![0; 4]).is_err());
        let grande = rgba_para_png(3000, 2000, vec![200; 3000 * 2000 * 4]).unwrap();
        assert!(importar(&mut Sheet::default(), &grande, "g.png", 0, 0).is_ok());
    }

    #[test]
    fn mover_e_redimensionar_respeitam_zoom_e_proporcao() {
        let p = Posicao { x: 100., y: 80., width: 200., height: 100. };
        let inicio = Point::new(100., 120.);
        let fim = Point::new(150., 140.);
        let movido = p.transformar(inicio, fim, 2., Gesto::Mover);
        assert_eq!(movido, Posicao { x: 125., y: 90., ..p });
        let resize = p.transformar(inicio, fim, 2., Gesto::Redimensionar);
        assert_eq!(resize.width, 225.);
        assert_eq!(resize.height, 112.5);
        assert_eq!(resize.x, p.x);
        assert_eq!(resize.y, p.y);
        assert_eq!(p.transformar(inicio, inicio, 1., Gesto::Mover), p);
        assert_eq!(p.transformar(inicio, inicio, 1., Gesto::Redimensionar), p);
    }

    #[test]
    fn alca_hit_test_acha_cantos_e_lados() {
        assert_eq!(Alca::em((2., 2.), 200., 100.), Some(Alca::Nw));
        assert_eq!(Alca::em((100., 3.), 200., 100.), Some(Alca::N));
        assert_eq!(Alca::em((197., 3.), 200., 100.), Some(Alca::Ne));
        assert_eq!(Alca::em((197., 50.), 200., 100.), Some(Alca::E));
        assert_eq!(Alca::em((197., 97.), 200., 100.), Some(Alca::Se));
        assert_eq!(Alca::em((100., 97.), 200., 100.), Some(Alca::S));
        assert_eq!(Alca::em((3., 97.), 200., 100.), Some(Alca::Sw));
        assert_eq!(Alca::em((3., 50.), 200., 100.), Some(Alca::W));
        assert_eq!(Alca::em((100., 50.), 200., 100.), None);
    }

    #[test]
    fn alca_do_canto_aumenta_largura_e_altura_e_shift_mantem_proporcao() {
        let p = Posicao { x: 100., y: 80., width: 200., height: 100. };
        let ini = Point::new(0., 0.);
        let se = p.redimensionar(ini, Point::new(50., 30.), 1., Alca::Se, false);
        assert_eq!((se.width, se.height, se.x, se.y), (250., 130., 100., 80.));
        let nw = p.redimensionar(ini, Point::new(-20., -10.), 1., Alca::Nw, false);
        assert_eq!((nw.x, nw.y, nw.width, nw.height), (80., 70., 220., 110.));
        let lado = p.redimensionar(ini, Point::new(40., 99.), 1., Alca::E, false);
        assert_eq!((lado.width, lado.height), (240., 100.));
        let prop = p.redimensionar(ini, Point::new(50., 0.), 1., Alca::Se, true);
        assert_eq!((prop.width, prop.height), (250., 125.));
        let minimo = p.redimensionar(ini, Point::new(9999., 9999.), 1., Alca::Nw, false);
        assert_eq!((minimo.width, minimo.height), (20., 20.));
        assert_eq!((minimo.x, minimo.y), (280., 160.));
    }

    #[test]
    fn redimensionar_e_apagar_imagem_desfazem_em_um_passo() {
        let mut sheet = Sheet::default();
        let id = importar(&mut sheet, &png(), "a.png", 0, 0).unwrap();
        let antes = sheet.im.clone();
        let base = Posicao::ler(&itens(&sheet).unwrap()[0]).unwrap();
        let nova = Posicao { width: 90., height: 70., ..base };
        assert!(salvar_posicao(&mut sheet, &id, nova).unwrap());
        assert_eq!(Posicao::ler(&itens(&sheet).unwrap()[0]).unwrap(), nova);
        assert!(sheet.undo());
        assert_eq!(sheet.im, antes);
        assert!(sheet.redo());
        assert_eq!(Posicao::ler(&itens(&sheet).unwrap()[0]).unwrap(), nova);
        assert!(remover_imagem(&mut sheet, &id).unwrap());
        assert!(itens(&sheet).unwrap().is_empty());
        assert!(sheet.undo());
        assert_eq!(itens(&sheet).unwrap().len(), 1);
    }

    #[test]
    fn limites_impedem_tamanho_negativo_e_imagem_gigante() {
        let p = Posicao { x: 20., y: 20., width: 100., height: 50. };
        let ini = Point::new(0., 0.);
        let grande = p.transformar(ini, Point::new(99999., 0.), 1., Gesto::Redimensionar);
        assert_eq!(grande.width, 4096.);
        assert_eq!(grande.height, 2048.);
        let pequena = p.transformar(ini, Point::new(-9999., -9999.), 1., Gesto::Redimensionar);
        assert_eq!(pequena.width, 48.); // altura mínima 24 com proporção 2:1
        assert_eq!(pequena.height, 24.);
        let sem_negativos = p.transformar(ini, Point::new(-100., -100.), 1., Gesto::Mover);
        assert_eq!(sem_negativos.x, 0.);
        assert_eq!(sem_negativos.y, 0.);
    }

    #[test]
    fn posicao_salva_sem_alterar_pixels_e_sobrevive_fshz() {
        let mut w = Workbook::new();
        let id = importar(&mut w.aba_mut(0).unwrap().sheet, &png(), "logo.png", 2, 3).unwrap();
        let original = itens(&w.aba(0).unwrap().sheet).unwrap();
        let src = original[0]["src"].clone();
        let base = Posicao::ler(&original[0]).unwrap();
        let alterada = Posicao {
            x: 300.0, y: 240.0, width: 120., height: 180.,
        };
        assert!(salvar_posicao(&mut w.aba_mut(0).unwrap().sheet, &id, alterada).unwrap());
        assert!(!salvar_posicao(&mut w.aba_mut(0).unwrap().sheet, &id, alterada).unwrap());
        let atual = itens(&w.aba(0).unwrap().sheet).unwrap();
        assert_eq!(atual[0]["src"], src);
        assert_eq!(atual[0]["name"], original[0]["name"]);
        assert_ne!(Posicao::ler(&atual[0]).unwrap(), base);
        let arquivo = feathersheet_io::da_workbook::pasta_gravar_bytes(&w, 0).unwrap();
        let decoded = feathersheet_io::fshz_read::ler_fshz(&arquivo).unwrap();
        let restaurada = feathersheet_io::para_workbook::para_workbook(&decoded);
        let imagens = itens(&restaurada.aba(0).unwrap().sheet).unwrap();
        assert_eq!(imagens, atual);
        assert_eq!(Posicao::ler(&imagens[0]).unwrap(), alterada);
        assert_eq!(restaurada.aba(0).unwrap().sheet.get_raw(2,3), "");
    }

    #[test]
    fn posicao_invalida_e_id_desconhecido_nao_mudam_fshz() {
        let mut sheet = Sheet::default();
        let id = importar(&mut sheet, &png(), "ok.png", 0, 0).unwrap();
        let salvo = sheet.im.clone();
        assert!(salvar_posicao(&mut sheet, "desconhecido", Posicao {
            x: 0., y: 0., width: 10., height: 10.,
        }).is_err());
        assert_eq!(sheet.im, salvo);
        sheet.im = Some("JSON corrompido".into());
        let erro = sheet.im.clone();
        assert!(salvar_posicao(&mut sheet, &id, Posicao {
            x: 0., y: 0., width: 40., height: 30.,
        }).is_err());
        assert_eq!(sheet.im, erro);
    }

    #[test]
    fn efeitos_substituem_so_pixels_sem_mover_metadados() {
        let mut sheet = Sheet::default();
        let primeira = importar(&mut sheet, &png(), "original.png", 3, 4).unwrap();
        let segunda = importar(&mut sheet, &png(), "segunda.png", 7, 8).unwrap();
        let antes = itens(&sheet).unwrap();
        let (id, bytes, nome) = ultima_imagem(&sheet).unwrap().unwrap();
        assert_eq!(id, segunda);
        assert_eq!(bytes, png());
        assert_eq!(nome, "segunda.png");

        let alterada = image::RgbaImage::from_pixel(2, 3, image::Rgba([100, 120, 130, 90]));
        let mut bytes_png = Vec::new();
        image::DynamicImage::ImageRgba8(alterada)
            .write_to(&mut Cursor::new(&mut bytes_png), image::ImageFormat::Png).unwrap();
        substituir_png(&mut sheet, &id, &bytes_png).unwrap();
        let depois = itens(&sheet).unwrap();
        assert_eq!(depois.len(), 2);
        assert_eq!(depois[0], antes[0]);
        assert_eq!(depois[1]["id"], segunda);
        assert_eq!(depois[0]["id"], primeira);
        for campo in ["name", "position", "aspectRatio"] {
            assert_eq!(depois[1][campo], antes[1][campo]);
        }
        assert_eq!(decodificar_src(depois[1]["src"].as_str().unwrap()), Some(bytes_png));
        let anterior = sheet.im.clone();
        assert!(substituir_png(&mut sheet, &id, b"nao e png").is_err());
        assert!(substituir_png(&mut sheet, "id-inexistente", &png()).is_err());
        assert_eq!(sheet.im, anterior);
    }

    #[test]
    fn insere_sem_tocar_celulas_e_conserva_fshz() {
        let mut book = Workbook::new();
        let id = importar(&mut book.aba_mut(0).unwrap().sheet, &png(), "logo.png", 3, 2).unwrap();
        let antes = itens(&book.aba(0).unwrap().sheet).unwrap();
        assert_eq!(antes.len(), 1);
        assert_eq!(antes[0]["id"], id);
        assert_eq!(antes[0]["position"]["width"], 2.);
        assert_eq!(antes[0]["position"]["height"], 3.);
        assert_eq!(book.aba(0).unwrap().sheet.get_raw(3, 2), "");
        let arquivo = feathersheet_io::da_workbook::pasta_gravar_bytes(&book, 0).unwrap();
        let reaberta = feathersheet_io::para_workbook::para_workbook(
            &feathersheet_io::fshz_read::ler_fshz(&arquivo).unwrap());
        let depois = itens(&reaberta.aba(0).unwrap().sheet).unwrap();
        assert_eq!(antes, depois);
        assert_eq!(decodificar_src(depois[0]["src"].as_str().unwrap()), Some(png()));
    }

    #[test]
    fn importacoes_multiplas_preservam_itens_existentes() {
        let mut s = Sheet::default();
        s.im = Some(r#"[{"id":"legado","foo":42}]"#.to_string());
        importar(&mut s, &png(), "nova.png", 0, 0).unwrap();
        let lista = itens(&s).unwrap();
        assert_eq!(lista.len(), 2);
        assert_eq!(lista[0]["foo"], 42);
        assert!(lista[1]["src"].as_str().unwrap().starts_with("data:image/png;base64,"));
    }

    #[test]
    fn jpeg_e_duas_imagens_tem_ids_proprios_e_persistencia() {
        let mut jpeg = Vec::new();
        let original = image::RgbImage::from_pixel(5, 2, image::Rgb([200, 40, 100]));
        image::DynamicImage::ImageRgb8(original)
            .write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        let mut w = Workbook::new();
        let s = &mut w.aba_mut(0).unwrap().sheet;
        let id1 = importar(s, &jpeg, "p.jpg", 8, 4).unwrap();
        let id2 = importar(s, &png(), "q.png", 8, 4).unwrap();
        assert_ne!(id1, id2);
        let lista = itens(s).unwrap();
        assert_eq!(lista.len(), 2);
        assert!(lista[0]["src"].as_str().unwrap().starts_with("data:image/jpeg;base64,"));
        assert_eq!(decodificar_src(lista[0]["src"].as_str().unwrap()), Some(jpeg));
        // Aba sem tamanhos próprios: a régua real é a padrão (122 × 28).
        assert_eq!(lista[0]["position"]["x"], 4. * 122. + 30.);
        assert_eq!(lista[0]["position"]["y"], 8. * 28. + 24.);
        assert_eq!(lista[0]["position"]["width"], 5.);
        assert_eq!(lista[0]["position"]["height"], 2.);
    }

    #[test]
    fn importacao_invalida_nao_suja_o_documento() {
        let mut s = Sheet::default();
        assert!(importar(&mut s, b"texto", "erro", 0, 0).is_err());
        assert!(s.im.is_none());
        s.im = Some("json quebrado".into());
        let antes = s.im.clone();
        assert!(importar(&mut s, &png(), "x", 0, 0).is_err());
        assert_eq!(s.im, antes);
    }
}
