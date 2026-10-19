//! Dados agregados pelo motor, nunca a planilha inteira: perfil de coluna,
//! amostra, busca e consulta (os passos do OpenQuery).
use std::collections::HashMap;

use feathersheet_core::celulas::refs::{cell_name, col_name, Rect};
use feathersheet_core::consulta::query::{executar_query_limitado, Passo, Tabela};
use feathersheet_core::consulta::sheet_query::{escrever_tabela, ler_intervalo};
use feathersheet_core::operacoes::ops::{find_all_com, SearchOptions};
use feathersheet_core::operacoes::perfil::perfil_coluna;
use serde_json::{json, Value as Json};

use crate::ferramentas::Saida;
use crate::hospede::Hospede;
use crate::texto::*;

/// Teto de células que uma consulta lê ou produz (o mesmo do OpenQuery).
const MAX_CONSULTA: u64 = 20_000_000;
const MAX_AMOSTRA: u64 = 50;

pub fn estatistica(h: &dyn Hospede, args: &Json) -> Saida {
    let (aba, area) = intervalo(h, args, "coluna")?;
    let pasta = h.pasta();
    let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
    let usado = sheet.used_rect();
    let mut s = String::new();
    // Uma coluna por vez; um intervalo largo mostra cada coluna (até 10).
    for c in area.c0..=area.c1.min(area.c0 + 9) {
        let p = perfil_coluna(Rect::new(area.r0, c, area.r1, c), usado.r1, |r, c| pasta.valor(aba, r, c));
        s.push_str(&format!("Coluna {} ({} linhas olhadas): {} números, {} textos, {} lógicos, {} erros, {} vazias; {} distintos, {} repetidos\n",
            col_name(c), p.linhas, p.numeros, p.textos, p.logicos, p.erros, p.vazias, p.distintos, p.repetidos));
        if p.numeros > 0 {
            s.push_str(&format!("  mín {} · máx {} · média {}", p.min, p.max, p.media));
            if let Some(q) = &p.quartis {
                s.push_str(&format!(" · Q1 {} · mediana {} · Q3 {}", q.q1, q.mediana, q.q3));
            }
            s.push('\n');
            if p.outliers > 0 {
                let ex: Vec<String> = p.exemplos_outliers.iter().take(5).map(|(r, v)| format!("{}={v}", cell_name(*r, c))).collect();
                s.push_str(&format!("  {} outlier(s): {}\n", p.outliers, ex.join(", ")));
            }
            let faixas: Vec<String> = p.histograma.iter().map(|f| format!("[{:.4}–{:.4}]:{}", f.de, f.ate, f.qtd)).collect();
            if !faixas.is_empty() { s.push_str(&format!("  histograma: {}\n", faixas.join(" "))); }
        }
        if !p.frequentes.is_empty() {
            let f: Vec<String> = p.frequentes.iter().take(8).map(|(v, n)| format!("{}×{n}", curto(v, 30))).collect();
            s.push_str(&format!("  mais frequentes: {}\n", f.join(", ")));
        }
    }
    Ok(conteudo(s))
}

/// As primeiras linhas (ou espalhadas pela planilha) da área usada.
pub fn amostra(h: &dyn Hospede, args: &Json) -> Saida {
    let pasta = h.pasta();
    let (aba, area) = match arg_texto(args, "intervalo") {
        Some(_) => intervalo(h, args, "intervalo")?,
        None => {
            let aba = aba_do_arg(h, args)?;
            let u = pasta.aba(aba).ok_or("Aba inexistente")?.sheet.used_rect();
            (aba, Rect::new(0, 0, u.r1, u.c1))
        }
    };
    let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
    let usado = sheet.used_rect();
    let r1 = area.r1.min(usado.r1);
    let c1 = area.c1.min(usado.c1).min(area.c0 + 39);
    let n = arg_num(args, "linhas", 20).min(MAX_AMOSTRA) as u32;
    let total = r1.saturating_sub(area.r0) + 1;
    // Espalhada: a primeira linha (cabeçalho) e as demais em passos iguais.
    let linhas: Vec<u32> = if arg_bool(args, "espalhada", false) && total > n {
        let passo = total as f64 / n as f64;
        (0..n).map(|i| area.r0 + (i as f64 * passo) as u32).collect()
    } else {
        (area.r0..=r1).take(n as usize).collect()
    };
    let mut s = format!("{}: {} linha(s) na área, mostrando {}\n\t{}", sheet.name, total, linhas.len(),
        (area.c0..=c1).map(col_name).collect::<Vec<_>>().join("\t"));
    for r in linhas {
        s.push_str(&format!("\n{}", r + 1));
        for c in area.c0..=c1 {
            s.push('\t');
            s.push_str(&curto(&valor_texto(&pasta.valor(aba, r, c)), 60).replace(['\t', '\n'], " "));
        }
    }
    Ok(conteudo(s))
}

