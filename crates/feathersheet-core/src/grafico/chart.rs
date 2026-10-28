//! O modelo do gráfico: de um intervalo da planilha aos números que o desenho
//! usa. Não desenha nada — o canvas continua no TypeScript (`ui/grafico/chart.ts`).
//!
//! Tradução de `src/core/chart.ts`. Os nomes públicos seguem o TypeScript
//! (`extractChartData` → [`extract_chart_data`]) para dar para pôr os dois lado
//! a lado. Os pontos que pedem atenção de quem mexer:
//!
//! - **Agrupar lê tudo, não a amostra.** Somar sobre uma amostra dá um total
//!   errado e calado — foi o defeito que o usuário pegou em 22/09/2026 (a barra
//!   489 vezes menor que o rodapé). O caminho que agrupa ACUMULA por categoria
//!   em vez de guardar uma entrada por linha; ver [`Acumulador`].
//! - **Conta por série** (`aggregateBySeries`) e **Moda**, de 22/09/2026. A
//!   série sem conta própria segue a do gráfico, então gráfico antigo não muda.
//! - **A leitura da célula vem de fora** ([`extract_chart_data_with`]): o
//!   modelo não sabe onde a célula mora, igual ao `EvalContext` do avaliador.
//!
//! Uma diferença consciente, travada por teste: o TypeScript caía no
//! `Date.parse` do navegador para reconhecer uma data escrita em texto que não
//! fosse `dd/mm/aaaa` nem `aaaa-mm-dd`. O que o `Date.parse` aceita varia de
//! motor para motor, então não há o que traduzir: aqui só as duas formas
//! explícitas viram data. Ver `data_em_texto_livre_nao_vira_data`.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::celulas::date_serial::serial_to_ymd;
use crate::celulas::number::to_js_string;
use crate::planilha::sheet::Sheet;
use crate::celulas::values::Value;

// ------------------------------------------------------------------ tipos

/// O tipo do gráfico. Só o `Scatter` muda os dados (a primeira série vira o
/// eixo X); os outros importam para o desenho e para [`get_stack_mode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChartType {
    #[default]
    Column,
    ColumnStacked,
    #[serde(rename = "column-stacked-100")]
    ColumnStacked100,
    Bar,
    BarStacked,
    #[serde(rename = "bar-stacked-100")]
    BarStacked100,
    Line,
    /// Linha no espaço: a 1ª série é o X, a 2ª o Y e a 3ª o Z.
    #[serde(rename = "line-3d")]
    Line3d,
    StepLine,
    StepLineStacked,
    #[serde(rename = "step-line-stacked-100")]
    StepLineStacked100,
    Area,
    StepArea,
    StepAreaStacked,
    #[serde(rename = "step-area-stacked-100")]
    StepAreaStacked100,
    // shortcut: a rosca do .xlsx sai como pizza cheia (o desenhista novo não tem anel), até o feather-charts ganhar a rosca.
    #[serde(alias = "doughnut")]
    Pie,
    Scatter,
    Bubble,
    Combo,
    Heatmap,
    Waterfall,
    Histogram,
    Radar,
    Candlestick,
    #[serde(rename = "gantt")]
    Gantt,
    /// Tipo que este código ainda não conhece: abre como coluna em vez de
    /// derrubar a leitura do arquivo inteiro.
    #[serde(other)]
    Outro,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StackMode {
    None,
    Stacked,
    Percent,
}

/// As contas que o gráfico sabe fazer ao agrupar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateFn {
    #[default]
    Sum,
    Avg,
    Count,
    Min,
    Max,
    Stddev,
    Mode,
    /// Quantos valores diferentes há na categoria (célula vazia não conta).
    /// Serve para série de texto, onde soma e média não têm sentido.
    #[serde(rename = "countDistinct")]
    CountDistinct,
}

impl AggregateFn {
    /// As contas que fazem sentido numa série de texto: só contar.
    pub fn serve_para_texto(self) -> bool {
        matches!(self, AggregateFn::Count | AggregateFn::CountDistinct)
    }

