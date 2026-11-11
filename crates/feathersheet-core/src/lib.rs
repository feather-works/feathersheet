//! `feathersheet-core` — o motor do FeatherSheet.
//!
//! Aqui mora tudo que sabe o que é uma célula, um valor e uma fórmula, sem
//! saber nada de janela, de canvas ou de Tauri. É o que o `src/core/` em
//! TypeScript era até 21/09/2026; a partir daí o motor é este, e o TypeScript
//! só desenha.
//!
//! ## Como ler este crate junto com o TypeScript antigo
//!
//! Enquanto a migração não acaba, os dois motores convivem e a tradução é
//! módulo a módulo, com o mesmo nome:
//!
//! | TypeScript          | Rust               |
//! | ------------------- | ------------------ |
//! | `core/refs.ts`      | [`refs`]           |
//! | `core/values.ts`    | [`values`]         |
//! | `core/tokenizer.ts` | [`tokenizer`]      |
//! | `core/ast.ts`       | [`ast`]            |
//! | `core/parser.ts`    | [`parser`]         |
//! | (o `Number`/`String` do JavaScript) | [`number`] |
//! | `core/pool.ts`       | [`pool`]           |
//! | `core/column-store.ts` | [`column_store`] |
//! | `core/data-table.ts` | [`data_table`]     |
//! | `core/sheet-deps.ts` | [`sheet_deps`]     |
//! | `core/date-serial.ts` | [`date_serial`]   |
//! | `core/lttb.ts`       | [`lttb`]            |
//! | `core/ops.ts`        | [`ops`]             |
//! | `core/query.ts`/tabela dinâmica | [`pivot`] |
//! | `core/formula-text.ts` | [`formula_text`]  |
//! | `core/formula-pattern.ts` | [`formula_pattern`] |
//!
//! Por isso os nomes públicos são em inglês, como os do TypeScript: dá para
//! pôr os dois arquivos lado a lado e conferir linha por linha. Os comentários
//! são em português, como o resto do projeto. Os comandos do Tauri, esses
//! continuam em português (`ler_arquivo`, `gravar_arquivo`) — a fronteira é a
//! camada de comandos, não o motor.
//!
//! ATENÇÃO — conflito em aberto (21/09/2026): [`sheet`] e [`data_table`] +
//! [`column_store`] + [`pool`] são duas tentativas independentes da mesma
//! etapa (armazenamento). `sheet.rs` guarda número solto num
//! `HashMap<u64, f64>` — sem texto, sem booleano, sem o desenho colunar por
//! bloco que `column-store.ts` existe para dar. Antes de qualquer um dos
//! dois virar o armazenamento de verdade, isso precisa de decisão: ver
//! `CLAUDE.md` e `docs/TAREFAS-GEMINI.md`. [`ops`] e [`pivot`] já usam [`sheet`]
//! (o `HashMap<u64, f64>`), então essa decisão também afeta os dois.

pub mod celulas;
pub mod idioma;
pub mod formula;
pub mod funcoes;
pub mod planilha;
pub mod operacoes;
pub mod grafico;
pub mod dinamica;
pub mod consulta;
pub mod diario;
pub mod paralelismo;
pub mod modelos;
#[cfg(test)]
pub(crate) mod teste_memoria;
#[cfg(test)]
mod tetos_de_memoria;

pub use formula::ast::{BinaryOp, Node, ParseError, UnaryOp};
pub use planilha::data_table::DataTable;
pub use celulas::locale::{get_locale_config, set_locale_config, LocaleConfig};
pub use operacoes::ops::{find_all, find_all_com, is_hit, replace_all, replace_all_em, replace_every, SearchMatch, SearchOptions};
pub use formula::parser::parse_formula;
pub use dinamica::pivot::{montar_dinamica, AgregacaoOp, AgruparData, DinamicaConfig, MedidaDinamica};
pub use celulas::pool::StringPool;
pub use consulta::query::{executar_query, Agregado, Passo, Tabela};
pub use celulas::refs::{cell_name, col_index, col_name, Rect, MAX_COLS, MAX_ROWS};
pub use planilha::sheet::{CellData, Sheet};
pub use planilha::sheet_deps::{is_volatile, DependencyGraph};
pub use celulas::values::{CellError, ErrorCode, Matrix, Value};
