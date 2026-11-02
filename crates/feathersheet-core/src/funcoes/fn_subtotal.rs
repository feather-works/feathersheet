//! `SUBTOTAL` e `AGGREGATE` (`AGREGAR`): agregam uma referência ignorando
//! linhas ocultas e outros subtotais aninhados, como o Excel.
//!
//! Precisam da REFERÊNCIA (não dos valores), por isso leem o `Node` direto.
//! Linha oculta = altura 0 (é como o filtro e o "ocultar" aparecem no motor);
//! o motor não distingue "oculta por filtro" de "oculta à mão", então 1–11
//! também ignoram linhas ocultas (o caso que importa: a tabela filtrada). A
//! exceção é a linha de um grupo da estrutura de tópicos: recolhida, ela é
//! "oculta à mão" como no Excel, e o 1–11 continua somando (o total do
//! Subtotal não some ao clicar no nível 2).
//! As opções (1–11, 101–111 e as de AGREGAR) seguem a documentação do Excel.

use crate::celulas::refs::Rect;
use crate::celulas::values::{Computed, ErrorCode, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;

fn erro(c: ErrorCode) -> Computed {
    Computed::Scalar(Value::erro(c))
}

/// `=SUBTOTAL(função; ref1; ...)`
pub fn fn_subtotal(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 2 {
        return erro(ErrorCode::Value);
    }
    let code = match ev.number(&args[0]) {
        Ok(n) => n.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let func = if code > 100 { code - 100 } else { code };
    if !(1..=11).contains(&func) {
        return erro(ErrorCode::Value);
    }
    // 1–11: só filtradas; 101–111: também as ocultas à mão. O motor só
    // separa as recolhidas da estrutura de tópicos (ver nota do módulo).
    // Subtotais aninhados saem sempre.
    let ocultas = if code > 100 { Ocultas::Todas } else { Ocultas::ForaDeTopico };
    executar(func, ocultas, false, true, &args[1..], None, ev)
}

/// `SUBTOTAL(3; DESLOC(faixa; LIN(faixa)-LIN(início); 0; 1))`: com uma matriz no
/// deslocamento, o DESLOC dá uma referência por item e o SUBTOTAL, um resultado por item —
/// é o jeito do Excel de contar só as linhas visíveis dentro do `SOMARPRODUTO`. `None` quando
/// a chamada não é desse formato (segue o caminho comum).
pub fn e_desloc(args: &[Node]) -> bool {
    matches!(args, [_, Node::Call { name, args: dentro }]
        if dentro.len() >= 3 && crate::funcoes::functions::canonical(name).unwrap_or(name).eq_ignore_ascii_case("OFFSET"))
}

pub fn desloc_por_item(args: &[Node], ev: &Evaluator<'_>) -> Option<Computed> {
    if !e_desloc(args) { return None; }
    let [codigo, Node::Call { name, args: dentro }] = args else { return None };
    let (i, m) = dentro.iter().enumerate().skip(1).find_map(|(i, a)| match ev.eval_array(a) {
        Computed::Matrix(m) if m.size() > 1 => Some((i, m)),
        _ => None,
    })?;
    let code = match ev.number(codigo) {
        Ok(n) => n.trunc() as i64,
        Err(e) => return Some(Computed::Scalar(Value::Error(e))),
    };
    let func = if code > 100 { code - 100 } else { code };
    if !(1..=11).contains(&func) {
        return Some(erro(ErrorCode::Value));
    }
    let ocultas = if code > 100 { Ocultas::Todas } else { Ocultas::ForaDeTopico };
    let mut novos = dentro.clone();
    let linhas = m.rows().iter().map(|linha| linha.iter().map(|v| {
        let Value::Number(n) = v else { return Value::erro(ErrorCode::Value) };
        novos[i] = Node::Num(*n);
        match crate::funcoes::fn_referencia::referencia(name, &novos, ev) {
            Ok(no) => match executar(func, ocultas, false, true, std::slice::from_ref(&no), None, ev) {
                Computed::Scalar(v) => v,
                _ => Value::erro(ErrorCode::Value),
            },
            Err(e) => Value::Error(e),
        }
    }).collect()).collect();
    Some(Computed::Matrix(crate::celulas::values::Matrix::new(linhas)))
}

/// `=AGGREGATE(função; opções; ref1; ...)` / `=AGREGAR(...)`
pub fn fn_aggregate(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 3 {
        return erro(ErrorCode::Value);
    }
    let func = match ev.number(&args[0]) {
        Ok(n) => n.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let opcoes = match ev.number(&args[1]) {
        Ok(n) => n.trunc() as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if !(1..=19).contains(&func) || !(0..=7).contains(&opcoes) {
        return erro(ErrorCode::Value);
    }
    let ocultas = if matches!(opcoes, 1 | 3 | 5 | 7) { Ocultas::Todas } else { Ocultas::Nenhuma };
    let erros = matches!(opcoes, 2 | 3 | 6 | 7);
    let aninhados = opcoes <= 3;
    let (refs, k) = if func >= 14 {
        match args.get(3) {
            None => return erro(ErrorCode::Value),
            Some(k) => match ev.number(k) {
                Ok(k) => (&args[2..3], Some(k)),
                Err(e) => return Computed::Scalar(Value::Error(e)),
            },
        }
    } else {
        (&args[2..], None)
    };
    executar(func, ocultas, erros, aninhados, refs, k, ev)
}

/// Quais linhas ocultas ficam de fora.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ocultas {
    Nenhuma,
    Todas,
    /// As ocultas, menos as recolhidas de um grupo (`SUBTOTAL` 1–11).
    ForaDeTopico,
}

fn executar(
    func: i64, ocultas: Ocultas, erros: bool, aninhados: bool,
    refs: &[Node], k: Option<f64>, ev: &Evaluator<'_>,
) -> Computed {
    let mut valores: Vec<Value> = Vec::new();
    for no in refs {
        let rect = match no {
            Node::Range { .. } | Node::Ref { .. } | Node::Name(_) => match ev.rect(no) {
                Ok(r) => Some(r),
                Err(e) => return Computed::Scalar(Value::Error(e)),
            },
            _ => None,
        };
        match rect {
            Some(rect) => coletar_ref(&rect, ocultas, aninhados, ev, &mut valores),
            None => match ev.matrix_arg(no) {
                Ok(m) => valores.extend(m.values().filter(|v| !v.is_empty()).cloned()),
                Err(e) => {
                    if !erros { return Computed::Scalar(Value::Error(e)); }
                }
            },
        }
    }
    if erros {
        valores.retain(|v| !matches!(v, Value::Error(_)));
    } else if func != 2 && func != 3 {
        if let Some(Value::Error(e)) = valores.iter().find(|v| matches!(v, Value::Error(_))) {
            return Computed::Scalar(Value::Error(e.clone()));
        }
    }
    calcular(func, &valores, k)
}

fn coletar_ref(rect: &Rect, ocultas: Ocultas, aninhados: bool, ev: &Evaluator<'_>, saida: &mut Vec<Value>) {
    let sheet = rect.sheet.as_deref();
    let mut achados: Vec<(u32, u32, Value)> = Vec::new();
    ev.ctx.for_each_in_range(rect, &mut |v, r, c| achados.push((r, c, v.clone())));
    achados.sort_by_key(|(r, c, _)| (*r, *c));
    achados.dedup_by_key(|(r, c, _)| (*r, *c));
    let mut linha_ok: Option<(u32, bool)> = None;
    for (r, c, v) in achados {
        if ocultas != Ocultas::Nenhuma {
            let visivel = match linha_ok {
                Some((lr, ok)) if lr == r => ok,
                _ => {
                    let ok = !ev.ctx.linha_oculta(r, sheet)
                        || (ocultas == Ocultas::ForaDeTopico && ev.ctx.linha_no_topico(r, sheet));
                    linha_ok = Some((r, ok));
                    ok
                }
            };
            if !visivel { continue; }
        }
        if aninhados && ev.ctx.celula_com_subtotal(r, c, sheet) { continue; }
        saida.push(v);
    }
}

fn numeros(valores: &[Value]) -> Vec<f64> {
    valores.iter().filter_map(|v| if let Value::Number(n) = v { Some(*n) } else { None }).collect()
}

fn num(n: f64) -> Computed {
    Computed::Scalar(Value::Number(n))
}

fn variancia(ns: &[f64], amostral: bool) -> Option<f64> {
    let n = ns.len();
    if n == 0 || (amostral && n < 2) { return None; }
    let m = ns.iter().sum::<f64>() / n as f64;
    let s: f64 = ns.iter().map(|x| (x - m).powi(2)).sum();
    Some(s / if amostral { (n - 1) as f64 } else { n as f64 })
}

fn percentil(ns: &mut [f64], p: f64, exclusivo: bool) -> Computed {
    ns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = ns.len();
    if n == 0 || !(0.0..=1.0).contains(&p) { return erro(ErrorCode::Num); }
    let pos = if exclusivo {
        let pos = p * (n as f64 + 1.0);
        if pos < 1.0 || pos > n as f64 { return erro(ErrorCode::Num); }
        pos - 1.0
    } else {
        p * (n as f64 - 1.0)
    };
    let i = pos.floor() as usize;
    let f = pos - i as f64;
    if i + 1 >= n { return num(ns[n - 1]); }
    num(ns[i] + f * (ns[i + 1] - ns[i]))
}

fn calcular(func: i64, valores: &[Value], k: Option<f64>) -> Computed {
    if func == 3 {
        return num(valores.iter().filter(|v| !v.is_empty()).count() as f64);
    }
    let mut ns = numeros(valores);
    let n = ns.len();
    let div0 = || erro(ErrorCode::Div0);
    match func {
        1 => if n == 0 { div0() } else { num(ns.iter().sum::<f64>() / n as f64) },
        2 => num(n as f64),
        4 => num(ns.iter().cloned().fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.max(x)))).unwrap_or(0.0)),
        5 => num(ns.iter().cloned().fold(None, |a: Option<f64>, x| Some(a.map_or(x, |a| a.min(x)))).unwrap_or(0.0)),
        6 => num(if n == 0 { 0.0 } else { ns.iter().product() }),
        7 => variancia(&ns, true).map_or_else(div0, |v| num(v.sqrt())),
        8 => variancia(&ns, false).map_or_else(div0, |v| num(v.sqrt())),
        // `+ 0.0`: a soma vazia do Rust é -0, que apareceria como "-0".
        9 => num(ns.iter().sum::<f64>() + 0.0),
        10 => variancia(&ns, true).map_or_else(div0, num),
        11 => variancia(&ns, false).map_or_else(div0, num),
        12 => {
            if n == 0 { return erro(ErrorCode::Num); }
            ns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            num(if n % 2 == 0 { (ns[n / 2 - 1] + ns[n / 2]) / 2.0 } else { ns[n / 2] })
        }
        13 => {
            let mut melhor: Option<(f64, usize)> = None;
            for (i, x) in ns.iter().enumerate() {
                let c = ns.iter().filter(|y| *y == x).count();
                if c > 1 && melhor.is_none_or(|(_, mc)| c > mc) { melhor = Some((ns[i], c)); }
            }
            melhor.map_or_else(|| erro(ErrorCode::Na), |(v, _)| num(v))
        }
        14 | 15 => {
            let k = k.unwrap_or(0.0).trunc();
            if k < 1.0 || k as usize > n { return erro(ErrorCode::Num); }
            ns.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let i = k as usize - 1;
            num(if func == 14 { ns[n - 1 - i] } else { ns[i] })
        }
        16 | 18 => percentil(&mut ns, k.unwrap_or(0.0), func == 18),
        17 | 19 => {
            let q = k.unwrap_or(-1.0).trunc();
            let (min, max) = if func == 17 { (0.0, 4.0) } else { (1.0, 3.0) };
            if q < min || q > max { return erro(ErrorCode::Num); }
            percentil(&mut ns, q / 4.0, func == 19)
        }
        _ => erro(ErrorCode::Value),
    }
}


