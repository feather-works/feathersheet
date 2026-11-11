use serde::{Deserialize, Serialize};

use crate::celulas::refs::Rect;
use crate::celulas::values::{to_number, Value};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NumeroOp {
    Entre,
    Maior,
    Menor,
    Igual,
    /// Os outros quatro do Excel (`notBetween`, `notEqual`, `>=`, `<=`).
    Fora,
    Diferente,
    MaiorIgual,
    MenorIgual,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataOp {
    Entre,
    Antes,
    Depois,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextoOp {
    Contem,
    Comeca,
    Email,
    Url,
}

/// O mesmo formato do `Regra` de `core/validation.ts`: um campo `tipo`
/// dizendo qual variante, os outros campos ao lado (não aninhados).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "lowercase")]
pub enum Regra {
    Lista { valores: Vec<String> },
    Intervalo { origem: Rect },
    Numero {
        op: NumeroOp,
        a: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<f64>,
        /// "Número inteiro" do Excel: decimal é recusado.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        inteiro: bool,
    },
    Data {
        op: DataOp,
        a: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<String>,
    },
    Texto {
        op: TextoOp,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        valor: Option<String>,
    },
    Caixa { marcado: String, desmarcado: String },
    /// Hora do dia (`HH:MM` ou `HH:MM:SS`), com os mesmos oito operadores do
    /// número. Aceita texto ou o número de série (só a parte fracionária conta).
    Hora {
        op: NumeroOp,
        a: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        b: Option<String>,
    },
    /// Lista cuja origem é uma fórmula: nome definido (`=Inversores`),
    /// intervalo de outra aba (`=Dados!A2:A9`) ou `INDIRETO(E28)`, que muda
    /// conforme outra célula (lista dependente). Texto como o motor guarda
    /// (pt-BR, com `=`). As opções saem de [`crate::planilha::lista_validacao`].
    Formula { formula: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuandoValidacao {
    /// Alerta "Aviso" do Excel: pergunta e deixa continuar se a pessoa quiser.
    Avisar,
    /// Alerta "Parar": o valor é recusado.
    Recusar,
    /// Alerta "Informações": só informa; o valor entra de qualquer jeito.
    Informar,
}

/// Estilo do alerta de erro, com o nome da janela do Excel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstiloAlerta {
    Parar,
    Aviso,
    Informacoes,
}

/// Mensagem de entrada (aparece ao selecionar a célula) e título do alerta.
/// O texto do alerta continua em [`Validacao::mensagem`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MensagensValidacao {
    /// Título da mensagem de entrada (até 32 caracteres no Excel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrada_titulo: Option<String>,
    /// Texto da mensagem de entrada (até 255 caracteres no Excel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrada_texto: Option<String>,
    /// Título da janela do alerta de erro (até 32 caracteres no Excel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub erro_titulo: Option<String>,
}

impl MensagensValidacao {
    pub fn vazio(&self) -> bool {
        *self == Self::default()
    }
}

/// O que a GUI mostra quando o valor digitado viola a regra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlertaErro {
    pub estilo: EstiloAlerta,
    pub titulo: String,
    pub texto: String,
    /// Aviso e Informações deixam a pessoa seguir com o valor; Parar não.
    pub permite_continuar: bool,
}

fn nao_vazio(t: &Option<String>) -> Option<&str> {
    t.as_deref().map(str::trim).filter(|t| !t.is_empty())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Validacao {
    pub ranges: Vec<Rect>,
    pub regra: Regra,
    pub quando: QuandoValidacao,
    /// Texto do alerta de erro.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mensagem: Option<String>,
    /// Mensagem de entrada e título do alerta.
    #[serde(default, skip_serializing_if = "MensagensValidacao::vazio")]
    pub mensagens: MensagensValidacao,
}

impl Validacao {
    /// A mensagem de entrada `(título, texto)` a mostrar ao selecionar a
    /// célula; `None` quando a regra não tem título nem texto.
    pub fn mensagem_de_entrada(&self) -> Option<(String, String)> {
        let titulo = nao_vazio(&self.mensagens.entrada_titulo);
        let texto = nao_vazio(&self.mensagens.entrada_texto);
        if titulo.is_none() && texto.is_none() {
            return None;
        }
        Some((titulo.unwrap_or("").to_string(), texto.unwrap_or("").to_string()))
    }

    pub fn estilo_do_alerta(&self) -> EstiloAlerta {
        match self.quando {
            QuandoValidacao::Recusar => EstiloAlerta::Parar,
            QuandoValidacao::Avisar => EstiloAlerta::Aviso,
            QuandoValidacao::Informar => EstiloAlerta::Informacoes,
        }
    }

    /// O alerta de erro: título e texto próprios, ou o `motivo` do
    /// [`validar`] quando a regra não define texto.
    pub fn alerta_de_erro(&self, motivo: Option<&str>) -> AlertaErro {
        let estilo = self.estilo_do_alerta();
        AlertaErro {
            estilo,
            titulo: nao_vazio(&self.mensagens.erro_titulo).unwrap_or("Microsoft Excel").to_string(),
            texto: nao_vazio(&self.mensagem).or(motivo).unwrap_or("O valor não é válido.").to_string(),
            permite_continuar: estilo != EstiloAlerta::Parar,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultadoValidacao {
    pub ok: bool,
    pub motivo: Option<String>,
}

fn value_to_text(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Text(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => if *b { "VERDADEIRO".to_string() } else { "FALSO".to_string() },
        Value::Error(e) => e.to_string(),
    }
}

/// Porta da regex /^^[^\s@]+@[^\s@]+\.[^\s@]+$/ do frontend
/// (a âncora inicial duplicada no comentário não afeta a implementação).
/// Um "@" ou ponto em qualquer lugar não basta para reconhecer e-mail.
fn email_valido(texto: &str) -> bool {
    let Some((local, dominio)) = texto.split_once('@') else {
        return false;
    };
    if local.is_empty() || dominio.is_empty()
        || local.chars().any(|c| c.is_whitespace() || c == '@')
        || dominio.chars().any(|c| c.is_whitespace() || c == '@')
    {
        return false;
    }
    let Some((prefixo, sufixo)) = dominio.rsplit_once('.') else {
        return false;
    };
    !prefixo.is_empty() && !sufixo.is_empty()
}

/// Sem dependência de regex no core: aplica as mesmas regras da expressão
/// ^(https?://)?[\w.-]+\.[a-z]{2,}(:\d+)?(/.*)?$ (case-insensitive).
fn url_valida(texto: &str) -> bool {
    let protocolo = texto.to_ascii_lowercase();
    let host_e_caminho = if protocolo.starts_with("https://") {
        &texto[8..]
    } else if protocolo.starts_with("http://") {
        &texto[7..]
    } else {
        texto
    };
    let (host_com_porta, caminho) = host_e_caminho
        .split_once('/')
        .unwrap_or((host_e_caminho, ""));
    if caminho.chars().any(|c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')) {
        return false;
    }
    let host = if let Some((host, porta)) = host_com_porta.rsplit_once(':') {
        if porta.is_empty() || !porta.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        host
    } else {
        host_com_porta
    };
    let Some((prefixo, tld)) = host.rsplit_once('.') else {
        return false;
    };
    !prefixo.is_empty()
        && tld.len() >= 2
        && tld.bytes().all(|b| b.is_ascii_alphabetic())
        && prefixo.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

fn parse_date_ms(v: &Value) -> Option<i64> {
    match v {
        Value::Empty => None,
        Value::Number(n) => Some(*n as i64),
        Value::Text(s) => {
            let str_val = s.trim();
            if str_val.is_empty() {
                return None;
            }
            // DD/MM/YYYY
            let parts: Vec<&str> = str_val.split('/').collect();
            if parts.len() == 3 {
                if let (Ok(d), Ok(m), Ok(y)) = (
                    parts[0].parse::<i32>(),
                    parts[1].parse::<i32>(),
                    parts[2].parse::<i32>(),
                ) {
                    return Some((y as i64) * 372 + (m as i64) * 31 + (d as i64));
                }
            }
            // O formulário TypeScript (input date) produz YYYY-MM-DD.
            // Normalizar os dois formatos para a mesma ordem cronológica.
            if let Some((y, m, d)) = str_val.split_once('-').and_then(|(y, resto)| {
                let (m, d) = resto.split_once('-')?;
                Some((y.parse::<i32>().ok()?, m.parse::<u32>().ok()?, d.parse::<u32>().ok()?))
            }) {
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    return Some((y as i64) * 372 + (m as i64) * 31 + d as i64);
                }
            }
            str_val.parse::<i64>().ok()
        }
        _ => None,
    }
}

/// `HH:MM`, `HH:MM:SS` ou número de série → segundos desde a meia-noite.
pub fn parse_hora_segundos(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) if n.is_finite() => Some(((n - n.floor()) * 86_400.0).round() as i64 % 86_400),
        Value::Text(t) => {
            let partes: Vec<&str> = t.trim().split(':').collect();
            if !(2..=3).contains(&partes.len()) {
                return None;
            }
            let h = partes[0].trim().parse::<i64>().ok()?;
            let m = partes[1].trim().parse::<i64>().ok()?;
            let sg = match partes.get(2) {
                Some(x) => x.trim().parse::<i64>().ok()?,
                None => 0,
            };
            ((0..24).contains(&h) && (0..60).contains(&m) && (0..60).contains(&sg)).then_some(h * 3600 + m * 60 + sg)
        }
        _ => None,
    }
}

/// Segundos do dia → `HH:MM` (ou `HH:MM:SS` quando há segundos).
pub fn formatar_hora(segundos: i64) -> String {
    let (h, m, sg) = (segundos / 3600, segundos % 3600 / 60, segundos % 60);
    if sg == 0 { format!("{h:02}:{m:02}") } else { format!("{h:02}:{m:02}:{sg:02}") }
}

pub fn validar<F>(valor: &Value, regra: &Regra, sheet_lookup: Option<F>) -> ResultadoValidacao
where
    F: Fn(u32, u32) -> Value,
{
    if matches!(valor, Value::Empty) {
        return ResultadoValidacao { ok: true, motivo: None };
    }

    let text_val = value_to_text(valor).trim().to_string();
    if text_val.is_empty() {
        return ResultadoValidacao { ok: true, motivo: None };
    }

    match regra {
        Regra::Lista { valores } => {
            let found = valores.iter().any(|item| item.trim() == text_val);
            if found {
                ResultadoValidacao { ok: true, motivo: None }
            } else {
                ResultadoValidacao {
                    ok: false,
                    motivo: Some(crate::trf!("O valor \"{}\" não está presente na lista de opções.", text_val)),
                }
            }
        }
        Regra::Intervalo { origem } => {
            let lookup = match sheet_lookup {
                Some(f) => f,
                None => return ResultadoValidacao { ok: true, motivo: None },
            };
            let origem = origem.normalized();
            // Evita percorrer uma coluna inteira (10 milhões de células) a
            // cada tecla ou célula colada. O formulário usa o mesmo limite.
            if origem.area() > 10_000 {
                return ResultadoValidacao {
                    ok: false,
                    motivo: Some(crate::tr!("O intervalo de origem excede 10.000 células.").into()),
                };
            }
            // Antes alocava uma Vec de todas as opções. Comparar em streaming
            // reduz o uso de memória e interrompe na primeira correspondência.
            for r in origem.r0..=origem.r1 {
                for c in origem.c0..=origem.c1 {
                    let txt = value_to_text(&lookup(r, c));
                    if txt.trim() == text_val {
                        return ResultadoValidacao { ok: true, motivo: None };
                    }
                }
            }
            ResultadoValidacao {
                ok: false,
                motivo: Some(crate::trf!("O valor \"{}\" não pertence ao intervalo especificado.", text_val)),
            }
        }
        Regra::Numero { inteiro: true, .. } if to_number(valor).is_ok_and(|n| n.fract() != 0.0) => {
            ResultadoValidacao { ok: false, motivo: Some(crate::tr!("O valor deve ser um número inteiro.")) }
        }
        Regra::Numero { op, a, b, .. } => {
            // Em validation.ts, toNumber usa a mesma coerção das fórmulas:
            // "1,5", "R$ 1.250,50", "25%" e booleanos são válidos.
            let num = match to_number(valor) {
                Ok(n) if !n.is_nan() => n,
                _ => {
                    return ResultadoValidacao {
                        ok: false,
                        motivo: Some(crate::tr!("O valor digitado não é um número válido.")),
                    };
                }
            };
            match op {
                NumeroOp::Entre => {
                    let min_val = a.min(b.unwrap_or(*a));
                    let max_val = a.max(b.unwrap_or(*a));
                    let ok = num >= min_val && num <= max_val;
                    if ok {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::trf!("O número {} deve estar entre {} e {}.", num, min_val, max_val)),
                        }
                    }
                }
                NumeroOp::Maior => {
                    if num > *a {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::trf!("O número {} deve ser maior que {}.", num, a)),
                        }
                    }
                }
                NumeroOp::Menor => {
                    if num < *a {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::trf!("O número {} deve ser menor que {}.", num, a)),
                        }
                    }
                }
                NumeroOp::Igual => {
                    if num == *a {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::trf!("O número {} deve ser igual a {}.", num, a)),
                        }
                    }
                }
                NumeroOp::Fora | NumeroOp::Diferente | NumeroOp::MaiorIgual | NumeroOp::MenorIgual => {
                    let (min_val, max_val) = (a.min(b.unwrap_or(*a)), a.max(b.unwrap_or(*a)));
                    let (ok, regra) = match op {
                        NumeroOp::Fora => (num < min_val || num > max_val, format!("fora de {min_val} a {max_val}")),
                        NumeroOp::Diferente => (num != *a, format!("diferente de {a}")),
                        NumeroOp::MaiorIgual => (num >= *a, format!("maior ou igual a {a}")),
                        _ => (num <= *a, format!("menor ou igual a {a}")),
                    };
                    ResultadoValidacao { ok, motivo: (!ok).then(|| crate::trf!("O número {num} deve ser {regra}.")) }
                }
            }
        }
        Regra::Data { op, a, b } => {
            let date_ms = match parse_date_ms(valor) {
                Some(ms) => ms,
                None => {
                    return ResultadoValidacao {
                        ok: false,
                        motivo: Some(crate::tr!("O valor digitado não é uma data válida.")),
                    };
                }
            };
            let date_a = parse_date_ms(&Value::Text(a.clone()));
            let date_b = b.as_ref().and_then(|str_b| parse_date_ms(&Value::Text(str_b.clone())));

            let da = match date_a {
                Some(ms) => ms,
                None => return ResultadoValidacao { ok: true, motivo: None },
            };

            match op {
                DataOp::Entre => {
                    let min_val = da.min(date_b.unwrap_or(da));
                    let max_val = da.max(date_b.unwrap_or(da));
                    let ok = date_ms >= min_val && date_ms <= max_val;
                    if ok {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::tr!("A data está fora do intervalo permitido.")),
                        }
                    }
                }
                DataOp::Antes => {
                    if date_ms < da {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::tr!("A data deve ser anterior à limite.")),
                        }
                    }
                }
                DataOp::Depois => {
                    if date_ms > da {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::tr!("A data deve ser posterior à limite.")),
                        }
                    }
                }
            }
        }
        Regra::Texto { op, valor: target_val } => {
            let target = target_val.as_deref().unwrap_or("").trim();
            match op {
                TextoOp::Contem => {
                    if text_val.contains(target) {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(format!("O texto deve conter \"{}\".", target)),
                        }
                    }
                }
                TextoOp::Comeca => {
                    if text_val.starts_with(target) {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::trf!("O texto deve começar com \"{}\".", target)),
                        }
                    }
                }
                TextoOp::Email => {
                    let is_email = email_valido(&text_val);
                    if is_email {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::tr!("O valor deve ser um e-mail válido.")),
                        }
                    }
                }
                TextoOp::Url => {
                    let is_url = url_valida(&text_val);
                    if is_url {
                        ResultadoValidacao { ok: true, motivo: None }
                    } else {
                        ResultadoValidacao {
                            ok: false,
                            motivo: Some(crate::tr!("O valor deve ser um link (URL) válido.")),
                        }
                    }
                }
            }
        }
        // Sem a pasta não dá para montar a lista: quem confere é
        // `Workbook::validar_na_pasta`.
        Regra::Formula { .. } => ResultadoValidacao { ok: true, motivo: None },
        Regra::Hora { op, a, b } => {
            let Some(t) = parse_hora_segundos(valor) else {
                return ResultadoValidacao { ok: false, motivo: Some(crate::tr!("O valor digitado não é uma hora válida.")) };
            };
            let Some(ta) = parse_hora_segundos(&Value::Text(a.clone())) else {
                return ResultadoValidacao { ok: true, motivo: None };
            };
            let tb = b.as_ref().and_then(|x| parse_hora_segundos(&Value::Text(x.clone()))).unwrap_or(ta);
            let (lo, hi) = (ta.min(tb), ta.max(tb));
            let (ok, exige) = match op {
                NumeroOp::Entre => (t >= lo && t <= hi, format!("entre {} e {}", formatar_hora(lo), formatar_hora(hi))),
                NumeroOp::Fora => (t < lo || t > hi, format!("fora de {} a {}", formatar_hora(lo), formatar_hora(hi))),
                NumeroOp::Igual => (t == ta, format!("igual a {}", formatar_hora(ta))),
                NumeroOp::Diferente => (t != ta, format!("diferente de {}", formatar_hora(ta))),
                NumeroOp::Maior => (t > ta, format!("depois de {}", formatar_hora(ta))),
                NumeroOp::Menor => (t < ta, format!("antes de {}", formatar_hora(ta))),
                NumeroOp::MaiorIgual => (t >= ta, format!("{} ou depois", formatar_hora(ta))),
                NumeroOp::MenorIgual => (t <= ta, format!("{} ou antes", formatar_hora(ta))),
            };
            ResultadoValidacao { ok, motivo: (!ok).then(|| format!("A hora {} deve estar {exige}.", formatar_hora(t))) }
        }
        Regra::Caixa { marcado, desmarcado } => {
            if text_val == *marcado || text_val == *desmarcado {
                ResultadoValidacao { ok: true, motivo: None }
            } else {
                ResultadoValidacao {
                    ok: false,
                    motivo: Some(crate::trf!("O valor deve ser \"{}\" ou \"{}\".", marcado, desmarcado)),
                }
            }
        }
    }
}

