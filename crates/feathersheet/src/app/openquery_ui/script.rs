//! Script da consulta, no espírito do M do Power Query mas com cara de JavaScript:
//!
//! ```text
//! let fonte = lerCsv("vendas.csv");
//! fonte.renomearColuna("DT_GERACAO", "dt");
//! fonte.mudarTipo("dt", "date");
//! ```
//!
//! Não é executado como JavaScript: o texto é lido aqui e vira as mesmas etapas
//! (`Passo`) que o motor Rust roda. As colunas vão pelo nome que têm naquele
//! ponto da consulta (ou pelo número, quando o nome se repete).
use super::*;
use serde_json::{json, Value as Json};

#[derive(Clone, Copy)]
enum Arg {
    /// Valor tal como no JSON das etapas.
    Valor,
    Col,
    /// Lista de colunas (ou `null`, quando o campo é opcional).
    Cols,
    /// Uma coluna ou uma lista (as chaves do `juntar`).
    ColOuCols,
    /// `[["Nome", "asc"], ["Valor", "desc"]]`.
    Ordem,
    /// `[["Valor", "soma", "Total"]]`.
    Agregados,
}
use Arg::*;

/// (tipo da etapa, método no script, campos na ordem dos argumentos).
const METODOS: &[(&str, &str, &[(&str, Arg)])] = &[
    ("filtrar", "filtrar", &[("col", Col), ("criterio", Valor)]),
    ("ordenar", "ordenar", &[("por", Ordem)]),
    ("colunas", "selecionarColunas", &[("manter", Cols)]),
    ("renomear", "renomearColuna", &[("col", Col), ("nome", Valor)]),
    ("tipar", "mudarTipo", &[("col", Col), ("para", Valor)]),
    ("substituir", "substituirValores", &[("col", Col), ("de", Valor), ("para", Valor)]),
    ("substituirErros", "substituirErros", &[("col", Col), ("para", Valor)]),
    ("removerErros", "removerErros", &[("col", Col)]),
    ("mesclar", "mesclarColunas", &[("cols", Cols), ("separador", Valor), ("nome", Valor)]),
    ("derivar", "extrair", &[("col", Col), ("parte", Valor), ("nome", Valor)]),
    ("dividir", "dividirEmLinhas", &[("col", Col), ("separador", Valor)]),
    ("dividirColunas", "dividirColuna", &[("col", Col), ("separador", Valor)]),
    ("distintas", "removerDuplicadas", &[("cols", Cols)]),
    ("agrupar", "agruparPor", &[("por", Cols), ("agregar", Agregados)]),
    ("pivotar", "pivotar", &[("linhas", Cols), ("colunas", Cols), ("valores", Agregados)]),
    ("juntar", "mesclarConsultas", &[("fonte", Valor), ("esquerda", ColOuCols), ("direita", Valor), ("modo", Valor)]),
    ("acrescentar", "acrescentarConsultas", &[("fonte", Valor)]),
    ("despivotar", "despivotarColunas", &[("manter", Cols), ("nomeAtributo", Valor), ("nomeValor", Valor)]),
    ("promoverCabecalho", "usarPrimeiraLinhaComoCabecalho", &[]),
    ("rebaixarCabecalho", "usarCabecalhoComoPrimeiraLinha", &[]),
    ("indice", "adicionarIndice", &[("inicio", Valor), ("incremento", Valor), ("nome", Valor)]),
    ("condicional", "colunaCondicional", &[("nome", Valor), ("col", Col), ("operador", Valor), ("valor", Valor), ("entao", Valor), ("senao", Valor)]),
    ("limite", "manterPrimeiras", &[("quantas", Valor)]),
];

/// Etapa sem método próprio: o objeto JSON inteiro, com as colunas em número.
const METODO_GENERICO: &str = "etapa";

/// `let fonte = …;` conforme a origem da consulta.
fn linha_da_fonte(origem: &str) -> String {
    let s = |t: &str| serde_json::to_string(t).unwrap_or_default();
    let chamada = if let Some(arquivo) = origem.strip_prefix("Arquivo: ") {
        let csv = [".csv", ".tsv", ".txt"].iter().any(|ext| arquivo.to_lowercase().ends_with(ext));
        format!("{}({})", if csv { "lerCsv" } else { "lerArquivo" }, s(arquivo))
    } else if let Some(url) = origem.strip_prefix("Web: ") {
        format!("lerWeb({})", s(url))
    } else if let Some(tabela) = origem.strip_prefix("Tabela: ") {
        format!("lerTabela({})", s(tabela))
    } else if origem == "Entrada manual" {
        "dadosDigitados()".into()
    } else {
        format!("lerAba({})", s(origem))
    };
    format!("let fonte = {chamada};")
}

