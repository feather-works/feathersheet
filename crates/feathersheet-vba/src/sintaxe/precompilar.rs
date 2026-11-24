//! Compilação condicional do VBA (`#If … #ElseIf … #Else … #End If`, `#Const`), achada em 13 dos 54 `.xlsm`
//! da bateria (#143): fica o ramo que o Excel do Windows de 64 bits escolheria. As linhas de diretiva e as do
//! ramo descartado viram linhas em branco, para os números de linha das pendências continuarem certos.
// shortcut: as constantes do projeto (Ferramentas ▸ Propriedades ▸ "Argumentos de compilação condicional")
// não são lidas do PROJECT; valem só as `#Const` do módulo, e nome desconhecido é 0 (como no VBA).

use std::collections::HashMap;

/// O fonte com só o ramo escolhido de cada `#If`.
pub fn pre_compilar(fonte: &str) -> String {
    if !fonte.lines().any(|l| l.trim_start().starts_with('#')) {
        return fonte.to_string();
    }
    let mut consts: HashMap<String, f64> = [("vba7", -1.0), ("vba6", -1.0), ("win64", -1.0), ("win32", -1.0), ("win16", 0.0), ("mac", 0.0)]
        .into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    // Pilha: (algum ramo deste #If já foi escolhido, o ramo atual está ativo, o #If pai estava ativo).
    let mut pilha: Vec<(bool, bool, bool)> = Vec::new();
    let ativo = |p: &Vec<(bool, bool, bool)>| p.last().is_none_or(|t| t.1);
    let mut saida = Vec::new();
    for linha in fonte.split_inclusive('\n') {
        let fim = if linha.ends_with("\r\n") { "\r\n" } else if linha.ends_with('\n') { "\n" } else { "" };
        let t = linha.trim();
        let minusc = t.to_lowercase();
        let diretiva = |p: &str| minusc.starts_with(p) && minusc[p.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        if diretiva("#if") {
            let pai = ativo(&pilha);
            let cond = pai && avaliar(depois_de(t, 3).trim_end_matches(|c: char| c.is_whitespace()).strip_suffix_ci("then"), &consts) != 0.0;
            pilha.push((cond, cond, pai));
        } else if diretiva("#elseif") {
            if let Some(topo) = pilha.last_mut() {
                let cond = topo.2 && !topo.0 && avaliar(depois_de(t, 7).trim_end().strip_suffix_ci("then"), &consts) != 0.0;
                topo.0 |= cond;
                topo.1 = cond;
            }
        } else if diretiva("#else") {
            if let Some(topo) = pilha.last_mut() {
                topo.1 = topo.2 && !topo.0;
                topo.0 = true;
            }
        } else if diretiva("#end if") || diretiva("#endif") {
            pilha.pop();
        } else if diretiva("#const") {
            if ativo(&pilha) {
                if let Some((nome, valor)) = depois_de(t, 6).split_once('=') {
                    let v = avaliar(valor, &consts);
                    consts.insert(nome.trim().to_lowercase(), v);
                }
            }
        } else if ativo(&pilha) {
            saida.push(linha.to_string());
            continue;
        }
        saida.push(fim.to_string());
    }
    saida.concat()
}

fn depois_de(t: &str, n: usize) -> &str {
    t.get(n..).unwrap_or("")
}

trait SemSufixo {
    fn strip_suffix_ci(&self, s: &str) -> &str;
}

impl SemSufixo for str {
    fn strip_suffix_ci(&self, s: &str) -> &str {
        let t = self.trim_end();
        if t.len() >= s.len() && t.is_char_boundary(t.len() - s.len()) && t[t.len() - s.len()..].eq_ignore_ascii_case(s) {
            &t[..t.len() - s.len()]
        } else {
            t
        }
    }
}

/// O valor de uma expressão de `#If`/`#Const` (verdadeiro é -1, como no VBA).
fn avaliar(texto: &str, consts: &HashMap<String, f64>) -> f64 {
    let fichas = fichas(texto);
    let mut i = 0;
    ou(&fichas, &mut i, consts)
}

fn fichas(texto: &str) -> Vec<String> {
    let mut v = Vec::new();
    let c: Vec<char> = texto.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let x = c[i];
        if x.is_whitespace() { i += 1; continue; }
        if x == '\'' { break; }
        let ini = i;
        if x.is_alphanumeric() || x == '_' || x == '.' {
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_' || c[i] == '.') { i += 1; }
        } else if (x == '<' || x == '>') && c.get(i + 1).is_some_and(|y| *y == '=' || (x == '<' && *y == '>')) {
            i += 2;
        } else {
            i += 1;
        }
        v.push(c[ini..i].iter().collect::<String>().to_lowercase());
    }
    v
}

