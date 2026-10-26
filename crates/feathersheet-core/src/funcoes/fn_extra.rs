//! Lote de funções do Excel 365 que faltavam (15/10/2026): matriz dinâmica
//! (EMPILHARV, PEGAR…), pesquisa (PROC, CORRESPX, ENDEREÇO), estatística
//! (EPADYX, PROB, TESTE.F, TESTE.Z, TENDÊNCIA, CRESCIMENTO), texto
//! (`*B`, CODIFURL), engenharia (CONVERTER, BESSELJ, BESSELI) e financeiras de
//! título de desconto (DESC, TAXAJUROS, RECEBER, OTN…).
//!
//! Cada função tem o nome em português e em inglês na tabela de apelidos de
//! `functions.rs`; a ajuda de cada uma está em `function_help_data.rs`.
//!
//! As funções `*B` (LENB, LEFTB…) contam bytes só em idiomas de byte duplo
//! (japonês, chinês, coreano); em todos os outros o Excel as trata como as
//! versões sem B, que é o que fazem aqui.

use crate::celulas::refs::col_name;
use crate::celulas::values::{compare, Computed, Matrix, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::make_predicate;
use crate::funcoes::fn_helpers::*;
use crate::funcoes::fn_text;

// --- auxiliares --------------------------------------------------------------

type Grade = Vec<Vec<Value>>;

fn grade(args: &[Node], ev: &Evaluator<'_>, i: usize) -> Res<Grade> {
    Ok(arg_matriz(args, ev, i)?.rows().to_vec())
}

fn largura(g: &Grade) -> usize {
    g.first().map_or(0, Vec::len)
}

fn nd() -> Value {
    Value::Error(e_nd())
}

/// Monta a matriz; sem nenhuma célula é `#CALC!`.
fn saida(rows: Grade) -> Res<Matrix> {
    montar_matriz(rows)
}

/// Os itens de uma grade que é linha ou coluna (ou um item). `#VALOR!` se for
/// uma tabela de verdade.
fn vetor(g: &Grade) -> Res<Vec<Value>> {
    if g.len() > 1 && largura(g) > 1 {
        return Err(e_valor());
    }
    Ok(g.iter().flatten().cloned().collect())
}

fn mesmo_tipo(a: &Value, b: &Value) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// Inteiro de quantidade (linhas/colunas): trunca; fora de 1..=1.048.576 é `#VALOR!`.
fn quantidade(n: f64) -> Res<usize> {
    if !n.is_finite() || n < 1.0 || n > 1_048_576.0 {
        return Err(e_valor());
    }
    Ok(n.trunc() as usize)
}

// --- matriz dinâmica ---------------------------------------------------------

/// `=EMPILHARV(matriz1; [matriz2]; …)` / `=VSTACK(array1, [array2], …)`
pub fn fn_vstack(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 254, || {
            let mut partes = Vec::new();
            for i in 0..args.len() {
                partes.push(grade(args, ev, i)?);
            }
            let w = partes.iter().map(largura).max().unwrap_or(0);
            let mut rows = Vec::new();
            for g in partes {
                for mut l in g {
                    l.resize(w, nd());
                    rows.push(l);
                }
            }
            saida(rows)
    })
}

/// `=EMPILHARH(matriz1; [matriz2]; …)` / `=HSTACK(array1, [array2], …)`
pub fn fn_hstack(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 254, || {
            let mut partes = Vec::new();
            for i in 0..args.len() {
                partes.push(grade(args, ev, i)?);
            }
            let h = partes.iter().map(Vec::len).max().unwrap_or(0);
            let mut rows: Grade = vec![Vec::new(); h];
            for g in partes {
                let w = largura(&g);
                for (r, linha) in rows.iter_mut().enumerate() {
                    match g.get(r) {
                        Some(l) => linha.extend(l.iter().cloned()),
                        None => linha.extend(vec![nd(); w]),
                    }
                }
            }
            saida(rows)
    })
}

/// Faixa `[ini, fim)` de um tamanho `total` para TAKE (`pegar`) ou DROP.
fn faixa_pegar(total: usize, n: i64, pegar: bool) -> (usize, usize) {
    let t = total as i64;
    let k = n.abs().min(t);
    let (ini, fim) = match (pegar, n >= 0) {
        (true, true) => (0, k),
        (true, false) => (t - k, t),
        (false, true) => (k, t),
        (false, false) => (0, t - k),
    };
    (ini as usize, fim as usize)
}

fn pegar_ou_descartar(args: &[Node], ev: &Evaluator<'_>, pegar: bool) -> Res<Matrix> {
    let g = grade(args, ev, 0)?;
    let (h, w) = (g.len(), largura(&g));
    let n_lin = if has(args, 1) { Some(arg_int(args, ev, 1)?) } else { None };
    let n_col = if has(args, 2) { Some(arg_int(args, ev, 2)?) } else { None };
    if pegar && (n_lin == Some(0) || n_col == Some(0)) {
        return Err(e_calc());
    }
    let (l0, l1) = n_lin.map_or((0, h), |n| faixa_pegar(h, n, pegar));
    let (c0, c1) = n_col.map_or((0, w), |n| faixa_pegar(w, n, pegar));
    if l0 >= l1 || c0 >= c1 {
        return Err(e_calc());
    }
    saida(g[l0..l1].iter().map(|l| l[c0..c1].to_vec()).collect())
}

/// `=PEGAR(matriz; [linhas]; [colunas])` / `=TAKE(array, [rows], [columns])`
pub fn fn_take(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 3, || {
            pegar_ou_descartar(args, ev, true)
    })
}

/// `=DESCARTAR(matriz; [linhas]; [colunas])` / `=DROP(array, [rows], [columns])`
pub fn fn_drop(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 3, || {
            pegar_ou_descartar(args, ev, false)
    })
}

/// Posições (base 0) pedidas por CHOOSECOLS/CHOOSEROWS; negativo conta do fim.
fn indices(args: &[Node], ev: &Evaluator<'_>, total: usize) -> Res<Vec<usize>> {
    let mut out = Vec::new();
    for i in 1..args.len() {
        for n in arg_numeros(args, ev, i)? {
            let n = n.trunc() as i64;
            let t = total as i64;
            let pos = if n > 0 && n <= t {
                n - 1
            } else if n < 0 && -n <= t {
                t + n
            } else {
                return Err(e_valor());
            };
            out.push(pos as usize);
        }
    }
    Ok(out)
}