#[cfg(test)]
mod testes {
    use crate::celulas::values::Value;
    use crate::planilha::sheet::Sheet;

    fn aba() -> Sheet {
        let mut s = Sheet::new("Aba1");
        for (i, v) in ["10", "20", "30", "40"].iter().enumerate() {
            s.set_cell_raw(i as u32, 0, (*v).into(), None);
        }
        s.row_heights.insert(1, 0); // A2 oculta
        s
    }

    fn num(s: &mut Sheet, f: &str, r: u32) -> Value {
        s.set_cell_raw(r, 3, f.into(), None);
        s.get_value(r, 3)
    }

    #[test]
    fn subtotal_9_e_109_ignoram_linha_oculta() {
        let mut s = aba();
        assert_eq!(num(&mut s, "=SUBTOTAL(9;A1:A4)", 9), Value::Number(80.0));
        assert_eq!(num(&mut s, "=SUBTOTAL(109;A1:A4)", 9), Value::Number(80.0));
        assert_eq!(num(&mut s, "=SUBTOTAL(103;A1:A4)", 9), Value::Number(3.0));
        assert_eq!(num(&mut s, "=SUM(A1:A4)", 9), Value::Number(100.0));
    }

    #[test]
    fn subtotal_de_desloc_com_matriz_da_um_por_linha() {
        // Contar só as linhas visíveis que batem com o critério (A2 = 20 está oculta).
        let mut s = aba();
        let f = "=SOMARPRODUTO(SUBTOTAL(3;DESLOC(A1:A4;LIN(A1:A4)-LIN(A1);0;1));--(A1:A4>15))";
        assert_eq!(num(&mut s, f, 9), Value::Number(2.0));
        assert_eq!(num(&mut s, "=SOMARPRODUTO(SUBTOTAL(9;DESLOC(A1;LIN(A1:A4)-1;0)))", 9), Value::Number(80.0));
        // Sem matriz no DESLOC segue o caminho comum.
        assert_eq!(num(&mut s, "=SUBTOTAL(9;DESLOC(A1;0;0;4))", 9), Value::Number(80.0));
    }

