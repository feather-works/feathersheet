use crate::celulas::refs::{col_index, col_name};
pub use crate::celulas::refs::quote_sheet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRef {


    /// Nome da aba, quando a referência é qualificada (Dados!A1).
    pub sheet: Option<String>,
    pub col_lock: bool,
    pub row_lock: bool,
    pub col: i32,
    pub row: i32,
}

/// Monta o texto de uma referência de volta, preservando os cifrões.
pub fn format_ref(ref_cell: &ParsedRef) -> String {
    if ref_cell.col < 0 || ref_cell.row < 0 {
        return "#REF!".to_string();
    }
    let prefix = match &ref_cell.sheet {
        Some(s) => format!("{}!", quote_sheet(s)),
        None => String::new(),
    };
    let col_lock = if ref_cell.col_lock { "$" } else { "" };
    let row_lock = if ref_cell.row_lock { "$" } else { "" };
    let cname = col_name(ref_cell.col as u32);
    format!("{}{}{}{}{}", prefix, col_lock, cname, row_lock, ref_cell.row + 1)
}

/// Reescreve todas as referências A1 de uma fórmula.
pub fn map_references<F>(raw: &str, mut fn_map: F) -> String
where
    F: FnMut(ParsedRef) -> String,
{
    if !raw.starts_with('=') {
        return raw.to_string();
    }

    let chars: Vec<char> = raw.chars().collect();
    let mut out = String::from("=");
    let mut i = 1;
    let mut in_string = false;

    while i < chars.len() {
        let c = chars[i];

        if in_string {
            out.push(c);
            if c == '"' {
                if i + 1 < chars.len() && chars[i + 1] == '"' {
                    out.push('"');
                    i += 2;
                    continue;
                }
                in_string = false;
            }
            i += 1;
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            i += 1;
            continue;
        }

        let qualified = match_sheet_prefix(&chars, i);
        let ref_start = if let Some(ref q) = qualified {
            i + q.len
        } else {
            i
        };

        let ref_match = match_ref_at(&chars, ref_start);
        let previous = if i > 0 { chars[i - 1] } else { '\0' };
        let glued = is_glued_char(previous);

        if let Some((col_lock, col_str, row_lock, row_str, match_len)) = ref_match {
            if qualified.is_some() || !glued {
                let after_idx = ref_start + match_len;
                let after = if after_idx < chars.len() { chars[after_idx] } else { '\0' };

                if qualified.is_none() && (after == '(' || is_name_char(after)) {
                    let matched_str: String = chars[i..after_idx].iter().collect();
                    out.push_str(&matched_str);
                    i = after_idx;
                    continue;
                }

                if let Some(col) = col_index(&col_str) {
                    if let Ok(row_num) = row_str.parse::<i32>() {
                        let parsed = ParsedRef {
                            sheet: qualified.map(|q| q.name),
                            col_lock,
                            row_lock,
                            col: col as i32,
                            row: row_num - 1,
                        };
                        out.push_str(&fn_map(parsed));
                        i = ref_start + match_len;
                        continue;
                    }
                }
            }
        }

        out.push(c);
        i += 1;
    }

    out
}

struct SheetPrefix {
    name: String,
    len: usize,
}

