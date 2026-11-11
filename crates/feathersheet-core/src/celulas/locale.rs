//! Idioma e regionalização: como os números aparecem (decimal, milhar,
//! moeda). A configuração vem do arquivo de língua do app
//! (`src/idiomas/<código>.json`, bloco `idioma`) pelo comando `idioma` da
//! ponte — qualquer língua, não uma lista fechada. pt-BR e en-US já vêm
//! prontas aqui ([`LocaleConfig::embutida`]), para os testes e para quem
//! ainda não configurou nada.
//!
//! O idioma é do processo (vale para todas as janelas). Os separadores
//! decimal e de milhar, lidos a cada número formatado, ficam em atômicos; o
//! resto, atrás de um `RwLock`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{OnceLock, RwLock, RwLockReadGuard};

/// A configuração regional de uma língua.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleConfig {
    /// O código do arquivo de língua (`pt-BR`, `en-US`, `de-DE`…).
    pub code: String,
    pub name: String,
    pub decimal_separator: char,
    pub thousands_separator: char,
    pub formula_arg_separator: char,
    pub currency_symbol: String,
    pub currency_code: String,
    /// O símbolo vem antes do número (`R$ 10`, `$10`) ou depois (`10 €`).
    pub currency_before: bool,
    /// Espaço não separável entre o símbolo e o número (`R$ 10` × `$10`).
    pub currency_space: bool,
}

impl LocaleConfig {
    /// Português do Brasil: `1.250,50`, `R$ 1.250,50`, fórmulas com `;`.
    pub fn pt_br() -> Self {
        LocaleConfig {
            code: "pt-BR".into(),
            name: "Português (Brasil)".into(),
            decimal_separator: ',',
            thousands_separator: '.',
            formula_arg_separator: ';',
            currency_symbol: "R$".into(),
            currency_code: "BRL".into(),
            currency_before: true,
            currency_space: true,
        }
    }

    /// Inglês dos EUA: `1,250.50`, `$1,250.50`, fórmulas com `,`.
    pub fn en_us() -> Self {
        LocaleConfig {
            code: "en-US".into(),
            name: "English (United States)".into(),
            decimal_separator: '.',
            thousands_separator: ',',
            formula_arg_separator: ',',
            currency_symbol: "$".into(),
            currency_code: "USD".into(),
            currency_before: true,
            currency_space: false,
        }
    }

    /// Espanhol: `1.250,50`, `1.250,50 €`, fórmulas com `;`.
    pub fn es_es() -> Self {
        LocaleConfig {
            code: "es-ES".into(),
            name: "Español".into(),
            decimal_separator: ',',
            thousands_separator: '.',
            formula_arg_separator: ';',
            currency_symbol: "€".into(),
            currency_code: "EUR".into(),
            currency_before: false,
            currency_space: true,
        }
    }

    /// Francês: `1 250,50`, `1 250,50 €`, fórmulas com `;`.
    pub fn fr_fr() -> Self {
        LocaleConfig {
            code: "fr-FR".into(),
            name: "Français".into(),
            decimal_separator: ',',
            thousands_separator: ' ',
            formula_arg_separator: ';',
            currency_symbol: "€".into(),
            currency_code: "EUR".into(),
            currency_before: false,
            currency_space: true,
        }
    }

    /// As que já vêm prontas, pelo código; as outras chegam do arquivo de língua.
    pub fn embutida(code: &str) -> Option<Self> {
        match code {
            "pt-BR" | "pt_BR" | "pt" => Some(Self::pt_br()),
            "en-US" | "en_US" | "en" => Some(Self::en_us()),
            "es-ES" | "es_ES" | "es" => Some(Self::es_es()),
            "fr-FR" | "fr_FR" | "fr" => Some(Self::fr_fr()),
            _ => None,
        }
    }
}

static ATUAL: OnceLock<RwLock<LocaleConfig>> = OnceLock::new();

fn atual() -> &'static RwLock<LocaleConfig> {
    ATUAL.get_or_init(|| RwLock::new(LocaleConfig::pt_br()))
}

