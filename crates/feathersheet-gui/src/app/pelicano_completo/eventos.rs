use super::*;

pub(super) struct Clipboard<'a> {
    source: &'a mut dyn p::Clipboard,
    lido: Option<String>,
    primario:&'a RefCell<Option<arboard::Clipboard>>,
}
impl<'a> Clipboard<'a> {
    pub fn new(source: &'a mut dyn p::Clipboard, ler: bool,primario:&'a RefCell<Option<arboard::Clipboard>>) -> Self {
        let lido = if ler { source.read() } else { None };
        Self { source, lido,primario }
    }
}
impl iced_runtime::core::Clipboard for Clipboard<'_> {
    fn read(&self, kind: iced_runtime::core::clipboard::Kind) -> Option<String> {
        match kind {iced_runtime::core::clipboard::Kind::Standard=>self.lido.clone(),
            iced_runtime::core::clipboard::Kind::Primary=>ler_primario(self.primario)}
    }
    fn write(&mut self, kind: iced_runtime::core::clipboard::Kind, contents: String) {
        if matches!(kind,iced_runtime::core::clipboard::Kind::Primary) {gravar_primario(self.primario,contents);return;}
        self.lido = Some(contents.clone());
        self.source.write(contents);
    }
}

pub(super) fn ler_clipboard(ev:&p::Event)->bool {
    match ev {
        p::Event::Key{key:p::Key::Character(c),modifiers,pressed:true,..}=>modifiers.control_key() && c.eq_ignore_ascii_case("v"),
        p::Event::Key{key:p::Key::Named(p::NamedKey::Paste),pressed:true,..}=>true,
        p::Event::Key{key:p::Key::Named(p::NamedKey::Insert),modifiers,pressed:true,..}=>modifiers.shift_key(),
        _=>false,
    }
}

pub(super) fn ler_primario(clipboard:&RefCell<Option<arboard::Clipboard>>)->Option<String> {
    #[cfg(target_os="linux")]
    {
        use arboard::GetExtLinux;
        let mut c=clipboard.borrow_mut();
        if c.is_none(){*c=arboard::Clipboard::new().ok();}
        c.as_mut()?.get().clipboard(arboard::LinuxClipboardKind::Primary).text().ok()
    }
    #[cfg(not(target_os="linux"))]
    {let _=clipboard;None}
}

pub(super) fn gravar_primario(clipboard:&RefCell<Option<arboard::Clipboard>>,texto:String) {
    #[cfg(target_os="linux")]
    {
        use arboard::SetExtLinux;
        let mut c=clipboard.borrow_mut();
        if c.is_none(){*c=arboard::Clipboard::new().ok();}
        if let Some(c)=c.as_mut(){let _=c.set().clipboard(arboard::LinuxClipboardKind::Primary).text(texto);}
    }
    #[cfg(not(target_os="linux"))]
    {let _=(clipboard,texto);}
}

fn mods(m: p::Modifiers) -> keyboard::Modifiers {
    let mut v = keyboard::Modifiers::empty();
    v.set(keyboard::Modifiers::SHIFT, m.shift_key());
    v.set(keyboard::Modifiers::CTRL, m.control_key());
    v.set(keyboard::Modifiers::ALT, m.alt_key());
    v.set(keyboard::Modifiers::LOGO, m.super_key());
    v
}

mod nomes;
fn tecla(key:&p::Key)->keyboard::Key {
    match key {
        p::Key::Character(c)=>keyboard::Key::Character(c.as_str().into()),
        p::Key::Named(n)=>nomes::converter(*n).map(keyboard::Key::Named).unwrap_or(keyboard::Key::Unidentified),
        _=>keyboard::Key::Unidentified,
    }
}

fn botao(b: p::MouseButton) -> mouse::Button {
    match b {
        p::MouseButton::Left => mouse::Button::Left,
        p::MouseButton::Right => mouse::Button::Right,
        p::MouseButton::Middle => mouse::Button::Middle,
        p::MouseButton::Back => mouse::Button::Back,
        p::MouseButton::Forward => mouse::Button::Forward,
        p::MouseButton::Other(n) => mouse::Button::Other(n),
    }
}

