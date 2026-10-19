//! Funções de lógica e informação em Rust: VERDADEIRO, FALSO, XOR, SEERRO, SENÃODISP, SES, PARÂMETRO, ESCOLHER, NÃODISP, ÉCÉL, ÉERRO, ÉERR, É.NÃO.DISP, ÉLÓGICO, É.NÃO.TEXTO, TIPO.ERRO, TIPO, N, ÉNÚM, ÉTEXTO, ÉPAR, ÉÍMPAR, LINS, COLS.

use crate::funcoes::fn_helpers::node_rect;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{arity, err_na, err_value, has};
use crate::celulas::values::{to_text, CellError, Computed, ErrorCode, Value};


/// `=VERDADEIRO()` / `=TRUE()`
pub fn fn_true(_args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    Computed::Scalar(Value::Bool(true))
}

/// `=FALSO()` / `=FALSE()`
pub fn fn_false(_args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    Computed::Scalar(Value::Bool(false))
}

/// `=XOR(args...)`
pub fn fn_xor(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.is_empty() {
        return err_value();
    }
    let mut true_count = 0;
    for a in args {
        let mut err: Option<CellError> = None;
        let res = ev.for_each_value(a, &mut |v, _| {
            match v {
                Value::Error(e) => {
                    if err.is_none() {
                        err = Some(e.clone());
                    }
                }
                Value::Bool(b) => {
                    if *b {
                        true_count += 1;
                    }
                }
                Value::Number(n) => {
                    if *n != 0.0 {
                        true_count += 1;
                    }
                }
                _ => {}
            }
        });
        if let Some(e) = res {
            return Computed::Scalar(Value::Error(e));
        }
        if let Some(e) = err {
            return Computed::Scalar(Value::Error(e));
        }
    }
    Computed::Scalar(Value::Bool(true_count % 2 == 1))
}

/// `=SENÃODISP(val; alt)` / `=IFNA(...)`
pub fn fn_ifna(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    if let Value::Error(ref e) = v {
        if e.code == ErrorCode::Na {
            return Computed::Scalar(ev.eval(&args[1]));
        }
    }
    Computed::Scalar(v)
}

/// `=SES(cond1; val1; [cond2; val2; ...])` / `=IFS(...)`
pub fn fn_ifs(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 2 || args.len() % 2 != 0 {
        return err_value();
    }
    for i in (0..args.len()).step_by(2) {
        let cond = ev.boolean(&args[i]);
        match cond {
            Ok(b) => {
                if b {
                    return Computed::Scalar(ev.eval(&args[i + 1]));
                }
            }
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    }
    err_na()
}

/// `=PARÂMETRO(expressao; caso1; valor1; ...; [padrao])` / `=SWITCH(...)`
pub fn fn_switch(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 3 {
        return err_value();
    }
    let target = ev.value(&args[0]);
    if let Value::Error(e) = target {
        return Computed::Scalar(Value::Error(e));
    }
    // Como no Excel: texto sem ligar para maiúscula, e número só casa com
    // número (`PARÂMETRO(1; "1"; …)` não casa). Antes comparava o texto dos
    // dois lados, e era o contrário nos dois casos (17/10/2026).
    let mut i = 1;
    while i < args.len() - 1 {
        let case_val = ev.value(&args[i]);
        if crate::funcoes::fn_helpers::valores_iguais(&target, &case_val) {
            return Computed::Scalar(ev.eval(&args[i + 1]));
        }
        i += 2;
    }
    if i < args.len() {
        return Computed::Scalar(ev.eval(&args[i]));
    }
    err_na()
}

/// `=ESCOLHER(posicao; valor1; valor2; ...)` / `=CHOOSE(...)`
pub fn fn_choose(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 2 {
        return err_value();
    }
    let idx = match ev.number(&args[0]) {
        Ok(n) => n.trunc() as usize,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if idx < 1 || idx >= args.len() {
        return err_value();
    }
    Computed::Scalar(ev.eval(&args[idx]))
}

/// `=NÃODISP()` / `=NA()`
pub fn fn_na(_args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    err_na()
}

/// `=ÉCÉL(valor)` / `=ISBLANK(...)`
pub fn fn_isblank(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(matches!(v, Value::Empty)))
}

/// `=ÉERRO(valor)` / `=ISERROR(...)`
pub fn fn_iserror(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(matches!(v, Value::Error(_))))
}

/// `=ÉNÚM(valor)` / `=ISNUMBER(...)`
pub fn fn_isnumber(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(matches!(v, Value::Number(_))))
}