/// `=ESCOLHERCOLS(matriz; núm_col1; …)` / `=CHOOSECOLS(array, col_num1, …)`
pub fn fn_choosecols(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 254, || {
            let g = grade(args, ev, 0)?;
            let cols = indices(args, ev, largura(&g))?;
            saida(g.iter().map(|l| cols.iter().map(|&c| l[c].clone()).collect()).collect())
    })
}

/// `=ESCOLHERLINS(matriz; núm_lin1; …)` / `=CHOOSEROWS(array, row_num1, …)`
pub fn fn_chooserows(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 254, || {
            let g = grade(args, ev, 0)?;
            let lins = indices(args, ev, g.len())?;
            saida(lins.iter().map(|&r| g[r].clone()).collect())
    })
}

/// Itens de uma matriz na ordem pedida, sem os que `ignorar` descarta
/// (0 nada, 1 vazios, 2 erros, 3 os dois).
fn achatar(args: &[Node], ev: &Evaluator<'_>) -> Res<Vec<Value>> {
    let g = grade(args, ev, 0)?;
    let ignorar = if has(args, 1) { arg_int(args, ev, 1)? } else { 0 };
    if !(0..=3).contains(&ignorar) {
        return Err(e_valor());
    }
    let por_coluna = arg_logico_ou(args, ev, 2, false)?;
    let (h, w) = (g.len(), largura(&g));
    let mut itens = Vec::with_capacity(h * w);
    let mut empurra = |v: &Value| {
        let pular = match v {
            Value::Empty => ignorar & 1 != 0,
            Value::Error(_) => ignorar & 2 != 0,
            _ => false,
        };
        if !pular {
            itens.push(v.clone());
        }
    };
    if por_coluna {
        for c in 0..w {
            for l in &g {
                empurra(&l[c]);
            }
        }
    } else {
        for l in &g {
            for v in l {
                empurra(v);
            }
        }
    }
    if itens.is_empty() {
        return Err(e_calc());
    }
    Ok(itens)
}

/// `=PARACOL(matriz; [ignorar]; [verificar_por_coluna])` / `=TOCOL(array, [ignore], [scan_by_column])`
pub fn fn_tocol(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 3, || {
            saida(achatar(args, ev)?.into_iter().map(|v| vec![v]).collect())
    })
}

/// `=PARALIN(matriz; [ignorar]; [verificar_por_coluna])` / `=TOROW(array, [ignore], [scan_by_column])`
pub fn fn_torow(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 3, || {
            saida(vec![achatar(args, ev)?])
    })
}

/// WRAPROWS (`por_linha`) e WRAPCOLS: quebra um vetor em blocos de `n`.
fn quebrar(args: &[Node], ev: &Evaluator<'_>, por_linha: bool) -> Res<Matrix> {
    let itens = vetor(&grade(args, ev, 0)?)?;
    let n = arg_int(args, ev, 1)?;
    if n < 1 {
        return Err(e_num());
    }
    let n = (n as usize).min(itens.len().max(1));
    let pad = if has(args, 2) { arg_valor(args, ev, 2)? } else { nd() };
    let blocos = itens.len().div_ceil(n);
    let mut rows: Grade = if por_linha {
        vec![vec![pad.clone(); n]; blocos]
    } else {
        vec![vec![pad.clone(); blocos]; n]
    };
    for (k, v) in itens.into_iter().enumerate() {
        let (bloco, dentro) = (k / n, k % n);
        if por_linha {
            rows[bloco][dentro] = v;
        } else {
            rows[dentro][bloco] = v;
        }
    }
    saida(rows)
}

/// `=QUEBRARLINS(vetor; contagem; [preencher_com])` / `=WRAPROWS(vector, wrap_count, [pad_with])`
pub fn fn_wraprows(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 3, || {
            quebrar(args, ev, true)
    })
}

/// `=QUEBRARCOLS(vetor; contagem; [preencher_com])` / `=WRAPCOLS(vector, wrap_count, [pad_with])`
pub fn fn_wrapcols(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 3, || {
            quebrar(args, ev, false)
    })
}

/// `=EXPANDIR(matriz; linhas; [colunas]; [preencher_com])` / `=EXPAND(array, rows, [columns], [pad_with])`
pub fn fn_expand(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 4, || {
            let mut g = grade(args, ev, 0)?;
            let (h, w) = (g.len(), largura(&g));
            let linhas = if has(args, 1) { quantidade(arg_num(args, ev, 1)?)? } else { h };
            let colunas = if has(args, 2) { quantidade(arg_num(args, ev, 2)?)? } else { w };
            if linhas < h || colunas < w {
                return Err(e_valor());
            }
            if linhas.checked_mul(colunas).is_none_or(|n| n > MAX_CELULAS) {
                return Err(e_num());
            }
            let pad = if has(args, 3) { arg_valor(args, ev, 3)? } else { nd() };
            for l in &mut g {
                l.resize(colunas, pad.clone());
            }
            g.resize(linhas, vec![pad; colunas]);
            saida(g)
    })
}

/// `=CLASSIFICARPOR(matriz; por_matriz1; [ordem1]; …)` / `=SORTBY(array, by_array1, [sort_order1], …)`
pub fn fn_sortby(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 255, || {
            let g = grade(args, ev, 0)?;
            let (h, w) = (g.len(), largura(&g));
            // Cada critério: (valores, ordem). Todos são colunas de `h` itens (ordena
            // linhas) ou todos linhas de `w` itens (ordena colunas).
            let mut criterios: Vec<(Vec<Value>, i64)> = Vec::new();
            let mut por_linhas: Option<bool> = None;
            let mut i = 1;
            while i < args.len() {
                let b = grade(args, ev, i)?;
                let (bh, bw) = (b.len(), largura(&b));
                let eh_coluna = bw == 1 && bh == h;
                let eh_linha = bh == 1 && bw == w;
                let linhas = if eh_coluna {
                    true
                } else if eh_linha {
                    false
                } else {
                    return Err(e_valor());
                };
                if *por_linhas.get_or_insert(linhas) != linhas {
                    return Err(e_valor());
                }
                let ordem = if i + 1 < args.len() && has(args, i + 1) { arg_int(args, ev, i + 1)? } else { 1 };
                if ordem != 1 && ordem != -1 {
                    return Err(e_valor());
                }
                criterios.push((b.into_iter().flatten().collect(), ordem));
                i += 2;
            }
            let n = if por_linhas == Some(true) { h } else { w };
            let mut ordem: Vec<usize> = (0..n).collect();
            ordem.sort_by(|&x, &y| {
                for (valores, sentido) in &criterios {
                    let c = compare(&valores[x], &valores[y]).unwrap_or(std::cmp::Ordering::Equal);
                    let c = if *sentido < 0 { c.reverse() } else { c };
                    if c != std::cmp::Ordering::Equal {
                        return c;
                    }
                }
                std::cmp::Ordering::Equal
            });
            if por_linhas == Some(true) {
                saida(ordem.into_iter().map(|r| g[r].clone()).collect())
            } else {
                saida(g.iter().map(|l| ordem.iter().map(|&c| l[c].clone()).collect()).collect())
            }
    })
}

