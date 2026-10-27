//! O filtro da tabela: que linhas ficam escondidas.
//!
//! Tradução do `reavaliarFiltrosTabela` e do `evaluateCustomPredicate` de
//! `ui/app/app.ts`. Mora aqui porque, na pasta do Rust, avaliar o filtro no
//! TypeScript pedia o valor de cada linha pela ponte — dois milhões de linhas
//! travavam a planilha sem erro nenhum (bug de 23/09/2026). O que volta são
//! só as **faixas** de linhas escondidas.
//!
//! Os critérios são os quatro que a interface grava:
//!
//! - **lista** dos valores marcados (a lista de caixinhas): a linha fica se o
//!   valor está na lista — igual, ou com o mesmo texto (`String(v)` do
//!   JavaScript, que é o que o TypeScript comparava);
//! - **fora da lista** (`not_in`): o contrário, para quando a lista de
//!   caixinhas veio cortada (coluna com valores distintos demais) — desmarcar
//!   esconde aqueles, e o resto, que nem apareceu, continua à vista;
//! - **regra** (`op` + `val`): começa com, contém, maior que…;
//! - **clássico**: o critério do `CONT.SE` (`">10"`, `"a*"`);
//! - **cor** (`Cor`): a cor de preenchimento ou da fonte da célula;
//! - **10 primeiros** (`Top10`): os N maiores/menores, em itens ou em %;
//! - **dinâmico** (`Dinamico`): período de data (hoje, este mês…) e acima/
//!   abaixo da média, como o `<dynamicFilter>` do `.xlsx`.
//!
//! Os três últimos precisam de mais que o valor (a cor da célula, a coluna
//! inteira, o dia de hoje): usam [`linhas_escondidas_com`] e o [`Contexto`].

use crate::funcoes::fn_helpers::make_predicate;
use crate::celulas::number::{from_js_string, to_js_string};
use crate::celulas::values::Value;
use crate::operacoes::filtro_data::{no_periodo, Dinamico};
use std::collections::HashSet;

/// Qual cor o filtro/ordenação olha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlvoCor {
    Fundo,
    Fonte,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Criterio {
    Lista(Vec<Value>),
    ForaDaLista(Vec<Value>),
    Regra { op: String, val: Value },
    Classico(Value),
    /// Células com esta cor (`#rrggbb`); `None` = "Sem preenchimento" /
    /// "Automático".
    Cor { alvo: AlvoCor, cor: Option<String> },
    /// "10 primeiros": `topo` = maiores (senão, menores); `n` itens ou, com
    /// `percentual`, n% dos itens numéricos da coluna (arredondado para
    /// baixo, no mínimo 1). Empates no corte ficam todos, como no Excel.
    Top10 { topo: bool, n: u32, percentual: bool },
    Dinamico(Dinamico),
}

/// Cor em forma comparável: minúscula, sem `#`, `rgb` expandido, sem alfa.
pub fn normalizar_cor(c: &str) -> String {
    let h = c.trim().trim_start_matches('#').to_lowercase();
    match h.len() {
        3 => h.chars().flat_map(|c| [c, c]).collect(),
        8 => h[2..].to_string(),
        _ => h,
    }
}

/// Se duas cores opcionais são a mesma (`None` só é igual a `None`).
pub fn mesma_cor(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => normalizar_cor(a) == normalizar_cor(b),
        _ => false,
    }
}

/// Cores de uma célula: (preenchimento, fonte).
pub type CoresDaCelula = (Option<String>, Option<String>);

/// O que o filtro precisa saber além do valor.
pub struct Contexto<'a> {
    pub valor: &'a dyn Fn(u32, u32) -> Value,
    /// Cores da célula; `None` quando o chamador não as dá (toda célula conta
    /// como sem cor).
    pub cores: Option<&'a dyn Fn(u32, u32) -> CoresDaCelula>,
    /// Serial inteiro do dia de hoje (nunca o relógio: quem chama decide o fuso).
    pub hoje: i64,
}

/// `String(v)` do JavaScript — é o texto que o filtro compara.
pub fn texto_js(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => to_js_string(*n),
        Value::Bool(b) => b.to_string(),
        Value::Text(t) => t.clone(),
        Value::Error(e) => e.code.as_str().to_string(),
    }
}