/// `=ÉTEXTO(valor)` / `=ISTEXT(...)`
pub fn fn_istext(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(matches!(v, Value::Text(_))))
}

/// `=ÉPAR(numero)` / `=ISEVEN(...)`
pub fn fn_iseven(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => (v.trunc() as i64).abs(),
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    Computed::Scalar(Value::Bool(n % 2 == 0))
}

/// `=ÉÍMPAR(numero)` / `=ISODD(...)`
pub fn fn_isodd(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let n = match ev.number(&args[0]) {
        Ok(v) => (v.trunc() as i64).abs(),
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    Computed::Scalar(Value::Bool(n % 2 == 1))
}

/// `=LINS(matriz)` / `=ROWS(...)`
pub fn fn_rows(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if !has(args, 0) {
        return err_value();
    }
    if let Some(rect) = node_rect(&args[0], ev) {
        Computed::Scalar(Value::Number((rect.r1 - rect.r0 + 1) as f64))
    } else {
        Computed::Scalar(Value::Number(1.0))
    }
}

/// `=COLS(matriz)` / `=COLUMNS(...)`
pub fn fn_columns(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if !has(args, 0) {
        return err_value();
    }
    if let Some(rect) = node_rect(&args[0], ev) {
        Computed::Scalar(Value::Number((rect.c1 - rect.c0 + 1) as f64))
    } else {
        Computed::Scalar(Value::Number(1.0))
    }
}

/// `=LIN([referência])` / `=ROW([reference])`
pub fn fn_row(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if !has(args, 0) {
        if let Some((r, _)) = ev.here {
            Computed::Scalar(Value::Number((r + 1) as f64))
        } else {
            err_value()
        }
    } else if let Some(rect) = node_rect(&args[0], ev) {
        if rect.r0 == rect.r1 {
            Computed::Scalar(Value::Number((rect.r0 + 1) as f64))
        } else {
            let rows: Vec<Vec<Value>> = (rect.r0..=rect.r1)
                .map(|r| vec![Value::Number((r + 1) as f64)])
                .collect();
            Computed::Matrix(crate::celulas::values::Matrix::new(rows))
        }
    } else {
        let val = ev.eval(&args[0]);
        match val {
            Value::Error(e) => Computed::Scalar(Value::Error(e)),
            _ => {
                if let Some((r, _)) = ev.here {
                    Computed::Scalar(Value::Number((r + 1) as f64))
                } else {
                    Computed::Scalar(Value::Number(1.0))
                }
            }
        }
    }
}

/// `=COL([referência])` / `=COLUMN([reference])`
pub fn fn_column(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if !has(args, 0) {
        if let Some((_, c)) = ev.here {
            Computed::Scalar(Value::Number((c + 1) as f64))
        } else {
            err_value()
        }
    } else if let Some(rect) = node_rect(&args[0], ev) {
        if rect.c0 == rect.c1 {
            Computed::Scalar(Value::Number((rect.c0 + 1) as f64))
        } else {
            let cols: Vec<Value> = (rect.c0..=rect.c1)
                .map(|c| Value::Number((c + 1) as f64))
                .collect();
            Computed::Matrix(crate::celulas::values::Matrix::new(vec![cols]))
        }
    } else {
        let val = ev.eval(&args[0]);
        match val {
            Value::Error(e) => Computed::Scalar(Value::Error(e)),
            _ => {
                if let Some((_, c)) = ev.here {
                    Computed::Scalar(Value::Number((c + 1) as f64))
                } else {
                    Computed::Scalar(Value::Number(1.0))
                }
            }
        }
    }
}

// ------------------------------------------------------------------- web
//
// Tradução do `fn-web.ts`. O motor confere os argumentos e faz a pergunta
// (`Evaluator::async_value`); quem busca na rede é a interface, que devolve
// o texto cru, e o `sheet-io/web.rs` o interpreta pela chave. As chaves são
// as do TypeScript: `web:texto:URL`, `web:dados:URL` + NUL + separador,
// `web:json:URL` + NUL + caminho.

fn url_valida(bruto: &str) -> Result<String, CellError> {
    let texto = bruto.trim();
    let minusculo = texto.to_lowercase();
    if !(minusculo.starts_with("http://") || minusculo.starts_with("https://")) {
        return Err(CellError::com_motivo(ErrorCode::Value, "O endereço precisa começar com http:// ou https://"));
    }
    Ok(texto.to_string())
}

fn url_do_argumento(args: &[Node], ev: &Evaluator<'_>) -> Result<String, CellError> {
    match ev.eval(&args[0]) {
        Value::Error(e) => Err(e),
        v => url_valida(&to_text(&v)),
    }
}

fn perguntar(ev: &Evaluator<'_>, chave: String) -> Computed {
    ev.async_value(&chave, Box::new(|| Computed::Scalar(Value::Empty)))
}

/// `=BUSCARTEXTO(url)` / `=FETCHTEXT(url)`
pub fn fn_fetchtext(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(e) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(e));
    }
    match url_do_argumento(args, ev) {
        Ok(url) => perguntar(ev, format!("web:texto:{url}")),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=IMPORTARDADOS(url; [separador])` / `=IMPORTDATA(...)`
pub fn fn_importdata(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(e) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(e));
    }
    let url = match url_do_argumento(args, ev) {
        Ok(u) => u,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let mut separador = String::new();
    if has(args, 1) {
        match ev.eval(&args[1]) {
            Value::Error(e) => return Computed::Scalar(Value::Error(e)),
            v => separador = to_text(&v),
        }
        if separador.chars().count() != 1 {
            return Computed::Scalar(Value::Error(CellError::com_motivo(
                ErrorCode::Value,
                "O separador é um caractere só (\";\" ou \",\")",
            )));
        }
    }
    perguntar(ev, format!("web:dados:{url}\u{0}{separador}"))
}

/// `=IMPORTARJSON(url; [caminho])` / `=IMPORTJSON(...)`
pub fn fn_importjson(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(e) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(e));
    }
    let url = match url_do_argumento(args, ev) {
        Ok(u) => u,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let mut caminho = String::new();
    if has(args, 1) {
        match ev.eval(&args[1]) {
            Value::Error(e) => return Computed::Scalar(Value::Error(e)),
            v => caminho = to_text(&v),
        }
    }
    perguntar(ev, format!("web:json:{url}\u{0}{caminho}"))
}

/// `=ÉERR(valor)` / `=ISERR(...)` — erro que não seja `#N/D`.
pub fn fn_iserr(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(matches!(&v, Value::Error(e) if e.code != ErrorCode::Na)))
}

