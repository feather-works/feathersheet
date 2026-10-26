//! Tradução da interface (menus, faixa, diálogos, janelas, barra de status).
//!
//! O texto em português escrito no código é a fonte e também a chave: em
//! pt-BR, [`tr`] devolve o próprio texto; em en-US, procura a chave em
//! `src/idiomas/interface.en-US.json` e, se ela faltar, cai no português.
//! O idioma é global ao processo e a tela troca na hora: as funções `view`
//! chamam `tr!` a cada quadro, então basta mudar o idioma (ver [`definir`]).
//!
//! Uso:
//! - `tr!("Salvar")` — texto fixo ou `&str` qualquer (o resultado vive o
//!   mesmo tanto que a entrada; quando traduzido, é `'static`).
//! - `trf!("Linha {} de {total}", n)` — mesmo formato do `format!`, conferido
//!   pelo compilador. A tradução repete os mesmos marcadores (`{}`, `{total}`,
//!   `{:.2}`...), em qualquer ordem; marcadores iguais mantêm a ordem relativa.
//!
//! As descrições das funções da planilha ficam em
//! `src/idiomas/funcoes.en-US.json`, lidas por [`ajuda_funcao`]; o motor
//! (`feathersheet-core`) continua só em português e sem dependência nova.
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

const JSON_INTERFACE_EN: &str = include_str!("../../../src/idiomas/interface.en-US.json");
const JSON_INTERFACE_ES: &str = include_str!("../../../src/idiomas/interface.es-ES.json");
const JSON_FUNCOES_PT: &str = include_str!("../../../src/idiomas/funcoes.pt-BR.json");
const JSON_FUNCOES_EN: &str = include_str!("../../../src/idiomas/funcoes.en-US.json");
const JSON_FUNCOES_ES: &str = include_str!("../../../src/idiomas/funcoes.es-ES.json");

/// Os idiomas da interface. O português é a fonte (as chaves).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lingua { Pt, En, Es }

impl Lingua {
    pub fn de_codigo(codigo: &str) -> Self {
        if codigo.starts_with("en") { Lingua::En }
        else if codigo.starts_with("es") { Lingua::Es }
        else { Lingua::Pt }
    }
    pub fn codigo(self) -> &'static str {
        match self { Lingua::Pt => "pt-BR", Lingua::En => "en-US", Lingua::Es => "es-ES" }
    }
    fn indice(self) -> u8 { self as u8 }
    fn de_indice(i: u8) -> Self {
        match i { 1 => Lingua::En, 2 => Lingua::Es, _ => Lingua::Pt }
    }
}

static LINGUA: AtomicU8 = AtomicU8::new(0);

#[cfg(test)]
thread_local! {
    /// Nos testes (paralelos) o global fica sempre em pt-BR; um teste que
    /// queira ver a tela em outro idioma liga só a própria thread.
    static LINGUA_THREAD: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// Chamado na abertura e ao trocar o idioma nas Configurações.
pub fn definir(idioma: &str) {
    if cfg!(test) { return; }
    LINGUA.store(Lingua::de_codigo(idioma).indice(), Ordering::Relaxed);
    // F12 e barra de baixo do feather-ui usam o mesmo dicionário.
    feather_ui::idioma::definir_tradutor(|pt| {
        if lingua() == Lingua::Pt { return None; }
        dicionario().get(pt).map(String::as_str)
    });
}

/// Só para testes: troca o idioma nesta thread.
#[cfg(test)]
pub fn definir_na_thread(idioma: &str) {
    LINGUA_THREAD.with(|c| c.set(Lingua::de_codigo(idioma).indice()));
}

/// O idioma da interface agora.
pub fn lingua() -> Lingua {
    #[cfg(test)]
    { Lingua::de_indice(LINGUA_THREAD.with(|c| c.get())) }
    #[cfg(not(test))]
    { Lingua::de_indice(LINGUA.load(Ordering::Relaxed)) }
}

/// Dicionário português → idioma atual da interface (vazio em português).
pub fn dicionario() -> &'static HashMap<String, String> {
    dicionario_de(lingua())
}