/// `Number(String(v).replace(',', '.'))` do JavaScript.
fn numero_js(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        outro => from_js_string(&texto_js(outro).replacen(',', ".", 1)).unwrap_or(f64::NAN),
    }
}

// A regra das caixas combina por String(valor) no TypeScript. Inclusive
// quando os tipos diferem (numero 10 vs texto "10"), a conversao JS
// e suficiente. Usar lista.contains() antes do HashSet era O(N) por linha.
fn na_lista(textos: &HashSet<String>, v: &Value) -> bool {
    // Células de texto são a maioria: consultar &str evita alocar uma
    // String temporária para CADA linha (inclusive em milhões de linhas).
    match v {
        Value::Text(s) => textos.contains(s.as_str()),
        Value::Empty => textos.contains(""),
        Value::Bool(true) => textos.contains("true"),
        Value::Bool(false) => textos.contains("false"),
        Value::Error(e) => textos.contains(e.code.as_str()),
        Value::Number(n) => textos.contains(to_js_string(*n).as_str()),
    }
}

/// `evaluateCustomPredicate`, regra por regra.
fn regra(op: &str, alvo: &Value, v: &Value) -> bool {
    let e_verdade = |v: &Value| match v {
        Value::Bool(b) => *b,
        Value::Number(n) => *n == 1.0,
        outro => matches!(texto_js(outro).to_lowercase().as_str(), "true" | "verdadeiro"),
    };
    let e_falso = |v: &Value| match v {
        Value::Bool(b) => !*b,
        Value::Number(n) => *n == 0.0,
        outro => matches!(texto_js(outro).to_lowercase().as_str(), "false" | "falso"),
    };
    match op {
        "is_true" => return e_verdade(v),
        "is_false" => return e_falso(v),
        "is_blank" => return matches!(v,Value::Empty)
            || matches!(v,Value::Text(s) if s.is_empty()),
        "is_not_blank" => return !matches!(v,Value::Empty)
            && !matches!(v,Value::Text(s) if s.is_empty()),
        _ => {}
    }
    if matches!(v, Value::Empty) {
        return false;
    }
    let (sv, st) = (texto_js(v).to_lowercase(), texto_js(alvo).to_lowercase());
    let (nv, nt) = (numero_js(v), numero_js(alvo));
    let numeros = !nv.is_nan() && !nt.is_nan();
    match op {
        "starts_with" => sv.starts_with(&st),
        "ends_with" => sv.ends_with(&st),
        "contains" => sv.contains(&st),
        "not_contains" => !sv.contains(&st),
        "not_starts_with" => !sv.starts_with(&st),
        "not_ends_with" => !sv.ends_with(&st),
        "equals_text" => sv == st,
        "not_equals_text" => sv != st,
        "gt" | ">" => numeros && nv > nt,
        "lt" | "<" => numeros && nv < nt,
        "gte" | ">=" => numeros && nv >= nt,
        "lte" | "<=" => numeros && nv <= nt,
        "eq_num" | "=" => numeros && nv == nt,
        "neq_num" | "<>" => numeros && nv != nt,
        _ => true,
    }
}

/// As linhas de `r0..=r1` que o filtro esconde, em faixas `(início, fim)`.
/// `filtros` são (coluna absoluta, critério); a linha fica se passa em todos.
/// Só vale para os critérios que dependem do valor; para cor, 10 primeiros e
/// datas dinâmicas use [`linhas_escondidas_com`].
pub fn linhas_escondidas(r0: u32, r1: u32, filtros: &[(u32, Criterio)], valor: &dyn Fn(u32, u32) -> Value) -> Vec<(u32, u32)> {
    linhas_escondidas_com(r0, r1, filtros, &Contexto { valor, cores: None, hoje: 0 })
}

/// Os números da coluna `col` em `r0..=r1` (data também é número).
fn numeros_da_coluna(r0: u32, r1: u32, col: u32, valor: &dyn Fn(u32, u32) -> Value) -> Vec<f64> {
    (r0..=r1)
        .filter_map(|r| match valor(r, col) {
            Value::Number(n) if n.is_finite() => Some(n),
            _ => None,
        })
        .collect()
}

