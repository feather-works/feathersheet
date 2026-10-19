//! Funções que devolvem ou leem referência: DESLOC, INDIRETO,
//! AJUSTARINTERVALO, ÁREAS, FÓRMULATEXTO, ÉFÓRMULA, PLAN, PLANS, CÉL,
//! INFORMAÇÃO e INFODADOSTABELADINÂMICA.
//!
//! As três primeiras devolvem **referência**, não valor: o avaliador as troca
//! pelo `Ref`/`Range` que calculam antes de chamar a função de fora
//! ([`Evaluator::resolver_referencias`]), e é por isso que `SOMA(DESLOC(…))`
//! soma uma coluna inteira e `LIN(INDIRETO("B7"))` dá 7. Sozinhas na célula,
//! derramam os valores do intervalo.

use crate::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};
use crate::celulas::values::{to_text, CellError, Computed, ErrorCode, Value};
use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::*;
use crate::funcoes::functions;

/// Nomes (inglês e português) das funções que devolvem referência.
const DEVOLVEM_REFERENCIA: &[&str] =
    &["OFFSET", "DESLOC", "INDIRECT", "INDIRETO", "TRIMRANGE", "AJUSTARINTERVALO"];

/// Se a chamada com este nome devolve referência.
pub fn devolve_referencia(nome: &str) -> bool {
    DEVOLVEM_REFERENCIA.iter().any(|n| nome.eq_ignore_ascii_case(n))
}

/// A referência que a chamada devolve, como nó `Ref` ou `Range`.
pub fn referencia(nome: &str, args: &[Node], ev: &Evaluator<'_>) -> Res<Node> {
    let args = ev.resolver_referencias(args);
    match functions::canonical(nome).unwrap_or(nome).to_uppercase().as_str() {
        "OFFSET" => deslocar(&args, ev),
        "INDIRECT" => indireto(&args, ev),
        _ => ajustar(&args, ev),
    }
}

fn no_de(rect: Rect) -> Node {
    if rect.r0 == rect.r1 && rect.c0 == rect.c1 {
        Node::Ref { row: rect.r0, col: rect.c0, sheet: rect.sheet }
    } else {
        Node::Range { r0: rect.r0, c0: rect.c0, r1: rect.r1, c1: rect.c1, sheet: rect.sheet }
    }
}

fn erro_motivo(code: ErrorCode, motivo: &str) -> CellError {
    CellError::com_motivo(code, motivo)
}