    /// O nome de cada uma, para o painel e para a legenda (`AGGREGATE_LABELS`).
    pub fn rotulo(self) -> &'static str {
        match self {
            AggregateFn::Sum => "Soma",
            AggregateFn::Avg => "Média",
            AggregateFn::Count => "Contagem",
            AggregateFn::Min => "Mínimo",
            AggregateFn::Max => "Máximo",
            AggregateFn::Stddev => "Desvio padrão",
            AggregateFn::Mode => "Moda",
            AggregateFn::CountDistinct => "Contagem distinta",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateHierarchy {
    #[default]
    None,
    Day,
    Month,
    Year,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PaletteName {
    Vibrant,
    Ocean,
    Forest,
    Sunset,
    Monochrome,
    Classic,
}

/// O intervalo do gráfico, como vem no arquivo (`{ r0, c0, r1, c1 }`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChartRange {
    pub r0: u32,
    pub c0: u32,
    pub r1: u32,
    pub c1: u32,
}

/// Só a parte do `ChartStyleConfig` que muda os DADOS ou a cor da série. O
/// resto (fonte, título, legenda…) é desenho e fica no TypeScript; quem
/// guarda o arquivo guarda o JSON inteiro, não esta struct.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChartStyle {
    pub hidden_series: Vec<u32>,
    pub date_hierarchy: Option<DateHierarchy>,
    pub stack_mode: Option<StackMode>,
    pub palette: Option<PaletteName>,
    pub series_colors: BTreeMap<u32, String>,
    pub point_shape: Option<String>,
    pub point_size: Option<f64>,
    pub series_point_shapes: BTreeMap<u32, String>,
    pub series_point_sizes: BTreeMap<u32, f64>,
}

/// O `ChartConfig` do TypeScript, sem `position` (é desenho).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ChartConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub tipo: ChartType,
    pub title: String,
    pub range: ChartRange,
    pub source_sheet: Option<String>,
    pub series_in_columns: bool,
    pub has_headers: bool,
    pub has_category_labels: bool,
    pub category_column_index: Option<u32>,
    pub aggregate: bool,
    /// A conta de quem não escolheu a sua em `aggregate_by_series`.
    pub aggregate_fn: Option<AggregateFn>,
    /// A conta de cada série, pela ordem das colunas de dados — a mesma chave
    /// do `hidden_series`, sem a coluna de categoria.
    pub aggregate_by_series: BTreeMap<u32, AggregateFn>,
    /// Linha 3D: as colunas (posição dentro do intervalo) de X, Y e Z,
    /// escolhidas no painel. Sem isto, a ordem das colunas numéricas decide.
    #[serde(rename = "eixos3d")]
    pub eixos_3d: Option<Eixos3d>,
    pub style: ChartStyle,
    /// Só as linhas visíveis (o `plotVisOnly` do Excel): a linha escondida
    /// pelo filtro fica fora do gráfico. Desligado nos gráficos feitos aqui.
    pub plot_vis_only: bool,
    /// As colunas (da aba) que viram série, como o Excel guarda cada `<c:ser>`;
    /// vazio = todas as numéricas do intervalo. Com séries que pulam colunas
    /// (B + L, M, N) as do meio ficam fora sem precisar de `hidden_series`.
    pub series_columns: Vec<u32>,
}

/// As colunas dos três eixos da linha 3D, relativas ao começo do intervalo.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Eixos3d {
    pub x: u32,
    pub y: u32,
    #[serde(default)]
    pub z: Option<u32>,
    /// A coluna de descrição: uma linha do gráfico para cada valor dela.
    #[serde(default)]
    pub grupo: Option<u32>,
}

/// Acima disto os valores do grupo viram um aviso, e não mais linhas.
pub const MAX_GRUPOS_3D: usize = 24;

/// Os eixos quando o painel ainda não escolheu: as três primeiras colunas
/// com número na primeira linha de dados, e a primeira de texto como grupo
/// (o painel mostra a mesma coisa — `eixosPadrao3d` no chart-panel.ts).
pub fn eixos_3d_padrao(num_cols: u32, primeira: impl Fn(u32) -> Value) -> Eixos3d {
    let tipos: Vec<Value> = (0..num_cols).map(&primeira).collect();
    let numericas: Vec<u32> = (0..num_cols).filter(|&i| matches!(tipos[i as usize], Value::Number(_))).collect();
    let grupo = (0..num_cols).find(|&i| matches!(&tipos[i as usize], Value::Text(s) if !s.trim().is_empty()));
    let pega = |k: usize, reserva: u32| numericas.get(k).copied().unwrap_or(reserva.min(num_cols.saturating_sub(1)));
    Eixos3d { x: pega(0, 0), y: pega(1, 1), z: numericas.get(2).copied(), grupo }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<f64>,
}

/// O corte feito por amostragem, para a tela poder AVISAR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amostra {
    pub disponiveis: usize,
    pub usados: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChartData {
    pub title: String,
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,
    pub x_values: Option<Vec<f64>>,
    /// A profundidade da linha 3D (`line-3d`).
    pub z_values: Option<Vec<f64>>,
    pub amostrado: Option<Amostra>,
    /// Outro aviso para a tela, como a moda que passou do teto.
    pub aviso: Option<String>,
}

// ------------------------------------------------------------- constantes

/// O teto de pontos por série quando o gráfico NÃO agrupa (`MAX_PONTOS_POR_SERIE`).
const MAX_PONTOS_POR_SERIE: usize = 4096;

/// Quantas categorias distintas a agregação exata aceita antes de desistir e
/// deixar o caminho amostrado assumir (que avisa na tela).
const MAX_CATEGORIAS_AGREGADAS: usize = 10_000;

/// Quantos valores distintos a moda de uma série pode contar, somando todas
/// as categorias. Passando disso a série fica sem moda e o gráfico avisa.
const MAX_VALORES_MODA: usize = 1_000_000;

// ------------------------------------------------------------- acumulador

/// O que se guarda por série e por categoria ao agrupar: cinco números bastam
/// para qualquer conta, menos a moda, que guarda a frequência de cada valor,
/// e a contagem distinta, que guarda a marca de cada valor visto.
#[derive(Clone, Debug)]
struct Acumulador {
    n: u64,
    soma: f64,
    soma_q: f64,
    min: f64,
    max: f64,
    freq: Option<Frequencia>,
    /// Marca (hash de 64 bits) de cada valor diferente, só na contagem
    /// distinta: guardar o texto inteiro de um milhão de linhas pesaria.
    distintos: Option<HashSet<u64>>,
}

/// Frequência com a ordem de chegada, que é o que decide o empate da moda
/// (o `Map` do JavaScript guarda essa ordem sozinho).
#[derive(Clone, Debug, Default)]
struct Frequencia {
    posicao: HashMap<u64, usize>,
    contagens: Vec<(f64, u64)>,
}

/// A chave de um número no mapa, com a igualdade do `Map` do JavaScript
/// (SameValueZero): `0` e `-0` são o mesmo, e todo `NaN` é o mesmo.
fn chave_do_numero(v: f64) -> u64 {
    if v == 0.0 {
        0
    } else if v.is_nan() {
        f64::NAN.to_bits()
    } else {
        v.to_bits()
    }
}