    #[test]
    fn subtotal_aninhado_nao_conta_duas_vezes() {
        let mut s = aba();
        s.set_cell_raw(4, 0, "=SUBTOTAL(9;A1:A4)".into(), None);
        s.set_cell_raw(5, 0, "=SUBTOTAL(9;A1:A5)".into(), None);
        assert_eq!(s.get_value(5, 0), Value::Number(80.0));
    }

    #[test]
    fn agregar_ignora_erros_e_muda_com_a_visibilidade() {
        let mut s = aba();
        s.set_cell_raw(2, 0, "=1/0".into(), None);
        s.set_cell_raw(9, 0, "=AGREGAR(9;6;A1:A4)".into(), None);
        assert_eq!(s.get_value(9, 0), Value::Number(70.0));
        s.set_cell_raw(10, 0, "=AGREGAR(9;7;A1:A4)".into(), None);
        assert_eq!(s.get_value(10, 0), Value::Number(50.0));
        s.set_cell_raw(11, 0, "=AGREGAR(9;4;A1:A4)".into(), None);
        assert!(matches!(s.get_value(11, 0), Value::Error(_)));
        s.set_cell_raw(12, 0, "=AGREGAR(14;6;A1:A4;1)".into(), None);
        assert_eq!(s.get_value(12, 0), Value::Number(40.0));
        // Ocultar depois: a fórmula (volátil) enxerga a mudança.
        s.row_heights.insert(3, 0);
        assert_eq!(s.get_value(10, 0), Value::Number(10.0));
    }
}
