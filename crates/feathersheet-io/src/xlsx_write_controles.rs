//! Controles de formulário da aba no `.xlsx`/`.xlsm`: as formas VML (`x:ClientData`) que o
//! Excel lê desde o 2007, no mesmo `vmlDrawingN.vml` das notas (a aba só tem um
//! `<legacyDrawing>`). O par lido em `xlsx_read/controles.rs`.

use std::fmt::Write as _;

use feathersheet_core::planilha::sheet::Sheet;

use crate::objetos_aba::{AlinharH, AlinharV, ObjetosAba, TipoControleForm, TipoObjeto};
use crate::xlsx_write::escape_xml;

/// O molde das formas de controle (o mesmo `_x0000_t201` que o Excel grava).
const MOLDE: &str = "<v:shapetype id=\"_x0000_t201\" coordsize=\"21600,21600\" o:spt=\"201\" path=\"m,l,21600r21600,l21600,xe\"><v:stroke joinstyle=\"miter\"/><v:path shadowok=\"f\" o:extrusionok=\"f\" strokeok=\"f\" fillok=\"f\" o:connecttype=\"rect\"/><o:lock v:ext=\"edit\" shapetype=\"t\"/></v:shapetype>";

/// As formas VML dos controles da aba `n` (base 1), com o molde na frente. `None` sem controle.
/// Os ids ficam de `n * 1024 + 512` em diante, longe dos das notas (`n * 1024 + 1…`).
pub(crate) fn formas_vml(sheet: &Sheet, n: usize) -> Option<String> {
    let objetos = ObjetosAba::ler(sheet.ob.as_deref()).ok()?;
    let mut vml = String::new();
    for (k, o) in objetos.objetos.iter().filter(|o| o.tipo == TipoObjeto::Controle).enumerate().take(512) {
        let Some(c) = o.controle.as_ref() else { continue };
        let botao = c.tipo == TipoControleForm::Botao;
        let _ = write!(
            vml,
            "<v:shape id=\"_x0000_s{}\" type=\"#_x0000_t201\" style=\"position:absolute;z-index:{};mso-wrap-style:tight\"{}{} o:insetmode=\"auto\">{}<o:lock v:ext=\"edit\" rotation=\"t\"/>",
            n * 1024 + 512 + k,
            k + 1,
            c.texto_alt.as_deref().map_or(String::new(), |t| format!(" alt=\"{}\"", escape_xml(t))),
            format!("{}{}{}",
                if botao { " o:button=\"t\"" } else { "" },
                match (&c.fundo, botao) {
                    (Some(f), _) => format!(" fillcolor=\"{}\" filled=\"t\"", escape_xml(f)),
                    (None, true) => " fillcolor=\"buttonFace [67]\"".into(),
                    (None, false) => " filled=\"f\"".into(),
                },
                match (&c.linha, botao) {
                    (Some(l), _) => format!(" strokecolor=\"{}\" stroked=\"t\"", escape_xml(l)),
                    (None, true) => " strokecolor=\"windowText [64]\"".into(),
                    (None, false) => " stroked=\"f\" strokecolor=\"windowText [64]\"".into(),
                }),
            if botao { "<v:fill color2=\"buttonFace [67]\" o:detectmouseclick=\"t\"/>" } else { "" },
        );
        let fonte = o.fonte.as_ref();
        let (negrito, italico) = (fonte.is_some_and(|f| f.negrito), fonte.is_some_and(|f| f.italico));
        if let Some(t) = o.texto.as_deref().filter(|t| !t.is_empty()) {
            let linhas: Vec<String> = t.lines().map(escape_xml).collect();
            let _ = write!(
                vml,
                "<v:textbox style=\"mso-direction-alt:auto\" o:singleclick=\"f\"><div style=\"text-align:{}\"><font face=\"{}\" size=\"{}\" color=\"{}\">{}{}{}{}{}</font></div></v:textbox>",
                if !botao { "left" } else { match o.alinhar.h { AlinharH::Esquerda => "left", AlinharH::Centro => "center", AlinharH::Direita => "right" } },
                escape_xml(fonte.and_then(|f| f.familia.as_deref()).unwrap_or("Calibri")),
                (fonte.and_then(|f| f.tamanho).unwrap_or(11.0) * 20.0).round() as i64,
                escape_xml(fonte.and_then(|f| f.cor.as_deref()).unwrap_or("#000000")),
                if negrito { "<b>" } else { "" }, if italico { "<i>" } else { "" },
                linhas.join("<br>"),
                if italico { "</i>" } else { "" }, if negrito { "</b>" } else { "" },
            );
        }
        let _ = write!(
            vml,
            "<x:ClientData ObjectType=\"{}\">{}<x:Anchor>{}, {}, {}, {}, {}, {}, {}, {}</x:Anchor>{}<x:AutoFill>False</x:AutoFill>",
            c.tipo.para_excel(),
            // Presença = NÃO move / NÃO redimensiona com as células (como o Excel grava as notas).
            match c.posicao { 2 => "<x:MoveWithCells/><x:SizeWithCells/>", 1 => "<x:SizeWithCells/>", _ => "" },
            o.de.coluna, o.de.dx.round() as i64, o.de.linha, o.de.dy.round() as i64,
            o.ate.coluna, o.ate.dx.round() as i64, o.ate.linha, o.ate.dy.round() as i64,
            if c.nao_imprimir { "<x:PrintObject>False</x:PrintObject>" } else { "" },
        );
        if c.desbloqueado { vml.push_str("<x:Locked>False</x:Locked>"); }
        if c.texto_desbloqueado { vml.push_str("<x:LockText>False</x:LockText>"); }
        let mut marca = |nome: &str, valor: &str| { let _ = write!(vml, "<x:{nome}>{}</x:{nome}>", escape_xml(valor)); };
        if let Some(m) = c.macro_excel.as_deref() { marca("FmlaMacro", m); }
        if botao {
            marca("TextHAlign", match o.alinhar.h { AlinharH::Esquerda => "Left", AlinharH::Centro => "Center", AlinharH::Direita => "Right" });
            marca("TextVAlign", match o.alinhar.v { AlinharV::Topo => "Top", AlinharV::Meio => "Center", AlinharV::Baixo => "Bottom" });
        }
        if let Some(v) = c.faixa.as_deref() { marca("FmlaRange", v); }
        if let Some(v) = c.celula.as_deref() { marca("FmlaLink", v); }
        if let Some(v) = c.valor {
            marca(if matches!(c.tipo, TipoControleForm::Combo | TipoControleForm::Lista) { "Sel" } else { "Val" }, &v.to_string());
        }
        for (nome, v) in [("Min", c.min), ("Max", c.max), ("Inc", c.passo), ("Page", c.pagina)] {
            if let Some(v) = v { marca(nome, &v.to_string()); }
        }
        if let Some(v) = c.marcado.filter(|v| *v > 0) { marca("Checked", &v.to_string()); }
        if let Some(v) = c.linhas { marca("DropLines", &v.to_string()); }
        if c.tipo == TipoControleForm::Combo { marca("DropStyle", "Combo"); }
        if c.horizontal { vml.push_str("<x:Horiz/>"); }
        if c.sem_3d { vml.push_str("<x:NoThreeD/>"); }
        vml.push_str("</x:ClientData></v:shape>");
    }
    (!vml.is_empty()).then(|| format!("{MOLDE}{vml}"))
}

