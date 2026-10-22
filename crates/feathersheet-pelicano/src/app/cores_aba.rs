//! APP-016: paleta e geometria do menu de abas. Sem lógica de células.
//! Cor gravada no mesmo campo `Sheet::tab_color` do FSHZ legado.

pub const PALETA: &[(&str, &str)] = &[
    ("Vermelho", "#ea4335"),
    ("Laranja", "#fb8c00"),
    ("Amarelo", "#fbbc04"),
    ("Verde", "#34a853"),
    ("Turquesa", "#00acc1"),
    ("Azul", "#4285f4"),
    ("Roxo", "#9334e6"),
    ("Rosa", "#e91e63"),
    ("Cinza", "#778899"),
    ("Marrom", "#a66a43"),
];

/// Aceita o valor #RRGGBB persistido pelos editores TS/Rust.
pub fn cor_css(valor: &str) -> Option<pelicano::Color> {
    let s = valor.strip_prefix('#')?;
    if s.len() != 6 { return None; }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some(pelicano::Color::rgb8(r, g, b))
}

/// Menu flutuante acima das abas e limitado à largura da área visível.
/// Coordenada X recebida do ponteiro relativo à barra, não da posição da grade.
pub fn x_menu_aba(cursor_x: f32, largura_view: f32, largura_menu: f32) -> f32 {
    let limite = (largura_view - largura_menu - 8.0).max(0.0);
    if !cursor_x.is_finite() { return 8.0_f32.min(limite); }
    (cursor_x - 14.0).clamp(0.0, limite)
}

pub fn altura_menu(quantidade: usize, limite_view: f32) -> f32 {
    // Linha = 34 px; não exibir painel com centenas de pixels vazios.
    (quantidade as f32 * 35.0 + 12.0)
        .min((limite_view - 8.0).max(72.0))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn menu_real_da_aba_tem_cor_e_preserva_os_dados_no_fshz() {
        use crate::app::{App, Contexto, Msg};
        let mut app = App::default();
        app.update(Msg::AdicionarAba);
        assert_eq!(app.workbook.len(), 2);
        // Clique na primeira aba enquanto a segunda estava ativa.
        app.update(Msg::PonteiroSobreAbas(112.0));
        app.update(Msg::AbrirContexto(Contexto::Aba(0), 8.0, 8.0));
        assert_eq!(app.folha, 0);
        let (tipo, x, y) = app.contexto.unwrap();
        assert!(matches!(tipo, Contexto::Aba(0)));
        assert_eq!(x, 98.0);
        assert_eq!(y, 0.0);
        // Altura acompanha os itens do menu (sem painel com sobra vazia).
        let itens = app.opcoes_contexto(tipo).len();
        assert_eq!(app.contexto_tamanho.1, itens as f32 * 35.0 + 12.0);
        let opcoes: Vec<_> = app.opcoes_contexto(tipo).into_iter().flatten().collect();
        let indice = opcoes.iter().position(|(rotulo, _)| rotulo == "Cor da aba…").unwrap();
        app.update(Msg::ContextoEscolher(indice));
        assert!(app.paleta_aba_aberta);
        assert!(app.contexto.is_some(), "a paleta continua sobre a aba clicada");
        let antes = app.workbook.aba(0).unwrap().sheet.get_raw(0,0);
        app.update(Msg::PintarAba(Some("#34a853")));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.tab_color.as_deref(),
            Some("#34a853"));
        assert_eq!(app.workbook.aba(1).unwrap().sheet.tab_color, None);
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(0,0), antes);
        assert!(app.alterado);
        assert!(app.contexto.is_none());

        let bytes = feathersheet_io::da_workbook::pasta_gravar_bytes(
            &app.workbook, app.folha as u32
        ).expect("serializar FSHZ");
        let lida = feathersheet_io::fshz_read::ler_fshz(&bytes).expect("abrir FSHZ");
        let restaurado = feathersheet_io::para_workbook::para_workbook(&lida);
        assert_eq!(restaurado.aba(0).unwrap().sheet.tab_color.as_deref(),
            Some("#34a853"));
        assert_eq!(restaurado.aba(1).unwrap().sheet.tab_color, None);

        app.update(Msg::PintarAba(None));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.tab_color, None);
    }

    #[test]
    fn cores_da_aba_sao_css_compativeis_com_fshz() {
        assert_eq!(PALETA.len(), 10);
        for (_, css) in PALETA {
            assert!(cor_css(css).is_some(), "{css}");
        }
        assert!(cor_css("#12").is_none());
        assert!(cor_css("#xxff00").is_none());
        assert!(cor_css("red").is_none());
    }

    #[test]
    fn popup_abre_na_aba_e_nao_no_topo_da_grade() {
        assert_eq!(x_menu_aba(80.0, 509.0, 248.0), 66.0);
        assert_eq!(x_menu_aba(500.0, 509.0, 248.0), 253.0);
        assert_eq!(x_menu_aba(-20.0, 509.0, 248.0), 0.0);
        assert_eq!(x_menu_aba(50.0, 180.0, 248.0), 0.0);
        assert!(altura_menu(4, 450.0) < 200.0);
    }
}
