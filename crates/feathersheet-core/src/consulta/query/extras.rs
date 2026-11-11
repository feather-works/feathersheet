//! Etapas de limpeza que o Power Query tem e o `query.ts` não tinha: formatar
//! texto, preencher, remover linhas, transpor, inverter, dividir por posição,
//! coluna personalizada (fórmula do FeatherSheet) e a chave da mesclagem aproximada.
use super::*;
use crate::celulas::refs::col_name;
use crate::celulas::values::{CellError, ErrorCode};

fn vazio(v: &Value) -> bool {
    matches!(v, Value::Empty) || matches!(v, Value::Text(s) if s.is_empty())
}

/// "Primeira Maiúscula" de cada palavra, o `Text.Proper` do Power Query.
fn titulo(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    let mut inicio = true;
    for ch in s.chars() {
        if ch.is_alphanumeric() {
            if inicio { saida.extend(ch.to_uppercase()); } else { saida.extend(ch.to_lowercase()); }
            inicio = false;
        } else {
            saida.push(ch);
            inicio = true;
        }
    }
    saida
}

/// `como`: aparar, limpar, maiusculas, minusculas, titulo, prefixo, sufixo.
pub(super) fn formatar(mut t: Tabela, col: usize, como: &str, texto: Option<&str>) -> Tabela {
    let extra = texto.unwrap_or("");
    for l in &mut t.linhas {
        let v = celula_ref(l, col);
        let novo = match (como, v) {
            ("prefixo", v) if !vazio(v) => Value::Text(format!("{extra}{}", string_js(v))),
            ("sufixo", v) if !vazio(v) => Value::Text(format!("{}{extra}", string_js(v))),
            (_, Value::Text(s)) => Value::Text(match como {
                "aparar" => s.trim().to_owned(),
                "limpar" => s.chars().filter(|c| !c.is_control()).collect(),
                "maiusculas" => s.to_uppercase(),
                "minusculas" => s.to_lowercase(),
                "titulo" => titulo(s),
                _ => continue,
            }),
            _ => continue,
        };
        gravar_em(l, col, novo);
    }
    t
}

/// Vazias recebem o valor de cima (ou de baixo, com `para_cima`).
pub(super) fn preencher(mut t: Tabela, col: usize, para_cima: bool) -> Tabela {
    let mut ultimo: Option<Value> = None;
    let n = t.linhas.len();
    for i in 0..n {
        let l = &mut t.linhas[if para_cima { n - 1 - i } else { i }];
        if vazio(celula_ref(l, col)) {
            if let Some(v) = &ultimo { gravar_em(l, col, v.clone()); }
        } else {
            ultimo = Some(celula(l, col));
        }
    }
    t
}

/// `modo`: primeiras (tira as N do começo), ultimas (tira as N do fim),
/// manter_ultimas, vazias (a linha inteira vazia).
pub(super) fn remover_linhas(mut t: Tabela, modo: &str, quantas: f64) -> Tabela {
    let n = linhas_do_limite(quantas).min(t.linhas.len());
    match modo {
        "primeiras" => { t.linhas.drain(..n); }
        "ultimas" => { let fim = t.linhas.len() - n; t.linhas.truncate(fim); }
        "manter_ultimas" => { let ini = t.linhas.len() - n; t.linhas.drain(..ini); }
        "vazias" => t.linhas.retain(|l| !l.iter().all(vazio)),
        _ => {}
    }
    t
}

/// Linhas viram colunas; o cabeçalho sai, como no Power Query (Usar 1ª linha
/// como cabeçalho devolve os nomes).
pub(super) fn transpor(t: Tabela) -> Tabela {
    let colunas = t.cabecalho.len().max(t.linhas.iter().map(Vec::len).max().unwrap_or(0));
    Tabela {
        cabecalho: (0..t.linhas.len()).map(coluna_sem_nome).collect(),
        linhas: (0..colunas).map(|c| t.linhas.iter().map(|l| celula(l, c)).collect()).collect(),
    }
}

