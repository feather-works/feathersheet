//! Os campos da tabela dinâmica e a prévia do campo calculado (24/09/2026).
//!
//! Era o `pivot-panel.ts` lendo a origem célula a célula pela ponte: o
//! cabeçalho e 15 linhas de amostra por coluna, para a lista de campos, e as
//! três primeiras linhas para a prévia do campo calculado. Agora é uma ida só.

use crate::celulas::values::{to_text, Value};
use crate::dinamica::campo_calculado::{avaliar, avaliar_sequencia, compilar, compilar_sequencia, numero_de};
use crate::celulas::refs::Rect;

/// Quantas linhas abaixo do cabeçalho entram na amostra do "é número?".
pub const AMOSTRA: u32 = 15;

#[derive(Debug, Clone, PartialEq)]
pub struct Campo {
    pub nome: String,
    /// Pelo menos metade da amostra preenchida é número (a regra do painel).
    pub numerico: bool,
    /// A primeira linha de dados é número (o que a caixa do campo calculado usa).
    pub primeiro_numero: bool,
}

/// O que o `Number(v)` do JavaScript aceita como número, para a amostra dar
/// o mesmo que dava no painel: número, verdadeiro/falso e texto numérico.
fn parece_numero(v: &Value) -> bool {
    match v {
        Value::Number(_) | Value::Bool(_) => true,
        Value::Text(t) => {
            let t = t.trim();
            t.is_empty() || (t.parse::<f64>().is_ok() && !t.eq_ignore_ascii_case("nan"))
        }
        _ => false,
    }
}

/// Nome e tipo de cada coluna de `origem` (a primeira linha é o cabeçalho).
pub fn campos(origem: Rect, valor: impl Fn(u32, u32) -> Value) -> Vec<Campo> {
    let o = origem.normalized();
    (o.c0..=o.c1)
        .map(|c| {
            let nome = to_text(&valor(o.r0, c)).trim().to_string();
            let nome = if nome.is_empty() { format!("Coluna {}", c - o.c0 + 1) } else { nome };
            let (mut amostra, mut numeros) = (0u32, 0u32);
            let fim = o.r1.min(o.r0.saturating_add(AMOSTRA));
            for r in o.r0 + 1..=fim {
                let v = valor(r, c);
                if matches!(&v, Value::Empty) || matches!(&v, Value::Text(t) if t.is_empty()) {
                    continue;
                }
                amostra += 1;
                if parece_numero(&v) {
                    numeros += 1;
                }
            }
            let primeiro_numero = o.r1 > o.r0 && matches!(valor(o.r0 + 1, c), Value::Number(_));
            Campo { nome, numerico: amostra > 0 && numeros * 2 >= amostra, primeiro_numero }
        })
        .collect()
}

/// A prévia do campo calculado: a fórmula nas `linhas` primeiras linhas de
/// dados, ou a mensagem de por que ela não compila.
pub fn previa(origem: Rect, formula: &str, linhas: u32, valor: impl Fn(u32, u32) -> Value) -> Result<Vec<f64>, String> {
    let o = origem.normalized();
    let nomes: Vec<String> = campos(o.clone(), &valor).into_iter().map(|c| c.nome).collect();
    let expr = compilar(formula, &nomes)?;
    let fim = o.r1.min(o.r0.saturating_add(linhas));
    Ok((o.r0 + 1..=fim)
        .map(|lin| avaliar(&expr, &|i| numero_de(&valor(lin, o.c0 + i as u32))))
        .collect())
}

/// Prévia de campos calculados encadeados sem materializar a origem.
pub fn previa_sequencia(
    origem: Rect,
    calculados: &[(String, String)],
    linhas: u32,
    valor: impl Fn(u32, u32) -> Value,
) -> Result<Vec<Vec<f64>>, String> {
    let o = origem.normalized();
    let nomes: Vec<String> = campos(o.clone(), &valor).into_iter().map(|c| c.nome).collect();
    let expressoes = compilar_sequencia(&nomes, calculados)?;
    let fim = o.r1.min(o.r0.saturating_add(linhas));
    Ok((o.r0 + 1..=fim).map(|r| {
        let originais: Vec<f64> = (o.c0..=o.c1).map(|c| numero_de(&valor(r, c))).collect();
        avaliar_sequencia(&expressoes, &originais)
    }).collect())
}