fn match_sheet_prefix(chars: &[char], at: usize) -> Option<SheetPrefix> {
    if at >= chars.len() {
        return None;
    }

    // `[Vendas.fshz]Dados!` — aba de outro arquivo sem aspas: o nome vai
    // inteiro, senão inserir linha na aba local "Dados" deslocaria a
    // referência ao outro arquivo.
    if chars[at] == '[' {
        let fecha = (at + 1..chars.len()).find(|&k| chars[k] == ']' || chars[k] == '[').filter(|&k| chars[k] == ']')?;
        let mut i = fecha + 1;
        while i < chars.len() && (is_name_char(chars[i]) || chars[i].is_ascii_digit() || chars[i] == '.') {
            i += 1;
        }
        if fecha > at + 1 && i > fecha + 1 && i < chars.len() && chars[i] == '!' {
            return Some(SheetPrefix { name: chars[at..i].iter().collect(), len: i + 1 - at });
        }
        return None;
    }

    if chars[at] == '\'' {
        let mut i = at + 1;
        let mut name = String::new();
        while i < chars.len() {
            if chars[i] == '\'' {
                if i + 1 < chars.len() && chars[i + 1] == '\'' {
                    name.push('\'');
                    i += 2;
                    continue;
                }
                if i + 1 < chars.len() && chars[i + 1] == '!' {
                    return Some(SheetPrefix {
                        name,
                        len: (i + 2) - at,
                    });
                }
                return None;
            }
            name.push(chars[i]);
            i += 1;
        }
        return None;
    }

    if !is_name_char(chars[at]) {
        return None;
    }

    let previous = if at > 0 { chars[at - 1] } else { '\0' };
    if is_glued_char(previous) {
        return None;
    }

    let mut i = at;
    let mut name = String::new();
    while i < chars.len() && (is_name_char(chars[i]) || chars[i].is_ascii_digit() || chars[i] == '.') {
        name.push(chars[i]);
        i += 1;
    }
    // `Jan:Dez!B5` (3D): as duas abas são o nome, senão sobrava só `Dez`.
    if i < chars.len() && chars[i] == ':' {
        let mut k = i + 1;
        while k < chars.len() && (is_name_char(chars[k]) || chars[k].is_ascii_digit() || chars[k] == '.') {
            k += 1;
        }
        if k > i + 1 && k < chars.len() && chars[k] == '!' {
            name.extend(&chars[i..k]);
            i = k;
        }
    }

    if i < chars.len() && chars[i] == '!' {
        Some(SheetPrefix {
            name,
            len: (i + 1) - at,
        })
    } else {
        None
    }
}

fn match_ref_at(chars: &[char], at: usize) -> Option<(bool, String, bool, String, usize)> {
    if at >= chars.len() {
        return None;
    }

    let mut i = at;
    let col_lock = chars[i] == '$';
    if col_lock {
        i += 1;
    }

    let col_start = i;
    while i < chars.len() && chars[i].is_ascii_alphabetic() {
        i += 1;
    }
    let col_len = i - col_start;
    if col_len == 0 || col_len > 3 {
        return None;
    }
    let col_str: String = chars[col_start..i].iter().collect();

    let row_lock = i < chars.len() && chars[i] == '$';
    if row_lock {
        i += 1;
    }

    let row_start = i;
    while i < chars.len() && chars[i].is_ascii_digit() {
        i += 1;
    }
    let row_len = i - row_start;
    if row_len == 0 || row_len > 9 {
        return None;
    }
    let row_str: String = chars[row_start..i].iter().collect();

    Some((col_lock, col_str, row_lock, row_str, i - at))
}

fn is_name_char(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

/// Preserva tokens desconhecidos na reescrita textual: `1e3`, `2A1`,
/// `_A1` e nomes contendo ponto/dólar não podem ser tratados como
/// referências embutidas. O TypeScript exigia esse limite lexical.
fn is_glued_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '$')
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Row,
    Col,
}

pub struct StructureAdjustOptions<'a> {
    pub axis: Axis,
    pub at: i32,
    pub count: i32,
    pub removing: bool,
    pub sheet: &'a str,
    pub own_sheet: &'a str,
}

