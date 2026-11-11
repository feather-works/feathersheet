//! Adaptador de api-estrutura.ts. O motor de dimensões e undo já existe no
//! Sheet Rust: definir_dimensao e alterar_visibilidade_dimensoes.
//! Esta camada só interpreta endereços VBA (base 1) e valida argumentos JSON.
use feathersheet_core::planilha::sheet::Sheet;
use serde_json::Value as Json;
use crate::enderecos;

pub(crate) enum Acao {
    Definir { coluna: bool, indice: u32, pixels: u32 },
    Ocultar { coluna: bool, indice: u32, esconder: bool },
}

fn indice_coluna(valor: &Json) -> Result<u32, String> {
    if let Some(texto) = valor.as_str() {
        return enderecos::coluna_texto(texto);
    }
    let numero = valor.as_u64().ok_or_else(|| feathersheet_core::tr!("Coluna deve ser letra ou número inteiro de 1 a 16384"))?;
    let numero = u32::try_from(numero).map_err(|_|feathersheet_core::tr!("Coluna fora dos limites"))?;
    enderecos::coluna_numero(numero)
}

fn indice_linha(valor: &Json) -> Result<u32, String> {
    let numero = valor.as_u64().ok_or_else(|| feathersheet_core::tr!("Linha deve ser número inteiro a partir de 1"))?;
    let numero = u32::try_from(numero).map_err(|_|feathersheet_core::tr!("Linha fora dos limites"))?;
    enderecos::linha_numero(numero)
}

pub(crate) fn preparar(op: &str, dados: &Json) -> Result<Acao, String> {
    let coluna = matches!(op, "setColumnWidth" | "hideColumn");
    let indice = if coluna { indice_coluna(&dados[1])? }
        else { indice_linha(&dados[1])? };
    if matches!(op, "hideRow" | "hideColumn") {
        let esconder = if dados[2].is_null() { true }
            else { dados[2].as_bool().ok_or_else(|| feathersheet_core::tr!("Ocultar exige true ou false"))? };
        return Ok(Acao::Ocultar { coluna, indice, esconder });
    }
    if !matches!(op, "setRowHeight" | "setColumnWidth") {
        return Err(feathersheet_core::trf!("Operação de dimensão desconhecida: {op}"));
    }
    let px = dados[2].as_f64().ok_or_else(|| feathersheet_core::tr!("Dimensão deve ser número de pixels"))?;
    if !px.is_finite() || !(0.0..=10_000.0).contains(&px) {
        return Err(feathersheet_core::tr!("Dimensão deve estar entre 0 e 10000 pixels").into());
    }
    // O modelo do core persiste dimensões inteiras. Arredondar aqui,
    // explicitamente, antes de mandar ao motor comum.
    // Medidas positivas fracionárias não podem virar zero ao arredondar:
    // zero significa "ocultar" no core e não deve surgir de 0.4 px.
    let pixels = if px == 0.0 { 0 } else { px.round().max(1.0) as u32 };
    Ok(Acao::Definir { coluna, indice, pixels })
}

/// Precisa ser executada DENTRO da transação da macro. Não cria
/// mecanismo paralelo de dimensões nem modifica diretamente HashMaps.
/// Retorna true quando a planilha de fato foi modificada.
pub(crate) fn aplicar(sheet: &mut Sheet, acao: Acao) -> Result<bool, String> {
    match acao {
        Acao::Definir { coluna, indice, pixels: 0 } |
        Acao::Ocultar { coluna, indice, esconder: true } =>
            Ok(sheet.alterar_visibilidade_dimensoes(coluna,true,indice,indice)? > 0),
        Acao::Definir { coluna, indice, pixels } =>
            Ok(sheet.definir_dimensao(coluna,indice,pixels)),
        Acao::Ocultar { coluna, indice, esconder: false } =>
            Ok(sheet.alterar_visibilidade_dimensoes(coluna,false,indice,indice)? > 0),
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn parseia_indices_vba_e_validacao_previa() {
        assert!(matches!(
            preparar("setColumnWidth",&serde_json::json!([0,"AA",134])),
            Ok(Acao::Definir {coluna:true,indice:26,pixels:134})
        ));
        assert!(preparar("setRowHeight",&serde_json::json!([0,0,20])).is_err());
        assert!(preparar("setColumnWidth",&serde_json::json!([0,"XFE",20])).is_err());
        assert!(matches!(
            preparar("setRowHeight",&serde_json::json!([0,1,0.3])),
            Ok(Acao::Definir { coluna:false, indice:0, pixels:1 })
        ));
        assert!(preparar("setRowHeight",&serde_json::json!([0,1,-1])).is_err());
        assert!(preparar("setRowHeight",&serde_json::json!([0,1,10001])).is_err());
        assert!(preparar("hideRow",&serde_json::json!([0,1,"yes"])).is_err());
    }
    #[test]
    fn reusa_dimensoes_e_undo_do_core() {
        let mut s=Sheet::new("A");
        s.abrir_transacao();
        assert!(aplicar(&mut s,preparar("setRowHeight",
            &serde_json::json!([0,2,42])).unwrap()).unwrap());
        assert!(aplicar(&mut s,preparar("setColumnWidth",
            &serde_json::json!([0,"C",108])).unwrap()).unwrap());
        s.fechar_transacao();
        assert_eq!(s.row_heights.get(&1),Some(&42));
        assert_eq!(s.col_widths.get(&2),Some(&108));
        assert!(s.undo());
        assert!(!s.row_heights.contains_key(&1));
        assert!(!s.col_widths.contains_key(&2));
    }
    #[test]
    fn ocultar_e_reexibir_mantem_dados_e_usa_undo_do_core() {
        let mut s=Sheet::new("A");
        s.set_cell(0,0,"valioso");
        s.abrir_transacao();
        assert!(aplicar(&mut s,preparar("hideColumn",
            &serde_json::json!([0,1])).unwrap()).unwrap());
        s.fechar_transacao();
        assert_eq!(s.col_widths.get(&0),Some(&0));
        assert_eq!(s.get_raw(0,0),"valioso");
        assert!(s.undo());
        assert_eq!(s.col_widths.get(&0),None);
    }
}