pub fn reflow_validations(
    validations: &[Validacao],
    axis_is_row: bool,
    at: u32,
    count: u32,
    removing: bool,
) -> Vec<Validacao> {
    let shift_start = |i: u32| -> u32 {
        if removing {
            if i < at {
                i
            } else if i < at + count {
                at
            } else {
                i - count
            }
        } else {
            if i >= at { i + count } else { i }
        }
    };

    let shift_end = |i: u32| -> u32 {
        if removing {
            if i < at {
                i
            } else if i < at + count {
                at.saturating_sub(1)
            } else {
                i - count
            }
        } else {
            if i >= at { i + count } else { i }
        }
    };

    let mut next = Vec::new();
    for v in validations {
        let mut ranges = Vec::new();
        for rect in &v.ranges {
            if removing && at == 0 && count > 0
                && (if axis_is_row { rect.r1 < count } else { rect.c1 < count })
            {
                // O fim seria -1 no TS, e o retângulo não sobrevive.
                continue;
            }
            let a = if axis_is_row { shift_start(rect.r0) } else { rect.r0 };
            let b = if axis_is_row { shift_end(rect.r1) } else { rect.r1 };
            let c = if !axis_is_row { shift_start(rect.c0) } else { rect.c0 };
            let d = if !axis_is_row { shift_end(rect.c1) } else { rect.c1 };
            if b >= a && d >= c {
                ranges.push(Rect { r0: a, r1: b, c0: c, c1: d, sheet: rect.sheet.clone() });
            }
        }
        let mut regra = v.regra.clone();
        if let Regra::Intervalo { origem: r } = &v.regra {
            let a = if axis_is_row { shift_start(r.r0) } else { r.r0 };
            let b = if axis_is_row { shift_end(r.r1) } else { r.r1 };
            let c = if !axis_is_row { shift_start(r.c0) } else { r.c0 };
            let d = if !axis_is_row { shift_end(r.c1) } else { r.c1 };
            if b >= a && d >= c {
                regra = Regra::Intervalo {
                    origem: Rect { r0: a, r1: b, c0: c, c1: d, sheet: r.sheet.clone() },
                };
            }
        }
        if !ranges.is_empty() {
            next.push(Validacao {
                ranges,
                regra,
                quando: v.quando,
                mensagem: v.mensagem.clone(),
                mensagens: v.mensagens.clone(),
            });
        }
    }
    next
}

