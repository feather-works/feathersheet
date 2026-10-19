//! As funções da planilha, a ajuda de cada uma e o que elas usam.

pub mod fn_compat;
pub mod fn_complexo;
pub mod fn_cupom;
pub mod fn_lambda;
pub mod fn_referencia;
pub mod fn_regressao;
/// Funções do FeatherSheet que o Excel não tem (nome internacional). Ao
/// exportar .xlsx, a interface avisa: no Excel essas células mostram #NOME?.
/// Função nova sem equivalente no Excel entra aqui.
pub const FORA_DO_EXCEL: &[&str] = &["COUNTUNIQUE", "SUMBYCOLOR", "COUNTBYCOLOR", "IMAGE.BASE64", "IMPORTDATA", "IMPORTJSON", "FETCHTEXT", "JOIN", "REGEXMATCH"];
pub mod fn_conversao;
pub mod fn_cor;
pub mod fn_date;
pub mod fn_db;
pub mod fn_dist;
pub mod fn_eng;
pub mod fn_extra;
pub mod fn_fin;
pub mod fn_helpers;
pub mod fn_info;
pub mod fn_lookup;
pub mod fn_lookup_matrix;
pub mod fn_math;
pub mod fn_stats;
pub mod fn_subtotal;
pub mod fn_text;
pub mod fn_titulos;
pub mod function_help;
pub mod function_help_data;
pub mod functions;
pub mod imagem_base64;
pub mod regex;

#[cfg(test)]
mod testes_fora_do_excel {
    #[test]
    fn toda_funcao_fora_do_excel_existe_no_catalogo() {
        for nome in super::FORA_DO_EXCEL {
            assert!(super::function_help::CATEGORIES.iter().any(|(_, nomes)| nomes.contains(nome)), "{nome}");
        }
    }
}
