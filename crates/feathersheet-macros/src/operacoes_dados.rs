//! Parte nativa da API de dados das macros (src/macros/api-dados.ts).
//! O JavaScript apenas informa os argumentos: planejamento e escrita são Rust.
use feathersheet_core::{
    celulas::refs::{col_index, parse_range_ref, Rect, MAX_COLS, MAX_ROWS},
    operacoes::ops::{self, OpcoesDivisao, PlanoDeOrdem, PlanoDeRepetidas, SortKey},
    planilha::{sheet::Sheet, workbook::Workbook},
};
use serde_json::{json, Value as Json};

pub(crate) enum Mudanca {
    Ordenar(PlanoDeOrdem),
    RemoverRepetidas(PlanoDeRepetidas),
    /// Texto para colunas: a primeira coluna do destino e, por linha do
    /// destino, os pedaços (mesmo motor do assistente da faixa).
    Dividir { c0: u32, linhas: Vec<(u32, Vec<String>)> },
}

pub(crate) struct Preparacao {
    pub resposta: Json,
    pub mudanca: Option<Mudanca>,
}

/// Coluna da API JS é letra ("B") ou número (2), ambos absolutos.
/// Números seguem o Excel: base 1; o motor Rust usa base 0.
fn coluna(v: &Json) -> Result<u32, String> {
    if let Some(nome) = v.as_str() {
        return col_index(nome.trim()).ok_or_else(|| feathersheet_core::trf!("Coluna inválida: {nome}"));
    }
    if let Some(numero) = v.as_u64() {
        if numero > 0 && numero <= u64::from(MAX_COLS) {
            return Ok(numero as u32 - 1);
        }
    }
    Err(feathersheet_core::trf!("Coluna inválida: {v} (use letra ou número a partir de 1)"))
}

fn dentro(v: &Json, area: &Rect) -> Result<u32, String> {
    let col = coluna(v)?;
    if col < area.c0 || col > area.c1 {
        return Err(feathersheet_core::trf!("A coluna {} não faz parte da área selecionada",
            feathersheet_core::celulas::refs::col_name(col),
        ));
    }
    Ok(col)
}