#[cfg(test)]
mod testes_datas_formulario {
    use super::*;

    #[test]
    fn aceita_datas_iso_do_formulario_typescript() {
        let regra = Regra::Data {
            op: DataOp::Entre,
            a: "2026-10-01".into(),
            b: Some("2026-10-31".into()),
        };
        assert!(validar(&Value::Text("2026-10-09".into()), &regra,
            None::<fn(u32, u32) -> Value>).ok);
        assert!(!validar(&Value::Text("2026-11-01".into()), &regra,
            None::<fn(u32, u32) -> Value>).ok);
        assert!(validar(&Value::Text("09/10/2026".into()), &regra,
            None::<fn(u32, u32) -> Value>).ok);
    }
}

#[cfg(test)]
mod testes_compatibilidade_typescript {
    use super::*;

    fn numero(valor: Value, alvo: f64) -> bool {
        validar(
            &valor,
            &Regra::Numero { op: NumeroOp::Igual, a: alvo, b: None, inteiro: false },
            None::<fn(u32, u32) -> Value>,
        ).ok
    }

    #[test]
    fn numero_aceita_moeda_decimal_percentual_e_booleano_do_ts() {
        assert!(numero(Value::Text("1,5".into()), 1.5));
        assert!(numero(Value::Text("R$ 1.250,50".into()), 1250.5));
        assert!(numero(Value::Text("25%".into()), 0.25));
        assert!(numero(Value::Bool(true), 1.0));
        assert!(!numero(Value::Text("sem número".into()), 1.0));
        // O TypeScript usa ===: igualdade aproximada não pode validar.
        assert!(!numero(Value::Number(1.000_000_000_1), 1.0));
    }

