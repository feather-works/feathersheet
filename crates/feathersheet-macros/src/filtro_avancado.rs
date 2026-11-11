//! O Filtro Avançado do Excel (`Range.AdvancedFilter`): a lista (primeira linha =
//! cabeçalhos) e o intervalo de critérios (cabeçalhos iguais aos da lista; cada linha
//! de baixo é uma alternativa — OU entre linhas, E dentro da linha). O critério é o
//! do `CONT.SE` (`">100"`, `"<>x"`, `"a*"`), com a regra do Filtro Avançado para texto
//! sem operador: `"SP"` pega o que começa com SP; `"=SP"`, só SP.
use feathersheet_core::celulas::refs::Rect;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::funcoes::fn_helpers::make_predicate;
use feathersheet_core::planilha::workbook::Workbook;
use std::collections::HashSet;

/// O nome em que o Excel guarda a lista filtrada no lugar (o `ShowAllData` a reexibe).
pub(crate) const NOME_DA_LISTA: &str = "_xlnm._FilterDatabase";

fn texto(v: &Value) -> String {
    feathersheet_core::operacoes::filtro::texto_js(v)
}

/// O critério de uma célula do intervalo de critérios, já como teste.
fn teste(v: &Value) -> Box<dyn Fn(&Value) -> bool> {
    let t = texto(v);
    let tem_operador = t.starts_with(['<', '>', '=']);
    let e_texto = matches!(v, Value::Text(_)) && !tem_operador && feathersheet_core::celulas::number::from_js_string(t.trim()).is_none();
    // Texto sem operador: "começa com", como o Excel (o CONT.SE pediria igual).
    let criterio = if e_texto { Value::Text(format!("{t}*")) } else { v.clone() };
    let p = make_predicate(&criterio);
    Box::new(move |x| p(x))
}

/// As linhas de dados (absolutas) da `lista` que passam nos `criterios`, sem repetidas
/// se `unicos` (a linha inteira igual a uma anterior).
pub(crate) fn linhas_que_passam(pasta: &Workbook, aba: usize, lista: &Rect, criterios: Option<&Rect>, unicos: bool) -> Result<Vec<u32>, String> {
    let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
    let ultima = lista.r1.min(sheet.max_row);
    let valor = |r, c| pasta.valor(aba, r, c);
    // (coluna da lista, teste) de cada linha de critério.
    let mut alternativas: Vec<Vec<(u32, Box<dyn Fn(&Value) -> bool>)>> = Vec::new();
    if let Some(c) = criterios {
        if c.r1 <= c.r0 { return Err(feathersheet_core::tr!("AdvancedFilter: o intervalo de critérios precisa dos cabeçalhos e de uma linha embaixo").into()); }
        let cabecalhos: Vec<String> = (lista.c0..=lista.c1).map(|col| texto(&valor(lista.r0, col)).trim().to_lowercase()).collect();
        for r in c.r0 + 1..=c.r1.min(c.r0 + 1000) {
            let mut linha = Vec::new();
            for col in c.c0..=c.c1 {
                let v = valor(r, col);
                if matches!(v, Value::Empty) || texto(&v).is_empty() { continue; }
                let nome = texto(&valor(c.r0, col)).trim().to_lowercase();
                let pos = cabecalhos.iter().position(|h| *h == nome)
                    .ok_or_else(|| feathersheet_core::trf!("AdvancedFilter: o critério \"{nome}\" não é cabeçalho da lista"))?;
                linha.push((lista.c0 + pos as u32, teste(&v)));
            }
            // Linha de critério vazia: tudo passa, como no Excel.
            alternativas.push(linha);
        }
    }
    let mut vistas: HashSet<Vec<String>> = HashSet::new();
    let mut saida = Vec::new();
    for r in lista.r0 + 1..=ultima {
        let passa = alternativas.is_empty() || alternativas.iter().any(|l| l.iter().all(|(col, t)| t(&valor(r, *col))));
        if !passa { continue; }
        if unicos && !vistas.insert((lista.c0..=lista.c1).map(|col| texto(&valor(r, col))).collect()) { continue; }
        saida.push(r);
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn criterios_com_ou_entre_linhas_e_comeca_com() {
        let mut wb = Workbook::new();
        let dados = [["Nome", "UF", "Valor"], ["Bia", "SP", "50"], ["Ana", "RJ", "150"], ["Caio", "SPX", "250"], ["Bia", "SP", "50"], ["Eva", "MG", "120"]];
        for (r, l) in dados.iter().enumerate() { for (c, v) in l.iter().enumerate() { wb.escrever_celula(0, r as u32 + 4, c as u32, (*v).into()); } }
        // Critérios: (UF começa com SP) OU (Valor > 140).
        for (r, l) in [["UF", "Valor"], ["SP", ""], ["", ">140"]].iter().enumerate() { for (c, v) in l.iter().enumerate() { wb.escrever_celula(0, r as u32, c as u32, (*v).into()); } }
        let lista = Rect::new(4, 0, 9, 2);
        let crit = Rect::new(0, 0, 2, 1);
        assert_eq!(linhas_que_passam(&wb, 0, &lista, Some(&crit), false).unwrap(), vec![5, 6, 7, 8]);
        assert_eq!(linhas_que_passam(&wb, 0, &lista, Some(&crit), true).unwrap(), vec![5, 6, 7], "a repetida sai");
        // Texto "=SP" na célula (a marca de texto do `.xlsx`; digitado, seria fórmula).
        wb.escrever_celula(0, 1, 0, format!("{}=SP", feathersheet_core::planilha::data_table::MARCA_TEXTO));
        assert_eq!(linhas_que_passam(&wb, 0, &lista, Some(&Rect::new(0, 0, 1, 1)), false).unwrap(), vec![5, 8], "=SP é exato");
    }
}
