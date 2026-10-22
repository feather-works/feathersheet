//! Linha de totais da tabela com células REAIS (como no Excel): cada célula
//! da linha logo abaixo da tabela guarda uma fórmula (`=SOMA(Tabela1[Valor])`)
//! e a escolha por coluna fica em `tb.totais` (persistida no `.fshz`).
//!
//! Tudo roda numa transação do `Sheet`: ligar/desligar a linha e escolher a
//! conta de uma coluna são UMA ação de desfazer (metadados + células juntos).
//! As contas são `=SUBTOTAL(109;Tabela1[Valor])` (e 101–107, 110), como o
//! Excel: a referência estruturada cresce com a tabela e as linhas
//! escondidas por filtro ficam de fora.
use super::{area_de, estilo, intersecta, listar, sob_id, Estilo};
use feathersheet_core::{
    celulas::refs::{col_name, Rect, MAX_ROWS},
    planilha::workbook::Workbook,
};

/// (chave gravada em `tb.totais`, rótulo do menu, código do `SUBTOTAL`: como
/// no Excel, 1xx ignora as linhas ocultas/filtradas). Arquivos antigos com
/// `=SOMA(...)` continuam abrindo: a escolha vem de `tb.totais`, não do texto.
pub(in crate::app) const CONTAS: &[(&str, &str, &str)] = &[
    ("nenhum", "Nenhum", ""),
    ("media", "Média", "101"),
    ("contagem", "Contagem", "103"),
    ("contagem-numeros", "Contar números", "102"),
    ("maximo", "Máx", "104"),
    ("minimo", "Mín", "105"),
    ("soma", "Soma", "109"),
    ("desvio", "Desvio padrão", "107"),
    ("variancia", "Variância", "110"),
];

/// A conta escolhida para a coluna relativa `rel`, ou `"nenhum"`.
pub(in crate::app) fn conta_da_coluna(tabela: &serde_json::Value, rel: u32) -> String {
    tabela["totais"].as_array()
        .and_then(|l| l.iter().find(|t| t["col"].as_u64() == Some(u64::from(rel))))
        .and_then(|t| t["fn"].as_str()).unwrap_or("nenhum").to_owned()
}

/// A tabela cuja linha de totais contém a célula, e a coluna relativa.
pub(in crate::app) fn tabela_da_linha_de_totais(
    w: &Workbook, aba: usize, r: u32, c: u32,
) -> Option<(String, u32)> {
    listar(&w.aba(aba)?.sheet).ok()?.iter().find_map(|t| {
        let a = area_de(t)?;
        (t["soFiltro"] != true && t["mostrarTotais"] == true
            && r == a.r1 + 1 && c >= a.c0 && c <= a.c1)
            .then(|| Some((t["id"].as_str()?.to_owned(), c - a.c0)))?
    })
}

/// Texto da referência à coluna: estruturada (`Tabela1[Valor]`) quando o
/// cabeçalho permite; senão o intervalo A1 dos dados.
fn referencia(w: &Workbook, aba: usize, t: &serde_json::Value, area: &Rect, c: u32) -> String {
    let dados0 = area.r0 + u32::from(t["temCabecalho"] == true);
    let a1 = format!("{}{}:{}{}", col_name(c), dados0 + 1, col_name(c), area.r1 + 1);
    let nome = t["nome"].as_str().unwrap_or("");
    if t["temCabecalho"] != true || nome.is_empty() { return a1; }
    let cab = w.aba(aba).map(|a| a.sheet.get_raw(area.r0, c)).unwrap_or_default();
    let ok = !cab.trim().is_empty() && !cab.contains(['[', ']', '#', '\'', '"', '\n']);
    if ok { format!("{nome}[{}]", cab.trim()) } else { a1 }
}

/// A fórmula (ou o texto) que a célula de total da coluna deve guardar.
fn conteudo_da_celula(
    w: &Workbook, aba: usize, t: &serde_json::Value, area: &Rect, rel: u32, conta: &str,
) -> String {
    match CONTAS.iter().find(|(k, _, _)| *k == conta) {
        Some((_, _, f)) if !f.is_empty() =>
            format!("=SUBTOTAL({f};{})", referencia(w, aba, t, area, area.c0 + rel)),
        _ if rel == 0 => tr!("Total").to_owned(),
        _ => String::new(),
    }
}

fn gravar_celula(w: &mut Workbook, aba: usize, r: u32, c: u32, texto: String) {
    let atual = w.aba(aba).map(|a| a.sheet.get_raw(r, c)).unwrap_or_default();
    if atual != texto { w.escrever_celula(aba, r, c, texto); }
}

fn em_transacao(
    w: &mut Workbook, aba: usize,
    f: impl FnOnce(&mut Workbook) -> Result<(), String>,
) -> Result<(), String> {
    w.aba_mut(aba).ok_or(tr!("Aba ausente."))?.sheet.abrir_transacao();
    let r = f(w);
    if let Some(a) = w.aba_mut(aba) {
        if r.is_ok() { a.sheet.fechar_transacao(); } else { a.sheet.reverter_transacao(); }
    }
    r
}

