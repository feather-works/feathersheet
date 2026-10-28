//! OpenQuery Motor de ETL — Transformação, filtragem, agregação, junção e pivotação de dados em Rust.
//!
//! Tradução direta e otimizada de `src/core/query.ts` para execução nativa ultra-rápida.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use crate::celulas::date_serial::serial_to_ymd;
use crate::celulas::number::to_js_string;
use crate::celulas::values::{compare, to_number, Value};
use crate::funcoes::fn_helpers::make_predicate;

mod limites;
mod fluxo;
pub use fluxo::executar_query_em_fluxo;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tabela {
    pub cabecalho: Vec<String>,
    pub linhas: Vec<Vec<Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Agregacao {
    Soma,
    Contagem,
    ContarValores,
    Media,
    Maximo,
    Minimo,
    Mediana,
    Distintos,
    Primeiro,
    Ultimo,
}

impl Agregacao {
    pub fn do_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "soma" | "sum" => Agregacao::Soma,
            "contagem" | "count" => Agregacao::Contagem,
            "contarvalores" | "contar_valores" => Agregacao::ContarValores,
            "media" | "média" | "avg" | "average" => Agregacao::Media,
            "maximo" | "máximo" | "max" => Agregacao::Maximo,
            "minimo" | "mínimo" | "min" => Agregacao::Minimo,
            "mediana" | "median" => Agregacao::Mediana,
            "distintos" | "distinct" => Agregacao::Distintos,
            "primeiro" | "first" => Agregacao::Primeiro,
            "ultimo" | "último" | "last" => Agregacao::Ultimo,
            _ => Agregacao::Soma,
        }
    }

    pub fn rotulo(&self) -> &'static str {
        match self {
            Agregacao::Soma => "Soma",
            Agregacao::Contagem => "Contagem",
            Agregacao::ContarValores => "Contagem",
            Agregacao::Media => "Média",
            Agregacao::Maximo => "Máximo",
            Agregacao::Minimo => "Mínimo",
            Agregacao::Mediana => "Mediana",
            Agregacao::Distintos => "Distintos",
            Agregacao::Primeiro => "Primeiro",
            Agregacao::Ultimo => "Último",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Agregado {
    pub col: usize,
    #[serde(default = "default_agregacao")]
    pub como: Agregacao,
    pub nome: Option<String>,
}

fn default_agregacao() -> Agregacao {
    Agregacao::Soma
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChaveOrdenacao {
    pub col: usize,
    pub desc: Option<bool>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum IndiceColuna {
    Single(usize),
    Multiple(Vec<usize>),
}

impl IndiceColuna {
    pub fn to_vec(&self) -> Vec<usize> {
        match self {
            IndiceColuna::Single(c) => vec![*c],
            IndiceColuna::Multiple(v) => v.clone(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Passo {
    Filtrar { col: usize, criterio: Value },
    Ordenar { por: Vec<ChaveOrdenacao> },
    Colunas { manter: Vec<usize> },
    Renomear { col: usize, nome: String },
    Tipar { col: usize, para: String },
    Substituir { col: usize, de: Value, para: Value },
    SubstituirErros { col: usize, para: Value },
    RemoverErros { col: usize },
    Mesclar { cols: Vec<usize>, separador: String, nome: Option<String> },
    Derivar { col: usize, parte: String, nome: Option<String> },
    /// Divide em linhas: cada pedaço vira uma linha, as outras colunas repetem.
    Dividir { col: usize, separador: String },
    /// Divide em colunas, como o Power Query: `Nome.1`, `Nome.2`… no lugar de `col`.
    DividirColunas { col: usize, separador: String },
    Distintas { cols: Option<Vec<usize>> },
    Agrupar { por: Vec<usize>, agregar: Vec<Agregado> },
    Pivotar { linhas: Vec<usize>, colunas: Vec<usize>, valores: Vec<Agregado> },
    Juntar {
        fonte: String,
        esquerda: IndiceColuna,
        direita: IndiceColuna,
        modo: Option<String>,
    },
    Acrescentar { fonte: String },
    Despivotar {
        manter: Vec<usize>,
        #[serde(rename = "nomeAtributo")]
        nome_atributo: Option<String>,
        #[serde(rename = "nomeValor")]
        nome_valor: Option<String>,
    },
    PromoverCabecalho,
    RebaixarCabecalho,
    /// `inicio`/`incremento` são número qualquer, como no `query.ts`: com
    /// `i64` um "1.5" digitado no assistente derrubava a consulta inteira
    /// ("passo de consulta inválido").
    Indice {
        inicio: Option<f64>,
        incremento: Option<f64>,
        nome: Option<String>,
    },
    Condicional {
        nome: String,
        col: usize,
        operador: String,
        valor: String,
        entao: String,
        senao: String,
    },
    /// `quantas` é número qualquer, como no `query.ts` (ver `linhas_do_limite`):
    /// com `usize`, "-5" ou "2.5" em "Manter linhas" derrubava a consulta.
    Limite { quantas: f64 },
}

/// Quantas linhas o passo `limite` deixa passar, como o `primeiras` do
/// `query.ts`: negativo é 0, e fração arredonda para cima (ele para quando já
/// viu `n` ou mais).
fn linhas_do_limite(quantas: f64) -> usize {
    if quantas.is_nan() || quantas <= 0.0 {
        0
    } else {
        // `as` satura: um número enorme vira `usize::MAX`, sem pânico.
        quantas.ceil() as usize
    }
}

static MESES: [&str; 12] = [
    "janeiro", "fevereiro", "março", "abril", "maio", "junho",
    "julho", "agosto", "setembro", "outubro", "novembro", "dezembro",
];
static DIAS: [&str; 7] = ["domingo", "segunda", "terça", "quarta", "quinta", "sexta", "sábado"];


/// Executor protegido usado pelo editor interativo OpenQuery. O executor
/// histórico permanece sem limite para consumidores que gerenciam memória.
pub fn executar_query_limitado(
    fonte: Tabela,
    passos: &[Passo],
    fontes: &HashMap<String, Tabela>,
    max_celulas: u64,
) -> Result<Tabela, String> {
    let mut atual = fonte;
    let inicial = (atual.cabecalho.len() as u64)
        .saturating_mul(atual.linhas.len() as u64);
    if inicial > max_celulas {
        return Err("Fonte OpenQuery excede o orçamento de memória".into());
    }
    for passo in passos {
        limites::verificar(&atual, passo, fontes, max_celulas)?;
        atual = aplicar_passo(atual, passo, fontes);
    }
    Ok(atual)
}

/// Executa uma sequência de passos do OpenQuery sobre a tabela fonte.
///
/// Tradução passo a passo do `executar` do `query.ts` (refeita em
/// 24/09/2026: a primeira versão divergia em ordem de grupos, contagem, dia
/// da semana, curinga do filtro e nos rótulos da pivotagem; o teste
/// `test/ponte/consulta-rust.test.ts` compara os dois).
pub fn executar_query(fonte: Tabela, passos: &[Passo], fontes: &HashMap<String, Tabela>) -> Tabela {
    let mut atual = fonte;
    for passo in passos {
        atual = aplicar_passo(atual, passo, fontes);
    }
    atual
}

/// `l[c] ?? null`
fn celula(l: &[Value], c: usize) -> Value {
    l.get(c).cloned().unwrap_or(Value::Empty)
}

static VAZIO: Value = Value::Empty;

/// `l[c] ?? null` sem copiar: o ordenar compara milhões de vezes, e clonar
/// um texto a cada comparação era alocação à toa.
fn celula_ref(l: &[Value], c: usize) -> &Value {
    l.get(c).unwrap_or(&VAZIO)
}

/// `copia[col] = v`, esticando a linha como o array do JavaScript estica.
fn gravar_em(l: &mut Vec<Value>, col: usize, v: Value) {
    if l.len() <= col {
        l.resize(col + 1, Value::Empty);
    }
    l[col] = v;
}

fn coluna_sem_nome(c: usize) -> String {
    format!("Coluna {}", c + 1)
}

fn nome_da_coluna(t: &Tabela, c: usize) -> String {
    t.cabecalho.get(c).cloned().unwrap_or_else(|| coluna_sem_nome(c))
}

/// `String(v)` do JavaScript: número como o JS escreve, lógico em inglês
/// minúsculo, erro pelo código, vazio como texto vazio.
fn string_js(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => to_js_string(*n),
        Value::Bool(b) => b.to_string(),
        Value::Text(t) => t.clone(),
        Value::Error(e) => e.code.as_str().to_string(),
    }
}

/// `Number(v)` do JavaScript, para a coluna condicional.
fn number_js(v: &Value) -> f64 {
    match v {
        Value::Empty => 0.0,
        Value::Number(n) => *n,
        Value::Bool(b) => f64::from(u8::from(*b)),
        Value::Error(_) => f64::NAN,
        Value::Text(t) => texto_para_number_js(t),
    }
}

fn texto_para_number_js(t: &str) -> f64 {
    let s = t.trim();
    if s.is_empty() {
        return 0.0;
    }
    // O `f64::from_str` aceita "inf" e "nan"; o JavaScript só "Infinity".
    match s.trim_start_matches(['+', '-']) {
        "Infinity" => return if s.starts_with('-') { f64::NEG_INFINITY } else { f64::INFINITY },
        r if r.chars().any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E') => return f64::NAN,
        _ => {}
    }
    s.parse::<f64>().unwrap_or(f64::NAN)
}

/// Identidade de um valor para agrupar: distingue 1 de "1" e de VERDADEIRO.
fn chave_de(v: &Value) -> String {
    match v {
        Value::Empty => "\u{0}vazio".to_string(),
        Value::Number(n) => format!("n{}", to_js_string(*n)),
        Value::Bool(b) => format!("b{b}"),
        Value::Text(t) => format!("s{t}"),
        Value::Error(e) => format!("e{}", e.code.as_str()),
    }
}

fn chave_das(l: &[Value], cols: &[usize]) -> String {
    cols.iter().map(|&c| chave_de(&celula(l, c))).collect::<Vec<_>>().join("\u{0}")
}

/// `textoDe`: o rótulo de um valor na pivotagem.
fn texto_de(v: &Value) -> String {
    match v {
        Value::Empty => "(vazio)".to_string(),
        outro => string_js(outro),
    }
}

/// O `===` do JavaScript entre dois valores (erro nunca é igual: são objetos).
fn identico(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Empty, Value::Empty) => true,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Text(x), Value::Text(y)) => x == y,
        _ => false,
    }
}

fn eh_erro(v: &Value) -> bool {
    matches!(v, Value::Error(_))
}

/// Um grupo em ordem de chegada (o `Map` do JavaScript guarda a ordem).
struct Grupos<T> {
    indice: HashMap<String, usize>,
    itens: Vec<T>,
}

impl<T> Grupos<T> {
    fn novo() -> Self {
        Grupos { indice: HashMap::new(), itens: Vec::new() }
    }
    fn achar_ou(&mut self, chave: String, criar: impl FnOnce() -> T) -> &mut T {
        let i = *self.indice.entry(chave).or_insert_with(|| {
            self.itens.push(criar());
            self.itens.len() - 1
        });
        &mut self.itens[i]
    }
}

fn aplicar_passo(t: Tabela, passo: &Passo, fontes: &HashMap<String, Tabela>) -> Tabela {
    match passo {
        Passo::Filtrar { col, criterio } => {
            let aceita = make_predicate(criterio);
            let linhas = t.linhas.into_iter().filter(|l| aceita(&celula(l, *col))).collect();
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::Limite { quantas } => {
            let linhas = t.linhas.into_iter().take(linhas_do_limite(*quantas)).collect();
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::Renomear { col, nome } => {
            let mut cabecalho = t.cabecalho;
            if *col < cabecalho.len() {
                cabecalho[*col] = nome.clone();
            }
            Tabela { cabecalho, linhas: t.linhas }
        }
        Passo::Colunas { manter } => Tabela {
            cabecalho: manter.iter().map(|&c| nome_da_coluna(&t, c)).collect(),
            linhas: t.linhas.iter().map(|l| manter.iter().map(|&c| celula(l, c)).collect()).collect(),
        },
        Passo::Tipar { col, para } => {
            let linhas = t
                .linhas
                .into_iter()
                .map(|mut l| {
                    let v = converter(celula(&l, *col), para);
                    gravar_em(&mut l, *col, v);
                    l
                })
                .collect();
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::Substituir { col, de, para } => {
            let linhas = t
                .linhas
                .into_iter()
                .map(|mut l| {
                    let val = celula(&l, *col);
                    let vazio = matches!(de, Value::Empty) && (matches!(val, Value::Empty) || matches!(&val, Value::Text(s) if s.is_empty()));
                    if identico(&val, de) || vazio {
                        gravar_em(&mut l, *col, para.clone());
                    }
                    l
                })
                .collect();
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::SubstituirErros { col, para } => {
            let linhas = t
                .linhas
                .into_iter()
                .map(|mut l| {
                    if eh_erro(&celula(&l, *col)) {
                        gravar_em(&mut l, *col, para.clone());
                    }
                    l
                })
                .collect();
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::RemoverErros { col } => {
            let linhas = t.linhas.into_iter().filter(|l| !eh_erro(&celula(l, *col))).collect();
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::Mesclar { cols, separador, nome } => {
            let mut cabecalho = t.cabecalho;
            cabecalho.push(nome.clone().unwrap_or_else(|| "Mesclado".to_string()));
            let linhas = t
                .linhas
                .into_iter()
                .map(|mut l| {
                    let partes: Vec<String> = cols.iter().map(|&c| string_js(&celula(&l, c))).collect();
                    l.push(Value::Text(partes.join(separador)));
                    l
                })
                .collect();
            Tabela { cabecalho, linhas }
        }
        Passo::Derivar { col, parte, nome } => {
            let mut cabecalho = t.cabecalho.clone();
            cabecalho.push(nome.clone().unwrap_or_else(|| format!("{} ({parte})", nome_da_coluna(&t, *col))));
            let linhas = t
                .linhas
                .into_iter()
                .map(|mut l| {
                    let v = pedaco_da_data(&celula(&l, *col), parte);
                    l.push(v);
                    l
                })
                .collect();
            Tabela { cabecalho, linhas }
        }
        Passo::Dividir { col, separador } => {
            let mut linhas = Vec::with_capacity(t.linhas.len());
            for l in t.linhas {
                match l.get(*col) {
                    Some(Value::Text(s)) if !separador.is_empty() && s.contains(separador.as_str()) => {
                        for pedaco in s.split(separador.as_str()) {
                            let mut copia = l.clone();
                            copia[*col] = Value::Text(pedaco.to_string());
                            linhas.push(copia);
                        }
                    }
                    _ => linhas.push(l),
                }
            }
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::DividirColunas { col, separador } => {
            let partes = |v: &Value| match v {
                Value::Text(s) if !separador.is_empty() => s.split(separador.as_str()).count(),
                _ => 1,
            };
            let n = t.linhas.iter().filter_map(|l| l.get(*col)).map(partes).max().unwrap_or(1);
            if *col >= t.cabecalho.len() {
                return t;
            }
            let nome = t.cabecalho[*col].clone();
            let mut cabecalho = t.cabecalho;
            cabecalho.splice(*col..=*col, (1..=n).map(|i| format!("{nome}.{i}")));
            let linhas = t.linhas.into_iter().map(|mut l| {
                if *col >= l.len() {
                    return l;
                }
                let pedacos: Vec<Value> = match &l[*col] {
                    Value::Text(s) if !separador.is_empty() =>
                        s.split(separador.as_str()).map(|p| Value::Text(p.to_owned())).collect(),
                    outro => vec![outro.clone()],
                };
                let faltam = n - pedacos.len();
                l.splice(*col..=*col, pedacos.into_iter().chain(std::iter::repeat_n(Value::Empty, faltam)));
                l
            }).collect();
            Tabela { cabecalho, linhas }
        }
        Passo::Ordenar { por } => {
            let mut linhas = t.linhas;
            // O `sort` do JavaScript é estável, e o `sort_by` também.
            linhas.sort_by(|a, b| {
                for chave in por {
                    let n = compare(celula_ref(a, chave.col), celula_ref(b, chave.col)).unwrap_or(Ordering::Equal);
                    if n != Ordering::Equal {
                        return if chave.desc.unwrap_or(false) { n.reverse() } else { n };
                    }
                }
                Ordering::Equal
            });
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::Distintas { cols } => {
            let cols: Vec<usize> = cols.clone().unwrap_or_else(|| (0..t.cabecalho.len()).collect());
            let mut vistas = HashSet::new();
            let linhas = t.linhas.into_iter().filter(|l| vistas.insert(chave_das(l, &cols))).collect();
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::Juntar { fonte, esquerda, direita, modo } => juntar(t, fontes.get(fonte), esquerda, direita, modo.as_deref()),
        Passo::Agrupar { por, agregar } => agrupar(t, por, agregar),
        Passo::Pivotar { linhas, colunas, valores } => pivotar(t, linhas, colunas, valores),
        Passo::Acrescentar { fonte } => {
            let Some(outra) = fontes.get(fonte) else { return t };
            // De onde vem cada coluna (o `indexOf` do `query.ts`, achando o
            // primeiro nome igual), calculado uma vez e não a cada linha.
            let origem: Vec<usize> = t
                .cabecalho
                .iter()
                .enumerate()
                .map(|(idx, nome)| outra.cabecalho.iter().position(|c| c == nome).unwrap_or(idx))
                .collect();
            let mut linhas = t.linhas;
            linhas.reserve(outra.linhas.len());
            for l in &outra.linhas {
                linhas.push(origem.iter().map(|&i| celula(l, i)).collect());
            }
            Tabela { cabecalho: t.cabecalho, linhas }
        }
        Passo::Despivotar { manter, nome_atributo, nome_valor } => {
            let nome_da = |c: usize| t.cabecalho.get(c).cloned().unwrap_or_else(|| format!("Coluna {}", c + 1));
            let mut cabecalho: Vec<String> = manter.iter().map(|&c| nome_da(c)).collect();
            cabecalho.push(nome_atributo.clone().unwrap_or_else(|| "Atributo".into()));
            cabecalho.push(nome_valor.clone().unwrap_or_else(|| "Valor".into()));
            let desfazer: Vec<usize> = (0..t.cabecalho.len()).filter(|i| !manter.contains(i)).collect();
            let mut linhas = Vec::new();
            for l in &t.linhas {
                let base: Vec<Value> = manter.iter().map(|&c| celula(l, c)).collect();
                for &c in &desfazer {
                    let mut nova = base.clone();
                    nova.push(Value::Text(nome_da(c)));
                    nova.push(celula(l, c));
                    linhas.push(nova);
                }
            }
            Tabela { cabecalho, linhas }
        }
        Passo::PromoverCabecalho => {
            if t.linhas.is_empty() {
                return t;
            }
            let mut linhas = t.linhas;
            let primeira = linhas.remove(0);
            let cabecalho = primeira
                .iter()
                .enumerate()
                .map(|(i, v)| {
                    let s = string_js(v);
                    if matches!(v, Value::Empty) || s.trim().is_empty() {
                        coluna_sem_nome(i)
                    } else {
                        s
                    }
                })
                .collect();
            Tabela { cabecalho, linhas }
        }
        Passo::RebaixarCabecalho => {
            let cabecalho = (0..t.cabecalho.len()).map(coluna_sem_nome).collect();
            let mut linhas = vec![t.cabecalho.iter().map(|c| Value::Text(c.clone())).collect()];
            linhas.extend(t.linhas);
            Tabela { cabecalho, linhas }
        }
        Passo::Indice { inicio, incremento, nome } => {
            let mut cabecalho = t.cabecalho;
            cabecalho.push(nome.clone().unwrap_or_else(|| "Índice".into()));
            let (mut cont, inc) = (inicio.unwrap_or(0.0), incremento.unwrap_or(1.0));
            let linhas = t
                .linhas
                .into_iter()
                .map(|mut l| {
                    l.push(Value::Number(cont));
                    cont += inc;
                    l
                })
                .collect();
            Tabela { cabecalho, linhas }
        }
        Passo::Condicional { nome, col, operador, valor, entao, senao } => {
            let alvo = valor.to_lowercase();
            let alvo_numero = texto_para_number_js(valor);
            let avalia = |v: &Value| -> bool {
                let s = string_js(v).to_lowercase();
                match operador.as_str() {
                    "=" => s == alvo,
                    "!=" => s != alvo,
                    "contem" => s.contains(&alvo),
                    "comeca_com" => s.starts_with(&alvo),
                    "termina_com" => s.ends_with(&alvo),
                    ">" => number_js(v) > alvo_numero,
                    "<" => number_js(v) < alvo_numero,
                    _ => false,
                }
            };
            let mut cabecalho = t.cabecalho;
            cabecalho.push(nome.clone());
            let linhas = t
                .linhas
                .into_iter()
                .map(|mut l| {
                    let r = if avalia(celula_ref(&l, *col)) { entao } else { senao };
                    l.push(Value::Text(r.clone()));
                    l
                })
                .collect();
            Tabela { cabecalho, linhas }
        }
    }
}

fn juntar(t: Tabela, outra: Option<&Tabela>, esquerda: &IndiceColuna, direita: &IndiceColuna, modo: Option<&str>) -> Tabela {
    let mut cabecalho = t.cabecalho.clone();
    let Some(outra) = outra else { return Tabela { cabecalho, linhas: t.linhas } };
    cabecalho.extend(outra.cabecalho.iter().cloned());

    let (esq, dir) = (esquerda.to_vec(), direita.to_vec());
    let mut indice: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, l) in outra.linhas.iter().enumerate() {
        indice.entry(chave_das(l, &dir)).or_default().push(i);
    }
    let vazia = vec![Value::Empty; outra.cabecalho.len()];
    let modo = modo.unwrap_or("esquerda");
    let mut usados = vec![false; outra.linhas.len()];
    let mut linhas = Vec::new();

    for l in &t.linhas {
        match indice.get(&chave_das(l, &esq)) {
            None => {
                if matches!(modo, "esquerda" | "total" | "anti_esquerda") {
                    let mut nova = l.clone();
                    nova.extend(vazia.iter().cloned());
                    linhas.push(nova);
                }
            }
            Some(pares) => {
                for &p in pares {
                    usados[p] = true;
                    if modo != "anti_esquerda" && modo != "anti_direita" {
                        let mut nova = l.clone();
                        nova.extend(outra.linhas[p].iter().cloned());
                        linhas.push(nova);
                    }
                }
            }
        }
    }
    if matches!(modo, "direita" | "total" | "anti_direita") {
        for (p, par) in outra.linhas.iter().enumerate() {
            if !usados[p] {
                let mut nova = vec![Value::Empty; t.cabecalho.len()];
                nova.extend(par.iter().cloned());
                linhas.push(nova);
            }
        }
    }
    Tabela { cabecalho, linhas }
}

fn agrupar(t: Tabela, por: &[usize], agregar: &[Agregado]) -> Tabela {
    // O grupo guarda a linha por referência: copiar cada linha da tabela só
    // para resumi-la dobrava a memória do agrupar.
    let mut grupos: Grupos<(Vec<Value>, Vec<&[Value]>)> = Grupos::novo();
    for l in &t.linhas {
        grupos
            .achar_ou(chave_das(l, por), || (por.iter().map(|&c| celula(l, c)).collect(), Vec::new()))
            .1
            .push(l.as_slice());
    }
    let mut cabecalho: Vec<String> = por.iter().map(|&c| nome_da_coluna(&t, c)).collect();
    for a in agregar {
        cabecalho.push(a.nome.clone().unwrap_or_else(|| format!("{} de {}", a.como.rotulo(), nome_da_coluna(&t, a.col))));
    }
    let linhas = grupos
        .itens
        .into_iter()
        .map(|(chave, ls)| {
            let mut linha = chave;
            linha.extend(agregar.iter().map(|a| resumir(&ls, a)));
            linha
        })
        .collect();
    Tabela { cabecalho, linhas }
}

fn pivotar(t: Tabela, campo_linha: &[usize], campo_coluna: &[usize], valores: &[Agregado]) -> Tabela {
    if campo_coluna.is_empty() {
        return agrupar(t, campo_linha, valores);
    }
    let mut colunas: Grupos<(String, Vec<Value>)> = Grupos::novo();
    let mut linhas_vistas: Grupos<(String, Vec<Value>)> = Grupos::novo();
    let mut celulas: HashMap<String, Vec<&[Value]>> = HashMap::new();
    for l in &t.linhas {
        let (kl, kc) = (chave_das(l, campo_linha), chave_das(l, campo_coluna));
        let vl: Vec<Value> = campo_linha.iter().map(|&c| celula(l, c)).collect();
        let vc: Vec<Value> = campo_coluna.iter().map(|&c| celula(l, c)).collect();
        linhas_vistas.achar_ou(kl.clone(), || (kl.clone(), vl));
        colunas.achar_ou(kc.clone(), || (kc.clone(), vc));
        celulas.entry(format!("{kl}\u{1}{kc}")).or_default().push(l.as_slice());
    }
    let mut cabecalho: Vec<String> = campo_linha.iter().map(|&c| nome_da_coluna(&t, c)).collect();
    for (_, vc) in &colunas.itens {
        let rotulo = vc.iter().map(texto_de).collect::<Vec<_>>().join(" / ");
        for a in valores {
            cabecalho.push(if valores.len() == 1 {
                rotulo.clone()
            } else {
                format!("{rotulo} · {}", a.nome.clone().unwrap_or_else(|| a.como.rotulo().to_string()))
            });
        }
    }
    let mut saida = Vec::new();
    for (kl, vl) in linhas_vistas.itens {
        let mut linha = vl;
        for (kc, _) in &colunas.itens {
            let grupo = celulas.get(&format!("{kl}\u{1}{kc}"));
            for a in valores {
                linha.push(grupo.map(|g| resumir(g, a)).unwrap_or(Value::Empty));
            }
        }
        saida.push(linha);
    }
    Tabela { cabecalho, linhas: saida }
}

/// Os números de um grupo: vazio, texto vazio e lógico ficam de fora, e o
/// resto passa pelo `to_number`.
fn numeros(valores: &[Value]) -> Vec<f64> {
    valores
        .iter()
        .filter(|v| !matches!(v, Value::Empty | Value::Bool(_)) && !matches!(v, Value::Text(s) if s.is_empty()))
        .filter_map(|v| to_number(v).ok())
        .collect()
}

/// Resume um grupo de linhas numa única célula (`resumir`).
fn resumir(linhas: &[&[Value]], a: &Agregado) -> Value {
    let brutos: Vec<Value> = linhas.iter().map(|l| celula(l, a.col)).collect();
    let preenchido = |v: &&Value| !matches!(v, Value::Empty) && !matches!(v, Value::Text(s) if s.is_empty());
    match a.como {
        Agregacao::Contagem => return Value::Number(numeros(&brutos).len() as f64),
        Agregacao::ContarValores => return Value::Number(brutos.iter().filter(preenchido).count() as f64),
        Agregacao::Distintos => {
            let distintos: HashSet<String> = brutos.iter().filter(preenchido).map(chave_de).collect();
            return Value::Number(distintos.len() as f64);
        }
        Agregacao::Primeiro => return brutos.first().cloned().unwrap_or(Value::Empty),
        Agregacao::Ultimo => return brutos.last().cloned().unwrap_or(Value::Empty),
        _ => {}
    }
    let nums = numeros(&brutos);
    if nums.is_empty() {
        return if a.como == Agregacao::Soma { Value::Number(0.0) } else { Value::Empty };
    }
    let soma = || nums.iter().fold(0.0, |s, n| s + n);
    Value::Number(match a.como {
        Agregacao::Soma => soma(),
        Agregacao::Media => soma() / nums.len() as f64,
        Agregacao::Maximo => nums.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        Agregacao::Minimo => nums.iter().copied().fold(f64::INFINITY, f64::min),
        Agregacao::Mediana => {
            let mut o = nums.clone();
            o.sort_by(|x, y| x.partial_cmp(y).unwrap_or(Ordering::Equal));
            let meio = o.len() / 2;
            if o.len() % 2 == 0 { (o[meio - 1] + o[meio]) / 2.0 } else { o[meio] }
        }
        _ => return Value::Empty,
    })
}

fn converter(v: Value, para: &str) -> Value {
    if matches!(v, Value::Empty) {
        return Value::Empty;
    }
    let numero = |v: &Value| to_number(v).ok();
    match para {
        "texto" => match v {
            Value::Text(_) => v,
            outro => Value::Text(texto_de(&outro)),
        },
        "logico" => match &v {
            Value::Bool(b) => Value::Bool(*b),
            Value::Number(n) => Value::Bool(*n != 0.0),
            outro => match string_js(outro).trim().to_uppercase().as_str() {
                "VERDADEIRO" | "TRUE" => Value::Bool(true),
                "FALSO" | "FALSE" => Value::Bool(false),
                _ => Value::Empty,
            },
        },
        "inteiro" => numero(&v).map(|n| Value::Number(n.trunc())).unwrap_or(Value::Empty),
        "decimal" | "numero" => numero(&v).map(Value::Number).unwrap_or(Value::Empty),
        "link" => {
            let s = string_js(&v);
            Value::Text(if s.starts_with("http") { s } else { format!("https://{s}") })
        }
        // ⚠ O `new Date(texto).toISOString()` do JavaScript não tem tradução
        // sem biblioteca de datas: número passa, texto vira vazio. Anotado.
        "data" | "dataHora" | "hora" => match v {
            Value::Number(_) => v,
            _ => Value::Empty,
        },
        _ => numero(&v).map(Value::Number).unwrap_or(Value::Empty),
    }
}

/// O pedaço pedido de uma data em série (`pedacoDaData`).
fn pedaco_da_data(v: &Value, parte: &str) -> Value {
    let Ok(n) = to_number(v) else { return Value::Empty };
    if !n.is_finite() || n < 1.0 {
        return Value::Empty;
    }
    let serial = n.floor() as i64;
    let (ano, mes, dia) = serial_to_ymd(serial);
    let mes0 = mes as usize - 1;
    match parte {
        "ano" => Value::Number(ano as f64),
        "trimestre" => Value::Text(format!("{ano} T{}", mes0 / 3 + 1)),
        "mes" => MESES.get(mes0).map(|m| Value::Text(m.to_string())).unwrap_or(Value::Empty),
        "anoMes" => Value::Text(format!("{ano}-{:02}", mes0 + 1)),
        "dia" => Value::Number(dia as f64),
        // 01/01/1970 (serial 25569) foi quinta-feira; o domingo é 0.
        "diaSemana" => Value::Text(DIAS[((serial - 25569).rem_euclid(7) as usize + 4) % 7].to_string()),
        _ => Value::Empty,
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn teste_executar_query_basico() {
        let fonte = Tabela {
            cabecalho: vec!["ID".into(), "Nome".into(), "Valor".into()],
            linhas: vec![
                vec![Value::Number(1.0), Value::Text("Ana".into()), Value::Number(100.0)],
                vec![Value::Number(2.0), Value::Text("Bob".into()), Value::Number(200.0)],
                vec![Value::Number(3.0), Value::Text("Carlos".into()), Value::Number(300.0)],
            ],
        };

        let passos = vec![
            Passo::Filtrar { col: 2, criterio: Value::Text(">150".into()) },
            Passo::Renomear { col: 1, nome: "Cliente".into() },
        ];

        let res = executar_query(fonte, &passos, &HashMap::new());
        assert_eq!(res.cabecalho, vec!["ID", "Cliente", "Valor"]);
        assert_eq!(res.linhas.len(), 2);
    }

    fn numeros_1_a_5() -> Tabela {
        Tabela {
            cabecalho: vec!["N".into()],
            linhas: (1..=5).map(|n| vec![Value::Number(f64::from(n))]).collect(),
        }
    }

    /// O passo como a ponte recebe: JSON vindo do assistente.
    fn passo(json: serde_json::Value) -> Passo {
        serde_json::from_value(json).expect("o passo tem de ser aceito")
    }

    #[test]
    fn limite_aceita_negativo_e_fracao_como_o_query_ts() {
        // Antes, `quantas: usize` recusava os dois e a consulta inteira caía.
        let negativo = passo(serde_json::json!({ "tipo": "limite", "quantas": -5 }));
        assert!(executar_query(numeros_1_a_5(), &[negativo], &HashMap::new()).linhas.is_empty());

        // O `primeiras` do TypeScript para quando já viu `n` ou mais: 2,5 → 3.
        let fracao = passo(serde_json::json!({ "tipo": "limite", "quantas": 2.5 }));
        assert_eq!(executar_query(numeros_1_a_5(), &[fracao], &HashMap::new()).linhas.len(), 3);

        let inteiro = passo(serde_json::json!({ "tipo": "limite", "quantas": 2 }));
        assert_eq!(executar_query(numeros_1_a_5(), &[inteiro], &HashMap::new()).linhas.len(), 2);
    }

    #[test]
    fn indice_aceita_inicio_e_incremento_fracionarios() {
        // Antes, `inicio: Option<i64>` recusava 0,5 e a consulta inteira caía.
        let p = passo(serde_json::json!({ "tipo": "indice", "inicio": 0.5, "incremento": 0.25 }));
        let r = executar_query(numeros_1_a_5(), &[p], &HashMap::new());
        let indices: Vec<Value> = r.linhas.iter().map(|l| l[1].clone()).collect();
        let esperado: Vec<Value> = [0.5, 0.75, 1.0, 1.25, 1.5].into_iter().map(Value::Number).collect();
        assert_eq!(indices, esperado);
        assert_eq!(r.cabecalho, vec!["N", "Índice"]);
    }

    #[test]
    fn acrescentar_alinha_pelo_nome_e_cai_na_posicao() {
        let fonte = Tabela { cabecalho: vec!["A".into(), "B".into()], linhas: vec![vec![Value::Number(1.0), Value::Number(2.0)]] };
        // "B" acha pelo nome (coluna 0 da outra); "A" não existe lá e vem da posição 0.
        let outra = Tabela { cabecalho: vec!["B".into(), "X".into()], linhas: vec![vec![Value::Number(20.0), Value::Number(99.0)]] };
        let fontes = HashMap::from([("Outra".to_string(), outra)]);
        let r = executar_query(fonte, &[Passo::Acrescentar { fonte: "Outra".into() }], &fontes);
        assert_eq!(r.linhas[1], vec![Value::Number(20.0), Value::Number(20.0)]);
    }

    #[test]
    fn agrupar_resume_cada_grupo_na_ordem_de_chegada() {
        let t = Tabela {
            cabecalho: vec!["R".into(), "V".into()],
            linhas: vec![
                vec![Value::Text("Sul".into()), Value::Number(1.0)],
                vec![Value::Text("Norte".into()), Value::Number(5.0)],
                vec![Value::Text("Sul".into()), Value::Number(2.0)],
            ],
        };
        let agregar = vec![Agregado { col: 1, como: Agregacao::Soma, nome: None }];
        let r = executar_query(t, &[Passo::Agrupar { por: vec![0], agregar }], &HashMap::new());
        assert_eq!(r.cabecalho, vec!["R", "Soma de V"]);
        assert_eq!(r.linhas, vec![
            vec![Value::Text("Sul".into()), Value::Number(3.0)],
            vec![Value::Text("Norte".into()), Value::Number(5.0)],
        ]);
    }

    #[test]
    fn dividir_em_colunas_como_o_power_query() {
        let t = Tabela {
            cabecalho: vec!["Id".into(), "Nome".into(), "Uf".into()],
            linhas: vec![
                vec![Value::Number(1.0), Value::Text("Eleição Ordinária".into()), Value::Text("RR".into())],
                vec![Value::Number(2.0), Value::Text("Suplementar".into()), Value::Text("AC".into())],
                vec![Value::Number(3.0), Value::Number(7.0), Value::Text("AP".into())],
            ],
        };
        let r = executar_query(t, &[Passo::DividirColunas { col: 1, separador: " ".into() }], &HashMap::new());
        assert_eq!(r.cabecalho, ["Id", "Nome.1", "Nome.2", "Uf"]);
        assert_eq!(r.linhas[0][1..3], [Value::Text("Eleição".into()), Value::Text("Ordinária".into())]);
        assert_eq!(r.linhas[1][1..4], [Value::Text("Suplementar".into()), Value::Empty, Value::Text("AC".into())]);
        assert_eq!(r.linhas[2][1..3], [Value::Number(7.0), Value::Empty]);
    }
}
