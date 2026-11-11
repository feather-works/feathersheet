//! API de formatação portada de src/macros/api-formato.ts.
//! Apenas operações sobre os estilos DIRETOS: não achata os estilos
//! herdados de aba, linha ou coluna no snapshot da célula.
use feathersheet_core::{
    celulas::{
        estilo_parcial::{Campo, EstiloParcial, mesclar},
        refs::Rect,
    },
    planilha::{sheet::{edicao::Snap, Sheet}},
};
use serde_json::Value as Json;
use crate::enderecos;

pub(crate) fn comando(op: &str) -> bool {
    matches!(op,
        "setFontFamily" | "setFontSize" | "setBold" | "setItalic"
        | "setUnderline" | "setStrikethrough" | "setFontColor"
        | "setBackground" | "setAlignHorizontal" | "setAlignVertical"
        | "setWrapText" | "setTextRotation" | "setHyperlink")
}

fn flag(valor: &Json, default: bool) -> Result<bool, String> {
    if valor.is_null() { return Ok(default); }
    valor.as_bool().ok_or(feathersheet_core::tr!("Esperado verdadeiro ou falso").into())
}

pub(super) fn cor(valor: &Json) -> Result<String, String> {
    // Números de cor VBA/BGR são convertidos em inteiro de 32 bits.
    // O JavaScript faz conversão bitwise também para valores negativos.
    if let Some(numero) = valor.as_i64() {
        return Ok(enderecos::vba_cor_hex((numero as u32) & 0xff_ffff));
    }
    if let Some(numero) = valor.as_u64() {
        return Ok(enderecos::vba_cor_hex((numero as u32) & 0xff_ffff));
    }
    if let Some(texto) = valor.as_str() {
        return enderecos::cor_hex(texto);
    }
    Err(feathersheet_core::tr!("Cor inválida: use #RRGGBB, #RGB ou número VBA BGR").into())
}

/// Valida antes de modificar; erros nunca deixam a macro meio aplicada.
pub(crate) fn preparar(op: &str, dados: &Json) -> Result<EstiloParcial, String> {
    let v = &dados[2];
    let mut patch = EstiloParcial::new();
    let (campo, valor) = match op {
        "setBold" => ("bold", Campo::Logico(flag(v, true)?)),
        "setItalic" => ("italic", Campo::Logico(flag(v, true)?)),
        "setStrikethrough" => ("strike", Campo::Logico(flag(v, true)?)),
        "setWrapText" => ("wrap", Campo::Logico(flag(v, true)?)),
        "setUnderline" => {
            let ativado = if v.is_null() { true }
                else if let Some(b) = v.as_bool() { b }
                else {
                    match v.as_str() {
                        Some("single" | "double") => true,
                        Some("none") => false,
                        _ => return Err(feathersheet_core::tr!("Sublinhado: use true, false, single, double ou none").into()),
                    }
                };
            ("underline", Campo::Logico(ativado))
        }
        "setFontFamily" => {
            let nome = v.as_str().ok_or_else(|| feathersheet_core::tr!("Nome de fonte precisa ser texto"))?;
            if nome.trim().is_empty() || nome.len() > 200 {
                return Err(feathersheet_core::tr!("Nome de fonte inválido").into());
            }
            ("fontFamily", Campo::Texto(nome.to_owned()))
        }
        "setFontSize" => {
            let n = v.as_f64().ok_or_else(|| feathersheet_core::tr!("Tamanho da fonte precisa ser número"))?;
            if !n.is_finite() || n <= 0.0 || n > 400.0 {
                return Err(feathersheet_core::tr!("Tamanho de fonte fora do intervalo (0, 400]").into());
            }
            ("fontSize", Campo::Numero(n))
        }
        "setFontColor" => ("color", Campo::Texto(cor(v)?)),
        "setBackground" => ("bg", Campo::Texto(cor(v)?)),
        "setAlignHorizontal" => {
            let align = v.as_str().ok_or_else(|| feathersheet_core::tr!("Alinhamento horizontal inválido"))?;
            match align {
                "left" | "center" | "right" => ("align", Campo::Texto(align.into())),
                "general" => ("align", Campo::Nulo),
                _ => return Err(feathersheet_core::tr!("Use left, center, right ou general").into()),
            }
        }
        "setAlignVertical" => {
            let align = v.as_str().ok_or_else(|| feathersheet_core::tr!("Alinhamento vertical inválido"))?;
            if !matches!(align, "top" | "middle" | "bottom") {
                return Err(feathersheet_core::tr!("Use top, middle ou bottom").into());
            }
            ("vAlign", Campo::Texto(align.into()))
        }
        "setTextRotation" => {
            let n = v.as_f64().ok_or_else(|| feathersheet_core::tr!("Rotação deve ser número"))?;
            if !n.is_finite() {
                return Err(feathersheet_core::tr!("Rotação não pode ser infinita").into());
            }
            ("rotation", Campo::Numero((n + 0.5).floor().clamp(-90.0, 90.0)))
        }
        "setHyperlink" => {
            let link = v.as_str().ok_or_else(|| feathersheet_core::tr!("Endereço do hyperlink deve ser texto"))?;
            if link.len() > 8192 { return Err("Link muito longo".into()); }
            ("link", Campo::Texto(link.into()))
        }
        _ => return Err(feathersheet_core::trf!("Comando de formatação desconhecido: {op}")),
    };
    patch.insert(campo.into(), valor);
    Ok(patch)
}