/// A marca de uma célula para a contagem distinta. O tipo da célula entra
/// na marca: `"12"` escrito como texto e o número `12` são valores
/// diferentes. Maiúscula e minúscula também ("Ana" e "ana" contam dois).
/// Vazio não é valor: `None`.
fn chave_distinta(v: &Value) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match v {
        Value::Empty => return None,
        Value::Number(n) => (0u8, chave_do_numero(*n)).hash(&mut h),
        Value::Text(t) if t.is_empty() => return None,
        Value::Text(t) => (1u8, t.as_str()).hash(&mut h),
        Value::Bool(b) => (2u8, *b).hash(&mut h),
        Value::Error(e) => (3u8, e.code.as_str()).hash(&mut h),
    }
    Some(h.finish())
}

impl Acumulador {
    fn novo(moda: bool, distintos: bool) -> Self {
        Acumulador {
            n: 0,
            soma: 0.0,
            soma_q: 0.0,
            min: f64::INFINITY,
            max: f64::NEG_INFINITY,
            freq: moda.then(Frequencia::default),
            distintos: distintos.then(HashSet::new),
        }
    }

    /// Anota a marca de um valor para a contagem distinta (se ela é a conta).
    fn anotar_distinto(&mut self, chave: Option<u64>) {
        if let (Some(conjunto), Some(chave)) = (self.distintos.as_mut(), chave) {
            conjunto.insert(chave);
        }
    }

    /// Soma `v`; devolve `true` quando a moda viu um valor novo.
    fn acumular(&mut self, v: f64) -> bool {
        self.n += 1;
        self.soma += v;
        self.soma_q += v * v;
        if v < self.min {
            self.min = v;
        }
        if v > self.max {
            self.max = v;
        }
        let Some(freq) = self.freq.as_mut() else { return false };
        match freq.posicao.get(&chave_do_numero(v)) {
            Some(&i) => {
                freq.contagens[i].1 += 1;
                false
            }
            None => {
                freq.posicao.insert(chave_do_numero(v), freq.contagens.len());
                freq.contagens.push((v, 1));
                true
            }
        }
    }

    fn resultado(&self, conta: AggregateFn) -> f64 {
        let n = self.n as f64;
        match conta {
            AggregateFn::Count => n,
            AggregateFn::Avg => {
                if self.n > 0 {
                    self.soma / n
                } else {
                    0.0
                }
            }
            AggregateFn::Min => {
                if self.n > 0 {
                    self.min
                } else {
                    0.0
                }
            }
            AggregateFn::Max => {
                if self.n > 0 {
                    self.max
                } else {
                    0.0
                }
            }
            AggregateFn::Stddev => {
                // Variância AMOSTRAL (n-1), como o DESVPAD; uma amostra só
                // não tem dispersão a medir.
                if self.n < 2 {
                    return 0.0;
                }
                let variancia = (self.soma_q - (self.soma * self.soma) / n) / (n - 1.0);
                if variancia > 0.0 {
                    variancia.sqrt()
                } else {
                    0.0
                }
            }
            AggregateFn::Mode => {
                // Como o MODO.ÚNICO: o que mais se repete, e no empate o que
                // chegou primeiro. Sem repetição não há moda: zero.
                let Some(freq) = &self.freq else { return 0.0 };
                let mut melhor = 0.0;
                let mut vezes = 1;
                for &(valor, n) in &freq.contagens {
                    if n > vezes {
                        melhor = valor;
                        vezes = n;
                    }
                }
                melhor
            }
            AggregateFn::CountDistinct => self.distintos.as_ref().map_or(0.0, |d| d.len() as f64),
            AggregateFn::Sum => self.soma,
        }
    }
}

/// A conta que vale para a série de ordem `ordem` (`contaDaSerie`).
pub fn conta_da_serie(config: &ChartConfig, ordem: u32) -> AggregateFn {
    config
        .aggregate_by_series
        .get(&ordem)
        .copied()
        .or(config.aggregate_fn)
        .unwrap_or_default()
}

/// "Média de ValorB" quando a série escolheu a sua conta; senão, o nome puro.
fn nome_com_conta(config: &ChartConfig, ordem: u32, nome: String) -> String {
    match config.aggregate_by_series.get(&ordem) {
        Some(conta) => format!("{} de {}", conta.rotulo(), nome),
        None => nome,
    }
}

// ---------------------------------------------------------------- leitura

/// O `parseFloat` do JavaScript: lê o número do COMEÇO do texto e ignora o
/// resto (`"12abc"` é 12); `NaN` quando não começa por número.
fn parse_float_js(texto: &str) -> f64 {
    let s = texto.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{feff}');
    let bytes = s.as_bytes();
    let mut i = 0;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    if s[i..].starts_with("Infinity") {
        return if bytes.first() == Some(&b'-') { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    let inicio_digitos = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let mut digitos = i - inicio_digitos;
    if i < bytes.len() && bytes[i] == b'.' {
        let depois = i + 1;
        let mut j = depois;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        digitos += j - depois;
        if digitos > 0 {
            i = j;
        }
    }
    if digitos == 0 {
        return f64::NAN;
    }
    // Expoente só conta se vier com dígito: "1e" é 1.
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let k = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > k {
            i = j;
        }
    }
    s[..i].parse::<f64>().unwrap_or(f64::NAN)
}

/// O número de uma célula para o gráfico (`getCellNum`): texto passa pelo
/// `parseFloat` com a primeira vírgula virando ponto; o que não é número vale 0.
fn numero_da_celula(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Text(t) => {
            let n = parse_float_js(&t.replacen(',', ".", 1));
            if n.is_nan() {
                0.0
            } else {
                n
            }
        }
        _ => 0.0,
    }
}

