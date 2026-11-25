//! Macros ▸ Gravar macro: traduz o que uma transação fez (`Sheet::transacoes_para_gravar`) em linhas da API das
//! macros, como o gravador do Excel traduz para VBA. Valor e fórmula viram `set`/`setFormula`, apagar vira
//! `clearContents`, cada campo de estilo que mudou vira o setter dele, largura/altura viram `setColumnWidth`/
//! `setRowHeight`. Células com a mesma mudança vão numa chamada só (`"A1:C1"`, ou várias áreas `"A1,C3"`).
//! Com referências relativas, os endereços saem a partir da célula ativa de quando a macro roda (`aqui(l, c)`).
// shortcut: classificar, mesclar, tabela, dinâmica, gráfico e objetos viram um comentário "(não gravado: …)";
// formato de número só vai quando tem código (`numFmt`); cor apagada e borda também viram comentário.
// shortcut: transação com mais de [`LIMITE_CELULAS`] células com valores diferentes vira um comentário com a área.
use std::collections::BTreeMap;

use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
use feathersheet_core::celulas::refs::{cell_name, col_name};
use feathersheet_core::planilha::sheet::edicao::{Gravado, Snap};
use serde_json::Value as Json;

const LIMITE_CELULAS: usize = 500;

/// Onde a macro escreve: `sheet` (a aba em que a gravação começou) ou `sheets("Outra")`.
pub struct Alvo<'a> {
    pub objeto: &'a str,
    /// Célula ativa no começo da gravação (linha, coluna), com referências relativas.
    pub relativa_a: Option<(u32, u32)>,
}

/// O cabeçalho da macro gravada (com referências relativas, a função `aqui`).
pub fn cabecalho(nome: &str, relativas: bool) -> String {
    let mut s = format!("// {}\n", feathersheet_core::trf!("Macro gravada: {nome}"));
    if relativas {
        s.push_str("const ativa = sheet.activeCell().address;\n");
        s.push_str("const aqui = (l, c, a = 1, b = 1) => sheet.resize(sheet.offset(ativa, l, c).address, a, b);\n");
    }
    s
}

/// O endereço de um conjunto de células: retângulos por linha (`A1:C1,A3`), ou o retângulo inteiro se cheio.
fn areas(celulas: &[(u32, u32)]) -> Vec<(u32, u32, u32, u32)> {
    let mut v = celulas.to_vec();
    v.sort_unstable();
    v.dedup();
    let (Some(r0), Some(r1)) = (v.iter().map(|x| x.0).min(), v.iter().map(|x| x.0).max()) else { return Vec::new() };
    let (c0, c1) = (v.iter().map(|x| x.1).min().unwrap_or(0), v.iter().map(|x| x.1).max().unwrap_or(0));
    if v.len() as u64 == u64::from(r1 - r0 + 1) * u64::from(c1 - c0 + 1) { return vec![(r0, c0, r1, c1)]; }
    // Trechos seguidos em cada linha; trechos iguais em linhas seguidas viram um retângulo.
    let mut trechos: Vec<(u32, u32, u32, u32)> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let (r, c) = v[i];
        let mut fim = c;
        while i + 1 < v.len() && v[i + 1] == (r, fim + 1) { i += 1; fim += 1; }
        match trechos.iter_mut().find(|t| t.2 + 1 == r && t.1 == c && t.3 == fim) {
            Some(t) => t.2 = r,
            None => trechos.push((r, c, r, fim)),
        }
        i += 1;
    }
    trechos
}

fn endereco((r0, c0, r1, c1): (u32, u32, u32, u32)) -> String {
    if (r0, c0) == (r1, c1) { cell_name(r0, c0) } else { format!("{}:{}", cell_name(r0, c0), cell_name(r1, c1)) }
}