/// Linhas lidas para achar as colunas de categoria (poucos valores repetidos).
const AMOSTRA_SUGESTOES: u32 = 2_000;
/// Mais valores distintos que isto na amostra não é categoria para agrupar.
const MAX_CATEGORIAS: usize = 60;

/// Uma dinâmica recomendada (Inserir ▸ Dinâmicas recomendadas): colunas pela posição na origem.
#[derive(Debug, Clone, PartialEq)]
pub struct Sugestao {
    pub linhas: Vec<usize>,
    pub colunas: Vec<usize>,
    pub valor: usize,
    /// `true`: contagem; `false`: soma.
    pub contagem: bool,
}

/// Até 5 resumos que fazem sentido para os dados, como as recomendadas do Excel: soma de um
/// número por uma categoria (texto com poucos valores), por duas, outro número, e contagem.
pub fn sugestoes(origem: Rect, valor: impl Fn(u32, u32) -> Value) -> Vec<Sugestao> {
    let o = origem.normalized();
    let lista = campos(o.clone(), &valor);
    let fim = o.r1.min(o.r0.saturating_add(AMOSTRA_SUGESTOES));
    let mut categorias = Vec::new();
    let mut numeros = Vec::new();
    for (i, campo) in lista.iter().enumerate() {
        let c = o.c0 + i as u32;
        let nome = campo.nome.to_lowercase();
        if campo.numerico {
            // Código e identificador não se somam.
            let codigo = ["id", "código", "codigo", "cód", "cod", "nº", "numero", "número", "cep", "ano"]
                .iter().any(|p| nome == *p || nome.starts_with(&format!("{p} ")) || nome.ends_with(&format!(" {p}")));
            if !codigo { numeros.push(i); }
            continue;
        }
        let mut distintos = std::collections::HashSet::new();
        let mut preenchidas = 0u32;
        for r in o.r0 + 1..=fim {
            let t = to_text(&valor(r, c)).trim().to_lowercase();
            if t.is_empty() { continue; }
            preenchidas += 1;
            distintos.insert(t);
            if distintos.len() > MAX_CATEGORIAS { break; }
        }
        // Categoria: repete (menos distintos que metade das linhas) e cabe numa tabela.
        if distintos.len() >= 2 && distintos.len() <= MAX_CATEGORIAS && (distintos.len() as u32) * 2 <= preenchidas {
            categorias.push(i);
        }
    }
    let mut saida: Vec<Sugestao> = Vec::new();
    let mut por = |linhas: Vec<usize>, colunas: Vec<usize>, valor: usize, contagem: bool| {
        let s = Sugestao { linhas, colunas, valor, contagem };
        if saida.len() < 5 && !saida.contains(&s) { saida.push(s); }
    };
    if let Some(&n) = numeros.first() {
        for &cat in categorias.iter().take(2) { por(vec![cat], vec![], n, false); }
        if let [a, b, ..] = categorias[..] { por(vec![a], vec![b], n, false); }
        if let (Some(&n2), Some(&cat)) = (numeros.get(1), categorias.first()) { por(vec![cat], vec![], n2, false); }
    }
    if let Some(&cat) = categorias.first() { por(vec![cat], vec![], cat, true); }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sugestoes_somam_numero_por_categoria_e_contam() {
        // ID | Região | Produto | Valor | Qtd ; 40 linhas, 4 regiões, produtos todos diferentes.
        let regioes = ["Norte", "Sul", "Leste", "Oeste"];
        let valor = |r: u32, c: u32| -> Value {
            if r == 0 { return Value::Text(["ID", "Região", "Produto", "Valor", "Qtd"][c as usize].into()); }
            match c {
                0 => Value::Number(r as f64),
                1 => Value::Text(regioes[(r % 4) as usize].into()),
                2 => Value::Text(format!("P{r}")),
                3 => Value::Number(r as f64 * 1.5),
                _ => Value::Number(2.0),
            }
        };
        let s = sugestoes(Rect::new(0, 0, 40, 4), valor);
        assert_eq!(s[0], Sugestao { linhas: vec![1], colunas: vec![], valor: 3, contagem: false }, "soma de Valor por Região");
        assert!(s.contains(&Sugestao { linhas: vec![1], colunas: vec![], valor: 4, contagem: false }), "Qtd por Região");
        assert!(s.contains(&Sugestao { linhas: vec![1], colunas: vec![], valor: 1, contagem: true }), "contagem por Região");
        assert!(s.iter().all(|x| x.valor != 0 && !x.linhas.contains(&2)), "ID não soma; Produto único não agrupa");
        assert!(sugestoes(Rect::new(0, 0, 3, 0), |_, _| Value::Number(1.0)).is_empty());
    }

    fn tabela() -> Vec<Vec<Value>> {
        let t = |s: &str| Value::Text(s.into());
        vec![
            vec![t("Produto"), t("Qtd"), Value::Empty, t("Preço")],
            vec![t("A"), Value::Number(2.0), Value::Empty, t("10")],
            vec![t("B"), Value::Number(3.0), Value::Empty, Value::Number(4.5)],
            vec![t("C"), t("x"), Value::Empty, Value::Bool(true)],
        ]
    }

    fn ler(t: &[Vec<Value>]) -> impl Fn(u32, u32) -> Value + '_ {
        move |r, c| t.get(r as usize).and_then(|l| l.get(c as usize)).cloned().unwrap_or(Value::Empty)
    }

    #[test]
    fn nome_vazio_vira_coluna_n_e_a_amostra_decide_o_tipo() {
        let t = tabela();
        let c = campos(Rect::new(0, 0, 3, 3), ler(&t));
        let nomes: Vec<&str> = c.iter().map(|x| x.nome.as_str()).collect();
        assert_eq!(nomes, ["Produto", "Qtd", "Coluna 3", "Preço"]);
        // Qtd: 2 de 3 são número; Preço: "10", 4,5 e VERDADEIRO contam, como no Number() do JS.
        let tipos: Vec<bool> = c.iter().map(|x| x.numerico).collect();
        assert_eq!(tipos, [false, true, false, true]);
        let primeiros: Vec<bool> = c.iter().map(|x| x.primeiro_numero).collect();
        assert_eq!(primeiros, [false, true, false, false], "\"10\" em texto não é número na primeira linha");
    }

    #[test]
    fn previa_calcula_as_primeiras_linhas_ou_diz_o_erro() {
        let t = tabela();
        let area = Rect::new(0, 0, 3, 3);
        assert_eq!(previa(area.clone(), "Qtd * 2", 3, ler(&t)), Ok(vec![4.0, 6.0, 0.0]));
        assert_eq!(previa(area.clone(), "qtd + preço", 2, ler(&t)), Ok(vec![12.0, 7.5]));
        assert!(previa(area, "Nada + 1", 3, ler(&t)).is_err());
    }

    #[test]
    fn previa_de_campos_encadeados() {
        let t = tabela();
        let formulas = vec![
            ("Dobro".into(), "Qtd * 2".into()),
            ("Total".into(), "Dobro + Preço".into()),
        ];
        let resultado = previa_sequencia(Rect::new(0, 0, 3, 3), &formulas, 2, ler(&t)).unwrap();
        assert_eq!(resultado, vec![vec![4.0, 14.0], vec![6.0, 10.5]]);
    }

    #[test]
    fn amostra_para_em_15_linhas() {
        // 20 linhas de texto depois de 15 números: continua numérico.
        let mut t = vec![vec![Value::Text("N".into())]];
        t.extend((0..15).map(|i| vec![Value::Number(i as f64)]));
        t.extend((0..20).map(|_| vec![Value::Text("x".into())]));
        assert!(campos(Rect::new(0, 0, 35, 0), ler(&t))[0].numerico);
    }
}
