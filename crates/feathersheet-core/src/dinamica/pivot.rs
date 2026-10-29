//! Tabela Dinâmica — agrupamento, agravo e pivotação de dados da planilha em Rust.
//!
//! Tradução direta e otimizada de `src/core/pivot.ts`.

use std::collections::HashMap;

use crate::celulas::refs::Rect;
use super::acumulador::Medida;
use crate::celulas::values::{to_text, Matrix, Value};

/// Operação de agregação para medidas na Tabela Dinâmica.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgregacaoOp {
    Soma,
    Contagem,
    Media,
    Min,
    Max,
    DesvioPadrao,
    Moda,
}

impl AgregacaoOp {
    pub fn do_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "contagem" | "conta" | "contar" | "count" | "contarvalores" => AgregacaoOp::Contagem,
            "media" | "média" | "avg" | "average" => AgregacaoOp::Media,
            "min" | "mín" | "minimo" | "mínimo" => AgregacaoOp::Min,
            "max" | "máx" | "maximo" | "máximo" => AgregacaoOp::Max,
            "desviopadrao" | "desvio_padrao" | "desvio" | "stdev" => AgregacaoOp::DesvioPadrao,
            "moda" | "mode" => AgregacaoOp::Moda,
            _ => AgregacaoOp::Soma,
        }
    }
}

/// Modo de agrupamento para colunas de data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgruparData {
    Mes,
    Ano,
    Trimestre,
}

impl AgruparData {
    pub fn do_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "mes" | "mês" | "month" => Some(AgruparData::Mes),
            "ano" | "year" => Some(AgruparData::Ano),
            "trimestre" | "quarter" => Some(AgruparData::Trimestre),
            _ => None,
        }
    }
}

/// Medida agregada (coluna + operação).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MedidaDinamica {
    pub coluna: usize,
    pub op: AgregacaoOp,
}

/// Configuração completa da Tabela Dinâmica.
#[derive(Debug, Clone)]
pub struct DinamicaConfig {
    pub origem: Rect,
    pub linhas: Vec<usize>,
    pub colunas: Vec<usize>,
    pub valores: Vec<MedidaDinamica>,
    pub agrupar_data: Option<AgruparData>,
}

/// [`parse_date_key`] de um valor já lido (dono): o texto comum vira a
/// chave sem cópia — é o caso de quase toda célula de rótulo.
pub(crate) fn chave_de(val: Value, group_mode: Option<AgruparData>) -> String {
    match (group_mode, val) {
        (None, Value::Text(t)) if !t.trim().is_empty() => t,
        (_, val) => parse_date_key(&val, group_mode),
    }
}

pub(crate) fn parse_date_key(val: &Value, group_mode: Option<AgruparData>) -> String {
    if matches!(val, Value::Empty) {
        return "(Vazio)".to_string();
    }
    let text = to_text(val);
    if text.trim().is_empty() {
        return "(Vazio)".to_string();
    }

    if let Some(mode) = group_mode {
        // Tentar extrair data no formato YYYY-MM ou YYYY-MM-DD
        let trimmed = text.trim();
        if trimmed.len() >= 7 {
            let bytes = trimmed.as_bytes();
            if bytes.len() >= 7
                && bytes[0..4].iter().all(|b| b.is_ascii_digit())
                && bytes[4] == b'-'
                && bytes[5..7].iter().all(|b| b.is_ascii_digit())
            {
                let y = &trimmed[0..4];
                if let Ok(m @ 1..=12) = trimmed[5..7].parse::<u32>() {
                    let q = (m + 2) / 3;
                    match mode {
                        AgruparData::Ano => return y.to_string(),
                        AgruparData::Trimestre => return format!("T{q} {y}"),
                        AgruparData::Mes => return format!("{y}-{m:02}"),
                    }
                }
            }
        }

        // Seriais vindos do XLSX podem ser número ou texto numérico.
        // O TypeScript usa Number(text) para datas importadas como string.
        let serial = match val {
            Value::Number(num) => Some(*num),
            Value::Text(raw) => raw.trim().parse::<f64>().ok(),
            _ => None,
        };
        if let Some(num) = serial.filter(|n| n.is_finite() && *n > 0.0 && *n < 3_000_000.0) {
            let (y, m, _d) = excel_serial_to_ymd(num.round() as i64);
            let q = (m + 2) / 3;
            match mode {
                AgruparData::Ano => return y.to_string(),
                AgruparData::Trimestre => return format!("T{q} {y}"),
                AgruparData::Mes => return format!("{y}-{m:02}"),
            }
        }
    }

    text
}