/// O texto de uma célula para o gráfico (`getCellStr`): o `String(valor)` do
/// JavaScript, e o código no caso de erro.
fn texto_da_celula(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => to_js_string(*n),
        Value::Text(t) => t.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Error(e) => e.code.as_str().to_string(),
    }
}

/// [`texto_da_celula`] consumindo o valor: o texto sai sem cópia (agrupar
/// lê a coluna de categoria inteira, até um milhão de linhas).
fn texto_movido(v: Value) -> String {
    match v {
        Value::Text(t) => t,
        outro => texto_da_celula(&outro),
    }
}

/// Se a célula conta como número para decidir que a coluna é série.
fn parece_numero(v: &Value) -> bool {
    match v {
        Value::Number(_) => true,
        Value::Text(t) => !t.trim().is_empty() && !parse_float_js(&t.replacen(',', ".", 1)).is_nan(),
        _ => false,
    }
}

// ------------------------------------------------------------------ datas

/// Dias desde 01/01/1970 de uma data civil (algoritmo de Howard Hinnant).
fn dias_desde_1970(ano: i64, mes: i64, dia: i64) -> i64 {
    let a = if mes <= 2 { ano - 1 } else { ano };
    let era = if a >= 0 { a } else { a - 399 } / 400;
    let ano_da_era = a - era * 400;
    let m = if mes > 2 { mes - 3 } else { mes + 9 };
    let dia_do_ano = (153 * m + 2) / 5 + dia - 1;
    let dia_da_era = ano_da_era * 365 + ano_da_era / 4 - ano_da_era / 100 + dia_do_ano;
    era * 146_097 + dia_da_era - 719_468
}

/// O `Date.UTC(ano, mes, dia)` do JavaScript, em (ano, mês 1-12, dia): o dia
/// que passa do fim do mês ROLA para o seguinte (31/02 vira 03/03), e ano de
/// 0 a 99 é lido como 1900 + ano.
fn data_utc(ano: i64, mes0: i64, dia: i64) -> (i32, u32, u32) {
    let ano = if (0..=99).contains(&ano) { 1900 + ano } else { ano };
    let dias = dias_desde_1970(ano, mes0 + 1, 1) + (dia - 1);
    serial_to_ymd(dias + 25_569)
}

/// Uma data de categoria, em (ano, mês, dia) — `parseCategoryDate`.
///
/// Número entre 1000 e 100000 é serial de data; texto vale em `dd/mm/aaaa`
/// (ano de dois dígitos vira 20xx) ou `aaaa-mm-dd`, com `/`, `.` ou `-`.
pub fn parse_category_date(v: &Value) -> Option<(i32, u32, u32)> {
    match v {
        Value::Number(n) => {
            if (1000.0..=100_000.0).contains(n) {
                Some(serial_to_ymd(n.floor() as i64))
            } else {
                None
            }
        }
        Value::Text(t) => parse_category_date_text(t),
        _ => None,
    }
}

fn parse_category_date_text(texto: &str) -> Option<(i32, u32, u32)> {
    let s = texto.trim();
    if s.is_empty() {
        return None;
    }
    let partes: Vec<&str> = s.split(['/', '.', '-']).collect();
    if partes.len() != 3 || partes.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit())) {
        return None;
    }
    let num = |p: &str| p.parse::<i64>().ok();
    let (a, b, c) = (partes[0], partes[1], partes[2]);
    let dentro = |dia: i64, mes0: i64| (1..=31).contains(&dia) && (0..=11).contains(&mes0);

    // dd/mm/aaaa (ou aa)
    if (1..=2).contains(&a.len()) && (1..=2).contains(&b.len()) && (2..=4).contains(&c.len()) {
        let (dia, mes0, mut ano) = (num(a)?, num(b)? - 1, num(c)?);
        if ano < 100 {
            ano += 2000;
        }
        if dentro(dia, mes0) {
            return Some(data_utc(ano, mes0, dia));
        }
    }
    // aaaa-mm-dd
    if a.len() == 4 && (1..=2).contains(&b.len()) && (1..=2).contains(&c.len()) {
        let (ano, mes0, dia) = (num(a)?, num(b)? - 1, num(c)?);
        if dentro(dia, mes0) {
            return Some(data_utc(ano, mes0, dia));
        }
    }
    None
}

/// `formatCategoryDate`: `dd/mm/aaaa`, `mm/aaaa` ou `aaaa`.
pub fn format_category_date((ano, mes, dia): (i32, u32, u32), nivel: DateHierarchy) -> String {
    match nivel {
        DateHierarchy::Month => format!("{mes:02}/{ano}"),
        DateHierarchy::Year => format!("{ano}"),
        DateHierarchy::Day | DateHierarchy::None => format!("{dia:02}/{mes:02}/{ano}"),
    }
}

// ------------------------------------------------------------- amostragem

fn indices_todos(inicio: u32, fim: u32) -> Vec<u32> {
    if fim < inicio {
        return Vec::new();
    }
    (inicio..=fim).collect()
}

/// No máximo [`MAX_PONTOS_POR_SERIE`] índices, garantindo o último.
fn indices_amostrados(inicio: u32, fim: u32) -> Vec<u32> {
    if fim < inicio {
        return Vec::new();
    }
    let total = (fim - inicio + 1) as usize;
    if total <= MAX_PONTOS_POR_SERIE {
        return indices_todos(inicio, fim);
    }
    let passo = total.div_ceil(MAX_PONTOS_POR_SERIE);
    let mut fora: Vec<u32> = (inicio..=fim).step_by(passo).collect();
    if fora.last() != Some(&fim) {
        fora.push(fim);
    }
    fora
}