static DECIMAL: AtomicU32 = AtomicU32::new(',' as u32);
static MILHAR: AtomicU32 = AtomicU32::new('.' as u32);
/// Os códigos de erro na língua do app: (canônico, na língua), vindos de `erros` no dicionário
/// de funções (`#VALUE!` → `#VALOR!`). Vazio: os canônicos.
static CODIGOS_DE_ERRO: RwLock<Vec<(String, String)>> = RwLock::new(Vec::new());

/// Define como os códigos de erro aparecem. O valor guardado e o arquivo continuam com o canônico.
pub fn definir_codigos_de_erro(pares: Vec<(String, String)>) {
    *CODIGOS_DE_ERRO.write().unwrap_or_else(|e| e.into_inner()) = pares;
}

/// O código de erro como a célula mostra na língua em uso.
pub fn texto_do_erro(canonico: &str) -> String {
    CODIGOS_DE_ERRO.read().unwrap_or_else(|e| e.into_inner()).iter()
        .find(|(c, _)| c == canonico).map_or_else(|| canonico.to_owned(), |(_, t)| t.clone())
}

/// O canônico de um código digitado na língua em uso (`#VALOR!` → `#VALUE!`).
pub fn erro_digitado(texto: &str) -> Option<String> {
    let t = texto.to_uppercase();
    CODIGOS_DE_ERRO.read().unwrap_or_else(|e| e.into_inner()).iter()
        .find(|(_, na_lingua)| na_lingua.to_uppercase() == t).map(|(c, _)| c.clone())
}

/// As palavras do valor lógico na língua do app, vindas do dicionário de funções dela
/// (`TRUE` → `VRAI`): quem carrega o dicionário chama [`definir_logicos`]. `None`: português.
static LOGICOS: RwLock<Option<(String, String)>> = RwLock::new(None);

/// Define como a célula mostra VERDADEIRO e FALSO (os nomes de `TRUE` e `FALSE` no dicionário
/// da língua). O valor guardado, o arquivo e o motor continuam com a forma única em português.
pub fn definir_logicos(verdadeiro: &str, falso: &str) {
    let par = (!verdadeiro.is_empty() && !falso.is_empty()).then(|| (verdadeiro.to_owned(), falso.to_owned()));
    *LOGICOS.write().unwrap_or_else(|e| e.into_inner()) = par;
}

/// O texto de um valor lógico na língua em uso.
pub fn texto_logico(valor: bool) -> String {
    match &*LOGICOS.read().unwrap_or_else(|e| e.into_inner()) {
        Some((sim, nao)) => if valor { sim.clone() } else { nao.clone() },
        None => if valor { "VERDADEIRO" } else { "FALSO" }.to_owned(),
    }
}

/// O lógico que a pessoa digitou: as formas de sempre (português e inglês) em qualquer
/// língua, mais as da língua em uso. `VRAI` só é lógico com o app em francês.
pub fn logico_digitado(texto: &str) -> Option<bool> {
    let e = |p: &str| texto.eq_ignore_ascii_case(p);
    if e("VERDADEIRO") || e("TRUE") { return Some(true); }
    if e("FALSO") || e("FALSE") { return Some(false); }
    let t = texto.to_uppercase();
    match &*LOGICOS.read().unwrap_or_else(|e| e.into_inner()) {
        Some((sim, _)) if t == sim.to_uppercase() => Some(true),
        Some((_, nao)) if t == nao.to_uppercase() => Some(false),
        _ => None,
    }
}

/// Troca a língua do processo.
pub fn set_locale_config(cfg: LocaleConfig) {
    DECIMAL.store(cfg.decimal_separator as u32, Ordering::Relaxed);
    MILHAR.store(cfg.thousands_separator as u32, Ordering::Relaxed);
    *atual().write().unwrap_or_else(|e| e.into_inner()) = cfg;
}

/// A configuração em uso (leitura; não segurar enquanto troca).
pub fn get_locale_config() -> RwLockReadGuard<'static, LocaleConfig> {
    atual().read().unwrap_or_else(|e| e.into_inner())
}

/// O separador decimal em uso — sem lock, é lido a cada número formatado.
pub fn decimal_separator() -> char {
    char::from_u32(DECIMAL.load(Ordering::Relaxed)).unwrap_or(',')
}

