//! Move de src/macros/api-valores.ts reutilizando Workbook::mover_intervalo.
//!
//! O Workbook já ajusta fórmulas locais, externas e colunares. O adaptador
//! somente valida o destino e mantém a transação da macro aberta até o final.
//! Abrir uma transação externa em cada aba é essencial: mover_intervalo
//! cria/fecha transações próprias nas abas com fórmulas reescritas; a externa
//! permite reverter TUDO quando o JavaScript lança uma exceção depois.
use std::collections::BTreeSet;
use feathersheet_core::{
    celulas::refs::{Rect, MAX_COLS, MAX_ROWS},
    planilha::workbook::Workbook,
};

const LIMITE_MACRO: u64 = 500_000;

/// Movimenta sem clonar Workbook ou células; transações não afetadas
/// são fechadas sem entrar na lista de abas modificadas.
pub(crate) fn mover(
    wb: &mut Workbook,
    alteradas: &mut BTreeSet<usize>,
    aba: usize,
    origem: &Rect,
    destino: &Rect,
) -> Result<(), String> {
    if origem.sheet.is_some() || destino.sheet.is_some() {
        return Err(feathersheet_core::tr!("Use wb.sheet(nome) para escolher a aba da operação").into());
    }
    let origem = origem.normalized();
    let destino = destino.normalized();
    if origem.area() > LIMITE_MACRO {
        return Err(feathersheet_core::tr!("Mover aceita no máximo 500.000 células por operação").into());
    }
    if origem.r1 >= MAX_ROWS || origem.c1 >= MAX_COLS
        || destino.r0 >= MAX_ROWS || destino.c0 >= MAX_COLS {
        return Err(feathersheet_core::tr!("Endereço fora dos limites da planilha").into());
    }
    let dr = i64::from(destino.r0) - i64::from(origem.r0);
    let dc = i64::from(destino.c0) - i64::from(origem.c0);
    if dr == 0 && dc == 0 {
        return Ok(());
    }
    let destino_r1 = i64::from(origem.r1) + dr;
    let destino_c1 = i64::from(origem.c1) + dc;
    if destino_r1 < 0 || destino_c1 < 0
        || destino_r1 >= i64::from(MAX_ROWS)
        || destino_c1 >= i64::from(MAX_COLS) {
        return Err(feathersheet_core::tr!("Mover ultrapassaria a última linha ou coluna da planilha").into());
    }
    let dr = i32::try_from(dr).map_err(|_|feathersheet_core::tr!("Deslocamento de linha inválido"))?;
    let dc = i32::try_from(dc).map_err(|_|feathersheet_core::tr!("Deslocamento de coluna inválido"))?;
    com_transacoes(wb, alteradas, aba, |wb| wb.mover_intervalo(aba, origem, dr, dc)
        .map(|_| ()).ok_or_else(|| feathersheet_core::tr!("Não foi possível mover intervalo; destino ou operação inválida").into()))
}

/// Roda `f` com uma transação aberta em cada aba ainda não mexida pela macro.
/// As operações do Workbook criam/fecham transações próprias nas abas com
/// fórmulas reescritas; a externa permite reverter TUDO se o JavaScript
/// lançar uma exceção depois. Abas que não mudaram saem sem transação vazia.
pub(crate) fn com_transacoes(
    wb: &mut Workbook,
    alteradas: &mut BTreeSet<usize>,
    aba: usize,
    f: impl FnOnce(&mut Workbook) -> Result<(), String>,
) -> Result<(), String> {
    if aba >= wb.len() {
        return Err("Aba inexistente".into());
    }
    let n = wb.len();
    let antes = (0..n).map(|i| wb.versao(i)).collect::<Vec<_>>();
    let mut abertas = Vec::new();
    for i in 0..n {
        if !alteradas.contains(&i) {
            wb.aba_mut(i).expect("índice de aba existente").sheet.abrir_transacao();
            abertas.push(i);
        }
    }
    if let Err(erro) = f(wb) {
        for i in abertas {
            wb.aba_mut(i).expect("índice existente").sheet.reverter_transacao();
        }
        return Err(erro);
    }
    for i in abertas {
        if i == aba || wb.versao(i) != antes[i] {
            // A transação permanece aberta, fechada pelo host da macro
            // ao finalizar o JavaScript, ou revertida se ocorrer erro.
            alteradas.insert(i);
        } else {
            wb.aba_mut(i).expect("índice existente").sheet.reverter_transacao();
        }
    }
    Ok(())
}