/// As linhas visíveis de `inicio..=fim`, com no máximo [`MAX_PONTOS_POR_SERIE`].
fn visiveis_amostradas(inicio: u32, fim: u32, oculta: &dyn Fn(u32) -> bool) -> Vec<u32> {
    if fim < inicio {
        return Vec::new();
    }
    let todas: Vec<u32> = (inicio..=fim).filter(|&r| !oculta(r)).collect();
    if todas.len() <= MAX_PONTOS_POR_SERIE {
        return todas;
    }
    let passo = todas.len().div_ceil(MAX_PONTOS_POR_SERIE);
    let mut fora: Vec<u32> = todas.iter().copied().step_by(passo).collect();
    if fora.last() != todas.last() {
        fora.extend(todas.last());
    }
    fora
}

// ---------------------------------------------------------------- extração

/// `extractChartData` lendo de um [`Sheet`].
pub fn extract_chart_data(sheet: &Sheet, config: &ChartConfig) -> ChartData {
    extract_chart_data_with(&|r, c| sheet.get_value(r, c), config)
}

/// `extractChartData`: os dados que o gráfico desenha. `celula(linha, coluna)`
/// é quem sabe onde a célula mora.
pub fn extract_chart_data_with(celula: &dyn Fn(u32, u32) -> Value, config: &ChartConfig) -> ChartData {
    extract_chart_data_visiveis(celula, &|_| false, config)
}

