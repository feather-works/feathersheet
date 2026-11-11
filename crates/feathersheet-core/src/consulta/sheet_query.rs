//! Ponte nativa entre as células e o motor de consultas do FeatherSheet.
//!
//! Tradução de `src/core/sheet-query.ts`. Os intervalos declarados (inclusive
//! colunas inteiras) são limitados à área realmente usada antes de alocar.
//! Os resultados são escritos como Value tipado, nunca como fórmula acidental.
//! O código é independente de Iced/Tauri e reutilizável numa futura WASM.

use crate::celulas::number::to_js_string;
use crate::celulas::refs::{Rect, MAX_COLS, MAX_ROWS};
use crate::celulas::values::Value;
use crate::consulta::query::Tabela;
use crate::planilha::sheet::Sheet;
use crate::planilha::workbook::Workbook;

fn titulo(valor: &Value) -> String {
    match valor {
        Value::Empty => String::new(),
        Value::Text(s) => s.clone(),
        Value::Number(n) => to_js_string(*n),
        Value::Bool(v) => if *v { "VERDADEIRO" } else { "FALSO" }.into(),
        Value::Error(e) => e.to_string(),
    }
}

fn linha_vazia(linha: &[Value]) -> bool {
    linha.iter().all(|v| matches!(v, Value::Empty)
        || matches!(v, Value::Text(t) if t.is_empty()))
}

/// Lê somente células existentes dentro do retângulo. A validação de
/// orçamento considera a INTERSEÇÃO com a área usada, nunca o intervalo
/// declarado inteiro. Isso evita ler 10 milhões de linhas ao selecionar A:A.
/// Usa Workbook::valor para avaliar fórmulas e referências entre abas.
pub fn ler_intervalo(
    workbook: &Workbook,
    indice: usize,
    area: Rect,
    tem_cabecalho: bool,
    max_celulas: u64,
) -> Result<Tabela, String> {
    let aba = workbook.aba(indice).ok_or_else(|| crate::tr!("A aba de origem não existe"))?;
    let vazio = || Tabela { cabecalho: Vec::new(), linhas: Vec::new() };
    if aba.sheet.is_empty() { return Ok(vazio()); }
    let retangulo = area.normalized();
    if retangulo.r0 >= MAX_ROWS || retangulo.c0 >= MAX_COLS {
        return Ok(vazio());
    }
    let usado = aba.sheet.used_rect();
    let r0 = retangulo.r0;
    let c0 = retangulo.c0;
    let r1 = retangulo.r1.min(usado.r1).min(MAX_ROWS - 1);
    let c1 = retangulo.c1.min(usado.c1).min(MAX_COLS - 1);
    if r1 < r0 || c1 < c0 { return Ok(vazio()); }

    let altura = u64::from(r1 - r0) + 1;
    let largura = u64::from(c1 - c0) + 1;
    let total = altura.saturating_mul(largura);
    if total > max_celulas {
        return Err(crate::trf!("Intervalo efetivo de {total} células: FeatherQuery aceita até {max_celulas} por fonte. Reduza a seleção."
        ));
    }
    let mut cabecalho = Vec::with_capacity(largura as usize);
    for col in c0..=c1 {
        let bruto = if tem_cabecalho { workbook.valor(indice, r0, col) }
            else { Value::Empty };
        let nome = titulo(&bruto);
        cabecalho.push(if nome.is_empty() {
            format!("Coluna {}", col - c0 + 1)
        } else { nome });
    }
    let mut linhas = Vec::new();
    let primeira = if tem_cabecalho { r0.saturating_add(1) } else { r0 };
    if primeira <= r1 {
        for r in primeira..=r1 {
            let linha: Vec<Value> = (c0..=c1).map(|c| workbook.valor(indice, r, c)).collect();
            if !linha_vazia(&linha) { linhas.push(linha); }
        }
    }
    Ok(Tabela { cabecalho, linhas })
}

/// Planeja dimensões ANTES de apagar o resultado anterior. Números em u64
/// evitam overflow de linha/coluna; o destino tem que caber na grade real.
pub fn area_saida(
    linha0: u32,
    coluna0: u32,
    tabela: &Tabela,
    com_cabecalho: bool,
    max_celulas: u64,
) -> Result<Rect, String> {
    let largura = tabela.linhas.iter().map(Vec::len)
        .max().unwrap_or(0).max(tabela.cabecalho.len()).max(1) as u64;
    let altura = tabela.linhas.len() as u64 + u64::from(com_cabecalho);
    if altura == 0 {
        return Err(crate::tr!("A consulta não possui registros nem cabeçalho para gravar").into());
    }
    if largura.saturating_mul(altura) > max_celulas
        || u64::from(coluna0) + largura > u64::from(MAX_COLS)
        || u64::from(linha0) + altura > u64::from(MAX_ROWS)
    {
        return Err(crate::tr!("Resultado FeatherQuery excede os limites da planilha ou de memória").into());
    }
    Ok(Rect::new(linha0, coluna0,
        (u64::from(linha0) + altura - 1) as u32,
        (u64::from(coluna0) + largura - 1) as u32))
}

