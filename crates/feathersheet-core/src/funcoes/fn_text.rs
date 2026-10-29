//! Funções de texto em Rust: CONCATENAR, MAIÚSCULA, MINÚSCULA, PRI.MAIÚSCULA, ESQUERDA, DIREITA, EXT.TEXTO, NÚM.CARACT, ARRUMAR,
//! CARACT, CÓDIGO, TIRAR, EXATO, T, MUDAR, CARACTUNICODE, UNICODE, VALORNUMÉRICO, DEF.NÚM.DEC, MOEDA, TEXTOANTES, TEXTODEPOIS,
//! VALORPARATEXTO, MATRIZPARATEXTO, REGEXTESTE.

use crate::formula::ast::Node;
use crate::formula::evaluator::Evaluator;
use crate::funcoes::fn_helpers::{
    arity, e_nd, e_valor, has, matriz_de, numero, por_elemento, texto, Res,
};
use crate::celulas::values::{to_text, Computed, Value};

/// `=MAIÚSCULA(texto)` / `=UPPER(texto)`
pub fn fn_upper(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let txt = to_text(&val);
    Computed::Scalar(Value::Text(txt.to_uppercase()))
}

/// `=IMAGEM(base64)` / `=IMAGE(base64)` e `=IMAGEM.BASE64(imagem)`: imagem ↔
/// base64 (`imagem_base64.rs`). A célula com o data URL vira imagem na grade.
fn imagem_ou_erro(args: &[Node], ev: &Evaluator<'_>, f: fn(&str) -> Option<String>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    match f(&to_text(&val)) {
        Some(t) => Computed::Scalar(Value::Text(t)),
        None => Computed::Scalar(Value::Error(crate::celulas::values::CellError::com_motivo(
            crate::celulas::values::ErrorCode::Value,
            "não parece uma imagem (PNG, JPEG, GIF, WebP, SVG ou BMP) em base64",
        ))),
    }
}

pub fn fn_image(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    imagem_ou_erro(args, ev, crate::funcoes::imagem_base64::para_data_url)
}

pub fn fn_image_base64(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    imagem_ou_erro(args, ev, crate::funcoes::imagem_base64::para_base64)
}

/// `=MINÚSCULA(texto)` / `=LOWER(texto)`
pub fn fn_lower(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let txt = to_text(&val);
    Computed::Scalar(Value::Text(txt.to_lowercase()))
}

/// `=NÚM.CARACT(texto)` / `=LEN(texto)`
pub fn fn_len(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let txt = to_text(&val);
    Computed::Scalar(Value::Number(txt.chars().count() as f64))
}

/// `=CONCAT(args...)`: cada célula do intervalo entra.
pub fn fn_concat(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    // `CONCAT(A1:A3)` é "102030". O `ev.value` reduzia o intervalo à primeira célula.
    let mut out = String::new();
    for a in args {
        if let Some(e) = ev.for_each_value(a, &mut |v, _| out.push_str(&to_text(v))) {
            return Computed::Scalar(Value::Error(e));
        }
    }
    Computed::Scalar(Value::Text(out))
}

/// `=ESQUERDA(texto; [num_caract])` / `=LEFT(texto; [num_caract])`
pub fn fn_left(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let count = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(n) => n.max(0.0) as usize,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };

    let txt = to_text(&val);
    let res: String = txt.chars().take(count).collect();
    Computed::Scalar(Value::Text(res))
}

/// `=DIREITA(texto; [num_caract])` / `=RIGHT(texto; [num_caract])`
pub fn fn_right(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let count = if has(args, 1) {
        match ev.number(&args[1]) {
            Ok(n) => n.max(0.0) as usize,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };

    let txt = to_text(&val);
    let total = txt.chars().count();
    let skip = total.saturating_sub(count);
    let res: String = txt.chars().skip(skip).collect();
    Computed::Scalar(Value::Text(res))
}

/// `=ARRUMAR(texto)` / `=TRIM(texto)`
pub fn fn_trim(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let txt = to_text(&val);
    let trimmed = txt.split_whitespace().collect::<Vec<_>>().join(" ");
    Computed::Scalar(Value::Text(trimmed))
}

/// `=EXT.TEXTO(texto; num_inicial; num_caract)` / `=MID(texto; start_num; num_chars)`
pub fn fn_mid(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let start = match ev.number(&args[1]) {
        Ok(n) => n as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    let len = match ev.number(&args[2]) {
        Ok(n) => n as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if start < 1 || len < 0 {
        return Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value)));
    }
    let txt = to_text(&val);
    let skip = (start - 1) as usize;
    let take = len as usize;
    let res: String = txt.chars().skip(skip).take(take).collect();
    Computed::Scalar(Value::Text(res))
}

/// `=REPETIR(texto; num_vezes)` / `=REPT(texto; number_times)`
pub fn fn_rept(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let times = match ev.number(&args[1]) {
        Ok(n) => n as i64,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    if times < 0 {
        return Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value)));
    }
    let txt = to_text(&val);
    // Como o Excel: resultado acima de 32.767 caracteres é #VALOR!. Sem o teto,
    // `=REPT("x";2000000000)` pedia 2 GB de uma vez (#125).
    if (txt.chars().count() as u128) * (times as u128) > 32_767 {
        return Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value)));
    }
    let res = txt.repeat(times as usize);
    Computed::Scalar(Value::Text(res))
}