/// `=É.NÃO.DISP(valor)` / `=ISNA(...)`
pub fn fn_isna(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(matches!(&v, Value::Error(e) if e.code == ErrorCode::Na)))
}

/// `=ÉLÓGICO(valor)` / `=ISLOGICAL(...)`
pub fn fn_islogical(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(matches!(v, Value::Bool(_))))
}

/// `=É.NÃO.TEXTO(valor)` / `=ISNONTEXT(...)` — célula vazia também conta.
pub fn fn_isnontext(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = ev.value(&args[0]);
    Computed::Scalar(Value::Bool(!matches!(v, Value::Text(_))))
}

/// Número do Excel para cada erro (`TIPO.ERRO`). Os que o Excel não tem
/// (`#ERRO!`, referência circular) ficam sem número.
fn numero_do_erro(code: ErrorCode) -> Option<f64> {
    Some(match code {
        ErrorCode::Div0 => 2.0,
        ErrorCode::Value => 3.0,
        ErrorCode::Ref => 4.0,
        ErrorCode::Name => 5.0,
        ErrorCode::Num => 6.0,
        ErrorCode::Na => 7.0,
        ErrorCode::Calculating => 8.0,
        ErrorCode::Spill => 9.0,
        ErrorCode::Error | ErrorCode::Circular => return None,
    })
}

/// `=TIPO.ERRO(erro)` / `=ERROR.TYPE(...)` — 1 a 9 conforme o erro; valor
/// que não é erro dá `#N/D`.
pub fn fn_error_type(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    match ev.value(&args[0]) {
        Value::Error(e) => match numero_do_erro(e.code) {
            Some(n) => Computed::Scalar(Value::Number(n)),
            None => err_na(),
        },
        _ => err_na(),
    }
}

/// `=TIPO(valor)` / `=TYPE(...)` — 1 número (e vazio), 2 texto, 4 lógico,
/// 16 erro, 64 matriz ou intervalo de várias células, 128 função.
pub fn fn_type(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let tipo = match ev.eval_array(&args[0]) {
        Computed::Lambda(_) => 128.0,
        // Matriz escrita ou calculada é sempre 64; intervalo de uma célula só
        // vale pelo que tem dentro.
        Computed::Matrix(m)
            if m.size() > 1 || !matches!(&args[0], Node::Ref { .. } | Node::Range { .. } | Node::Name(_)) =>
        {
            64.0
        }
        outro => match outro.scalar() {
            Value::Empty | Value::Number(_) => 1.0,
            Value::Text(_) => 2.0,
            Value::Bool(_) => 4.0,
            Value::Error(_) => 16.0,
        },
    };
    Computed::Scalar(Value::Number(tipo))
}