pub fn localizar(h: &dyn Hospede, args: &Json) -> Saida {
    let busca = arg_texto(args, "texto").ok_or("Falta `texto` (o que procurar).")?;
    let opcoes = SearchOptions {
        match_case: arg_bool(args, "maiusculas", false),
        whole_cell: arg_bool(args, "inteira", false),
        in_formulas: arg_bool(args, "formulas", false),
    };
    let limite = arg_num(args, "limite", 50).min(500) as usize;
    let pasta = h.pasta();
    let abas: Vec<usize> = if arg_bool(args, "todas_as_abas", false) { (0..pasta.len()).collect() } else { vec![aba_do_arg(h, args)?] };
    let mut achados: Vec<String> = Vec::new();
    let mut total = 0;
    for aba in abas {
        let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
        let lista = find_all_com(sheet, busca, opcoes, |r, c| pasta.valor(aba, r, c));
        total += lista.len();
        for m in lista.iter().take(limite.saturating_sub(achados.len())) {
            achados.push(format!("{}!{}\t{}", sheet.name, cell_name(m.row, m.col),
                curto(&valor_texto(&pasta.valor(aba, m.row, m.col)), 60)));
        }
    }
    Ok(conteudo(if total == 0 { format!("Nada encontrado para \"{busca}\".") } else {
        format!("{total} célula(s) com \"{busca}\"{}:\n{}", if total > achados.len() { format!(" (mostrando {})", achados.len()) } else { String::new() }, achados.join("\n"))
    }))
}

/// Passos do OpenQuery sobre um intervalo: devolve a prévia e, com
/// `destino`, grava o resultado numa aba (nova ou existente, a partir de A1).
pub fn consulta(h: &mut dyn Hospede, args: &Json) -> Saida {
    let (aba, area) = intervalo(h, args, "fonte")?;
    let cabecalho = arg_bool(args, "cabecalho", true);
    let passos: Vec<Passo> = serde_json::from_value(args.get("passos").cloned().unwrap_or_else(|| json!([])))
        .map_err(|e| format!("Passos inválidos: {e}. Exemplo: [{{\"tipo\":\"filtrar\",\"col\":0,\"criterio\":{{\"Text\":\"SP\"}}}}, {{\"tipo\":\"agrupar\",\"por\":[0],\"agregar\":[{{\"col\":2,\"como\":\"soma\"}}]}}]"))?;
    let fonte = ler_intervalo(h.pasta(), aba, area, cabecalho, MAX_CONSULTA)?;
    let entrada = fonte.linhas.len();
    let resultado = executar_query_limitado(fonte, &passos, &HashMap::new(), MAX_CONSULTA)?;
    let mut s = format!("{} linha(s) de entrada → {} linha(s) × {} coluna(s)\n{}", entrada,
        resultado.linhas.len(), resultado.cabecalho.len(), previa(&resultado, arg_num(args, "previa", 20).min(MAX_AMOSTRA) as usize));
    if let Some(destino) = arg_texto(args, "destino") {
        let pasta = h.pasta_mut();
        let indice = match indice_aba(pasta, &Json::String(destino.into())) {
            Ok(i) => i,
            Err(_) => pasta.criar_aba(destino),
        };
        let mut erro = None;
        pasta.editar(indice, |sheet| {
            sheet.abrir_transacao();
            if let Err(e) = escrever_tabela(sheet, 0, 0, &resultado, true, None, MAX_CONSULTA) { erro = Some(e); }
            sheet.fechar_transacao();
        });
        if let Some(e) = erro { return Err(e); }
        h.marcar_alterado();
        s.push_str(&format!("\nGravado em {destino}!A1."));
    }
    Ok(conteudo(s))
}

fn previa(t: &Tabela, n: usize) -> String {
    let mut s = t.cabecalho.join("\t");
    for linha in t.linhas.iter().take(n) {
        s.push('\n');
        s.push_str(&linha.iter().map(|v| curto(&valor_texto(v), 40)).collect::<Vec<_>>().join("\t"));
    }
    if t.linhas.len() > n { s.push_str(&format!("\n… e mais {} linha(s)", t.linhas.len() - n)); }
    s
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::hospede::Avulso;

    fn pasta() -> Avulso {
        let mut h = Avulso::default();
        let linhas = [("Cidade", "Valor"), ("SP", "10"), ("RJ", "20"), ("SP", "30"), ("BH", "1000")];
        for (r, (a, b)) in linhas.iter().enumerate() {
            h.pasta.escrever_celula(0, r as u32, 0, a.to_string());
            h.pasta.escrever_celula(0, r as u32, 1, b.to_string());
        }
        h
    }

    fn texto(r: Saida) -> String { r.unwrap()[0]["text"].as_str().unwrap().to_string() }

    #[test]
    fn estatistica_amostra_e_localizar() {
        let h = pasta();
        let e = texto(estatistica(&h, &json!({ "coluna": "B:B" })));
        assert!(e.contains("4 números") && e.contains("máx 1000"), "{e}");
        // #13: só a letra também é a coluna inteira.
        assert_eq!(texto(estatistica(&h, &json!({ "coluna": "B" }))), e);
        let a = texto(amostra(&h, &json!({ "linhas": 2 })));
        assert!(a.contains("5 linha(s)") && a.contains("Cidade"), "{a}");
        let l = texto(localizar(&h, &json!({ "texto": "SP", "inteira": true })));
        assert!(l.starts_with("2 célula(s)"), "{l}");
    }

    #[test]
    fn consulta_agrupa_e_grava_em_aba_nova() {
        let mut h = pasta();
        let r = texto(consulta(&mut h, &json!({
            "fonte": "A1:B5",
            "passos": [{ "tipo": "agrupar", "por": [0], "agregar": [{ "col": 1, "como": "soma" }] }],
            "destino": "Resumo",
        })));
        assert!(r.contains("→ 3 linha(s)") && r.contains("Gravado"), "{r}");
        let i = indice_aba(&h.pasta, &json!("Resumo")).unwrap();
        assert_eq!(valor_texto(&h.pasta.valor(i, 1, 1)), "40");
    }
}