/// O VML inteiro da aba `n` só com controles (sem notas).
pub(crate) fn vml_sozinho(formas: &str, n: usize) -> String {
    format!(
        "<xml xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:x=\"urn:schemas-microsoft-com:office:excel\"><o:shapelayout v:ext=\"edit\"><o:idmap v:ext=\"edit\" data=\"{n}\"/></o:shapelayout>{formas}</xml>"
    )
}

#[cfg(test)]
mod testes {
    use feathersheet_core::planilha::workbook::Workbook;

    use crate::objetos_aba::{AlinharH, Ancora, ControleFormulario, ObjetoSolto, ObjetosAba, TipoControleForm};

    /// Botão com macro e botão de girar ligado a uma célula: gravar e ler de volta dá o mesmo.
    #[test]
    fn controles_vao_e_voltam_no_xlsx() {
        let mut wb = Workbook::new();
        let mut lista = ObjetosAba::default();
        let mut c = ControleFormulario::novo(TipoControleForm::Botao);
        c.macro_excel = Some("[0]!AbrirPainel".into());
        (c.fundo, c.linha, c.posicao, c.desbloqueado, c.nao_imprimir, c.texto_alt) =
            (Some("#ffff99".into()), Some("#c00000".into()), 1, true, true, Some("Abre o painel".into()));
        let mut botao = ObjetoSolto::novo_controle(c, Ancora { linha: 1, coluna: 13, dx: 0.0, dy: 3.0 }, Ancora { linha: 3, coluna: 14, dx: 110.0, dy: 27.0 });
        botao.texto = Some("Abrir & Painel".into());
        botao.fonte = Some(crate::objetos_aba::FonteTexto { tamanho: Some(14.0), negrito: true, cor: Some("#4472c4".into()), ..Default::default() });
        botao.alinhar.h = AlinharH::Esquerda;
        lista.adicionar(botao);
        let mut g = ControleFormulario::novo(TipoControleForm::Girar);
        (g.celula, g.valor, g.min, g.max, g.passo, g.pagina) = (Some("$C$8".into()), Some(3810), Some(1), Some(30000), Some(1), Some(10));
        lista.adicionar(ObjetoSolto::novo_controle(g, Ancora { linha: 5, coluna: 1, dx: 4.0, dy: 2.0 }, Ancora { linha: 8, coluna: 1, dx: 66.0, dy: 0.0 }));
        wb.aba_mut(0).unwrap().sheet.ob = lista.gravar();
        let bytes = crate::xlsx_write::gerar_xlsx(&wb).unwrap();
        let (lida, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
        let ob = ObjetosAba::ler(lida.aba(0).unwrap().sheet.ob.as_deref()).unwrap();
        assert_eq!(ob.objetos.len(), 2);
        let (b, s) = (&ob.objetos[0], &ob.objetos[1]);
        assert_eq!(b.texto.as_deref(), Some("Abrir & Painel"));
        assert_eq!(b.controle.as_ref().unwrap().macro_excel.as_deref(), Some("[0]!AbrirPainel"));
        assert_eq!((b.de, b.ate.coluna, b.ate.dx), (Ancora { linha: 1, coluna: 13, dx: 0.0, dy: 3.0 }, 14, 110.0));
        let bc = b.controle.as_ref().unwrap();
        assert_eq!((bc.fundo.as_deref(), bc.linha.as_deref(), bc.posicao, bc.desbloqueado, bc.nao_imprimir, bc.texto_alt.as_deref(), bc.sem_3d),
            (Some("#ffff99"), Some("#c00000"), 1, true, true, Some("Abre o painel"), false));
        let f = b.fonte.as_ref().unwrap();
        assert_eq!((f.tamanho, f.negrito, f.italico, f.cor.as_deref(), b.alinhar.h), (Some(14.0), true, false, Some("#4472c4"), AlinharH::Esquerda));
        assert!(s.fonte.is_none(), "girar sem texto não ganha fonte");
        let s = s.controle.as_ref().unwrap();
        assert_eq!((s.fundo.as_deref(), s.posicao, s.nao_imprimir), (None, 0, false));
        assert_eq!((s.tipo, s.celula.as_deref(), s.valor, s.max, s.pagina), (TipoControleForm::Girar, Some("$C$8"), Some(3810), Some(30000), Some(10)));
    }
}