fn ou(f: &[String], i: &mut usize, k: &HashMap<String, f64>) -> f64 {
    let mut a = e(f, i, k);
    while let Some(op) = f.get(*i).filter(|t| *t == "or" || *t == "xor") {
        let xor = op == "xor";
        *i += 1;
        let b = e(f, i, k);
        a = if xor { ((a as i64) ^ (b as i64)) as f64 } else { ((a as i64) | (b as i64)) as f64 };
    }
    a
}

fn e(f: &[String], i: &mut usize, k: &HashMap<String, f64>) -> f64 {
    let mut a = nao(f, i, k);
    while f.get(*i).is_some_and(|t| t == "and") {
        *i += 1;
        a = ((a as i64) & (nao(f, i, k) as i64)) as f64;
    }
    a
}

fn nao(f: &[String], i: &mut usize, k: &HashMap<String, f64>) -> f64 {
    if f.get(*i).is_some_and(|t| t == "not") {
        *i += 1;
        return !(nao(f, i, k) as i64) as f64;
    }
    comparacao(f, i, k)
}

fn comparacao(f: &[String], i: &mut usize, k: &HashMap<String, f64>) -> f64 {
    let a = termo(f, i, k);
    let Some(op) = f.get(*i).filter(|t| ["=", "<>", "<", ">", "<=", ">="].contains(&t.as_str())).cloned() else { return a };
    *i += 1;
    let b = termo(f, i, k);
    let v = match op.as_str() { "=" => a == b, "<>" => a != b, "<" => a < b, ">" => a > b, "<=" => a <= b, _ => a >= b };
    if v { -1.0 } else { 0.0 }
}

fn termo(f: &[String], i: &mut usize, k: &HashMap<String, f64>) -> f64 {
    let Some(t) = f.get(*i) else { return 0.0 };
    *i += 1;
    match t.as_str() {
        "(" => {
            let v = ou(f, i, k);
            if f.get(*i).is_some_and(|t| t == ")") { *i += 1; }
            v
        }
        "-" => -termo(f, i, k),
        "true" => -1.0,
        "false" => 0.0,
        _ => t.parse::<f64>().ok().or_else(|| k.get(t).copied()).unwrap_or(0.0),
    }
}

#[cfg(test)]
mod testes {
    use super::pre_compilar;

    #[test]
    fn fica_o_ramo_do_windows_de_64_bits() {
        let fonte = "a\n#If Mac Then\nmac\n#ElseIf VBA7 Then\nvba7\n#Else\nantigo\n#End If\n#Const DEPURAR = 1\n#If DEPURAR And Not Mac Then\ndepurar\n#End If\n#If Win64 Then\n#If X > 1 Then\nnao\n#Else\nsim\n#End If\n#End If\nz\n";
        let saida = pre_compilar(fonte);
        assert_eq!(saida.lines().count(), fonte.lines().count(), "as linhas ficam no lugar");
        let ficou: Vec<&str> = saida.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(ficou, ["a", "vba7", "depurar", "sim", "z"]);
    }
}
