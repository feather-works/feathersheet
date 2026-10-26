//! O pedaço do canvas do Iced 0.13 que a interface usava, sobre o `Renderer` do pelicano.
//! Existe para o desenho portado ficar linha a linha igual ao do `feathersheet-gui`:
//! mudança feita num lado se repete no outro sem retraduzir.
#![allow(dead_code)] // a API do Iced espelhada inteira: nem toda peça é usada fora dos testes.
use pelicano::{Background, Border, FillRule, Font, Point, Rect, Renderer, Size, Transform};
pub use pelicano::{Align, Color};

/// `iced::Vector`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vector {
    pub x: f32,
    pub y: f32,
}
impl Vector {
    pub const fn new(x: f32, y: f32) -> Self {
        Vector { x, y }
    }
}

/// `iced::Radians`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Radians(pub f32);

/// `iced::Pixels`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pixels(pub f32);
impl From<f32> for Pixels {
    fn from(v: f32) -> Self {
        Pixels(v)
    }
}
impl From<u16> for Pixels {
    fn from(v: u16) -> Self {
        Pixels(v as f32)
    }
}

/// `iced::Rectangle` (o `Rect` do pelicano com os métodos que faltam).
pub type Rectangle = Rect;
pub trait Retangulo {
    fn position(&self) -> Point;
    fn center(&self) -> Point;
    fn expand(&self, v: f32) -> Rect;
    fn intersects(&self, o: &Rect) -> bool;
    fn center_x(&self) -> f32;
    fn center_y(&self) -> f32;
    /// A parte comum, ou `None` se não se tocam.
    fn intersection(&self, o: &Rect) -> Option<Rect>;
}
impl Retangulo for Rect {
    fn position(&self) -> Point {
        Point::new(self.x, self.y)
    }
    fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
    fn expand(&self, v: f32) -> Rect {
        Rect { x: self.x - v, y: self.y - v, width: self.width + 2.0 * v, height: self.height + 2.0 * v }
    }
    fn intersects(&self, o: &Rect) -> bool {
        self.x < o.x + o.width && o.x < self.x + self.width && self.y < o.y + o.height && o.y < self.y + self.height
    }
    fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }
    fn center_y(&self) -> f32 {
        self.y + self.height / 2.0
    }
    fn intersection(&self, o: &Rect) -> Option<Rect> {
        let r = self.intersect(o);
        (r.width > 0.0 && r.height > 0.0).then_some(r)
    }
}

/// `canvas::Text`: `position` é o ponto de ancoragem segundo os dois alinhamentos.
#[derive(Debug, Clone)]
pub struct Text {
    pub content: String,
    pub position: Point,
    pub color: Color,
    pub size: Pixels,
    pub font: Font,
    pub horizontal_alignment: Align,
    pub vertical_alignment: Align,
    /// Altura da linha em múltiplos do tamanho.
    pub line_height: f32,
}
impl Default for Text {
    fn default() -> Self {
        Text {
            content: String::new(),
            position: Point::ORIGIN,
            color: Color::BLACK,
            size: Pixels(16.0),
            font: Font::DEFAULT,
            horizontal_alignment: Align::Start,
            vertical_alignment: Align::Start,
            line_height: 1.3,
        }
    }
}

/// `canvas::LineDash`.
#[derive(Debug, Clone, Copy, Default)]
pub struct LineDash<'a> {
    pub segments: &'a [f32],
    pub offset: usize,
}
pub use pelicano::{LineCap, LineJoin};

/// `canvas::Stroke`.
#[derive(Debug, Clone)]
pub struct Stroke<'a> {
    pub color: Color,
    pub width: f32,
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub line_dash: LineDash<'a>,
}
impl Default for Stroke<'_> {
    fn default() -> Self {
        Stroke { color: Color::BLACK, width: 1.0, line_cap: LineCap::Butt, line_join: LineJoin::Miter, line_dash: LineDash::default() }
    }
}
impl<'a> Stroke<'a> {
    pub fn with_color(self, color: Color) -> Self {
        Stroke { color, ..self }
    }
    pub fn with_width(self, width: f32) -> Self {
        Stroke { width, ..self }
    }
    pub fn with_line_cap(self, line_cap: LineCap) -> Self {
        Stroke { line_cap, ..self }
    }
    pub fn with_line_join(self, line_join: LineJoin) -> Self {
        Stroke { line_join, ..self }
    }
    fn pelicano(&self) -> pelicano::Stroke {
        let s = pelicano::Stroke::new(self.width).cap(self.line_cap).join(self.line_join);
        if self.line_dash.segments.is_empty() { s } else { s.dash(self.line_dash.segments.to_vec()) }
    }
}