/// Chaves que começam com `//` são comentários do arquivo e não entram.
pub fn dicionario_de(l: Lingua) -> &'static HashMap<String, String> {
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    static ES: OnceLock<HashMap<String, String>> = OnceLock::new();
    static PT: OnceLock<HashMap<String, String>> = OnceLock::new();
    let ler = |json: &str| -> HashMap<String, String> {
        let mapa: HashMap<String, String> = serde_json::from_str(json)
            .expect("arquivo de idioma da interface inválido");
        mapa.into_iter().filter(|(k, _)| !k.starts_with("//")).collect()
    };
    match l {
        Lingua::Pt => PT.get_or_init(HashMap::new),
        Lingua::En => EN.get_or_init(|| ler(JSON_INTERFACE_EN)),
        Lingua::Es => ES.get_or_init(|| ler(JSON_INTERFACE_ES)),
    }
}

/// Traduz um texto da interface para o idioma atual; sem tradução, devolve
/// o próprio texto em português.
pub fn tr(pt: &str) -> &str {
    if lingua() == Lingua::Pt { return pt; }
    match dicionario().get(pt) {
        Some(t) => t.as_str(),
        None => pt,
    }
}

/// O texto em português de um texto já traduzido (o inverso de [`tr`]);
/// sem correspondência, devolve o próprio texto. Serve a quem decide algo
/// pelo rótulo exibido (ícone do item de menu, por exemplo).
pub fn original(mostrado: &str) -> &str {
    static INVERSOS: OnceLock<[HashMap<&'static str, &'static str>; 3]> = OnceLock::new();
    let l = lingua();
    if l == Lingua::Pt { return mostrado; }
    let inv = INVERSOS.get_or_init(|| [Lingua::Pt, Lingua::En, Lingua::Es].map(|l| {
        let mut m = HashMap::new();
        for (pt, t) in dicionario_de(l) { m.entry(t.as_str()).or_insert(pt.as_str()); }
        m
    }));
    inv[l.indice() as usize].get(mostrado).copied().unwrap_or(mostrado)
}

/// Versão traduzida de uma lista fixa de opções (listas suspensas pedem
/// `&'static [&'static str]`). Cada lista é traduzida uma vez por idioma.
pub fn lista(pt: &'static [&'static str]) -> &'static [&'static str] {
    let l = lingua();
    if l == Lingua::Pt { return pt; }
    type Cache = std::sync::Mutex<Vec<((usize, usize, u8), &'static [&'static str])>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let chave = (pt.as_ptr() as usize, pt.len(), l.indice());
    let mut cache = CACHE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, lista)) = cache.iter().find(|(k, _)| *k == chave) { return lista; }
    let traduzida: &'static [&'static str] = Box::leak(pt.iter().map(|s| tr(s)).collect::<Vec<_>>().into_boxed_slice());
    cache.push((chave, traduzida));
    traduzida
}

/// Volta da tela para a chave: dentre `chaves` (em português), a que,
/// traduzida, é o texto `mostrado` (listas suspensas devolvem o texto exibido).
pub fn chave_de<'a>(chaves: &[&'a str], mostrado: &str) -> Option<&'a str> {
    chaves.iter().copied().find(|pt| *pt == mostrado || tr(pt) == mostrado)
}

/// Um pedaço do modelo de formato: texto literal ou marcador `{...}`.
#[derive(Debug, Clone, PartialEq)]
enum Peca { Texto(String), Marcador(String) }

/// Divide o modelo do `format!` em texto e marcadores (`{{`/`}}` viram chave).
fn pecas(modelo: &str) -> Vec<Peca> {
    let mut saida = Vec::new();
    let mut texto = String::new();
    let mut it = modelo.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => { it.next(); texto.push('{'); }
            '}' if it.peek() == Some(&'}') => { it.next(); texto.push('}'); }
            '{' => {
                if !texto.is_empty() { saida.push(Peca::Texto(std::mem::take(&mut texto))); }
                let mut m = String::from("{");
                for d in it.by_ref() { m.push(d); if d == '}' { break; } }
                saida.push(Peca::Marcador(m));
            }
            _ => texto.push(c),
        }
    }
    if !texto.is_empty() { saida.push(Peca::Texto(texto)); }
    saida
}

