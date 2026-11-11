//! Tabela dinâmica: montar, desenhar e os campos calculados.

pub mod campo_calculado;
pub mod campos;
pub mod dinamica_layout;
pub mod exibicao;
pub mod pivot;
pub(crate) mod acumulador;
#[cfg(test)]
mod testes_exibicao;

/// O rótulo da linha (e da coluna) de total geral, na língua do app.
pub fn total_geral() -> String { crate::tr!("Total Geral") }

/// O texto de uma célula é um rótulo de total geral? Vale a língua do app e o português,
/// porque uma dinâmica montada antes de trocar a língua fica com o rótulo antigo até atualizar.
pub fn e_total_geral(texto: &str) -> bool {
    texto.starts_with(&total_geral()) || texto.starts_with("Total Geral")
}