/// Pelo nome quando ele é único naquele ponto; senão, pelo número.
fn col_para_script(cab: &[String], v: &Json) -> Json {
    let Some(i) = v.as_u64().map(|i| i as usize) else { return v.clone() };
    match cab.get(i) {
        Some(nome) if cab.iter().filter(|c| *c == nome).count() == 1 => json!(nome),
        _ => json!(i),
    }
}

fn col_do_script(cab: &[String], v: &Json) -> Result<Json, String> {
    match v {
        Json::Number(_) => Ok(v.clone()),
        Json::String(nome) => cab.iter().position(|c| c == nome).map(|i| json!(i))
            .ok_or_else(|| trf!("A coluna \"{nome}\" não existe neste ponto da consulta.")),
        _ => Err(tr!("Coluna inválida: use o nome entre aspas ou o número.").into()),
    }
}

fn para_script(arg: Arg, cab: &[String], v: &Json) -> Json {
    let lista = |v: &Json| v.as_array().cloned().unwrap_or_default();
    match arg {
        Valor => v.clone(),
        Col => col_para_script(cab, v),
        Cols | ColOuCols if v.is_array() => Json::Array(lista(v).iter().map(|c| col_para_script(cab, c)).collect()),
        Cols => v.clone(),
        ColOuCols => col_para_script(cab, v),
        Ordem => Json::Array(lista(v).iter().map(|k| json!([
            col_para_script(cab, &k["col"]),
            if k["desc"].as_bool().unwrap_or(false) { "desc" } else { "asc" },
        ])).collect()),
        Agregados => Json::Array(lista(v).iter().map(|a| {
            let mut item = vec![col_para_script(cab, &a["col"]), a["como"].clone()];
            if !a["nome"].is_null() { item.push(a["nome"].clone()); }
            Json::Array(item)
        }).collect()),
    }
}

fn do_script(arg: Arg, cab: &[String], v: &Json) -> Result<Json, String> {
    let lista = |v: &Json| v.as_array().cloned().ok_or_else(|| tr!("Esperava uma lista [ … ].").to_owned());
    Ok(match arg {
        Valor => v.clone(),
        Col => col_do_script(cab, v)?,
        Cols | ColOuCols if v.is_array() => Json::Array(lista(v)?.iter().map(|c| col_do_script(cab, c)).collect::<Result<_, _>>()?),
        Cols if v.is_null() => Json::Null,
        Cols => Json::Array(vec![col_do_script(cab, v)?]),
        ColOuCols => col_do_script(cab, v)?,
        Ordem => Json::Array(lista(v)?.iter().map(|k| {
            let par = k.as_array().ok_or_else(|| tr!("Cada ordem é [\"coluna\", \"asc\" ou \"desc\"].").to_owned())?;
            let col = col_do_script(cab, par.first().unwrap_or(&Json::Null))?;
            Ok(json!({ "col": col, "desc": par.get(1).and_then(Json::as_str) == Some("desc") }))
        }).collect::<Result<_, String>>()?),
        Agregados => Json::Array(lista(v)?.iter().map(|a| {
            let item = a.as_array().ok_or_else(|| tr!("Cada valor é [\"coluna\", \"soma\", \"nome\"].").to_owned())?;
            let col = col_do_script(cab, item.first().unwrap_or(&Json::Null))?;
            Ok(json!({ "col": col, "como": item.get(1).cloned().unwrap_or(json!("soma")),
                "nome": item.get(2).cloned().unwrap_or(Json::Null) }))
        }).collect::<Result<_, String>>()?),
    })
}

/// Uma etapa sobre a tabela do ponto anterior (o mesmo que o motor faz em série).
fn avancar(atual: Tabela, passo: &Passo, fontes: &HashMap<String, Tabela>) -> Result<Tabela, String> {
    executar_query_limitado(atual, std::slice::from_ref(passo), fontes, u64::MAX)
}

/// Gera o script e devolve também o resultado da consulta (as colunas de cada
/// ponto saem da execução, então gerar já é executar).
pub(super) fn gerar(
    origem: &str,
    fonte: &Tabela,
    passos: &[Passo],
    fontes: &HashMap<String, Tabela>,
) -> Result<(String, Tabela), String> {
    let objetos: Vec<Json> = serde_json::from_str(&json_passos::escrever(passos)?)
        .map_err(|e| e.to_string())?;
    let mut linhas = vec![linha_da_fonte(origem)];
    let mut atual = fonte.clone();
    for (passo, objeto) in passos.iter().zip(&objetos) {
        let tipo = objeto["tipo"].as_str().unwrap_or("");
        let linha = match METODOS.iter().find(|(t, _, _)| *t == tipo) {
            Some((_, metodo, campos)) => {
                let mut args: Vec<Json> = campos.iter()
                    .map(|(campo, arg)| para_script(*arg, &atual.cabecalho, &objeto[*campo]))
                    .collect();
                while args.last().is_some_and(Json::is_null) { args.pop(); }
                let args: Vec<String> = args.iter().map(|a| match a.as_f64() {
                    // 5 e não 5.0: o motor guarda número como f64.
                    Some(n) if n.fract() == 0.0 && n.abs() < 1e15 => (n as i64).to_string(),
                    _ => a.to_string(),
                }).collect();
                format!("fonte.{metodo}({});", args.join(", "))
            }
            None => format!("fonte.{METODO_GENERICO}({objeto});"),
        };
        linhas.push(linha);
        atual = avancar(atual, passo, fontes)?;
    }
    Ok((linhas.join("\n") + "\n", atual))
}