/// Como [`extract_chart_data_with`], pulando as linhas em que `oculta` diz
/// sim quando o gráfico pede `plot_vis_only`.
// shortcut: só séries em colunas; com séries em linhas a coluna oculta continua entrando.
pub fn extract_chart_data_visiveis(
    celula: &dyn Fn(u32, u32) -> Value,
    oculta: &dyn Fn(u32) -> bool,
    config: &ChartConfig,
) -> ChartData {
    let oculta = |r: u32| config.plot_vis_only && oculta(r);
    let fora_das_series = |c: u32| !config.series_columns.is_empty() && !config.series_columns.contains(&c);
    let ChartRange { r0, c0, r1, c1 } = config.range;
    let titulo = if config.title.is_empty() { "Gráfico".to_string() } else { config.title.clone() };
    let vazio = |titulo: String| ChartData {
        title: titulo,
        categories: Vec::new(),
        series: Vec::new(),
        x_values: None,
        z_values: None,
        amostrado: None,
        aviso: None,
    };
    if r1 < r0 || c1 < c0 {
        return vazio(titulo);
    }
    let num_rows = r1 - r0 + 1;
    let num_cols = c1 - c0 + 1;
    let header_offset = u32::from(config.has_headers && num_rows > 1);
    let hierarquia = config.style.date_hierarchy.unwrap_or_default();
    let com_hierarquia = hierarquia != DateHierarchy::None;

    let str_de = |r: u32, c: u32| texto_da_celula(&celula(r, c));
    // A categoria de uma célula, passando pela hierarquia de datas. A célula
    // vazia vira "Item {item}" — montado só quando precisa, não a cada linha.
    let categoria = |r: u32, c: u32, item: u32| -> String {
        let valor = celula(r, c);
        if com_hierarquia {
            if let Some(data) = parse_category_date(&valor) {
                return format_category_date(data, hierarquia);
            }
        }
        let bruto = texto_movido(valor);
        if bruto.is_empty() { format!("Item {item}") } else { bruto }
    };
    let nome_da_coluna = |c: u32| {
        let reserva = format!("Série {}", c - c0 + 1);
        if header_offset > 0 {
            let s = str_de(r0, c);
            if s.is_empty() { reserva } else { s }
        } else {
            reserva
        }
    };

    let mut amostrado: Option<Amostra> = None;
    let mut anotar_amostra = |disponiveis: usize, usados: usize| {
        if usados >= disponiveis {
            return;
        }
        if amostrado.map_or(true, |a| disponiveis > a.disponiveis) {
            amostrado = Some(Amostra { disponiveis, usados });
        }
    };

    // Linha 3D com as colunas escolhidas no painel: X, Y e Z lidos direto,
    // como número, sem coluna de categoria nem a regra "só coluna numérica".
    // Sem escolha, as três primeiras colunas (o mesmo que o painel mostra).
    if let (ChartType::Line3d, true) = (config.tipo, config.series_in_columns) {
        let data_row_start = r0 + header_offset;
        let e = config.eixos_3d.unwrap_or_else(|| eixos_3d_padrao(num_cols, |i| celula(data_row_start, c0 + i)));
        let linhas = indices_amostrados(data_row_start, r1);
        anotar_amostra((r1 + 1).saturating_sub(data_row_start) as usize, linhas.len());
        let coluna = |i: u32| c0 + i.min(num_cols - 1);
        let ler = |c: u32| linhas.iter().map(|&r| numero_da_celula(&celula(r, c))).collect::<Vec<f64>>();
        let y = coluna(e.y);
        let (categories, series, aviso) = match e.grupo.map(coluna) {
            // Uma série por valor da descrição: cada uma só tem número nas
            // linhas dela (NaN nas outras, que o desenho pula).
            Some(g) => {
                let rotulos: Vec<String> = linhas.iter().map(|&r| texto_da_celula(&celula(r, g)).trim().to_string()).collect();
                let mut grupos: Vec<String> = Vec::new();
                let mut sobrou = false;
                for k in &rotulos {
                    if !grupos.contains(k) {
                        if grupos.len() < MAX_GRUPOS_3D {
                            grupos.push(k.clone());
                        } else {
                            sobrou = true;
                        }
                    }
                }
                let ys = ler(y);
                let series = grupos
                    .iter()
                    .map(|k| ChartSeries {
                        name: if k.is_empty() { "(vazio)".into() } else { k.clone() },
                        values: ys.iter().zip(&rotulos).map(|(v, r)| if r == k { *v } else { f64::NAN }).collect(),
                    })
                    .collect();
                let aviso = sobrou.then(|| format!("Só os {MAX_GRUPOS_3D} primeiros valores da descrição viram linhas."));
                (rotulos, series, aviso)
            }
            None => (
                linhas.iter().map(|&r| (r - data_row_start + 1).to_string()).collect(),
                vec![ChartSeries { name: nome_da_coluna(y), values: ler(y) }],
                None,
            ),
        };
        return ChartData {
            title: titulo,
            categories,
            series,
            x_values: Some(ler(coluna(e.x))),
            z_values: e.z.map(|z| ler(coluna(z))),
            amostrado,
            aviso,
        };
    }

    let vai_agregar = config.aggregate || com_hierarquia;

    // ⚠ Agrupando, lê TODAS as linhas e acumula — nunca a amostra.
    if vai_agregar && config.series_in_columns && config.has_category_labels {
        let cat_col = c0 + config.category_column_index.unwrap_or(0).min(num_cols - 1);
        let data_row_start = r0 + header_offset;

        // As escondidas saem antes de ler qualquer célula.
        let escondidas: std::collections::HashSet<u32> = config.style.hidden_series.iter().copied().collect();
        let mut colunas: Vec<u32> = Vec::new();
        let mut ordens: Vec<u32> = Vec::new();
        let mut ordem = 0;
        for c in c0..=c1 {
            if c == cat_col || fora_das_series(c) {
                continue;
            }
            if !escondidas.contains(&ordem) {
                colunas.push(c);
                ordens.push(ordem);
            }
            ordem += 1;
        }
        let contas: Vec<AggregateFn> = ordens.iter().map(|&o| conta_da_serie(config, o)).collect();

        let mut indice: HashMap<String, usize> = HashMap::new();
        let mut nomes: Vec<String> = Vec::new();
        let mut acc: Vec<Vec<Acumulador>> = vec![Vec::new(); colunas.len()];
        let mut distintos = vec![0usize; colunas.len()];
        let mut moda_estourou = vec![false; colunas.len()];
        let mut desistiu = false;

        for r in (data_row_start..=r1).filter(|&r| !oculta(r)) {
            let cat = categoria(r, cat_col, r - data_row_start + 1);
            let at = match indice.get(&cat) {
                Some(&i) => i,
                None => {
                    if nomes.len() >= MAX_CATEGORIAS_AGREGADAS {
                        desistiu = true;
                        break;
                    }
                    let i = nomes.len();
                    indice.insert(cat.clone(), i);
                    nomes.push(cat);
                    for (k, linha) in acc.iter_mut().enumerate() {
                        linha.push(Acumulador::novo(
                            contas[k] == AggregateFn::Mode && !moda_estourou[k],
                            contas[k] == AggregateFn::CountDistinct,
                        ));
                    }
                    i
                }
            };
            for (k, &c) in colunas.iter().enumerate() {
                // Uma leitura da célula serve ao número e à contagem distinta.
                let valor = celula(r, c);
                if contas[k] == AggregateFn::CountDistinct {
                    acc[k][at].anotar_distinto(chave_distinta(&valor));
                }
                if acc[k][at].acumular(numero_da_celula(&valor)) {
                    distintos[k] += 1;
                    if distintos[k] > MAX_VALORES_MODA {
                        // Solta a memória da moda desta série de uma vez.
                        moda_estourou[k] = true;
                        for a in &mut acc[k] {
                            a.freq = None;
                        }
                    }
                }
            }
        }

        if !desistiu {
            let mut sem_moda: Vec<String> = Vec::new();
            let mut series = Vec::with_capacity(colunas.len());
            for (k, &c) in colunas.iter().enumerate() {
                let nome = nome_da_coluna(c);
                if moda_estourou[k] {
                    sem_moda.push(nome.clone());
                }
                series.push(ChartSeries {
                    name: nome_com_conta(config, ordens[k], nome),
                    values: acc[k].iter().map(|a| a.resultado(contas[k])).collect(),
                });
            }
            return ChartData {
                title: titulo,
                categories: nomes,
                series,
                x_values: None,
        z_values: None,
                // Sem amostra: este caminho leu tudo.
                amostrado: None,
                aviso: (!sem_moda.is_empty()).then(|| {
                    format!("Valores distintos demais para calcular a moda de {}.", sem_moda.join(", "))
                }),
            };
        }
    }

    let mut categories: Vec<String> = Vec::new();
    let mut series: Vec<ChartSeries> = Vec::new();

    if config.series_in_columns {
        let data_row_start = r0 + header_offset;
        let cat_col = c0 + config.category_column_index.unwrap_or(0).min(num_cols - 1);
        let use_cat = config.has_category_labels;

        let linhas = if config.plot_vis_only {
            visiveis_amostradas(data_row_start, r1, &oculta)
        } else {
            indices_amostrados(data_row_start, r1)
        };
        anotar_amostra((r1 + 1).saturating_sub(data_row_start) as usize, linhas.len());

        for &r in &linhas {
            let item = r - data_row_start + 1;
            categories.push(if use_cat { categoria(r, cat_col, item) } else { format!("Item {item}") });
        }
        for c in c0..=c1 {
            if (use_cat && c == cat_col) || fora_das_series(c) {
                continue;
            }
            let mut numerica = false;
            let values = linhas
                .iter()
                .map(|&r| {
                    let v = celula(r, c);
                    let e_num = parece_numero(&v);
                    numerica |= e_num;
                    if e_num { numero_da_celula(&v) } else { f64::NAN }
                })
                .collect();
            if numerica {
                series.push(ChartSeries { name: nome_da_coluna(c), values });
            }
        }
    } else {
        let data_row_start = r0 + header_offset;
        let cat_row = r0 + config.category_column_index.unwrap_or(0).min(num_rows - 1);
        let use_cat = config.has_category_labels;

        let mut data_col_start = c0 + u32::from(use_cat || config.has_headers);
        if config.has_headers && num_cols > 2 {
            for c_check in data_col_start..c1 {
                let mut has_num = false;
                for r_check in data_row_start..=r1 {
                    let v = celula(r_check, c_check);
                    if parece_numero(&v) {
                        has_num = true;
                        break;
                    }
                }
                if !has_num {
                    data_col_start = c_check + 1;
                } else {
                    break;
                }
            }
        }

        let colunas =
            if vai_agregar { indices_todos(data_col_start, c1) } else { indices_amostrados(data_col_start, c1) };
        let linhas_serie = indices_amostrados(data_row_start, r1);
        anotar_amostra((c1 + 1).saturating_sub(data_col_start) as usize, colunas.len());
        anotar_amostra((r1 + 1).saturating_sub(data_row_start) as usize, linhas_serie.len());

        for &c in &colunas {
            let item = c - data_col_start + 1;
            categories.push(if use_cat { categoria(cat_row, c, item) } else { format!("Item {item}") });
        }
        for &r in &linhas_serie {
            if use_cat && r == cat_row {
                continue;
            }
            let name = if config.has_headers {
                let mut name_parts = Vec::new();
                for lc in c0..data_col_start {
                    let s = str_de(r, lc).trim().to_string();
                    if !s.is_empty() && !name_parts.contains(&s) {
                        name_parts.push(s);
                    }
                }
                if name_parts.is_empty() {
                    format!("Série {}", r - r0 + 1)
                } else {
                    name_parts.join(" - ")
                }
            } else {
                format!("Série {}", r - r0 + 1)
            };
            let mut numerica = false;
            let values = colunas
                .iter()
                .map(|&c| {
                    let v = celula(r, c);
                    let e_num = parece_numero(&v);
                    numerica |= e_num;
                    if e_num { numero_da_celula(&v) } else { f64::NAN }
                })
                .collect();
            if numerica {
                series.push(ChartSeries { name, values });
            }
        }
    }

    let (mut x_values, mut z_values) = (None, None);
    if config.tipo == ChartType::Scatter && series.len() > 1 {
        x_values = Some(series.remove(0).values);
    } else if config.tipo == ChartType::Line3d && series.len() >= 2 {
        // X, Y e (se houver) Z: sobra uma série só, a do Y.
        x_values = Some(series.remove(0).values);
        if series.len() >= 2 {
            z_values = Some(series.remove(1).values);
        }
        series.truncate(1);
    }

    // A ordem original de cada série, que sobrevive ao filtro das escondidas.
    let mut ordens: Vec<u32> = (0..series.len() as u32).collect();
    if !config.style.hidden_series.is_empty() {
        let escondidas: std::collections::HashSet<u32> = config.style.hidden_series.iter().copied().collect();
        let mut k = 0;
        series.retain(|_| {
            let fica = !escondidas.contains(&(k as u32));
            k += 1;
            fica
        });
        ordens.retain(|o| !escondidas.contains(o));
    }

    if vai_agregar && !categories.is_empty() && !series.is_empty() {
        let mut unicas: Vec<String> = Vec::new();
        let mut mapa: HashMap<&str, usize> = HashMap::new();
        let mut destino = Vec::with_capacity(categories.len());
        for cat in &categories {
            let i = *mapa.entry(cat.as_str()).or_insert_with(|| {
                unicas.push(cat.clone());
                unicas.len() - 1
            });
            destino.push(i);
        }
        if unicas.len() < categories.len() {
            // Os valores já estão em memória (vieram amostrados): a moda aqui
            // não tem teto a respeitar.
            let agregadas = series
                .into_iter()
                .zip(&ordens)
                .map(|(s, &ordem)| {
                    let conta = conta_da_serie(config, ordem);
                    let distinta = conta == AggregateFn::CountDistinct;
                    let mut acc: Vec<Acumulador> =
                        (0..unicas.len()).map(|_| Acumulador::novo(conta == AggregateFn::Mode, distinta)).collect();
                    for (i, &d) in destino.iter().enumerate() {
                        let v = s.values.get(i).copied().unwrap_or(0.0);
                        if distinta {
                            // Aqui só há números (NaN = célula sem número).
                            acc[d].anotar_distinto((!v.is_nan()).then(|| chave_do_numero(v)));
                        }
                        acc[d].acumular(v);
                    }
                    ChartSeries {
                        name: nome_com_conta(config, ordem, s.name),
                        values: acc.iter().map(|a| a.resultado(conta)).collect(),
                    }
                })
                .collect();
            return ChartData { title: titulo, categories: unicas, series: agregadas, x_values, z_values, amostrado, aviso: None };
        }
    }

    ChartData { title: titulo, categories, series, x_values, z_values, amostrado, aviso: None }
}