fn valores_da_referencia(nome: &str, args: &[Node], ev: &Evaluator<'_>) -> Computed {
    match referencia(nome, args, ev) {
        Ok(no) => ev.eval_array(&no),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

/// `=DESLOC(ref; lins; cols; [altura]; [largura])` / `=OFFSET(reference, rows, cols, [height], [width])`
pub fn fn_offset(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    valores_da_referencia("OFFSET", args, ev)
}

fn deslocar(args: &[Node], ev: &Evaluator<'_>) -> Res<Node> {
    if let Some(e) = arity(args, 3, 5) {
        return Err(e);
    }
    let base = node_rect(&args[0], ev).ok_or_else(|| erro_motivo(ErrorCode::Value, "O primeiro argumento de DESLOC precisa ser uma referência"))?;
    let (lins, cols) = (arg_int(args, ev, 1)?, arg_int(args, ev, 2)?);
    let altura = if has(args, 3) { arg_int(args, ev, 3)? } else { (base.r1 - base.r0 + 1) as i64 };
    let largura = if has(args, 4) { arg_int(args, ev, 4)? } else { (base.c1 - base.c0 + 1) as i64 };
    if altura < 1 || largura < 1 {
        return Err(e_ref());
    }
    let r0 = base.r0 as i64 + lins;
    let c0 = base.c0 as i64 + cols;
    let (r1, c1) = (r0 + altura - 1, c0 + largura - 1);
    if r0 < 0 || c0 < 0 || r1 >= MAX_ROWS as i64 || c1 >= MAX_COLS as i64 {
        return Err(erro_motivo(ErrorCode::Ref, "DESLOC saiu da planilha"));
    }
    Ok(no_de(Rect { r0: r0 as u32, c0: c0 as u32, r1: r1 as u32, c1: c1 as u32, sheet: base.sheet }))
}

/// `=INDIRETO(texto_ref; [a1])` / `=INDIRECT(ref_text, [a1])`
pub fn fn_indirect(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    valores_da_referencia("INDIRECT", args, ev)
}

fn indireto(args: &[Node], ev: &Evaluator<'_>) -> Res<Node> {
    if let Some(e) = arity(args, 1, 2) {
        return Err(e);
    }
    let texto = arg_texto(args, ev, 0)?;
    let texto = texto.trim().trim_start_matches('=');
    let a1 = if has(args, 1) { arg_logico(args, ev, 1)? } else { true };
    let invalida = || erro_motivo(ErrorCode::Ref, &format!("\"{texto}\" não é uma referência"));
    if !a1 {
        return l1c1(texto, ev.here).map(no_de).ok_or_else(invalida);
    }
    match crate::formula::parser::parse_formula(texto) {
        Ok(no @ (Node::Ref { .. } | Node::Range { .. })) => Ok(no),
        Ok(Node::Name(n)) => ev.ctx.resolve_name(&n).map(no_de).ok_or_else(invalida),
        _ => Err(invalida()),
    }
}

/// `L2C3`, `R[1]C[-1]`, `Aba!R1C1:R5C2`: estilo L1C1 (em inglês ou em
/// português), relativo à célula da fórmula quando tem colchete.
fn l1c1(texto: &str, aqui: Option<(u32, u32)>) -> Option<Rect> {
    let (aba, resto) = match texto.rfind('!') {
        Some(i) => (Some(texto[..i].trim_matches('\'').to_string()), &texto[i + 1..]),
        None => (None, texto),
    };
    let mut partes = resto.split(':');
    let (r0, c0) = celula_l1c1(partes.next()?, aqui)?;
    let (r1, c1) = match partes.next() {
        Some(p) => celula_l1c1(p, aqui)?,
        None => (r0, c0),
    };
    if partes.next().is_some() {
        return None;
    }
    Some(Rect { r0: r0.min(r1), c0: c0.min(c1), r1: r0.max(r1), c1: c0.max(c1), sheet: aba })
}

fn celula_l1c1(texto: &str, aqui: Option<(u32, u32)>) -> Option<(u32, u32)> {
    let t = texto.trim().to_uppercase();
    let (lin, col) = t.split_once(['C'])?;
    let lin = lin.strip_prefix('R').or_else(|| lin.strip_prefix('L'))?;
    let (r_aqui, c_aqui) = aqui.unwrap_or((0, 0));
    let parte = |p: &str, atual: u32, max: u32| -> Option<u32> {
        let n = if p.is_empty() {
            atual as i64
        } else if let Some(rel) = p.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            atual as i64 + rel.parse::<i64>().ok()?
        } else {
            p.parse::<i64>().ok()? - 1
        };
        (0..max as i64).contains(&n).then_some(n as u32)
    };
    Some((parte(lin, r_aqui, MAX_ROWS)?, parte(col, c_aqui, MAX_COLS)?))
}

/// `=AJUSTARINTERVALO(intervalo; [aparar_linhas]; [aparar_colunas])` / `=TRIMRANGE(range, [trim_rows], [trim_cols])`
pub fn fn_trimrange(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    valores_da_referencia("TRIMRANGE", args, ev)
}

fn ajustar(args: &[Node], ev: &Evaluator<'_>) -> Res<Node> {
    if let Some(e) = arity(args, 1, 3) {
        return Err(e);
    }
    let rect = node_rect(&args[0], ev).ok_or_else(e_valor)?;
    let modo = |i: usize| -> Res<i64> {
        let m = if has(args, i) { arg_int(args, ev, i)? } else { 3 };
        if (0..=3).contains(&m) { Ok(m) } else { Err(e_valor()) }
    };
    let (ml, mc) = (modo(1)?, modo(2)?);
    let (mut r0, mut c0, mut r1, mut c1) = (u32::MAX, u32::MAX, 0, 0);
    ev.ctx.for_each_in_range(&rect, &mut |v, r, c| {
        if !matches!(v, Value::Empty) {
            r0 = r0.min(r);
            r1 = r1.max(r);
            c0 = c0.min(c);
            c1 = c1.max(c);
        }
    });
    if r0 == u32::MAX {
        // shortcut: intervalo todo vazio vira a primeira célula; o Excel não documenta o caso.
        return Ok(no_de(Rect { r1: rect.r0, c1: rect.c0, ..rect }));
    }
    // 1 = só as do começo, 2 = só as do fim, 3 = as duas.
    let aparar = |m: i64, ini: u32, fim: u32, achado_ini: u32, achado_fim: u32| {
        (if m & 1 != 0 { achado_ini } else { ini }, if m & 2 != 0 { achado_fim } else { fim })
    };
    let (nr0, nr1) = aparar(ml, rect.r0, rect.r1, r0, r1);
    let (nc0, nc1) = aparar(mc, rect.c0, rect.c1, c0, c1);
    Ok(no_de(Rect { r0: nr0, c0: nc0, r1: nr1, c1: nc1, sheet: rect.sheet }))
}

/// `=ÁREAS(ref)` / `=AREAS(reference)`
pub fn fn_areas(args: &[Node], _ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 1, || {
        // shortcut: o analisador não tem a união `(A1;B2:C3)`; toda referência é uma área só.
        match &args[0] {
            Node::Ref { .. } | Node::Range { .. } => numero(1.0),
            Node::Error(c) => Err(CellError::new(ErrorCode::from_code(c).unwrap_or(ErrorCode::Ref))),
            _ => Err(e_valor()),
        }
    })
}