/// Sorteio simples (xorshift), semente tirada do relógio na primeira chamada.
fn sorteio() -> f64 {
    use std::cell::Cell;
    thread_local! {
        static SEMENTE: Cell<u64> = const { Cell::new(0) };
    }
    SEMENTE.with(|s| {
        let mut x = s.get();
        if x == 0 {
            x = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x9e37_79b9_7f4a_7c15, |d| d.as_nanos() as u64)
                | 1;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.set(x);
        (x >> 11) as f64 / (1u64 << 53) as f64
    })
}

/// `=MATRIZALEATÓRIA([linhas]; [colunas]; [mín]; [máx]; [inteiro])` / `=RANDARRAY([rows], [columns], [min], [max], [integer])`
pub fn fn_randarray(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 0, 5, || {
            let linhas = if has(args, 0) { quantidade(arg_num(args, ev, 0)?)? } else { 1 };
            let colunas = if has(args, 1) { quantidade(arg_num(args, ev, 1)?)? } else { 1 };
            let min = if has(args, 2) { arg_num(args, ev, 2)? } else { 0.0 };
            let max = if has(args, 3) { arg_num(args, ev, 3)? } else { 1.0 };
            let inteiro = arg_logico_ou(args, ev, 4, false)?;
            if min > max {
                return Err(e_valor());
            }
            if linhas.checked_mul(colunas).is_none_or(|n| n > MAX_CELULAS) {
                return Err(e_num());
            }
            let dados = (0..linhas * colunas)
                .map(|_| {
                    let r = sorteio();
                    if inteiro {
                        let (lo, hi) = (min.ceil(), max.floor());
                        Value::Number((lo + (r * (hi - lo + 1.0)).floor()).min(hi))
                    } else {
                        Value::Number(min + r * (max - min))
                    }
                })
                .collect();
            if inteiro && min.ceil() > max.floor() {
                return Err(e_valor());
            }
            matriz_plana(linhas, colunas, dados)
    })
}

// --- pesquisa e referência -----------------------------------------------------

/// `=PROC(valor_procurado; vetor_proc; [vetor_result])` / `=LOOKUP(lookup_value, lookup_vector, [result_vector])`
pub fn fn_lookup(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 3, || {
            let alvo = arg_valor(args, ev, 0)?;
            if let Value::Error(e) = &alvo {
                return Err(e.clone());
            }
            let g = grade(args, ev, 1)?;
            let (busca, resultado): (Vec<Value>, Vec<Value>) = if has(args, 2) {
                (vetor(&g)?, vetor(&grade(args, ev, 2)?)?)
            } else if largura(&g) > g.len() {
                // Matriz mais larga que alta: procura na 1ª linha, devolve a última.
                (g[0].clone(), g[g.len() - 1].clone())
            } else {
                (
                    g.iter().map(|l| l[0].clone()).collect(),
                    g.iter().map(|l| l[l.len() - 1].clone()).collect(),
                )
            };
            let mut achado = None;
            for (i, v) in busca.iter().enumerate() {
                if !mesmo_tipo(v, &alvo) {
                    continue;
                }
                match compare(v, &alvo) {
                    Ok(std::cmp::Ordering::Greater) => break,
                    Ok(_) => achado = Some(i),
                    Err(_) => {}
                }
            }
            match achado.and_then(|i| resultado.get(i)) {
                Some(v) => Ok(v.clone()),
                None => Err(e_nd()),
            }
    })
}

/// `=CORRESPX(valor_procurado; matriz_procurada; [modo_correspondência]; [modo_pesquisa])`
/// / `=XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`
pub fn fn_xmatch(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 4, || {
            let alvo = arg_valor(args, ev, 0)?;
            if let Value::Error(e) = &alvo {
                return Err(e.clone());
            }
            let itens = vetor(&grade(args, ev, 1)?)?;
            let modo = if has(args, 2) { arg_int(args, ev, 2)? } else { 0 };
            let pesquisa = if has(args, 3) { arg_int(args, ev, 3)? } else { 1 };
            if !(-1..=2).contains(&modo) || !matches!(pesquisa, -2 | -1 | 1 | 2) {
                return Err(e_valor());
            }
            numero((posicao_xmatch(itens.len(), |i| itens[i].clone(), &alvo, modo, pesquisa)? + 1) as f64)
    })
}

/// O miolo do `CORRESPX`: a posição (a partir de 0) de `alvo` em `itens`.
/// Também usado por CORRESP, PROCV e PROCH (`fn_lookup.rs`), que só traduzem
/// o tipo de correspondência para o `modo` daqui. `item(i)` lê o i-ésimo
/// valor, para o PROCV não montar a coluna inteira num vetor a cada chamada.
pub(crate) fn posicao_xmatch(n: usize, item: impl Fn(usize) -> Value, alvo: &Value, modo: i64, pesquisa: i64) -> Res<usize> {
    let ordem: Box<dyn Iterator<Item = usize>> = if pesquisa < 0 { Box::new((0..n).rev()) } else { Box::new(0..n) };
    let curinga = match (alvo, modo) {
        (Value::Text(t), 2) if t.contains('*') || t.contains('?') => Some(make_predicate(alvo)),
        _ => None,
    };
    let mut melhor: Option<usize> = None;
    for i in ordem {
        let v = &item(i);
        if !mesmo_tipo(v, alvo) {
            continue;
        }
        let c = match compare(v, alvo) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if c.is_eq() || curinga.as_ref().is_some_and(|p| p(v)) {
            return Ok(i);
        }
        let candidato = match modo {
            -1 => c.is_lt(),
            1 => c.is_gt(),
            _ => false,
        };
        if candidato {
            let troca = match melhor.map(&item) {
                None => true,
                Some(m) => {
                    let o = compare(v, &m).unwrap_or(std::cmp::Ordering::Equal);
                    if modo == -1 { o.is_gt() } else { o.is_lt() }
                }
            };
            if troca {
                melhor = Some(i);
            }
        }
    }
    melhor.ok_or_else(e_nd)
}

