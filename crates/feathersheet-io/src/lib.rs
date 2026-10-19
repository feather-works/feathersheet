//! `feathersheet-io` — leitura e gravação de arquivo, em Rust.
//!
//! Local previsto em `docs/ARQUITETURA.md`: "é para onde o `src-tauri/src/xlsx`
//! e o `fshz.rs` se mudam, e onde o Arrow entra". [`fshz_write`] grava o
//! `.fshz` colunar (assinatura `FSH2`); [`fshz_read`] lê de volta;
//! [`recents`] é a lista de arquivos recentes; [`csv`] corta CSV/TSV. O
//! `.xlsx` continua onde está até ter razão para se mudar para cá também —
//! mover por mover não é meta.

/// Vigia de memória RAM da suíte (limite das Configurações).
pub use feather_io::memoria;
pub mod arquivo;
mod arrow_fluxo;
pub mod backup_policy;
pub mod backup_snapshot;
pub mod csv;
mod csv_colunas;
pub mod csv_workbook;
pub mod json_workbook;
pub mod filtro_json;
pub mod mesclas;
pub mod grafico_compartilhado;
pub mod grafico_migrado;
pub mod graficos_salvos;
pub mod da_workbook;
pub mod externos_guardados;
pub mod enxugar;
pub mod para_workbook;
pub mod pendentes;
pub mod fshz_read;
pub mod progresso;
pub mod fshz_write;
/// Tinta da aba Desenhar: âncoras nas células, `.fshz` e desenho do `.xlsx`.
pub mod tinta_aba;
/// Objetos soltos sobre a grade (formas, ícones, SmartArt) do campo `ob`.
pub mod objetos_aba;
pub mod impressao_pdf;
pub mod parquet_read;
pub mod recents;
pub mod web;

pub mod xlsx_read;
pub mod xlsx_write;
mod xlsx_validacao_custom;
mod xlsx_write_condicional;
mod xlsx_write_futuras;
mod xlsx_write_autofiltro;
mod xlsx_write_comentarios;
mod xlsx_write_dinamica;
mod xlsx_write_graficos;
mod xlsx_write_vinculos;
pub mod zip_write;
