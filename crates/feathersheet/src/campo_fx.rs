//! Adaptador fino do TextInput do pelicano para observar o caret real.
 //! Não redesenha nem reimplementa o editor; delega todos os eventos ao pelicano.
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use pelicano::{
    Captured, DrawCx, Element, Event, EventCx, Key, LayoutCx, Length, Limits, MouseButton, NamedKey,
    Node, Renderer, TextInput, Widget,
};
use crate::app::Msg;

/// Tamanho do texto e recuo esquerdo do campo fx (`interface_editor.rs`: tamanho padrão, `padding(8)`).
const TAMANHO: f32 = 14.0;
const RECUO: f32 = 8.0;

/// Pedido de [`desfocar`], atendido pelo campo no próximo evento.
static DESFOCAR: AtomicBool = AtomicBool::new(false);

/// Pedido de [`posicionar`] (índice em caracteres; `usize::MAX` = nenhum).
static CURSOR: AtomicUsize = AtomicUsize::new(usize::MAX);

/// Onde o último clique no campo começou (índice em caracteres).
#[derive(Default)]
struct InicioClique(usize);

pub struct CampoFx<'a> {
    input: TextInput<'a,Msg>,
    texto: &'a str,
}
impl<'a> CampoFx<'a> {
    pub fn new(input:TextInput<'a,Msg>,texto:&'a str)->Self {
        Self {input,texto}
    }
    /// Caractere sob `x` (relativo ao começo do texto), como o TextInput acha o seu.
    // shortcut: o pelicano não expõe o cursor do TextInput; o índice é recalculado aqui sem a
    // rolagem do texto longo (clique num texto rolado erra), até o pelicano expor o cursor.
    fn indice_em(&self, r: &mut Renderer, x: f32) -> usize {
        let carets = r.carets(self.texto, TAMANHO, pelicano::Font::DEFAULT);
        let byte = carets.iter().find(|g| x < (g.1 + g.2) / 2.0).map_or(self.texto.len(), |g| g.0);
        self.texto.get(..byte).map_or(self.texto.chars().count(), |s| s.chars().count())
    }
}
impl Widget<Msg> for CampoFx<'_> {
    fn size(&self)->(Length,Length){Widget::size(&self.input)}
    fn layout(&self,cx:&mut LayoutCx,limits:Limits)->Node {
        Widget::layout(&self.input,cx,limits)
    }
    fn draw(&self,n:&Node,cx:&mut DrawCx) {
        Widget::draw(&self.input,n,cx);
    }
    fn event(&self,n:&Node,evento:&Event,cx:&mut EventCx<Msg>)->Captured {
        if DESFOCAR.swap(false, Ordering::Relaxed) && cx.states.focus == Some(n.key) {
            cx.states.focus = None;
            cx.redraw = true;
        }
        // Enter com Alt é uma quebra de linha, não um submit. O evento
        // global de teclado inicia a edição multilinha na célula ativa.
        if let Event::Key { key: Key::Named(NamedKey::Enter), modifiers, pressed: true, .. } = evento {
            if modifiers.alt_key() && cx.states.focus == Some(n.key) { return Captured::Yes; }
        }
        // O pelicano não move o cursor do TextInput por Task: o campo leva o
        // caret até lá com Home + → antes de o TextInput ver o evento.
        if cx.states.focus == Some(n.key) {
            let pos = CURSOR.swap(usize::MAX, Ordering::Relaxed);
            if pos != usize::MAX {
                let tecla = |k| Event::Key { key: Key::Named(k), text: None, modifiers: Default::default(), pressed: true };
                Widget::event(&self.input, n, &tecla(NamedKey::Home), cx);
                for _ in 0..pos { Widget::event(&self.input, n, &tecla(NamedKey::ArrowRight), cx); }
                cx.redraw = true;
            }
        }
        let dentro=cx.cursor.is_some_and(|p| n.rect.contains(p));
        let indice = match evento {
            Event::MousePressed(MouseButton::Left) | Event::MouseReleased(MouseButton::Left) if dentro => {
                let x = cx.cursor.map_or(0.0, |p| p.x) - n.rect.x - RECUO;
                Some(self.indice_em(cx.renderer, x))
            }
            _ => None,
        };
        let resultado=Widget::event(&self.input,n,evento,cx);
        if let Some(i) = indice {
            // Estado do campo depois do click/arrasto do mouse.
            if matches!(evento, Event::MousePressed(_)) {
                cx.states.get_mut::<InicioClique>(n.key).0 = i;
                cx.publish(Msg::PosicionarCursorFx(i));
            } else {
                let inicio = cx.states.get::<InicioClique>(n.key).map_or(i, |c| c.0);
                if inicio == i { cx.publish(Msg::PosicionarCursorFx(i)); }
                else { cx.publish(Msg::SelecionarTextoFx(inicio.min(i),inicio.max(i))); }
            }
        }
        resultado
    }
}
/// Remove o foco do campo fx, devolvendo os atalhos de teclado à grade.
///
/// O pelicano não tem `Task` para tirar o foco: o campo atende o pedido
/// no próximo evento que passar por ele (antes de o TextInput ver a tecla).
pub fn desfocar() -> pelicano::Task<Msg> {
    DESFOCAR.store(true, Ordering::Relaxed);
    pelicano::Task::none()
}

/// Põe o caret do campo fx no caractere `pos` (depois do foco, no próximo evento).
pub fn posicionar(pos: usize) -> pelicano::Task<Msg> {
    CURSOR.store(pos, Ordering::Relaxed);
    pelicano::Task::focus("feathersheet.fx")
}

/// Transfere o foco do campo fx para o editor multilinha (o foco é um só:
/// o fx perde o dele).
pub fn focar_multilinha() -> pelicano::Task<Msg> {
    pelicano::Task::focus("feathersheet.multilinha")
}

impl<'a> From<CampoFx<'a>> for Element<'a,Msg> {
    fn from(campo:CampoFx<'a>)->Self {Element::new(campo)}
}