/// Converte número serial do Excel (1899-12-30) em (Ano, Mês, Dia).
pub(crate) fn excel_serial_to_ymd(serial: i64) -> (i32, u32, u32) {
    let l = serial + 68569 + 2415019;
    let n = (4 * l) / 146097;
    let l = l - (146097 * n + 3) / 4;
    let i = (4000 * (l + 1)) / 1461001;
    let l = l - (1461 * i) / 4 + 31;
    let j = (80 * l) / 2447;
    let d = l - (2447 * j) / 80;
    let l = j / 11;
    let m = j + 2 - 12 * l;
    let y = 100 * (n - 49) + i + l;
    (y as i32, m as u32, d as u32)
}

#[cfg(test)]
pub(crate) fn aggregate(nums: &[f64], op: AgregacaoOp) -> f64 {
    if nums.is_empty() {
        return 0.0;
    }
    match op {
        AgregacaoOp::Soma => nums.iter().sum(),
        AgregacaoOp::Contagem => nums.len() as f64,
        AgregacaoOp::Media => nums.iter().sum::<f64>() / (nums.len() as f64),
        AgregacaoOp::Min => nums.iter().cloned().fold(f64::INFINITY, f64::min),
        AgregacaoOp::Max => nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        AgregacaoOp::DesvioPadrao => {
            if nums.len() < 2 {
                return 0.0;
            }
            let mean = nums.iter().sum::<f64>() / (nums.len() as f64);
            let var = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / ((nums.len() - 1) as f64);
            var.sqrt()
        }
        AgregacaoOp::Moda => {
            let mut tally = std::collections::HashMap::new();
            let mut best_val = nums[0];
            let mut best_count = 0;
            for &n in nums {
                let bits = n.to_bits();
                let count = tally.entry(bits).or_insert(0);
                *count += 1;
                if *count > best_count {
                    best_count = *count;
                    best_val = n;
                }
            }
            best_val
        }
    }
}


/// Executa a montagem da Tabela Dinâmica a partir de uma matriz de dados de entrada.
pub fn montar_dinamica(data_rows: &[Vec<Value>], config: &DinamicaConfig) -> Matrix {
    montar_dinamica_com(
        data_rows.len(),
        |r, c| data_rows[r].get(c).cloned().unwrap_or(Value::Empty),
        config,
    )
}

/// A mesma conta, lendo cada valor por `valor(linha, coluna)` — as duas
/// relativas à `origem`, a linha 0 é o cabeçalho. É o que deixa calcular
/// direto da planilha viva, sem montar uma cópia da área de origem: numa
/// planilha de dois milhões de linhas, essa cópia ia ao JavaScript em texto e
/// voltava em JSON (bug de 23/09/2026).
/// Incremental por grupos: para Soma/Contagem/Média/Min/Max nenhum número
/// de entrada é retido depois da linha atual. O custo de memória é proporcional
/// à quantidade de grupos distintos, não à quantidade de linhas de origem.
fn acumuladores(valores: &[MedidaDinamica]) -> Vec<super::acumulador::Acumulador> {
    valores.iter().map(|v| super::acumulador::Acumulador::novo(v.op)).collect()
}

fn acumular(totais: &mut [super::acumulador::Acumulador], dados: &[Medida]) {
    for (acc, &m) in totais.iter_mut().zip(dados) { acc.receber_medida(m); }
}

pub fn montar_dinamica_com(
    total_linhas: usize,
    valor: impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
) -> Matrix {
    montar_dinamica_filtrada(total_linhas, valor, config, |_| true)
}

