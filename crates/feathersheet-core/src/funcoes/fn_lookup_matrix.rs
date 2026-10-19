//! Álgebra de matrizes em Rust: MATRIZ.MULT, INVERSO, DETERM.

use crate::funcoes::fn_helpers::node_rect;
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{arity, err_value};
use crate::celulas::values::{to_number, CellError, Computed, ErrorCode, Matrix, Value};


/// A matriz de números de um argumento — o `numberMatrix` do TypeScript:
/// intervalo, matriz escrita na fórmula (`{1\\2;3\\4}`) ou o resultado de
/// outra conta, cada item pelo `to_number` (vazio é 0, "10" é 10, texto que
/// não é número dá `#VALUE!`). Antes só intervalo passava, e a matriz
/// escrita dava `#VALUE!` em `MATRIZ.MULT`, `MATRIZ.INVERSO` e `MATRIZ.DETERM`.
fn matriz_de_numeros(node: &Node, ev: &Evaluator<'_>) -> Result<Vec<Vec<f64>>, Computed> {
    let valores = extract_value_matrix(node, ev)?;
    valores
        .iter()
        .map(|linha| {
            linha
                .iter()
                .map(|v| to_number(v).map_err(|e| Computed::Scalar(Value::Error(e))))
                .collect::<Result<Vec<f64>, Computed>>()
        })
        .collect()
}

/// `=MATRIZ.MULT(matriz1; matriz2)` / `=MMULT(...)`
pub fn fn_mmult(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let m1 = match matriz_de_numeros(&args[0], ev) { Ok(m) => m, Err(e) => return e };
    let m2 = match matriz_de_numeros(&args[1], ev) { Ok(m) => m, Err(e) => return e };

    let r1 = m1.len();
    let c1 = m1.first().map_or(0, Vec::len);
    let r2 = m2.len();
    let c2 = m2.first().map_or(0, Vec::len);

    if r1 == 0 || r2 == 0 || c1 == 0 || c2 == 0
        || m1.iter().any(|row| row.len() != c1)
        || m2.iter().any(|row| row.len() != c2)
        || c1 != r2
    {
        // A frase é a do `fn-lookup-matrix.ts`: é por ela que o "Por que deu
        // erro?" sabe dizer que são as dimensões (`formula/explicar.rs`).
        return Computed::Scalar(Value::Error(CellError::com_motivo(ErrorCode::Value, "As dimensões não se encaixam")));
    }
    // O mesmo teto do TypeScript para o resultado.
    if r1 * c2 > 200_000 {
        return crate::funcoes::fn_helpers::err_num();
    }

    let mut res = vec![vec![Value::Empty; c2]; r1];
    for i in 0..r1 {
        for j in 0..c2 {
            let mut sum = 0.0;
            for k in 0..c1 {
                sum += m1[i][k] * m2[k][j];
            }
            res[i][j] = Value::Number(sum);
        }
    }

    Computed::Matrix(Matrix::new(res))
}

/// `=DETERM(matriz)` / `=MDETERM(...)`
pub fn fn_mdeterm(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let m = match matriz_de_numeros(&args[0], ev) { Ok(m) => m, Err(e) => return e };
    let n = m.len();
    if n == 0 || m.iter().any(|row| row.len() != n) {
        return err_value();
    }

    let det = determinant(&m);
    Computed::Scalar(Value::Number(det))
}

/// Eliminação gaussiana com pivoteamento parcial (O(n³)), portada do TS.
fn determinant(matrix: &[Vec<f64>]) -> f64 {
    let n = matrix.len();
    let mut m = matrix.to_vec();
    let mut det = 1.0;
    for i in 0..n {
        let mut pivot = i;
        let mut max = m[i][i].abs();
        for r in (i + 1)..n {
            let val = m[r][i].abs();
            if val > max {
                max = val;
                pivot = r;
            }
        }
        if max < 1e-12 {
            return 0.0;
        }
        if pivot != i {
            m.swap(i, pivot);
            det = -det;
        }
        let diag = m[i][i];
        det *= diag;
        // As linhas abaixo são mutáveis sem copiar a linha-pivô.
        let (acima, abaixo) = m.split_at_mut(i + 1);
        let pivo = &acima[i];
        for linha in abaixo {
            let fator = linha[i] / diag;
            for c in i..n {
                linha[c] -= fator * pivo[c];
            }
        }
    }
    det
}