/// Marcadores do modelo, na ordem (usado nos testes de consistência).
#[cfg_attr(not(test), allow(dead_code))]
pub fn marcadores(modelo: &str) -> Vec<String> {
    pecas(modelo).into_iter().filter_map(|p| match p {
        Peca::Marcador(m) => Some(m), Peca::Texto(_) => None,
    }).collect()
}

/// Recupera o valor de cada marcador comparando o texto já formatado em
/// português com o modelo. `None` se o texto não casar com o modelo.
fn valores(modelo: &[Peca], formatado: &str) -> Option<Vec<String>> {
    let mut resto = formatado;
    let mut valores = Vec::new();
    let mut i = 0;
    while i < modelo.len() {
        match &modelo[i] {
            Peca::Texto(t) => { resto = resto.strip_prefix(t.as_str())?; i += 1; }
            Peca::Marcador(_) => {
                // O valor vai até o próximo texto literal (ou até o fim).
                match modelo.get(i + 1) {
                    Some(Peca::Texto(prox)) => {
                        let ultimo = i + 2 >= modelo.len();
                        let fim = if ultimo { resto.rfind(prox.as_str())? } else { resto.find(prox.as_str())? };
                        if ultimo && fim + prox.len() != resto.len() { return None; }
                        valores.push(resto[..fim].to_owned());
                        resto = &resto[fim..];
                    }
                    // Dois marcadores colados: não dá para separar com segurança.
                    Some(Peca::Marcador(_)) => return None,
                    None => { valores.push(resto.to_owned()); resto = ""; }
                }
                i += 1;
            }
        }
    }
    resto.is_empty().then_some(valores)
}

/// Monta a versão traduzida de um texto formatado: `modelo_pt` é o literal
/// do `format!`, `formatado` é o resultado em português.
pub fn traduzir_formatado(modelo_pt: &str, formatado: String) -> String {
    if lingua() == Lingua::Pt { return formatado; }
    let Some(modelo_en) = dicionario().get(modelo_pt) else { return formatado; };
    let pt = pecas(modelo_pt);
    let Some(vals) = valores(&pt, &formatado) else { return formatado; };
    let marc_pt: Vec<&str> = pt.iter().filter_map(|p| match p {
        Peca::Marcador(m) => Some(m.as_str()), Peca::Texto(_) => None,
    }).collect();
    let mut usados = vec![false; marc_pt.len()];
    let mut saida = String::with_capacity(formatado.len() + 16);
    for p in pecas(modelo_en) {
        match p {
            Peca::Texto(t) => saida.push_str(&t),
            Peca::Marcador(m) => {
                // Primeiro marcador ainda não usado com o mesmo texto.
                let achado = marc_pt.iter().enumerate()
                    .position(|(j, x)| !usados[j] && *x == m)
                    .or_else(|| marc_pt.iter().position(|x| *x == m));
                match achado {
                    Some(j) => { usados[j] = true; saida.push_str(&vals[j]); }
                    None => return formatado,
                }
            }
        }
    }
    saida
}

/// `tr!(texto)`: traduz para o idioma atual da interface.
#[macro_export]
macro_rules! tr {
    ($e:expr) => { $crate::idioma::tr($e) };
}

/// `trf!(modelo, args...)`: como `format!`, traduzindo o modelo.
#[macro_export]
macro_rules! trf {
    ($modelo:literal $($resto:tt)*) => {
        $crate::idioma::traduzir_formatado($modelo, format!($modelo $($resto)*))
    };
}

/// Descrição e argumentos de uma função da planilha no idioma da interface.
/// O motor guarda só o português; nos outros idiomas a descrição vem de
/// `funcoes.<código>.json` (por nome canônico) e cada argumento, do mapa de
/// argumentos (preservando `[ ]` de opcional e números no fim).
pub fn ajuda_funcao(nome: &str) -> Option<(Vec<String>, String)> {
    ajuda_funcao_em(nome, lingua().codigo())
}

/// Como [`ajuda_funcao`], com o idioma dado (a Ajuda F1 monta um catálogo por idioma).
pub fn ajuda_funcao_em(nome: &str, idioma: &str) -> Option<(Vec<String>, String)> {
    let ajuda = feathersheet_core::funcoes::function_help::help_for(nome)?;
    let l = Lingua::de_codigo(idioma);
    if l == Lingua::Pt {
        return Some((ajuda.args.iter().map(|a| (*a).to_owned()).collect(), ajuda.doc.to_owned()));
    }
    let doc = descricao(l, nome).unwrap_or_else(|| ajuda.doc.to_owned());
    let args = ajuda.args.iter().map(|a| argumento_em(l, a)).collect();
    Some((args, doc))
}

