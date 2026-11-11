//! Como um valor aparece na célula — a tradução do `src/core/style.ts`.
//!
//! Existe porque o `docs/ARQUITETURA.md` promete que **quem formata é o Rust**: o
//! que atravessa a ponte é texto pronto para desenhar. Isso apaga uma classe
//! inteira de bug — a mesma célula com dois formatadores diferentes — e é o
//! que permite o formato condicional e o `.xlsx` concordarem sem combinar
//! nada.
//!
//! ## Por que não dá para usar o `to_js_string`
//!
//! O [`crate::celulas::number::to_js_string`] é o `String(n)` do JavaScript, e existe
//! para o **arquivo**: é ele que decide se o texto digitado reconstrói do
//! valor. Ele escreve `199.9`, com ponto. Quem decide o que a pessoa **vê** é
//! outro caminho, e em pt-BR o separador decimal é vírgula. Confundir os dois
//! foi o primeiro erro que este arquivo pegou: o quadro binário saía com
//! `1449.9` onde a grade mostra `1.449,9`.
//!
//! ## Portabilidade
//!
//! O núcleo Rust implementa os formatos automático, número, inteiro, moeda,
//! porcentagem, científico, contábil, duração, data, data/hora, hora e texto.
//! `format_value` centraliza também texto, booleanos, vazios e erros;
//! a grade Iced não mantém um segundo formatador para esses tipos.
//!
//! O original usa `Intl.NumberFormat('pt-BR', { minimumFractionDigits: 0,
//! maximumFractionDigits: 10 })`. Traduzido: no máximo dez casas decimais,
//! sem zero à toa no fim, milhar separado por ponto, decimal por vírgula, e
//! **nunca** notação científica — `1e21` sai por extenso, com os vinte e um
//! dígitos.

/// Quantas casas decimais o formato automático mostra, no máximo.
const MAX_CASAS: usize = 10;

/// O texto de um número no formato automático, em pt-BR.
///
/// Fiel ao `formatAuto` do `style.ts`, inclusive nos cantos que surpreendem:
/// `1e-11` vira `"0"` (passou das dez casas), `2/3` arredonda na décima casa,
/// e `1e21` sai inteiro em vez de virar `1e+21`.
pub fn format_auto(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "∞".to_string() } else { "-∞".to_string() };
    }

    let negativo = n.is_sign_negative();
    let absoluto = n.abs();

    let (inteiro, decimais) = arredondar_meio_para_cima(absoluto, MAX_CASAS);
    let decimal = decimais.trim_end_matches('0').to_string();

    let mut saida = String::with_capacity(inteiro.len() + decimal.len() + 8);
    // O sinal FICA mesmo quando o valor arredonda para zero: o `Intl` escreve
    // "-0" para `-1e-11`. Conferido contra o Node — eu tinha suposto o
    // contrário, e os 1.344 casos comparados pegaram a suposição.
    if negativo {
        saida.push('-');
    }
    empilhar_milhar(&inteiro, &mut saida);
    if !decimal.is_empty() {
        saida.push(crate::celulas::locale::decimal_separator());
        saida.push_str(&decimal);
    }
    saida
}

/// Os dígitos do número na representação **mais curta** que volta ao mesmo
/// valor — a mesma que o `String(n)` do JavaScript produz —, já expandida em
/// parte inteira e parte decimal, sem notação científica.
///
/// ⚠ É daqui que vem a fidelidade ao `Intl`, e não é óbvio: o
/// `Intl.NumberFormat` **não** arredonda a expansão binária exata do número,
/// arredonda a representação curta. A diferença aparece o tempo todo:
///
/// - `1234567.891` tem expansão exata `1234567.8910000001…`; arredondar isso
///   a dez casas dá `1.234.567,8910000001`, mas a grade mostra
///   `1.234.567,891`;
/// - `0.155` é na verdade `0.1549999999…`; arredondar o exato a duas casas dá
///   `0,15`, mas a grade mostra `0,16`;
/// - `1.005` é `1.00499999…` e mesmo assim vira `1,01`.
///
/// Foi uma comparação de 1.344 casos contra o `style.ts` rodando que revelou
/// isto — a tabela curada de vinte casos passava sem acusar nada.
fn digitos_de(absoluto: f64) -> (String, String) {
    let texto = crate::celulas::number::to_js_string(absoluto);
    let (mantissa, expoente) = match texto.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().unwrap_or(0)),
        None => (texto.as_str(), 0),
    };
    let (parte_inteira, parte_decimal) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digitos: String = format!("{parte_inteira}{parte_decimal}");
    // Onde a vírgula cai depois de aplicar o expoente.
    let ponto = parte_inteira.len() as i32 + expoente;

    let (mut inteiro, decimal) = if ponto <= 0 {
        ("0".to_string(), format!("{}{}", "0".repeat((-ponto) as usize), digitos))
    } else if ponto as usize >= digitos.len() {
        (
            format!("{}{}", digitos, "0".repeat(ponto as usize - digitos.len())),
            String::new(),
        )
    } else {
        (digitos[..ponto as usize].to_string(), digitos[ponto as usize..].to_string())
    };

    // "007" nunca aparece, mas o caminho do expoente pode produzir zero à
    // esquerda; tirar mantendo pelo menos um dígito.
    while inteiro.len() > 1 && inteiro.starts_with('0') {
        inteiro.remove(0);
    }
    (inteiro, decimal)
}