/// `=SUBSTITUIR(texto; texto_antigo; texto_novo; [núm_ocorrência])` / `=SUBSTITUTE(...)`
pub fn fn_substitute(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 3, 4) {
        return Computed::Scalar(Value::Error(err));
    }
    let text_val = ev.value(&args[0]);
    if let Value::Error(e) = text_val { return Computed::Scalar(Value::Error(e)); }
    let from_val = ev.value(&args[1]);
    if let Value::Error(e) = from_val { return Computed::Scalar(Value::Error(e)); }
    let to_val = ev.value(&args[2]);
    if let Value::Error(e) = to_val { return Computed::Scalar(Value::Error(e)); }

    let text = to_text(&text_val);
    let from = to_text(&from_val);
    let to = to_text(&to_val);

    if from.is_empty() {
        return Computed::Scalar(Value::Text(text));
    }

    if has(args, 3) {
        let instance = match ev.number(&args[3]) {
            Ok(n) => n as usize,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        };
        if instance == 0 {
            return Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value)));
        }
        let mut count = 0;
        let mut result = String::new();
        let mut last_end = 0;

        for (idx, _) in text.match_indices(&from) {
            count += 1;
            if count == instance {
                result.push_str(&text[last_end..idx]);
                result.push_str(&to);
                result.push_str(&text[idx + from.len()..]);
                return Computed::Scalar(Value::Text(result));
            }
            last_end = idx;
        }
        Computed::Scalar(Value::Text(text))
    } else {
        Computed::Scalar(Value::Text(text.replace(&from, &to)))
    }
}

/// `=PROCURAR(texto_procurado; no_texto; [num_inicial])` / `=FIND(...)`
pub fn fn_find(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let needle_val = ev.value(&args[0]);
    if let Value::Error(e) = needle_val { return Computed::Scalar(Value::Error(e)); }
    let haystack_val = ev.value(&args[1]);
    if let Value::Error(e) = haystack_val { return Computed::Scalar(Value::Error(e)); }

    let start = if has(args, 2) {
        match ev.number(&args[2]) {
            Ok(n) => n as usize,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };

    let needle = to_text(&needle_val);
    let haystack = to_text(&haystack_val);

    if start < 1 || start > haystack.chars().count() + 1 {
        return Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value)));
    }

    let skip_bytes = haystack.chars().take(start - 1).map(|c| c.len_utf8()).sum();
    if let Some(pos) = haystack[skip_bytes..].find(&needle) {
        let char_idx = haystack[..skip_bytes + pos].chars().count() + 1;
        Computed::Scalar(Value::Number(char_idx as f64))
    } else {
        Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value)))
    }
}

/// `=PRI.MAIÚSCULA(texto)` / `=PROPER(texto)`
pub fn fn_proper(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val {
        return Computed::Scalar(Value::Error(e));
    }
    let txt = to_text(&val);
    let mut res = String::new();
    let mut prev_is_letter = false;
    for c in txt.chars() {
        if c.is_alphabetic() {
            if !prev_is_letter {
                res.extend(c.to_uppercase());
            } else {
                res.extend(c.to_lowercase());
            }
            prev_is_letter = true;
        } else {
            res.push(c);
            prev_is_letter = false;
        }
    }
    Computed::Scalar(Value::Text(res))
}

/// `=CONCATENAR(args...)` / `=CONCATENATE(args...)`
///
/// Função antiga: não aceita intervalo, só concatena os argumentos (um
/// intervalo entra pela primeira célula, como `ev.value`). Quem quer o
/// intervalo inteiro usa `CONCAT`.
pub fn fn_concatenate(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let mut out = String::new();
    for a in args {
        match ev.value(a) {
            Value::Error(e) => return Computed::Scalar(Value::Error(e)),
            v => out.push_str(&to_text(&v)),
        }
    }
    Computed::Scalar(Value::Text(out))
}

/// `=LOCALIZAR(texto_procurado; no_texto; [num_inicial])` / `=SEARCH(...)`
pub fn fn_search(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 3) {
        return Computed::Scalar(Value::Error(err));
    }
    let needle_val = ev.value(&args[0]);
    if let Value::Error(e) = needle_val { return Computed::Scalar(Value::Error(e)); }
    let haystack_val = ev.value(&args[1]);
    if let Value::Error(e) = haystack_val { return Computed::Scalar(Value::Error(e)); }

    let start = if has(args, 2) {
        match ev.number(&args[2]) {
            Ok(n) => n as usize,
            Err(e) => return Computed::Scalar(Value::Error(e)),
        }
    } else {
        1
    };

    let needle = to_text(&needle_val).to_lowercase();
    let haystack = to_text(&haystack_val).to_lowercase();

    if start < 1 || start > haystack.chars().count() + 1 {
        return Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value)));
    }

    let h: Vec<char> = haystack.chars().collect();
    let p: Vec<char> = needle.chars().collect();
    match (start - 1..=h.len()).find(|&i| casa_prefixo_curinga(&p, &h[i..])) {
        Some(i) => Computed::Scalar(Value::Number((i + 1) as f64)),
        None => Computed::Scalar(Value::Error(crate::celulas::values::CellError::new(crate::celulas::values::ErrorCode::Value))),
    }
}

/// O padrão (`?` um caractere, `*` qualquer trecho, `~` escapa) casa o começo
/// do texto? Usado por `LOCALIZAR`/`SEARCH`, que aceita curingas.
fn casa_prefixo_curinga(p: &[char], t: &[char]) -> bool {
    match p.first() {
        None => true,
        Some('*') => (0..=t.len()).any(|k| casa_prefixo_curinga(&p[1..], &t[k..])),
        Some('?') => !t.is_empty() && casa_prefixo_curinga(&p[1..], &t[1..]),
        Some('~') if p.len() > 1 => t.first() == Some(&p[1]) && casa_prefixo_curinga(&p[2..], &t[1..]),
        Some(c) => t.first() == Some(c) && casa_prefixo_curinga(&p[1..], &t[1..]),
    }
}