/// Descrição no idioma; sem ela, a do inglês.
fn descricao(l: Lingua, canonico: &str) -> Option<String> {
    funcoes_de(l).descricoes.get(canonico)
        .or_else(|| funcoes_de(Lingua::En).descricoes.get(canonico)).cloned()
}

/// Argumentos e descrição de uma ajuda já obtida do motor (por exemplo, a
/// do assistente de fórmula), no idioma da interface.
pub fn ajuda_traduzida(h: &feathersheet_core::funcoes::function_help::FunctionHelp) -> (Vec<String>, String) {
    let l = lingua();
    if l == Lingua::Pt {
        return (h.args.iter().map(|a| (*a).to_owned()).collect(), h.doc.to_owned());
    }
    let nome = feathersheet_core::funcoes::function_help::all().iter()
        .find(|(_, x)| std::ptr::eq(x.doc, h.doc) || x.doc == h.doc).map(|(n, _)| *n);
    let doc = nome.and_then(|n| descricao(l, n)).unwrap_or_else(|| h.doc.to_owned());
    (h.args.iter().map(|a| argumento_em(l, a)).collect(), doc)
}

/// Assinatura `NOME(arg1; arg2)` no idioma da interface: o nome do de-para
/// e o separador do idioma (vírgula só em inglês).
pub fn assinatura(nome: &str, h: &feathersheet_core::funcoes::function_help::FunctionHelp) -> String {
    let l = lingua();
    if l == Lingua::Pt { return feathersheet_core::funcoes::function_help::signature(nome, h); }
    let separador = if l == Lingua::En { ", " } else { "; " };
    format!("{}({})", nome_funcao(nome), ajuda_traduzida(h).0.join(separador))
}

/// Nome de categoria de funções no idioma da interface.
pub fn categoria_funcao(pt: &str) -> &str {
    let l = lingua();
    if l == Lingua::Pt { return pt; }
    funcoes_de(l).categorias.get(pt).map(String::as_str).unwrap_or(pt)
}

/// Conteúdo de `funcoes.<código>.json`.
#[derive(serde::Deserialize, Default)]
pub struct FuncoesIdioma {
    #[serde(default)]
    pub categorias: HashMap<String, String>,
    #[serde(default)]
    pub argumentos: HashMap<String, String>,
    #[serde(default)]
    pub descricoes: HashMap<String, String>,
    /// De-para dos nomes: canônico (inglês) → nome no idioma.
    #[serde(default)]
    pub nomes: HashMap<String, String>,
    /// O inverso de `nomes` (nome no idioma, maiúsculo → canônico).
    #[serde(skip)]
    pub canonicos: HashMap<String, String>,
}

pub fn funcoes_de(l: Lingua) -> &'static FuncoesIdioma {
    static F: OnceLock<[FuncoesIdioma; 3]> = OnceLock::new();
    let todos = F.get_or_init(|| [JSON_FUNCOES_PT, JSON_FUNCOES_EN, JSON_FUNCOES_ES].map(|json| {
        let mut f: FuncoesIdioma = serde_json::from_str(json).expect("funcoes.<idioma>.json inválido");
        f.canonicos = f.nomes.iter().map(|(c, n)| (n.to_uppercase(), c.clone())).collect();
        f
    }));
    &todos[l.indice() as usize]
}

#[cfg(test)]
pub fn funcoes_en() -> &'static FuncoesIdioma {
    funcoes_de(Lingua::En)
}

/// O nome que o idioma atual mostra para uma função digitada em qualquer
/// idioma (SOMA, SUM ou SUMA → o do idioma). Sem de-para, o próprio nome.
pub fn nome_funcao(nome: &str) -> String {
    let maiusculo = nome.to_uppercase();
    let canonico = canonico_funcao(&maiusculo).unwrap_or(maiusculo.clone());
    funcoes_de(lingua()).nomes.get(&canonico).cloned().unwrap_or(maiusculo)
}