/// Compatível com setStyle por célula sem perder a herança nem criar
/// transações aninhadas por célula. A transação é iniciada pelo host.
pub(crate) fn aplicar(
    sheet: &mut Sheet, area: &Rect, patch: &EstiloParcial, texto: Option<&str>,
) -> usize {
    let mut feitas = 0;
    for r in area.r0..=area.r1 {
        for c in area.c0..=area.c1 {
            let antes = sheet.snapshot(r, c);
            let base = antes.as_ref().and_then(|foto|foto.style.as_ref());
            let depois = mesclar(base, patch);
            let mut raw = antes.as_ref().map(|s|s.raw.clone()).unwrap_or_default();
            if r == area.r0 && c == area.c0 {
                if let Some(t) = texto { raw = t.to_owned(); }
            }
            let next = if raw.is_empty() && depois.is_empty() {
                None
            } else {
                Some(Snap {raw, style: (!depois.is_empty()).then_some(depois)})
            };
            if sheet.apply(r,c,next,true) { feitas += 1; }
        }
    }
    feitas
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::estilo_parcial::Campo;

    #[test]
    fn fonte_cor_vba_alinhamento_e_limites() {
        assert_eq!(preparar("setFontColor",&serde_json::json!([0,"A1",255])).
            unwrap().get("color"),Some(&Campo::Texto("#ff0000".into())));
        assert_eq!(preparar("setBackground",&serde_json::json!([0,"A1","#F0A"])) 
            .unwrap().get("bg"), Some(&Campo::Texto("#ff00aa".into())));
        assert_eq!(preparar("setAlignHorizontal",&serde_json::json!([0,"A1","general"]))
            .unwrap().get("align"),Some(&Campo::Nulo));
        assert!(preparar("setFontSize",&serde_json::json!([0,"A1",401])).is_err());
        assert!(preparar("setTextRotation",&serde_json::json!([0,"A1","90"])).is_err());
        assert_eq!(preparar("setTextRotation",&serde_json::json!([0,"A1",-130]))
            .unwrap().get("rotation"),Some(&Campo::Numero(-90.0)));
    }

    #[test]
    fn estilo_direto_preserva_heranca_e_undo() {
        let mut sheet=Sheet::new("A");
        sheet.set_cell(0,0,"12");
        sheet.abrir_transacao();
        let bold=preparar("setBold",&serde_json::json!([0,"A1",true])).unwrap();
        assert_eq!(aplicar(&mut sheet,&Rect::cell(0,0),&bold,None),1);
        let patch=preparar("setBackground",&serde_json::json!([0,"A1","#fff"])).unwrap();
        assert_eq!(aplicar(&mut sheet,&Rect::cell(0,0),&patch,None),1);
        sheet.fechar_transacao();
        let s=sheet.estilo_resolvido(0,0).unwrap();
        assert!(s.bold);
        assert_eq!(s.bg,Some("#ffffff".into()));
        assert_eq!(sheet.get_raw(0,0),"12");
        assert!(sheet.undo());
        assert!(sheet.estilo_resolvido(0,0).is_none());
    }

    #[test]
    fn hyperlink_escreve_texto_so_na_primeira_celula() {
        let mut sheet=Sheet::new("A");
        sheet.set_cell(0,1,"segundo");
        let patch=preparar("setHyperlink",
            &serde_json::json!([0,"A1:B1","https://example.com","clique"])).unwrap();
        sheet.abrir_transacao();
        assert_eq!(aplicar(&mut sheet,&Rect::new(0,0,0,1),&patch,Some("clique")),2);
        sheet.fechar_transacao();
        assert_eq!(sheet.get_raw(0,0),"clique");
        assert_eq!(sheet.get_raw(0,1),"segundo");
        assert_eq!(sheet.estilo_resolvido(0,1).unwrap().link,
            Some("https://example.com".into()));
    }
}