/// Arredonda para `casas` decimais **com a metade para cima**, sobre a
/// representação curta. Devolve parte inteira e decimal, já com exatamente
/// `casas` dígitos depois da vírgula.
///
/// O Rust e o JavaScript também discordam no empate: o `{:.N}` do Rust
/// arredonda "metade para o par" (1250,5 com zero casas vira 1250) e o `Intl`
/// arredonda "metade para cima" (vira 1251). Aqui a conta é feita em cima dos
/// dígitos, em texto, então não há empate mal resolvido nem erro de ponto
/// flutuante no meio do caminho.
fn arredondar_meio_para_cima(absoluto: f64, casas: usize) -> (String, String) {
    if !absoluto.is_finite() {
        return (absoluto.to_string(), String::new());
    }
    let (inteiro, decimal) = digitos_de(absoluto);

    if decimal.len() <= casas {
        let mut completo = decimal;
        while completo.len() < casas {
            completo.push('0');
        }
        return (inteiro, completo);
    }

    let (fica, resto) = decimal.split_at(casas);
    let sobe = resto.as_bytes().first().is_some_and(|d| *d >= b'5');

    let mut digitos: Vec<u8> = inteiro.bytes().chain(fica.bytes()).map(|b| b - b'0').collect();
    if sobe {
        let mut i = digitos.len();
        loop {
            if i == 0 {
                digitos.insert(0, 1);
                break;
            }
            i -= 1;
            if digitos[i] == 9 {
                digitos[i] = 0;
            } else {
                digitos[i] += 1;
                break;
            }
        }
    }

    let texto: String = digitos.iter().map(|d| (d + b'0') as char).collect();
    let corte = texto.len() - casas;
    (texto[..corte].to_string(), texto[corte..].to_string())
}

/// Escreve a parte inteira com o separador de milhar a cada três dígitos
/// (o ponto em pt-BR, a vírgula em en-US).
fn empilhar_milhar(inteiro: &str, saida: &mut String) {
    let milhar = crate::celulas::locale::thousands_separator();
    let digitos = inteiro.len();
    for (i, c) in inteiro.chars().enumerate() {
        // O separador entra ANTES do dígito quando o que falta é múltiplo de
        // três — e nunca no começo, senão "1000" sairia ".1.000".
        if i > 0 && (digitos - i) % 3 == 0 {
            saida.push(milhar);
        }
        saida.push(c);
    }
}


// ------------------------------------------------------------------ o estilo

/// Os formatos de número, iguais aos do `style.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumberFormat {
    /// Sem formato escolhido: o número aparece como [`format_auto`] o escreve.
    #[default]
    Auto,
    Number,
    Integer,
    Currency,
    Scientific,
    Accounting,
    Duration,
    Percent,
    Date,
    Datetime,
    Time,
    /// Mostra o valor como texto, mesmo sendo número.
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Auto,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

/// Bordas de uma célula. A cor vale para os quatro lados.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Borders {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
    pub color: Option<String>,
}

