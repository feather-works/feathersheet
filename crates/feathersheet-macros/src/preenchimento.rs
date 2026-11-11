//! Porta a validação/roteamento do autoFill de src/macros/api-valores.ts.
//! A série numérica, texto incremental, estilo e fórmulas já pertencem ao
//! motor Rust (operacoes/fill.rs). Não reimplementar essa lógica nas macros.
use feathersheet_core::{
    celulas::refs::{Rect, MAX_COLS, MAX_ROWS},
    planilha::sheet::Sheet,
};

const TETO_CELULAS: u64 = 500_000;

/// Aceita extensão da fonte em uma única direção (alça do Excel).
/// Pré-valida antes de alocar o plano de preenchimento no core.
pub(crate) fn validar(origem: &Rect, destino: &Rect) -> Result<bool, String> {
    if origem.sheet.is_some() || destino.sheet.is_some() {
        return Err(feathersheet_core::tr!("autoFill aceita endereços da aba atual; selecione wb.sheet(nome)").into());
    }
    let o = origem.normalized();
    let d = destino.normalized();
    if o.r1 >= MAX_ROWS || o.c1 >= MAX_COLS ||
        d.r1 >= MAX_ROWS || d.c1 >= MAX_COLS {
        return Err(feathersheet_core::tr!("autoFill ultrapassa limites de linhas ou colunas").into());
    }
    if o.area() > TETO_CELULAS || d.area() > TETO_CELULAS {
        return Err(feathersheet_core::tr!("autoFill excede 500.000 células: restrinja o intervalo").into());
    }
    if o == d {
        return Ok(false);
    }
    if d.r0 > o.r0 || d.c0 > o.c0 || d.r1 < o.r1 || d.c1 < o.c1 {
        return Err(feathersheet_core::tr!("O destino deve incluir todo o intervalo de origem").into());
    }
    let vertical = d.c0 == o.c0 && d.c1 == o.c1
        && ((d.r0 < o.r0 && d.r1 == o.r1)
            || (d.r0 == o.r0 && d.r1 > o.r1));
    let horizontal = d.r0 == o.r0 && d.r1 == o.r1
        && ((d.c0 < o.c0 && d.c1 == o.c1)
            || (d.c0 == o.c0 && d.c1 > o.c1));
    if !vertical && !horizontal {
        return Err(feathersheet_core::tr!("O destino deve ampliar a origem em uma direção: cima, baixo, esquerda ou direita").into());
    }
    Ok(true)
}

/// Somente chamado após validar(...)=Ok(true). A transação é propriedade
/// do host: a chamada a Sheet::preencher aninha a sua transação de célula
/// na transação maior da macro, de forma a manter um único Ctrl+Z.
pub(crate) fn aplicar(sheet: &mut Sheet, origem: &Rect, destino: &Rect) -> usize {
    sheet.preencher(origem, destino)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limites_e_direcoes_rejeitados_antes_da_escrita() {
        let a = Rect::new(0,0,1,1);
        assert!(!validar(&a, &a).unwrap());
        for valido in [
            Rect::new(0,0,4,1), Rect::new(0,0,1,4),
        ] {
            assert!(validar(&a, &valido).unwrap());
        }
        for invalido in [
            Rect::new(1,0,4,1), Rect::new(0,1,1,4),
            Rect::new(0,0,4,4), Rect::new(0,0,0,1),
            Rect::new(0,0,500_000,1),
        ] {
            assert!(validar(&a, &invalido).is_err(), "{invalido:?}");
        }
        let no_meio = Rect::new(3,3,4,4);
        assert!(validar(&no_meio, &Rect::new(1,3,4,4)).unwrap());
        assert!(validar(&no_meio, &Rect::new(3,1,4,4)).unwrap());
    }

    #[test]
    fn reusa_mecanismo_de_series_formulas_e_undo() {
        let mut s = Sheet::new("Dados");
        s.set_cell(0,0,"10");
        s.set_cell(1,0,"20");
        s.set_cell(0,1,"=A1*2");
        let o=Rect::new(0,0,1,1);
        let d=Rect::new(0,0,3,1);
        assert!(validar(&o,&d).unwrap());
        s.abrir_transacao();
        assert_eq!(aplicar(&mut s,&o,&d),4);
        s.fechar_transacao();
        assert_eq!(s.get_raw(2,0),"30");
        assert_eq!(s.get_raw(2,1),"=A3*2");
        assert_eq!(s.get_raw(3,0),"40");
        assert!(s.undo());
        assert_eq!(s.get_raw(2,0),"");
        assert_eq!(s.get_raw(2,1),"");
    }
}