/// `=VALOR(texto)` / `=VALUE(texto)`
///
/// Como o `VALUE` do `fn-text.ts`: o argumento vira texto e o texto vira
/// número pela mesma regra de toda conta (`to_number`): ponto de milhar,
/// vírgula decimal, `R$`, `%`. Até 24/09/2026 isto era um `parse` à parte, e
/// `VALOR("1.234")` dava 1,234 em vez de 1234.
pub fn fn_value(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 1, 1) {
        return Computed::Scalar(Value::Error(err));
    }
    let texto = match ev.text(&args[0]) {
        Ok(t) => t,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };
    match crate::celulas::values::to_number(&Value::Text(texto)) {
        Ok(n) => Computed::Scalar(Value::Number(n)),
        Err(e) => Computed::Scalar(Value::Error(e)),
    }
}

// ---- caracteres, comparação e substituição por posição ----

/// Os 32 pontos de código do Windows-1252 entre 128 e 159 (0 = sem caractere).
const CP1252_ALTO: [u32; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039,
    0x0152, 0x8D, 0x017D, 0x8F, 0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014,
    0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

/// `=CARACT(núm)` / `=CHAR(number)` — caractere do código 1–255 (Windows-1252).
pub fn fn_char(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let n = e.int(0)?;
        if !(1..=255).contains(&n) {
            return Err(e_valor());
        }
        let ponto = if (128..=159).contains(&n) { CP1252_ALTO[(n - 128) as usize] } else { n as u32 };
        texto(char::from_u32(ponto).unwrap_or('?').to_string())
    })
}

/// `=CÓDIGO(texto)` / `=CODE(text)` — código (Windows-1252) do primeiro caractere.
pub fn fn_code(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let t = e.texto(0)?;
        let c = t.chars().next().ok_or_else(e_valor)?;
        let p = c as u32;
        let codigo = if p < 128 || (160..=255).contains(&p) {
            p
        } else if let Some(i) = CP1252_ALTO.iter().position(|x| *x == p) {
            128 + i as u32
        } else {
            63 // '?', o que o Excel devolve para o que não existe na página de código
        };
        numero(codigo as f64)
    })
}

/// `=TIRAR(texto)` / `=CLEAN(text)` — remove os caracteres de controle 0–31.
pub fn fn_clean(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| texto(e.texto(0)?.chars().filter(|c| (*c as u32) >= 32).collect::<String>()))
}

/// `=EXATO(texto1; texto2)` / `=EXACT(text1; text2)` — igualdade que diferencia maiúsculas.
pub fn fn_exact(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 2, |e| Ok(Value::Bool(e.texto(0)? == e.texto(1)?)))
}

/// `=T(valor)` — o próprio valor se for texto, senão texto vazio. Erro passa adiante.
pub fn fn_t(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| match e.valor(0) {
        Value::Error(er) => Err(er.clone()),
        Value::Text(t) => texto(t.clone()),
        _ => texto(""),
    })
}

/// `=MUDAR(texto_antigo; núm_inicial; núm_caract; novo_texto)` /
/// `=REPLACE(old_text; start_num; num_chars; new_text)`
pub fn fn_replace(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 4, 4, |e| {
        let antigo: Vec<char> = e.texto(0)?.chars().collect();
        let inicio = e.int(1)?;
        let quantos = e.int(2)?;
        let novo = e.texto(3)?;
        if inicio < 1 || quantos < 0 {
            return Err(e_valor());
        }
        let i = ((inicio - 1) as usize).min(antigo.len());
        let f = i.saturating_add(quantos as usize).min(antigo.len());
        let mut s: String = antigo[..i].iter().collect();
        s.push_str(&novo);
        s.extend(antigo[f..].iter());
        texto(s)
    })
}

/// `=CARACTUNICODE(núm)` / `=UNICHAR(number)`
pub fn fn_unichar(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let n = e.int(0)?;
        match u32::try_from(n).ok().and_then(char::from_u32) {
            Some(c) if n >= 1 => texto(c.to_string()),
            _ => Err(e_valor()),
        }
    })
}

/// `=UNICODE(texto)` — ponto de código Unicode do primeiro caractere.
pub fn fn_unicode(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 1, |e| {
        let t = e.texto(0)?;
        numero(t.chars().next().ok_or_else(e_valor)? as u32 as f64)
    })
}

// ---- números como texto ----

/// `=VALORNUMÉRICO(texto; [sep_decimal]; [sep_grupo])` /
/// `=NUMBERVALUE(text; [decimal_separator]; [group_separator])`.
/// Sem separadores, vale o idioma do programa: vírgula decimal e ponto de grupo.
pub fn fn_numbervalue(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 3, |e| {
        let t = e.texto(0)?;
        let primeiro = |i: usize, padrao: char| -> Res<char> {
            if e.tem(i) {
                e.texto(i)?.chars().next().ok_or_else(e_valor)
            } else {
                Ok(padrao)
            }
        };
        let dec = primeiro(1, ',')?;
        let grupo = primeiro(2, '.')?;
        if dec == grupo {
            return Err(e_valor());
        }
        let limpo: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        if limpo.is_empty() {
            return Ok(Value::Number(0.0));
        }
        let mut corpo = limpo.as_str();
        let mut percentuais = 0;
        while let Some(r) = corpo.strip_suffix('%') {
            corpo = r;
            percentuais += 1;
        }
        let mut saida = String::new();
        let mut viu_decimal = false;
        for c in corpo.chars() {
            if c == dec {
                if viu_decimal {
                    return Err(e_valor());
                }
                viu_decimal = true;
                saida.push('.');
            } else if c == grupo {
                if viu_decimal {
                    return Err(e_valor());
                }
            } else {
                saida.push(c);
            }
        }
        if !saida.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E')) {
            return Err(e_valor());
        }
        let n: f64 = saida.parse().map_err(|_| e_valor())?;
        numero(n / 100f64.powi(percentuais))
    })
}