/// `=MATRIZ.INVERSO(matriz)` / `=MINVERSE(...)`
pub fn fn_minverse(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let m = match matriz_de_numeros(&args[0], ev) { Ok(m) => m, Err(e) => return e };
    let n = m.len();
    if n == 0 || m.iter().any(|row| row.len() != n) {
        return err_value();
    }

    let mut aug = vec![vec![0.0; 2 * n]; n];
    for i in 0..n {
        for j in 0..n {
            aug[i][j] = m[i][j];
        }
        aug[i][n + i] = 1.0;
    }

    for i in 0..n {
        let mut pivot = i;
        let mut max_val = aug[i][i].abs();
        for r in (i + 1)..n {
            let val = aug[r][i].abs();
            if val > max_val {
                max_val = val;
                pivot = r;
            }
        }
        if max_val < 1e-12 {
            return crate::funcoes::fn_helpers::err_num();
        }
        if pivot != i {
            aug.swap(i, pivot);
        }
        let div = aug[i][i];
        for c in 0..(2 * n) {
            aug[i][c] /= div;
        }
        for r in 0..n {
            if r != i {
                let factor = aug[r][i];
                for c in 0..(2 * n) {
                    let dec = factor * aug[i][c];
                    aug[r][c] -= dec;
                }
            }
        }
    }

    let mut res = vec![vec![Value::Empty; n]; n];
    for i in 0..n {
        for j in 0..n {
            res[i][j] = Value::Number(aug[i][n + j]);
        }
    }
    Computed::Matrix(Matrix::new(res))
}

fn extract_value_matrix(node: &Node, ev: &Evaluator<'_>) -> Result<Vec<Vec<Value>>, Computed> {
    if let Some(rect) = node_rect(node, ev) {
        let rows = (rect.r1 - rect.r0 + 1) as usize;
        let cols = (rect.c1 - rect.c0 + 1) as usize;
        let mut mat = vec![vec![Value::Empty; cols]; rows];
        for r in 0..rows {
            for c in 0..cols {
                mat[r][c] = ev.ctx.get_value(rect.r0 + r as u32, rect.c0 + c as u32, rect.sheet.as_deref());
            }
        }
        Ok(mat)
    } else {
        match ev.eval_array(node) {
            Computed::Scalar(v) => Ok(vec![vec![v]]),
            Computed::Matrix(m) => Ok(m.rows().to_vec()),
            lambda @ Computed::Lambda(_) => Ok(vec![vec![lambda.scalar()]]),
        }
    }
}

/// `=FILTRO(matriz; incluir; [se_vazio])` / `=FILTER(...)`
pub fn fn_filter(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let source = match extract_value_matrix(&args[0], ev) {
        Ok(m) => m,
        Err(e) => return e,
    };
    let include = match extract_value_matrix(&args[1], ev) {
        Ok(m) => m,
        Err(e) => return e,
    };

    let height = source.len();
    let width = if height > 0 { source[0].len() } else { 0 };

    let by_row = include.len() == height && include.len() > 1;
    let by_col = !by_row && include.first().map_or(0, |r| r.len()) == width && width > 1;

    if !by_row && !by_col {
        return err_value();
    }

    let mut rows = Vec::new();
    if by_row {
        for r in 0..height {
            let val = include[r].first().cloned().unwrap_or(Value::Empty);
            let manter = match crate::celulas::values::to_boolean(&val) {
                Ok(b) => b,
                Err(e) => return Computed::Scalar(Value::Error(e)),
            };
            if manter {
                rows.push(source[r].clone());
            }
        }
    } else {
        let mut keep_cols = Vec::new();
        for c in 0..width {
            let val = &include[0][c];
            let manter = match crate::celulas::values::to_boolean(val) {
                Ok(b) => b,
                Err(e) => return Computed::Scalar(Value::Error(e)),
            };
            if manter {
                keep_cols.push(c);
            }
        }
        for r in 0..height {
            let row = keep_cols.iter().map(|&c| source[r][c].clone()).collect();
            rows.push(row);
        }
    }

    if rows.is_empty() || rows.first().map_or(0, |r| r.len()) == 0 {
        if crate::funcoes::fn_helpers::has(args, 2) {
            return ev.eval_array(&args[2]);
        } else {
            return crate::funcoes::fn_helpers::err_na();
        }
    }
    Computed::Matrix(Matrix::new(rows))
}