/// Ajusta uma fórmula depois de inserir ou remover linhas/colunas.
pub fn adjust_for_structure(raw: &str, options: StructureAdjustOptions) -> String {
    let same = |name: Option<&str>| {
        let n = name.unwrap_or(options.own_sheet);
        n.eq_ignore_ascii_case(options.sheet)
    };

    // A qualificação da primeira ponta vale para a faixa inteira, mas o
    // texto da segunda ponta continua sem prefixo. Antes, inserir uma linha
    // em Produtos ajustava só A1 de `Produtos!A1:A9`.
    use crate::formula::formula_pattern::{pattern_of, pedacos, Pedaco};
    let mut abas = Vec::new();
    // `faixas[i]`: a referência `i` é a primeira ponta de `i:i+1`.
    let mut faixas: Vec<bool> = Vec::new();
    let mut anterior = None;
    let mut separador = false;
    for pedaco in pedacos(&pattern_of(raw, 0, 0)) {
        match pedaco {
            Pedaco::Texto(t) => separador = t.trim() == ":",
            Pedaco::Ref(r) => {
                let aba = r.sheet.or_else(|| if separador { anterior.clone() } else { None });
                anterior = aba.clone();
                abas.push(aba);
                if separador {
                    if let Some(ultima) = faixas.last_mut() {
                        *ultima = true;
                    }
                }
                faixas.push(false);
                separador = false;
            }
        }
    }
    // As referências na ordem em que o `map_references` as visita.
    let mut refs: Vec<ParsedRef> = Vec::new();
    let _ = map_references(raw, |r| {
        let texto = format_ref(&r);
        refs.push(r);
        texto
    });
    let eixo = |r: &ParsedRef| if options.axis == Axis::Row { r.row } else { r.col };
    let afetada = |i: usize| {
        let aba = abas.get(i).and_then(|s| s.as_deref());
        same(aba.or(refs[i].sheet.as_deref()))
    };
    let fim = options.at + options.count;
    // O índice novo de uma referência sozinha (`None` = #REF!).
    let sozinha = |indice: i32| -> Option<i32> {
        if options.removing {
            if indice >= options.at && indice < fim {
                None
            } else if indice >= fim {
                Some(indice - options.count)
            } else {
                Some(indice)
            }
        } else if indice >= options.at {
            Some(indice + options.count)
        } else {
            Some(indice)
        }
    };
    let mut novos: Vec<Option<i32>> = refs.iter().map(|r| sozinha(eixo(r))).collect();
    // Excluir parte de um intervalo encolhe o intervalo, como no Excel
    // (`$I$2:$I$2000` sem a linha 2 fica `$I$2:$I$1999`); só o intervalo
    // inteiro excluído vira #REF!.
    if options.removing && faixas.len() == refs.len() {
        for i in 0..refs.len().saturating_sub(1) {
            if !(faixas[i] && afetada(i) && afetada(i + 1)) {
                continue;
            }
            let (a, b) = (eixo(&refs[i]), eixo(&refs[i + 1]));
            if a > b || (a >= options.at && b < fim) {
                continue;
            }
            novos[i] = Some(if a >= fim { a - options.count } else if a >= options.at { options.at } else { a });
            novos[i + 1] = Some(if b >= fim { b - options.count } else if b >= options.at { options.at - 1 } else { b });
        }
    }
    let mut indice = 0;
    let celulas = map_references(raw, |ref_cell| {
        let i = indice;
        indice += 1;
        let aba = abas.get(i).and_then(|s| s.as_deref());
        if !same(aba.or(ref_cell.sheet.as_deref())) {
            return format_ref(&ref_cell);
        }
        let Some(next) = novos.get(i).copied().unwrap_or_else(|| sozinha(eixo(&ref_cell))) else {
            return "#REF!".to_string();
        };
        if options.axis == Axis::Row {
            format_ref(&ParsedRef { row: next, ..ref_cell })
        } else {
            format_ref(&ParsedRef { col: next, ..ref_cell })
        }
    });
    ajustar_inteiras(&celulas, &options, &same)
}