/// Parte inteira e fracionária de `n` (sem sinal) arredondadas, metade para
/// longe do zero, em `casas` casas (negativas arredondam à esquerda). O Excel
/// trabalha com 15 algarismos significativos, então o arredondamento parte
/// deles — assim 2,675 vira 2,68 e 0,1+0,2 vale 0,3.
fn arredondar_decimal(n: f64, casas: i64) -> (String, String) {
    let n = n.abs();
    if n == 0.0 {
        return ("0".into(), "0".repeat(casas.max(0) as usize));
    }
    let cientifico = format!("{:.14e}", n);
    let (mant, exp) = cientifico.split_once('e').unwrap();
    let exp: i64 = exp.parse().unwrap();
    let sig: Vec<u8> = mant.bytes().filter(|b| b.is_ascii_digit()).map(|b| b - b'0').collect();
    // todos = 0 + parte inteira + parte fracionária; `ponto` = onde a vírgula cai.
    let mut todos: Vec<u8> = vec![0];
    let ponto;
    if exp >= 0 {
        let inteira = exp as usize + 1;
        for i in 0..inteira {
            todos.push(*sig.get(i).unwrap_or(&0));
        }
        ponto = todos.len();
        todos.extend(sig.iter().skip(inteira));
    } else {
        ponto = 1;
        todos.extend(std::iter::repeat_n(0, (-exp - 1) as usize));
        todos.extend(sig.iter());
    }
    let corte = ponto as i64 + casas;
    if corte < 0 {
        return ("0".into(), "0".repeat(casas.max(0) as usize));
    }
    let corte = corte as usize;
    let sobe = todos.get(corte).is_some_and(|d| *d >= 5);
    let mut r: Vec<u8> = todos.iter().take(corte).copied().collect();
    r.resize(corte.max(ponto), 0);
    if casas < 0 {
        for d in r.iter_mut().skip(corte) {
            *d = 0;
        }
    }
    if sobe && corte >= 1 {
        let mut i = corte - 1;
        loop {
            if r[i] == 9 {
                r[i] = 0;
                if i == 0 {
                    break;
                }
                i -= 1;
            } else {
                r[i] += 1;
                break;
            }
        }
    }
    let txt = |v: &[u8]| v.iter().map(|d| (b'0' + d) as char).collect::<String>();
    let inteira = txt(&r[..ponto]);
    let inteira = inteira.trim_start_matches('0');
    let inteira = if inteira.is_empty() { "0".to_string() } else { inteira.to_string() };
    (inteira, txt(&r[ponto..]))
}

/// Número com separador de milhar (opcional) e `casas` decimais, em pt-BR.
fn numero_pt(n: f64, casas: i64, milhar: bool) -> String {
    let (inteira, fracao) = arredondar_decimal(n, casas);
    let nulo = inteira.bytes().all(|b| b == b'0') && fracao.bytes().all(|b| b == b'0');
    let mut agrupada = String::new();
    if milhar {
        for (i, c) in inteira.chars().enumerate() {
            if i > 0 && (inteira.len() - i) % 3 == 0 {
                agrupada.push('.');
            }
            agrupada.push(c);
        }
    } else {
        agrupada = inteira;
    }
    if !fracao.is_empty() {
        agrupada.push(',');
        agrupada.push_str(&fracao);
    }
    if n < 0.0 && !nulo {
        agrupada.insert(0, '-');
    }
    agrupada
}

/// Casas decimais de FIXED/DOLLAR: argumento ausente vale 2; escrito em
/// branco (`FIXED(x;;VERDADEIRO)`) vale 0. Mais de 127 é `#VALOR!`.
fn casas_decimais(e: &crate::funcoes::fn_helpers::Entradas<'_>) -> Res<i64> {
    let c = if e.len() > 1 { e.int(1)? } else { 2 };
    if c > 127 {
        return Err(e_valor());
    }
    Ok(c.max(-400))
}

/// `=DEF.NÚM.DEC(núm; [casas]; [sem_pontos])` / `=FIXED(number; [decimals]; [no_commas])`
pub fn fn_fixed(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 3, |e| {
        let n = e.num(0)?;
        let casas = casas_decimais(e)?;
        let sem = e.logico_ou(2, false)?;
        texto(numero_pt(n, casas, !sem))
    })
}

/// `=MOEDA(núm; [casas])` / `=DOLLAR(number; [decimals])` — em reais, `R$ 1.234,57`.
pub fn fn_dollar(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 2, |e| {
        let n = e.num(0)?;
        let casas = casas_decimais(e)?;
        let t = numero_pt(n, casas, true);
        match t.strip_prefix('-') {
            Some(resto) => texto(format!("-R$ {resto}")),
            None => texto(format!("R$ {t}")),
        }
    })
}

// ---- antes e depois de um delimitador ----

/// Um achado de delimitador: posição inicial e final, em caracteres.
type Achado = (usize, usize);

