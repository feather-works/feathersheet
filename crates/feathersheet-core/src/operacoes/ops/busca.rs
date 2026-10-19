//! Localizar e substituir.

use std::collections::{HashMap, HashSet};
use crate::celulas::refs::MAX_ROWS;
use crate::celulas::values::{to_text, Value};
use crate::planilha::data_table::Cru;
use crate::planilha::sheet::Sheet;
use crate::planilha::textos_crus::TextosCrus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    pub match_case: bool,
    /// O texto precisa ser o conteúdo inteiro da célula.
    pub whole_cell: bool,
    /// Procura no texto digitado (fórmulas) em vez do valor exibido.
    pub in_formulas: bool,
}

/// Uma célula achada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    pub row: u32,
    pub col: u32,
}

/// `hit` do `ops.ts`.
pub fn is_hit(text: &str, query: &str, options: SearchOptions) -> bool {
    let (a, b) = if options.match_case {
        (text.to_string(), query.to_string())
    } else {
        (text.to_lowercase(), query.to_lowercase())
    };
    if options.whole_cell {
        a == b
    } else {
        !b.is_empty() && a.contains(&b)
    }
}

/// A consulta preparada uma vez: o `hit` do `ops.ts` sem as duas cópias em
/// minúsculas por célula. Bate exatamente onde o [`is_hit`] bate.
pub(super) struct Consulta {
    texto: String,
    minusculas: String,
    opcoes: SearchOptions,
    /// Só pode bater num número se todo caractere da consulta pode aparecer
    /// num número escrito. O valor (`to_text`) tem algarismo, `.`, `-`, `+`,
    /// `e` ou é `#NUM!` (não finito); o texto digitado de um número
    /// (`to_js_string`) tem os mesmos, ou `Infinity`/`NaN`.
    numero_pode_bater: bool,
}

impl Consulta {
    fn new(query: &str, opcoes: SearchOptions) -> Self {
        let minusculas = query.to_lowercase();
        let possiveis = if opcoes.in_formulas { "0123456789.-+einftya" } else { "0123456789.-+e#num!" };
        let numero_pode_bater = minusculas.chars().all(|c| possiveis.contains(c));
        Consulta { texto: query.to_string(), minusculas, opcoes, numero_pode_bater }
    }

    fn bate(&self, texto: &str) -> bool {
        if self.opcoes.match_case {
            return if self.opcoes.whole_cell { texto == self.texto } else { !self.texto.is_empty() && texto.contains(&self.texto) };
        }
        // Tudo ASCII: compara sem montar a cópia em minúsculas.
        if texto.is_ascii() && self.minusculas.is_ascii() {
            let (a, b) = (texto.as_bytes(), self.minusculas.as_bytes());
            if self.opcoes.whole_cell {
                return a.eq_ignore_ascii_case(b);
            }
            return !b.is_empty() && a.len() >= b.len() && a.windows(b.len()).any(|w| w.eq_ignore_ascii_case(b));
        }
        is_hit(texto, &self.texto, self.opcoes)
    }

    /// O número bate? Como o valor (`to_text`) ou como o texto digitado
    /// (`to_js_string`), conforme a busca. Inteiro é escrito direto num
    /// buffer, sem alocar — é igual nos dois abaixo de 1e15.
    fn numero_bate(&self, n: f64, cache: &mut HashMap<u64, bool>) -> bool {
        if !self.numero_pode_bater {
            return false;
        }
        if n.fract() == 0.0 && n.abs() < 1e15 {
            let mut buf = [0u8; 20];
            let mut i = buf.len();
            let mut v = n.abs() as u64;
            loop {
                i -= 1;
                buf[i] = b'0' + (v % 10) as u8;
                v /= 10;
                if v == 0 {
                    break;
                }
            }
            if n < 0.0 {
                i -= 1;
                buf[i] = b'-';
            }
            return self.bate(std::str::from_utf8(&buf[i..]).unwrap_or_default());
        }
        *cache.entry(n.to_bits()).or_insert_with(|| {
            let escrito = if self.opcoes.in_formulas { crate::celulas::number::to_js_string(n) } else { to_text(&Value::Number(n)) };
            self.bate(&escrito)
        })
    }
}

