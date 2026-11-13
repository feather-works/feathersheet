//! Autocompletar no editor de macros (#8): ao digitar `sheet.` ou o começo de um nome,
//! a lista mostra os métodos da API com a assinatura e o exemplo do guia (`ajuda_macros`),
//! a mesma fonte da Ajuda F1: método novo no guia já aparece aqui.

/// Uma sugestão: o nome que entra no código, a assinatura e o exemplo do guia.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::app) struct Sugestao {
    pub nome: &'static str,
    pub assinatura: &'static str,
    pub exemplo: &'static str,
    /// Função ou método: aceitar escreve também o `(`.
    pub chamada: bool,
}

/// (objeto, sugestão) de cada assinatura do guia: `sheet.value(address)` é do `sheet`,
/// `range.values()` do intervalo, `cell(address)` é solto (objeto vazio).
fn catalogo() -> &'static [(&'static str, Sugestao)] {
    static CATALOGO: std::sync::OnceLock<Vec<(&'static str, Sugestao)>> = std::sync::OnceLock::new();
    CATALOGO.get_or_init(|| {
        let mut lista: Vec<(&'static str, Sugestao)> = Vec::new();
        for (_, itens) in crate::app::ajuda_macros::GUIA {
            for &(_, assinaturas, exemplo) in itens.iter() {
                for assinatura in assinaturas.split(" · ").map(str::trim) {
                    let cabeca = assinatura.split('(').next().unwrap_or("").trim();
                    if cabeca.is_empty() || !cabeca.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '_') {
                        continue;
                    }
                    let (objeto, nome) = cabeca.rsplit_once('.').unwrap_or(("", cabeca));
                    let objeto = objeto.rsplit('.').next().unwrap_or("");
                    let s = Sugestao { nome, assinatura, exemplo, chamada: assinatura.contains('(') };
                    if !lista.iter().any(|(o, x)| *o == objeto && x.nome == nome) {
                        lista.push((objeto, s));
                    }
                }
            }
        }
        // Os objetos de entrada também se completam soltos (`sh` → `sheet`).
        for objeto in ["sheet", "wb", "app"] {
            if lista.iter().any(|(o, _)| *o == objeto) && !lista.iter().any(|(o, s)| o.is_empty() && s.nome == objeto) {
                lista.push(("", Sugestao { nome: objeto, assinatura: objeto, exemplo: "", chamada: false }));
            }
        }
        lista
    })
}

/// O que está sendo digitado antes do cursor: (objeto antes do ponto, começo do nome).
/// `None` dentro de texto entre aspas ou de comentário `//`.
pub(in crate::app) fn contexto(antes: &str) -> Option<(String, String)> {
    if antes.contains("//") || antes.chars().filter(|&c| c == '"').count() % 2 == 1
        || antes.chars().filter(|&c| c == '\'').count() % 2 == 1 || antes.chars().filter(|&c| c == '`').count() % 2 == 1 {
        return None;
    }
    let palavra = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let chars: Vec<char> = antes.chars().collect();
    let mut i = chars.len();
    while i > 0 && palavra(chars[i - 1]) { i -= 1; }
    let parcial: String = chars[i..].iter().collect();
    if i == 0 || chars[i - 1] != '.' {
        return Some((String::new(), parcial));
    }
    let mut j = i - 1;
    // `sheet.range("A1").` — o que vem depois de uma chamada é, quase sempre, um intervalo.
    if j > 0 && chars[j - 1] == ')' { return Some(("range".into(), parcial)); }
    while j > 0 && palavra(chars[j - 1]) { j -= 1; }
    Some((chars[j..i - 1].iter().collect(), parcial))
}

/// As sugestões para (objeto, começo): no máximo 12, o começo exato (com maiúsculas) primeiro.
pub(in crate::app) fn sugestoes(objeto: &str, parcial: &str) -> Vec<Sugestao> {
    let solto = objeto.is_empty();
    // Solto, só a partir da segunda letra: não abrir a lista a cada palavra do código.
    if solto && parcial.chars().count() < 2 { return Vec::new(); }
    let objeto = match objeto { "planilha" => "sheet", "workbook" => "wb", "" => "", o => o };
    let conhecido = catalogo().iter().any(|(o, _)| *o == objeto);
    let minusculo = parcial.to_lowercase();
    let mut lista: Vec<Sugestao> = catalogo().iter()
        // Variável de tipo desconhecido (`r.`, `aba.`): métodos do intervalo e da aba.
        .filter(|(o, _)| if conhecido { *o == objeto } else { !solto && matches!(*o, "range" | "sheet") })
        .map(|(_, s)| *s)
        .filter(|s| s.nome.to_lowercase().starts_with(&minusculo) && s.nome != parcial)
        .collect();
    lista.sort_by_key(|s| (!s.nome.starts_with(parcial), s.nome.to_lowercase()));
    lista.dedup_by_key(|s| s.nome);
    lista.truncate(12);
    lista
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sheet_ponto_lista_os_metodos_da_aba_e_filtra_pelo_comeco() {
        assert_eq!(contexto("  const x = sheet.va"), Some(("sheet".into(), "va".into())));
        let nomes: Vec<_> = sugestoes("sheet", "va").iter().map(|s| s.nome).collect();
        assert_eq!(nomes, ["value"]);
        let s = sugestoes("sheet", "setBo")[0];
        assert_eq!((s.nome, s.chamada), ("setBold", true));
        assert!(s.assinatura.starts_with("sheet.setBold("));
        assert!(sugestoes("sheet", "").len() >= 10, "só o ponto já mostra a lista");
    }

    #[test]
    fn solto_pede_duas_letras_e_nao_abre_em_texto_nem_comentario() {
        assert!(sugestoes("", "s").is_empty());
        assert!(sugestoes("", "sh").iter().any(|s| s.nome == "sheet"));
        assert_eq!(contexto("sheet.log(\"sheet.va"), None);
        assert_eq!(contexto("// sheet.va"), None);
        // Depois de uma chamada, os métodos do intervalo.
        assert_eq!(contexto("sheet.range(\"A1\").va"), Some(("range".into(), "va".into())));
        assert!(sugestoes("range", "va").iter().any(|s| s.nome == "values"));
        // Variável desconhecida: intervalo e aba.
        assert!(!sugestoes("r", "set").is_empty());
    }
}