fn minuscula_c(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Todos os achados dos delimitadores, da esquerda para a direita, sem se sobrepor.
fn achados(t: &[char], delims: &[Vec<char>], ignorar: bool) -> Vec<Achado> {
    let igual = |a: char, b: char| if ignorar { minuscula_c(a) == minuscula_c(b) } else { a == b };
    let mut out = Vec::new();
    let mut i = 0;
    while i < t.len() {
        let mut melhor = 0;
        for d in delims {
            if !d.is_empty() && i + d.len() <= t.len() && d.iter().zip(&t[i..]).all(|(a, b)| igual(*a, *b)) {
                melhor = melhor.max(d.len());
            }
        }
        if melhor > 0 {
            out.push((i, i + melhor));
            i += melhor;
        } else {
            i += 1;
        }
    }
    out
}

fn antes_ou_depois(args: &[Node], ev: &Evaluator<'_>, antes: bool) -> Computed {
    if let Some(er) = arity(args, 2, 6) {
        return Computed::Scalar(Value::Error(er));
    }
    let r: Res<Value> = (|| {
        let t = match ev.value(&args[0]) {
            Value::Error(er) => return Err(er),
            v => to_text(&v),
        };
        let delims: Vec<Vec<char>> = match &args[1] {
            Node::Blank => vec![vec![]],
            n => {
                let m = matriz_de(ev, n)?;
                let mut v = Vec::new();
                for x in m.values() {
                    if let Value::Error(er) = x {
                        return Err(er.clone());
                    }
                    v.push(to_text(x).chars().collect());
                }
                v
            }
        };
        let num_ou = |i: usize, padrao: i64| -> Res<i64> {
            if has(args, i) {
                crate::funcoes::fn_helpers::inteiro(&ev.value(&args[i]))
            } else {
                Ok(padrao)
            }
        };
        let instancia = num_ou(2, 1)?;
        let modo = num_ou(3, 0)?;
        let fim = num_ou(4, 0)?;
        if instancia == 0 || !(0..=1).contains(&modo) || !(0..=1).contains(&fim) {
            return Err(e_valor());
        }
        let cs: Vec<char> = t.chars().collect();
        if instancia.unsigned_abs() as usize > cs.len().max(1) && !cs.is_empty() {
            return Err(e_valor());
        }
        let nao_achou = || -> Res<Value> {
            if has(args, 5) {
                match ev.value(&args[5]) {
                    Value::Error(er) => Err(er),
                    v => Ok(v),
                }
            } else {
                Err(e_nd())
            }
        };
        // Delimitador vazio: casa no começo (instância positiva) ou no fim (negativa).
        let achado: Option<Achado> = if delims.iter().any(|d| d.is_empty()) {
            if instancia.abs() != 1 {
                None
            } else if instancia > 0 {
                Some((0, 0))
            } else {
                Some((cs.len(), cs.len()))
            }
        } else {
            let mut lista = achados(&cs, &delims, modo == 1);
            if instancia > 0 {
                if fim == 1 {
                    lista.push((cs.len(), cs.len()));
                }
                lista.get(instancia as usize - 1).copied()
            } else {
                if fim == 1 {
                    lista.insert(0, (0, 0));
                }
                let k = (-instancia) as usize;
                if k <= lista.len() { Some(lista[lista.len() - k]) } else { None }
            }
        };
        let Some((a, b)) = achado else { return nao_achou() };
        let pedaco: String = if antes { cs[..a].iter().collect() } else { cs[b..].iter().collect() };
        Ok(Value::Text(pedaco))
    })();
    crate::funcoes::fn_helpers::concluir(r)
}

/// `=TEXTOANTES(texto; delimitador; [instância]; [modo]; [fim]; [se_não_achar])` /
/// `=TEXTBEFORE(text; delimiter; [instance_num]; [match_mode]; [match_end]; [if_not_found])`
pub fn fn_textbefore(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    antes_ou_depois(args, ev, true)
}

/// `=TEXTODEPOIS(texto; delimitador; [instância]; [modo]; [fim]; [se_não_achar])` /
/// `=TEXTAFTER(text; delimiter; [instance_num]; [match_mode]; [match_end]; [if_not_found])`
pub fn fn_textafter(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    antes_ou_depois(args, ev, false)
}

// ---- valor ou matriz como texto ----

/// Texto de um valor: no formato estrito (1) o texto vem entre aspas.
fn valor_como_texto(v: &Value, estrito: bool) -> String {
    match v {
        Value::Text(t) if estrito => format!("\"{}\"", t.replace('"', "\"\"")),
        outro => to_text(outro),
    }
}

/// `=VALORPARATEXTO(valor; [formato])` / `=VALUETOTEXT(value; [format])`
pub fn fn_valuetotext(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 1, 2, |e| {
        let formato = e.int_ou(1, 0)?;
        if !(0..=1).contains(&formato) {
            return Err(e_valor());
        }
        texto(valor_como_texto(e.valor(0), formato == 1))
    })
}

/// `=MATRIZPARATEXTO(matriz; [formato])` / `=ARRAYTOTEXT(array; [format])`
pub fn fn_arraytotext(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    let r: Res<Value> = (|| {
        if let Some(er) = arity(args, 1, 2) {
            return Err(er);
        }
        let formato = if has(args, 1) { crate::funcoes::fn_helpers::inteiro(&ev.value(&args[1]))? } else { 0 };
        if !(0..=1).contains(&formato) {
            return Err(e_valor());
        }
        let m = matriz_de(ev, &args[0])?;
        let estrito = formato == 1;
        let linhas: Vec<Vec<String>> = m
            .rows()
            .iter()
            .map(|l| l.iter().map(|v| valor_como_texto(v, estrito)).collect())
            .collect();
        if estrito {
            let corpo: Vec<String> = linhas.iter().map(|l| l.join(",")).collect();
            Ok(Value::Text(format!("{{{}}}", corpo.join(";"))))
        } else {
            let todos: Vec<String> = linhas.into_iter().flatten().collect();
            Ok(Value::Text(todos.join(", ")))
        }
    })();
    crate::funcoes::fn_helpers::concluir(r)
}