/// Divide o texto nas `posicoes` (em caracteres, a primeira normalmente 0) ou a
/// cada `cada` caracteres: `Nome.1`, `Nome.2`… no lugar da coluna.
pub(super) fn dividir_posicoes(t: Tabela, col: usize, posicoes: &[usize], cada: Option<usize>) -> Tabela {
    if col >= t.cabecalho.len() { return t; }
    let cortes = |s: &str| -> Vec<Value> {
        let chars: Vec<char> = s.chars().collect();
        let mut inicios: Vec<usize> = match cada {
            Some(n) if n > 0 => (0..chars.len().max(1)).step_by(n).collect(),
            _ => posicoes.to_vec(),
        };
        inicios.sort_unstable();
        inicios.dedup();
        if inicios.first() != Some(&0) { inicios.insert(0, 0); }
        inicios.iter().enumerate().map(|(i, &a)| {
            let b = inicios.get(i + 1).copied().unwrap_or(chars.len()).min(chars.len());
            Value::Text(chars[a.min(b)..b].iter().collect())
        }).collect()
    };
    let pedacos: Vec<Option<Vec<Value>>> = t.linhas.iter()
        .map(|l| (!vazio(celula_ref(l, col))).then(|| cortes(&string_js(celula_ref(l, col))))).collect();
    let n = pedacos.iter().flatten().map(Vec::len).max().unwrap_or(1).max(1);
    let nome = t.cabecalho[col].clone();
    let mut cabecalho = t.cabecalho;
    cabecalho.splice(col..=col, (1..=n).map(|i| format!("{nome}.{i}")));
    let linhas = t.linhas.into_iter().zip(pedacos).map(|(mut l, p)| {
        if l.len() <= col { l.resize(col + 1, Value::Empty); }
        let p = p.unwrap_or_else(|| vec![l[col].clone()]);
        let faltam = n - p.len();
        l.splice(col..=col, p.into_iter().chain(std::iter::repeat_n(Value::Empty, faltam)));
        l
    }).collect();
    Tabela { cabecalho, linhas }
}

/// Troca cada `[Coluna]` da fórmula pela célula da primeira linha (`B1`): na
/// planilha de rascunho, a linha 1 é a primeira linha de dados. Texto entre
/// aspas fica como está. Erro se a coluna não existe.
pub fn traduzir_formula(formula: &str, cabecalho: &[String]) -> Result<String, String> {
    let f = formula.trim();
    let f = f.strip_prefix('=').unwrap_or(f);
    let mut saida = String::from("=");
    let mut aspas = false;
    let mut resto = f.char_indices().peekable();
    while let Some((i, ch)) = resto.next() {
        if ch == '"' { aspas = !aspas; saida.push(ch); continue; }
        if ch == '[' && !aspas {
            let fim = f[i + 1..].find(']').ok_or_else(|| crate::trf!("Falta o ] depois de \"{}\"", &f[i..]))?;
            let nome = &f[i + 1..i + 1 + fim];
            let alvo = nome.trim().to_lowercase();
            let c = cabecalho.iter().position(|h| h.to_lowercase() == alvo)
                .ok_or_else(|| crate::trf!("A coluna [{nome}] não existe neste ponto da consulta"))?;
            saida.push_str(&format!("{}1", col_name(c as u32)));
            while resto.peek().is_some_and(|(j, _)| *j <= i + 1 + fim) { resto.next(); }
            continue;
        }
        saida.push(ch);
    }
    Ok(saida)
}

