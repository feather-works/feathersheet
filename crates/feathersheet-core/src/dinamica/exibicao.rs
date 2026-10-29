//! Como a tabela dinâmica exibe os números e os itens (pedido de 15/10/2026,
//! paridade com o Excel): "Mostrar valores como", agrupar campos em faixas
//! numéricas e partes de data, classificar itens e ligar/desligar os totais
//! gerais.
//!
//! Tudo aqui é configuração + conta pura. A varredura da origem e o desenho
//! continuam em [`crate::dinamica::dinamica_layout`], que recebe um
//! [`Extras`] em [`montar_com_extras`](crate::dinamica::dinamica_layout::montar_com_extras).
//! O `Extras::default()` é a dinâmica de sempre. Serializa
//! (`serde`: `serde_json::to_value(&extras)` / `from_value`, que o core não
//! puxa de propósito) para ir junto da dinâmica gravada (chave `extras` no
//! JSON da dinâmica, dentro do `.fshz`). JSON ausente ou `{}` = padrão.
//!
//! Semântica de cada "Mostrar valores como" (igual ao Excel):
//!
//! - **% do total geral**: valor / total geral da medida.
//! - **% do total da coluna**: valor / total da coluna (linha "Total Geral").
//! - **% do total da linha**: valor / total da linha (coluna "Total Geral").
//! - **% de**: valor / valor do item base (mesmo lugar, com o campo base
//!   trocado pelo item base). O item base mostra 100%.
//! - **Diferença de** / **% diferença de**: valor − base, e (valor − base) / base.
//!   O próprio item base (e o primeiro, com "anterior") fica em branco.
//! - **Total acumulado em**: soma dos itens do campo base até o atual, dentro
//!   do mesmo grupo pai (recomeça a cada item do campo de fora).
//! - **% do total acumulado**: o acumulado / soma de todos os itens do campo.
//! - **Classificar do menor/maior**: posição (1, 2, 3...) entre os itens do
//!   campo base; empates dividem a mesma posição.
//! - **Índice**: (valor × total geral) / (total da linha × total da coluna).
//!
//! Onde o campo base não existe na célula (total geral, subtotal de um campo
//! acima dele), as opções que dependem de um campo base saem em branco, como
//! no Excel. Divisão por zero dá `#DIV/0!`.

use serde::{Deserialize, Serialize};

use crate::celulas::values::{to_text, ErrorCode, Value};
use crate::dinamica::pivot::excel_serial_to_ymd;

/// O eixo de um campo: Linhas ou Colunas da dinâmica.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Eixo {
    Linhas,
    Colunas,
}

/// O campo base de um "% de", "diferença de", "total acumulado em"...: o
/// campo na posição `posicao` da lista de Linhas (ou de Colunas), contada de
/// fora para dentro, a partir de 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampoBase {
    pub eixo: Eixo,
    pub posicao: usize,
}

/// O item base de "% de" e "diferença de".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum ItemBase {
    /// Um item pelo rótulo (como aparece na dinâmica).
    Item { nome: String },
    /// "(anterior)": o item de cima/da esquerda.
    Anterior,
    /// "(seguinte)": o item de baixo/da direita.
    Seguinte,
}

/// "Mostrar valores como" de um campo de valor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum MostrarComo {
    #[default]
    Normal,
    PercTotalGeral,
    PercTotalColuna,
    PercTotalLinha,
    PercDe { campo: CampoBase, item: ItemBase },
    DiferencaDe { campo: CampoBase, item: ItemBase },
    PercDiferencaDe { campo: CampoBase, item: ItemBase },
    TotalAcumuladoEm { campo: CampoBase },
    PercTotalAcumuladoEm { campo: CampoBase },
    ClassificarMenorMaior { campo: CampoBase },
    ClassificarMaiorMenor { campo: CampoBase },
    Indice,
}