    #[test]
    fn email_e_url_seguem_expressao_do_formulario_typescript() {
        assert!(email_valido("pessoa@exemplo.com"));
        assert!(email_valido("a+b@exemplo.com.br"));
        for invalido in ["sem-arroba.com", "pessoa@localhost", "@exemplo.com", "a@@b.com", "a b@x.com"] {
            assert!(!email_valido(invalido), "e-mail indevido: {invalido}");
        }
        for valido in ["exemplo.com", "https://exemplo.com/path?q=1", "HTTP://site.com:443/a", "dominio.com.br"] {
            assert!(url_valida(valido), "URL válida recusada: {valido}");
        }
        for invalido in ["http://sem-ponto", "ftp://exemplo.com", "https://exemplo.c", "https://exemplo.com:abc",
                         "https://exemplo.com?sem-barra=1", "exemplo.com outro"] {
            assert!(!url_valida(invalido), "URL inválida aceita: {invalido}");
        }
    }

    #[test]
    fn excluir_primeira_linha_ou_coluna_descarta_regra_totalmente_apagada() {
        let origem = Rect { r0: 0, c0: 0, r1: 0, c1: 0, sheet: None };
        let v = Validacao {
            ranges: vec![origem.clone()],
            regra: Regra::Lista { valores: vec!["Sim".into()] },
            quando: QuandoValidacao::Recusar,
            mensagem: None,
            mensagens: Default::default(),
        };
        assert!(reflow_validations(&[v.clone()], true, 0, 1, true).is_empty());
        assert!(reflow_validations(&[v.clone()], false, 0, 1, true).is_empty());
        assert_eq!(reflow_validations(&[v.clone()], true, 0, 1, false)[0].ranges[0].r0, 1);
        assert_eq!(reflow_validations(&[v.clone()], false, 0, 1, false)[0].ranges[0].c0, 1);
    }
}