/// O estilo de uma célula — a tradução do `Style` do `style.ts`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    /// Tamanho da fonte em pixels; ausente usa o padrão do tema.
    pub font_size: Option<f32>,
    pub font_family: Option<String>,
    /// Casas decimais fixas. Ausente = o padrão de cada formato.
    pub decimals: Option<u8>,
    pub borders: Option<Borders>,
    pub align: Align,
    pub valign: VAlign,
    pub wrap: bool,
    pub format: NumberFormat,
    pub color: Option<String>,
    pub bg: Option<String>,
    /// Rotação do texto em graus (-90 a 90).
    pub rotation: Option<i16>,
    /// Endereço do link da célula, à parte do texto que ela mostra (o
    /// hiperlink do `.xlsx`). Espelha o `link` do `style.ts`.
    pub link: Option<String>,
    /// Formato personalizado no código do Excel (`#,##0.00" kWp"`, `;;;`).
    /// Quando existe, manda sobre `format` e `decimals` na hora de mostrar.
    pub code: Option<String>,
}

impl Style {
    /// Se o estilo tem alguma coisa. Um estilo vazio não precisa ir no quadro.
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }
}

/// Quantas casas decimais um formato mostra hoje.
///
/// Tradução do `decimalsOf`. A casa escolhida à mão ganha de tudo; sem ela,
/// cada formato tem o seu padrão.
pub fn decimals_of(style: Option<&Style>) -> u8 {
    if let Some(s) = style {
        if let Some(d) = s.decimals {
            return d;
        }
        return match s.format {
            NumberFormat::Integer | NumberFormat::Duration => 0,
            NumberFormat::Number
            | NumberFormat::Currency
            | NumberFormat::Scientific
            | NumberFormat::Accounting
            | NumberFormat::Percent => 2,
            _ => 0,
        };
    }
    0
}

/// O espaço que separa `R$` do número na moeda em pt-BR.
///
/// É um espaço **não separável** (U+00A0), não o espaço comum. É o que o
/// `Intl.NumberFormat` produz, e trocar por espaço normal faria o texto que
/// atravessa a ponte diferir do que a grade sempre desenhou — a diferença
/// aparece ao copiar uma célula para fora do aplicativo.
const ESPACO_MOEDA: char = '\u{00A0}';

/// O maior instante que o `Date` do JavaScript representa: ±8,64e15 ms
/// (cerca de 273.790 anos em volta de 1970). Passou disso, lá vira `Invalid
/// Date` e o `style.ts` desiste de escrever a data.
const MAX_MS_DE_DATA: f64 = 8.64e15;

/// Um número com casas decimais fixas, em pt-BR.
///
/// Tradução do `decimalFormat`: mesmo separador de milhar, mesma vírgula,
/// e `R$` na frente quando é moeda. O sinal de menos vem antes do `R$`
/// (`-R$ 1.250,50`), como o `Intl` faz.
pub fn format_decimal(n: f64, decimals: u8, currency: bool) -> String {
    if n.is_nan() {
        return "NaN".to_string();
    }
    if n.is_infinite() {
        return if n > 0.0 { "∞".to_string() } else { "-∞".to_string() };
    }
    let casas = decimals as usize;
    let (inteiro, decimal) = arredondar_meio_para_cima(n.abs(), casas);

    let mut saida = String::new();
    // Mesma regra do automático: o menos fica, mesmo em "-0,00".
    if n.is_sign_negative() {
        saida.push('-');
    }
    let cfg = crate::celulas::locale::get_locale_config();
    // Como o `Intl`: `R$ 1.250,50` (espaço não separável) em pt-BR,
    // `$1,250.50` colado em en-US, `1.250,50 €` depois em de-DE — a posição e
    // o espaço vêm do arquivo de língua.
    if currency && cfg.currency_before {
        saida.push_str(&cfg.currency_symbol);
        if cfg.currency_space {
            saida.push(ESPACO_MOEDA);
        }
    }
    empilhar_milhar(&inteiro, &mut saida);
    if !decimal.is_empty() {
        saida.push(cfg.decimal_separator);
        saida.push_str(&decimal);
    }
    if currency && !cfg.currency_before {
        if cfg.currency_space {
            saida.push(ESPACO_MOEDA);
        }
        saida.push_str(&cfg.currency_symbol);
    }
    saida
}