/// A célula de cima à esquerda do argumento de referência.
fn celula_do_arg(args: &[Node], i: usize, ev: &Evaluator<'_>) -> Res<(u32, u32, Option<String>)> {
    match args.get(i) {
        Some(Node::Error(c)) => Err(CellError::new(ErrorCode::from_code(c).unwrap_or(ErrorCode::Ref))),
        Some(no) => node_rect(no, ev).map(|r| (r.r0, r.c0, r.sheet)).ok_or_else(e_valor),
        None => Err(e_valor()),
    }
}

/// `=FÓRMULATEXTO(ref)` / `=FORMULATEXT(reference)`
pub fn fn_formulatext(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 1, || {
        let (r, c, aba) = celula_do_arg(args, 0, ev)?;
        match ev.ctx.formula_de(r, c, aba.as_deref()) {
            Some(f) => texto(f),
            None => Err(erro_motivo(ErrorCode::Na, "A célula não tem fórmula")),
        }
    })
}

/// `=ÉFÓRMULA(ref)` / `=ISFORMULA(reference)`
pub fn fn_isformula(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 1, || {
        let (r, c, aba) = celula_do_arg(args, 0, ev)?;
        Ok(Value::Bool(ev.ctx.formula_de(r, c, aba.as_deref()).is_some()))
    })
}

fn posicao_da_aba(nomes: &[String], nome: &str) -> Option<usize> {
    nomes.iter().position(|n| crate::planilha::sheet::mesmo_nome(n, nome)).map(|i| i + 1)
}

/// `=PLAN([valor])` / `=SHEET([value])`
pub fn fn_sheet(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 0, 1, || {
        let (nomes, atual) = ev.ctx.abas();
        let alvo = match args.first() {
            None | Some(Node::Blank) => atual,
            Some(Node::Ref { sheet, .. } | Node::Range { sheet, .. }) => sheet.clone().or(atual),
            Some(Node::Error(c)) => return Err(CellError::new(ErrorCode::from_code(c).unwrap_or(ErrorCode::Ref))),
            Some(no) => match ev.eval(no) {
                Value::Error(e) => return Err(e),
                Value::Text(t) => Some(t),
                _ => return Err(e_valor()),
            },
        };
        let nome = alvo.ok_or_else(e_nd)?;
        // Sem a lista da pasta (avaliação avulsa), a aba da fórmula é a 1.
        if nomes.is_empty() {
            return numero(1.0);
        }
        posicao_da_aba(&nomes, &nome).map(|i| Value::Number(i as f64)).ok_or_else(e_nd)
    })
}

