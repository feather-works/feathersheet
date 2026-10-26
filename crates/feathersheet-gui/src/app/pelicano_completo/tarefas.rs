use super::*;
use iced_runtime::{clipboard, window::Action as W, Action};

pub(super) fn converter(tarefa: iced::Task<Comando>) -> p::Task<Msg> {
    iced_runtime::task::into_stream(tarefa).map_or_else(p::Task::none, |s| {
        p::Task::run(s, |a| Msg::Acao(Arc::new(Mutex::new(Some(a)))))
    })
}

impl Interface {
    pub(super) fn acao(&mut self, acao: Action<Comando>) -> p::Task<Msg> {
        match acao {
            Action::Output(m) => self.update(Msg::Comando(m)),
            Action::Clipboard(clipboard::Action::Write { contents, .. }) => {
                p::Task::write_clipboard(contents)
            }
            Action::Clipboard(clipboard::Action::Read { channel, .. }) => {
                p::Task::read_clipboard(move |s| {
                    let _ = channel.send(s);
                    Msg::Ocioso
                })
            }
            Action::LoadFont { bytes, channel } => {
                iced_tiny_skia::graphics::text::font_system()
                    .write()
                    .expect("fontes disponíveis")
                    .load_font(bytes);
                let _ = channel.send(Ok(()));
                p::Task::none()
            }
            Action::Widget(mut operation) => {
                loop {
                    for estado in self.janelas.values() {
                        let mut estado = estado.borrow_mut();
                        let mut ui = estado.construir(&self.app);
                        ui.operate(&estado.renderer, operation.as_mut());
                        estado.cache = ui.into_cache();
                        estado.suja = true;
                    }
                    match operation.finish() {
                        iced_runtime::core::widget::operation::Outcome::Chain(next) => {
                            operation = next
                        }
                        _ => break,
                    }
                }
                p::Task::none()
            }
            Action::Window(W::Open(id, s, channel)) => {
                if self.pid(id).is_some() {
                    let _ = channel.send(id);
                    return p::Task::none();
                }
                let pid = p::WindowId(self.proxima_janela);
                self.proxima_janela += 1;
                self.janelas
                    .insert(pid, Rc::new(RefCell::new(EstadoJanela::nova(id, s.size))));
                let settings = p::Settings {
                    title: self.app.titulo(id),
                    size: p::Size::new(s.size.width, s.size.height),
                    min_size: s.min_size.map(|s| p::Size::new(s.width, s.height)),
                    decorations: s.decorations,
                    transparent: s.transparent,
                    resizable: s.resizable,
                    close_on_request: false,
                    start_hidden: !s.visible,
                    position: match s.position {
                        window::Position::Specific(p) => Some(p::Point::new(p.x, p.y)),
                        _ => None,
                    },
                    app_id: Some("feathersheet".into()),
                    ..Default::default()
                };
                let _ = channel.send(id);
                self.publicar(
                    id,
                    iced::Event::Window(window::Event::Opened {
                        position: None,
                        size: s.size,
                    }),
                    iced::event::Status::Ignored,
                );
                p::Task::open_window(pid, settings)
            }
            Action::Window(W::GetOldest(c)) => {
                let _ = c.send(self.app.janelas.principal);
                p::Task::none()
            }
            Action::Window(W::GetLatest(c)) => {
                let id = self
                    .janelas
                    .iter()
                    .max_by_key(|(id, _)| id.0)
                    .map(|(_, s)| s.borrow().id);
                let _ = c.send(id);
                p::Task::none()
            }
            Action::Window(action) => self.janela(action),
            Action::Exit => p::Task::window(p::WindowCmd::Exit),
            Action::System(_) => {
                self.app.status = "Consulta de sistema indisponível nesta janela.".into();
                p::Task::none()
            }
        }
    }