/// Colunas e linhas inteiras (`B:B`, `Dados!$C:$I`, `5:7`) depois de
/// inserir ou remover: o `map_references` só vê referência de célula, e até
/// 17/10/2026 inserir a coluna B deixava `=SOMA(B:B)` lendo a coluna errada.
/// Mesma regra do intervalo de células: anda junto, encolhe quando parte
/// some, `#REF!` quando some inteiro.
fn ajustar_inteiras(raw: &str, options: &StructureAdjustOptions, same: &dyn Fn(Option<&str>) -> bool) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let letras = |at: usize| -> Option<(bool, u32, usize)> {
        let lock = chars.get(at) == Some(&'$');
        let ini = at + lock as usize;
        let mut fim = ini;
        while fim < chars.len() && chars[fim].is_ascii_alphabetic() && fim - ini < 3 { fim += 1; }
        let nome: String = chars[ini..fim].iter().collect();
        Some((lock, col_index(&nome.to_ascii_uppercase())?, fim))
    };
    let digitos = |at: usize| -> Option<(bool, u32, usize)> {
        let lock = chars.get(at) == Some(&'$');
        let ini = at + lock as usize;
        let mut fim = ini;
        while fim < chars.len() && chars[fim].is_ascii_digit() { fim += 1; }
        let n: u32 = chars[ini..fim].iter().collect::<String>().parse().ok().filter(|n| *n >= 1)?;
        Some((lock, n - 1, fim))
    };
    let fim_do_corte = options.at + options.count;
    // As duas pontas novas, ou `None` (#REF!) se o intervalo some inteiro.
    let ajustar = |a: u32, b: u32| -> Option<(i32, i32)> {
        let (a, b) = (a.min(b) as i32, a.max(b) as i32);
        if !options.removing {
            let mover = |x: i32| if x >= options.at { x + options.count } else { x };
            return Some((mover(a), mover(b)));
        }
        if a >= options.at && b < fim_do_corte {
            return None;
        }
        let na = if a >= fim_do_corte { a - options.count } else if a >= options.at { options.at } else { a };
        let nb = if b >= fim_do_corte { b - options.count } else if b >= options.at { options.at - 1 } else { b };
        Some((na, nb))
    };
    // A coluna/linha inteira que começa em `i`: o texto reescrito e onde
    // ela termina.
    let inteira_em = |i: usize| -> Option<(String, usize)> {
        let prefixo = match_sheet_prefix(&chars, i);
        let ini = i + prefixo.as_ref().map_or(0, |p| p.len);
        let aba = prefixo.as_ref().map(|p| p.name.as_str());
        for eixo in [Axis::Col, Axis::Row] {
            let ler = |at: usize| if eixo == Axis::Col { letras(at) } else { digitos(at) };
            let Some((lock_a, a, fim_a)) = ler(ini) else { continue };
            if chars.get(fim_a) != Some(&':') { continue; }
            let Some((lock_b, b, fim_b)) = ler(fim_a + 1) else { continue };
            let depois = chars.get(fim_b).copied().unwrap_or('\0');
            if is_glued_char(depois) || depois == '(' || depois == '!' { continue; }
            if eixo != options.axis || !same(aba) {
                return Some((chars[i..fim_b].iter().collect(), fim_b));
            }
            let prefixo_txt: String = chars[i..ini].iter().collect();
            let escrever = |lock: bool, n: i32| {
                let n = if eixo == Axis::Col { col_name(n as u32) } else { (n + 1).to_string() };
                format!("{}{n}", if lock { "$" } else { "" })
            };
            return Some((match ajustar(a, b) {
                None => "#REF!".to_string(),
                Some((na, nb)) => format!("{prefixo_txt}{}:{}", escrever(lock_a, na), escrever(lock_b, nb)),
            }, fim_b));
        }
        None
    };
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    let mut in_string = false;
    while i < chars.len() {
        let c = chars[i];
        if in_string || c == '"' {
            if c == '"' { in_string = !in_string; }
            out.push(c);
            i += 1;
            continue;
        }
        let colada = i > 0 && (is_glued_char(chars[i - 1]) || chars[i - 1] == ':');
        if !colada {
            if let Some((texto, fim)) = inteira_em(i) {
                out.push_str(&texto);
                i = fim;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ref() {
        let r = ParsedRef {
            sheet: None,
            col_lock: false,
            row_lock: false,
            col: 0,
            row: 0,
        };
        assert_eq!(format_ref(&r), "A1");

        let r_lock = ParsedRef {
            sheet: Some("Plan1".to_string()),
            col_lock: true,
            row_lock: true,
            col: 1,
            row: 4,
        };
        assert_eq!(format_ref(&r_lock), "Plan1!$B$5");
    }

    /// A aba de outro arquivo vai inteira na referência reescrita (arrastar a
    /// fórmula desloca a referência e mantém o arquivo).
    #[test]
    fn referencia_a_outro_arquivo_mantem_o_arquivo_ao_deslocar() {
        let deslocar = |raw: &str| map_references(raw, |mut r| {
            r.row += 1;
            format_ref(&r)
        });
        assert_eq!(deslocar("=[Vendas.fshz]Dados!A1*2"), "=[Vendas.fshz]Dados!A2*2");
        assert_eq!(deslocar("='/home/x/[Vendas.fshz]Dados'!B3"), "='/home/x/[Vendas.fshz]Dados'!B4");
        let mut abas = Vec::new();
        map_references("=[Vendas.fshz]Dados!A1+Dados!A1", |r| {
            abas.push(r.sheet.clone());
            format_ref(&r)
        });
        assert_eq!(abas, vec![Some("[Vendas.fshz]Dados".to_string()), Some("Dados".to_string())]);
    }

    #[test]
    fn colunas_e_linhas_inteiras_andam_com_a_estrutura() {
        let ajustar = |raw: &str, axis, at, count, removing| adjust_for_structure(raw, StructureAdjustOptions {
            axis, at, count, removing, sheet: "Dados", own_sheet: "Dados",
        });
        assert_eq!(ajustar("=SOMA(B:B)+B1", Axis::Col, 1, 1, false), "=SOMA(C:C)+C1");
        assert_eq!(ajustar("=MÍNIMOSES(AO:AO;A:A;A2)", Axis::Col, 1, 2, false), "=MÍNIMOSES(AQ:AQ;A:A;A2)");
        assert_eq!(ajustar("=PROCV(X;Dados!$C:$I;2;0)", Axis::Col, 3, 1, true), "=PROCV(X;Dados!$C:$H;2;0)");
        assert_eq!(ajustar("=SOMA(C:C)", Axis::Col, 2, 1, true), "=SOMA(#REF!)");
        assert_eq!(ajustar("=SOMA(Outra!B:B)", Axis::Col, 0, 1, false), "=SOMA(Outra!B:B)");
        assert_eq!(ajustar("=SOMA(5:7)+SOMA(B:B)", Axis::Row, 0, 1, false), "=SOMA(6:8)+SOMA(B:B)");
        assert_eq!(ajustar("=\"B:B\"&LOG10(2)", Axis::Col, 0, 1, false), "=\"B:B\"&LOG10(2)");
    }

    #[test]
    fn test_quote_sheet() {
        assert_eq!(quote_sheet("Dados"), "Dados");
        assert_eq!(quote_sheet("Meu Plano"), "'Meu Plano'");
        assert_eq!(quote_sheet("D'água"), "'D''água'");
        assert_eq!(quote_sheet("Σoma"), "'Σoma'");
        assert_eq!(format_ref(&ParsedRef {
            sheet: Some("Σoma".into()), col_lock: false,
            row_lock: false, col: 0, row: 0,
        }), "'Σoma'!A1");
    }

    #[test]
    fn test_map_references() {
        let formula = "=SOMA(A1; B2; \"texto A1\"; Plan1!C3)";
        let mapped = map_references(formula, |mut ref_cell| {
            ref_cell.row += 1;
            format_ref(&ref_cell)
        });
        assert_eq!(mapped, "=SOMA(A2; B3; \"texto A1\"; Plan1!C4)");
    }

    #[test]
    fn nao_reescreve_referencia_no_meio_de_identificador_ou_numero() {
        let formula = "=1e3+2A1+_B2+f.A3+$C$4+LOG10(A1)+A1";
        let reescrita = map_references(formula, |mut r| {
            r.row += 1;
            format_ref(&r)
        });
        assert_eq!(
            reescrita,
            "=1e3+2A1+_B2+f.A3+$C$5+LOG10(A2)+A2"
        );
        assert_eq!(map_references("texto A1", |_| "X".into()), "texto A1");
    }

    #[test]
    fn referencias_qualificadas_e_strings_escapadas_sao_preservadas() {
        let original = "=A1+'O''Brien'!$B$2+\"A1 \"\" B2\"+Dados!C3";
        let atualizado = map_references(original, |mut r| {
            r.row += 1;
            format_ref(&r)
        });
        assert_eq!(
            atualizado,
            "=A2+'O''Brien'!$B$3+\"A1 \"\" B2\"+Dados!C4"
        );
        // Alterar só uma aba: não confundir referências locais e externas.
        assert_eq!(
            adjust_for_structure(
                "=A3+Outra!A3+Dados!$B$3",
                StructureAdjustOptions {
                    axis: Axis::Row,
                    at: 2,
                    count: 1,
                    removing: true,
                    sheet: "Dados",
                    own_sheet: "Dados",
                },
            ),
            "=#REF!+Outra!A3+#REF!"
        );
    }

    #[test]
    fn test_adjust_for_structure() {
        let formula = "=A1 + B5";
        let res = adjust_for_structure(
            formula,
            StructureAdjustOptions {
                axis: Axis::Row,
                at: 2,
                count: 1,
                removing: false,
                sheet: "Aba1",
                own_sheet: "Aba1",
            },
        );
        assert_eq!(res, "=A1 + B6");
    }

    fn excluir_linhas(formula: &str, at: i32, count: i32) -> String {
        adjust_for_structure(formula, StructureAdjustOptions {
            axis: Axis::Row, at, count, removing: true, sheet: "Dados", own_sheet: "Resumo",
        })
    }

    #[test]
    fn excluir_a_primeira_linha_do_intervalo_encolhe_o_intervalo() {
        // O caso da "Planilha Pesada": a linha 2 (índice 1) sai.
        assert_eq!(
            excluir_linhas("=SOMASES(Dados!$I$2:$I$2000;Dados!$C$2:$C$2000;A1)", 1, 1),
            "=SOMASES(Dados!$I$2:$I$1999;Dados!$C$2:$C$1999;A1)"
        );
        // Meio do intervalo e a última ponta.
        assert_eq!(excluir_linhas("=SOMA(Dados!A2:A5)", 2, 2), "=SOMA(Dados!A2:A3)");
        assert_eq!(excluir_linhas("=SOMA(Dados!A2:A5)", 4, 1), "=SOMA(Dados!A2:A4)");
        // Começo e fim atravessados pela exclusão.
        assert_eq!(excluir_linhas("=SOMA(Dados!A2:A5)", 0, 3), "=SOMA(Dados!A1:A2)");
        // O intervalo inteiro excluído: #REF!.
        assert_eq!(excluir_linhas("=SOMA(Dados!A3:A4)", 2, 2), "=SOMA(#REF!:#REF!)");
        // Outra aba não muda; referência sozinha excluída continua #REF!.
        assert_eq!(excluir_linhas("=Outra!A2:A5+Dados!B3", 2, 1), "=Outra!A2:A5+#REF!");
    }

    #[test]
    fn excluir_colunas_tambem_encolhe_o_intervalo() {
        let r = adjust_for_structure("=SOMA(B1:E1)", StructureAdjustOptions {
            axis: Axis::Col, at: 1, count: 1, removing: true, sheet: "Aba1", own_sheet: "Aba1",
        });
        assert_eq!(r, "=SOMA(B1:D1)");
    }
}