/// O separador de milhar em uso — sem lock, é lido a cada número formatado.
pub fn thousands_separator() -> char {
    char::from_u32(MILHAR.load(Ordering::Relaxed)).unwrap_or('.')
}

/// Separador padrão de CSV e de Dividir texto, vinculado à regionalização.
/// Paridade com `getTextSeparator()` do TypeScript legado: apenas vírgula
/// escolhe vírgula; separadores desconhecidos usam ponto e vírgula.
pub fn separador_texto(cfg: &LocaleConfig) -> char {
    if cfg.formula_arg_separator == ',' { ',' } else { ';' }
}

/// A representação interna permanece pt-BR para não reinterpretar fórmulas
/// existentes quando o usuário troca a língua. Somente a edição é localizada.
pub fn normalizar_formula(raw: &str, cfg: &LocaleConfig) -> String {
    converter_formula(raw, cfg, false)
}

pub fn apresentar_formula(raw: &str, cfg: &LocaleConfig) -> String {
    converter_formula(raw, cfg, true)
}

fn converter_formula(raw: &str, cfg: &LocaleConfig, apresentar: bool) -> String {
    use crate::formula::tokenizer::{tokenize_com_decimal, TokenType};
    if !raw.starts_with('=') || cfg.formula_arg_separator != ',' { return raw.to_owned(); }
    let decimal = if apresentar { ',' } else { cfg.decimal_separator };
    let Ok(tokens) = tokenize_com_decimal(raw, decimal) else { return raw.to_owned() };
    let chars: Vec<char> = raw.chars().collect();
    let mut saida = String::with_capacity(raw.len());
    let mut anterior = 0;
    let mut matriz = 0;
    for t in tokens {
        if t.kind == TokenType::Eof { break; }
        let fim = t.pos + t.text.chars().count();
        // Str/Ref podem ter texto normalizado pelo lexer: somente números e
        // separadores são substituídos; todo o restante permanece byte a byte.
        let troca = match t.kind {
            TokenType::LBrace => { matriz += 1; None }
            TokenType::RBrace => { matriz -= 1; None }
            TokenType::Num if apresentar && t.text.contains(',') => Some(t.text.replace(',', ".")),
            TokenType::Comma if apresentar && matriz == 0 && t.text == ";" => Some(",".into()),
            TokenType::Comma if apresentar && matriz > 0 && t.text == "\\" => Some(",".into()),
            TokenType::Comma if !apresentar && t.text == "," => Some(if matriz > 0 { "\\" } else { ";" }.into()),
            _ => None,
        };
        if let Some(troca) = troca {
            saida.extend(chars[anterior..t.pos].iter());
            saida.push_str(&troca);
            anterior = fim;
        }
    }
    saida.extend(chars[anterior..].iter());
    saida
}