/// [`linhas_escondidas`] com cor da célula, 10 primeiros e períodos de data.
/// Os critérios que olham a coluna toda (10 primeiros, média) a leem uma vez,
/// antes de testar as linhas, ignorando os outros filtros (como o Excel).
pub fn linhas_escondidas_com(r0: u32, r1: u32, filtros: &[(u32, Criterio)], ctx: &Contexto) -> Vec<(u32, u32)> {
    type Teste<'a> = Box<dyn Fn(u32, &Value) -> bool + 'a>;
    let hoje = ctx.hoje;
    let testes: Vec<(u32, Teste)> = filtros
        .iter()
        .map(|(col, c)| {
            let col = *col;
            let t: Teste = match c.clone() {
                Criterio::Lista(lista) => {
                    let textos: HashSet<String> = lista.iter().map(texto_js).collect();
                    Box::new(move |_, v| na_lista(&textos, v))
                }
                Criterio::ForaDaLista(lista) => {
                    let textos: HashSet<String> = lista.iter().map(texto_js).collect();
                    Box::new(move |_, v| !na_lista(&textos, v))
                }
                Criterio::Regra { op, val } => Box::new(move |_, v| regra(&op, &val, v)),
                Criterio::Classico(v) => {
                    let p = make_predicate(&v);
                    Box::new(move |_, x| p(x))
                }
                Criterio::Cor { alvo, cor } => {
                    let cores = ctx.cores;
                    Box::new(move |r, _| {
                        let (fundo, fonte) = cores.map_or((None, None), |f| f(r, col));
                        let da_celula = if alvo == AlvoCor::Fundo { fundo } else { fonte };
                        mesma_cor(da_celula.as_deref(), cor.as_deref())
                    })
                }
                Criterio::Top10 { topo, n, percentual } => {
                    let mut nums = numeros_da_coluna(r0, r1, col, ctx.valor);
                    nums.sort_by(|a, b| if topo { b.total_cmp(a) } else { a.total_cmp(b) });
                    let quantos = if percentual { ((nums.len() as u64 * n as u64) / 100).max(1) } else { n as u64 } as usize;
                    // Sem número ou n = 0: ninguém passa.
                    let corte = if quantos == 0 || nums.is_empty() { None } else { Some(nums[quantos.min(nums.len()) - 1]) };
                    Box::new(move |_, v| match (v, corte) {
                        (Value::Number(x), Some(c)) => x.is_finite() && if topo { *x >= c } else { *x <= c },
                        _ => false,
                    })
                }
                Criterio::Dinamico(d) if d.e_media() => {
                    let nums = numeros_da_coluna(r0, r1, col, ctx.valor);
                    let media = (!nums.is_empty()).then(|| nums.iter().sum::<f64>() / nums.len() as f64);
                    let acima = d == Dinamico::AcimaDaMedia;
                    Box::new(move |_, v| match (v, media) {
                        (Value::Number(x), Some(m)) => if acima { *x > m } else { *x < m },
                        _ => false,
                    })
                }
                Criterio::Dinamico(d) => Box::new(move |_, v| match v {
                    Value::Number(x) => no_periodo(d, *x, hoje),
                    _ => false,
                }),
            };
            (col, t)
        })
        .collect();
    let mut faixas: Vec<(u32, u32)> = Vec::new();
    for r in r0..=r1 {
        let fica = testes.iter().all(|(c, t)| t(r, &(ctx.valor)(r, *c)));
        if fica {
            continue;
        }
        match faixas.last_mut() {
            Some(u) if u.1 + 1 == r => u.1 = r,
            _ => faixas.push((r, r)),
        }
    }
    faixas
}

/// As cores que existem na coluna `col` em `r0..=r1`, na ordem em que
/// aparecem, para a lista "Filtrar por cor": (fundos, fontes). `None` entra
/// no máximo uma vez e significa "sem preenchimento"/"automático".
pub fn cores_da_coluna(r0: u32, r1: u32, col: u32, cores: &dyn Fn(u32, u32) -> CoresDaCelula) -> (Vec<Option<String>>, Vec<Option<String>>) {
    let mut fundos: Vec<Option<String>> = Vec::new();
    let mut fontes: Vec<Option<String>> = Vec::new();
    let junta = |lista: &mut Vec<Option<String>>, c: Option<String>| {
        if !lista.iter().any(|x| mesma_cor(x.as_deref(), c.as_deref())) {
            lista.push(c.map(|c| format!("#{}", normalizar_cor(&c))));
        }
    };
    for r in r0..=r1 {
        let (fundo, fonte) = cores(r, col);
        junta(&mut fundos, fundo);
        junta(&mut fontes, fonte);
    }
    (fundos, fontes)
}