#[cfg(test)]
mod testes_hora_e_alertas {
    use super::*;

    fn sem_pasta(v: &Value, r: &Regra) -> ResultadoValidacao {
        validar(v, r, None::<fn(u32, u32) -> Value>)
    }

    fn val(regra: Regra, quando: QuandoValidacao) -> Validacao {
        Validacao {
            ranges: vec![Rect { r0: 0, c0: 0, r1: 0, c1: 0, sheet: None }],
            regra, quando, mensagem: None, mensagens: Default::default(),
        }
    }

    #[test]
    fn hora_entre_aceita_texto_e_numero_de_serie() {
        let r = Regra::Hora { op: NumeroOp::Entre, a: "08:00".into(), b: Some("18:30".into()) };
        assert!(sem_pasta(&Value::Text("09:15".into()), &r).ok);
        assert!(sem_pasta(&Value::Text("18:30:00".into()), &r).ok);
        assert!(sem_pasta(&Value::Number(0.5), &r).ok, "12:00");
        assert!(sem_pasta(&Value::Number(45_000.75), &r).ok, "só a fração: 18:00");
        assert!(!sem_pasta(&Value::Text("07:59".into()), &r).ok);
        assert!(!sem_pasta(&Value::Number(0.99), &r).ok);
        let nao_hora = sem_pasta(&Value::Text("abc".into()), &r);
        assert!(!nao_hora.ok && nao_hora.motivo.unwrap().contains("hora válida"));
        assert!(!sem_pasta(&Value::Text("25:00".into()), &r).ok);
        assert!(sem_pasta(&Value::Empty, &r).ok, "vazio passa");
    }