/// O texto de um valor numa célula — a tradução do `formatValue`.
///
/// Só número é formatado. Texto, lógico e erro saem como são, porque é assim
/// que o original se comporta: o formato é do número, não da célula.
pub fn format_number(n: f64, style: Option<&Style>) -> String {
    let formato = style.map(|s| s.format).unwrap_or_default();
    let casas = decimals_of(style);
    match formato {
        NumberFormat::Text => to_texto_simples(n),
        NumberFormat::Number => format_decimal(n, casas, false),
        NumberFormat::Integer => {
            // ⚠ `Math.round` do JavaScript arredonda a metade para CIMA (rumo
            // a +∞), não "para longe do zero" como o `round()` do Rust:
            // `Math.round(-2.5)` é `-2`, e `(-2.5f64).round()` é `-3`. Sem
            // isto, todo negativo terminado em ,5 saía diferente da grade —
            // foram 64 das 1.344 linhas comparadas contra o `style.ts`.
            let mut arredondado = (n + 0.5).floor();
            // E o `Math.round(-0.5)` é `-0`, que o `Intl` escreve como "-0":
            // o `floor` perde esse sinal, então ele é reposto.
            if arredondado == 0.0 {
                arredondado = arredondado.copysign(n);
            }
            format_decimal(arredondado, 0, false)
        }
        NumberFormat::Currency => format_decimal(n, casas, true),
        NumberFormat::Percent => format!("{}%", format_decimal(n * 100.0, casas, false)),
        NumberFormat::Scientific => {
            if n == 0.0 {
                return format!("{}E+00", format_decimal(0.0, casas, false));
            }
            let expoente = n.abs().log10().floor() as i32;
            let mantissa = n / 10f64.powi(expoente);
            let sinal = if expoente >= 0 { '+' } else { '-' };
            format!(
                "{}E{}{:02}",
                format_decimal(mantissa, casas, false),
                sinal,
                expoente.abs(),
            )
        }
        NumberFormat::Accounting => {
            // O negativo vai entre parênteses, sem o menos — é a convenção
            // contábil, e é o que o `style.ts` faz.
            // O símbolo vem da língua, como na Moeda: em inglês era "R$" fixo.
            let texto = format_decimal(n.abs(), casas, false);
            let simbolo = crate::celulas::locale::get_locale_config().currency_symbol.clone();
            if n < 0.0 {
                format!("({simbolo} {texto})")
            } else {
                format!("{simbolo} {texto}")
            }
        }
        NumberFormat::Duration => {
            // Tudo em `f64`, como no `style.ts` — e não em inteiro. Parece
            // detalhe e não é: uma duração absurda (1e21 dias) dá
            // `2.4e+22:24:16` lá, porque o `String(h)` do JavaScript vira
            // notação científica. Em `i64` isso saturaria e sairia outro
            // número. Foi a última das 1.344 linhas comparadas a fechar.
            let sinal = if n < 0.0 { "-" } else { "" };
            let total_segundos = (n.abs() * 86_400.0).round();
            let h = (total_segundos / 3600.0).floor();
            let m = ((total_segundos % 3600.0) / 60.0).floor();
            let s = total_segundos % 60.0;
            format!("{sinal}{}:{}:{}", com_dois_digitos(h), com_dois_digitos(m), com_dois_digitos(s))
        }
        NumberFormat::Date | NumberFormat::Datetime | NumberFormat::Time => {
            formatar_data(n, formato).unwrap_or_else(|| to_texto_simples(n))
        }
        NumberFormat::Auto => match style.and_then(|s| s.decimals) {
            // No automático, casa decimal escolhida à mão manda.
            Some(d) => format_decimal(n, d, false),
            None => format_auto(n),
        },
    }
}

/// Texto efetivamente exibido na grade para qualquer valor da planilha.
///
/// Porta `style.ts::formatValue`: só números recebem formatos numéricos;
/// texto, vazio, booleano e erro mantêm o texto canônico do motor.
pub fn format_value(valor: &crate::celulas::values::Value, estilo: Option<&Style>) -> String {
    // `-0` nunca aparece (Excel e JavaScript escrevem "0"), com ou sem formato.
    let zero = crate::celulas::values::Value::Number(0.0);
    let valor = match valor {
        crate::celulas::values::Value::Number(n) if *n == 0.0 => &zero,
        v => v,
    };
    if let Some(codigo) = estilo.and_then(|e| e.code.as_deref()) {
        if let Some(texto) = crate::celulas::formato_personalizado::formatar(valor, codigo) {
            return texto;
        }
    }
    match valor {
        crate::celulas::values::Value::Number(n) => format_number(*n, estilo),
        _ => crate::celulas::values::to_text(valor),
    }
}