/// `sheet.setBold("A1:C1", true)` ou, com referências relativas, `aqui(0, 0, 1, 3).setBold(true)` por área.
fn chamar(alvo: &Alvo, celulas: &[(u32, u32)], metodo: &str, args: &str, saida: &mut Vec<String>) {
    let lista = areas(celulas);
    let virgula = if args.is_empty() { "" } else { ", " };
    match alvo.relativa_a {
        Some((ra, ca)) => for (r0, c0, r1, c1) in lista {
            let (l, c) = (i64::from(r0) - i64::from(ra), i64::from(c0) - i64::from(ca));
            let tam = if (r0, c0) == (r1, c1) { String::new() } else { format!(", {}, {}", r1 - r0 + 1, c1 - c0 + 1) };
            saida.push(format!("aqui({l}, {c}{tam}).{metodo}({args});"));
        },
        None => {
            let end: Vec<String> = lista.into_iter().map(endereco).collect();
            saida.push(format!("{}.{metodo}({:?}{virgula}{args});", alvo.objeto, end.join(",")));
        }
    }
}

/// O valor como a pessoa digitou: número vira número, o resto vai como texto (a macro analisa como a digitação).
fn literal(raw: &str) -> String {
    match raw.parse::<f64>() {
        Ok(n) if n.is_finite() && feathersheet_core::celulas::number::to_js_string(n) == raw => raw.to_string(),
        _ => Json::String(raw.to_string()).to_string(),
    }
}

/// A chamada que leva o campo de estilo `campo` ao valor `valor` (`None` = o campo saiu).
fn setter(campo: &str, valor: Option<&Campo>) -> Result<(&'static str, String), String> {
    let logico = |v: Option<&Campo>| matches!(v, Some(Campo::Logico(true))).to_string();
    let texto = |v: Option<&Campo>| match v { Some(Campo::Texto(t)) => Some(Json::String(t.clone()).to_string()), _ => None };
    let sem = || feathersheet_core::trf!("(não gravado: formato \"{campo}\")");
    Ok(match campo {
        "bold" => ("setBold", logico(valor)),
        "italic" => ("setItalic", logico(valor)),
        "strike" => ("setStrikethrough", logico(valor)),
        "wrap" => ("setWrapText", logico(valor)),
        "underline" => ("setUnderline", logico(valor)),
        "fontFamily" => ("setFontFamily", texto(valor).ok_or_else(sem)?),
        "fontSize" => match valor { Some(Campo::Numero(n)) => ("setFontSize", feathersheet_core::celulas::number::to_js_string(*n)), _ => return Err(sem()) },
        "color" => ("setFontColor", texto(valor).ok_or_else(sem)?),
        "bg" => ("setBackground", texto(valor).ok_or_else(sem)?),
        "align" => ("setAlignHorizontal", texto(valor).unwrap_or_else(|| "\"general\"".into())),
        "vAlign" => ("setAlignVertical", texto(valor).ok_or_else(sem)?),
        "numFmt" => ("setNumberFormat", texto(valor).ok_or_else(sem)?),
        // O código (`numFmt`) já leva o formato; estes acompanham.
        "format" | "decimals" => return Err(String::new()),
        _ => return Err(sem()),
    })
}