/// `=ÚNICO(matriz; [por_coluna])` / `=UNIQUE(...)`
pub fn fn_unique(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let source = match extract_value_matrix(&args[0], ev) {
        Ok(m) => m,
        Err(e) => return e,
    };
    let by_column = if crate::funcoes::fn_helpers::has(args, 1) {
        match ev.boolean(&args[1]) {
            Ok(b) => b,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        false
    };

    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut rows = Vec::new();

    let height = source.len();
    let width = if height > 0 { source[0].len() } else { 0 };

    if by_column {
        let mut keep_cols = Vec::new();
        for c in 0..width {
            let key = (0..height).map(|r| crate::celulas::values::to_text(&source[r][c])).collect::<Vec<_>>().join("\u{0}");
            if seen.insert(key) {
                keep_cols.push(c);
            }
        }
        for r in 0..height {
            let row = keep_cols.iter().map(|&c| source[r][c].clone()).collect();
            rows.push(row);
        }
    } else {
        for row in source {
            let key = row.iter().map(|v| crate::celulas::values::to_text(v)).collect::<Vec<_>>().join("\u{0}");
            if seen.insert(key) {
                rows.push(row);
            }
        }
    }

    Computed::Matrix(Matrix::new(rows))
}

/// `=CLASSIFICAR(matriz; [coluna]; [ordem])` / `=SORT(...)`
pub fn fn_sort(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let mut source = match extract_value_matrix(&args[0], ev) {
        Ok(m) => m,
        Err(e) => return e,
    };
    let col = if crate::funcoes::fn_helpers::has(args, 1) {
        match ev.number(&args[1]) {
            Ok(n) if n.is_finite() && n >= 1.0 => n.trunc() as usize,
            Ok(_) => return err_value(),
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };
    let ascending = if crate::funcoes::fn_helpers::has(args, 2) {
        match ev.boolean(&args[2]) {
            Ok(b) => b,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        true
    };

    let idx = col.saturating_sub(1);
    let width = source.first().map_or(0, |r| r.len());
    if idx >= width {
        return err_value();
    }

    source.sort_by(|row_a, row_b| {
        let val_a = &row_a[idx];
        let val_b = &row_b[idx];
        let cmp = crate::celulas::values::compare(val_a, val_b).unwrap_or(std::cmp::Ordering::Equal);
        if ascending { cmp } else { cmp.reverse() }
    });

    Computed::Matrix(Matrix::new(source))
}

/// `=TRANSPOR(matriz)` / `=TRANSPOSE(...)`
pub fn fn_transpose(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let source = match extract_value_matrix(&args[0], ev) {
        Ok(m) => m,
        Err(e) => return e,
    };
    let height = source.len();
    let width = if height > 0 { source[0].len() } else { 0 };

    let mut rows = vec![vec![Value::Empty; height]; width];
    for r in 0..height {
        for c in 0..width {
            rows[c][r] = source[r][c].clone();
        }
    }
    Computed::Matrix(Matrix::new(rows))
}

/// `=SEQUÊNCIA(linhas; [colunas]; [inicio]; [passo])` / `=SEQUENCE(...)`
pub fn fn_sequence(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 4) {
        return Computed::Scalar(Value::Error(err));
    }
    let rows_num = match ev.number(&args[0]) {
        Ok(n) => n.trunc() as usize,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let cols_num = if crate::funcoes::fn_helpers::has(args, 1) {
        match ev.number(&args[1]) {
            Ok(n) => n.trunc() as usize,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };
    let start = if crate::funcoes::fn_helpers::has(args, 2) {
        match ev.number(&args[2]) {
            Ok(n) => n,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1.0
    };
    let step = if crate::funcoes::fn_helpers::has(args, 3) {
        match ev.number(&args[3]) {
            Ok(n) => n,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1.0
    };

    if rows_num < 1 || cols_num < 1 {
        return err_value();
    }
    if rows_num > 200_000 / cols_num {
        return crate::funcoes::fn_helpers::err_num();
    }

    let mut matrix_rows = Vec::with_capacity(rows_num);
    let mut val = start;
    for _ in 0..rows_num {
        let mut line = Vec::with_capacity(cols_num);
        for _ in 0..cols_num {
            line.push(Value::Number(val));
            val += step;
        }
        matrix_rows.push(line);
    }
    Computed::Matrix(Matrix::new(matrix_rows))
}

#[cfg(test)]
mod tests {
    use super::determinant;

    #[test]
    fn determinante_ajusta_sinal_na_troca_de_linhas() {
        let matriz = vec![
            vec![0.0, 2.0, 0.0],
            vec![3.0, 0.0, 0.0],
            vec![0.0, 0.0, 4.0],
        ];
        assert_eq!(determinant(&matriz), -24.0);
    }

    #[test]
    fn determinante_singular_retorna_zero() {
        assert_eq!(determinant(&[vec![1.0, 2.0], vec![1.0, 2.0]]), 0.0);
    }

    #[test]
    fn determinante_grande_usa_algoritmo_polinomial() {
        let mut matriz = vec![vec![0.0; 12]; 12];
        for (i, linha) in matriz.iter_mut().enumerate() {
            linha[i] = if i == 0 { 2.0 } else if i == 1 { 3.0 } else { 1.0 };
        }
        assert_eq!(determinant(&matriz), 6.0);
    }
}