/// `range.deleteCells('up'|'left')` / `range.insertCells('down'|'right')`:
/// o Excluir/Inserir células… da Página Inicial (o `Delete Shift:=xlUp` do VBA).
pub(crate) fn deslocar_celulas(
    wb: &mut Workbook,
    alteradas: &mut BTreeSet<usize>,
    aba: usize,
    area: &Rect,
    inserir: bool,
    sentido: &str,
) -> Result<(), String> {
    use feathersheet_core::planilha::deslocar_celulas::Deslocamento;
    let sentido = match (inserir, sentido) {
        (false, "up") | (true, "down") => Deslocamento::Vertical,
        (false, "left") | (true, "right") => Deslocamento::Horizontal,
        _ => return Err(if inserir { feathersheet_core::tr!("insertCells: use 'down' ou 'right'").into() }
            else { feathersheet_core::tr!("deleteCells: use 'up' ou 'left'").into() }),
    };
    let area = area.normalized();
    com_transacoes(wb, alteradas, aba, |wb| {
        if inserir { wb.inserir_celulas(aba, area, sentido) } else { wb.excluir_celulas(aba, area, sentido) }
            .map(|_| ())
    })
}

/// `sheet.insertRow(linha, n)`, `deleteRow`, `insertColumn`, `deleteColumn`:
/// linha do 1, coluna em letra ou número do 1, como no Excel.
pub(crate) fn linhas_colunas(
    wb: &mut Workbook,
    alteradas: &mut BTreeSet<usize>,
    aba: usize,
    op: &str,
    onde: &serde_json::Value,
    quantas: &serde_json::Value,
) -> Result<(), String> {
    let coluna = op.ends_with("Column");
    let at = if coluna {
        match onde.as_str() {
            Some(t) => crate::enderecos::coluna_texto(t)?,
            None => crate::enderecos::coluna_numero(onde.as_u64().and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| feathersheet_core::tr!("Coluna deve ser letra ou número de 1 a 16384"))?)?,
        }
    } else {
        crate::enderecos::linha_numero(onde.as_u64().and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| feathersheet_core::tr!("Linha deve ser número inteiro a partir de 1"))?)?
    };
    let n = if quantas.is_null() { 1 } else {
        quantas.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|&n| n >= 1)
            .ok_or_else(|| feathersheet_core::tr!("A quantidade deve ser inteiro a partir de 1"))?
    };
    let limite = if coluna { MAX_COLS } else { MAX_ROWS };
    if u64::from(at) + u64::from(n) > u64::from(limite) {
        return Err(feathersheet_core::tr!("Ultrapassa o fim da planilha").into());
    }
    com_transacoes(wb, alteradas, aba, |wb| {
        let feito = match op {
            "insertRow" => wb.inserir_linhas(aba, at, n),
            "deleteRow" => wb.excluir_linhas(aba, at, n),
            "insertColumn" => wb.inserir_colunas(aba, at, n),
            _ => wb.excluir_colunas(aba, at, n),
        };
        feito.map(|_| ()).ok_or_else(|| feathersheet_core::trf!("{op}: não foi possível alterar a planilha"))
    })
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn movimento_reaproveita_conserto_de_formulas_de_outra_aba() {
        let mut wb = Workbook::new();
        wb.criar_aba("Outra");
        let nome = wb.aba(0).unwrap().sheet.name.clone();
        wb.escrever_celula(0,0,0,"10".into());
        wb.escrever_celula(0,1,0,"20".into());
        wb.escrever_celula(1,0,0,format!("={nome}!A1+{nome}!A2"));
        let mut alteradas = BTreeSet::new();
        mover(&mut wb,&mut alteradas,0,&Rect::new(0,0,1,0),
            &Rect::cell(3,3)).unwrap();
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(3,3),"10");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(4,3),"20");
        assert_eq!(wb.aba(1).unwrap().sheet.get_raw(0,0),
            format!("={nome}!D4+{nome}!D5"));
        assert!(alteradas.contains(&0));
        assert!(alteradas.contains(&1));
        // Simula um erro JS posterior ao movimento.
        for i in &alteradas {
            wb.aba_mut(*i).unwrap().sheet.reverter_transacao();
        }
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"10");
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(3,3),"");
        assert_eq!(wb.aba(1).unwrap().sheet.get_raw(0,0),
            format!("={nome}!A1+{nome}!A2"));
    }

    #[test]
    fn limite_rejeitado_sem_abrir_transacoes() {
        let mut wb = Workbook::new();
        wb.escrever_celula(0,0,0,"preservado".into());
        let mut alteradas = BTreeSet::new();
        let versao = wb.versao(0);
        assert!(mover(&mut wb,&mut alteradas,0,&Rect::new(0,0,1,1),
            &Rect::cell(MAX_ROWS-1,MAX_COLS-1)).is_err());
        assert_eq!(wb.versao(0),versao);
        assert!(alteradas.is_empty());
        assert_eq!(wb.aba(0).unwrap().sheet.get_raw(0,0),"preservado");
    }

    #[test]
    fn mover_para_mesma_posicao_nao_modifica_documento() {
        let mut wb = Workbook::new();
        let mut alteradas=BTreeSet::new();
        mover(&mut wb,&mut alteradas,0,&Rect::cell(0,0),&Rect::cell(0,0))
            .unwrap();
        assert!(alteradas.is_empty());
    }
}