/// [`linhas_escondidas_com`] lendo valor e cores direto da aba. `hoje` é o
/// serial do dia (ver [`crate::operacoes::filtro_data::serial_de_data`]).
pub fn linhas_escondidas_da_aba(sheet: &crate::planilha::sheet::Sheet, r0: u32, r1: u32, filtros: &[(u32, Criterio)], hoje: i64) -> Vec<(u32, u32)> {
    let valor = |r, c| sheet.get_value(r, c);
    let cores = |r, c| cores_da_celula(sheet, r, c);
    linhas_escondidas_com(r0, r1, filtros, &Contexto { valor: &valor, cores: Some(&cores), hoje })
}

/// Preenchimento e cor da fonte efetivos da célula (aba, coluna, linha e
/// célula, uma sobre a outra).
pub fn cores_da_celula(sheet: &crate::planilha::sheet::Sheet, r: u32, c: u32) -> CoresDaCelula {
    sheet.estilo_resolvido(r, c).map_or((None, None), |s| (s.bg, s.color))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn coluna() -> Vec<Value> {
        vec![
            Value::Text("ALTO".into()),
            Value::Text("BAIXO".into()),
            Value::Number(10.0),
            Value::Empty,
            Value::Text("alto demais".into()),
            Value::Number(2.5),
            Value::Bool(true),
        ]
    }

    fn escondidas(c: Criterio) -> Vec<(u32, u32)> {
        let dados = coluna();
        linhas_escondidas(0, dados.len() as u32 - 1, &[(0, c)], &|r, _| dados[r as usize].clone())
    }

    #[test]
    fn lista_de_caixinhas_e_fora_da_lista() {
        // Fica ALTO e o 10 (que a lista traz como texto, como o JavaScript).
        let lista = vec![Value::Text("ALTO".into()), Value::Text("10".into())];
        assert_eq!(escondidas(Criterio::Lista(lista.clone())), vec![(1, 1), (3, 6)]);
        assert_eq!(escondidas(Criterio::ForaDaLista(lista)), vec![(0, 0), (2, 2)]);
        assert_eq!(escondidas(Criterio::Lista(vec![Value::Empty])), vec![(0, 2), (4, 6)], "(Vazias)");
    }

    #[test]
    fn lista_numerica_logica_e_vazia_mantem_semantica_js() {
        let valores=[
            Value::Empty,
            Value::Text(String::new()),
            Value::Number(10.0),
            Value::Text("10".into()),
            Value::Bool(true),
            Value::Text("true".into()),
            Value::Number(11.0),
        ];
        let lista=vec![
            Value::Empty,
            Value::Text("10".into()),
            Value::Bool(true),
        ];
        let rodar=|criterio| linhas_escondidas(0,6,&[(0,criterio)],
            &|r,_|valores[r as usize].clone());
        assert_eq!(rodar(Criterio::Lista(lista.clone())),vec![(6,6)]);
        assert_eq!(rodar(Criterio::ForaDaLista(lista)),vec![(0,5)]);
    }

    #[test]
    fn filtro_um_milhao_de_linhas_sem_materializar_planilha() {
        // Stress no motor: recebe valores sob demanda e so armazena faixas
        // ocultas. Nao cria um milhao de Cells/Strings simultaneamente.
        use std::cell::Cell;
        let visitas=Cell::new(0_usize);
        let faixas=linhas_escondidas(0,1_000_000,
            &[(0,Criterio::Lista(vec![Value::Text("OK".into())]))],
            &|_,_| {
                visitas.set(visitas.get()+1);
                Value::Text("OK".into())
            });
        assert!(faixas.is_empty());
        // O intervalo inclui ambos os extremos.
        assert_eq!(visitas.get(),1_000_001);
    }

    #[test]
    fn regras_de_texto_numero_e_logico() {
        let r = |op: &str, val: Value| escondidas(Criterio::Regra { op: op.into(), val });
        assert_eq!(r("starts_with", Value::Text("al".into())), vec![(1, 3), (5, 6)]);
        assert_eq!(r("gt", Value::Number(3.0)), vec![(0, 1), (3, 6)], "só o 10 passa");
        assert_eq!(r("lte", Value::Text("2,5".into())), vec![(0, 4), (6, 6)], "vírgula decimal no alvo; vazio nunca passa em regra");
        assert_eq!(r("is_true", Value::Bool(true)), vec![(0, 5)]);
    }

    #[test]
    fn valores_vazios_e_nao_vazios_no_motor_de_filtro() {
        let valores=[
            Value::Empty,Value::Text(String::new()),
            Value::Text(" ".into()),Value::Number(0.0),Value::Bool(false),
        ];
        let executar=|op:&str| linhas_escondidas(0,4,
            &[(0,Criterio::Regra{op:op.to_owned(),val:Value::Empty})],
            &|r,_|valores[r as usize].clone());
        assert_eq!(executar("is_blank"),vec![(2,4)]);
        assert_eq!(executar("is_not_blank"),vec![(0,1)]);
    }

    #[test]
    fn criterio_classico_do_cont_se() {
        // Como o CONT.SE do Excel: ">5" só olha número; texto, vazio e
        // lógico ficam de fora (o TypeScript deixava texto e lógico passar).
        assert_eq!(escondidas(Criterio::Classico(Value::Text(">5".into()))), vec![(0, 1), (3, 6)]);
    }

    // ---- cor, 10 primeiros, média e datas ------------------------------

    use crate::operacoes::filtro_data::{serial_de_data, Dinamico};

    fn num(v: &[f64]) -> Vec<Value> {
        v.iter().map(|n| Value::Number(*n)).collect()
    }

    fn rodar(dados: &[Value], c: Criterio, hoje: i64) -> Vec<(u32, u32)> {
        let cores = |_: u32, _: u32| (None, None);
        linhas_escondidas_com(
            0,
            dados.len() as u32 - 1,
            &[(0, c)],
            &Contexto { valor: &|r, _| dados[r as usize].clone(), cores: Some(&cores), hoje },
        )
    }

    #[test]
    fn filtro_por_cor_de_fundo_e_de_fonte() {
        let fundos = ["#FF0000", "", "#ff0000", "#00F", "", "#FF0000"];
        let fontes = ["", "#0000ff", "#0000FF", "", "", "#000000"];
        let cores = |r: u32, _: u32| {
            let f = |t: &str| (!t.is_empty()).then(|| t.to_string());
            (f(fundos[r as usize]), f(fontes[r as usize]))
        };
        let valor = |_: u32, _: u32| Value::Number(1.0);
        let ctx = Contexto { valor: &valor, cores: Some(&cores), hoje: 0 };
        let esc = |alvo, cor: Option<&str>| {
            linhas_escondidas_com(0, 5, &[(0, Criterio::Cor { alvo, cor: cor.map(String::from) })], &ctx)
        };
        // Maiúscula/minúscula e #rgb não importam.
        assert_eq!(esc(AlvoCor::Fundo, Some("#ff0000")), vec![(1, 1), (3, 4)]);
        assert_eq!(esc(AlvoCor::Fundo, Some("#0000ff")), vec![(0, 2), (4, 5)]);
        assert_eq!(esc(AlvoCor::Fundo, None), vec![(0, 0), (2, 3), (5, 5)], "Sem preenchimento");
        assert_eq!(esc(AlvoCor::Fonte, Some("#0000ff")), vec![(0, 0), (3, 5)]);
        assert_eq!(esc(AlvoCor::Fonte, None), vec![(1, 2), (5, 5)], "Automático");
        let (f, t) = cores_da_coluna(0, 5, 0, &cores);
        assert_eq!(f, vec![Some("#ff0000".to_string()), None, Some("#0000ff".to_string())]);
        assert_eq!(t, vec![None, Some("#0000ff".to_string()), Some("#000000".to_string())]);
    }

    #[test]
    fn dez_primeiros_itens_e_ultimos() {
        let d = num(&[5.0, 9.0, 1.0, 9.0, 7.0, 3.0]);
        // Top 2: o 9 repetido e o 7 não — empate no corte: 9 e 9 (2 itens).
        assert_eq!(rodar(&d, Criterio::Top10 { topo: true, n: 2, percentual: false }, 0), vec![(0, 0), (2, 2), (4, 5)]);
        // Top 3: 9, 9, 7.
        assert_eq!(rodar(&d, Criterio::Top10 { topo: true, n: 3, percentual: false }, 0), vec![(0, 0), (2, 2), (5, 5)]);
        // Últimos 2: 1 e 3.
        assert_eq!(rodar(&d, Criterio::Top10 { topo: false, n: 2, percentual: false }, 0), vec![(0, 1), (3, 4)]);
        // n maior que a coluna: tudo passa.
        assert!(rodar(&d, Criterio::Top10 { topo: true, n: 99, percentual: false }, 0).is_empty());
    }

    #[test]
    fn dez_primeiros_por_cento_arredonda_para_baixo_com_minimo_de_um() {
        let d = num(&(1..=20).map(f64::from).collect::<Vec<_>>());
        // 10% de 20 = 2 itens: 19 e 20.
        assert_eq!(rodar(&d, Criterio::Top10 { topo: true, n: 10, percentual: true }, 0), vec![(0, 17)]);
        // 1% de 20 = 0,2 -> mínimo 1.
        assert_eq!(rodar(&d, Criterio::Top10 { topo: true, n: 1, percentual: true }, 0), vec![(0, 18)]);
        // 25% dos últimos: 5 itens.
        assert_eq!(rodar(&d, Criterio::Top10 { topo: false, n: 25, percentual: true }, 0), vec![(5, 19)]);
    }

    #[test]
    fn dez_primeiros_ignora_texto_e_vazio() {
        let d = vec![Value::Text("x".into()), Value::Number(4.0), Value::Empty, Value::Number(8.0)];
        assert_eq!(rodar(&d, Criterio::Top10 { topo: true, n: 1, percentual: false }, 0), vec![(0, 2)]);
    }

    #[test]
    fn acima_e_abaixo_da_media() {
        // média = 6; o 6 não é acima nem abaixo.
        let d = num(&[2.0, 6.0, 10.0, 6.0, 8.0, 4.0]);
        assert_eq!(rodar(&d, Criterio::Dinamico(Dinamico::AcimaDaMedia), 0), vec![(0, 1), (3, 3), (5, 5)]);
        assert_eq!(rodar(&d, Criterio::Dinamico(Dinamico::AbaixoDaMedia), 0), vec![(1, 4)]);
    }

    #[test]
    fn filtro_de_data_usa_o_hoje_injetado() {
        let hoje = serial_de_data(2026, 10, 15);
        let d = vec![
            Value::Number(serial_de_data(2026, 10, 15) as f64),
            Value::Number(serial_de_data(2026, 10, 14) as f64),
            Value::Text("15/10/2026".into()),
            Value::Empty,
            Value::Number(serial_de_data(2026, 11, 1) as f64),
        ];
        assert_eq!(rodar(&d, Criterio::Dinamico(Dinamico::Hoje), hoje), vec![(1, 4)]);
        assert_eq!(rodar(&d, Criterio::Dinamico(Dinamico::EsteMes), hoje), vec![(2, 4)]);
        // Outro "hoje" muda o resultado: não é o relógio.
        let amanha = hoje + 1;
        assert_eq!(rodar(&d, Criterio::Dinamico(Dinamico::Ontem), amanha), vec![(1, 4)]);
        assert_eq!(rodar(&d, Criterio::Dinamico(Dinamico::Mes(11)), hoje), vec![(0, 3)]);
    }

    #[test]
    fn criterios_combinam_com_e() {
        let hoje = serial_de_data(2026, 10, 15);
        let datas = [serial_de_data(2026, 10, 1), serial_de_data(2026, 10, 2), serial_de_data(2026, 9, 1)];
        let valores = [10.0, 1.0, 10.0];
        let valor = |r: u32, c: u32| Value::Number(if c == 0 { datas[r as usize] as f64 } else { valores[r as usize] });
        let f = linhas_escondidas_com(
            0,
            2,
            &[(0, Criterio::Dinamico(Dinamico::EsteMes)), (1, Criterio::Top10 { topo: true, n: 1, percentual: false })],
            &Contexto { valor: &valor, cores: None, hoje },
        );
        assert_eq!(f, vec![(1, 2)]);
    }
}