/// `=REGEXTESTE(texto; padrão; [diferenciar])` / `=REGEXTEST(text; pattern; [case_sensitivity])`
/// — 0 (padrão) diferencia maiúsculas, 1 ignora.
pub fn fn_regextest(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    por_elemento(args, ev, 2, 3, |e| {
        let t = e.texto(0)?;
        let padrao = e.texto(1)?;
        let modo = e.int_ou(2, 0)?;
        if !(0..=1).contains(&modo) {
            return Err(e_valor());
        }
        let (t, padrao) = if modo == 1 {
            // Sem opção de maiúsculas no motor: baixa os dois, poupando os escapes (\D, \W…).
            let mut p = String::new();
            let mut escapado = false;
            for c in padrao.chars() {
                if escapado {
                    p.push(c);
                    escapado = false;
                } else if c == '\\' {
                    p.push(c);
                    escapado = true;
                } else {
                    p.extend(c.to_lowercase());
                }
            }
            (t.to_lowercase(), p)
        } else {
            (t, padrao)
        };
        let re = crate::funcoes::regex::Regex::new(&padrao).map_err(|_| e_valor())?;
        Ok(Value::Bool(re.casa(&t).map_err(|_| e_valor())?))
    })
}

#[cfg(test)]
mod testes_valor {
    use crate::celulas::values::Value;
    use crate::planilha::sheet::Sheet;

    fn valor(f: &str) -> Value {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, format!("={f}"), None);
        sheet.get_value(0, 0)
    }

    /// Os esperados saíram do `VALUE` do TypeScript, menos um.
    #[test]
    fn valor_como_o_typescript() {
        // Como o Excel em português: o ponto é separador de milhar, "1.234" = 1234
        // (decidido em 14/10/2026; o teste antigo esperava 1,234).
        assert_eq!(valor("VALOR(\"1.234\")"), Value::Number(1234.0));
        assert_eq!(valor("VALOR(\"1.234,5\")"), Value::Number(1234.5));
        assert_eq!(valor("VALOR(\"R$ 5,00\")"), Value::Number(5.0));
        assert_eq!(valor("VALOR(\"50%\")"), Value::Number(0.5));
        assert_eq!(valor("VALOR(\"1e3\")"), Value::Number(1000.0));
        assert_eq!(valor("VALOR(12)"), Value::Number(12.0));
        assert_eq!(valor("VALOR(\"\")"), Value::Number(0.0));
        let Value::Error(e) = valor("VALOR(\"abc\")") else { panic!("abc") };
        assert_eq!(e.detail.as_deref(), Some("\"abc\" não é número"));
        let Value::Error(e) = valor("VALOR(VERDADEIRO)") else { panic!("VERDADEIRO") };
        assert_eq!(e.detail.as_deref(), Some("\"VERDADEIRO\" não é número"));
    }
    #[test]
    fn texto_com_mascara_de_data() {
        assert_eq!(valor("TEXTO(DATA(2020;1;5);\"dd/mm/aaaa\")"), Value::Text("05/01/2020".into()));
        assert_eq!(valor("TEXTO(DATA(2020;1;5);\"dd/mm/yyyy\")"), Value::Text("05/01/2020".into()));
        assert_eq!(valor("TEXTO(1250,5;\"R$ #.##0,00\")"), Value::Text("R$ 1.250,50".into()));
    }
}

/// `=TEXTO(valor; formato)` / `=TEXT(valor; formato)`
pub fn fn_text_fmt(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if let Some(err) = arity(args, 2, 2) {
        return Computed::Scalar(Value::Error(err));
    }
    let val = ev.value(&args[0]);
    if let Value::Error(e) = val { return Computed::Scalar(Value::Error(e)); }
    let fmt_val = ev.value(&args[1]);
    if let Value::Error(e) = fmt_val { return Computed::Scalar(Value::Error(e)); }

    let fmt = to_text(&fmt_val);
    let n = match &val {
        Value::Number(n) => *n,
        Value::Bool(b) => if *b { 1.0 } else { 0.0 },
        Value::Text(s) => match s.trim().parse::<f64>() {
            Ok(n) => n,
            Err(_) => return Computed::Scalar(Value::Text(to_text(&val))),
        },
        Value::Empty => 0.0,
        Value::Error(e) => return Computed::Scalar(Value::Error(e.clone())),
    };

    // Sem `0`/`#` não é máscara de número (`dd/mm/aaaa`, `hh:mm`): vai pelo
    // formato personalizado da célula, que sabe data e hora.
    if !fmt.contains(['0', '#']) {
        if let Some(t) = crate::celulas::formato_personalizado::formatar(&Value::Number(n), &fmt) {
            return Computed::Scalar(Value::Text(t));
        }
    }
    Computed::Scalar(Value::Text(aplicar_mascara(n, &fmt)))
}