/// `=PLANS([ref])` / `=SHEETS([reference])`
pub fn fn_sheets(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 0, 1, || match args.first() {
        None | Some(Node::Blank) => numero(ev.ctx.abas().0.len().max(1) as f64),
        Some(Node::Ref { .. } | Node::Range { .. }) => numero(1.0),
        Some(Node::Error(c)) => Err(CellError::new(ErrorCode::from_code(c).unwrap_or(ErrorCode::Ref))),
        Some(_) => Err(e_valor()),
    })
}

/// `=CÉL(tipo_info; [ref])` / `=CELL(info_type, [reference])`
pub fn fn_cell(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 2, || {
        let tipo = arg_texto(args, ev, 0)?.trim().to_lowercase();
        let (r, c, aba) = if has(args, 1) {
            celula_do_arg(args, 1, ev)?
        } else {
            let (r, c) = ev.here.ok_or_else(e_valor)?;
            (r, c, None)
        };
        let valor = ev.ctx.get_value(r, c, aba.as_deref());
        match tipo.as_str() {
            "address" | "endereço" | "endereco" => {
                let endereco = format!("${}${}", crate::celulas::refs::col_name(c), r + 1);
                texto(match aba {
                    Some(a) => format!("{}!{endereco}", crate::celulas::refs::quote_sheet(&a)),
                    None => endereco,
                })
            }
            "col" => numero((c + 1) as f64),
            "row" | "lin" => numero((r + 1) as f64),
            "contents" | "conteúdo" | "conteudo" => Ok(valor),
            "type" | "tipo" => texto(match valor {
                Value::Empty => "b",
                Value::Text(_) => "l",
                _ => "v",
            }),
            "prefix" | "prefixo" => texto(if matches!(valor, Value::Text(_)) { "'" } else { "" }),
            "filename" | "arquivo" => texto(""),
            // shortcut: o motor não enxerga formato nem largura da coluna; respostas do formato Geral e da largura padrão.
            "format" | "formato" => texto("G"),
            "width" | "largura" => numero(8.0),
            "color" | "cor" | "parentheses" | "parênteses" | "parenteses" => numero(0.0),
            "protect" | "proteger" => numero(1.0),
            _ => Err(e_valor()),
        }
    })
}

/// `=INFORMAÇÃO(tipo)` / `=INFO(type_text)`
pub fn fn_info(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    executar(args, 1, 1, || match arg_texto(args, ev, 0)?.trim().to_lowercase().as_str() {
        "directory" | "diretório" | "diretorio" => texto(""),
        "numfile" | "numarquivo" | "numarq" => numero(ev.ctx.abas().0.len().max(1) as f64),
        "origin" | "origem" => texto("$A:$A$1"),
        "osversion" | "versãoso" | "versaoso" => texto(std::env::consts::OS),
        "recalc" => texto("Automático"),
        "release" | "versão" | "versao" => texto(env!("CARGO_PKG_VERSION")),
        "system" | "sistema" => texto(if cfg!(target_os = "macos") { "mac" } else { "pcdos" }),
        _ => Err(e_valor()),
    })
}