/// O que a busca lê de uma coluna, sem o `Sheet` (que tem o cálculo atrás
/// de um `RefCell` e não atravessa thread): só o que é dado guardado.
pub(super) struct Colunas<'a> {
    data: &'a crate::planilha::data_table::DataTable,
    formulas: &'a crate::formula::formula_table::FormulaTable,
    pool: &'a crate::celulas::pool::StringPool,
    textos: &'a TextosCrus,
    /// As células do `Map`, que a busca já olhou à parte.
    vistas: &'a HashSet<u64>,
}

impl Colunas<'_> {
    /// Uma coluna inteira: o que bate, e as fórmulas que precisam calcular
    /// antes de responder (sujas, ou com resultado lógico/erro/vazio).
    fn varrer(&self, col: u32, consulta: &Consulta) -> (Vec<SearchMatch>, Vec<SearchMatch>) {
        let mut achadas = Vec::new();
        let mut calcular = Vec::new();
        let mut por_id: HashMap<i32, bool> = HashMap::new();
        let mut por_numero: HashMap<u64, bool> = HashMap::new();
        let em_formulas = consulta.opcoes.in_formulas;
        let fora = |row: u32| self.vistas.is_empty() || !self.vistas.contains(&Sheet::cell_key(row, col));
        self.data.for_each_cru_da_coluna(col, &mut |row, cru| {
            if !fora(row) {
                return;
            }
            // O texto digitado que não volta do valor ("007", data) mora à parte.
            let digitado = if em_formulas { self.textos.get(&Sheet::cell_key(row, col)) } else { None };
            let bate = match (digitado, cru) {
                (Some(t), _) => consulta.bate(t),
                (None, Cru::Numero(n)) => consulta.numero_bate(n, &mut por_numero),
                (None, Cru::Texto(id)) => *por_id.entry(id).or_insert_with(|| consulta.bate(self.pool.get(id))),
            };
            if bate {
                achadas.push(SearchMatch { row, col });
            }
        });
        self.formulas.for_each_in_range(0, MAX_ROWS - 1, col, col, |row, _| {
            if !fora(row) {
                return;
            }
            if em_formulas {
                if consulta.bate(&self.formulas.raw_at(row, col).unwrap_or_default()) {
                    achadas.push(SearchMatch { row, col });
                }
                return;
            }
            // A fórmula já calculada guarda o resultado cru.
            let bate = match self.formulas.cru_at(row, col) {
                Some(Cru::Numero(n)) => consulta.numero_bate(n, &mut por_numero),
                Some(Cru::Texto(id)) => *por_id.entry(id).or_insert_with(|| consulta.bate(self.pool.get(id))),
                None => {
                    calcular.push(SearchMatch { row, col });
                    false
                }
            };
            if bate {
                achadas.push(SearchMatch { row, col });
            }
        });
        (achadas, calcular)
    }
}


/// `findAll`: toda célula com conteúdo cujo texto bate, em ordem de linha e
/// coluna. Passa pelas três moradas de conteúdo — o literal colunar, a
/// coluna de fórmulas empacotadas e o resto (`cells`). Célula só de estilo
/// não tem texto, e texto vazio nunca bate.
///
/// Medido em 23/09/2026 nas planilhas do usuário: 4 s por busca na de 2
/// milhões de linhas, 18 s buscando "s" em fórmulas na de 97 milhões de
/// células — cada célula virava texto, e o texto e a consulta viravam
/// minúsculas, com três cópias por célula. Agora cada texto distinto do pool
/// é conferido uma vez, número só é escrito quando a consulta poderia bater
/// num número, e as colunas são varridas em paralelo.
pub fn find_all_com(sheet: &Sheet, query: &str, options: SearchOptions, valor: impl Fn(u32, u32) -> Value) -> Vec<SearchMatch> {
    if query.is_empty() {
        return Vec::new();
    }
    let consulta = Consulta::new(query, options);
    let mut out = Vec::new();
    let vistas: HashSet<u64> = sheet.cells.keys().copied().collect();
    for &k in sheet.cells.keys() {
        let (row, col) = Sheet::key_to_coord(k);
        let texto = if options.in_formulas { sheet.get_raw(row, col) } else { to_text(&valor(row, col)) };
        if consulta.bate(&texto) {
            out.push(SearchMatch { row, col });
        }
    }
    let mut colunas = sheet.data.colunas();
    colunas.extend(sheet.formulas.colunas());
    colunas.sort_unstable();
    colunas.dedup();
    let leitor = Colunas { data: &sheet.data, formulas: &sheet.formulas, pool: &sheet.pool, textos: &sheet.textos, vistas: &vistas };
    for (achadas, calcular) in crate::paralelismo::por_coluna(&colunas, |col| leitor.varrer(col, &consulta)) {
        out.extend(achadas);
        // O que precisa calcular é aqui, em série: o cálculo não atravessa thread.
        for m in calcular {
            if consulta.bate(&to_text(&valor(m.row, m.col))) {
                out.push(m);
            }
        }
    }
    out.sort_unstable_by_key(|m| (m.row, m.col));
    out
}