/// `canvas::path::Arc`.
#[derive(Debug, Clone, Copy)]
pub struct Arc {
    pub center: Point,
    pub radius: f32,
    pub start_angle: Radians,
    pub end_angle: Radians,
}

/// `canvas::path::Builder`.
pub struct Builder(pelicano::PathBuilder);
impl Default for Builder {
    fn default() -> Self {
        Builder(pelicano::PathBuilder::new())
    }
}
impl Builder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn move_to(&mut self, p: Point) {
        self.0.move_to(p);
    }
    pub fn line_to(&mut self, p: Point) {
        self.0.line_to(p);
    }
    pub fn quadratic_curve_to(&mut self, c: Point, p: Point) {
        self.0.quadratic_to(c, p);
    }
    pub fn bezier_curve_to(&mut self, c1: Point, c2: Point, p: Point) {
        self.0.bezier_to(c1, c2, p);
    }
    pub fn close(&mut self) {
        self.0.close();
    }
    pub fn rectangle(&mut self, p: Point, s: Size) {
        self.0.rect(Rect::new(p, s));
    }
    pub fn circle(&mut self, c: Point, r: f32) {
        self.0.circle(c, r);
    }
    pub fn arc(&mut self, a: Arc) {
        self.0.arc(a.center, Point::new(a.radius, a.radius), a.start_angle.0, a.end_angle.0);
    }
    pub fn ellipse(&mut self, e: Elipse) {
        self.0.ellipse(e.center, Point::new(e.radii.x, e.radii.y));
    }
    pub fn rounded_rectangle(&mut self, p: Point, s: Size, raio: impl Into<Raio>) {
        let r = raio.into().0.min(s.width / 2.0).min(s.height / 2.0).max(0.0);
        if r <= 0.0 {
            return self.rectangle(p, s);
        }
        // Quarto de círculo por cúbica (k = 0,5523).
        let k = r * 0.552_284_8;
        let (x0, y0, x1, y1) = (p.x, p.y, p.x + s.width, p.y + s.height);
        let b = &mut self.0;
        b.move_to(Point::new(x0 + r, y0));
        b.line_to(Point::new(x1 - r, y0));
        b.bezier_to(Point::new(x1 - r + k, y0), Point::new(x1, y0 + r - k), Point::new(x1, y0 + r));
        b.line_to(Point::new(x1, y1 - r));
        b.bezier_to(Point::new(x1, y1 - r + k), Point::new(x1 - r + k, y1), Point::new(x1 - r, y1));
        b.line_to(Point::new(x0 + r, y1));
        b.bezier_to(Point::new(x0 + r - k, y1), Point::new(x0, y1 - r + k), Point::new(x0, y1 - r));
        b.line_to(Point::new(x0, y0 + r));
        b.bezier_to(Point::new(x0, y0 + r - k), Point::new(x0 + r - k, y0), Point::new(x0 + r, y0));
        b.close();
    }
    pub fn build(self) -> Path {
        Path(self.0.build())
    }
}

/// `canvas::path::arc::Elliptical` (só centro e raios).
#[derive(Debug, Clone, Copy)]
pub struct Elipse {
    pub center: Point,
    pub radii: Vector,
}

/// `border::Radius` de um valor só.
#[derive(Debug, Clone, Copy)]
pub struct Raio(pub f32);
impl From<f32> for Raio {
    fn from(v: f32) -> Self {
        Raio(v)
    }
}

/// `canvas::Path`: vazio quando a forma não fecha (o pelicano devolve `None`).
#[derive(Clone)]
pub struct Path(Option<pelicano::Path>);
impl Path {
    pub fn new(f: impl FnOnce(&mut Builder)) -> Path {
        let mut b = Builder::new();
        f(&mut b);
        b.build()
    }
    pub fn line(a: Point, b: Point) -> Path {
        Path(Some(pelicano::Path::line(a, b)))
    }
    pub fn rectangle(p: Point, s: Size) -> Path {
        Path(pelicano::Path::rect(Rect::new(p, s)))
    }
    pub fn rounded_rectangle(p: Point, s: Size, raio: impl Into<Raio>) -> Path {
        Path::new(|b| b.rounded_rectangle(p, s, raio))
    }
    pub fn circle(c: Point, r: f32) -> Path {
        Path(pelicano::Path::circle(c, r))
    }
}

