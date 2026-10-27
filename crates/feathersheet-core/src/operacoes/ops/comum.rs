//! Auxiliares compartilhados pelas operações (texto no estilo JavaScript, escrita e foto de linha).


// ------------------------------------------------ as operações por linha
//
// Passo 3 da migração (23/09/2026): ordenar, remover repetidas, dividir
// texto, aparar espaços, bordas e casas decimais, traduzidos do `ops.ts`.
// Pela ponte eram uma leitura e uma escrita por célula.
//
// Cada uma escreve pelo caminho de quem edita (`set_cell`, `set_style`,
// `clear`) dentro de uma transação: um Ctrl+Z desfaz tudo, como no
// TypeScript. As versões anteriores de repetidas, aparar e dividir gravavam
// por `set_cell_raw` — fora do desfazer, e as duas últimas perdiam o estilo.
//
// Quem precisa do valor calculado é dividido em duas metades, porque o valor
// vem da pasta (`Workbook::valor`) e a escrita pede o `Sheet` emprestado: o
// `planejar_*` lê, o `aplicar_*` escreve. Os atalhos sem prefixo leem o
// `Sheet` sozinho (testes).

/// O `\s` e o `trim()` do JavaScript. O `char::is_whitespace` do Rust conta
/// o U+0085 e não conta o U+FEFF — o contrário do JavaScript.
pub(super) fn espaco_js(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
            | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'
    )
}

pub(super) fn trim_js(texto: &str) -> &str {
    texto.trim_matches(espaco_js)
}


