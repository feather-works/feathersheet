//! Endereços, abas e valores em texto: o que as ferramentas recebem e devolvem.
use feathersheet_core::celulas::locale::{apresentar_formula, get_locale_config, normalizar_formula};
use feathersheet_core::celulas::number::to_js_string;
use feathersheet_core::celulas::refs::{cell_name, parse_range_ref, Rect};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::workbook::Workbook;
use serde_json::Value as Json;

use crate::hospede::Hospede;

/// Texto de um argumento, se veio.
pub fn arg_texto<'a>(args: &'a Json, nome: &str) -> Option<&'a str> {
    args.get(nome).and_then(Json::as_str).map(str::trim).filter(|s| !s.is_empty())
}

pub fn arg_bool(args: &Json, nome: &str, padrao: bool) -> bool {
    args.get(nome).and_then(Json::as_bool).unwrap_or(padrao)
}

pub fn arg_num(args: &Json, nome: &str, padrao: u64) -> u64 {
    args.get(nome).and_then(Json::as_u64).unwrap_or(padrao)
}

/// Índice da aba pelo nome (sem ligar para maiúscula) ou pelo número (base 0).
pub fn indice_aba(pasta: &Workbook, aba: &Json) -> Result<usize, String> {
    if let Some(i) = aba.as_u64() {
        return (i < pasta.len() as u64).then_some(i as usize)
            .ok_or_else(|| format!("Não existe a aba {i}: a pasta tem {} aba(s).", pasta.len()));
    }
    let nome = aba.as_str().unwrap_or("").trim().trim_matches('\'');
    pasta.nomes().iter().position(|n| n.eq_ignore_ascii_case(nome) || n.to_lowercase() == nome.to_lowercase())
        .ok_or_else(|| format!("Não existe a aba \"{nome}\". Abas: {}.", pasta.nomes().join(", ")))
}

/// A aba do argumento `aba`, ou a ativa.
pub fn aba_do_arg(h: &dyn Hospede, args: &Json) -> Result<usize, String> {
    match args.get("aba") {
        Some(a) if !a.is_null() && a.as_str() != Some("") => indice_aba(h.pasta(), a),
        _ => Ok(h.aba_ativa()),
    }
}

/// "A1:C9", "Dados!A1:B2" ou "'Meu plano'!C:C" → (aba, retângulo). O nome
/// da aba no texto ganha do argumento `aba`.
pub fn intervalo(h: &dyn Hospede, args: &Json, campo: &str) -> Result<(usize, Rect), String> {
    let texto = arg_texto(args, campo).ok_or_else(|| format!("Falta `{campo}` (ex.: \"A1:C10\")."))?;
    let (aba, ref_) = match texto.rsplit_once('!') {
        Some((aba, r)) => (indice_aba(h.pasta(), &Json::String(aba.to_string()))?, r),
        None => (aba_do_arg(h, args)?, texto),
    };
    // "G" é a coluna G inteira e "3" a linha 3 (o `parse_range_ref` só aceita G:G e 3:3).
    let sozinho = !ref_.contains(':') && (ref_.chars().all(|c| c.is_ascii_alphabetic()) || ref_.chars().all(|c| c.is_ascii_digit()));
    let ref_ = if sozinho { format!("{ref_}:{ref_}") } else { ref_.to_string() };
    let rect = parse_range_ref(&ref_).ok_or_else(|| format!("Endereço inválido: \"{texto}\"."))?;
    Ok((aba, rect.normalized()))
}

pub fn nome_area(r: &Rect) -> String {
    if (r.r0, r.c0) == (r.r1, r.c1) { cell_name(r.r0, r.c0) }
    else { format!("{}:{}", cell_name(r.r0, r.c0), cell_name(r.r1, r.c1)) }
}

/// Valor cru para a IA: número com ponto, sem formato de exibição.
pub fn valor_texto(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => to_js_string(*n),
        Value::Text(t) => t.clone(),
        Value::Bool(b) => if *b { "VERDADEIRO" } else { "FALSO" }.into(),
        Value::Error(e) => e.code.as_str().into(),
    }
}

/// Fórmula como a pessoa vê (pt-BR: `;` e vírgula decimal).
pub fn formula_exibida(bruto: &str) -> String {
    apresentar_formula(bruto, &get_locale_config())
}

/// Conteúdo vindo da IA → texto que a célula guarda. Fórmula em pt-BR
/// (`=SOMA(A1;B1)`) é normalizada; número JSON vira número.
pub fn conteudo_da_ia(v: &Json) -> String {
    match v {
        Json::Null => String::new(),
        Json::Bool(b) => if *b { "VERDADEIRO" } else { "FALSO" }.into(),
        Json::Number(n) => n.as_f64().map(to_js_string).unwrap_or_default(),
        Json::String(s) if s.starts_with('=') => normalizar_formula(s, &get_locale_config()),
        Json::String(s) => s.clone(),
        outro => outro.to_string(),
    }
}

/// Corta texto longo de célula para não estourar a resposta.
pub fn curto(t: &str, limite: usize) -> String {
    if t.chars().count() > limite {
        format!("{}…", t.chars().take(limite - 1).collect::<String>())
    } else { t.to_owned() }
}

/// Uma tabela TSV com letras de coluna e números de linha, que é o jeito
/// mais barato em tokens de mostrar um pedaço de planilha.
pub fn tabela(area: &Rect, mut celula: impl FnMut(u32, u32) -> String) -> String {
    use feathersheet_core::celulas::refs::col_name;
    let mut s = String::new();
    for c in area.c0..=area.c1 {
        s.push('\t');
        s.push_str(&col_name(c));
    }
    for r in area.r0..=area.r1 {
        s.push('\n');
        s.push_str(&(r + 1).to_string());
        for c in area.c0..=area.c1 {
            s.push('\t');
            s.push_str(&celula(r, c).replace(['\t', '\n'], " "));
        }
    }
    s
}

pub fn conteudo(texto: impl Into<String>) -> Vec<Json> {
    vec![serde_json::json!({ "type": "text", "text": texto.into() })]
}