/// `=ENDEREÇO(lin; col; [abs]; [a1]; [planilha])` / `=ADDRESS(row_num, column_num, [abs_num], [a1], [sheet_text])`
pub fn fn_address(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 5, || {
            let lin = arg_int(args, ev, 0)?;
            let col = arg_int(args, ev, 1)?;
            let abs = if has(args, 2) { arg_int(args, ev, 2)? } else { 1 };
            let a1 = arg_logico_ou(args, ev, 3, true)?;
            if !(1..=1_048_576).contains(&lin) || !(1..=16_384).contains(&col) || !(1..=4).contains(&abs) {
                return Err(e_valor());
            }
            let (lin_abs, col_abs) = (abs == 1 || abs == 2, abs == 1 || abs == 3);
            let ref_ = if a1 {
                format!(
                    "{}{}{}{}",
                    if col_abs { "$" } else { "" },
                    col_name((col - 1) as u32),
                    if lin_abs { "$" } else { "" },
                    lin
                )
            } else {
                let parte = |letra: char, n: i64, absoluto: bool| {
                    if absoluto { format!("{letra}{n}") } else { format!("{letra}[{n}]") }
                };
                format!("{}{}", parte('R', lin, lin_abs), parte('C', col, col_abs))
            };
            if has(args, 4) {
                let aba = arg_texto(args, ev, 4)?;
                if !aba.is_empty() {
                    let simples = aba.chars().all(|c| c.is_alphanumeric() || c == '_');
                    let aba = if simples { aba } else { format!("'{}'", aba.replace('\'', "''")) };
                    return texto(format!("{aba}!{ref_}"));
                }
            }
            texto(ref_)
    })
}

/// `=HIPERLINK(local_vínculo; [nome_amigável])` / `=HYPERLINK(link_location, [friendly_name])`
/// O valor é o nome amigável (ou o endereço); abrir o vínculo é função da interface.
pub fn fn_hyperlink(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 2, || {
            if has(args, 1) {
                arg_valor(args, ev, 1)
            } else {
                arg_valor(args, ev, 0)
            }
    })
}

/// `=ÉREF(valor)` / `=ISREF(value)`: só olha a árvore, não o valor.
pub fn fn_isref(args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    if let Some(e) = crate::funcoes::fn_helpers::arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(e));
    }
    Computed::Scalar(Value::Bool(matches!(args[0], Node::Ref { .. } | Node::Range { .. })))
}

// --- estatística ---------------------------------------------------------------

/// Pares (x, y) onde os dois são número; erro numa célula propaga; tamanhos
/// diferentes dão `#N/D`.
fn pares_xy(args: &[Node], ev: &Evaluator<'_>, i_y: usize, i_x: usize) -> Res<(Vec<f64>, Vec<f64>)> {
    let y = grade(args, ev, i_y)?;
    let x = grade(args, ev, i_x)?;
    let (y, x): (Vec<&Value>, Vec<&Value>) = (y.iter().flatten().collect(), x.iter().flatten().collect());
    if y.len() != x.len() {
        return Err(e_nd());
    }
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for (p, q) in x.into_iter().zip(y) {
        for v in [p, q] {
            if let Value::Error(e) = v {
                return Err(e.clone());
            }
        }
        if let (Value::Number(px), Value::Number(qy)) = (p, q) {
            xs.push(*px);
            ys.push(*qy);
        }
    }
    Ok((xs, ys))
}

fn soma_desvios(x: &[f64], y: &[f64]) -> (f64, f64, f64, f64, f64) {
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, y.iter().sum::<f64>() / n);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for (px, py) in x.iter().zip(y) {
        sxx += (px - mx) * (px - mx);
        syy += (py - my) * (py - my);
        sxy += (px - mx) * (py - my);
    }
    (sxx, syy, sxy, mx, my)
}

/// `=EPADYX(y_conhecidos; x_conhecidos)` / `=STEYX(known_y's, known_x's)`
pub fn fn_steyx(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
            let (x, y) = pares_xy(args, ev, 0, 1)?;
            if x.len() < 3 {
                return Err(e_div0());
            }
            let (sxx, syy, sxy, _, _) = soma_desvios(&x, &y);
            if sxx == 0.0 {
                return Err(e_div0());
            }
            numero(((syy - sxy * sxy / sxx).max(0.0) / (x.len() - 2) as f64).sqrt())
    })
}

/// `=PROB(intervalo_x; intervalo_prob; limite_inferior; [limite_superior])`
/// / `=PROB(x_range, prob_range, lower_limit, [upper_limit])`
pub fn fn_prob(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 3, 4, || {
            let xs = grade(args, ev, 0)?;
            let ps = grade(args, ev, 1)?;
            let xs: Vec<&Value> = xs.iter().flatten().collect();
            let ps: Vec<&Value> = ps.iter().flatten().collect();
            if xs.len() != ps.len() {
                return Err(e_nd());
            }
            let inferior = arg_num(args, ev, 2)?;
            let superior = if has(args, 3) { arg_num(args, ev, 3)? } else { inferior };
            let (mut total, mut soma) = (0.0, 0.0);
            for (x, p) in xs.into_iter().zip(ps) {
                for v in [x, p] {
                    if let Value::Error(e) = v {
                        return Err(e.clone());
                    }
                }
                let p = match p {
                    Value::Number(p) => *p,
                    _ => return Err(e_num()),
                };
                if p <= 0.0 || p > 1.0 {
                    return Err(e_num());
                }
                soma += p;
                if let Value::Number(x) = x {
                    if *x >= inferior && *x <= superior {
                        total += p;
                    }
                }
            }
            if (soma - 1.0).abs() > 1e-9 {
                return Err(e_num());
            }
            numero(total)
    })
}

/// `=TESTE.F(matriz1; matriz2)` / `=F.TEST(array1, array2)`
pub fn fn_f_test(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 2, || {
            let (x, y) = (arg_numeros(args, ev, 0)?, arg_numeros(args, ev, 1)?);
            if x.len() < 2 || y.len() < 2 {
                return Err(e_div0());
            }
            let var = |v: &[f64]| {
                let m = v.iter().sum::<f64>() / v.len() as f64;
                v.iter().map(|p| (p - m) * (p - m)).sum::<f64>() / (v.len() - 1) as f64
            };
            let (v1, v2) = (var(&x), var(&y));
            if v1 == 0.0 || v2 == 0.0 {
                return Err(e_div0());
            }
            let p = crate::funcoes::fn_dist::f_cdf(v1 / v2, (x.len() - 1) as f64, (y.len() - 1) as f64);
            numero(2.0 * p.min(1.0 - p))
    })
}