/// `canvas::Frame`: desenha direto no `Renderer`, com a transformação e o recorte do Iced.
/// `base` é o canto do canvas na janela; `t` é a transformação dentro dele.
pub struct Frame<'r> {
    pub renderer: &'r mut Renderer,
    base: Point,
    tamanho: Size,
    t: Transform,
    /// Escala e deslocamento quando `t` não gira: aí retângulo e texto vão pelos caminhos rápidos.
    simples: Option<(f32, f32, f32)>,
}

impl<'r> Frame<'r> {
    /// O quadro de um canvas em `bounds` (coordenadas da janela), recortado nele.
    pub fn new(renderer: &'r mut Renderer, bounds: Rect) -> Self {
        renderer.push_clip(bounds, [0.0; 4]);
        Frame { renderer, base: Point::new(bounds.x, bounds.y), tamanho: bounds.size(), t: Transform::default(), simples: Some((1.0, 0.0, 0.0)) }
    }
    /// Fecha o recorte aberto em [`Frame::new`].
    pub fn fim(self) {
        self.renderer.pop_clip();
    }
    pub fn width(&self) -> f32 {
        self.tamanho.width
    }
    pub fn height(&self) -> f32 {
        self.tamanho.height
    }
    pub fn size(&self) -> Size {
        self.tamanho
    }
    pub fn center(&self) -> Point {
        Point::new(self.tamanho.width / 2.0, self.tamanho.height / 2.0)
    }
    fn total(&self) -> Transform {
        self.t.then(Transform::translate(self.base.x, self.base.y))
    }
    fn aplicar(&mut self, novo: Transform, simples: Option<(f32, f32, f32)>) {
        self.t = novo.then(self.t);
        self.simples = match (self.simples, simples) {
            (Some((s, x, y)), Some((s2, x2, y2))) => Some((s * s2, x + x2 * s, y + y2 * s)),
            _ => None,
        };
    }
    pub fn translate(&mut self, v: Vector) {
        self.aplicar(Transform::translate(v.x, v.y), Some((1.0, v.x, v.y)));
    }
    pub fn scale(&mut self, s: f32) {
        self.aplicar(Transform::scale(s, s), Some((s, 0.0, 0.0)));
    }
    pub fn rotate(&mut self, a: impl Into<Radians>) {
        let a = a.into().0;
        if a != 0.0 {
            self.aplicar(Transform::rotate(a.to_degrees()), None);
        }
    }
    pub fn with_save<R>(&mut self, f: impl FnOnce(&mut Frame) -> R) -> R {
        let (t, s) = (self.t, self.simples);
        let r = f(self);
        self.t = t;
        self.simples = s;
        r
    }
    /// Como no Iced: `region` vale sem a transformação atual, e dentro dela a origem é o canto.
    pub fn with_clip<R>(&mut self, region: Rect, f: impl FnOnce(&mut Frame) -> R) -> R {
        let base = Point::new(self.base.x + region.x, self.base.y + region.y);
        self.renderer.push_clip(Rect { x: base.x, y: base.y, width: region.width, height: region.height }, [0.0; 4]);
        let mut dentro = Frame { renderer: &mut *self.renderer, base, tamanho: region.size(), t: Transform::default(), simples: Some((1.0, 0.0, 0.0)) };
        let r = f(&mut dentro);
        self.renderer.pop_clip();
        r
    }
    pub fn fill_rectangle(&mut self, p: Point, s: Size, cor: impl Into<Background>) {
        if let Some((k, x, y)) = self.simples {
            let r = Rect { x: self.base.x + x + p.x * k, y: self.base.y + y + p.y * k, width: s.width * k, height: s.height * k };
            self.renderer.fill_quad(r, Some(&cor.into()), Border::default(), None);
        } else {
            self.fill(&Path::rectangle(p, s), cor);
        }
    }
    pub fn fill(&mut self, path: &Path, cor: impl Into<Background>) {
        self.fill_regra(path, cor, FillRule::NonZero);
    }
    pub fn fill_regra(&mut self, path: &Path, cor: impl Into<Background>, regra: FillRule) {
        let Some(p) = &path.0 else { return };
        self.renderer.push_transform(self.total());
        self.renderer.fill_path(p, cor, regra);
        self.renderer.pop_transform();
    }
    pub fn stroke(&mut self, path: &Path, traco: Stroke) {
        let Some(p) = &path.0 else { return };
        self.renderer.push_transform(self.total());
        self.renderer.stroke_path(p, &traco.pelicano(), traco.color);
        self.renderer.pop_transform();
    }
    pub fn fill_text(&mut self, texto: impl Into<Text>) {
        let t = texto.into();
        if t.content.is_empty() {
            return;
        }
        let tam = t.size.0;
        let medida = self.renderer.measure(&t.content, tam, t.font, None);
        // O Iced alinha cada linha; o pelicano alinha o bloco. Centro/direita: uma linha por vez.
        if t.horizontal_alignment != Align::Start && t.content.contains('\n') {
            let n = t.content.split('\n').count() as f32;
            let alto = self.renderer.measure("Ág", tam, t.font, None).height;
            let topo = t.position.y - match t.vertical_alignment {
                Align::Start => 0.0,
                Align::Center => alto * n / 2.0,
                Align::End => alto * n,
            };
            for (i, linha) in t.content.split('\n').enumerate() {
                self.fill_text(Text {
                    content: linha.to_owned(),
                    position: Point::new(t.position.x, topo + i as f32 * alto),
                    vertical_alignment: Align::Start,
                    ..t.clone()
                });
            }
            return;
        }
        let dx = match t.horizontal_alignment {
            Align::Start => 0.0,
            Align::Center => -medida.width / 2.0,
            Align::End => -medida.width,
        };
        let dy = match t.vertical_alignment {
            Align::Start => 0.0,
            Align::Center => -medida.height / 2.0,
            Align::End => -medida.height,
        };
        if let Some((k, x, y)) = self.simples {
            let at = Point::new(self.base.x + x + (t.position.x + dx) * k, self.base.y + y + (t.position.y + dy) * k);
            self.renderer.draw_text_lh(&t.content, tam * k, t.font, t.color, at, None, t.line_height);
        } else {
            self.renderer.push_transform(self.total());
            self.renderer.fill_text(&t.content, tam, t.font, t.color, Point::new(t.position.x + dx, t.position.y + dy));
            self.renderer.pop_transform();
        }
    }
    pub fn draw_image(&mut self, r: Rect, img: &pelicano::Image) {
        if let Some((k, x, y)) = self.simples {
            let r = Rect { x: self.base.x + x + r.x * k, y: self.base.y + y + r.y * k, width: r.width * k, height: r.height * k };
            self.renderer.draw_image(img, r, [0.0; 4]);
        } else {
            self.renderer.push_transform(self.total());
            self.renderer.draw_image(img, r, [0.0; 4]);
            self.renderer.pop_transform();
        }
    }
    pub fn draw_svg(&mut self, r: Rect, svg: &pelicano::Svg, cor: Option<Color>) {
        if let Some((k, x, y)) = self.simples {
            let r = Rect { x: self.base.x + x + r.x * k, y: self.base.y + y + r.y * k, width: r.width * k, height: r.height * k };
            self.renderer.draw_svg(svg, r, cor);
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn recorte_poe_a_origem_no_canto_e_ignora_a_escala_de_fora() {
        let mut r = Renderer::new();
        let mut f = Frame::new(&mut r, Rect { x: 10.0, y: 20.0, width: 100.0, height: 100.0 });
        f.scale(2.0);
        f.translate(Vector::new(3.0, 4.0));
        assert_eq!(f.simples, Some((2.0, 6.0, 8.0)));
        f.with_clip(Rect { x: 5.0, y: 5.0, width: 10.0, height: 10.0 }, |d| {
            assert_eq!(d.base, Point::new(15.0, 25.0));
            assert_eq!(d.simples, Some((1.0, 0.0, 0.0)));
        });
        f.rotate(Radians(0.5));
        assert!(f.simples.is_none());
        f.with_save(|g| g.scale(3.0));
        assert!(f.simples.is_none());
        f.fim();
    }
}

/// `iced::keyboard`: a tecla e os modificadores são os do winit (os mesmos nomes).
pub mod keyboard {
    pub use pelicano::{Key, Modifiers};
    pub mod key {
        pub use pelicano::NamedKey as Named;
    }
    /// Os métodos e constantes do Iced sobre o `ModifiersState` do winit.
    pub trait Modificadores {
        const CTRL: Modifiers = Modifiers::CONTROL;
        const COMMAND: Modifiers = Modifiers::CONTROL;
        const LOGO: Modifiers = Modifiers::SUPER;
        fn shift(&self) -> bool;
        fn control(&self) -> bool;
        fn alt(&self) -> bool;
        fn logo(&self) -> bool;
        /// Ctrl (no macOS seria a tecla Command; a suíte roda no Linux e no Windows).
        fn command(&self) -> bool;
    }
    impl Modificadores for Modifiers {
        fn shift(&self) -> bool {
            self.shift_key()
        }
        fn control(&self) -> bool {
            self.control_key()
        }
        fn alt(&self) -> bool {
            self.alt_key()
        }
        fn logo(&self) -> bool {
            self.super_key()
        }
        fn command(&self) -> bool {
            self.control_key()
        }
    }
}

/// `iced::mouse`: botão e ícone do cursor do winit.
pub mod mouse {
    pub use pelicano::{CursorIcon as Interaction, MouseButton as Button};
}

/// `iced::window`: o id é do pelicano (o app escolhe o número).
pub mod window {
    pub use pelicano::raw_window_handle;
    pub use pelicano::WindowId as Id;
    /// Ids das janelas abertas fora do `feather_ui::janelas` (que conta a partir do 1).
    pub fn novo_id() -> Id {
        static PROXIMO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1 << 32);
        Id(PROXIMO.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}

/// `iced::widget::canvas`: o quadro acima e o `Program` do pelicano.
#[allow(unused_imports)]
pub mod canvas {
    pub use super::{Arc, Builder, Elipse, Frame, LineCap, LineDash, LineJoin, Path, Raio, Stroke, Text, Vector};
    pub use pelicano::{Event, FillRule, Image, Program};
    pub mod path {
        pub use super::super::{Arc, Builder};
        pub mod arc {
            pub use super::super::super::Elipse as Elliptical;
        }
    }
}
/// O widget `canvas(programa)` do Iced.
pub fn canvas<P>(programa: P) -> pelicano::ProgramCanvas<P> {
    pelicano::program(programa)
}

/// `iced::alignment`: os dois eixos usam o mesmo `Align` no pelicano.
pub mod alignment {
    pub use pelicano::Align as Horizontal;
    pub use pelicano::Align as Vertical;
}

/// `iced::Point::distance`.
pub trait Distancia {
    fn distance(&self, o: Point) -> f32;
}
impl Distancia for Point {
    fn distance(&self, o: Point) -> f32 {
        (self.x - o.x).hypot(self.y - o.y)
    }
}

/// `canvas::Cache` sem nada dentro: o pelicano já só repinta o que mudou, e
/// `clear()` fica para quem pedia redesenho (`App::update`) continuar igual.
#[derive(Debug, Default)]
pub struct CacheCanvas;
impl CacheCanvas {
    pub fn clear(&self) {}
}

/// `pick_list(vec, ..)` do Iced, que aceitava a lista por valor: o do pelicano só empresta
/// `&'a [T]`, e a lista montada dentro da `view` não vive tanto. Este guarda a lista junto.
pub struct ListaPropria<'a, T, M> {
    lista: std::mem::ManuallyDrop<pelicano::PickList<'a, T, M>>,
    // Dono das opções, por ponteiro cru (um `Box` movido invalidaria o empréstimo da `lista`).
    dono: std::ptr::NonNull<[T]>,
}
pub fn pick_list_vec<'a, T: Clone + PartialEq + ToString + 'a, M>(
    opcoes: Vec<T>,
    selecionado: Option<T>,
    escolher: impl Fn(T) -> M + 'a,
) -> ListaPropria<'a, T, M> {
    let dono = std::ptr::NonNull::from(Box::leak(opcoes.into_boxed_slice()));
    // SEGURANÇA: a memória só é solta no `drop`, depois da `lista` (única que a empresta).
    let fatia: &'a [T] = unsafe { dono.as_ref() };
    ListaPropria { lista: std::mem::ManuallyDrop::new(pelicano::pick_list(fatia, selecionado, escolher)), dono }
}
impl<T, M> Drop for ListaPropria<'_, T, M> {
    fn drop(&mut self) {
        // SEGURANÇA: a `lista` cai primeiro e não é usada de novo; `dono` veio de um `Box`.
        unsafe {
            std::mem::ManuallyDrop::drop(&mut self.lista);
            drop(Box::from_raw(self.dono.as_ptr()));
        }
    }
}
impl<'a, T: Clone + PartialEq + ToString, M> ListaPropria<'a, T, M> {
    fn trocar(mut self, f: impl FnOnce(pelicano::PickList<'a, T, M>) -> pelicano::PickList<'a, T, M>) -> Self {
        // SEGURANÇA: a `lista` tirada volta logo, antes de qualquer `drop`.
        let l = unsafe { std::mem::ManuallyDrop::take(&mut self.lista) };
        self.lista = std::mem::ManuallyDrop::new(f(l));
        self
    }
    pub fn size(self, s: impl Into<pelicano::Num>) -> Self {
        self.trocar(|l| l.size(s))
    }
    pub fn padding(self, p: impl Into<pelicano::Padding>) -> Self {
        self.trocar(|l| l.padding(p))
    }
    pub fn width(self, w: impl Into<pelicano::Length>) -> Self {
        self.trocar(|l| l.width(w))
    }
}
impl<'a, T: Clone + PartialEq + ToString + 'a, M> pelicano::Widget<M> for ListaPropria<'a, T, M> {
    fn size(&self) -> (pelicano::Length, pelicano::Length) {
        pelicano::Widget::<M>::size(&*self.lista)
    }
    fn layout(&self, cx: &mut pelicano::LayoutCx, l: pelicano::Limits) -> pelicano::Node {
        pelicano::Widget::<M>::layout(&*self.lista, cx, l)
    }
    fn draw(&self, n: &pelicano::Node, cx: &mut pelicano::DrawCx) {
        pelicano::Widget::<M>::draw(&*self.lista, n, cx)
    }
    fn event(&self, n: &pelicano::Node, ev: &pelicano::Event, cx: &mut pelicano::EventCx<M>) -> pelicano::Captured {
        pelicano::Widget::<M>::event(&*self.lista, n, ev, cx)
    }
    fn opaque_at(&self, n: &pelicano::Node, p: Point) -> bool {
        pelicano::Widget::<M>::opaque_at(&*self.lista, n, p)
    }
}
impl<'a, T: Clone + PartialEq + ToString + 'a, M: 'a> From<ListaPropria<'a, T, M>> for pelicano::Element<'a, M> {
    fn from(w: ListaPropria<'a, T, M>) -> Self {
        pelicano::Element::new(w)
    }
}