/// Data, hora ou as duas, a partir do serial. `None` quando o serial não é
/// uma data que dê para escrever.
fn formatar_data(serial: f64, formato: NumberFormat) -> Option<String> {
    if !serial.is_finite() {
        return None;
    }
    let ms = crate::celulas::date_serial::serial_to_timestamp(serial);
    if !ms.is_finite() || ms.abs() > MAX_MS_DE_DATA {
        // Fora do alcance do `Date` do JavaScript. Lá isso vira `Invalid
        // Date` e o `style.ts` cai no texto simples; aqui a conta daria uma
        // data do ano 339913, que é pior do que não responder.
        return None;
    }
    let dias = (ms / crate::celulas::date_serial::DAY_MS).floor();
    let (ano, mes, dia) = crate::celulas::date_serial::serial_to_ymd(serial.floor() as i64);
    // A parte da hora sai do resto do dia, em milissegundos. **Truncando**,
    // não arredondando: o `getUTCSeconds` do JavaScript trunca, e arredondar
    // deixava 16:17:36 virar 16:17:37.
    let resto = ms - dias * crate::celulas::date_serial::DAY_MS;
    let segundos_do_dia = (resto / 1000.0).floor().rem_euclid(86_400.0) as i64;
    let hh = segundos_do_dia / 3600;
    let mm = (segundos_do_dia % 3600) / 60;
    let ss = segundos_do_dia % 60;

    let data = format!("{dia:02}/{mes:02}/{ano:04}");
    let hora = format!("{hh:02}:{mm:02}:{ss:02}");
    Some(match formato {
        NumberFormat::Date => data,
        NumberFormat::Time => hora,
        _ => format!("{data} {hora}"),
    })
}

/// O número com pelo menos dois dígitos, como o `String(n).padStart(2, '0')`
/// do JavaScript: `0` vira `"00"`, `12` continua `"12"`, e um número enorme
/// sai em notação científica sem enchimento nenhum.
fn com_dois_digitos(n: f64) -> String {
    let texto = crate::celulas::number::to_js_string(n);
    if texto.len() >= 2 {
        texto
    } else {
        format!("0{texto}")
    }
}

/// O número escrito como texto simples, sem formato — o caminho do
/// `NumberFormat::Text` e o do serial que não vira data.
///
/// É o `toText` do `values.ts`, não o `String(n)`: ele corta em doze dígitos
/// significativos (`0,333333333333`, não `0,3333333333333333`). Usar o
/// `to_js_string` aqui foi um erro meu que só a comparação dos 1.344 casos
/// pegou.
fn to_texto_simples(n: f64) -> String {
    crate::celulas::number::format_number(n)
}