/// `=INFODADOSTABELADINÂMICA(campo_dados; tabela; [campo1; item1]; …)` / `=GETPIVOTDATA(data_field, pivot_table, [field1, item1], ...)`
///
/// Lê a tabela dinâmica já desenhada na planilha: a região contínua em
/// volta de `tabela`, com a linha de cabeçalho em cima e "Total Geral" na
/// última linha. Os pares campo/item escolhem a linha (campos de Linhas) e a
/// coluna (itens de Colunas, que aparecem no cabeçalho como `item (valor)`).
pub fn fn_getpivotdata(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 2 || args.len() % 2 != 0 {
        return err_value();
    }
    concluir((|| {
        let campo = arg_texto(args, ev, 0)?.trim().to_lowercase();
        let (r, c, aba) = celula_do_arg(args, 1, ev)?;
        let aba = aba.as_deref();
        let ler = |r: u32, c: u32| ev.ctx.get_value(r, c, aba);
        let txt = |r: u32, c: u32| to_text(&ler(r, c)).trim().to_lowercase();
        // Topo: sobe enquanto há algo na coluna; esquerda e direita pelo cabeçalho.
        let mut topo = r;
        while topo > 0 && !ler(topo - 1, c).is_empty() {
            topo -= 1;
        }
        let mut esq = c;
        while esq > 0 && !ler(topo, esq - 1).is_empty() {
            esq -= 1;
        }
        let mut dir = c;
        while dir + 1 < MAX_COLS && !ler(topo, dir + 1).is_empty() {
            dir += 1;
        }
        let cabecalho: Vec<String> = (esq..=dir).map(|x| txt(topo, x)).collect();
        let mut fundo = topo;
        while fundo + 1 < MAX_ROWS && (esq..=dir).any(|x| !ler(fundo + 1, x).is_empty()) {
            fundo += 1;
            if txt(fundo, esq) == "total geral" || txt(fundo, esq) == "grand total" {
                break;
            }
        }
        let nao_achou = || erro_motivo(ErrorCode::Ref, "Campo ou item não está na tabela dinâmica");
        let mut linha_alvo: Option<u32> = None;
        let mut itens_coluna: Vec<String> = Vec::new();
        for par in args[2..].chunks(2) {
            let nome = ev.text(&par[0])?.trim().to_lowercase();
            let item = to_text(&ev.eval(&par[1])).trim().to_lowercase();
            match cabecalho.iter().position(|h| *h == nome) {
                Some(i) => {
                    let col = esq + i as u32;
                    let linhas: Vec<u32> = (topo + 1..=fundo)
                        .filter(|&l| txt(l, col) == item && linha_alvo.is_none_or(|a| a == l))
                        .collect();
                    // shortcut: sem subtotais na saída, o par precisa achar exatamente uma linha.
                    match linhas.as_slice() {
                        [l] => linha_alvo = Some(*l),
                        _ => return Err(nao_achou()),
                    }
                }
                None => itens_coluna.push(item),
            }
        }
        let linha = linha_alvo.unwrap_or(fundo);
        let coluna = cabecalho
            .iter()
            .position(|h| {
                let (rotulo, medida) = match h.rsplit_once(" (") {
                    Some((itens, m)) => (itens, m.trim_end_matches(')')),
                    None => ("", h.as_str()),
                };
                let medida = medida.strip_prefix("total geral (").unwrap_or(medida);
                let bate_medida = medida == campo || medida.ends_with(&format!(" {campo}"));
                let itens: Vec<&str> = rotulo.split(" | ").filter(|s| !s.is_empty()).collect();
                let total = h.starts_with("total geral");
                bate_medida && if itens_coluna.is_empty() { total || itens.is_empty() } else { itens == itens_coluna }
            })
            .ok_or_else(nao_achou)?;
        match ler(linha, esq + coluna as u32) {
            v @ Value::Number(_) => Ok(v),
            _ => Err(nao_achou()),
        }
    })())
}

#[cfg(test)]
mod testes {
    use crate::celulas::values::{ErrorCode, Value};
    use crate::funcoes::fn_helpers::apoio_teste::*;

    fn n(x: f64) -> Value {
        Value::Number(x)
    }