    fn janela(&mut self, acao: W) -> p::Task<Msg> {
        match acao {
            W::Close(id) => {
                let Some(pid) = self.pid(id) else {
                    return p::Task::none();
                };
                self.publicar(
                    id,
                    iced::Event::Window(window::Event::Closed),
                    iced::event::Status::Ignored,
                );
                self.janelas.remove(&pid);
                p::Task::window_of(pid, p::WindowCmd::Close)
            }
            W::Drag(id) => self.comando_janela(id, p::WindowCmd::Drag),
            W::Resize(id, s) => {
                self.comando_janela(id, p::WindowCmd::Resize(p::Size::new(s.width, s.height)))
            }
            W::Maximize(id, v) => {
                if let Some(pid) = self.pid(id) {
                    self.janelas[&pid].borrow_mut().maximizada = v;
                }
                self.comando_janela(id, p::WindowCmd::Maximize(v))
            }
            W::ToggleMaximize(id) => {
                if let Some(pid) = self.pid(id) {
                    let mut s = self.janelas[&pid].borrow_mut();
                    s.maximizada = !s.maximizada;
                }
                self.comando_janela(id, p::WindowCmd::ToggleMaximize)
            }
            W::Minimize(id, v) => {
                if let Some(pid) = self.pid(id) {
                    self.janelas[&pid].borrow_mut().minimizada = v;
                }
                self.comando_janela(
                    id,
                    if v {
                        p::WindowCmd::Minimize
                    } else {
                        p::WindowCmd::Focus
                    },
                )
            }
            W::Move(id, pos) => {
                self.comando_janela(id, p::WindowCmd::Move(p::Point::new(pos.x, pos.y)))
            }
            W::GainFocus(id) => self.comando_janela(id, p::WindowCmd::Focus),
            W::ChangeMode(id, mode) => {
                if let Some(pid) = self.pid(id) {
                    self.janelas[&pid].borrow_mut().modo = mode;
                }
                self.comando_janela(
                    id,
                    p::WindowCmd::Fullscreen(mode == window::Mode::Fullscreen),
                )
            }
            W::ChangeLevel(id, nivel) => self.comando_janela(
                id,
                p::WindowCmd::Level(match nivel {
                    window::Level::AlwaysOnTop => p::WindowLevel::AlwaysOnTop,
                    window::Level::AlwaysOnBottom => p::WindowLevel::AlwaysOnBottom,
                    _ => p::WindowLevel::Normal,
                }),
            ),
            W::GetSize(id, c) => {
                if let Some(pid) = self.pid(id) {
                    let _ = c.send(self.janelas[&pid].borrow().tamanho);
                }
                p::Task::none()
            }
            W::GetPosition(id, c) => {
                let _ = c.send(
                    self.pid(id)
                        .and_then(|pid| self.janelas[&pid].borrow().posicao),
                );
                p::Task::none()
            }
            W::GetScaleFactor(id, c) => {
                if let Some(pid) = self.pid(id) {
                    let _ = c.send(self.janelas[&pid].borrow().escala as f32);
                }
                p::Task::none()
            }
            W::GetMaximized(id, c) => {
                if let Some(pid) = self.pid(id) {
                    let _ = c.send(self.janelas[&pid].borrow().maximizada);
                }
                p::Task::none()
            }
            W::GetMinimized(id, c) => {
                let _ = c.send(
                    self.pid(id)
                        .map(|pid| self.janelas[&pid].borrow().minimizada),
                );
                p::Task::none()
            }
            W::GetMode(id, c) => {
                if let Some(pid) = self.pid(id) {
                    let _ = c.send(self.janelas[&pid].borrow().modo);
                }
                p::Task::none()
            }
            W::RunWithHandle(id, f) => {
                let Some(pid) = self.pid(id) else {
                    return p::Task::none();
                };
                p::Task::window_handle(pid, move |handle| {
                    // SAFETY: Pelicano mantém a janela viva durante esta chamada na thread da janela.
                    f(unsafe { p::raw_window_handle::WindowHandle::borrow_raw(handle) });
                    Msg::Ocioso
                })
            }
            W::GetRawId(id, c) => {
                let Some(pid) = self.pid(id) else {
                    return p::Task::none();
                };
                p::Task::window_handle(pid, move |handle| {
                    let id = match handle {
                        p::raw_window_handle::RawWindowHandle::Xlib(h) => h.window,
                        p::raw_window_handle::RawWindowHandle::Xcb(h) => h.window.get() as u64,
                        _ => 0,
                    };
                    let _ = c.send(id);
                    Msg::Ocioso
                })
            }
            W::Screenshot(id, c) => {
                if let Some(pid) = self.pid(id) {
                    let mut s = self.janelas[&pid].borrow_mut();
                    let scale = s.escala;
                    s.desenhar(&self.app, self.revisao, scale, &self.sender, pid);
                    let size = iced::Size::new(
                        (s.tamanho.width as f64 * scale).ceil() as u32,
                        (s.tamanho.height as f64 * scale).ceil() as u32,
                    );
                    let _ = c.send(window::Screenshot::new(s.pixels.clone(), size, scale));
                }
                p::Task::none()
            }
            // Estas ações não são emitidas pelos comandos do FeatherSheet.
            W::RequestUserAttention(..)
            | W::ToggleDecorations(_)
            | W::ChangeIcon(..)
            | W::ShowSystemMenu(_)
            | W::EnableMousePassthrough(_)
            | W::DisableMousePassthrough(_) => p::Task::none(),
            W::Open(..) | W::GetOldest(_) | W::GetLatest(_) => {
                unreachable!("abertura tratada antes")
            }
        }
    }

    fn comando_janela(&self, id: window::Id, cmd: p::WindowCmd) -> p::Task<Msg> {
        self.pid(id)
            .map_or_else(p::Task::none, |pid| p::Task::window_of(pid, cmd))
    }
}