/// Coluna nova com a fórmula calculada linha a linha pelo motor da planilha
/// (`=[Preço]*[Qtd]`): os dados vão para uma aba de rascunho e a fórmula é
/// preenchida para baixo, como na grade.
pub(super) fn personalizada(t: Tabela, nome: &str, formula: &str) -> Tabela {
    let n = t.linhas.len();
    let col_nova = t.cabecalho.len() as u32;
    let valores: Vec<Value> = match traduzir_formula(formula, &t.cabecalho) {
        Err(motivo) => vec![Value::Error(CellError::com_motivo(ErrorCode::Name, motivo)); n],
        Ok(_) if n == 0 => Vec::new(),
        Ok(raw) => {
            let mut s = crate::planilha::sheet::Sheet::new("Consulta");
            for (r, l) in t.linhas.iter().enumerate() {
                for (c, v) in l.iter().enumerate().take(col_nova as usize) {
                    if !matches!(v, Value::Empty) { s.set_cell_valor(r as u32, c as u32, v.clone()); }
                }
            }
            let ultima = (n - 1) as u32;
            if !s.preencher_formula_faixa_vazia(col_nova, 0, ultima, &raw) {
                for r in 0..n as u32 {
                    s.set_cell(r, col_nova, &crate::operacoes::fill::shift_formula(&raw, r as i32, 0));
                }
            }
            (0..n as u32).map(|r| s.get_value(r, col_nova)).collect()
        }
    };
    let mut cabecalho = t.cabecalho;
    cabecalho.push(nome.to_owned());
    let linhas = t.linhas.into_iter().zip(valores).map(|(mut l, v)| {
        l.resize(col_nova as usize, Value::Empty);
        l.push(v);
        l
    }).collect();
    Tabela { cabecalho, linhas }
}

/// Parte do texto da coluna (`como`: antes, depois, entre, primeiros, ultimos, tamanho).
/// Delimitador que não aparece dá vazio, como o TEXTOANTES/TEXTODEPOIS dão #N/D.
/// Com `nome`, o resultado vai numa coluna nova no fim; sem, troca a coluna.
pub(super) fn extrair_texto(t: Tabela, col: usize, como: &str, texto: Option<&str>, texto2: Option<&str>,
    quantos: Option<usize>, nome: Option<&str>) -> Tabela
{
    let (a, b, n) = (texto.unwrap_or(""), texto2.unwrap_or(""), quantos.unwrap_or(0));
    let parte = |v: &Value| -> Value {
        if vazio(v) { return Value::Empty; }
        let s = string_js(v);
        let pedaco = match como {
            "antes" if !a.is_empty() => s.split_once(a).map(|(x, _)| x.to_owned()),
            "depois" if !a.is_empty() => s.split_once(a).map(|(_, y)| y.to_owned()),
            "entre" if !a.is_empty() && !b.is_empty() => s.split_once(a).and_then(|(_, y)| y.split_once(b)).map(|(x, _)| x.to_owned()),
            "primeiros" => Some(s.chars().take(n).collect()),
            "ultimos" => { let total = s.chars().count(); Some(s.chars().skip(total.saturating_sub(n)).collect()) }
            "tamanho" => return Value::Number(s.chars().count() as f64),
            _ => None,
        };
        pedaco.map_or(Value::Empty, Value::Text)
    };
    let mut t = t;
    match nome {
        Some(nome) => {
            let largura = t.cabecalho.len();
            t.cabecalho.push(nome.to_owned());
            for l in &mut t.linhas {
                let v = parte(celula_ref(l, col));
                l.resize(largura, Value::Empty);
                l.push(v);
            }
        }
        None => for l in &mut t.linhas {
            let v = parte(celula_ref(l, col));
            gravar_em(l, col, v);
        },
    }
    t
}

/// Texto para comparar na mesclagem aproximada: minúsculas, sem acento nem
/// pontuação, espaços únicos ("Cia. São Paulo" → "cia sao paulo").
pub(super) fn normalizar(s: &str) -> String {
    let mut saida = String::with_capacity(s.len());
    for ch in s.to_lowercase().chars() {
        let base = match ch {
            'á' | 'à' | 'ã' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'õ' | 'ô' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c if c.is_alphanumeric() => c,
            _ => ' ',
        };
        if base == ' ' && (saida.is_empty() || saida.ends_with(' ')) { continue; }
        saida.push(base);
    }
    saida.trim_end().to_owned()
}

