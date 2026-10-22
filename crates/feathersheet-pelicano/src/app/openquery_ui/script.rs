//! Migração do gerador de script de src/openquery/core/script-generator.ts.
//! Apenas exibição: o pipeline é executado pelo motor Rust, não por eval/JS.
use super::*;
use serde_json::Value as Json;

fn texto(v: &Json) -> String {
    serde_json::to_string(v.as_str().unwrap_or("")).unwrap_or_else(|_| "\"\"".into())
}

fn nome_coluna(cabecalho: &[String], valor: &Json) -> String {
    let indice = valor.as_u64().unwrap_or(0) as usize;
    let nome = cabecalho.get(indice).cloned()
        .unwrap_or_else(|| format!("coluna_{}", indice + 1));
    serde_json::to_string(&nome).unwrap_or_else(|_| "\"\"".into())
}

fn nome_colunas(cabecalho: &[String], valores: &Json) -> String {
    let itens = valores.as_array().map(Vec::as_slice).unwrap_or(&[]);
    let nomes: Vec<String> = itens.iter().map(|c| nome_coluna(cabecalho, c)).collect();
    format!("[{}]", nomes.join(", "))
}

/// Reproduz as operações declarativas exportadas pelo antigo gerador TS.
/// Texto ilustrativo para inspeção; a API JavaScript fluente não é executada
/// nem usada como origem da verdade. Passos e estado permanecem em Rust.
pub(super) fn gerar(
    nome: &str,
    cabecalho: &[String],
    passos: &[Passo],
) -> Result<String, String> {
    let passos_json: Vec<Json> = serde_json::from_str(&json_passos::escrever(passos)?)
        .map_err(|e| e.to_string())?;
    let mut cols = cabecalho.to_vec();
    let mut linhas = vec![
        "// OpenQuery • script de leitura (não executável nesta janela)".to_string(),
        "// Execução real: feathersheet-core / consulta / query".into(),
        "import { OpenQuery } from '@feathersheet/query';".into(),
        String::new(),
        "export function executarPipeline() {".into(),
        format!("  return OpenQuery.de({})", serde_json::to_string(nome)
            .map_err(|e| e.to_string())?),
    ];
    for passo in &passos_json {
        let tipo = passo["tipo"].as_str().unwrap_or("");
        let linha = match tipo {
            "filtrar" => format!("    .filtrar({}, {})",
                nome_coluna(&cols, &passo["col"]), passo["criterio"]),
            "ordenar" => {
                let keys = passo["por"].as_array().map(Vec::as_slice).unwrap_or(&[]);
                let itens = keys.iter().map(|chave| format!("{}, {}",
                    nome_coluna(&cols, &chave["col"]),
                    chave["desc"].as_bool().unwrap_or(false)))
                    .collect::<Vec<_>>().join(", ");
                format!("    .ordenar({itens})")
            }
            "colunas" => {
                let nomes = nome_colunas(&cols, &passo["manter"]);
                cols = passo["manter"].as_array().map(Vec::as_slice).unwrap_or(&[])
                    .iter().map(|c| {
                        let indice = c.as_u64().unwrap_or(0) as usize;
                        cols.get(indice).cloned().unwrap_or_else(|| format!("coluna_{}",indice+1))
                    }).collect();
                format!("    .selecionarColunas({nomes})")
            }
            "renomear" => {
                let anterior = nome_coluna(&cols,&passo["col"]);
                let indice = passo["col"].as_u64().unwrap_or(0) as usize;
                if let Some(col) = cols.get_mut(indice) {
                    *col = passo["nome"].as_str().unwrap_or("").to_owned();
                }
                format!("    .renomearColuna({anterior}, {})",texto(&passo["nome"]))
            }
            "tipar" => format!("    .mudarTipo({}, {})",
                nome_coluna(&cols,&passo["col"]),texto(&passo["para"])),
            "substituir" => format!("    .substituirValores({}, {}, {})",
                nome_coluna(&cols,&passo["col"]),passo["de"],passo["para"]),
            "substituirErros" => format!("    .substituirErros({}, {})",
                nome_coluna(&cols,&passo["col"]),passo["para"]),
            "removerErros" => format!("    .removerErros({})",
                nome_coluna(&cols,&passo["col"])),
            "mesclar" => format!("    .mesclarColunas({}, {})",
                nome_colunas(&cols,&passo["cols"]),texto(&passo["separador"])),
            "juntar" => format!("    .mesclarConsultas({}, {})",
                texto(&passo["fonte"]),passo.get("modo")
                    .and_then(Json::as_str).map(|s|serde_json::to_string(s).unwrap_or_default())
                    .unwrap_or_else(||"\"esquerda\"".into())),
            "acrescentar" => format!("    .acrescentarConsultas({})",texto(&passo["fonte"])),
            "dividir" => format!("    .dividirColuna({}, {})",
                nome_coluna(&cols,&passo["col"]),texto(&passo["separador"])),
            "distintas" => "    .removerDuplicadas()".into(),
            "despivotar" => "    .despivotarColunas()".into(),
            "promoverCabecalho" => "    .promoverPrimeiraLinhaComoCabecalho()".into(),
            "rebaixarCabecalho" => "    .usarCabecalhoComoPrimeiraLinha()".into(),
            "indice" => format!("    .adicionarColunaIndice({}, {})",
                passo.get("inicio").filter(|v|!v.is_null()).unwrap_or(&Json::Null),
                passo.get("incremento").filter(|v|!v.is_null()).unwrap_or(&Json::Null)),
            "condicional" => format!("    .adicionarColunaCondicional({}, {}, {}, {})",
                texto(&passo["nome"]),texto(&passo["operador"]),
                texto(&passo["valor"]),texto(&passo["entao"])),
            "limite" => format!("    .manterPrimeirasLinhas({})",passo["quantas"]),
            // O formato fluente é apenas ilustrativo; operações ainda sem método
            // dedicado conservam TODOS os argumentos, em vez de perder
            // configurações de agrupamento, pivot e afins.
            _ => format!("    .transformar({})",
                serde_json::to_string(passo).map_err(|e| e.to_string())?),
        };
        linhas.push(linha);
    }
    linhas.push("    .executar();".into());
    linhas.push("}".into());
    Ok(linhas.join("\n"))
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn gera_script_escape_e_mapeia_renomeacao_sem_executar_js() {
        let passos=vec![
            Passo::Renomear { col:0,nome:"Total \"R$\"".into() },
            Passo::Filtrar { col:0,criterio:Value::Text(">100".into()) },
            Passo::Limite { quantas:5.0 },
        ];
        let js=gerar("Nome \"especial\"", &["MWh".into()], &passos).unwrap();
        assert!(js.contains("OpenQuery.de(\"Nome \\\"especial\\\"\")"));
        assert!(js.contains("filtrar(\"Total \\\"R$\\\"\", \">100\")"));
        assert!(js.contains("manterPrimeirasLinhas(5"));
    }

    #[test]
    fn script_de_operacao_sem_atalho_exibe_parametros_completos() {
        let passos = vec![Passo::Pivotar {
            linhas: vec![0],
            colunas: vec![1],
            valores: vec![Agregado {
                col: 2,
                como: Agregacao::do_str("soma"),
                nome: Some("Total".into()),
            }],
        }];
        let gerado = gerar("Consulta", &["Regiao".into(), "Ano".into(), "MWh".into()], &passos).unwrap();
        assert!(gerado.contains(".transformar({"));
        assert!(gerado.contains(r#""pivotar""#));
        assert!(gerado.contains(r#""linhas":[0]"#));
        assert!(gerado.contains(r#""colunas":[1]"#));
        assert!(gerado.contains(r#""Total""#));
    }
}