/// O nome canônico (inglês) de um nome de função em qualquer dos idiomas.
pub fn canonico_funcao(nome: &str) -> Option<String> {
    let maiusculo = nome.to_uppercase();
    if funcoes_de(Lingua::En).nomes.contains_key(&maiusculo) { return Some(maiusculo); }
    if let Some(c) = feathersheet_core::funcoes::functions::canonical(&maiusculo) { return Some(c.to_owned()); }
    [Lingua::Pt, Lingua::Es].iter().find_map(|l| funcoes_de(*l).canonicos.get(&maiusculo).cloned())
}

/// Fórmula para mostrar: os nomes das funções no idioma da interface.
pub fn mostrar_formula(raw: &str) -> String {
    if !raw.starts_with('=') { return raw.to_owned(); }
    let l = lingua();
    feathersheet_core::celulas::locale::traduzir_funcoes(raw, &|n| {
        let c = canonico_funcao(n)?;
        let alvo = funcoes_de(l).nomes.get(&c)?;
        (!alvo.eq_ignore_ascii_case(n)).then(|| alvo.clone())
    })
}

/// Fórmula digitada para o motor: nomes em espanhol viram o canônico (o
/// motor entende português e inglês, não espanhol).
pub fn formula_para_motor(raw: &str) -> String {
    if lingua() != Lingua::Es || !raw.starts_with('=') { return raw.to_owned(); }
    feathersheet_core::celulas::locale::traduzir_funcoes(raw, &|n| {
        let m = n.to_uppercase();
        // Nome que o motor já entende (pt ou en) fica como está.
        if funcoes_de(Lingua::En).nomes.contains_key(&m)
            || feathersheet_core::funcoes::functions::canonical(&m).is_some() { return None; }
        funcoes_de(Lingua::Es).canonicos.get(&m).cloned()
    })
}

fn argumento_em(l: Lingua, pt: &str) -> String {
    argumento_opt(l, pt).unwrap_or_else(|| pt.to_owned())
}

/// Tradução de um argumento em inglês, se o mapa a cobrir.
#[cfg(test)]
pub fn argumento_en_opt(pt: &str) -> Option<String> {
    argumento_opt(Lingua::En, pt)
}