/// Alinhamento efetivo: número à direita, texto à esquerda, lógico e erro no
/// meio. Tradução do `effectiveAlign`.
pub fn effective_align(valor: &crate::celulas::values::Value, style: Option<&Style>) -> Align {
    if let Some(s) = style {
        if s.align != Align::Auto {
            return s.align;
        }
    }
    match valor {
        crate::celulas::values::Value::Number(_) => Align::Right,
        crate::celulas::values::Value::Bool(_) | crate::celulas::values::Value::Error(_) => Align::Center,
        _ => Align::Left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os valores e as respostas saíram do `Intl` de verdade, rodado no Node
    /// com o mesmo `Intl.NumberFormat('pt-BR', …)` que o `style.ts` usa. Quem
    /// mexer aqui e quiser mudar uma expectativa: confira lá primeiro, porque
    /// é esse o comportamento que está no disco de quem usa o aplicativo.
    #[test]
    fn bate_com_o_intl_do_typescript() {
        let casos: &[(f64, &str)] = &[
            (1449.9, "1.449,9"),
            (1250.0, "1.250"),
            (199.9, "199,9"),
            (0.5, "0,5"),
            (0.0, "0"),
            (-3.0, "-3"),
            (100.0, "100"),
            (1_000_000.0, "1.000.000"),
            (-1234.5, "-1.234,5"),
            (12_345_678.125, "12.345.678,125"),
            (9_007_199_254_740_991.0, "9.007.199.254.740.991"),
            (1e21, "1.000.000.000.000.000.000.000"),
            (1e-7, "0,0000001"),
            (-0.000001, "-0,000001"),
            (1e-10, "0,0000000001"),
            (1.005, "1,005"),
        ];
        for (valor, esperado) in casos {
            assert_eq!(&format_auto(*valor), esperado, "format_auto({valor})");
        }
    }

    #[test]
    fn passou_de_dez_casas_vira_zero() {
        // O `Intl` com `maximumFractionDigits: 10` faz exatamente isto.
        assert_eq!(format_auto(1e-11), "0");
        assert_eq!(format_auto(-1e-11), "-0", "o Intl mantém o sinal: -1e-11 vira \"-0\"");
    }

    #[test]
    fn arredonda_na_decima_casa() {
        assert_eq!(format_auto(1.0 / 3.0), "0,3333333333");
        assert_eq!(format_auto(2.0 / 3.0), "0,6666666667");
    }

    #[test]
    fn o_lixo_do_ponto_flutuante_nao_aparece() {
        // 0,1 + 0,2 é 0.30000000000000004 em binário. Na tela é "0,3".
        assert_eq!(format_auto(0.1 + 0.2), "0,3");
    }

    #[test]
    fn sem_zero_a_toa_no_fim() {
        assert_eq!(format_auto(2.50), "2,5");
        assert_eq!(format_auto(2.0), "2");
    }

    #[test]
    fn o_milhar_entra_de_tres_em_tres_e_nunca_no_comeco() {
        assert_eq!(format_auto(1.0), "1");
        assert_eq!(format_auto(12.0), "12");
        assert_eq!(format_auto(123.0), "123");
        assert_eq!(format_auto(1234.0), "1.234");
        assert_eq!(format_auto(12345.0), "12.345");
        assert_eq!(format_auto(123456.0), "123.456");
        assert_eq!(format_auto(1234567.0), "1.234.567");
    }

    /// Um estilo só com o formato, para encurtar os casos.
    fn com(formato: NumberFormat) -> Style {
        Style { format: formato, ..Default::default() }
    }

    /// ⚠ Esta tabela saiu do `formatValue` do TypeScript RODANDO, não da minha
    /// cabeça: `node --import tsx` chamando `src/core/style.ts` com os mesmos
    /// valores. É o comportamento que está no disco de quem usa o aplicativo.
    /// Quem quiser mudar uma expectativa aqui precisa mudar o `style.ts`
    /// primeiro — senão a ponte passa a mostrar coisa diferente da grade.
    #[test]
    fn bate_com_o_format_value_do_typescript() {
        let casos: &[(f64, Style, &str)] = &[
            (1250.5, com(NumberFormat::Currency), "R$\u{00A0}1.250,50"),
            (-1250.5, com(NumberFormat::Currency), "-R$\u{00A0}1.250,50"),
            (0.0, com(NumberFormat::Currency), "R$\u{00A0}0,00"),
            (1_234_567.891, com(NumberFormat::Currency), "R$\u{00A0}1.234.567,89"),
            (1250.5, com(NumberFormat::Number), "1.250,50"),
            (-3.0, com(NumberFormat::Number), "-3,00"),
            (1_234_567.891, com(NumberFormat::Number), "1.234.567,89"),
            (1250.5, com(NumberFormat::Integer), "1.251"),
            (-3.4, com(NumberFormat::Integer), "-3"),
            (0.155, com(NumberFormat::Percent), "15,50%"),
            (1.0, com(NumberFormat::Percent), "100,00%"),
            (0.0, com(NumberFormat::Percent), "0,00%"),
            (1250.5, com(NumberFormat::Accounting), "R$ 1.250,50"),
            (-1250.5, com(NumberFormat::Accounting), "(R$ 1.250,50)"),
            (0.0, com(NumberFormat::Accounting), "R$ 0,00"),
            (0.5, com(NumberFormat::Duration), "12:00:00"),
            (1.25, com(NumberFormat::Duration), "30:00:00"),
            (-0.5, com(NumberFormat::Duration), "-12:00:00"),
            (0.0, com(NumberFormat::Duration), "00:00:00"),
            (12345.6789, com(NumberFormat::Scientific), "1,23E+04"),
            (0.0, com(NumberFormat::Scientific), "0,00E+00"),
            (-0.00042, com(NumberFormat::Scientific), "-4,20E-04"),
            (46_000.0, com(NumberFormat::Date), "09/12/2025"),
            (1.0, com(NumberFormat::Date), "31/12/1899"),
            (45_000.75, com(NumberFormat::Date), "15/03/2023"),
            (46_000.5, com(NumberFormat::Datetime), "09/12/2025 12:00:00"),
            (45_000.75, com(NumberFormat::Datetime), "15/03/2023 18:00:00"),
            (46_000.5, com(NumberFormat::Time), "12:00:00"),
            (0.25, com(NumberFormat::Time), "06:00:00"),
            (1250.5, com(NumberFormat::Text), "1250.5"),
            (
                1250.5,
                Style { format: NumberFormat::Number, decimals: Some(4), ..Default::default() },
                "1.250,5000",
            ),
            (
                1250.5,
                Style { decimals: Some(3), ..Default::default() },
                "1.250,500",
            ),
        ];
        for (valor, estilo, esperado) in casos {
            assert_eq!(
                &format_number(*valor, Some(estilo)),
                esperado,
                "format_number({valor}, {:?})",
                estilo.format,
            );
        }
    }

    #[test]
    fn a_moeda_usa_espaco_nao_separavel() {
        // O `Intl` produz U+00A0 entre o "R$" e o número. Trocar por espaço
        // comum faz o texto copiado do aplicativo diferir do que sempre foi.
        let texto = format_number(1.0, Some(&com(NumberFormat::Currency)));
        assert!(texto.contains('\u{00A0}'), "a moeda saiu com espaço comum: {texto:?}");
        assert!(!texto.contains("R$ "), "a moeda não pode ter espaço comum depois do R$");
    }

    #[test]
    fn casa_decimal_escolhida_a_mao_ganha_do_padrao_do_formato() {
        let padrao = Style { format: NumberFormat::Currency, ..Default::default() };
        assert_eq!(decimals_of(Some(&padrao)), 2);
        let escolhido = Style { format: NumberFormat::Currency, decimals: Some(0), ..Default::default() };
        assert_eq!(decimals_of(Some(&escolhido)), 0);
        assert_eq!(format_number(1250.5, Some(&escolhido)), "R$\u{00A0}1.251");
    }

    #[test]
    fn sem_estilo_nenhum_e_o_formato_automatico() {
        assert_eq!(format_number(1449.9, None), "1.449,9");
        assert_eq!(format_number(1449.9, Some(&Style::default())), "1.449,9");
    }

    #[test]
    fn o_alinhamento_segue_o_tipo_do_valor() {
        use crate::celulas::values::Value;
        assert_eq!(effective_align(&Value::Number(1.0), None), Align::Right);
        assert_eq!(effective_align(&Value::Text("a".into()), None), Align::Left);
        assert_eq!(effective_align(&Value::Bool(true), None), Align::Center);
        // Escolha à mão ganha do tipo.
        let esquerda = Style { align: Align::Left, ..Default::default() };
        assert_eq!(effective_align(&Value::Number(1.0), Some(&esquerda)), Align::Left);
    }

    #[test]
    fn formato_valor_portado_para_texto_logico_vazio_e_erro() {
        use crate::celulas::values::{CellError, ErrorCode, Value};
        let percentual = Style { format: NumberFormat::Percent, ..Default::default() };
        assert_eq!(format_value(&Value::Empty, Some(&percentual)), "");
        assert_eq!(format_value(&Value::Text("00123".into()), Some(&percentual)), "00123");
        assert_eq!(format_value(&Value::Bool(true), Some(&percentual)), "VERDADEIRO");
        assert_eq!(format_value(&Value::Bool(false), Some(&percentual)), "FALSO");
        assert_eq!(format_value(&Value::Error(CellError::new(ErrorCode::Div0)), Some(&percentual)), "#DIV/0!");
        assert_eq!(format_value(&Value::Number(0.125), Some(&percentual)), "12,50%");
        let formato_texto = Style { format: NumberFormat::Text, ..Default::default() };
        assert_eq!(format_value(&Value::Number(1250.5), Some(&formato_texto)), "1250.5");
    }

    #[test]
    fn estilo_vazio_se_reconhece() {
        assert!(Style::default().is_empty());
        assert!(!Style { bold: true, ..Default::default() }.is_empty());
    }
}