/// `=N(valor)` — número fica, VERDADEIRO vale 1, texto e vazio valem 0, erro
/// passa adiante.
pub fn fn_n(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let v = match ev.value(&args[0]) {
        Value::Number(n) => Value::Number(n),
        Value::Bool(b) => Value::Number(if b { 1.0 } else { 0.0 }),
        Value::Error(e) => Value::Error(e),
        Value::Text(_) | Value::Empty => Value::Number(0.0),
    };
    Computed::Scalar(v)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::planilha::sheet::Sheet;

    #[test]
    fn endereco_web_tambem_e_validado_no_motor_rust() {
        assert_eq!(
            url_valida("  HTTPS://exemplo.test/dados.csv  "),
            Ok("HTTPS://exemplo.test/dados.csv".into())
        );
        for invalida in ["file:///etc/passwd", "data:text/plain,abc", "ftp://exemplo.test", "caminho/local.csv"] {
            let erro = url_valida(invalida).expect_err("apenas http(s) e permitido");
            assert_eq!(erro.code, ErrorCode::Value);
        }
    }

    #[test]
    fn test_funcoes_logica_e_info() {
        let mut sheet = Sheet::new("Plan1");

        // VERDADEIRO e FALSO
        sheet.set_cell_raw(0, 0, "=VERDADEIRO()".into(), None);
        assert_eq!(sheet.get_value(0, 0), Value::Bool(true));

        sheet.set_cell_raw(1, 0, "=FALSO()".into(), None);
        assert_eq!(sheet.get_value(1, 0), Value::Bool(false));

        // SES(1>2; "A"; 3>2; "B") = "B"
        sheet.set_cell_raw(2, 0, "=SES(1>2; \"A\"; 3>2; \"B\")".into(), None);
        assert_eq!(sheet.get_value(2, 0), Value::Text("B".into()));

        // PARÂMETRO / SWITCH: caso, padrão, sem padrão, maiúsculas, tipo.
        for (formula, esperado) in [
            ("=PARÂMETRO(2; 1; \"um\"; 2; \"dois\"; \"outro\")", Value::Text("dois".into())),
            ("=PARÂMETRO(9; 1; \"um\"; 2; \"dois\"; \"outro\")", Value::Text("outro".into())),
            ("=SWITCH(\"b\"; \"A\"; 1; \"B\"; 2)", Value::Number(2.0)),
            ("=PARÂMETRO(1; \"1\"; \"texto\"; \"número\")", Value::Text("número".into())),
        ] {
            sheet.set_cell_raw(10, 5, formula.into(), None);
            assert_eq!(sheet.get_value(10, 5), esperado, "{formula}");
        }
        sheet.set_cell_raw(10, 5, "=PARÂMETRO(9; 1; \"um\")".into(), None);
        assert!(matches!(sheet.get_value(10, 5), Value::Error(e) if e.code == ErrorCode::Na));
        sheet.set_cell_raw(10, 5, String::new(), None);

        // ESCOLHER(2; "X"; "Y"; "Z") = "Y"
        sheet.set_cell_raw(3, 0, "=ESCOLHER(2; \"X\"; \"Y\"; \"Z\")".into(), None);
        assert_eq!(sheet.get_value(3, 0), Value::Text("Y".into()));

        // ÉPAR(4) = true, ÉÍMPAR(4) = false
        sheet.set_cell_raw(4, 0, "=ÉPAR(4)".into(), None);
        assert_eq!(sheet.get_value(4, 0), Value::Bool(true));

        sheet.set_cell_raw(5, 0, "=ÉÍMPAR(4)".into(), None);
        assert_eq!(sheet.get_value(5, 0), Value::Bool(false));

        // LINS(A1:A5) = 5
        sheet.set_cell_raw(6, 0, "=LINS(A1:A5)".into(), None);
        assert_eq!(sheet.get_value(6, 0), Value::Number(5.0));

        // COLS(A1:C5) = 3
        sheet.set_cell_raw(7, 0, "=COLS(A1:C5)".into(), None);
        assert_eq!(sheet.get_value(7, 0), Value::Number(3.0));
    }
}
