//! Executa as tarefas dos comandos existentes no laço do Pelicano.
use super::*;
use iced_runtime::{clipboard, window::Action as Janela, Action};

pub(super) fn converter(tarefa: iced::Task<Comando>) -> Task<Msg> {
    iced_runtime::task::into_stream(tarefa).map_or_else(Task::none, |fluxo| {
        Task::run(fluxo, |a| Msg::Tarefa(Arc::new(Mutex::new(Some(a)))))
    })
}

impl Interface {
    pub(super) fn acao(&mut self, acao: Action<Comando>) -> Task<Msg> {
        match acao {
            // Resultados assíncronos continuam chegando mesmo com um modal bloqueando a interação.
            Action::Output(msg) => converter(self.app.update(msg)),
            Action::Clipboard(clipboard::Action::Write { contents, .. }) => {
                Task::write_clipboard(contents)
            }
            Action::Clipboard(clipboard::Action::Read { channel, .. }) => {
                Task::read_clipboard(move |s| {
                    let _ = channel.send(s);
                    Msg::Pulso
                })
            }
            Action::LoadFont { bytes, channel } => {
                p::load_font(bytes.into_owned());
                let _ = channel.send(Ok(()));
                Task::none()
            }
            Action::Exit => Task::window(p::WindowCmd::Exit),
            Action::Window(Janela::Close(_)) => Task::window(p::WindowCmd::Close),
            Action::Window(Janela::GainFocus(_)) => Task::window(p::WindowCmd::Focus),
            Action::Window(Janela::GetOldest(channel) | Janela::GetLatest(channel)) => {
                let _ = channel.send(self.app.janelas.principal);
                Task::none()
            }
            // shortcut: foco Iced não tem IDs nesta árvore, remover quando os comandos também forem migrados.
            Action::Widget(_) => Task::none(),
            _ => {
                self.app.status = "Este recurso ainda aguarda migração para Pelicano.".into();
                Task::none()
            }
        }
    }
}