/// As linhas JavaScript de uma transação.
pub fn traduzir(alvo: &Alvo, passos: &[Gravado]) -> Vec<String> {
    let mut saida = Vec::new();
    // Valores: (texto digitado) → células.
    let mut valores: BTreeMap<String, Vec<(u32, u32)>> = BTreeMap::new();
    // Estilo: (campo, valor em JSON) → células; e as células que perderam todo o estilo.
    let mut estilos: BTreeMap<(String, String), (Option<Campo>, Vec<(u32, u32)>)> = BTreeMap::new();
    let mut sem_estilo: Vec<(u32, u32)> = Vec::new();
    let raw = |s: &Option<Snap>| s.as_ref().map_or(String::new(), |s| s.raw.clone());
    let vazio = EstiloParcial::new();
    for p in passos {
        match p {
            Gravado::Celula { r, c, antes, depois } => {
                let (a, d) = (raw(antes), raw(depois));
                if a != d { valores.entry(d).or_default().push((*r, *c)); }
                let ea = antes.as_ref().and_then(|s| s.style.as_ref()).unwrap_or(&vazio);
                let ed = depois.as_ref().and_then(|s| s.style.as_ref()).unwrap_or(&vazio);
                if ea == ed { continue; }
                if ed.is_empty() { sem_estilo.push((*r, *c)); continue; }
                for campo in ea.keys().chain(ed.keys()) {
                    let (va, vd) = (ea.get(campo), ed.get(campo));
                    if va == vd { continue; }
                    let chave = (campo.clone(), format!("{vd:?}"));
                    let e = estilos.entry(chave).or_insert_with(|| (vd.cloned(), Vec::new()));
                    if !e.1.contains(&(*r, *c)) { e.1.push((*r, *c)); }
                }
            }
            Gravado::Largura { coluna, indice, depois: Some(px) } => saida.push(if *coluna {
                format!("{}.setColumnWidth({:?}, {px});", alvo.objeto, col_name(*indice))
            } else {
                format!("{}.setRowHeight({}, {px});", alvo.objeto, indice + 1)
            }),
            Gravado::Largura { .. } => {}
            // Os nomes vêm do motor (`Gravado::Outro`) e estão nos dicionários (conferidos à mão: `tr!(variável)`).
            Gravado::Outro(o) => { let o = feathersheet_core::tr!(*o); saida.push(format!("// {}", feathersheet_core::trf!("(não gravado: {o})"))) }
        }
    }
    let total: usize = valores.values().map(Vec::len).sum();
    if total > LIMITE_CELULAS && valores.len() > 1 {
        let todas: Vec<(u32, u32)> = valores.values().flatten().copied().collect();
        let (r0, c0) = (todas.iter().map(|x| x.0).min().unwrap_or(0), todas.iter().map(|x| x.1).min().unwrap_or(0));
        let (r1, c1) = (todas.iter().map(|x| x.0).max().unwrap_or(0), todas.iter().map(|x| x.1).max().unwrap_or(0));
        saida.push(format!("// {}", feathersheet_core::trf!("(não gravado: {total} células em {})", endereco((r0, c0, r1, c1)))));
    } else {
        for (texto, celulas) in &valores {
            if texto.is_empty() { chamar(alvo, celulas, "clearContents", "", &mut saida); }
            else if texto.starts_with('=') { chamar(alvo, celulas, "setFormula", &Json::String(texto.clone()).to_string(), &mut saida); }
            else { chamar(alvo, celulas, "set", &literal(texto), &mut saida); }
        }
    }
    if !sem_estilo.is_empty() { chamar(alvo, &sem_estilo, "clearFormats", "", &mut saida); }
    for ((campo, _), (valor, celulas)) in &estilos {
        match setter(campo, valor.as_ref()) {
            Ok((metodo, arg)) => chamar(alvo, celulas, metodo, &arg, &mut saida),
            Err(e) if !e.is_empty() => saida.push(format!("// {e}")),
            Err(_) => {}
        }
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    fn snap(raw: &str, estilo: &[(&str, Campo)]) -> Option<Snap> {
        Some(Snap { raw: raw.into(), style: (!estilo.is_empty()).then(|| estilo.iter().map(|(k, v)| ((*k).to_string(), v.clone())).collect()) })
    }

    #[test]
    fn valores_formulas_e_negrito_viram_chamadas_agrupadas() {
        let passos = vec![
            Gravado::Celula { r: 0, c: 0, antes: None, depois: snap("Total", &[("bold", Campo::Logico(true))]) },
            Gravado::Celula { r: 0, c: 1, antes: None, depois: snap("=SOMA(B2:B9)", &[("bold", Campo::Logico(true))]) },
            Gravado::Celula { r: 1, c: 0, antes: snap("x", &[]), depois: None },
            Gravado::Largura { coluna: true, indice: 2, depois: Some(120) },
            Gravado::Outro("classificar"),
        ];
        let l = traduzir(&Alvo { objeto: "sheet", relativa_a: None }, &passos);
        assert_eq!(l, vec![
            "sheet.setColumnWidth(\"C\", 120);",
            "// (não gravado: classificar)",
            "sheet.clearContents(\"A2\");",
            "sheet.setFormula(\"B1\", \"=SOMA(B2:B9)\");",
            "sheet.set(\"A1\", \"Total\");",
            "sheet.setBold(\"A1:B1\", true);",
        ]);
    }

    #[test]
    fn relativas_saem_da_celula_ativa_e_numero_vai_como_numero() {
        let passos = vec![
            Gravado::Celula { r: 4, c: 2, antes: None, depois: snap("10", &[]) },
            Gravado::Celula { r: 4, c: 3, antes: None, depois: snap("10", &[]) },
        ];
        let l = traduzir(&Alvo { objeto: "sheet", relativa_a: Some((3, 2)) }, &passos);
        assert_eq!(l, vec!["aqui(1, 0, 1, 2).set(10);"]);
        assert!(cabecalho("Teste", true).contains("const aqui"));
    }

    /// A prova: o que se fez numa pasta, gravado e rodado numa pasta limpa, deixa as mesmas células.
    #[test]
    fn macro_gravada_refaz_o_que_foi_feito() {
        use feathersheet_core::celulas::refs::Rect;
        use feathersheet_core::planilha::workbook::Workbook;
        let mut feita = Workbook::new();
        let mut linhas = Vec::new();
        let mut gesto = |w: &mut Workbook, f: &dyn Fn(&mut feathersheet_core::planilha::sheet::Sheet)| {
            let s = &mut w.aba_mut(0).unwrap().sheet;
            s.abrir_transacao();
            f(s);
            s.fechar_transacao();
            for t in s.transacoes_para_gravar(1) { linhas.extend(traduzir(&Alvo { objeto: "sheet", relativa_a: None }, &t)); }
        };
        gesto(&mut feita, &|s| { s.set_cell(0, 0, "Região"); s.set_cell(0, 1, "Valor"); s.set_cell(1, 1, "10"); s.set_cell(2, 1, "32"); });
        gesto(&mut feita, &|s| s.set_cell(3, 1, "=SOMA(B2:B3)"));
        gesto(&mut feita, &|s| {
            let mut patch = EstiloParcial::new();
            patch.insert("bold".into(), Campo::Logico(true));
            patch.insert("bg".into(), Campo::Texto("#fef08a".into()));
            s.formatar_faixa(&Rect::new(0, 0, 0, 1), &patch);
        });
        gesto(&mut feita, &|s| s.set_cell(2, 1, ""));
        let codigo = format!("{}{}", cabecalho("Teste", false), linhas.join("\n"));
        let mut limpa = Workbook::new();
        let r = crate::executar(&mut limpa, 0, &codigo);
        assert!(r.erro.is_none(), "{:?}\n{codigo}", r.erro);
        for (l, c) in [(0, 0), (0, 1), (1, 1), (2, 1), (3, 1)] {
            let (a, b) = (&feita.aba(0).unwrap().sheet, &limpa.aba(0).unwrap().sheet);
            assert_eq!(a.snapshot(l, c), b.snapshot(l, c), "célula {l},{c}\n{codigo}");
        }
    }

    #[test]
    fn areas_juntam_retangulos() {
        assert_eq!(areas(&[(0, 0), (0, 1), (2, 0), (3, 0)]), vec![(0, 0, 0, 1), (2, 0, 3, 0)]);
        assert_eq!(areas(&[(1, 1), (1, 2), (2, 1), (2, 2)]), vec![(1, 1, 2, 2)]);
    }
}
