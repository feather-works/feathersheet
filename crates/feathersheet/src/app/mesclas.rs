//! Mesclagem: a regra e o JSON do `sheet.mc` moram em `feathersheet_io::mesclas`
//! (o MCP usa a mesma); aqui ficam só as variações do botão Mesclar ▾.
mod modos;
pub use modos::ModoMescla;

pub(crate) use feathersheet_io::mesclas::{cruzam, desmesclar, exatamente_mesclada, expandir, ler, mesclar, refluir};
