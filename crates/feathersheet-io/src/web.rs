//! A resposta de uma busca na rede, interpretada pela chave da pergunta
//! (`feathersheet_core::planilha::assincrono`). A interface só busca e devolve o
//! texto cru; o que ele vira é daqui. Tradução do miolo do `fn-web.ts`.

use serde_json::Value as Json;
use feathersheet_core::celulas::input::parse_literal;
use feathersheet_core::celulas::values::{CellError, ErrorCode, Value};
use feathersheet_core::planilha::assincrono::Resposta;

fn erro(code: ErrorCode, motivo: impl Into<String>) -> Resposta {
    Resposta::Valor(Value::Error(CellError::com_motivo(code, motivo)))
}

/// O texto que a busca de `chave` trouxe, já como valor da célula.
pub fn interpretar(chave: &str, texto: &str) -> Resposta {
    if chave.starts_with("web:texto:") {
        return Resposta::Valor(Value::Text(texto.to_string()));
    }
    if let Some(resto) = chave.strip_prefix("web:dados:") {
        let separador = resto.split_once('\u{0}').map(|(_, s)| s).unwrap_or("");
        return dados(texto, separador.chars().next());
    }
    if let Some(resto) = chave.strip_prefix("web:json:") {
        let caminho = resto.split_once('\u{0}').map(|(_, c)| c).unwrap_or("");
        return json(texto, caminho);
    }
    erro(ErrorCode::Value, "pergunta desconhecida")
}

/// `IMPORTARDADOS`: cada campo passa pela mesma leitura da digitação —
/// número vira número, data vira serial.
fn dados(texto: &str, separador: Option<char>) -> Resposta {
    let sep = separador.unwrap_or_else(|| crate::csv::detectar_separador(texto));
    let linhas = crate::csv::dividir_csv(texto, sep);
    if linhas.is_empty() {
        return erro(ErrorCode::Na, &feathersheet_core::tr!("O endereço não devolveu nenhuma linha"));
    }
    let largura = linhas.iter().map(Vec::len).max().unwrap_or(0);
    Resposta::Matriz(
        linhas
            .iter()
            .map(|l| (0..largura).map(|c| parse_literal(l.get(c).map(String::as_str).unwrap_or("")).value).collect())
            .collect(),
    )
}

/// `IMPORTARJSON`: segue o caminho (`dados.itens[0].nome`) e traz o pedaço.
fn json(texto: &str, caminho: &str) -> Resposta {
    let Ok(dado) = serde_json::from_str::<Json>(texto) else {
        return erro(ErrorCode::Value, &feathersheet_core::tr!("O endereço não devolveu um JSON válido"));
    };
    match seguir_caminho(&dado, caminho) {
        Some(alvo) => como_valor(alvo),
        None => erro(ErrorCode::Na, feathersheet_core::trf!("O caminho \"{caminho}\" não existe nesse JSON")),
    }
}

/// `seguirCaminho`: partes separadas por ponto, cada uma um nome e índices
/// entre colchetes.
pub fn seguir_caminho<'a>(dado: &'a Json, caminho: &str) -> Option<&'a Json> {
    let limpo = caminho.trim();
    if limpo.is_empty() {
        return Some(dado);
    }
    let mut atual = dado;
    for parte in limpo.split('.') {
        let parte = parte.trim();
        let fim_do_nome = parte.find('[').unwrap_or(parte.len());
        let (nome, indices) = parte.split_at(fim_do_nome);
        if nome.contains(']') {
            return None;
        }
        // `((?:\[\d+\])*)`: só índices de algarismos, colados.
        let mut numeros = Vec::new();
        let mut resto = indices;
        while !resto.is_empty() {
            let dentro = resto.strip_prefix('[')?;
            let fecha = dentro.find(']')?;
            let n = &dentro[..fecha];
            if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            numeros.push(n.parse::<usize>().ok()?);
            resto = &dentro[fecha + 1..];
        }
        if !nome.is_empty() {
            atual = match atual {
                Json::Object(o) => o.get(nome)?,
                Json::Array(a) => a.get(nome.parse::<usize>().ok()?)?,
                _ => return None,
            };
        }
        for i in numeros {
            atual = atual.as_array()?.get(i)?;
        }
    }
    Some(atual)
}

/// `comoValor`: lista de objetos vira tabela com cabeçalho, lista de listas
/// vira tabela, lista simples vira coluna, objeto solto vira campo e valor.
pub fn como_valor(no: &Json) -> Resposta {
    match no {
        Json::Array(itens) => {
            if itens.is_empty() {
                return erro(ErrorCode::Na, &feathersheet_core::tr!("A lista está vazia"));
            }
            if itens.iter().all(Json::is_object) {
                let mut campos: Vec<&String> = Vec::new();
                for item in itens {
                    for campo in item.as_object().into_iter().flat_map(|o| o.keys()) {
                        if !campos.contains(&campo) {
                            campos.push(campo);
                        }
                    }
                }
                let mut linhas = vec![campos.iter().map(|c| Value::Text((*c).clone())).collect::<Vec<_>>()];
                for item in itens {
                    linhas.push(campos.iter().map(|c| escalar(item.get(c.as_str()))).collect());
                }
                return Resposta::Matriz(linhas);
            }
            if itens.iter().all(Json::is_array) {
                let largura = itens.iter().filter_map(Json::as_array).map(Vec::len).max().unwrap_or(0);
                return Resposta::Matriz(
                    itens
                        .iter()
                        .map(|l| (0..largura).map(|c| escalar(l.as_array().and_then(|a| a.get(c)))).collect())
                        .collect(),
                );
            }
            Resposta::Matriz(itens.iter().map(|i| vec![escalar(Some(i))]).collect())
        }
        Json::Object(o) => {
            if o.is_empty() {
                return erro(ErrorCode::Na, &feathersheet_core::tr!("O objeto está vazio"));
            }
            Resposta::Matriz(o.iter().map(|(c, v)| vec![Value::Text(c.clone()), escalar(Some(v))]).collect())
        }
        outro => Resposta::Valor(escalar(Some(outro))),
    }
}