/// `extractRawChartSeries`: todas as séries, inclusive as escondidas, com o
/// nome puro da coluna — é o que o editor mostra nos chips.
pub fn extract_raw_chart_series(sheet: &Sheet, config: &ChartConfig) -> Vec<(u32, String)> {
    extract_raw_chart_series_with(&|r, c| sheet.get_value(r, c), config)
}

pub fn extract_raw_chart_series_with(
    celula: &dyn Fn(u32, u32) -> Value,
    config: &ChartConfig,
) -> Vec<(u32, String)> {
    let mut cru = config.clone();
    cru.aggregate_by_series.clear();
    cru.style.hidden_series.clear();
    extract_chart_data_with(celula, &cru)
        .series
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let nome = if s.name.is_empty() { format!("Série {}", i + 1) } else { s.name };
            (i as u32, nome)
        })
        .collect()
}

// ----------------------------------------------------------- utilitários

/// `getStackMode`: o empilhamento dito no estilo, ou o que o tipo implica.
pub fn get_stack_mode(config: &ChartConfig) -> StackMode {
    if let Some(modo) = config.style.stack_mode {
        return modo;
    }
    match config.tipo {
        ChartType::ColumnStacked | ChartType::BarStacked => StackMode::Stacked,
        ChartType::ColumnStacked100 | ChartType::BarStacked100 => StackMode::Percent,
        _ => StackMode::None,
    }
}