pub(super) fn converter(ev: &p::Event, b: p::Rect) -> Option<iced::Event> {
    use iced::window::Event as W;
    use iced::Event as E;
    let point = |p: &p::Point| iced::Point::new(p.x - b.x, p.y - b.y);
    Some(match ev {
        p::Event::CursorMoved(p) => E::Mouse(mouse::Event::CursorMoved { position: point(p) }),
        p::Event::CursorLeft => E::Mouse(mouse::Event::CursorLeft),
        p::Event::MousePressed(b) => E::Mouse(mouse::Event::ButtonPressed(botao(*b))),
        p::Event::MouseReleased(b) => E::Mouse(mouse::Event::ButtonReleased(botao(*b))),
        p::Event::Wheel { dx, dy, .. } => E::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x: *dx, y: *dy },
        }),
        p::Event::ModifiersChanged(m) => E::Keyboard(keyboard::Event::ModifiersChanged(mods(*m))),
        p::Event::Key {
            key,
            text,
            modifiers,
            pressed,
        } => {
            let key = tecla(key);
            let modifiers = mods(*modifiers);
            E::Keyboard(if *pressed {
                keyboard::Event::KeyPressed {
                    modified_key: key.clone(),
                    key,
                    physical_key: keyboard::key::Physical::Unidentified(
                        keyboard::key::NativeCode::Unidentified,
                    ),
                    location: keyboard::Location::Standard,
                    modifiers,
                    text: text.as_deref().map(Into::into),
                }
            } else {
                keyboard::Event::KeyReleased {
                    key,
                    location: keyboard::Location::Standard,
                    modifiers,
                }
            })
        }
        p::Event::Resized(s) => E::Window(W::Resized(iced::Size::new(s.width, s.height))),
        p::Event::Moved(p) => E::Window(W::Moved(iced::Point::new(p.x, p.y))),
        p::Event::Focused(true) => E::Window(W::Focused),
        p::Event::Focused(false) => E::Window(W::Unfocused),
        p::Event::CloseRequested => E::Window(W::CloseRequested),
        p::Event::FileDropped(p) => E::Window(W::FileDropped(p.clone())),
        p::Event::FileHovered(p) => E::Window(W::FileHovered(p.clone())),
        p::Event::FileHoverCancelled => E::Window(W::FilesHoveredLeft),
        p::Event::FingerPressed { id, position } => E::Touch(iced::touch::Event::FingerPressed {
            id: iced::touch::Finger(*id),
            position: point(position),
        }),
        p::Event::FingerMoved { id, position } => E::Touch(iced::touch::Event::FingerMoved {
            id: iced::touch::Finger(*id),
            position: point(position),
        }),
        p::Event::FingerLifted { id, position } => E::Touch(iced::touch::Event::FingerLifted {
            id: iced::touch::Finger(*id),
            position: point(position),
        }),
        p::Event::FingerLost { id, position } => E::Touch(iced::touch::Event::FingerLost {
            id: iced::touch::Finger(*id),
            position: point(position),
        }),
        _ => return None,
    })
}

pub(super) fn cursor(i: mouse::Interaction) -> p::CursorIcon {
    use mouse::Interaction as I;
    use p::CursorIcon as C;
    match i {
        I::Pointer => C::Pointer,
        I::Grab => C::Grab,
        I::Grabbing => C::Grabbing,
        I::Text => C::Text,
        I::Crosshair => C::Crosshair,
        I::Working => C::Progress,
        I::ResizingHorizontally => C::EwResize,
        I::ResizingVertically => C::NsResize,
        I::ResizingDiagonallyUp => C::NeswResize,
        I::ResizingDiagonallyDown => C::NwseResize,
        I::NotAllowed => C::NotAllowed,
        I::ZoomIn => C::ZoomIn,
        I::ZoomOut => C::ZoomOut,
        I::Cell => C::Cell,
        I::Move => C::Move,
        I::Copy => C::Copy,
        I::Help => C::Help,
        _ => C::Default,
    }
}