/// Comandos de nível de cima, com a linha onde começam. `;` e quebra de linha
/// fora de string e de parênteses encerram o comando; `//` é comentário.
fn comandos(texto: &str) -> Vec<(usize, String)> {
    let mut saida = Vec::new();
    let (mut atual, mut linha, mut inicio) = (String::new(), 1, 1);
    let (mut aspas, mut escape, mut fundo, mut comentario) = (false, false, 0i32, false);
    let fechar = |atual: &mut String, inicio: usize, saida: &mut Vec<(usize, String)>| {
        let t = atual.trim();
        if !t.is_empty() { saida.push((inicio, t.to_owned())); }
        atual.clear();
    };
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' { linha += 1; comentario = false; }
        if comentario { continue; }
        if aspas {
            atual.push(c);
            if escape { escape = false } else if c == '\\' { escape = true } else if c == '"' { aspas = false }
            continue;
        }
        match c {
            '/' if chars.peek() == Some(&'/') => { comentario = true; continue; }
            '"' => aspas = true,
            '(' | '[' | '{' => fundo += 1,
            ')' | ']' | '}' => fundo -= 1,
            ';' if fundo <= 0 => { fechar(&mut atual, inicio, &mut saida); continue; }
            '\n' if fundo <= 0 => { fechar(&mut atual, inicio, &mut saida); continue; }
            _ => {}
        }
        if atual.trim().is_empty() && !c.is_whitespace() { inicio = linha; }
        atual.push(c);
    }
    fechar(&mut atual, inicio, &mut saida);
    saida
}