impl MostrarComo {
    /// O formato de número que o Excel aplica sozinho a este modo: `Some("0,00%")`
    /// para os percentuais, `Some("0")` para a classificação, `Some("0,00")`
    /// para o índice, `None` para manter o formato da medida (valores normais,
    /// diferenças e acumulados).
    pub fn formato_sugerido(&self) -> Option<&'static str> {
        match self {
            MostrarComo::PercTotalGeral
            | MostrarComo::PercTotalColuna
            | MostrarComo::PercTotalLinha
            | MostrarComo::PercDe { .. }
            | MostrarComo::PercDiferencaDe { .. }
            | MostrarComo::PercTotalAcumuladoEm { .. } => Some("0,00%"),
            MostrarComo::ClassificarMenorMaior { .. } | MostrarComo::ClassificarMaiorMenor { .. } => Some("0"),
            MostrarComo::Indice => Some("0,00"),
            _ => None,
        }
    }

    /// O rótulo do menu do Excel (para a GUI listar as opções).
    pub fn rotulo(&self) -> &'static str {
        match self {
            MostrarComo::Normal => "Sem cálculo",
            MostrarComo::PercTotalGeral => "% do total geral",
            MostrarComo::PercTotalColuna => "% do total da coluna",
            MostrarComo::PercTotalLinha => "% do total da linha",
            MostrarComo::PercDe { .. } => "% de",
            MostrarComo::DiferencaDe { .. } => "Diferença de",
            MostrarComo::PercDiferencaDe { .. } => "% de diferença de",
            MostrarComo::TotalAcumuladoEm { .. } => "Total acumulado em",
            MostrarComo::PercTotalAcumuladoEm { .. } => "% do total acumulado em",
            MostrarComo::ClassificarMenorMaior { .. } => "Classificar do menor para o maior",
            MostrarComo::ClassificarMaiorMenor { .. } => "Classificar do maior para o menor",
            MostrarComo::Indice => "Índice",
        }
    }
}

/// A parte da data usada para agrupar um campo de data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParteData {
    /// `2026`
    Ano,
    /// `T1`..`T4`
    Trimestre,
    /// `jan`..`dez`
    Mes,
    /// `1`..`31`
    Dia,
}

/// Agrupar um campo numérico em faixas: de `inicio` até `fim`, de
/// `intervalo` em `intervalo`. Fora do alcance cai em `<inicio` e `>fim`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Faixas {
    pub inicio: f64,
    pub fim: f64,
    pub intervalo: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Agrupamento {
    Faixas(Faixas),
    Data { parte: ParteData },
}

/// Como os itens de um campo se ordenam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Ordenacao {
    /// A ordem de sempre: pelo texto (ou pela ordem natural do agrupamento).
    #[default]
    Padrao,
    /// A→Z (números antes do texto, sem diferenciar maiúsculas).
    Crescente,
    /// Z→A.
    Decrescente,
    /// Pelo total do item numa medida (índice em `valores`).
    PorValor { medida: usize, crescente: bool },
}

/// Agrupamento e ordem de um campo de Linhas ou de Colunas (na mesma
/// posição da lista `linhas`/`colunas` do `DinamicaConfig`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CampoExtra {
    #[serde(default)]
    pub agrupamento: Option<Agrupamento>,
    #[serde(default)]
    pub ordenacao: Ordenacao,
}

fn verdadeiro() -> bool {
    true
}

/// Tudo que a dinâmica ganhou além de `DinamicaConfig` + `Opcoes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Extras {
    /// "Mostrar valores como" de cada medida (mesma posição de `valores`;
    /// o que faltar é `Normal`).
    #[serde(default)]
    pub valores: Vec<MostrarComo>,
    /// Por campo de Linhas (mesma posição de `linhas`).
    #[serde(default)]
    pub linhas: Vec<CampoExtra>,
    /// Por campo de Colunas (mesma posição de `colunas`).
    #[serde(default)]
    pub colunas: Vec<CampoExtra>,
    /// Totais gerais "das linhas" (`rowGrandTotals` do Excel): a coluna
    /// "Total Geral" à direita, com o total de cada linha. Só existe com
    /// campo em Colunas.
    #[serde(default = "verdadeiro")]
    pub totais_gerais_linhas: bool,
    /// Totais gerais "das colunas" (`colGrandTotals`): a linha "Total Geral"
    /// embaixo, com o total de cada coluna.
    #[serde(default = "verdadeiro")]
    pub totais_gerais_colunas: bool,
}