/// Valida os argumentos antes de tocar na pasta. Lê valores calculados com
/// Workbook::valor, para que fórmulas entre abas entrem corretamente na ordem.
pub(crate) fn preparar(
    pasta: &Workbook,
    aba: usize,
    area: &Rect,
    op: &str,
    dados: &Json,
) -> Result<Preparacao, String> {
    let folha = pasta.aba(aba).ok_or("Aba inexistente")?;
    let max_row = folha.sheet.used_rect().r1;
    match op {
        "sort" => {
            let entradas = dados[2].as_array()
                .ok_or("sort: informe as chaves como [{col:'B',order:'asc'}]")?;
            if entradas.is_empty() || entradas.len() > 32 {
                return Err(feathersheet_core::tr!("sort: informe entre 1 e 32 colunas de ordenação").into());
            }
            let mut chaves = Vec::with_capacity(entradas.len());
            for item in entradas {
                let col = dentro(&item["col"], area)?;
                let sentido = item["order"].as_str().unwrap_or("asc");
                if sentido != "asc" && sentido != "desc" {
                    return Err(feathersheet_core::trf!("sort: ordem inválida: {sentido} (use asc ou desc)"));
                }
                chaves.push(SortKey { col, ascending: sentido != "desc" });
            }
            let tem_cabecalho = dados[3]["header"].as_bool().unwrap_or(false);
            let mudanca = (!folha.sheet.is_empty())
                .then(|| ops::planejar_ordenacao(
                    area.clone(), &chaves, tem_cabecalho, max_row,
                    |r, c| pasta.valor(aba, r, c),
                ))
                .flatten()
                .map(Mudanca::Ordenar);
            Ok(Preparacao { resposta: Json::Null, mudanca })
        }
        "removeDuplicates" => {
            let cols = match &dados[2] {
                Json::Null => None,
                Json::Array(v) => {
                    let mut colunas = Vec::with_capacity(v.len());
                    for col in v {
                        colunas.push(dentro(col, area)?);
                    }
                    Some(colunas)
                }
                _ => return Err(feathersheet_core::tr!("removeDuplicates: colunas deve ser uma lista ou omitido").into()),
            };
            let tem_cabecalho = dados[3]["header"].as_bool().unwrap_or(false);
            let plano = (!folha.sheet.is_empty())
                .then(|| ops::planejar_repetidas(
                    area.clone(), tem_cabecalho, cols.as_deref(), max_row,
                    |r, c| pasta.valor(aba, r, c),
                ))
                .flatten();
            let quantidade = plano.as_ref().map_or(0, |p| p.removidas);
            Ok(Preparacao {
                resposta: json!(quantidade),
                mudanca: plano.map(Mudanca::RemoverRepetidas),
            })
        }
        "textToColumns" => {
            // Uma coluna só, como o Excel ("só pode converter uma coluna por vez").
            if area.c0 != area.c1 {
                return Err(feathersheet_core::tr!("textToColumns: selecione uma coluna só").into());
            }
            let o = &dados[2];
            let mut delimitadores: Vec<String> = match &o["delimiters"] {
                Json::Null => vec![],
                Json::String(s) => vec![s.clone()],
                Json::Array(v) => v.iter().filter_map(|d| d.as_str().map(str::to_owned)).collect(),
                _ => return Err(feathersheet_core::tr!("textToColumns: delimiters deve ser texto ou lista de textos").into()),
            };
            delimitadores.retain(|d| !d.is_empty());
            delimitadores.truncate(16);
            if delimitadores.is_empty() {
                return Err(feathersheet_core::tr!("textToColumns: informe ao menos um delimitador").into());
            }
            let qualificador = match &o["qualifier"] {
                Json::Null => Some('"'),
                Json::String(s) if s.is_empty() => None,
                Json::String(s) => s.chars().next(),
                _ => None,
            };
            let opcoes = OpcoesDivisao {
                delimitadores,
                consecutivos_como_um: o["consecutive"].as_bool().unwrap_or(false),
                qualificador_texto: qualificador,
            };
            // Destino: o canto onde a primeira parte cai (padrão: a própria origem).
            let (dr, c0) = match o["destination"].as_str() {
                None => (0i64, area.c0),
                Some(d) => {
                    let alvo = parse_range_ref(d.trim().trim_start_matches('=').replace('$', "").as_str())
                        .ok_or_else(|| feathersheet_core::trf!("textToColumns: destino inválido: {d}"))?;
                    if alvo.sheet.is_some() {
                        return Err(feathersheet_core::tr!("textToColumns: o destino fica na mesma aba").into());
                    }
                    (i64::from(alvo.r0) - i64::from(area.r0), alvo.c0)
                }
            };
            let linhas = (!folha.sheet.is_empty())
                .then(|| ops::planejar_divisao(area.clone(), &opcoes, max_row, |r, c| pasta.valor(aba, r, c)))
                .flatten()
                .unwrap_or_default();
            let mut destino = Vec::with_capacity(linhas.len());
            for (r, partes) in linhas {
                let r = u32::try_from(i64::from(r) + dr).ok().filter(|&r| r < MAX_ROWS)
                    .ok_or_else(|| feathersheet_core::tr!("textToColumns: o destino passa do limite de linhas"))?;
                if u64::from(c0) + partes.len() as u64 > u64::from(MAX_COLS) {
                    return Err(feathersheet_core::tr!("textToColumns: o destino passa do limite de colunas").into());
                }
                destino.push((r, partes));
            }
            let quantidade = destino.len();
            Ok(Preparacao {
                resposta: json!(quantidade),
                mudanca: (quantidade > 0).then_some(Mudanca::Dividir { c0, linhas: destino }),
            })
        }
        _ => Err(feathersheet_core::trf!("Operação de dados desconhecida: {op}")),
    }
}

/// Chamada dentro da transação geral da macro; operações nativas podem abrir
/// subtransações, fechadas apenas quando a macro inteira for concluída.
pub(crate) fn aplicar(sheet: &mut Sheet, area: &Rect, mudanca: Mudanca) {
    match mudanca {
        Mudanca::Ordenar(plano) => ops::aplicar_ordenacao(sheet, area.c0, area.c1, &plano),
        Mudanca::RemoverRepetidas(plano) => ops::aplicar_repetidas(sheet, area.c0, area.c1, &plano),
        Mudanca::Dividir { c0, linhas } => { ops::aplicar_divisao(sheet, c0, &linhas, None); }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn colunas_da_macro_sao_base_um() {
        assert_eq!(coluna(&json!("XFD")).unwrap(), MAX_COLS - 1);
        assert_eq!(coluna(&json!(2)).unwrap(), 1);
        assert!(coluna(&json!(0)).is_err());
        assert!(coluna(&json!("XFE")).is_err());
    }

    #[test]
    fn chaves_fora_da_area_nao_sao_aceitas() {
        let wb = Workbook::new();
        let argumentos = json!([0, "A1:B2", [{"col": "C"}], {"header": true}]);
        assert!(preparar(&wb, 0, &Rect::new(0,0,1,1), "sort", &argumentos).is_err());
    }
}