/// Lê o script de volta para etapas. A linha `let` só dá o nome da variável:
/// trocar a fonte é pela faixa (Obter dados).
pub(super) fn ler(
    texto: &str,
    fonte: &Tabela,
    fontes: &HashMap<String, Tabela>,
) -> Result<Vec<Passo>, String> {
    let mut lista = comandos(texto).into_iter();
    let Some((_, primeiro)) = lista.next() else { return Ok(Vec::new()) };
    let declaracao = ["let ", "const ", "var "].iter().find_map(|p| primeiro.strip_prefix(p))
        .and_then(|resto| resto.split_once('='))
        .map(|(nome, _)| nome.trim().to_owned())
        .filter(|nome| !nome.is_empty())
        .ok_or_else(|| tr!("Linha 1: o script começa com let fonte = …;").to_owned())?;
    let mut passos = Vec::new();
    let mut atual = fonte.clone();
    for (linha, comando) in lista {
        let erro = |msg: &str| trf!("Linha {linha}: {msg}");
        if comando == "executar()" || comando == format!("{declaracao}.executar()") { continue; }
        let chamada = comando.strip_prefix(&format!("{declaracao}."))
            .ok_or_else(|| erro(&trf!("cada etapa é {declaracao}.metodo(…);")))?;
        let (metodo, args) = chamada.split_once('(')
            .and_then(|(m, resto)| Some((m.trim(), resto.trim_end().strip_suffix(')')?)))
            .ok_or_else(|| erro(tr!("faltam os parênteses da chamada.")))?;
        let args: Vec<Json> = serde_json::from_str(&format!("[{args}]"))
            .map_err(|e| erro(&trf!("argumentos inválidos ({e}).")))?;
        let objeto = if metodo == METODO_GENERICO {
            args.into_iter().next().ok_or_else(|| erro(tr!("etapa({ … }) precisa do objeto.")))?
        } else {
            let (tipo, _, campos) = METODOS.iter().find(|(_, m, _)| *m == metodo)
                .ok_or_else(|| erro(&trf!("não conheço o método {metodo}.")))?;
            if args.len() > campos.len() {
                return Err(erro(&trf!("{metodo} recebe no máximo {} argumentos.", campos.len())));
            }
            let mut objeto = serde_json::Map::new();
            objeto.insert("tipo".into(), json!(tipo));
            for (i, (campo, arg)) in campos.iter().enumerate() {
                let v = do_script(*arg, &atual.cabecalho, args.get(i).unwrap_or(&Json::Null))
                    .map_err(|e| erro(&e))?;
                objeto.insert((*campo).into(), v);
            }
            Json::Object(objeto)
        };
        let passo = json_passos::ler(&Json::Array(vec![objeto]).to_string())
            .map_err(|e| erro(&e))?
            .pop().ok_or_else(|| erro(tr!("etapa vazia.")))?;
        atual = avancar(atual, &passo, fontes).map_err(|e| erro(&e))?;
        passos.push(passo);
    }
    Ok(passos)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn fonte() -> Tabela {
        Tabela {
            cabecalho: vec!["Regiao".into(), "Ano".into(), "MWh".into()],
            linhas: vec![vec![Value::Text("Norte".into()), Value::Number(2025.0), Value::Number(10.0)]],
        }
    }

    #[test]
    fn script_usa_let_e_nomes_e_volta_para_as_mesmas_etapas() {
        let passos = vec![
            Passo::Renomear { col: 0, nome: "Total \"R$\"".into() },
            Passo::Filtrar { col: 0, criterio: Value::Text(">100".into()) },
            Passo::Ordenar { por: vec![ChaveOrdenacao { col: 2, desc: Some(true) }] },
            Passo::Agrupar { por: vec![1], agregar: vec![Agregado { col: 2, como: Agregacao::Soma, nome: Some("Total".into()) }] },
            Passo::Limite { quantas: 5.0 },
        ];
        let (texto, resultado) = gerar("Arquivo: vendas.csv", &fonte(), &passos, &HashMap::new()).unwrap();
        assert!(texto.starts_with("let fonte = lerCsv(\"vendas.csv\");\n"));
        assert!(texto.contains("fonte.renomearColuna(\"Regiao\", \"Total \\\"R$\\\"\");"), "{texto}");
        assert!(texto.contains("fonte.filtrar(\"Total \\\"R$\\\"\", \">100\");"), "{texto}");
        assert!(texto.contains("fonte.ordenar([[\"MWh\",\"desc\"]]);"), "{texto}");
        assert!(texto.contains("fonte.agruparPor([\"Ano\"], [[\"MWh\",\"soma\",\"Total\"]]);"), "{texto}");
        assert_eq!(resultado.cabecalho, vec!["Ano".to_string(), "Total".into()]);
        let lidos = ler(&texto, &fonte(), &HashMap::new()).unwrap();
        assert_eq!(json_passos::escrever(&lidos).unwrap(), json_passos::escrever(&passos).unwrap());
    }

    #[test]
    fn script_escrito_a_mao_com_comentario_e_outro_nome_de_variavel() {
        let texto = "// minha consulta\nconst dados = lerCsv(\"x.csv\")\ndados.renomearColuna(\"Ano\", \"Exercício\") // renomeia\ndados.mudarTipo(\"Exercício\", \"texto\");\ndados.executar();";
        let passos = ler(texto, &fonte(), &HashMap::new()).unwrap();
        assert!(matches!(&passos[0], Passo::Renomear { col: 1, nome } if nome == "Exercício"));
        assert!(matches!(&passos[1], Passo::Tipar { col: 1, para } if para == "texto"));
    }

    #[test]
    fn erros_dizem_a_linha() {
        let erro = ler("let fonte = lerCsv(\"x\");\nfonte.renomearColuna(\"Nada\", \"y\");", &fonte(), &HashMap::new()).unwrap_err();
        assert!(erro.starts_with("Linha 2:") && erro.contains("\"Nada\""), "{erro}");
        let erro = ler("let fonte = lerCsv(\"x\");\n\nfonte.voar();", &fonte(), &HashMap::new()).unwrap_err();
        assert!(erro.starts_with("Linha 3:") && erro.contains("voar"), "{erro}");
        assert!(ler("fonte.voar();", &fonte(), &HashMap::new()).unwrap_err().starts_with("Linha 1:"));
    }

    #[test]
    fn coluna_de_nome_repetido_vai_pelo_numero_e_etapa_sem_metodo_vai_inteira() {
        let fonte = Tabela { cabecalho: vec!["A".into(), "A".into()], linhas: Vec::new() };
        let passos = vec![Passo::Renomear { col: 1, nome: "B".into() }];
        let (texto, _) = gerar("Planilha1", &fonte, &passos, &HashMap::new()).unwrap();
        assert!(texto.contains("fonte.renomearColuna(1, \"B\");"), "{texto}");
        assert!(texto.starts_with("let fonte = lerAba(\"Planilha1\");"));
        let lidos = ler("let f = lerAba(\"x\");\nf.etapa({\"tipo\":\"renomear\",\"col\":0,\"nome\":\"Z\"});", &fonte, &HashMap::new()).unwrap();
        assert!(matches!(&lidos[0], Passo::Renomear { col: 0, nome } if nome == "Z"));
    }
}