/// `=TESTE.Z(matriz; x; [sigma])` / `=Z.TEST(array, x, [sigma])`: valor P unicaudal.
pub fn fn_z_test(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 2, 3, || {
            let v = arg_numeros(args, ev, 0)?;
            if v.is_empty() {
                return Err(e_nd());
            }
            let n = v.len() as f64;
            let media = v.iter().sum::<f64>() / n;
            let x = arg_num(args, ev, 1)?;
            let sigma = if has(args, 2) {
                arg_num(args, ev, 2)?
            } else {
                if v.len() < 2 {
                    return Err(e_div0());
                }
                (v.iter().map(|p| (p - media) * (p - media)).sum::<f64>() / (n - 1.0)).sqrt()
            };
            if sigma <= 0.0 {
                return if has(args, 2) { Err(e_num()) } else { Err(e_div0()) };
            }
            numero(1.0 - crate::funcoes::fn_dist::norm_cdf((media - x) / (sigma / n.sqrt())))
    })
}

/// Ajuste por mínimos quadrados de uma variável: `(a, b)` em `y = a + b·x`;
/// sem constante, `y = b·x`.
fn ajuste(x: &[f64], y: &[f64], constante: bool) -> Res<(f64, f64)> {
    if x.len() < 2 && constante {
        return Err(e_div0());
    }
    if constante {
        let (sxx, _, sxy, mx, my) = soma_desvios(x, y);
        if sxx == 0.0 {
            return Err(e_div0());
        }
        let b = sxy / sxx;
        Ok((my - b * mx, b))
    } else {
        let sxx: f64 = x.iter().map(|v| v * v).sum();
        if sxx == 0.0 {
            return Err(e_div0());
        }
        Ok((0.0, x.iter().zip(y).map(|(p, q)| p * q).sum::<f64>() / sxx))
    }
}

/// Núcleo de TENDÊNCIA e CRESCIMENTO (`exponencial` ajusta ln y). Só aceita
/// uma variável X (vetor); várias colunas de X dão `#VALOR!`.
fn tendencia(args: &[Node], ev: &Evaluator<'_>, exponencial: bool) -> Res<Matrix> {
    let gy = grade(args, ev, 0)?;
    let y_itens = vetor(&gy)?;
    let n = y_itens.len();
    let x_itens: Vec<Value> = if has(args, 1) {
        let gx = grade(args, ev, 1)?;
        let v = vetor(&gx)?;
        if v.len() != n {
            return Err(e_ref());
        }
        v
    } else {
        (1..=n).map(|i| Value::Number(i as f64)).collect()
    };
    let numero = |v: &Value| match v {
        Value::Number(n) => Ok(*n),
        Value::Error(e) => Err(e.clone()),
        _ => Err(e_valor()),
    };
    let ys = y_itens.iter().map(numero).collect::<Res<Vec<_>>>()?;
    let xs = x_itens.iter().map(numero).collect::<Res<Vec<_>>>()?;
    let constante = arg_logico_ou(args, ev, 3, true)?;
    let alvo_shape = if has(args, 2) { grade(args, ev, 2)? } else if has(args, 1) { grade(args, ev, 1)? } else { gy.clone() };
    let novos: Vec<Vec<f64>> = if has(args, 2) || has(args, 1) {
        alvo_shape.iter().map(|l| l.iter().map(numero).collect::<Res<Vec<_>>>()).collect::<Res<_>>()?
    } else {
        let mut k = 0.0;
        gy.iter().map(|l| l.iter().map(|_| { k += 1.0; k }).collect()).collect()
    };
    let ys_aj = if exponencial {
        if ys.iter().any(|&v| v <= 0.0) {
            return Err(e_num());
        }
        ys.iter().map(|v| v.ln()).collect()
    } else {
        ys
    };
    let (a0, b) = ajuste(&xs, &ys_aj, constante)?;
    let rows = novos
        .iter()
        .map(|l| {
            l.iter()
                .map(|&x| {
                    let v = a0 + b * x;
                    Value::Number(if exponencial { v.exp() } else { v })
                })
                .collect()
        })
        .collect();
    saida(rows)
}

/// `=TENDÊNCIA(y_conhecidos; [x_conhecidos]; [novos_x]; [constante])` / `=TREND(known_y's, [known_x's], [new_x's], [const])`
pub fn fn_trend(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 4, || {
            tendencia(args, ev, false)
    })
}

/// `=CRESCIMENTO(y_conhecidos; [x_conhecidos]; [novos_x]; [constante])` / `=GROWTH(known_y's, [known_x's], [new_x's], [const])`
pub fn fn_growth(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 4, || {
            tendencia(args, ev, true)
    })
}

// --- texto -----------------------------------------------------------------

