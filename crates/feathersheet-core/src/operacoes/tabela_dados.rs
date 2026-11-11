//! "Tabela de dados" do Excel (Teste de hipóteses), com o resultado gravado
//! como valores. A área segue o arranjo do Excel:
//!
//! - **Uma variável em coluna** (só a célula de entrada da coluna): os
//!   valores de entrada descem pela primeira coluna e as fórmulas ficam na
//!   primeira linha, uma por coluna.
//! - **Uma variável em linha** (só a de linha): valores na primeira linha,
//!   fórmulas na primeira coluna.
//! - **Duas variáveis**: a fórmula no canto superior esquerdo, os valores da
//!   linha no topo e os da coluna à esquerda.
//!
//! Cada combinação é escrita na célula de entrada sem histórico, a fórmula é
//! lida, e no fim a entrada volta ao valor original. Quem chama grava o
//! resultado numa transação.
use crate::celulas::{number::to_js_string, refs::Rect, values::Value};
use crate::planilha::{sheet::edicao::Snap, workbook::Workbook};

/// Acima disto a tabela recalcularia a planilha milhões de vezes.
pub const MAX_COMBINACOES: u64 = 100_000;

/// Texto que a célula de resultado recebe (o valor, não a fórmula).
fn como_texto(v: &Value) -> String {
    match v {
        Value::Empty => String::new(),
        Value::Number(n) => to_js_string(*n),
        Value::Text(t) => t.clone(),
        Value::Bool(true) => "VERDADEIRO".into(),
        Value::Bool(false) => "FALSO".into(),
        Value::Error(e) => e.code.as_str().into(),
    }
}

/// Calcula a tabela da `area` na aba `aba`. Devolve as células do miolo e o
/// texto de cada uma, sem escrevê-las.
pub fn calcular(workbook: &mut Workbook, aba: usize, area: &Rect,
    entrada_linha: Option<(u32, u32)>, entrada_coluna: Option<(u32, u32)>)
    -> Result<Vec<(u32, u32, String)>, String> {
    let a = area.normalized();
    if a.r1 <= a.r0 || a.c1 <= a.c0 {
        return Err(crate::tr!("Selecione a tabela inteira: valores de entrada e fórmulas, ao menos 2 × 2.").into());
    }
    let combinacoes = u64::from(a.r1 - a.r0) * u64::from(a.c1 - a.c0);
    if combinacoes > MAX_COMBINACOES {
        return Err(crate::trf!("A tabela teria {combinacoes} resultados; o limite é {MAX_COMBINACOES}."));
    }
    let dentro = |(r, c): (u32, u32)| r >= a.r0 && r <= a.r1 && c >= a.c0 && c <= a.c1;
    if entrada_linha.is_some_and(dentro) || entrada_coluna.is_some_and(dentro) {
        return Err(crate::tr!("A célula de entrada precisa ficar fora da tabela.").into());
    }
    let sheet = &workbook.aba(aba).ok_or_else(|| crate::tr!("A aba não existe."))?.sheet;
    let formula = |r: u32, c: u32| sheet.get_raw(r, c).starts_with('=');
    // Onde ficam as fórmulas em cada arranjo, conferido antes de calcular.
    match (entrada_linha, entrada_coluna) {
        (None, None) => return Err(crate::tr!("Informe ao menos uma célula de entrada.").into()),
        (Some(_), Some(_)) if !formula(a.r0, a.c0) =>
            return Err(crate::tr!("Com duas entradas, a fórmula fica no canto superior esquerdo da tabela.").into()),
        (None, Some(_)) if !(a.c0 + 1..=a.c1).any(|c| formula(a.r0, c)) =>
            return Err(crate::tr!("Coloque as fórmulas na primeira linha da tabela.").into()),
        (Some(_), None) if !(a.r0 + 1..=a.r1).any(|r| formula(r, a.c0)) =>
            return Err(crate::tr!("Coloque as fórmulas na primeira coluna da tabela.").into()),
        _ => {}
    }
    let originais: Vec<((u32, u32), Option<Snap>)> = [entrada_linha, entrada_coluna].into_iter()
        .flatten().map(|p| (p, sheet.snapshot(p.0, p.1))).collect();
    let valor_texto = |w: &Workbook, r: u32, c: u32| como_texto(&w.valor(aba, r, c));
    let escrever = |w: &mut Workbook, (r, c): (u32, u32), raw: String| {
        let estilo = originais.iter().find(|(p, _)| *p == (r, c))
            .and_then(|(_, s)| s.as_ref()).and_then(|s| s.style.clone());
        w.editar(aba, |s| { s.apply(r, c, Some(Snap { raw, style: estilo }), false); });
    };

    let mut saida = Vec::new();
    for r in a.r0 + 1..=a.r1 {
        for c in a.c0 + 1..=a.c1 {
            // Valores de entrada lidos antes de mexer na planilha.
            let da_coluna = valor_texto(workbook, r, a.c0);
            let da_linha = valor_texto(workbook, a.r0, c);
            if let Some(p) = entrada_coluna { escrever(workbook, p, da_coluna); }
            if let Some(p) = entrada_linha { escrever(workbook, p, da_linha); }
            let lido = match (entrada_linha, entrada_coluna) {
                (Some(_), Some(_)) => valor_texto(workbook, a.r0, a.c0),
                (None, Some(_)) => valor_texto(workbook, a.r0, c),
                _ => valor_texto(workbook, r, a.c0),
            };
            saida.push((r, c, lido));
        }
    }
    for ((r, c), original) in originais {
        workbook.editar(aba, |s| { s.apply(r, c, original, false); });
    }
    Ok(saida)
}