    #[test]
    fn hora_outros_operadores() {
        let maior = Regra::Hora { op: NumeroOp::Maior, a: "12:00".into(), b: None };
        assert!(sem_pasta(&Value::Text("12:01".into()), &maior).ok);
        assert!(!sem_pasta(&Value::Text("12:00".into()), &maior).ok);
        let fora = Regra::Hora { op: NumeroOp::Fora, a: "12:00".into(), b: Some("13:00".into()) };
        assert!(sem_pasta(&Value::Text("11:00".into()), &fora).ok);
        assert!(!sem_pasta(&Value::Text("12:30".into()), &fora).ok);
        assert_eq!(formatar_hora(3661), "01:01:01");
    }

    #[test]
    fn alerta_de_erro_usa_titulo_texto_e_estilo() {
        let mut v = val(Regra::Lista { valores: vec!["a".into()] }, QuandoValidacao::Informar);
        let a = v.alerta_de_erro(Some("motivo do motor"));
        assert_eq!(a.estilo, EstiloAlerta::Informacoes);
        assert!(a.permite_continuar);
        assert_eq!(a.texto, "motivo do motor");
        v.mensagem = Some("Escolha da lista".into());
        v.mensagens.erro_titulo = Some("Atenção".into());
        v.quando = QuandoValidacao::Recusar;
        let a = v.alerta_de_erro(Some("x"));
        assert_eq!((a.estilo, a.titulo.as_str(), a.texto.as_str(), a.permite_continuar),
            (EstiloAlerta::Parar, "Atenção", "Escolha da lista", false));
        v.quando = QuandoValidacao::Avisar;
        assert_eq!(v.alerta_de_erro(None).estilo, EstiloAlerta::Aviso);
    }