/// Semelhança de 0 a 1 entre dois textos já normalizados (1 − distância de
/// edição / tamanho do maior, Levenshtein), ou `None` sem calcular quando a diferença de tamanho já a põe
/// abaixo de `minimo` (a distância de edição é pelo menos essa diferença): a
/// mesclagem aproximada compara milhões de pares na thread da janela.
pub(super) fn semelhanca_minima(a: &str, b: &str, minimo: f64) -> Option<f64> {
    if a == b { return Some(1.0); }
    let (la, lb) = (a.chars().count(), b.chars().count());
    let maior = la.max(lb);
    if maior == 0 { return Some(1.0); }
    if 1.0 - (la.abs_diff(lb) as f64) / (maior as f64) < minimo { return None; }
    let b: Vec<char> = b.chars().collect();
    let mut anterior: Vec<usize> = (0..=lb).collect();
    let mut atual = vec![0; lb + 1];
    for (i, ca) in a.chars().enumerate() {
        atual[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            atual[j + 1] = (anterior[j] + usize::from(ca != *cb)).min(anterior[j + 1] + 1).min(atual[j] + 1);
        }
        std::mem::swap(&mut anterior, &mut atual);
    }
    let s = 1.0 - anterior[lb] as f64 / maior as f64;
    (s >= minimo).then_some(s)
}

/// Comparações que a mesclagem aproximada aceita fazer (linhas × chaves distintas).
pub(super) const MAX_COMPARACOES_APROXIMADAS: u64 = 20_000_000;

#[cfg(test)]
mod testes {
    use super::*;

    fn t(cab: &[&str], linhas: &[&[Value]]) -> Tabela {
        Tabela { cabecalho: cab.iter().map(|s| s.to_string()).collect(), linhas: linhas.iter().map(|l| l.to_vec()).collect() }
    }
    fn tx(s: &str) -> Value { Value::Text(s.into()) }

    fn semelhanca(a: &str, b: &str) -> f64 { semelhanca_minima(a, b, 0.0).unwrap() }

    #[test]
    fn extrair_texto_antes_depois_entre_e_tamanho() {
        let base = t(&["Email"], &[&[tx("ana@x.com.br")], &[tx("sem arroba")], &[Value::Empty]]);
        let antes = extrair_texto(base.clone(), 0, "antes", Some("@"), None, None, None);
        assert_eq!(antes.linhas.iter().map(|l| l[0].clone()).collect::<Vec<_>>(), [tx("ana"), Value::Empty, Value::Empty], "no lugar; sem @ fica vazio");
        let depois = extrair_texto(base.clone(), 0, "depois", Some("@"), None, None, Some("Domínio"));
        assert_eq!((depois.cabecalho[1].as_str(), &depois.linhas[0][1], &depois.linhas[0][0]), ("Domínio", &tx("x.com.br"), &tx("ana@x.com.br")));
        assert_eq!(extrair_texto(base.clone(), 0, "entre", Some("@"), Some("."), None, None).linhas[0][0], tx("x"));
        assert_eq!(extrair_texto(base.clone(), 0, "primeiros", None, None, Some(2), None).linhas[0][0], tx("an"));
        assert_eq!(extrair_texto(base.clone(), 0, "ultimos", None, None, Some(2), None).linhas[0][0], tx("br"));
        assert_eq!(extrair_texto(base, 0, "tamanho", None, None, None, None).linhas[0][0], Value::Number(12.0));
    }