/// Troca o nome de cada função (identificador seguido de `(`) pelo que
/// `trocar` devolver; o resto da fórmula fica byte a byte. É o de-para dos
/// nomes por idioma: os mapas vêm dos arquivos de língua do app, o motor não
/// conhece idiomas. Texto que não é fórmula volta igual.
pub fn traduzir_funcoes(raw: &str, trocar: &dyn Fn(&str) -> Option<String>) -> String {
    use crate::formula::tokenizer::{tokenize_com_decimal, TokenType};
    if !raw.starts_with('=') { return raw.to_owned(); }
    let Ok(tokens) = tokenize_com_decimal(raw, decimal_separator()) else { return raw.to_owned() };
    let chars: Vec<char> = raw.chars().collect();
    let mut saida = String::with_capacity(raw.len());
    let mut anterior = 0;
    for (i, t) in tokens.iter().enumerate() {
        if t.kind != TokenType::Ident { continue; }
        if tokens.get(i + 1).map(|p| p.kind) != Some(TokenType::LParen) { continue; }
        let fim = t.pos + t.text.chars().count();
        // O texto do token pode vir normalizado: confere com o original.
        let original: String = chars.get(t.pos..fim).map(|c| c.iter().collect()).unwrap_or_default();
        let Some(novo) = trocar(&original) else { continue };
        saida.extend(chars[anterior..t.pos].iter());
        saida.push_str(&novo);
        anterior = fim;
    }
    saida.extend(chars[anterior..].iter());
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn porta_regionalizacao_ts_unicode_csv_e_formula_original() {
        let ingles = LocaleConfig::en_us();
        let digitada = "=SUM('São Paulo'!A1,2.5)";
        let interna = "=SUM('São Paulo'!A1;2.5)";
        assert_eq!(normalizar_formula(digitada, &ingles), interna);
        assert_eq!(apresentar_formula(interna, &ingles), digitada);
        assert_eq!(normalizar_formula("texto, fora de formula", &ingles), "texto, fora de formula");
        assert_eq!(separador_texto(&ingles), ',');
        assert_eq!(separador_texto(&LocaleConfig::pt_br()), ';');

        let mut configuracao_inesperada = LocaleConfig::pt_br();
        configuracao_inesperada.formula_arg_separator = '|';
        assert_eq!(separador_texto(&configuracao_inesperada), ';');
        assert_eq!(normalizar_formula("=SOMA(1;2)", &configuracao_inesperada), "=SOMA(1;2)");
    }

    #[test]
    fn de_para_troca_so_nomes_de_funcao() {
        let es = |n: &str| match n.to_uppercase().as_str() {
            "SOMA" | "SUM" => Some("SUMA".to_string()),
            "SE" | "IF" => Some("SI".to_string()),
            _ => None,
        };
        assert_eq!(traduzir_funcoes("=SE(A1>0;SOMA(B1:B3);\"SOMA(\")", &es),
            "=SI(A1>0;SUMA(B1:B3);\"SOMA(\")");
        // Referência, nome definido e texto fora de fórmula não mudam.
        assert_eq!(traduzir_funcoes("=SOMA+Sum1*2", &es), "=SOMA+Sum1*2");
        assert_eq!(traduzir_funcoes("SOMA(1)", &es), "SOMA(1)");
        assert_eq!(traduzir_funcoes("=sum(1;2)", &es), "=SUMA(1;2)");
    }

    #[test]
    fn as_embutidas_pelo_codigo() {
        assert_eq!(LocaleConfig::embutida("pt-BR").unwrap().decimal_separator, ',');
        assert_eq!(LocaleConfig::embutida("en").unwrap().formula_arg_separator, ',');
        assert_eq!(LocaleConfig::embutida("es").unwrap().formula_arg_separator, ';');
        assert_eq!(LocaleConfig::embutida("fr-FR").unwrap().thousands_separator, ' ');
        assert!(LocaleConfig::embutida("de-DE").is_none(), "as outras vêm do arquivo de língua");
    }

    #[test]
    fn argumentos_numericos_ingleses_nao_viram_decimal() {
        let en = LocaleConfig::en_us();
        assert_eq!(normalizar_formula("=SUM(1,2,3.5)", &en), "=SUM(1;2;3.5)");
        assert_eq!(apresentar_formula("=SOMA(1;2;3,5)", &en), "=SOMA(1,2,3.5)");
        let raw = "=SUM(1,2,3.5)";
        assert_eq!(apresentar_formula(&normalizar_formula(raw, &en), &en), raw);
        assert_eq!(normalizar_formula("=SOMA(1;2;3,5)", &LocaleConfig::pt_br()), "=SOMA(1;2;3,5)");
        assert_eq!(separador_texto(&en), ',');
        assert_eq!(separador_texto(&LocaleConfig::pt_br()), ';');
    }

    #[test]
    fn preserva_strings_referencias_e_matrizes_ao_localizar() {
        let en = LocaleConfig::en_us();
        assert_eq!(normalizar_formula(r#"=IF('A,B'!A1,"a,b;""c""",SUM(1,2))"#, &en),
            r#"=IF('A,B'!A1;"a,b;""c""";SUM(1;2))"#);
        assert_eq!(normalizar_formula("={1,2;3,4}", &en), "={1\\2;3\\4}");
        assert_eq!(apresentar_formula("={1\\2;3\\4}", &en), "={1,2;3,4}");
        assert_eq!(normalizar_formula("=SUM(T[a,b],1,2)", &en), "=SUM(T[a,b];1;2)");
        assert_eq!(normalizar_formula("texto, normal;", &en), "texto, normal;");
    }
}