    #[test]
    fn desloc_e_indireto_viram_referencia() {
        let cel = [("A1", n(1.0)), ("A2", n(2.0)), ("A3", n(3.0)), ("B2", n(20.0))];
        close(calc_cel(&cel, "SOMA(DESLOC(A1;0;0;3))"), 6.0);
        close(calc_cel(&cel, "DESLOC(A1;1;1)"), 20.0);
        close(calc_cel(&cel, "LIN(DESLOC(A1;4;0))"), 5.0);
        close(calc_cel(&cel, "SOMA(INDIRETO(\"A1:A3\"))"), 6.0);
        close(calc_cel(&cel, "INDIRETO(\"L2C2\";FALSO)"), 20.0);
        close(calc_cel(&cel, "SOMA(DESLOC(INDIRETO(\"A1\");1;0;2))"), 5.0);
        assert_eq!(calc_cel(&cel, "ÉREF(DESLOC(A1;1;1))"), Value::Bool(true));
        close(calc_cel(&cel, "SOMA(OFFSET(A1;0;0;3;1))"), 6.0);
        is_err(calc_cel(&cel, "DESLOC(A1;-1;0)"), ErrorCode::Ref);
        is_err(calc_cel(&cel, "INDIRETO(\"não é ref\")"), ErrorCode::Ref);
        assert_eq!(calc_cel(&cel, "ISERROR(INDIRETO(\"xx yy\"))"), Value::Bool(true));
    }

    #[test]
    fn ajustarintervalo_corta_as_bordas_vazias() {
        let cel = [("B3", n(1.0)), ("B5", n(2.0))];
        close(calc_cel(&cel, "LINS(AJUSTARINTERVALO(A1:C10))"), 3.0);
        close(calc_cel(&cel, "COLS(AJUSTARINTERVALO(A1:C10))"), 1.0);
        close(calc_cel(&cel, "LIN(AJUSTARINTERVALO(A1:C10;2))"), 1.0);
        close(calc_cel(&cel, "SOMA(TRIMRANGE(B:B))"), 3.0);
    }

    #[test]
    fn informacao_e_areas() {
        close(calc("ÁREAS(B2:C9)"), 1.0);
        close(calc("PLANS()"), 1.0);
        is_text(calc_cel(&[("C4", n(1.0))], "CÉL(\"endereço\";C4)"), "$C$4");
        is_text(calc_cel(&[("C4", Value::Text("x".into()))], "CELL(\"type\";C4)"), "l");
        close(calc("CÉL(\"lin\";C4)"), 4.0);
        is_text(calc("INFORMAÇÃO(\"origem\")"), "$A:$A$1");
        is_err(calc("FÓRMULATEXTO(A1)"), ErrorCode::Na);
        assert_eq!(calc("ÉFÓRMULA(A1)"), Value::Bool(false));
    }

    #[test]
    fn infodados_le_a_dinamica_desenhada() {
        let t = |s: &str| Value::Text(s.into());
        let cel = [
            ("A1", t("Região")), ("B1", t("Maçã (Vendas)")), ("C1", t("Uva (Vendas)")), ("D1", t("Total Geral (Vendas)")),
            ("A2", t("Norte")), ("B2", n(1.0)), ("C2", n(2.0)), ("D2", n(3.0)),
            ("A3", t("Sul")), ("B3", n(4.0)), ("C3", n(5.0)), ("D3", n(9.0)),
            ("A4", t("Total Geral")), ("B4", n(5.0)), ("C4", n(7.0)), ("D4", n(12.0)),
        ];
        close(calc_cel(&cel, "INFODADOSTABELADINÂMICA(\"Vendas\";A1)"), 12.0);
        close(calc_cel(&cel, "GETPIVOTDATA(\"Vendas\";B2;\"Região\";\"Sul\")"), 9.0);
        close(calc_cel(&cel, "GETPIVOTDATA(\"Vendas\";A1;\"Região\";\"Sul\";\"Produto\";\"Uva\")"), 5.0);
        close(calc_cel(&cel, "GETPIVOTDATA(\"Vendas\";A1;\"Produto\";\"Maçã\")"), 5.0);
        is_err(calc_cel(&cel, "GETPIVOTDATA(\"Vendas\";A1;\"Região\";\"Leste\")"), ErrorCode::Ref);
    }
}