impl Default for Extras {
    fn default() -> Self {
        Extras {
            valores: Vec::new(),
            linhas: Vec::new(),
            colunas: Vec::new(),
            totais_gerais_linhas: true,
            totais_gerais_colunas: true,
        }
    }
}

impl Extras {
    /// Nada mudou em relação à dinâmica de sempre.
    pub fn eh_padrao(&self) -> bool {
        self.totais_gerais_linhas
            && self.totais_gerais_colunas
            && self.valores.iter().all(|v| *v == MostrarComo::Normal)
            && self.linhas.iter().chain(self.colunas.iter()).all(|c| *c == CampoExtra::default())
    }

    pub fn mostrar_como(&self, medida: usize) -> &MostrarComo {
        static NORMAL: MostrarComo = MostrarComo::Normal;
        self.valores.get(medida).unwrap_or(&NORMAL)
    }

    pub fn campo(&self, eixo: Eixo, posicao: usize) -> CampoExtra {
        let lista = if eixo == Eixo::Linhas { &self.linhas } else { &self.colunas };
        lista.get(posicao).copied().unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Agrupar
// ---------------------------------------------------------------------------

const MESES: [&str; 12] = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];

fn numero_curto(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{}", x as i64)
    } else {
        format!("{x}").replace('.', ",")
    }
}

/// O rótulo e a ordem do item de `valor` no agrupamento. A ordem é o que
/// põe `10-19` depois de `0-9`; vazio e inválido vão para o fim.
pub fn chave_agrupada(valor: &Value, agrupamento: &Agrupamento) -> (String, f64) {
    let vazio = || ("(Vazio)".to_string(), f64::MAX);
    match agrupamento {
        Agrupamento::Faixas(f) => {
            let x = match valor {
                Value::Number(n) => *n,
                Value::Text(t) => match t.trim().replace(',', ".").parse::<f64>() {
                    Ok(n) => n,
                    Err(_) => return vazio(),
                },
                Value::Bool(b) => *b as u8 as f64,
                _ => return vazio(),
            };
            if !x.is_finite() || !f.intervalo.is_finite() || f.intervalo <= 0.0 {
                return vazio();
            }
            if x < f.inicio {
                return (format!("<{}", numero_curto(f.inicio)), f64::MIN);
            }
            if x > f.fim {
                return (format!(">{}", numero_curto(f.fim)), f64::MAX / 2.0);
            }
            // O último intervalo inclui o fim; o valor igual ao fim cai nele.
            let n_faixas = (((f.fim - f.inicio) / f.intervalo).ceil() as i64).max(1);
            let k = (((x - f.inicio) / f.intervalo).floor() as i64).min(n_faixas - 1);
            let baixo = f.inicio + k as f64 * f.intervalo;
            let ultima = k == n_faixas - 1;
            let inteiros = f.inicio.fract() == 0.0 && f.intervalo.fract() == 0.0;
            let alto = if ultima {
                f.fim
            } else if inteiros {
                baixo + f.intervalo - 1.0
            } else {
                baixo + f.intervalo
            };
            (format!("{}-{}", numero_curto(baixo), numero_curto(alto)), baixo)
        }
        Agrupamento::Data { parte } => match ymd_de(valor) {
            None => vazio(),
            Some((a, m, d)) => match parte {
                ParteData::Ano => (a.to_string(), a as f64),
                ParteData::Trimestre => {
                    let q = m.div_ceil(3);
                    (format!("T{q}"), q as f64)
                }
                ParteData::Mes => (MESES[(m - 1) as usize].to_string(), m as f64),
                ParteData::Dia => (d.to_string(), d as f64),
            },
        },
    }
}

