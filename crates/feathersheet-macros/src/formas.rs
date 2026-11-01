//! `sheet.shape(nome)`: as formas e caixas de texto da aba pelo nome do Excel
//! ("TextBox 39"), para o `Shapes("…").TextFrame2.TextRange.Font` do VBA.
//!
//! As formas moram no campo `ob` da aba (JSON `{"v": n, "objetos": [...]}`, modelo em
//! `feathersheet_io::objetos_aba`); o nome vem do `.xlsx` em `nome`. Aqui só se mexe no
//! JSON (sem depender do `feathersheet-io`), e cada mudança passa por
//! `Sheet::alterar_objetos`, dentro da transação da macro (um Ctrl+Z, e o erro desfaz).
use feathersheet_core::planilha::sheet::Sheet;
use serde_json::{json, Map, Value as Json};

/// As operações desta parte.
pub(crate) const OPERACOES: &[&str] = &["formaLer", "formaFonte", "formaTexto"];

fn objetos(sheet: &Sheet) -> Result<Json, String> {
    match sheet.ob.as_deref() {
        None | Some("") => Ok(json!({ "v": 1, "objetos": [] })),
        Some(t) => serde_json::from_str(t).map_err(|e| format!("Formas da aba ilegíveis: {e}")),
    }
}

/// Os nomes padrão das formas em cada língua do Excel. O Excel em português chama a caixa
/// de "CaixaDeTexto 39", mas a macro gravada escreve `Shapes("TextBox 39")` e funciona: o
/// nome padrão em inglês vale para a forma com o mesmo número.
const NOMES_PADRAO: &[&[&str]] = &[
    &["textbox", "caixadetexto", "cuadrotexto", "cuadro de texto"],
    &["rectangle", "retângulo", "rectángulo"],
    &["rounded rectangle", "retângulo de cantos arredondados", "rectángulo redondeado"],
    &["oval", "elipse"],
    &["picture", "imagem", "imagen"],
    &["button", "botão", "botón"],
    &["group", "grupo", "agrupar"],
    &["straight connector", "conector reto", "conector recto"],
    &["chart", "gráfico"],
];

/// "TextBox 39" → (grupo do nome padrão, 39); `None` se não é nome padrão.
fn padrao(nome: &str) -> Option<(usize, &str)> {
    let (base, numero) = nome.trim().rsplit_once(' ')?;
    if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) { return None; }
    let base = base.trim().to_lowercase();
    NOMES_PADRAO.iter().position(|g| g.contains(&base.as_str())).map(|g| (g, numero))
}

/// A posição da forma de nome `nome` (sem diferenciar maiúsculas, como o VBA; o nome padrão
/// vale em qualquer língua).
fn achar(ob: &Json, nome: &str) -> Option<usize> {
    let lista = ob["objetos"].as_array()?;
    let nome_de = |o: &Json| o["nome"].as_str().map(str::trim).map(str::to_owned);
    lista.iter().position(|o| nome_de(o).is_some_and(|n| n.to_lowercase() == nome.trim().to_lowercase()))
        .or_else(|| {
            let p = padrao(nome)?;
            lista.iter().position(|o| nome_de(o).is_some_and(|n| padrao(&n) == Some(p)))
        })
}

fn cor(v: &Json) -> Result<String, String> {
    match v {
        Json::String(s) if s.starts_with('#') && matches!(s.len(), 7 | 9) && s[1..].chars().all(|c| c.is_ascii_hexdigit()) => Ok(s.to_lowercase()),
        Json::Number(n) => n.as_u64().filter(|&x| x <= 0xFF_FF_FF).map(|x| crate::enderecos::vba_cor_hex(x as u32))
            .ok_or_else(|| format!("Cor inválida: {n}")),
        outro => Err(format!("Cor inválida: {outro} (use \"#RRGGBB\" ou app.rgb)")),
    }
}

/// O que a macro lê: existe, texto e letra.
fn ler(ob: &Json, nome: &str) -> Json {
    let Some(i) = achar(ob, nome) else { return json!({ "exists": false }) };
    let o = &ob["objetos"][i];
    let f = &o["fonte"];
    json!({ "exists": true, "text": o["texto"].as_str().unwrap_or(""),
        "bold": f["negrito"].as_bool().unwrap_or(false), "italic": f["italico"].as_bool().unwrap_or(false),
        "size": f["tamanho"], "color": f["cor"] })
}

/// Aplica `mudar` à forma e grava; `Ok(false)` se nada mudou.
fn alterar(sheet: &mut Sheet, nome: &str, mudar: impl FnOnce(&mut Map<String, Json>) -> Result<(), String>) -> Result<bool, String> {
    let mut ob = objetos(sheet)?;
    let i = achar(&ob, nome).ok_or_else(|| format!("A aba não tem a forma \"{nome}\""))?;
    let o = ob["objetos"][i].as_object_mut().ok_or("Forma inválida")?;
    mudar(o)?;
    Ok(sheet.alterar_objetos(Some(ob.to_string())))
}

fn fonte(o: &mut Map<String, Json>, pedido: &Json) -> Result<(), String> {
    let pedido = pedido.as_object().ok_or("font: passe um objeto { bold, italic, size, color }")?;
    let f = o.entry("fonte").or_insert_with(|| json!({}));
    if !f.is_object() { *f = json!({}); }
    let f = f.as_object_mut().expect("objeto");
    for (k, v) in pedido {
        match k.as_str() {
            "bold" => { f.insert("negrito".into(), json!(v.as_bool().unwrap_or(v.as_f64().is_some_and(|x| x != 0.0)))); }
            "italic" => { f.insert("italico".into(), json!(v.as_bool().unwrap_or(v.as_f64().is_some_and(|x| x != 0.0)))); }
            "size" => {
                let t = v.as_f64().filter(|t| (1.0..=409.0).contains(t)).ok_or("font: size de 1 a 409")?;
                f.insert("tamanho".into(), json!(t));
            }
            "color" => { f.insert("cor".into(), json!(cor(v)?)); }
            outro => return Err(format!("font: propriedade \"{outro}\" desconhecida")),
        }
    }
    Ok(())
}

/// Executa a operação na aba `sheet`. `Ok(true)` quando mudou a aba.
pub(crate) fn executar(op: &str, sheet: &mut Sheet, dados: &Json) -> Result<(Json, bool), String> {
    let nome = dados[1].as_str().ok_or("shape: informe o nome da forma")?;
    match op {
        "formaLer" => Ok((ler(&objetos(sheet)?, nome), false)),
        "formaFonte" => alterar(sheet, nome, |o| fonte(o, &dados[2])).map(|m| (Json::Null, m)),
        "formaTexto" => {
            let t: String = match &dados[2] { Json::String(s) => s.clone(), Json::Null => String::new(), v => v.to_string() };
            alterar(sheet, nome, |o| { o.insert("texto".into(), json!(t.chars().take(32_768).collect::<String>())); Ok(()) }).map(|m| (Json::Null, m))
        }
        _ => Err(format!("Operação desconhecida: {op}")),
    }
}