/// Escolhe a conta de UMA coluna e grava a fórmula na célula de total.
pub(in crate::app) fn definir_total(
    w: &mut Workbook, aba: usize, id: &str, rel: u32, conta: &str,
) -> Result<(), String> {
    if !CONTAS.iter().any(|(k, _, _)| *k == conta) {
        return Err(tr!("Função de total desconhecida.").into());
    }
    let t = sob_id(w, aba, id)?;
    let area = area_de(&t).ok_or(tr!("Área de tabela inválida."))?;
    if t["mostrarTotais"] != true { return Err(tr!("A linha de totais está desligada.").into()); }
    if rel > area.c1 - area.c0 { return Err(tr!("Coluna fora da tabela.").into()); }
    em_transacao(w, aba, |w| {
        estilo(w, aba, id, Estilo::TotalColuna { col: rel, funcao: conta.to_owned() })?;
        let texto = conteudo_da_celula(w, aba, &t, &area, rel, conta);
        gravar_celula(w, aba, area.r1 + 1, area.c0 + rel, texto);
        Ok(())
    })
}

/// Liga ou desliga a linha de totais; as células nascem/morrem junto.
pub(in crate::app) fn alternar_totais(
    w: &mut Workbook, aba: usize, id: &str, ligar: bool,
) -> Result<(), String> {
    let t = sob_id(w, aba, id)?;
    let area = area_de(&t).ok_or(tr!("Área de tabela inválida."))?;
    if (t["mostrarTotais"] == true) == ligar { return Ok(()); }
    let linha = area.r1 + 1;
    if ligar {
        if linha >= MAX_ROWS { return Err(tr!("Não há linha livre abaixo da tabela.").into()); }
        let sheet = &w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet;
        if (area.c0..=area.c1).any(|c| !sheet.get_raw(linha, c).is_empty()) {
            return Err(tr!("Há dados logo abaixo da tabela; libere a linha antes de ligar os totais.").into());
        }
        let faixa = Rect::new(area.r0, area.c0, linha, area.c1);
        let colide = listar(sheet)?.iter().filter(|o| o["id"] != t["id"]).any(|o| {
            area_de(o).is_some_and(|mut a| {
                if o["mostrarTotais"] == true { a.r1 += 1; }
                intersecta(&faixa, &a)
            })
        });
        if colide { return Err(tr!("A linha de totais cruzaria outra tabela.").into()); }
    }
    em_transacao(w, aba, |w| {
        estilo(w, aba, id, Estilo::Totais(ligar))?;
        if ligar {
            // Como no Excel: sem escolha anterior, a última coluna com números soma.
            let sem_escolha = t["totais"].as_array().is_none_or(|l| l.is_empty());
            let ultima = area.c1 - area.c0;
            if sem_escolha && ultima > 0 {
                let dados0 = area.r0 + u32::from(t["temCabecalho"] == true);
                let tem_numero = {
                    let sheet = &w.aba(aba).ok_or(tr!("Aba ausente."))?.sheet;
                    (dados0..=area.r1).any(|r| sheet.get_raw(r, area.c1).trim()
                        .replace(',', ".").parse::<f64>().is_ok())
                };
                if tem_numero {
                    estilo(w, aba, id, Estilo::TotalColuna { col: ultima, funcao: "soma".into() })?;
                }
            }
            let t = sob_id(w, aba, id)?;
            for rel in 0..=(area.c1 - area.c0) {
                let conta = conta_da_coluna(&t, rel);
                let texto = conteudo_da_celula(w, aba, &t, &area, rel, &conta);
                gravar_celula(w, aba, linha, area.c0 + rel, texto);
            }
        } else {
            for c in area.c0..=area.c1 { gravar_celula(w, aba, linha, c, String::new()); }
        }
        Ok(())
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::values::Value;

    fn base() -> Workbook {
        let mut w = Workbook::new();
        for (r, (a, b)) in [("a", "b"), ("x", "10"), ("y", "20"), ("z", "30")].iter().enumerate() {
            w.escrever_celula(0, r as u32, 0, (*a).into());
            w.escrever_celula(0, r as u32, 1, (*b).into());
        }
        super::super::criar(&mut w, 0, Rect::new(0, 0, 3, 1), "Tabela1", true, false).unwrap();
        w
    }
    fn bruto(w: &Workbook, r: u32, c: u32) -> String { w.aba(0).unwrap().sheet.get_raw(r, c) }
    fn num(w: &Workbook, r: u32, c: u32) -> Option<f64> {
        match w.valor(0, r, c) { Value::Number(n) => Some(n), _ => None }
    }

    #[test]
    fn ligar_escreve_total_e_soma_na_ultima_coluna_com_um_undo() {
        let mut w = base();
        alternar_totais(&mut w, 0, "tab_Tabela1", true).unwrap();
        assert_eq!(bruto(&w, 4, 0), "Total");
        assert_eq!(bruto(&w, 4, 1), "=SUBTOTAL(109;Tabela1[b])");
        assert_eq!(num(&w, 4, 1), Some(60.0));
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        assert_eq!(bruto(&w, 4, 1), "");
        assert_eq!(bruto(&w, 4, 0), "");
        assert_ne!(sob_id(&w, 0, "tab_Tabela1").unwrap()["mostrarTotais"], true);
        assert!(w.aba_mut(0).unwrap().sheet.redo());
        assert_eq!(num(&w, 4, 1), Some(60.0));
    }

    #[test]
    fn escolher_conta_grava_formula_valor_e_desfaz() {
        let mut w = base();
        alternar_totais(&mut w, 0, "tab_Tabela1", true).unwrap();
        definir_total(&mut w, 0, "tab_Tabela1", 1, "media").unwrap();
        assert_eq!(bruto(&w, 4, 1), "=SUBTOTAL(101;Tabela1[b])");
        assert_eq!(num(&w, 4, 1), Some(20.0));
        assert_eq!(conta_da_coluna(&sob_id(&w, 0, "tab_Tabela1").unwrap(), 1), "media");
        for (k, esperado) in [("maximo", 30.0), ("minimo", 10.0), ("contagem", 3.0),
            ("contagem-numeros", 3.0), ("desvio", 10.0), ("variancia", 100.0), ("soma", 60.0)] {
            definir_total(&mut w, 0, "tab_Tabela1", 1, k).unwrap();
            assert_eq!(num(&w, 4, 1), Some(esperado), "{k}");
        }
        definir_total(&mut w, 0, "tab_Tabela1", 1, "nenhum").unwrap();
        assert_eq!(bruto(&w, 4, 1), "");
        assert_eq!(conta_da_coluna(&sob_id(&w, 0, "tab_Tabela1").unwrap(), 1), "nenhum");
        assert!(w.aba_mut(0).unwrap().sheet.undo());
        assert_eq!(bruto(&w, 4, 1), "=SUBTOTAL(109;Tabela1[b])");
        assert_eq!(conta_da_coluna(&sob_id(&w, 0, "tab_Tabela1").unwrap(), 1), "soma");
    }

    #[test]
    fn total_com_filtro_ativo_soma_so_as_linhas_visiveis() {
        let mut w = base();
        alternar_totais(&mut w, 0, "tab_Tabela1", true).unwrap();
        assert_eq!(num(&w, 4, 1), Some(60.0));
        w.editar(0, |s| { s.row_heights.insert(2, 0); }); // filtro esconde "y" (20)
        assert_eq!(num(&w, 4, 1), Some(40.0));
        definir_total(&mut w, 0, "tab_Tabela1", 1, "contagem").unwrap();
        assert_eq!(num(&w, 4, 1), Some(2.0));
    }

    #[test]
    fn total_acompanha_o_filtro_de_verdade_da_tabela() {
        let mut w = base();
        alternar_totais(&mut w, 0, "tab_Tabela1", true).unwrap();
        assert_eq!(num(&w, 4, 1), Some(60.0));
        // Filtro da coluna b > 15: sobra y (20) e z (30).
        super::super::migracao::filtrar(&mut w, 0, "tab_Tabela1", 1, Some(serde_json::json!({"op":"gt","val":15}))).unwrap();
        assert_eq!(num(&w, 4, 1), Some(50.0));
        super::super::migracao::filtrar(&mut w, 0, "tab_Tabela1", 1, None).unwrap();
        assert_eq!(num(&w, 4, 1), Some(60.0));
    }

    #[test]
    fn nao_ligar_sobre_dados_e_desligar_limpa() {
        let mut w = base();
        w.escrever_celula(0, 4, 1, "9".into());
        assert!(alternar_totais(&mut w, 0, "tab_Tabela1", true).is_err());
        assert_eq!(bruto(&w, 4, 1), "9");
        w.escrever_celula(0, 4, 1, String::new());
        alternar_totais(&mut w, 0, "tab_Tabela1", true).unwrap();
        alternar_totais(&mut w, 0, "tab_Tabela1", false).unwrap();
        assert_eq!(bruto(&w, 4, 1), "");
        assert_eq!(bruto(&w, 4, 0), "");
    }

    #[test]
    fn localiza_tabela_pela_celula_de_total() {
        let mut w = base();
        assert!(tabela_da_linha_de_totais(&w, 0, 4, 1).is_none());
        alternar_totais(&mut w, 0, "tab_Tabela1", true).unwrap();
        assert_eq!(tabela_da_linha_de_totais(&w, 0, 4, 1), Some(("tab_Tabela1".into(), 1)));
        assert!(tabela_da_linha_de_totais(&w, 0, 5, 1).is_none());
    }
}