    #[test]
    fn mensagem_de_entrada_so_existe_com_titulo_ou_texto() {
        let mut v = val(Regra::Lista { valores: vec![] }, QuandoValidacao::Recusar);
        assert_eq!(v.mensagem_de_entrada(), None);
        v.mensagens.entrada_texto = Some("  ".into());
        assert_eq!(v.mensagem_de_entrada(), None);
        v.mensagens.entrada_titulo = Some("Hora".into());
        assert_eq!(v.mensagem_de_entrada(), Some(("Hora".into(), "".into())));
        v.mensagens.entrada_texto = Some("Use HH:MM".into());
        assert_eq!(v.mensagem_de_entrada(), Some(("Hora".into(), "Use HH:MM".into())));
    }

    #[test]
    fn json_antigo_le_e_novo_volta_igual() {
        let antigo = r#"{"ranges":[{"r0":0,"c0":0,"r1":0,"c1":0}],"regra":{"tipo":"lista","valores":["a"]},"quando":"recusar"}"#;
        let v: Validacao = serde_json::from_str(antigo).unwrap();
        assert!(v.mensagens.vazio());
        assert!(!serde_json::to_string(&v).unwrap().contains("mensagens"));
        let mut n = val(Regra::Hora { op: NumeroOp::Entre, a: "08:00".into(), b: Some("12:00".into()) }, QuandoValidacao::Informar);
        n.mensagens = MensagensValidacao { entrada_titulo: Some("T".into()), entrada_texto: Some("X".into()), erro_titulo: Some("E".into()) };
        let json = serde_json::to_string(&n).unwrap();
        assert!(json.contains("\"informar\"") && json.contains("\"hora\""), "{json}");
        assert_eq!(serde_json::from_str::<Validacao>(&json).unwrap(), n);
    }
}
