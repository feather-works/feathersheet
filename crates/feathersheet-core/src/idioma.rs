//! Tradução dos textos prontos do motor (mensagens de erro, rótulos da dinâmica, nomes de
//! meses): o motor não carrega dicionário (regra 11, sem dependência). Quem tem o dicionário
//! — o app — entrega duas funções em [`definir_tradutor`], e todo texto pronto do motor
//! passa por [`tr!`](crate::tr) ou [`trf!`](crate::trf). Sem tradutor (testes, MCP avulso),
//! sai o português de sempre.
//!
//! A chave do dicionário é o texto em português, igual à da interface
//! (`src/idiomas/interface.<idioma>.json`).

use std::sync::RwLock;

/// Texto exato em português → traduzido (`None`: sem tradução, fica o português).
pub type Exato = fn(&str) -> Option<String>;
/// Modelo do `format!` em português + o texto já formatado → o texto na língua.
pub type Formatado = fn(&str, String) -> String;

static TRADUTOR: RwLock<Option<(Exato, Formatado)>> = RwLock::new(None);

/// Liga o motor ao dicionário de quem o hospeda.
pub fn definir_tradutor(exato: Exato, formatado: Formatado) {
    *TRADUTOR.write().unwrap_or_else(|e| e.into_inner()) = Some((exato, formatado));
}

/// O texto pronto na língua do app (o próprio texto sem tradutor ou sem a chave).
pub fn tr(pt: &str) -> String {
    let tradutor = *TRADUTOR.read().unwrap_or_else(|e| e.into_inner());
    tradutor.and_then(|(exato, _)| exato(pt)).unwrap_or_else(|| pt.to_owned())
}

/// O texto formatado na língua do app: `modelo` é o literal do `format!`.
pub fn traduzir_formatado(modelo: &str, formatado: String) -> String {
    let tradutor = *TRADUTOR.read().unwrap_or_else(|e| e.into_inner());
    match tradutor {
        Some((_, formatar)) => formatar(modelo, formatado),
        None => formatado,
    }
}

/// O `texto` (já na língua do app) saiu do modelo `modelo_pt`? Confere os trechos fixos do
/// modelo traduzido, na ordem. É como a explicação de erro reconhece um motivo sem depender
/// de palavra em português.
pub fn veio_de(texto: &str, modelo_pt: &str) -> bool {
    let modelo = tr(modelo_pt);
    let mut resto = texto;
    let mut fixo = String::new();
    let mut letras = modelo.chars().peekable();
    let mut casa = |fixo: &mut String, resto: &mut &str| -> bool {
        if fixo.is_empty() { return true; }
        let achou = resto.find(fixo.as_str());
        if let Some(i) = achou { *resto = &resto[i + fixo.len()..]; }
        fixo.clear();
        achou.is_some()
    };
    while let Some(c) = letras.next() {
        match c {
            '{' if letras.peek() == Some(&'{') => { letras.next(); fixo.push('{'); }
            '}' if letras.peek() == Some(&'}') => { letras.next(); fixo.push('}'); }
            '{' => {
                if !casa(&mut fixo, &mut resto) { return false; }
                for d in letras.by_ref() { if d == '}' { break; } }
            }
            c => fixo.push(c),
        }
    }
    casa(&mut fixo, &mut resto)
}

/// `tr!("texto")`: o texto pronto na língua do app, como `String`.
#[macro_export]
macro_rules! tr {
    ($e:expr) => { $crate::idioma::tr($e) };
}

/// `trf!("modelo {x}", ...)`: como `format!`, com o modelo traduzido.
#[macro_export]
macro_rules! trf {
    ($modelo:literal $($resto:tt)*) => {
        $crate::idioma::traduzir_formatado($modelo, format!($modelo $($resto)*))
    };
}

#[cfg(test)]
mod testes {
    #[test]
    fn sem_tradutor_sai_o_portugues() {
        assert_eq!(crate::tr!("Total Geral"), "Total Geral");
        let n = 3;
        assert_eq!(crate::trf!("{n} linhas"), "3 linhas");
        assert!(super::veio_de("\"abc\" não é número", "\"{texto}\" não é número"));
        assert!(!super::veio_de("abc não é texto", "\"{texto}\" não é número"));
        assert!(super::veio_de("Sobra \")\" no fim da fórmula", "Sobra \"{}\" no fim da fórmula"));
    }
}