/// A máscara do `TEXTO` — a mesma regra do `aplicarMascara` do
/// `fn-text.ts`, e com os mesmos casos de teste nos dois lados.
///
/// Aceita os dois jeitos de escrever: pt-BR (`"#.##0,00"`) e inglês
/// (`"#,##0.00"`). O separador decimal é o ÚLTIMO dos dois; havendo um só,
/// ele é de milhar quando aparece mais de uma vez, ou quando vem seguido de
/// exatamente três marcadores e tem `#` antes (`"#,##0"`). O resultado sai
/// sempre em pt-BR, vírgula decimal e ponto de milhar, e arredonda como o
/// Excel: metade para longe do zero (1250,5 com zero casas é 1251).
///
/// Antes os dois motores só entendiam a máscara em inglês, e o Rust ainda
/// arredondava metade para o par: `TEXTO(1250,5; "R$ #.##0,00")` dava
/// "R$ 1.251" no TypeScript e "R$ 1250" aqui, contra "R$ 1.250,50".
pub fn aplicar_mascara(n: f64, mascara: &str) -> String {
    let chars: Vec<char> = mascara.chars().collect();
    let marcador = |c: char| c == '0' || c == '#';
    let (Some(primeiro), Some(ultimo)) = (chars.iter().position(|&c| marcador(c)), chars.iter().rposition(|&c| marcador(c))) else {
        return mascara.to_string();
    };
    let prefixo: String = chars[..primeiro].iter().collect();
    let corpo = &chars[primeiro..=ultimo];
    let sufixo: String = chars[ultimo + 1..].iter().collect();
    let porcento = mascara.contains('%');

    let seps: Vec<usize> = corpo.iter().enumerate().filter(|(_, &c)| c == '.' || c == ',').map(|(i, _)| i).collect();
    let decimal = match seps.as_slice() {
        [] => None,
        [um] => {
            let depois = corpo.len() - um - 1;
            let milhar = depois == 3 && corpo[..*um].contains(&'#');
            if milhar { None } else { Some(*um) }
        }
        varios => {
            let ultimo_sep = *varios.last().unwrap();
            let tipos_diferentes = varios.iter().any(|&i| corpo[i] != corpo[ultimo_sep]);
            if tipos_diferentes { Some(ultimo_sep) } else { None }
        }
    };
    let (inteira, fracao) = match decimal {
        Some(i) => (&corpo[..i], &corpo[i + 1..]),
        None => (corpo, &corpo[corpo.len()..]),
    };
    let agrupar = inteira.iter().any(|&c| c == '.' || c == ',');
    let min_casas = fracao.iter().filter(|&&c| c == '0').count();
    let max_casas = fracao.iter().filter(|&&c| marcador(c)).count();
    let min_inteiros = inteira.iter().filter(|&&c| c == '0').count();

    let v = if porcento { n * 100.0 } else { n };
    let fator = 10f64.powi(max_casas as i32);
    let arredondado = (v.abs() * fator).round() / fator;
    let texto = format!("{:.*}", max_casas, arredondado);
    let (parte_int, parte_frac) = texto.split_once('.').unwrap_or((texto.as_str(), ""));
    let mut frac = parte_frac.to_string();
    while frac.len() > min_casas && frac.ends_with('0') {
        frac.pop();
    }
    let mut int = if parte_int == "0" && min_inteiros == 0 { String::new() } else { parte_int.to_string() };
    while int.len() < min_inteiros {
        int.insert(0, '0');
    }
    if agrupar {
        let digitos: Vec<char> = int.chars().collect();
        let mut agrupado = String::new();
        for (i, d) in digitos.iter().enumerate() {
            if i > 0 && (digitos.len() - i) % 3 == 0 {
                agrupado.push('.');
            }
            agrupado.push(*d);
        }
        int = agrupado;
    }
    let negativo = arredondado != 0.0 && v < 0.0;
    let numero = if frac.is_empty() { int } else { format!("{int},{frac}") };
    format!("{}{prefixo}{numero}{sufixo}", if negativo { "-" } else { "" })
}

/// `=UNIRTEXTO(delimitador; ignorar_vazios; texto1; [texto2]; ...)` / `=TEXTJOIN(...)`
pub fn fn_textjoin(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 3 {
        return crate::funcoes::fn_helpers::err_value();
    }
    let delim_val = ev.value(&args[0]);
    if let Value::Error(e) = delim_val { return Computed::Scalar(Value::Error(e)); }
    let delim = to_text(&delim_val);

    let ignore_empty = match ev.boolean(&args[1]) {
        Ok(b) => b,
        Err(e) => return Computed::Scalar(Value::Error(e)),
    };

    let mut parts = Vec::new();
    for a in &args[2..] {
        let mut err = None;
        ev.for_each_value(a, &mut |v, _| {
            if err.is_some() { return; }
            if let Value::Error(e) = v {
                err = Some(e.clone());
                return;
            }
            let s = to_text(v);
            if !ignore_empty || !s.is_empty() {
                parts.push(s);
            }
        });
        if let Some(e) = err {
            return Computed::Scalar(Value::Error(e));
        }
    }
    Computed::Scalar(Value::Text(parts.join(&delim)))
}

/// `=DIVIDIRTEXTO(texto; col_delim; [row_delim])` / `=TEXTSPLIT(...)`
pub fn fn_textsplit(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 2 {
        return crate::funcoes::fn_helpers::err_value();
    }
    let text_val = ev.value(&args[0]);
    if let Value::Error(e) = text_val { return Computed::Scalar(Value::Error(e)); }
    let col_delim_val = ev.value(&args[1]);
    if let Value::Error(e) = col_delim_val { return Computed::Scalar(Value::Error(e)); }

    let text = to_text(&text_val);
    let col_delim = to_text(&col_delim_val);

    if has(args, 2) {
        let row_delim_val = ev.value(&args[2]);
        if let Value::Error(e) = row_delim_val { return Computed::Scalar(Value::Error(e)); }
        let row_delim = to_text(&row_delim_val);

        let row_splits: Vec<&str> = text.split(&row_delim).collect();
        let matrix: Vec<Vec<Value>> = row_splits
            .into_iter()
            .map(|r| r.split(&col_delim).map(|cell| Value::Text(cell.to_string())).collect())
            .collect();
        Computed::Matrix(crate::celulas::values::Matrix::new(matrix))
    } else {
        let cols: Vec<Value> = text.split(&col_delim).map(|cell| Value::Text(cell.to_string())).collect();
        Computed::Matrix(crate::celulas::values::Matrix::new(vec![cols]))
    }
}