/// `image::Handle::from_bytes` do Iced: decodifica qualquer formato do crate `image`
/// (PNG, JPEG, GIF, BMP, WebP); `None` se não decodifica.
pub fn imagem_de_bytes(bytes: &[u8]) -> Option<pelicano::Image> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    pelicano::Image::from_rgba(img.width(), img.height(), img.into_raw())
}

/// O `on_move` da `mouse_area` do Iced: o ponto RELATIVO ao canto do widget
/// (o `on_move` do pelicano dá o ponto na janela). Não pega o evento.
pub struct MoverRelativo<'a, M> {
    conteudo: pelicano::Element<'a, M>,
    f: Box<dyn Fn(Point) -> M + 'a>,
}
pub fn ao_mover_relativo<'a, M: 'a>(conteudo: impl Into<pelicano::Element<'a, M>>, f: impl Fn(Point) -> M + 'a) -> MoverRelativo<'a, M> {
    MoverRelativo { conteudo: conteudo.into(), f: Box::new(f) }
}
impl<M> pelicano::Widget<M> for MoverRelativo<'_, M> {
    fn size(&self) -> (pelicano::Length, pelicano::Length) {
        self.conteudo.widget().size()
    }
    fn layout(&self, cx: &mut pelicano::LayoutCx, l: pelicano::Limits) -> pelicano::Node {
        let n = cx.child(0, &self.conteudo, l);
        pelicano::Node { rect: n.rect, key: cx.key(), children: vec![n], ..Default::default() }
    }
    fn draw(&self, n: &pelicano::Node, cx: &mut pelicano::DrawCx) {
        self.conteudo.widget().draw(&n.children[0], cx)
    }
    fn event(&self, n: &pelicano::Node, ev: &pelicano::Event, cx: &mut pelicano::EventCx<M>) -> pelicano::Captured {
        if let pelicano::Event::CursorMoved(p) = ev {
            if n.rect.contains(*p) {
                cx.publish((self.f)(Point::new(p.x - n.rect.x, p.y - n.rect.y)));
            }
        }
        self.conteudo.widget().event(&n.children[0], ev, cx)
    }
    fn opaque_at(&self, n: &pelicano::Node, p: Point) -> bool {
        self.conteudo.widget().opaque_at(&n.children[0], p)
    }
}
impl<'a, M: 'a> From<MoverRelativo<'a, M>> for pelicano::Element<'a, M> {
    fn from(w: MoverRelativo<'a, M>) -> Self {
        pelicano::Element::new(w)
    }
}

