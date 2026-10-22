//! Atalhos locais da janela de macros, herdados do editor TypeScript.
//! Eles não são aplicados à janela principal da planilha.
use super::*;
use crate::compat::keyboard::key::Named; use crate::compat::keyboard::Key; use crate::compat::keyboard::Modifiers;

/// Tradução pura: não modifica a pasta nem depende do foco no text_editor.
/// Depuração (F8/F9/Shift+F5) não é ativada enquanto não houver motor equivalente.
pub(super) fn comando(key: &Key, mods: Modifiers) -> Option<Msg> {
    let combo = feather_ui::atalhos::combo_de(key, mods)?;
    match combo.as_str() {
        "F5" | "Ctrl+Enter" => Some(Msg::MacroExecutar),
        "F1" => Some(Msg::AjudaAbrirSecao("macros")),
        "Ctrl+S" => Some(Msg::MacroGuardarModulo),
        "Ctrl+Shift+S" => Some(Msg::MacroSalvar),
        "Ctrl+O" => Some(Msg::MacroAbrir),
        "Ctrl+G" => Some(Msg::MacroVisual(EventoVisual::Painel(0))),
        "Alt+Q" => Some(Msg::FecharAux(Auxiliar::Macros)),
        // Localizar/Substituir no código e desfazer do editor (codigo.rs).
        "Ctrl+F" => Some(codigo(EventoCodigo::Abrir(false))),
        "Ctrl+H" => Some(codigo(EventoCodigo::Abrir(true))),
        "F3" => Some(codigo(EventoCodigo::Proximo)),
        "Shift+F3" => Some(codigo(EventoCodigo::Anterior)),
        "Ctrl+Z" => Some(codigo(EventoCodigo::Desfazer)),
        "Ctrl+Y" | "Ctrl+Shift+Z" => Some(codigo(EventoCodigo::Refazer)),
        // Em primeiro lugar fechar menus abertos, depois fechar a janela.
        // Escape é resolvido em App::atalho_macro por depender do estado visual.
        "Escape" => None,
        _ => None,
    }
}

fn codigo(e: EventoCodigo) -> Msg { Msg::MacroVisual(EventoVisual::Codigo(e)) }

/// Interceptar somente teclas reconhecidas, mesmo se text_editor as consumir.
/// As ações continuam restritas à janela Auxiliar::Macros no App::update.
pub(in crate::app) fn deve_ouvir_capturada(key: &Key, mods: Modifiers) -> bool {
    comando(key, mods).is_some()
        || (matches!(key, Key::Named(Named::Escape)) && mods == Modifiers::default())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mapeia_comandos_que_existem_no_motor_rust() {
        let none = Modifiers::default();
        assert!(matches!(comando(&Key::Named(Named::F5), none), Some(Msg::MacroExecutar)));
        assert!(matches!(comando(&Key::Named(Named::Enter), Modifiers::CTRL), Some(Msg::MacroExecutar)));
        assert!(matches!(comando(&Key::Character("s".into()), Modifiers::CTRL), Some(Msg::MacroGuardarModulo)));
        assert!(matches!(comando(&Key::Character("S".into()), Modifiers::CTRL | Modifiers::SHIFT), Some(Msg::MacroSalvar)));
        assert!(matches!(comando(&Key::Character("o".into()), Modifiers::CTRL), Some(Msg::MacroAbrir)));
        assert!(matches!(comando(&Key::Character("g".into()), Modifiers::CTRL), Some(Msg::MacroVisual(EventoVisual::Painel(0)))));
        assert!(matches!(comando(&Key::Character("q".into()), Modifiers::ALT), Some(Msg::FecharAux(Auxiliar::Macros))));
    }

    #[test]
    fn localizar_substituir_e_desfazer_tem_atalho_no_editor() {
        let none = Modifiers::default();
        let evento = |k: Key, m: Modifiers| match comando(&k, m) {
            Some(Msg::MacroVisual(EventoVisual::Codigo(e))) => format!("{e:?}"),
            _ => String::new(),
        };
        assert_eq!(evento(Key::Character("f".into()), Modifiers::CTRL), "Abrir(false)");
        assert_eq!(evento(Key::Character("h".into()), Modifiers::CTRL), "Abrir(true)");
        assert_eq!(evento(Key::Named(Named::F3), none), "Proximo");
        assert_eq!(evento(Key::Named(Named::F3), Modifiers::SHIFT), "Anterior");
        assert_eq!(evento(Key::Character("z".into()), Modifiers::CTRL), "Desfazer");
        assert_eq!(evento(Key::Character("y".into()), Modifiers::CTRL), "Refazer");
        assert_eq!(evento(Key::Character("Z".into()), Modifiers::CTRL | Modifiers::SHIFT), "Refazer");
        assert!(deve_ouvir_capturada(&Key::Character("f".into()), Modifiers::CTRL));
    }

    #[test]
    fn atalhos_de_depuracao_indisponiveis_nao_disparam_execucao() {
        let none = Modifiers::default();
        for key in [Named::F8, Named::F9] {
            assert!(comando(&Key::Named(key), none).is_none());
        }
        assert!(comando(&Key::Named(Named::F5), Modifiers::SHIFT).is_none());
        assert!(comando(&Key::Named(Named::F8), Modifiers::CTRL | Modifiers::SHIFT).is_none());
        assert!(comando(&Key::Named(Named::F9), Modifiers::CTRL).is_none());
        assert!(comando(&Key::Character("n".into()), Modifiers::CTRL).is_none());
        assert!(!deve_ouvir_capturada(&Key::Character("a".into()), Modifiers::default()));
        assert!(deve_ouvir_capturada(&Key::Named(Named::F5), none));
        assert!(deve_ouvir_capturada(&Key::Named(Named::Escape), none));
    }
}