#[cfg(test)]
mod testes {
    use super::*;

    fn pasta(celulas: &[((u32, u32), &str)]) -> Workbook {
        let mut w = Workbook::new();
        for ((r, c), t) in celulas { w.escrever_celula(0, *r, *c, (*t).into()); }
        w
    }

    #[test]
    fn uma_variavel_em_coluna_e_entrada_volta_ao_original() {
        // A1 = taxa, B1 = A1*100. Tabela D1:E4: valores 1,2,3 em D2:D4; E1 = B1.
        let mut w = pasta(&[((0, 0), "5"), ((0, 1), "=A1*100"),
            ((1, 3), "1"), ((2, 3), "2"), ((3, 3), "3"), ((0, 4), "=B1")]);
        let r = calcular(&mut w, 0, &Rect::new(0, 3, 3, 4), None, Some((0, 0))).unwrap();
        assert_eq!(r, vec![(1, 4, "100".into()), (2, 4, "200".into()), (3, 4, "300".into())]);
        assert_eq!(w.valor(0, 0, 0), Value::Number(5.));
    }

    #[test]
    fn duas_variaveis_cruzam_linha_e_coluna() {
        // C1 = A1*B1; tabela E1:G3 com E1 = C1, F1:G1 = 10,20 (linha → B1), E2:E3 = 1,2 (coluna → A1).
        let mut w = pasta(&[((0, 0), "0"), ((0, 1), "0"), ((0, 2), "=A1*B1"),
            ((0, 4), "=C1"), ((0, 5), "10"), ((0, 6), "20"), ((1, 4), "1"), ((2, 4), "2")]);
        let r = calcular(&mut w, 0, &Rect::new(0, 4, 2, 6), Some((0, 1)), Some((0, 0))).unwrap();
        let textos: Vec<_> = r.iter().map(|(_, _, t)| t.as_str()).collect();
        assert_eq!(textos, ["10", "20", "20", "40"]);
    }

    #[test]
    fn arranjo_errado_e_entrada_dentro_da_tabela_sao_recusados() {
        let mut w = pasta(&[((0, 0), "1"), ((1, 3), "1")]);
        assert!(calcular(&mut w, 0, &Rect::new(0, 3, 2, 4), None, Some((0, 0))).unwrap_err()
            .contains("primeira linha"));
        assert!(calcular(&mut w, 0, &Rect::new(0, 3, 2, 4), None, Some((1, 3))).unwrap_err()
            .contains("fora da tabela"));
        assert!(calcular(&mut w, 0, &Rect::new(0, 3, 2, 4), None, None).is_err());
    }
}