/// Escreve o resultado tipado, limpa o retângulo anterior e devolve o novo.
/// Nunca converte Value::Text("=...") em fórmula. A validação ocorre ANTES
/// de qualquer alteração, inclusive da limpeza do resultado anterior.
pub fn escrever_tabela(
    sheet: &mut Sheet,
    linha0: u32,
    coluna0: u32,
    tabela: &Tabela,
    com_cabecalho: bool,
    area_anterior: Option<Rect>,
    max_celulas: u64,
) -> Result<Rect, String> {
    let nova = area_saida(linha0, coluna0, tabela, com_cabecalho, max_celulas)?;
    if let Some(antiga) = &area_anterior {
        let area = antiga.normalized();
        if area.r1 >= MAX_ROWS || area.c1 >= MAX_COLS
            || area.area() > max_celulas
        {
            return Err(crate::tr!("Área anterior inválida ou acima do orçamento FeatherQuery").into());
        }
    }

    if let Some(antiga) = area_anterior {
        let area = antiga.normalized();
        for r in area.r0..=area.r1 {
            for c in area.c0..=area.c1 {
                sheet.apply(r, c, None, false);
            }
        }
    }
    let mut r = linha0;
    if com_cabecalho {
        for (c, nome) in tabela.cabecalho.iter().enumerate() {
            sheet.set_cell_valor(r, coluna0 + c as u32, Value::Text(nome.clone()));
        }
        r += 1;
    }
    for linha in &tabela.linhas {
        for (c, valor) in linha.iter().enumerate() {
            if !valor.is_empty() {
                sheet.set_cell_valor(r, coluna0 + c as u32, valor.clone());
            }
        }
        r += 1;
    }
    Ok(nova)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn coluna_inteira_recorta_area_usada_e_ignora_linhas_vazias() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 0, "Produto".into());
        wb.escrever_celula(0, 1, 0, "Café".into());
        wb.escrever_celula(0, 3, 0, "Chá".into());
        // Sem o recorte a área passaria de 10 milhões e seria recusada.
        let tabela = ler_intervalo(&wb, 0,
            Rect::new(0, 0, MAX_ROWS - 1, 0), true, 12).unwrap();
        assert_eq!(tabela.cabecalho, vec!["Produto"]);
        assert_eq!(tabela.linhas.len(), 2);
        assert_eq!(tabela.linhas[0], vec![Value::Text("Café".into())]);
        assert_eq!(tabela.linhas[1], vec![Value::Text("Chá".into())]);
    }

    #[test]
    fn leitura_sem_cabecalho_e_limite_real_de_celulas() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0, 0, 1, "10".into());
        wb.escrever_celula(0, 1, 1, "=B1*2".into());
        let tabela = ler_intervalo(&wb, 0, Rect::new(0, 0, MAX_ROWS - 1, 1), false, 4).unwrap();
        assert_eq!(tabela.cabecalho, vec!["Coluna 1", "Coluna 2"]);
        assert_eq!(tabela.linhas.len(), 2);
        assert_eq!(tabela.linhas[1][1], Value::Number(20.0));
        assert!(ler_intervalo(&wb, 0,
            Rect::new(0, 0, MAX_ROWS - 1, 1), false, 3).is_err());
    }

    #[test]
    fn grava_texto_parecido_com_formula_sem_executar() {
        let mut wb = Workbook::new();
        let t = Tabela {
            cabecalho: vec!["Expressão".into()],
            linhas: vec![vec![Value::Text("=1+1".into())]],
        };
        let r = wb.editar(0, |sheet| {
            escrever_tabela(sheet, 0, 0, &t, true, None, 10).unwrap();
        });
        assert!(r.is_some());
        assert_eq!(wb.valor(0, 1, 0), Value::Text("=1+1".into()));
        assert_eq!(wb.valor(0, 0, 0), Value::Text("Expressão".into()));
    }

    #[test]
    fn atualizar_apaga_restos_e_rejeita_limite_sem_editar() {
        let mut wb = Workbook::new();
        let primeiro = Tabela {
            cabecalho: vec!["N".into()],
            linhas: vec![vec![Value::Number(1.0)], vec![Value::Number(2.0)]],
        };
        let area = wb.aba(0).unwrap().sheet.used_rect();
        let _ = area;
        wb.editar(0, |s| {
            escrever_tabela(s, 0, 0, &primeiro, true, None, 10).unwrap();
        });
        let segundo = Tabela {
            cabecalho: vec!["N".into()],
            linhas: vec![vec![Value::Number(3.0)]],
        };
        wb.editar(0, |s| {
            let old = Rect::new(0, 0, 2, 0);
            assert!(escrever_tabela(s, 0, 0, &segundo, true, Some(old.clone()), 1).is_err());
            escrever_tabela(s, 0, 0, &segundo, true, Some(old), 10).unwrap();
        });
        assert_eq!(wb.valor(0, 1, 0), Value::Number(3.0));
        assert_eq!(wb.valor(0, 2, 0), Value::Empty);
    }
}
