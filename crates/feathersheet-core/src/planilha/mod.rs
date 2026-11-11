//! A pasta de trabalho e a aba: guardar, recalcular, derramar, mover.

pub mod agregado;
pub mod assincrono;
pub mod bloco;
pub mod calculo_manual;
pub(crate) mod calculo_paralelo;
pub mod column_store;
pub mod data_table;
pub mod deslocar_celulas;
pub mod faixa_lida;
pub mod frame;
pub mod image;
pub mod iterativo;
pub mod impressao;
pub mod lista_validacao;
pub mod mover;
pub mod sheet;
pub mod sheet_deps;
pub mod sheet_spill;
pub mod subtotais;
pub mod textos_crus;
pub mod vinculo;
pub mod workbook;