    #[test]
    fn filtros_renomear_reordenar_e_duplicar() {
        let base = t(&["UF", "Valor"], &[&[tx("SP"), Value::Number(50.)], &[tx("RJ"), Value::Number(150.)], &[tx("MG"), Value::Number(10.)]]);
        let rodar = |p: Passo| executar_query(base.clone(), &[p], &HashMap::new());
        let ou = rodar(Passo::FiltrarVarias { condicoes: vec![(0, tx("SP")), (1, tx(">100"))], ou: true });
        assert_eq!(ou.linhas.len(), 2, "SP ou >100");
        let e = rodar(Passo::FiltrarVarias { condicoes: vec![(0, tx("<>SP")), (1, tx(">20"))], ou: false });
        assert_eq!(e.linhas, vec![vec![tx("RJ"), Value::Number(150.)]]);
        let lista = rodar(Passo::FiltrarValores { col: 0, valores: vec![tx("MG"), tx("SP")] });
        assert_eq!(lista.linhas.len(), 2);
        assert_eq!(rodar(Passo::FiltrarValores { col: 1, valores: vec![tx("150")] }).linhas.len(), 1, "número pelo texto");
        assert_eq!(rodar(Passo::RenomearColunas { nomes: vec![(0, "Estado".into()), (1, "Total".into())] }).cabecalho, ["Estado", "Total"]);
        let r = rodar(Passo::Reordenar { ordem: vec![1] });
        assert_eq!((r.cabecalho.as_slice(), &r.linhas[0][0]), (&["Valor".to_string(), "UF".into()][..], &Value::Number(50.)));
        let d = rodar(Passo::DuplicarColuna { col: 0, nome: None });
        assert_eq!((d.cabecalho[2].as_str(), &d.linhas[1][2]), ("UF - Cópia", &tx("RJ")));
    }

    #[test]
    fn formatar_texto_e_preencher() {
        let base = t(&["A"], &[&[tx("  são PAULO ")], &[Value::Empty], &[tx("rio\u{7}")], &[Value::Number(3.0)]]);
        let f = formatar(base.clone(), 0, "aparar", None);
        assert_eq!(f.linhas[0][0], tx("são PAULO"));
        assert_eq!(formatar(f.clone(), 0, "titulo", None).linhas[0][0], tx("São Paulo"));
        assert_eq!(formatar(base.clone(), 0, "limpar", None).linhas[2][0], tx("rio"));
        assert_eq!(formatar(base.clone(), 0, "prefixo", Some("UF-")).linhas[3][0], tx("UF-3"));
        assert_eq!(formatar(base.clone(), 0, "prefixo", Some("UF-")).linhas[1][0], Value::Empty, "vazio fica vazio");
        let p = preencher(base.clone(), 0, false);
        assert_eq!(p.linhas[1][0], tx("  são PAULO "));
        assert_eq!(preencher(base, 0, true).linhas[1][0], tx("rio\u{7}"));
    }

    #[test]
    fn remover_linhas_transpor_e_dividir() {
        let base = t(&["A", "B"], &[&[tx("x"), tx("1")], &[Value::Empty, tx("")], &[tx("y"), tx("2")], &[tx("Total"), tx("3")]]);
        assert_eq!(remover_linhas(base.clone(), "ultimas", 1.0).linhas.len(), 3);
        assert_eq!(remover_linhas(base.clone(), "primeiras", 1.0).linhas[0][0], Value::Empty);
        assert_eq!(remover_linhas(base.clone(), "manter_ultimas", 2.0).linhas[0][0], tx("y"));
        assert_eq!(remover_linhas(base.clone(), "vazias", 0.0).linhas.len(), 3);
        assert_eq!(remover_linhas(base.clone(), "ultimas", 99.0).linhas.len(), 0);
        let tr = transpor(base);
        assert_eq!((tr.cabecalho.len(), tr.linhas.len()), (4, 2));
        assert_eq!(tr.linhas[1][2], tx("2"));
        let d = dividir_posicoes(t(&["Cod", "X"], &[&[tx("SP2026001"), tx("a")], &[Value::Empty, tx("b")]]), 0, &[0, 2, 6], None);
        assert_eq!(d.cabecalho, ["Cod.1", "Cod.2", "Cod.3", "X"]);
        assert_eq!(d.linhas[0][..3], [tx("SP"), tx("2026"), tx("001")]);
        assert_eq!(d.linhas[1], [Value::Empty, Value::Empty, Value::Empty, tx("b")]);
        let c = dividir_posicoes(t(&["Cod"], &[&[tx("abcde")]]), 0, &[], Some(2));
        assert_eq!(c.linhas[0], [tx("ab"), tx("cd"), tx("e")]);
    }