/// `feather_ui::tema::estilo_menu_da_faixa` do Iced (o feather-ui v0.3 não tem): campo legível com
/// realce de foco. A seta azul vai no `.handle_color(COR_AZUL_ACENTO())` de quem usa.
pub fn estilo_menu_da_faixa(_: &pelicano::Theme, status: pelicano::Status) -> pelicano::Style {
    use feather_ui::tema::{COR_AZUL_FUNDO, COR_BORDA, COR_FOCO, COR_PAINEL, COR_TEXTO};
    let sobre = matches!(status, pelicano::Status::Hovered | pelicano::Status::Pressed);
    pelicano::Style::default()
        .background(if sobre { COR_AZUL_FUNDO() } else { COR_PAINEL() })
        .text_color(COR_TEXTO())
        .border(Border::rounded(5.0).color(if sobre { COR_FOCO(COR_AZUL_FUNDO()) } else { COR_BORDA() }, 1.0))
}

/// `iced::Radians` a partir de `f32` (o `frame.rotate(graus.to_radians())` do Iced).
impl From<f32> for Radians {
    fn from(v: f32) -> Self {
        Radians(v)
    }
}

/// O `button` do Iced 0.13: o conteúdo fica no canto de cima à esquerda (o do pelicano centraliza).
/// Num botão mais largo ou mais alto que o conteúdo, ele vai num container que ocupa a sobra.
pub struct Botao<'a, M> {
    conteudo: pelicano::Element<'a, M>,
    on_press: Option<M>,
    padding: pelicano::Padding,
    width: pelicano::Length,
    height: pelicano::Length,
    estilo: Option<Box<dyn Fn(&pelicano::Theme, pelicano::Status) -> pelicano::Style + 'a>>,
}
pub fn button<'a, M: 'a>(conteudo: impl Into<pelicano::Element<'a, M>>) -> Botao<'a, M> {
    Botao {
        conteudo: conteudo.into(),
        on_press: None,
        // O recuo padrão do Iced 0.13 (o do pelicano é 6×12).
        padding: [5u16, 10].into(),
        width: pelicano::Length::Shrink,
        height: pelicano::Length::Shrink,
        estilo: None,
    }
}
impl<'a, M: 'a> Botao<'a, M> {
    pub fn on_press(mut self, m: M) -> Self {
        self.on_press = Some(m);
        self
    }
    pub fn on_press_maybe(mut self, m: Option<M>) -> Self {
        self.on_press = m;
        self
    }
    pub fn padding(mut self, p: impl Into<pelicano::Padding>) -> Self {
        self.padding = p.into();
        self
    }
    pub fn width(mut self, w: impl Into<pelicano::Length>) -> Self {
        self.width = w.into();
        self
    }
    pub fn height(mut self, h: impl Into<pelicano::Length>) -> Self {
        self.height = h.into();
        self
    }
    pub fn style(mut self, f: impl Fn(&pelicano::Theme, pelicano::Status) -> pelicano::Style + 'a) -> Self {
        self.estilo = Some(Box::new(f));
        self
    }
}
impl<'a, M: Clone + 'a> From<Botao<'a, M>> for pelicano::Element<'a, M> {
    fn from(b: Botao<'a, M>) -> Self {
        use pelicano::Length::Shrink;
        let sobra = |l| if l == Shrink { Shrink } else { pelicano::Length::FILL };
        let conteudo: pelicano::Element<'a, M> = if b.width == Shrink && b.height == Shrink {
            b.conteudo
        } else {
            pelicano::container(b.conteudo).width(sobra(b.width)).height(sobra(b.height)).into()
        };
        let mut p = pelicano::button(conteudo).on_press_maybe(b.on_press)
            .padding(b.padding).width(b.width).height(b.height);
        if let Some(f) = b.estilo {
            p = p.style(f);
        }
        p.into()
    }
}