/// As paletas (`PALETTES`).
pub fn palette(nome: PaletteName) -> [&'static str; 8] {
    match nome {
        PaletteName::Vibrant => ["#2563eb", "#10b981", "#f59e0b", "#8b5cf6", "#ec4899", "#14b8a6", "#f97316", "#06b6d4"],
        PaletteName::Ocean => ["#0284c7", "#06b6d4", "#0d9488", "#3b82f6", "#6366f1", "#0ea5e9", "#14b8a6", "#22d3ee"],
        PaletteName::Forest => ["#059669", "#16a34a", "#65a30d", "#10b981", "#84cc16", "#047857", "#15803d", "#4d7c0f"],
        PaletteName::Sunset => ["#dc2626", "#ea580c", "#d97706", "#ca8a04", "#e11d48", "#f97316", "#f59e0b", "#e11d48"],
        PaletteName::Monochrome => ["#334155", "#475569", "#64748b", "#94a3b8", "#cbd5e1", "#1e293b", "#0f172a", "#475569"],
        PaletteName::Classic => ["#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b", "#e377c2", "#7f7f7f"],
    }
}

/// `getSeriesColor`: a cor escolhida para a série, ou a da paleta em ciclo.
pub fn get_series_color(style: &ChartStyle, indice: u32) -> String {
    if let Some(cor) = style.series_colors.get(&indice).filter(|c| !c.is_empty()) {
        return cor.clone();
    }
    let cores = palette(style.palette.unwrap_or(PaletteName::Vibrant));
    cores[indice as usize % cores.len()].to_string()
}

/// Forma do marcador de uma série; prioridade à configuração específica,
/// depois à geral. Tradução de `getSeriesPointShape` no modelo TS.
///
/// Os nomes são mantidos literalmente (circle/square/triangle/diamond/star/x/none)
/// para não alterar documentos FSHZ já salvos; quem desenha escolhe o glifo.
pub fn get_series_point_shape(style: &ChartStyle, indice: u32) -> &str {
    style.series_point_shapes.get(&indice)
        .filter(|forma| !forma.is_empty())
        .map(String::as_str)
        .or(style.point_shape.as_deref())
        .unwrap_or("circle")
}

/// Tamanho escolhido para a série, ou o geral, ou 5. Assim como o TS,
/// uma configuração explícita de tamanho zero não usa o fallback.
pub fn get_series_point_size(style: &ChartStyle, indice: u32) -> f64 {
    style.series_point_sizes.get(&indice).copied()
        .or(style.point_size)
        .unwrap_or(5.0)
}

/// A reta de tendência `y = mx + b` e o R².
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tendencia {
    pub slope: f64,
    pub intercept: f64,
    pub r_squared: f64,
}

/// `computeLinearTrendline`: mínimos quadrados sobre os dois vetores.
pub fn compute_linear_trendline(x: &[f64], y: &[f64]) -> Option<Tendencia> {
    let n = x.len().min(y.len());
    if n < 2 {
        return None;
    }
    let nf = n as f64;
    let (mut sx, mut sy, mut sxy, mut sxx) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        sx += x[i];
        sy += y[i];
        sxy += x[i] * y[i];
        sxx += x[i] * x[i];
    }
    let denom = nf * sxx - sx * sx;
    if denom == 0.0 {
        return None;
    }
    let slope = (nf * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / nf;
    let media = sy / nf;
    let (mut sst, mut ssr) = (0.0, 0.0);
    for i in 0..n {
        let previsto = slope * x[i] + intercept;
        sst += (y[i] - media).powi(2);
        ssr += (y[i] - previsto).powi(2);
    }
    let r_squared = if sst == 0.0 { 1.0 } else { (1.0 - ssr / sst).max(0.0) };
    Some(Tendencia { slope, intercept, r_squared })
}

/// `computeMovingAverage`: a média das últimas `periodo` posições; as
/// primeiras `periodo - 1` ficam sem valor. Valor que falta conta como zero,
/// como o `?? 0` do TypeScript.
pub fn compute_moving_average(y: &[f64], periodo: usize) -> Vec<Option<f64>> {
    let periodo = periodo.max(1);
    (0..y.len())
        .map(|i| {
            if i + 1 < periodo {
                return None;
            }
            let soma: f64 = (0..periodo).map(|j| y[i - j]).sum();
            Some(soma / periodo as f64)
        })
        .collect()
}

// ------------------------------------------------------------------ testes

#[cfg(test)]
mod tests;