/// `=JUNTAR(delimitador; valor1; [valor2]; ...)` / `=JOIN(...)`
pub fn fn_join(args: &[Node], ev: &Evaluator<'_>) -> Computed {
    if args.len() < 2 {
        return crate::funcoes::fn_helpers::err_value();
    }
    let delim_val = ev.value(&args[0]);
    if let Value::Error(e) = delim_val { return Computed::Scalar(Value::Error(e)); }
    let delim = to_text(&delim_val);

    let mut parts = Vec::new();
    for a in &args[1..] {
        let mut err = None;
        ev.for_each_value(a, &mut |v, _| {
            if err.is_some() { return; }
            if let Value::Error(e) = v {
                err = Some(e.clone());
                return;
            }
            if !matches!(v, Value::Empty) {
                parts.push(to_text(v));
            }
        });
        if let Some(e) = err {
            return Computed::Scalar(Value::Error(e));
        }
    }
    Computed::Scalar(Value::Text(parts.join(&delim)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planilha::sheet::Sheet;

    #[test]
    fn test_text_g20() {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, "=PRI.MAIÚSCULA(\"maria da silva\")".into(), None);
        assert_eq!(sheet.get_value(0, 0), Value::Text("Maria Da Silva".into()));

        sheet.set_cell_raw(0, 1, "=LOCALIZAR(\"MUND\"; \"olá mundo\")".into(), None);
        assert_eq!(sheet.get_value(0, 1), Value::Number(5.0));

        sheet.set_cell_raw(1, 0, "=PROCURAR(\"a\";\"ABC\")".into(), None);
        assert!(matches!(sheet.get_value(1, 0), Value::Error(_)), "PROCURAR diferencia maiúsculas");
        sheet.set_cell_raw(1, 1, "=LOCALIZAR(\"a\";\"ABC\")".into(), None);
        assert_eq!(sheet.get_value(1, 1), Value::Number(1.0));
        sheet.set_cell_raw(1, 2, "=LOCALIZAR(\"b*\";\"abc\")".into(), None);
        assert_eq!(sheet.get_value(1, 2), Value::Number(2.0));
        sheet.set_cell_raw(1, 3, "=PROCURAR(\"b\";\"abc\")".into(), None);
        assert_eq!(sheet.get_value(1, 3), Value::Number(2.0));

        sheet.set_cell_raw(0, 2, "=VALOR(\"123,45\")".into(), None);
        assert_eq!(sheet.get_value(0, 2), Value::Number(123.45));

        sheet.set_cell_raw(0, 3, "=UNIRTEXTO(\"; \"; VERDADEIRO; \"A\"; \"\"; \"B\")".into(), None);
        assert_eq!(sheet.get_value(0, 3), Value::Text("A; B".into()));
    }

    fn calc(f: &str) -> Value {
        let mut sheet = Sheet::new("Plan1");
        sheet.set_cell_raw(0, 0, format!("={f}"), None);
        sheet.get_value(0, 0)
    }

    fn texto(s: &str) -> Value {
        Value::Text(s.into())
    }

    #[test]
    fn caracteres_e_codigos() {
        assert_eq!(calc("CARACT(65)"), texto("A"));
        assert_eq!(calc("CÓDIGO(\"A\")"), Value::Number(65.0));
        assert_eq!(calc("CARACT(128)"), texto("\u{20ac}"));
        assert_eq!(calc("UNICODE(\"\u{20ac}\")"), Value::Number(8364.0));
        assert_eq!(calc("CARACTUNICODE(9731)"), texto("\u{2603}"));
        assert!(matches!(calc("CARACT(0)"), Value::Error(_)));
        assert_eq!(calc("EXATO(\"a\"; \"A\")"), Value::Bool(false));
        assert_eq!(calc("T(5)"), texto(""));
        assert_eq!(calc("TIRAR(\"a\"&CARACT(7)&\"b\")"), texto("ab"));
        assert_eq!(calc("MUDAR(\"abcdef\"; 2; 3; \"XY\")"), texto("aXYef"));
    }

    #[test]
    fn numeros_como_texto() {
        assert_eq!(calc("VALORNUMÉRICO(\"3,5%\")"), Value::Number(0.035));
        assert_eq!(calc("VALORNUMÉRICO(\"2.500,27\"; \",\"; \".\")"), Value::Number(2500.27));
        assert_eq!(calc("DEF.NÚM.DEC(1234,567; 1)"), texto("1.234,6"));
        assert_eq!(calc("DEF.NÚM.DEC(1234,567; -1)"), texto("1.230"));
        assert_eq!(calc("DEF.NÚM.DEC(-1234,567; 0; VERDADEIRO)"), texto("-1235"));
        assert_eq!(calc("MOEDA(1234,567)"), texto("R$ 1.234,57"));
        assert_eq!(calc("MOEDA(-0,5; 0)"), texto("-R$ 1"));
    }

    #[test]
    fn antes_depois_e_expressao_regular() {
        assert_eq!(calc("TEXTOANTES(\"Vermelho, Verde\"; \", \")"), texto("Vermelho"));
        assert_eq!(calc("TEXTODEPOIS(\"a-b-c\"; \"-\"; -1)"), texto("c"));
        assert_eq!(calc("TEXTOANTES(\"a-b-c\"; \"-\"; 3; 0; 1)"), texto("a-b-c"));
        assert!(matches!(calc("TEXTOANTES(\"abc\"; \"x\")"), Value::Error(_)));
        assert_eq!(calc("TEXTOANTES(\"abc\"; \"x\"; 1; 0; 0; \"nada\")"), texto("nada"));
        assert_eq!(calc("VALORPARATEXTO(\"oi\"; 1)"), texto("\"oi\""));
        assert_eq!(calc("REGEXTESTE(\"ABC\"; \"b\"; 1)"), Value::Bool(true));
        assert_eq!(calc("REGEXTESTE(\"ABC\"; \"b\")"), Value::Bool(false));
    }
}