/// `findAll` lendo o valor do próprio `Sheet`.
pub fn find_all(sheet: &Sheet, query: &str, options: SearchOptions) -> Vec<SearchMatch> {
    find_all_com(sheet, query, options, |r, c| sheet.get_value(r, c))
}

/// `replaceEvery`: troca toda ocorrência de `query` no texto.
///
/// Sem diferenciar maiúsculas, a busca corre no texto em minúsculas e a troca
/// no original. O `ops.ts` usa o índice de um no outro, o que só é seguro
/// quando o minúsculo tem o mesmo tamanho; aqui cada byte do minúsculo lembra
/// de que letra original veio (`İ` vira dois caracteres), para o corte nunca
/// cair no meio de uma letra — no Rust isso seria pânico.
pub fn replace_every(text: &str, query: &str, replacement: &str, match_case: bool) -> String {
    if query.is_empty() {
        return text.to_string();
    }
    if match_case {
        return text.replace(query, replacement);
    }
    let mut hay = String::with_capacity(text.len());
    // Para cada byte de `hay`: onde começa e onde acaba a letra original.
    let mut inicio: Vec<usize> = Vec::with_capacity(text.len());
    let mut fim: Vec<usize> = Vec::with_capacity(text.len());
    for (i, ch) in text.char_indices() {
        let antes = hay.len();
        hay.extend(ch.to_lowercase());
        for _ in antes..hay.len() {
            inicio.push(i);
            fim.push(i + ch.len_utf8());
        }
    }
    let needle = query.to_lowercase();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut feito) = (0usize, 0usize);
    while let Some(p) = hay[i..].find(&needle) {
        let at = i + p;
        let de = inicio[at].max(feito);
        out.push_str(&text[feito..de]);
        out.push_str(replacement);
        i = at + needle.len();
        feito = fim[i - 1];
    }
    out.push_str(&text[feito..]);
    out
}

/// `replaceAll`: substitui nas células achadas, numa transação só (um Ctrl+Z
/// desfaz tudo), pelo mesmo caminho da digitação. Devolve quantas mudaram.
pub fn replace_all_em(sheet: &mut Sheet, achadas: &[SearchMatch], query: &str, replacement: &str, options: SearchOptions) -> usize {
    if achadas.is_empty() {
        return 0;
    }
    sheet.abrir_transacao();
    let mut alteradas = 0;
    for m in achadas {
        let raw = sheet.get_raw(m.row, m.col);
        let next = if options.whole_cell {
            replacement.to_string()
        } else {
            replace_every(&raw, query, replacement, options.match_case)
        };
        // A busca por valor calculado pode encontrar uma fórmula cujo texto
        // bruto não contém o termo. Não registrar uma escrita fictícia.
        if next != raw {
            sheet.set_cell(m.row, m.col, &next);
            alteradas += 1;
        }
    }
    sheet.fechar_transacao();
    alteradas
}

/// `replaceAll` lendo o valor do próprio `Sheet`.
pub fn replace_all(sheet: &mut Sheet, query: &str, replacement: &str, options: SearchOptions) -> usize {
    let achadas = find_all(sheet, query, options);
    replace_all_em(sheet, &achadas, query, replacement, options)
}

