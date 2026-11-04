//! O texto que a célula mostra para um valor já calculado.

use crate::celulas::style::format_number;
use crate::celulas::values::Value;

/// O `formatValue` do `style.ts`, para o valor já calculado.
pub fn texto_formatado(valor: &Value, estilo: Option<&crate::celulas::style::Style>) -> String {
    // Formato personalizado (`;;;`, `0" kWp"`) manda sobre o resto.
    if let Some(codigo) = estilo.and_then(|e| e.code.as_deref()) {
        if let Some(t) = crate::celulas::formato_personalizado::formatar(valor, codigo) { return t; }
    }
    match valor {
        Value::Empty => String::new(),
        Value::Error(e) => e.code.as_str().to_string(),
        Value::Number(n) => format_number(*n, estilo),
        Value::Text(t) => t.clone(),
        Value::Bool(b) => if *b { "VERDADEIRO" } else { "FALSO" }.to_string(),
    }
}