/// Mesmo motor incremental, descartando a linha antes de ler suas dimensões
/// e medidas. Evita criar Vec<usize> com milhões de índices na filtragem.
pub(crate) fn montar_dinamica_filtrada(
    total_linhas: usize,
    valor: impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
    aceita: impl Fn(usize) -> bool,
) -> Matrix {
    montar_dinamica_com_chaves(total_linhas, valor, config, aceita).0
}

/// [`montar_dinamica_filtrada`] devolvendo também as chaves de Linhas (uma
/// por linha de item, na ordem da saída) e de Colunas (uma por grupo de
/// medidas; `[[]]` sem campo em Colunas): o duplo clique (Detalhes) acha as
/// linhas da origem sem montar a dinâmica de novo.
pub(crate) fn montar_dinamica_com_chaves(
    total_linhas: usize,
    valor: impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
    aceita: impl Fn(usize) -> bool,
) -> (Matrix, Vec<Vec<String>>, Vec<Vec<String>>) {
    if total_linhas <= 1 {
        return (Matrix::new(vec![vec![]]), Vec::new(), Vec::new());
    }
    let DinamicaConfig { origem, linhas, colunas, valores, agrupar_data } = config;
    let c0 = origem.c0 as usize;
    let c1 = origem.c1 as usize;

    let mut cabecalhos = Vec::with_capacity(c1 - c0 + 1);
    for c in c0..=c1 {
        let texto = to_text(&valor(0, c - c0));
        cabecalhos.push(if texto.trim().is_empty() {
            format!("Coluna {}", c + 1)
        } else { texto });
    }

    // Só há Vec<f64> de linha atual. Cada texto distinto de um campo vira um
    // código uma vez só, e os grupos somam por código: antes eram ~12 cópias
    // de texto por linha (0,48 s em 1 milhão de linhas, 17/10/2026).
    let mut codigos_l: Vec<HashMap<String, u32>> = vec![HashMap::new(); linhas.len()];
    let mut codigos_c: Vec<HashMap<String, u32>> = vec![HashMap::new(); colunas.len()];
    let mut combos_l: HashMap<Vec<u32>, usize> = HashMap::new();
    let mut combos_c: HashMap<Vec<u32>, usize> = HashMap::new();
    let mut grupos: HashMap<(usize, usize), Vec<super::acumulador::Acumulador>> = HashMap::new();
    let mut por_linha: Vec<Vec<super::acumulador::Acumulador>> = Vec::new();
    let mut por_coluna: Vec<Vec<super::acumulador::Acumulador>> = Vec::new();
    let mut total_geral = acumuladores(valores);
    let mut medidas = Vec::with_capacity(valores.len());
    let mut ids: Vec<u32> = Vec::new();
    let codigo = |mapa: &mut HashMap<String, u32>, chave: String| -> u32 {
        if let Some(&k) = mapa.get(&chave) { return k; }
        let k = mapa.len() as u32;
        mapa.insert(chave, k);
        k
    };
    let combo = |mapa: &mut HashMap<Vec<u32>, usize>, ids: &[u32], somas: &mut Vec<Vec<super::acumulador::Acumulador>>| -> usize {
        if let Some(&k) = mapa.get(ids) { return k; }
        let k = mapa.len();
        mapa.insert(ids.to_vec(), k);
        somas.push(acumuladores(valores));
        k
    };

    for r in 1..total_linhas {
        if !aceita(r) { continue; }
        ids.clear();
        for (d, &col) in linhas.iter().enumerate() {
            ids.push(codigo(&mut codigos_l[d], chave_de(valor(r, col.saturating_sub(c0)), *agrupar_data)));
        }
        let li = combo(&mut combos_l, &ids, &mut por_linha);
        ids.clear();
        for (d, &col) in colunas.iter().enumerate() {
            ids.push(codigo(&mut codigos_c[d], chave_de(valor(r, col.saturating_sub(c0)), *agrupar_data)));
        }
        let ci = combo(&mut combos_c, &ids, &mut por_coluna);
        medidas.clear();
        for medida in valores {
            medidas.push(Medida::de(&valor(r, medida.coluna.saturating_sub(c0))));
        }
        acumular(grupos.entry((li, ci)).or_insert_with(|| acumuladores(valores)), &medidas);
        acumular(&mut por_linha[li], &medidas);
        acumular(&mut por_coluna[ci], &medidas);
        acumular(&mut total_geral, &medidas);
    }

    // De volta aos textos, na ordem de antes (BTreeSet = ordem do texto).
    fn textos(codigos: &[HashMap<String, u32>]) -> Vec<Vec<&str>> {
        codigos.iter().map(|m| {
            let mut v = vec![""; m.len()];
            for (t, &k) in m { v[k as usize] = t.as_str(); }
            v
        }).collect()
    }
    let (textos_l, textos_c) = (textos(&codigos_l), textos(&codigos_c));
    let ordenar = |combos: HashMap<Vec<u32>, usize>, textos: &[Vec<&str>]| -> Vec<(Vec<String>, usize)> {
        let mut v: Vec<(Vec<String>, usize)> = combos.into_iter().map(|(ids, k)| {
            (ids.iter().enumerate().map(|(d, &i)| textos[d][i as usize].to_owned()).collect(), k)
        }).collect();
        // Como o Excel: 2 antes de 10 (antes, ordem do texto: "10" < "2").
        v.sort_by(|(a, _), (b, _)| a.iter().zip(b).map(|(x, y)| super::exibicao::comparar_natural(x, y))
            .find(|o| o.is_ne()).unwrap_or_else(|| a.len().cmp(&b.len())));
        v
    };
    // Campo vazio: uma combinação só (a vazia), como antes.
    let mut chaves_linhas = ordenar(combos_l, &textos_l);
    let mut chaves_colunas = ordenar(combos_c, &textos_c);
    if chaves_linhas.is_empty() { chaves_linhas.push((Vec::new(), usize::MAX)); }
    if chaves_colunas.is_empty() { chaves_colunas.push((Vec::new(), usize::MAX)); }
    let com_dimensao_coluna = !colunas.is_empty();
    let vazio = acumuladores(valores);

    let mut cabecalho = Vec::new();
    if linhas.is_empty() {
        cabecalho.push(Value::Text("Rótulos de Linha".to_owned()));
    }
    for &col in linhas {
        cabecalho.push(Value::Text(cabecalhos
            .get(col.saturating_sub(c0)).cloned()
            .unwrap_or_else(|| format!("Coluna {}", col+1))));
    }
    for (chave_coluna, _) in &chaves_colunas {
        for medida in valores {
            let nome = cabecalhos.get(medida.coluna.saturating_sub(c0))
                .cloned().unwrap_or_else(|| format!("Val {}",medida.coluna+1));
            cabecalho.push(Value::Text(if chave_coluna.is_empty() {
                nome
            } else {
                format!("{} ({nome})", chave_coluna.join(" | "))
            }));
        }
    }
    if com_dimensao_coluna {
        for medida in valores {
            let nome = cabecalhos.get(medida.coluna.saturating_sub(c0))
                .cloned().unwrap_or_else(|| format!("Val {}",medida.coluna+1));
            cabecalho.push(Value::Text(format!("Total Geral ({nome})")));
        }
    }
    let mut linhas_saida = vec![cabecalho];
    // Sem campo em Linhas a única linha seria igual ao Total Geral: fica só
    // ele, como no Excel (tirar o último campo de Linhas mostra o total).
    for (chave_linha, li) in chaves_linhas.iter().filter(|_| !linhas.is_empty()) {
        let mut saida: Vec<Value> = chave_linha.iter().cloned().map(Value::Text).collect();
        for (_, ci) in &chaves_colunas {
            let grupo = grupos.get(&(*li, *ci)).unwrap_or(&vazio);
            saida.extend(grupo.iter().map(|acc| Value::Number(acc.resultado())));
        }
        if com_dimensao_coluna {
            let totais = por_linha.get(*li).unwrap_or(&vazio);
            saida.extend(totais.iter().map(|acc| Value::Number(acc.resultado())));
        }
        linhas_saida.push(saida);
    }
    let mut total = vec![Value::Text(String::new()); linhas.len().max(1)];
    total[0] = Value::Text("Total Geral".to_owned());
    for (_, ci) in &chaves_colunas {
        let acumulados = por_coluna.get(*ci).unwrap_or(&vazio);
        total.extend(acumulados.iter().map(|acc| Value::Number(acc.resultado())));
    }
    if com_dimensao_coluna {
        total.extend(total_geral.iter().map(|acc| Value::Number(acc.resultado())));
    }
    linhas_saida.push(total);
    if linhas.is_empty() { chaves_linhas.clear(); }
    let so_textos = |v: Vec<(Vec<String>, usize)>| v.into_iter().map(|(t, _)| t).collect();
    (Matrix::new(linhas_saida), so_textos(chaves_linhas), so_textos(chaves_colunas))
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::to_number;

    fn criar_dados_exemplo() -> (Rect, Vec<Vec<Value>>) {
        let origem = Rect::new(0, 0, 5, 3);
        let dados = vec![
            vec![
                Value::Text("Região".into()),
                Value::Text("Produto".into()),
                Value::Text("Vendas".into()),
                Value::Text("Data".into()),
            ],
            vec![
                Value::Text("Sul".into()),
                Value::Text("Maçã".into()),
                Value::Number(100.0),
                Value::Text("2026-01-15".into()),
            ],
            vec![
                Value::Text("Sul".into()),
                Value::Text("Banana".into()),
                Value::Number(200.0),
                Value::Text("2026-01-20".into()),
            ],
            vec![
                Value::Text("Norte".into()),
                Value::Text("Maçã".into()),
                Value::Number(150.0),
                Value::Text("2026-02-10".into()),
            ],
            vec![
                Value::Text("Norte".into()),
                Value::Text("Banana".into()),
                Value::Number(300.0),
                Value::Text("2026-02-12".into()),
            ],
            vec![
                Value::Text("Sul".into()),
                Value::Text("Maçã".into()),
                Value::Number(120.0),
                Value::Text("2026-03-01".into()),
            ],
        ];
        (origem, dados)
    }

    #[test]
    fn agrupamento_nao_aceita_mes_invalido() {
        for mes in ["00", "13", "99"] {
            let texto = format!("2026-{mes}-15");
            let valor = Value::Text(texto.clone());
            assert_eq!(parse_date_key(&valor, Some(AgruparData::Mes)), texto);
        }
        assert_eq!(
            parse_date_key(&Value::Text("2026-12-15".into()), Some(AgruparData::Trimestre)),
            "T4 2026"
        );
    }

    #[test]
    fn operacoes_da_ponte_typescript_mantem_contagem() {
        assert_eq!(AgregacaoOp::do_str("conta"), AgregacaoOp::Contagem);
        assert_eq!(AgregacaoOp::do_str("contar"), AgregacaoOp::Contagem);
        assert_eq!(AgregacaoOp::do_str("desvioPadrao"), AgregacaoOp::DesvioPadrao);
    }

    #[test]
    fn pivot_soma_por_linhas() {
        let (origem, dados) = criar_dados_exemplo();
        let config = DinamicaConfig {
            origem,
            linhas: vec![0], // Região
            colunas: vec![],
            valores: vec![MedidaDinamica {
                coluna: 2,
                op: AgregacaoOp::Soma,
            }],
            agrupar_data: None,
        };

        let matrix = montar_dinamica(&dados, &config);
        assert_equal_rows(
            &matrix,
            vec![
                vec!["Região", "Vendas"],
                vec!["Norte", "450"], // 150 + 300
                vec!["Sul", "420"],   // 100 + 200 + 120
                vec!["Total Geral", "870"],
            ],
        );
    }

    #[test]
    fn sem_campo_em_linhas_sai_so_o_total_geral() {
        let data: Vec<Vec<Value>> = vec![
            vec![Value::Text("Região".into()), Value::Text("Valor".into())],
            vec![Value::Text("Sul".into()), Value::Number(10.)],
            vec![Value::Text("Norte".into()), Value::Number(6.)],
        ];
        let config = DinamicaConfig {
            origem: Rect::new(0, 0, 2, 1),
            linhas: vec![], colunas: vec![],
            valores: vec![MedidaDinamica { coluna: 1, op: AgregacaoOp::Soma }],
            agrupar_data: None,
        };
        let m = montar_dinamica(&data, &config);
        assert_eq!(m.height(), 2, "cabeçalho e Total Geral, sem a linha \"Total\" repetida");
        assert_eq!(m.at(1, 0), &Value::Text("Total Geral".to_owned()));
        assert_eq!(m.at(1, 1), &Value::Number(16.));
    }

    #[test]
    fn rotulos_total_e_separadores_nao_desalinham_colunas() {
        // Duas chaves diferentes imprimem o mesmo texto com join(" | ").
        // Devem continuar sendo dois grupos distintos e duas dimensões.
        let data = vec![
            vec!["Departamento","Equipe","Quantidade"].into_iter().map(|s| Value::Text(s.into())).collect(),
            vec![Value::Text("Total".into()),Value::Text("Norte".into()),Value::Number(2.)],
            vec![Value::Text("A | B".into()),Value::Text("C".into()),Value::Number(3.)],
            vec![Value::Text("A".into()),Value::Text("B | C".into()),Value::Number(7.)],
        ];
        let config = DinamicaConfig {
            origem: Rect::new(0,0,3,2),
            linhas: vec![0,1], colunas: vec![],
            valores: vec![MedidaDinamica { coluna:2, op:AgregacaoOp::Soma }],
            agrupar_data: None,
        };
        let m = montar_dinamica(&data,&config);
        assert_eq!(m.width(),3);
        assert_eq!(m.height(),5);
        assert!(m.rows().iter().all(|row| row.len()==3));
        assert_eq!(m.at(3,0), &Value::Text("Total".to_owned()));
        assert_eq!(m.at(3,1), &Value::Text("Norte".to_owned()));
        assert_eq!(m.at(3,2), &Value::Number(2.));
        assert_eq!(m.at(4,0), &Value::Text("Total Geral".to_owned()));
        assert_eq!(m.at(4,2), &Value::Number(12.));
    }

    #[test]
    fn pivot_linhas_e_colunas_cruzadas() {
        let (origem, dados) = criar_dados_exemplo();
        let config = DinamicaConfig {
            origem,
            linhas: vec![0],  // Região
            colunas: vec![1], // Produto
            valores: vec![MedidaDinamica {
                coluna: 2,
                op: AgregacaoOp::Soma,
            }],
            agrupar_data: None,
        };

        let matrix = montar_dinamica(&dados, &config);
        assert_eq!(matrix.rows().len(), 4);
        assert_eq!(matrix.width(), 4);
    }

    #[test]
    fn pivot_agrupamento_por_data_mes() {
        let (origem, dados) = criar_dados_exemplo();
        let config = DinamicaConfig {
            origem,
            linhas: vec![3], // Data
            colunas: vec![],
            valores: vec![MedidaDinamica {
                coluna: 2,
                op: AgregacaoOp::Soma,
            }],
            agrupar_data: Some(AgruparData::Mes),
        };

        let matrix = montar_dinamica(&dados, &config);
        assert_equal_rows(
            &matrix,
            vec![
                vec!["Data", "Vendas"],
                vec!["2026-01", "300"],
                vec!["2026-02", "450"],
                vec!["2026-03", "120"],
                vec!["Total Geral", "870"],
            ],
        );
    }

    #[test]
    fn pivot_desvio_padrao_e_moda() {
        let (origem, dados) = criar_dados_exemplo();
        let config = DinamicaConfig {
            origem,
            linhas: vec![0], // Região
            colunas: vec![],
            valores: vec![
                MedidaDinamica {
                    coluna: 2,
                    op: AgregacaoOp::DesvioPadrao,
                },
                MedidaDinamica {
                    coluna: 2,
                    op: AgregacaoOp::Moda,
                },
            ],
            agrupar_data: None,
        };

        let matrix = montar_dinamica(&dados, &config);
        assert_eq!(matrix.rows().len(), 4);
        assert_eq!(matrix.width(), 3); // Região, DesvioPadrao, Moda
    }

    #[test]
    fn pivot_apenas_por_colunas_mantem_formato_retangular() {
        let (origem, dados) = criar_dados_exemplo();
        let config = DinamicaConfig {
            origem, linhas: vec![], colunas: vec![1],
            valores: vec![MedidaDinamica { coluna: 2, op: AgregacaoOp::Soma }],
            agrupar_data: None,
        };
        let m = montar_dinamica(&dados, &config);
        assert_eq!(m.width(), 4);
        // Cabeçalho e Total Geral (sem a linha "Total" igual a ele).
        assert_eq!(m.height(), 2);
        assert!(m.rows().iter().all(|r| r.len() == 4));
        assert_eq!(m.at(0, 0), &Value::Text("Rótulos de Linha".into()));
        assert_eq!(m.at(1, 0), &Value::Text("Total Geral".into()));
    }

    #[test]
    fn filtro_online_descarta_linhas_sem_guardar_seus_indices() {
        let (origem,dados) = criar_dados_exemplo();
        let c = DinamicaConfig {
            origem, linhas: vec![], colunas: vec![1],
            valores: vec![MedidaDinamica { coluna:2, op:AgregacaoOp::Soma }],
            agrupar_data: None,
        };
        let result = montar_dinamica_filtrada(
            dados.len(),
            |r,c| dados[r].get(c).cloned().unwrap_or(Value::Empty),
            &c,
            |r| r >= 4,
        );
        assert_eq!(result.width(),4);
        assert_eq!(result.height(),2);
        assert_eq!(result.at(1,3),&Value::Number(420.));
    }

    #[test]
    fn pivot_data_serial_excel_consistente_com_epoch_1899_12_30() {
        assert_eq!(excel_serial_to_ymd(0), (1899, 12, 30));
        assert_eq!(excel_serial_to_ymd(2), (1900, 1, 1));
        assert_eq!(parse_date_key(&Value::Number(44562.), Some(AgruparData::Mes)), "2022-01");
        assert_eq!(parse_date_key(&Value::Text("44562".into()), Some(AgruparData::Mes)), "2022-01");
        assert_eq!(parse_date_key(&Value::Text("44562.5".into()), Some(AgruparData::Ano)), "2022");
        assert_eq!(parse_date_key(&Value::Text("NaN".into()), Some(AgruparData::Mes)), "NaN");
    }

    fn assert_equal_rows(matrix: &Matrix, expected: Vec<Vec<&str>>) {
        let actual_rows = matrix.rows();
        assert_eq!(
            actual_rows.len(),
            expected.len(),
            "Número de linhas incorreto"
        );
        for (r_idx, exp_row) in expected.iter().enumerate() {
            let act_row = &actual_rows[r_idx];
            assert_eq!(
                act_row.len(),
                exp_row.len(),
                "Comprimento da linha {r_idx} incorreto"
            );
            for (c_idx, &exp_val) in exp_row.iter().enumerate() {
                let act_val = to_text(&act_row[c_idx]);
                assert_eq!(
                    act_val, exp_val,
                    "Célula ({r_idx}, {c_idx}) divergente: obtido '{act_val}', esperado '{exp_val}'"
                );
            }
        }
    }
    /// Medida, não roda no `cargo test`: `cargo test --release -p sheet-core
    /// medir_dinamica -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn medir_dinamica() {
        let n = 500_000usize;
        let config = DinamicaConfig {
            origem: Rect::new(0, 0, n as u32, 2),
            linhas: vec![0],
            colunas: vec![1],
            valores: vec![
                MedidaDinamica { coluna: 2, op: AgregacaoOp::Soma },
                MedidaDinamica { coluna: 2, op: AgregacaoOp::Media },
            ],
            agrupar_data: None,
        };
        let t = std::time::Instant::now();
        let m = montar_dinamica_com(
            n + 1,
            |r, c| {
                if r == 0 {
                    return Value::Text(["Tipo", "Região", "Valor"][c].into());
                }
                match c {
                    0 => Value::Text(["ALTO", "BAIXO", "NORMAL", "MEDIO"][r % 4].into()),
                    1 => Value::Text(["N", "S", "L"][r % 3].into()),
                    _ => Value::Number((r % 97) as f64),
                }
            },
            &config,
        );
        println!("MEDIDA dinamica {n} linhas: {} ms ({} linhas no resultado)", t.elapsed().as_millis(), m.rows().len());
    }

    /// Conferência independente da varredura por códigos: dados variados
    /// (vazios, números como texto, rótulos repetidos), cada célula somada à
    /// mão pelas linhas de origem.
    #[test]
    fn cada_celula_bate_com_a_soma_feita_a_mao() {
        let mut semente = 7u64;
        let mut sorteio = |n: u64| { semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (semente >> 33) % n };
        let mut dados = vec![vec![Value::Text("A".into()), Value::Text("B".into()), Value::Text("C".into()), Value::Text("V".into())]];
        for _ in 0..3000 {
            let a = ["x", "y", "z", "", "10"][sorteio(5) as usize];
            let b = sorteio(7);
            let c = ["p", "q"][sorteio(2) as usize];
            let rot = |t: &str| if t.is_empty() { Value::Empty } else { Value::Text(t.into()) };
            dados.push(vec![rot(a), Value::Number(b as f64), rot(c), Value::Number(sorteio(100) as f64)]);
        }
        let texto = |v: &Value| parse_date_key(v, None);
        for (linhas, colunas) in [(vec![0, 1], vec![2]), (vec![1], vec![]), (vec![], vec![0, 2])] {
            let config = DinamicaConfig {
                origem: Rect::new(0, 0, dados.len() as u32 - 1, 3),
                linhas: linhas.clone(), colunas: colunas.clone(),
                valores: vec![MedidaDinamica { coluna: 3, op: AgregacaoOp::Soma }],
                agrupar_data: None,
            };
            let (m, chaves_l, chaves_c) = montar_dinamica_com_chaves(dados.len(),
                |r, c| dados[r].get(c).cloned().unwrap_or(Value::Empty), &config, |_| true);
            let mut ordenadas = chaves_l.clone();
            let natural = |a: &Vec<String>, b: &Vec<String>| a.iter().zip(b)
                .map(|(x, y)| super::super::exibicao::comparar_natural(x, y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal);
            ordenadas.sort_by(natural);
            ordenadas.dedup();
            assert_eq!(ordenadas, chaves_l, "linhas distintas e em ordem");
            let soma = |l: Option<&Vec<String>>, c: Option<&Vec<String>>| -> f64 {
                dados[1..].iter().filter(|d| {
                    l.is_none_or(|l| linhas.iter().zip(l).all(|(&i, t)| texto(&d[i]) == *t))
                        && c.is_none_or(|c| colunas.iter().zip(c).all(|(&i, t)| texto(&d[i]) == *t))
                }).map(|d| to_number(&d[3]).unwrap()).sum()
            };
            let n = linhas.len().max(1);
            let com_total = !colunas.is_empty();
            let corpo = if linhas.is_empty() { 0 } else { chaves_l.len() };
            assert_eq!(m.height(), corpo + 2);
            for (i, l) in chaves_l.iter().enumerate().take(corpo) {
                let linha = &m.rows()[i + 1];
                for (j, c) in chaves_c.iter().enumerate() {
                    assert_eq!(linha[n + j], Value::Number(soma(Some(l), Some(c))), "{l:?} × {c:?}");
                }
                if com_total {
                    assert_eq!(linha[n + chaves_c.len()], Value::Number(soma(Some(l), None)));
                }
            }
            let total = m.rows().last().unwrap();
            for (j, c) in chaves_c.iter().enumerate() {
                assert_eq!(total[n + j], Value::Number(soma(None, Some(c))));
            }
            if com_total {
                assert_eq!(total[n + chaves_c.len()], Value::Number(soma(None, None)));
            }
        }
    }
}