macro_rules! versao_b {
    ($(#[$m:meta])* $nome:ident => $origem:path) => {
        $(#[$m])*
        pub fn $nome(args: &[Node], ev: &Evaluator<'_>) -> Computed {
            $origem(args, ev)
        }
    };
}

versao_b!(/// `=NÚM.CARACTB(texto)` / `=LENB(text)`
    fn_lenb => fn_text::fn_len);
versao_b!(/// `=ESQUERDAB(texto; [núm_bytes])` / `=LEFTB(text, [num_bytes])`
    fn_leftb => fn_text::fn_left);
versao_b!(/// `=DIREITAB(texto; [núm_bytes])` / `=RIGHTB(text, [num_bytes])`
    fn_rightb => fn_text::fn_right);
versao_b!(/// `=EXT.TEXTOB(texto; núm_inicial; núm_bytes)` / `=MIDB(text, start_num, num_bytes)`
    fn_midb => fn_text::fn_mid);
versao_b!(/// `=PROCURARB(texto_procurado; no_texto; [núm_inicial])` / `=FINDB(find_text, within_text, [start_num])`
    fn_findb => fn_text::fn_find);
versao_b!(/// `=LOCALIZARB(texto_procurado; no_texto; [núm_inicial])` / `=SEARCHB(find_text, within_text, [start_num])`
    fn_searchb => fn_text::fn_search);
versao_b!(/// `=MUDARB(texto_antigo; núm_inicial; núm_bytes; novo_texto)` / `=REPLACEB(old_text, start_num, num_bytes, new_text)`
    fn_replaceb => fn_text::fn_replace);

/// `=CODIFURL(texto)` / `=ENCODEURL(text)`: codificação percentual em UTF-8
/// (só letras, dígitos e `-_.~` ficam como estão).
pub fn fn_encodeurl(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 1, || {
            let mut s = String::new();
            for b in arg_texto(args, ev, 0)?.bytes() {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                    s.push(char::from(b));
                } else {
                    s.push_str(&format!("%{b:02X}"));
                }
            }
            texto(s)
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::ErrorCode;
    use crate::funcoes::fn_helpers::apoio_teste::*;

    fn n(x: f64) -> Value {
        Value::Number(x)
    }

    /// Coluna `col` com os valores a partir da linha 1.
    fn coluna(col: char, v: &[Value]) -> Vec<(String, Value)> {
        v.iter().enumerate().map(|(i, x)| (format!("{col}{}", i + 1), x.clone())).collect()
    }

    fn cal(c: &[(String, Value)], f: &str) -> Value {
        let r: Vec<(&str, Value)> = c.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        calc_cel(&r, f)
    }

    fn mat(c: &[(String, Value)], f: &str) -> Vec<Vec<Value>> {
        let r: Vec<(&str, Value)> = c.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        match calc_com(&r, f) {
            Computed::Matrix(m) => m.rows().to_vec(),
            outro => vec![vec![outro.scalar()]],
        }
    }

    fn nums(rows: &[Vec<Value>]) -> Vec<Vec<f64>> {
        rows.iter()
            .map(|l| l.iter().map(|v| if let Value::Number(x) = v { *x } else { f64::NAN }).collect())
            .collect()
    }

    fn perto(v: Value, esperado: f64, tol: f64) {
        match v {
            Value::Number(x) => assert!((x - esperado).abs() < tol, "obtido {x}, esperado {esperado}"),
            o => panic!("esperava número, veio {o:?}"),
        }
    }

    #[test]
    fn empilhar() {
        let r = nums(&calc_matriz("VSTACK({1\\2};{3\\4})"));
        assert_eq!(r, vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
        let r = calc_matriz("EMPILHARV({1\\2};{3})");
        assert_eq!(r[1][1], Value::Error(e_nd()), "largura que falta vira #N/D");
        let r = nums(&calc_matriz("HSTACK({1;2};{3;4})"));
        assert_eq!(r, vec![vec![1.0, 3.0], vec![2.0, 4.0]]);
        let r = calc_matriz("EMPILHARH({1;2};{3})");
        assert_eq!(r[1][1], Value::Error(e_nd()));
        is_err(calc("VSTACK(1/0)"), ErrorCode::Div0);
    }

    #[test]
    fn pegar_e_descartar() {
        let m = "{1\\2\\3;4\\5\\6;7\\8\\9}";
        assert_eq!(nums(&calc_matriz(&format!("TAKE({m};2;2)"))), vec![vec![1.0, 2.0], vec![4.0, 5.0]]);
        assert_eq!(nums(&calc_matriz(&format!("PEGAR({m};-1;-2)"))), vec![vec![8.0, 9.0]]);
        assert_eq!(nums(&calc_matriz(&format!("TAKE({m};1)"))), vec![vec![1.0, 2.0, 3.0]]);
        assert_eq!(nums(&calc_matriz(&format!("DROP({m};1;1)"))), vec![vec![5.0, 6.0], vec![8.0, 9.0]]);
        assert_eq!(nums(&calc_matriz(&format!("DESCARTAR({m};-2)"))), vec![vec![1.0, 2.0, 3.0]]);
        is_err(calc(&format!("TAKE({m};0)")), ErrorCode::Error);
        is_err(calc(&format!("DROP({m};3)")), ErrorCode::Error);
        is_err(calc("TAKE(1/0;1)"), ErrorCode::Div0);
    }

    #[test]
    fn escolher_colunas_e_linhas() {
        let m = "{1\\2\\3;4\\5\\6}";
        assert_eq!(nums(&calc_matriz(&format!("CHOOSECOLS({m};3;1)"))), vec![vec![3.0, 1.0], vec![6.0, 4.0]]);
        assert_eq!(nums(&calc_matriz(&format!("ESCOLHERCOLS({m};-1)"))), vec![vec![3.0], vec![6.0]]);
        assert_eq!(nums(&calc_matriz(&format!("CHOOSEROWS({m};2;1)"))), vec![vec![4.0, 5.0, 6.0], vec![1.0, 2.0, 3.0]]);
        assert_eq!(nums(&calc_matriz(&format!("ESCOLHERLINS({m};-1)"))), vec![vec![4.0, 5.0, 6.0]]);
        is_err(calc(&format!("CHOOSECOLS({m};4)")), ErrorCode::Value);
        is_err(calc(&format!("CHOOSEROWS({m};0)")), ErrorCode::Value);
    }

    #[test]
    fn para_coluna_e_linha() {
        let m = "{1\\2;3\\4}";
        assert_eq!(nums(&calc_matriz(&format!("TOCOL({m})"))), vec![vec![1.0], vec![2.0], vec![3.0], vec![4.0]]);
        assert_eq!(nums(&calc_matriz(&format!("PARACOL({m};0;VERDADEIRO)"))), vec![vec![1.0], vec![3.0], vec![2.0], vec![4.0]]);
        assert_eq!(nums(&calc_matriz(&format!("TOROW({m})"))), vec![vec![1.0, 2.0, 3.0, 4.0]]);
        assert_eq!(nums(&calc_matriz(&format!("PARALIN({m};0;TRUE)"))), vec![vec![1.0, 3.0, 2.0, 4.0]]);
        let c = vec![("A1".to_string(), n(1.0)), ("A3".to_string(), n(3.0)), ("A2".to_string(), Value::Error(e_nd()))];
        assert_eq!(mat(&c, "TOCOL(A1:A3;1)").len(), 3, "nenhuma célula vazia: nada sai");
        assert_eq!(mat(&c, "TOCOL(A1:A4;1)").len(), 3, "A4 é vazia e sai");
        assert_eq!(mat(&c, "TOCOL(A1:A3;2)").len(), 2, "o erro de A2 sai");
        assert_eq!(nums(&mat(&c, "TOCOL(A1:A3;3)")), vec![vec![1.0], vec![3.0]]);
        is_err(calc("TOCOL({1;2};7)"), ErrorCode::Value);
    }

    #[test]
    fn quebrar_vetor() {
        let v = "{1;2;3;4;5}";
        let r = calc_matriz(&format!("WRAPROWS({v};2)"));
        assert_eq!(r.len(), 3);
        assert_eq!(r[2][0], n(5.0));
        assert_eq!(r[2][1], Value::Error(e_nd()));
        assert_eq!(nums(&calc_matriz(&format!("QUEBRARLINS({v};2;0)")))[2], vec![5.0, 0.0]);
        let r = nums(&calc_matriz(&format!("WRAPCOLS({v};2;0)")));
        assert_eq!(r, vec![vec![1.0, 3.0, 5.0], vec![2.0, 4.0, 0.0]]);
        assert_eq!(nums(&calc_matriz("QUEBRARCOLS({1\\2\\3\\4};2)")), vec![vec![1.0, 3.0], vec![2.0, 4.0]]);
        is_err(calc(&format!("WRAPROWS({v};0)")), ErrorCode::Num);
        is_err(calc("WRAPROWS({1\\2;3\\4};2)"), ErrorCode::Value);
    }

    #[test]
    fn expandir() {
        let r = calc_matriz("EXPAND({1\\2};2;3;0)");
        assert_eq!(nums(&r), vec![vec![1.0, 2.0, 0.0], vec![0.0, 0.0, 0.0]]);
        let r = calc_matriz("EXPANDIR({1\\2};2)");
        assert_eq!(r[1][0], Value::Error(e_nd()));
        is_err(calc("EXPAND({1\\2;3\\4};1)"), ErrorCode::Value);
    }

    #[test]
    fn ordenar_por() {
        let c = [coluna('A', &[Value::texto("a"), Value::texto("b"), Value::texto("c")]), coluna('B', &[n(2.0), n(3.0), n(1.0)])].concat();
        assert_eq!(cal(&c, "INDEX(SORTBY(A1:A3;B1:B3);1)"), Value::texto("c"));
        let r = mat(&c, "CLASSIFICARPOR(A1:A3;B1:B3;-1)");
        assert_eq!(r, vec![vec![Value::texto("b")], vec![Value::texto("a")], vec![Value::texto("c")]]);
        let r = nums(&calc_matriz("SORTBY({1\\2\\3};{3\\1\\2})"));
        assert_eq!(r, vec![vec![2.0, 3.0, 1.0]]);
        is_err(cal(&c, "SORTBY(A1:A3;B1:B2)"), ErrorCode::Value);
        is_err(cal(&c, "SORTBY(A1:A3;B1:B3;2)"), ErrorCode::Value);
    }

    #[test]
    fn matriz_aleatoria() {
        let r = nums(&calc_matriz("RANDARRAY(3;2;5;10;TRUE)"));
        assert_eq!((r.len(), r[0].len()), (3, 2));
        assert!(r.iter().flatten().all(|v| (5.0..=10.0).contains(v) && v.fract() == 0.0));
        let r = nums(&calc_matriz("MATRIZALEATÓRIA(2;2)"));
        assert!(r.iter().flatten().all(|v| (0.0..1.0).contains(v)));
        is_err(calc("RANDARRAY(1;1;5;1)"), ErrorCode::Value);
        is_err(calc("RANDARRAY(0)"), ErrorCode::Value);
    }

    #[test]
    fn proc_vetor_e_matriz() {
        let c = [coluna('A', &[n(1.0), n(3.0), n(5.0)]), coluna('B', &[Value::texto("x"), Value::texto("y"), Value::texto("z")])].concat();
        assert_eq!(cal(&c, "LOOKUP(4;A1:A3;B1:B3)"), Value::texto("y"));
        assert_eq!(cal(&c, "PROC(5;A1:A3;B1:B3)"), Value::texto("z"));
        assert_eq!(cal(&c, "PROC(3;A1:B3)"), Value::texto("y"));
        is_err(cal(&c, "PROC(0;A1:A3;B1:B3)"), ErrorCode::Na);
        is_err(cal(&c, "PROC(\"a\";A1:A3;B1:B3)"), ErrorCode::Na);
    }

    #[test]
    fn corresp_x() {
        let c = coluna('A', &[Value::texto("Maçã"), Value::texto("pera"), Value::texto("uva"), Value::texto("pera")]);
        assert_eq!(cal(&c, "XMATCH(\"pera\";A1:A4)"), n(2.0));
        assert_eq!(cal(&c, "CORRESPX(\"PERA\";A1:A4;0;-1)"), n(4.0));
        assert_eq!(cal(&c, "XMATCH(\"u*\";A1:A4;2)"), n(3.0));
        let nv = coluna('A', &[n(10.0), n(20.0), n(30.0)]);
        assert_eq!(cal(&nv, "XMATCH(25;A1:A3;-1)"), n(2.0));
        assert_eq!(cal(&nv, "XMATCH(25;A1:A3;1)"), n(3.0));
        is_err(cal(&nv, "XMATCH(5;A1:A3)"), ErrorCode::Na);
        is_err(cal(&nv, "XMATCH(5;A1:A3;3)"), ErrorCode::Value);
        is_err(calc("XMATCH(1;{1\\2;3\\4})"), ErrorCode::Value);
    }

    #[test]
    fn endereco() {
        is_text(calc("ADDRESS(2;3)"), "$C$2");
        is_text(calc("ENDEREÇO(2;3;4)"), "C2");
        is_text(calc("ADDRESS(2;3;2)"), "C$2");
        is_text(calc("ADDRESS(2;3;3)"), "$C2");
        is_text(calc("ADDRESS(2;3;1;FALSE)"), "R2C3");
        is_text(calc("ADDRESS(2;3;4;FALSE)"), "R[2]C[3]");
        is_text(calc("ADDRESS(1;27;1;TRUE;\"Plan 1\")"), "'Plan 1'!$AA$1");
        is_text(calc("ADDRESS(1;1;1;TRUE;\"Dados\")"), "Dados!$A$1");
        is_err(calc("ADDRESS(0;1)"), ErrorCode::Value);
        is_err(calc("ADDRESS(1;1;5)"), ErrorCode::Value);
    }

    #[test]
    fn hiperlink_e_ref() {
        is_text(calc("HYPERLINK(\"http://x.com\";\"Abrir\")"), "Abrir");
        is_text(calc("HIPERLINK(\"http://x.com\")"), "http://x.com");
        is_err(calc("HYPERLINK()"), ErrorCode::Value);
        assert_eq!(calc("ISREF(A1)"), Value::Bool(true));
        assert_eq!(calc("ÉREF(A1:B2)"), Value::Bool(true));
        assert_eq!(calc("ISREF(5)"), Value::Bool(false));
        assert_eq!(calc("ISREF(\"A1\")"), Value::Bool(false));
    }

    fn dados_regressao() -> Vec<(String, Value)> {
        let y = [2.0, 3.0, 9.0, 1.0, 8.0, 7.0, 5.0].map(n);
        let x = [6.0, 5.0, 11.0, 7.0, 5.0, 4.0, 4.0].map(n);
        [coluna('A', &y), coluna('B', &x)].concat()
    }

    #[test]
    fn epadyx() {
        let c = dados_regressao();
        perto(cal(&c, "STEYX(A1:A7;B1:B7)"), 3.305719, 1e-6);
        perto(cal(&c, "EPADYX(A1:A7;B1:B7)"), 3.305719, 1e-6);
        is_err(cal(&c, "STEYX(A1:A7;B1:B3)"), ErrorCode::Na);
        is_err(cal(&c, "STEYX(A1:A2;B1:B2)"), ErrorCode::Div0);
    }

    #[test]
    fn prob() {
        let c = [coluna('A', &[0.0, 1.0, 2.0, 3.0].map(n)), coluna('B', &[0.2, 0.3, 0.1, 0.4].map(n))].concat();
        perto(cal(&c, "PROB(A1:A4;B1:B4;2)"), 0.1, 1e-12);
        perto(cal(&c, "PROB(A1:A4;B1:B4;1;3)"), 0.8, 1e-12);
        is_err(cal(&c, "PROB(A1:A4;B1:B3;2)"), ErrorCode::Na);
        let ruim = [coluna('A', &[0.0, 1.0].map(n)), coluna('B', &[0.5, 0.2].map(n))].concat();
        is_err(cal(&ruim, "PROB(A1:A2;B1:B2;0)"), ErrorCode::Num);
        is_err(cal(&c, "PROB(A1:A4;B1:B4;\"x\")"), ErrorCode::Value);
    }

    #[test]
    fn testes_f_e_z() {
        let c = [
            coluna('A', &[6.0, 7.0, 9.0, 15.0, 21.0].map(n)),
            coluna('B', &[20.0, 28.0, 31.0, 38.0, 40.0].map(n)),
        ]
        .concat();
        perto(cal(&c, "F.TEST(A1:A5;B1:B5)"), 0.64831785, 1e-6);
        perto(cal(&c, "TESTE.F(A1:A5;B1:B5)"), 0.64831785, 1e-6);
        perto(cal(&c, "FTEST(A1:A5;B1:B5)"), 0.64831785, 1e-6);
        is_err(cal(&c, "F.TEST(A1:A1;B1:B5)"), ErrorCode::Div0);

        let z = coluna('A', &[3.0, 6.0, 7.0, 8.0, 6.0, 5.0, 4.0, 2.0, 1.0, 9.0].map(n));
        perto(cal(&z, "Z.TEST(A1:A10;4)"), 0.090574, 1e-6);
        perto(cal(&z, "TESTE.Z(A1:A10;6)"), 0.863043, 1e-6);
        perto(cal(&z, "ZTEST(A1:A10;6;2)"), 1.0 - crate::funcoes::fn_dist::norm_cdf((5.1 - 6.0) / (2.0 / 10f64.sqrt())), 1e-12);
        is_err(cal(&z, "Z.TEST(A1:A10;4;0)"), ErrorCode::Num);
        is_err(cal(&z, "Z.TEST(A1:A10;\"x\")"), ErrorCode::Value);
    }

    #[test]
    fn tendencia_e_crescimento() {
        let c = [coluna('A', &[3.0, 5.0, 7.0, 9.0].map(n)), coluna('B', &[1.0, 2.0, 3.0, 4.0].map(n))].concat();
        perto(cal(&c, "TREND(A1:A4;B1:B4;5)"), 11.0, 1e-9);
        perto(cal(&c, "TENDÊNCIA(A1:A4;B1:B4;0)"), 1.0, 1e-9);
        let r = nums(&mat(&c, "TREND(A1:A4)"));
        assert_eq!(r.len(), 4);
        perto(Value::Number(r[3][0]), 9.0, 1e-9);
        let g = [coluna('A', &[2.0, 4.0, 8.0, 16.0].map(n)), coluna('B', &[1.0, 2.0, 3.0, 4.0].map(n))].concat();
        perto(cal(&g, "GROWTH(A1:A4;B1:B4;5)"), 32.0, 1e-9);
        perto(cal(&g, "CRESCIMENTO(A1:A4;B1:B4;6)"), 64.0, 1e-9);
        is_err(cal(&c, "TREND(A1:A4;B1:B3;5)"), ErrorCode::Ref);
        let neg = [coluna('A', &[2.0, -4.0].map(n)), coluna('B', &[1.0, 2.0].map(n))].concat();
        is_err(cal(&neg, "GROWTH(A1:A2;B1:B2;3)"), ErrorCode::Num);
        is_err(cal(&c, "TREND(A1:A4;B1:B4;\"x\")"), ErrorCode::Value);
    }

    #[test]
    fn funcoes_com_b() {
        assert_eq!(calc("LENB(\"casa\")"), n(4.0));
        assert_eq!(calc("NÚM.CARACTB(\"ação\")"), n(4.0));
        is_text(calc("LEFTB(\"abcdef\";2)"), "ab");
        is_text(calc("DIREITAB(\"abcdef\";2)"), "ef");
        is_text(calc("MIDB(\"abcdef\";2;3)"), "bcd");
        assert_eq!(calc("FINDB(\"c\";\"abcdef\")"), n(3.0));
        assert_eq!(calc("LOCALIZARB(\"C\";\"abcdef\")"), n(3.0));
        is_text(calc("MUDARB(\"abcdef\";1;2;\"XY\")"), "XYcdef");
        is_err(calc("FINDB(\"z\";\"abc\")"), ErrorCode::Value);
        is_err(calc("LENB()"), ErrorCode::Value);
    }

    #[test]
    fn codifica_url() {
        is_text(calc("ENCODEURL(\"http://x.com/a b\")"), "http%3A%2F%2Fx.com%2Fa%20b");
        is_text(calc("CODIFURL(\"ação\")"), "a%C3%A7%C3%A3o");
        is_text(calc("ENCODEURL(\"a-b_c.d~\")"), "a-b_c.d~");
        is_err(calc("ENCODEURL(1/0)"), ErrorCode::Div0);
    }
}