/// Ano, mês e dia de uma célula de data: serial do Excel (número ou texto
/// numérico), `AAAA-MM-DD` (com hora opcional) ou `DD/MM/AAAA`.
pub fn ymd_de(valor: &Value) -> Option<(i32, u32, u32)> {
    let serial = |n: f64| {
        (1.0..3_000_000.0).contains(&n).then(|| excel_serial_to_ymd(n.floor() as i64))
    };
    match valor {
        Value::Number(n) => serial(*n),
        Value::Text(t) => {
            let t = t.trim();
            let b = t.as_bytes();
            if b.len() >= 10 && b[4] == b'-' && b[7] == b'-' {
                let (a, m, d) = (t[0..4].parse::<i32>().ok()?, t[5..7].parse::<u32>().ok()?, t[8..10].parse::<u32>().ok()?);
                return ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((a, m, d));
            }
            if b.len() >= 10 && b[2] == b'/' && b[5] == b'/' {
                let (d, m, a) = (t[0..2].parse::<u32>().ok()?, t[3..5].parse::<u32>().ok()?, t[6..10].parse::<i32>().ok()?);
                return ((1..=12).contains(&m) && (1..=31).contains(&d)).then_some((a, m, d));
            }
            serial(t.parse::<f64>().ok()?)
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Ordenar
// ---------------------------------------------------------------------------

/// A→Z natural: números antes de texto, números pelo valor, texto sem
/// diferenciar maiúsculas e com os trechos de algarismos pelo valor
/// ("6/9/2026" antes de "6/10/2026", "350 - 400" antes de "1000 - 1200").
pub fn comparar_natural(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let n = |s: &str| s.trim().replace(',', ".").parse::<f64>().ok().filter(|x| x.is_finite());
    match (n(a), n(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal).then_with(|| a.cmp(b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => texto_natural(&a.to_lowercase(), &b.to_lowercase()).then_with(|| a.cmp(b)),
    }
}

fn texto_natural(a: &str, b: &str) -> std::cmp::Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return std::cmp::Ordering::Equal,
            (None, _) => return std::cmp::Ordering::Less,
            (_, None) => return std::cmp::Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let fim = |s: &[u8]| s.iter().position(|c| !c.is_ascii_digit()).unwrap_or(s.len());
                let (na, nb) = (fim(a), fim(b));
                // Sem zeros à esquerda, o trecho mais longo é o maior número.
                let tira = |s: &[u8]| { let z = s.iter().take_while(|&&c| c == b'0').count(); s[z..].to_vec() };
                let (da, db) = (tira(&a[..na]), tira(&b[..nb]));
                let ord = da.len().cmp(&db.len()).then_with(|| da.cmp(&db));
                if ord.is_ne() { return ord; }
                (a, b) = (&a[na..], &b[nb..]);
            }
            (Some(x), Some(y)) => {
                if x != y { return x.cmp(y); }
                (a, b) = (&a[1..], &b[1..]);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Mostrar valores como
// ---------------------------------------------------------------------------

/// O que a conta precisa saber da dinâmica. Coordenadas: o caminho da linha
/// (níveis unidos por `SEPARADOR`; `""` = todas as linhas) e a chave da coluna
/// (idem; `""` = todas as colunas).
pub struct Grade<'a> {
    /// O valor cru (agregado) da medida no cruzamento.
    pub cru: &'a dyn Fn(&str, &str, usize) -> f64,
    /// Os filhos, já ordenados, do nó com esse prefixo no eixo.
    pub irmaos: &'a dyn Fn(Eixo, &[String]) -> Vec<String>,
}

fn partes(coordenada: &str) -> Vec<String> {
    if coordenada.is_empty() {
        Vec::new()
    } else {
        coordenada.split(crate::dinamica::dinamica_layout::SEPARADOR).map(str::to_string).collect()
    }
}

fn unir(p: &[String]) -> String {
    p.join(&crate::dinamica::dinamica_layout::SEPARADOR.to_string())
}

fn branco() -> Value {
    Value::Text(String::new())
}

fn dividir(a: f64, b: f64) -> Value {
    if b == 0.0 || !b.is_finite() {
        Value::erro(ErrorCode::Div0)
    } else {
        Value::Number(a / b)
    }
}

/// Onde a célula (`caminho`, `chave`) está entre os irmãos do campo base.
struct Posicao {
    irmaos: Vec<String>,
    atual: usize,
    eixo: Eixo,
    caminho: Vec<String>,
    chave: Vec<String>,
    posicao: usize,
}

impl Posicao {
    fn nova(g: &Grade, campo: &CampoBase, caminho: &str, chave: &str) -> Option<Posicao> {
        let (cam, cha) = (partes(caminho), partes(chave));
        let alvo = if campo.eixo == Eixo::Linhas { &cam } else { &cha };
        if alvo.len() <= campo.posicao {
            return None;
        }
        let irmaos = (g.irmaos)(campo.eixo, &alvo[..campo.posicao]);
        let atual = irmaos.iter().position(|n| *n == alvo[campo.posicao])?;
        Some(Posicao { irmaos, atual, eixo: campo.eixo, caminho: cam, chave: cha, posicao: campo.posicao })
    }

    /// O valor cru da mesma célula com o item do campo base trocado.
    fn valor_com(&self, g: &Grade, item: &str, medida: usize) -> f64 {
        let (mut cam, mut cha) = (self.caminho.clone(), self.chave.clone());
        if self.eixo == Eixo::Linhas {
            cam[self.posicao] = item.to_string();
        } else {
            cha[self.posicao] = item.to_string();
        }
        (g.cru)(&unir(&cam), &unir(&cha), medida)
    }

    fn indice_base(&self, item: &ItemBase) -> Option<usize> {
        match item {
            ItemBase::Item { nome } => self.irmaos.iter().position(|n| n == nome),
            ItemBase::Anterior => self.atual.checked_sub(1),
            ItemBase::Seguinte => (self.atual + 1 < self.irmaos.len()).then_some(self.atual + 1),
        }
    }
}

/// O número que a célula mostra no modo `modo`.
pub fn mostrar_como(modo: &MostrarComo, medida: usize, caminho: &str, chave: &str, g: &Grade) -> Value {
    let v = (g.cru)(caminho, chave, medida);
    match modo {
        MostrarComo::Normal => Value::Number(v),
        MostrarComo::PercTotalGeral => dividir(v, (g.cru)("", "", medida)),
        MostrarComo::PercTotalColuna => dividir(v, (g.cru)("", chave, medida)),
        MostrarComo::PercTotalLinha => dividir(v, (g.cru)(caminho, "", medida)),
        MostrarComo::Indice => {
            let denominador = (g.cru)(caminho, "", medida) * (g.cru)("", chave, medida);
            dividir(v * (g.cru)("", "", medida), denominador)
        }
        MostrarComo::PercDe { campo, item }
        | MostrarComo::DiferencaDe { campo, item }
        | MostrarComo::PercDiferencaDe { campo, item } => {
            let Some(p) = Posicao::nova(g, campo, caminho, chave) else { return branco() };
            let Some(i) = p.indice_base(item) else { return branco() };
            let diferenca = !matches!(modo, MostrarComo::PercDe { .. });
            if diferenca && i == p.atual {
                return branco(); // o item base de "diferença de" fica vazio
            }
            let base = p.valor_com(g, &p.irmaos[i].clone(), medida);
            match modo {
                MostrarComo::PercDe { .. } => dividir(v, base),
                MostrarComo::DiferencaDe { .. } => Value::Number(v - base),
                _ => dividir(v - base, base),
            }
        }
        MostrarComo::TotalAcumuladoEm { campo } | MostrarComo::PercTotalAcumuladoEm { campo } => {
            let Some(p) = Posicao::nova(g, campo, caminho, chave) else { return branco() };
            let valores: Vec<f64> = p.irmaos.iter().map(|n| p.valor_com(g, n, medida)).collect();
            let acumulado: f64 = valores[..=p.atual].iter().sum();
            if matches!(modo, MostrarComo::TotalAcumuladoEm { .. }) {
                Value::Number(acumulado)
            } else {
                dividir(acumulado, valores.iter().sum())
            }
        }
        MostrarComo::ClassificarMenorMaior { campo } | MostrarComo::ClassificarMaiorMenor { campo } => {
            let Some(p) = Posicao::nova(g, campo, caminho, chave) else { return branco() };
            let crescente = matches!(modo, MostrarComo::ClassificarMenorMaior { .. });
            let antes = p
                .irmaos
                .iter()
                .filter(|n| {
                    let x = p.valor_com(g, n, medida);
                    if crescente { x < v } else { x > v }
                })
                .count();
            Value::Number((antes + 1) as f64)
        }
    }
}

/// Texto do item para mensagens (usado nos testes).
#[allow(dead_code)]
pub(crate) fn texto(v: &Value) -> String {
    to_text(v)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn faixas(x: f64, ini: f64, fim: f64, int: f64) -> (String, f64) {
        chave_agrupada(&Value::Number(x), &Agrupamento::Faixas(Faixas { inicio: ini, fim, intervalo: int }))
    }

    #[test]
    fn faixas_inteiras_como_no_excel() {
        assert_eq!(faixas(0.0, 0.0, 100.0, 10.0).0, "0-9");
        assert_eq!(faixas(9.0, 0.0, 100.0, 10.0).0, "0-9");
        assert_eq!(faixas(10.0, 0.0, 100.0, 10.0).0, "10-19");
        assert_eq!(faixas(95.0, 0.0, 100.0, 10.0).0, "90-100");
        assert_eq!(faixas(100.0, 0.0, 100.0, 10.0).0, "90-100", "o fim entra na última faixa");
        assert_eq!(faixas(-1.0, 0.0, 100.0, 10.0).0, "<0");
        assert_eq!(faixas(101.0, 0.0, 100.0, 10.0).0, ">100");
        assert_eq!(faixas(0.7, 0.0, 2.0, 0.5).0, "0,5-1");
        assert!(faixas(5.0, 0.0, 100.0, 10.0).1 < faixas(15.0, 0.0, 100.0, 10.0).1);
    }

    #[test]
    fn partes_de_data() {
        let d = |t: &str, p| chave_agrupada(&Value::Text(t.into()), &Agrupamento::Data { parte: p }).0;
        assert_eq!(d("2026-02-10", ParteData::Ano), "2026");
        assert_eq!(d("2026-02-10", ParteData::Trimestre), "T1");
        assert_eq!(d("2026-08-10", ParteData::Trimestre), "T3");
        assert_eq!(d("2026-02-10", ParteData::Mes), "fev");
        assert_eq!(d("10/02/2026", ParteData::Dia), "10");
        // 44562 = 2022-01-01
        let n = chave_agrupada(&Value::Number(44562.0), &Agrupamento::Data { parte: ParteData::Mes });
        assert_eq!(n, ("jan".to_string(), 1.0));
        assert_eq!(d("abc", ParteData::Ano), "(Vazio)");
    }

    #[test]
    fn natural_poe_numero_antes_de_texto() {
        let mut v = vec!["b", "10", "A", "2"];
        v.sort_by(|a, b| comparar_natural(a, b));
        assert_eq!(v, ["2", "10", "A", "b"]);
        let mut v = vec!["6/10/2026", "1000 - 1200", "6/9/2026", "350 - 400", "LINHA10", "linha2"];
        v.sort_by(|a, b| comparar_natural(a, b));
        assert_eq!(v, ["6/9/2026", "6/10/2026", "350 - 400", "1000 - 1200", "linha2", "LINHA10"]);
    }

    #[test]
    fn json_vai_e_volta_e_ausente_vira_padrao() {
        let e = Extras {
            valores: vec![MostrarComo::PercDe {
                campo: CampoBase { eixo: Eixo::Linhas, posicao: 0 },
                item: ItemBase::Item { nome: "Sul".into() },
            }],
            linhas: vec![CampoExtra {
                agrupamento: Some(Agrupamento::Faixas(Faixas { inicio: 0.0, fim: 100.0, intervalo: 10.0 })),
                ordenacao: Ordenacao::PorValor { medida: 0, crescente: false },
            }],
            totais_gerais_colunas: false,
            ..Extras::default()
        };
        let de = |v: serde_json::Value| serde_json::from_value::<Extras>(v).unwrap_or_default();
        assert_eq!(de(serde_json::to_value(&e).unwrap()), e);
        assert!(de(serde_json::Value::Null).eh_padrao());
        assert!(de(serde_json::json!({})).eh_padrao());
        assert!(!e.eh_padrao());
    }
}