/// Tradução de um argumento, se o mapa do idioma a cobrir (direta, entre
/// colchetes de opcional, ou com número no fim: `valor2` → `value2`).
fn argumento_opt(l: Lingua, pt: &str) -> Option<String> {
    let mapa = &funcoes_de(l).argumentos;
    if let Some(t) = mapa.get(pt) { return Some(t.clone()); }
    let (abre, meio, fecha) = match pt.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        Some(m) => ("[", m, "]"),
        None => ("", pt, ""),
    };
    if let Some(t) = mapa.get(meio) { return Some(format!("{abre}{t}{fecha}")); }
    let base = meio.trim_end_matches(|c: char| c.is_ascii_digit());
    let num = &meio[base.len()..];
    mapa.get(base).map(|t| format!("{abre}{t}{num}{fecha}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portugues_devolve_o_proprio_texto() {
        definir_na_thread("pt-BR");
        assert_eq!(tr("Salvar"), "Salvar");
        assert_eq!(trf!("Linha {} de {}", 3, 9), "Linha 3 de 9");
    }

    #[test]
    fn ingles_traduz_e_cai_no_portugues_sem_chave() {
        definir_na_thread("en-US");
        assert_eq!(tr("Salvar"), "Save");
        assert_eq!(tr("texto que não existe no dicionário"), "texto que não existe no dicionário");
        definir_na_thread("pt-BR");
    }

    #[test]
    fn formatado_recupera_os_valores_dos_marcadores() {
        let modelo = pecas("{} de {total} ({:.1}%)");
        let v = valores(&modelo, "3 de 10 (30.0%)").unwrap();
        assert_eq!(v, vec!["3", "10", "30.0"]);
        assert!(valores(&pecas("A{}B"), "XYZ").is_none());
    }

    #[test]
    fn dicionario_mantem_os_mesmos_marcadores() {
        for (pt, en) in dicionario() {
            let mut a = marcadores(pt); a.sort();
            let mut b = marcadores(en); b.sort();
            assert_eq!(a, b, "marcadores diferentes em {pt:?} → {en:?}");
        }
    }

    #[test]
    fn toda_funcao_tem_descricao_e_argumentos_em_ingles() {
        use feathersheet_core::funcoes::function_help::all;
        let f = funcoes_en();
        let mut faltam = Vec::new();
        for (nome, ajuda) in all() {
            if !f.descricoes.contains_key(*nome) { faltam.push(format!("descrição de {nome}")); }
            for a in ajuda.args {
                if *a != "…" && argumento_en_opt(a).is_none() {
                    faltam.push(format!("argumento {a} de {nome}"));
                }
            }
        }
        assert!(faltam.is_empty(), "faltam traduções: {faltam:#?}");
    }

    #[test]
    fn categorias_de_funcoes_tem_ingles() {
        for (c, _) in feathersheet_core::funcoes::function_help::CATEGORIES {
            assert!(funcoes_en().categorias.contains_key(*c), "categoria {c}");
        }
    }

    /// Lê o literal de string Rust que começa em `ini` (logo após a aspa);
    /// devolve o texto já sem escapes e onde ele termina.
    fn literal(fonte: &str, ini: usize) -> Option<(String, usize)> {
        let b = fonte.as_bytes();
        let mut texto = String::new();
        let mut i = ini;
        while i < b.len() {
            match b[i] {
                b'"' => return Some((texto, i + 1)),
                b'\\' => {
                    let prox = fonte[i + 1..].chars().next()?;
                    i += 1 + prox.len_utf8();
                    match prox {
                        'n' => texto.push('\n'),
                        't' => texto.push('\t'),
                        '0' => texto.push('\0'),
                        'u' => {
                            let fim = fonte[i..].find('}')? + i;
                            let cod = u32::from_str_radix(&fonte[i + 1..fim], 16).ok()?;
                            texto.push(char::from_u32(cod)?);
                            i = fim + 1;
                        }
                        '\n' => {
                            while i < b.len() && (b[i] as char).is_whitespace() { i += 1; }
                        }
                        c => texto.push(c),
                    }
                }
                _ => {
                    let c = fonte[i..].chars().next()?;
                    texto.push(c);
                    i += c.len_utf8();
                }
            }
        }
        None
    }

    /// Todo `tr!("…")` e `trf!("…")` da interface (fora deste arquivo) com
    /// um texto fixo, e onde ele aparece.
    fn chaves_usadas() -> Vec<(String, String)> {
        fn varrer(dir: &std::path::Path, saida: &mut Vec<(String, String)>) {
            let Ok(entradas) = std::fs::read_dir(dir) else { return };
            for e in entradas.flatten() {
                let p = e.path();
                if p.is_dir() { varrer(&p, saida); continue; }
                if p.extension().and_then(|x| x.to_str()) != Some("rs") || p.ends_with("idioma.rs") { continue; }
                let Ok(fonte) = std::fs::read_to_string(&p) else { continue };
                for marca in ["tr!(", "trf!("] {
                    let mut de = 0;
                    while let Some(pos) = fonte[de..].find(marca) {
                        let ini = de + pos;
                        de = ini + marca.len();
                        // `include_str!(` e afins não contam.
                        if fonte[..ini].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') { continue; }
                        let resto = &fonte[de..];
                        let pulo = resto.len() - resto.trim_start().len();
                        if !resto[pulo..].starts_with('"') { continue; }
                        if let Some((texto, _)) = literal(&fonte, de + pulo + 1) {
                            let linha = fonte[..ini].matches('\n').count() + 1;
                            saida.push((texto, format!("{}:{linha}", p.display())));
                        }
                    }
                }
            }
        }
        let mut saida = Vec::new();
        varrer(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut saida);
        saida
    }

    /// O teste pedido: com en-US, toda chave usada na interface tem tradução.
    #[test]
    fn toda_chave_usada_na_interface_tem_traducao() {
        let usadas = chaves_usadas();
        assert!(usadas.len() > 2000, "a varredura achou só {} chaves", usadas.len());
        // Inglês e espanhol, cada um com o seu arquivo.
        let faltam: Vec<_> = [Lingua::En, Lingua::Es].iter().flat_map(|l| usadas.iter()
            .filter(|(k, _)| !dicionario_de(*l).contains_key(k))
            .map(move |(k, onde)| format!("{}: {onde}: {k:?}", l.codigo()))).collect();
        assert!(faltam.is_empty(), "{} chave(s) sem tradução:\n{}", faltam.len(), faltam.join("\n"));
    }

    /// Com en-US, toda chave usada sai diferente do português, salvo as que
    /// são iguais nas duas línguas (nomes próprios, siglas, unidades).
    #[test]
    fn com_ingles_nenhuma_chave_usada_fica_em_portugues() {
        definir_na_thread("en-US");
        let iguais: Vec<_> = chaves_usadas().into_iter()
            .filter(|(k, _)| tr(k) == k && k.chars().any(|c| c.is_alphabetic() && !c.is_ascii()))
            .map(|(k, onde)| format!("{onde}: {k:?}")).collect();
        definir_na_thread("pt-BR");
        assert!(iguais.is_empty(), "continuam em português:\n{}", iguais.join("\n"));
    }

    /// Espanhol: as chaves com acento do português não podem sair idênticas
    /// (palavras iguais nas duas línguas, como "Área", são aceitas na lista).
    #[test]
    fn com_espanhol_quase_nada_fica_em_portugues() {
        definir_na_thread("es-ES");
        let iguais: Vec<_> = chaves_usadas().into_iter()
            .filter(|(k, _)| tr(k) == k && (k.contains('ã') || k.contains('ç') || k.contains('õ')))
            .map(|(k, onde)| format!("{onde}: {k:?}")).collect();
        assert_eq!(tr("Salvar"), "Guardar");
        definir_na_thread("pt-BR");
        assert!(iguais.is_empty(), "continuam em português:\n{}", iguais.join("\n"));
    }

    #[test]
    fn de_para_dos_nomes_nos_tres_idiomas() {
        definir_na_thread("es-ES");
        assert_eq!(nome_funcao("SOMA"), "SUMA");
        assert_eq!(nome_funcao("vlookup"), "BUSCARV");
        assert_eq!(mostrar_formula("=SE(A1>0;SOMA(B1:B2);0)"), "=SI(A1>0;SUMA(B1:B2);0)");
        assert_eq!(formula_para_motor("=SI(A1>0;SUMA(B1:B2);0)"), "=IF(A1>0;SUM(B1:B2);0)");
        // Nome que o motor já entende fica como foi digitado.
        assert_eq!(formula_para_motor("=SOMA(1;2)"), "=SOMA(1;2)");
        definir_na_thread("en-US");
        assert_eq!(mostrar_formula("=SOMA(1;MÉDIA(2;3))"), "=SUM(1;AVERAGE(2;3))");
        definir_na_thread("pt-BR");
        assert_eq!(mostrar_formula("=SUM(1;AVERAGE(2;3))"), "=SOMA(1;MÉDIA(2;3))");
        assert_eq!(nome_funcao("SUMBYCOLOR"), "SOMAR.POR.COR");
    }

    #[test]
    fn toda_funcao_tem_descricao_e_nome_em_espanhol() {
        let es = funcoes_de(Lingua::Es);
        let faltam: Vec<&str> = feathersheet_core::funcoes::function_help::all().iter()
            .map(|(n, _)| *n)
            .filter(|n| !es.descricoes.contains_key(*n) || !es.nomes.contains_key(*n)).collect();
        assert!(faltam.is_empty(), "sem descrição/nome em espanhol: {faltam:?}");
    }

    #[test]
    fn faixa_e_menus_ja_tem_traducao_no_dicionario() {
        // A faixa e o menu Editar ainda escrevem o texto direto (estão sendo
        // refeitos por outra pessoa); o dicionário já cobre seus rótulos.
        for pt in ["Área de transferência", "Pincel", "Classificar e filtrar", "Tabela dinâmica",
            "Gráficos", "Mesclar e centralizar", "Definir área de impressão", "Configurar Página",
            "Boas-vindas ao FeatherSheet", "Ferramentas de dados", "Proteger intervalo"] {
            assert!(dicionario_de(Lingua::En).contains_key(pt), "{pt}");
            assert!(dicionario_de(Lingua::Es).contains_key(pt), "{pt}");
        }
    }
}