/// Um valor de célula; o aninhado demais vai como JSON mesmo.
fn escalar(no: Option<&Json>) -> Value {
    match no {
        None | Some(Json::Null) => Value::Empty,
        Some(Json::Bool(b)) => Value::Bool(*b),
        Some(Json::Number(n)) => n.as_f64().map(Value::Number).unwrap_or(Value::Error(CellError::new(ErrorCode::Num))),
        Some(Json::String(s)) => Value::Text(s.clone()),
        Some(outro) => Value::Text(outro.to_string()),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn csv_multilinha_usa_parser_nativo_e_preenche_campos_ausentes() {
        let texto = "Nome;Valor;Observação\r\n\"João;Silva\";1,5;\"linha 1\nlinha 2\"\r\nAna;2\r\n";
        let obtido = interpretar("web:dados:https://exemplo.test\u{0};", texto);
        assert_eq!(obtido, Resposta::Matriz(vec![
            vec![Value::Text("Nome".into()), Value::Text("Valor".into()), Value::Text("Observação".into())],
            vec![Value::Text("João;Silva".into()), Value::Number(1.5), Value::Text("linha 1\nlinha 2".into())],
            vec![Value::Text("Ana".into()), Value::Number(2.0), Value::Empty],
        ]));
        assert!(matches!(
            interpretar("web:dados:https://exemplo.test\u{0};", ""),
            Resposta::Valor(Value::Error(e)) if e.code == ErrorCode::Na
        ));
    }

    #[test]
    fn json_caminhos_matrizes_e_tipos_preservam_comportamento_do_ts() {
        let dados = r#"{"dados":{"itens":[{"nome":"Ana","ativo":true,"meta":{"origem":"api"}},{"nome":"João","ativo":false}]}}"#;
        assert_eq!(
            interpretar("web:json:https://api.test\u{0}dados.itens[1].nome", dados),
            Resposta::Valor(Value::Text("João".into()))
        );
        assert_eq!(
            interpretar("web:json:https://api.test\u{0}dados.itens[0].meta", dados),
            Resposta::Matriz(vec![
                vec![Value::Text("origem".into()), Value::Text("api".into())]
            ])
        );
        assert_eq!(
            interpretar("web:json:https://api.test\u{0}dados.itens[0].meta.origem", dados),
            Resposta::Valor(Value::Text("api".into()))
        );
        assert!(matches!(
            interpretar("web:json:https://api.test\u{0}dados.itens[-1]", dados),
            Resposta::Valor(Value::Error(e)) if e.code == ErrorCode::Na
        ));
        assert!(matches!(
            interpretar("web:json:https://api.test\u{0}", "[]"),
            Resposta::Valor(Value::Error(e)) if e.code == ErrorCode::Na
        ));
    }

    #[test]
    fn dados_json_e_texto() {
        let r = interpretar("web:dados:http://x\u{0}", "nome;valor\nAna;10\n");
        assert_eq!(
            r,
            Resposta::Matriz(vec![
                vec![Value::Text("nome".into()), Value::Text("valor".into())],
                vec![Value::Text("Ana".into()), Value::Number(10.0)],
            ])
        );
        let j = interpretar("web:json:http://x\u{0}dados.itens", r#"{"dados":{"itens":[{"b":1,"a":"x"},{"a":"y","c":true}]}}"#);
        assert_eq!(
            j,
            Resposta::Matriz(vec![
                vec![Value::Text("b".into()), Value::Text("a".into()), Value::Text("c".into())],
                vec![Value::Number(1.0), Value::Text("x".into()), Value::Empty],
                vec![Value::Empty, Value::Text("y".into()), Value::Bool(true)],
            ])
        );
        assert_eq!(interpretar("web:json:u\u{0}a[1]", r#"{"a":[5,6]}"#), Resposta::Valor(Value::Number(6.0)));
        assert!(matches!(interpretar("web:json:u\u{0}b", "{}"), Resposta::Valor(Value::Error(e)) if e.code == ErrorCode::Na));
        assert!(matches!(interpretar("web:json:u\u{0}", "nada"), Resposta::Valor(Value::Error(e)) if e.code == ErrorCode::Value));
        assert_eq!(interpretar("web:texto:u", "oi"), Resposta::Valor(Value::Text("oi".into())));
    }
}