    #[test]
    fn coluna_personalizada_usa_as_formulas_da_planilha() {
        let base = t(&["Preço", "Qtd"], &[&[Value::Number(10.0), Value::Number(3.0)], &[Value::Number(2.5), Value::Number(4.0)]]);
        let r = personalizada(base.clone(), "Total", "=[Preço]*[qtd]");
        assert_eq!(r.cabecalho, ["Preço", "Qtd", "Total"]);
        assert_eq!(r.linhas[0][2], Value::Number(30.0));
        assert_eq!(r.linhas[1][2], Value::Number(10.0));
        let s = personalizada(base.clone(), "Faixa", "SE([Preço]>5;\"[alto]\";\"baixo\")");
        assert_eq!(s.linhas[0][2], tx("[alto]"), "colchete dentro de aspas é texto");
        assert_eq!(s.linhas[1][2], tx("baixo"));
        assert!(matches!(personalizada(base.clone(), "X", "=[Nada]+1").linhas[0][2], Value::Error(_)));
        let maiusculo = t(&["PREÇO"], &[&[Value::Number(2.0)]]);
        assert_eq!(personalizada(maiusculo, "D", "=[preço]*2").linhas[0][1], Value::Number(4.0), "caixa de letra acentuada");
    }

    #[test]
    fn mesclagem_aproximada_acha_o_nome_parecido() {
        let esq = t(&["Cliente"], &[&[tx("CIA BRASILEIRA ALIMENTOS")], &[tx("Padaria Sol")], &[tx("São Paulo Ltda")]]);
        let dir = t(&["Nome", "UF"], &[&[tx("Cia. Brasileira de Alimentos"), tx("SP")], &[tx("sao paulo LTDA."), tx("RJ")]]);
        let mut fontes = HashMap::new();
        fontes.insert("Clientes".to_string(), dir);
        let passo = |s: Option<f64>| Passo::Juntar { fonte: "Clientes".into(), esquerda: IndiceColuna::Single(0),
            direita: IndiceColuna::Single(0), modo: Some("interna".into()), semelhanca: s };
        let r = executar_query(esq.clone(), &[passo(Some(0.8))], &fontes);
        assert_eq!(r.linhas.len(), 2);
        assert_eq!(r.linhas[0][2], tx("SP"));
        assert_eq!(r.linhas[1][2], tx("RJ"), "acento, caixa e pontuação não contam");
        assert_eq!(executar_query(esq, &[passo(None)], &fontes).linhas.len(), 0, "sem semelhança: chave igual");
    }

    #[test]
    fn semelhanca_ignora_acento_pontuacao_e_caixa() {
        assert_eq!(normalizar("Cia. Brasileira  de Alimentos!"), "cia brasileira de alimentos");
        assert_eq!(normalizar("São Paulo"), "sao paulo");
        assert!(semelhanca(&normalizar("CIA BRASILEIRA ALIMENTOS"), &normalizar("Cia. Brasileira de Alimentos")) > 0.8);
        assert!(semelhanca("abc", "xyz") < 0.1);
        assert_eq!(semelhanca_minima("ana", "ana maria souza", 0.8), None, "tamanho muito diferente nem calcula");
        assert_eq!(semelhanca_minima("padaria sol", "padaria sul", 0.8), Some(semelhanca("padaria sol", "padaria sul")));
    }
}
