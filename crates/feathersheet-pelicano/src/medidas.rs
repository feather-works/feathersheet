//! Autoajuste não bloqueante por amostragem limitada de células da coluna/linha.
//! A medição tipográfica precisa ainda depende de layout de texto do Iced.
use crate::grade::{Dimensao, ALTURA_PADRAO, LARGURA_PADRAO};
use feathersheet_core::planilha::workbook::Workbook;

/// É intencional limitar a amostra: a pasta suporta 100 milhões de linhas.
/// O autoajuste nunca aloca um objeto por célula da aba inteira.
pub fn autoajuste(w: &Workbook, indice: usize, dimensao: Dimensao) -> u32 {
    let Some(aba)=w.aba(indice) else {return LARGURA_PADRAO as u32};
    let sheet=&aba.sheet;
    match dimensao {
        Dimensao::Coluna(col) => {
            let mut maior=0usize;
            // Observa a parte inicial e a cauda dos dados para cobrir abas longas,
            // sem explorar milhões de linhas para uma operação de UI.
            let limite=sheet.max_row.min(9_999);
            for r in 0..=limite {
                maior=maior.max(sheet.get_raw(r,col).lines().map(|l|l.chars().count()).max().unwrap_or(0));
            }
            if sheet.max_row>limite {
                for r in sheet.max_row.saturating_sub(255)..=sheet.max_row {
                    maior=maior.max(sheet.get_raw(r,col).lines().map(|l|l.chars().count()).max().unwrap_or(0));
                }
            }
            if maior==0 {crate::grade::largura_padrao_da(sheet) as u32}
            else { ((maior as f32*7.3+20.0).ceil() as u32).clamp(48,400) }
        }
        Dimensao::Linha(row) => {
            let mut linhas=1usize;
            let limite=sheet.max_col.min(255);
            for c in 0..=limite {
                let raw=sheet.get_raw(row,c);
                if raw.is_empty() {continue;}
                let largura=sheet.col_widths.get(&c).copied().unwrap_or(crate::grade::largura_padrao_da(sheet) as u32);
                let caracteres=((largura as f32-16.0)/7.3).max(1.0) as usize;
                let ocupadas=raw.lines().map(|l|l.chars().count().max(1).div_ceil(caracteres)).sum::<usize>();
                linhas=linhas.max(ocupadas);
            }
            ((linhas as f32*18.0+10.0).max(ALTURA_PADRAO).ceil() as u32).clamp(22,100)
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn vazios_ajustam_para_padrao() {
        let w=Workbook::new();
        assert_eq!(autoajuste(&w,0,Dimensao::Coluna(0)),122);
        assert_eq!(autoajuste(&w,0,Dimensao::Linha(0)),28);
    }
    #[test]
    fn textos_longos_aumentam_largura_e_quebras_aumentam_altura() {
        let mut w=Workbook::new();
        w.escrever_celula(0,0,0,"Descrição muito extensa de produto".into());
        w.escrever_celula(0,0,1,"uma linha\noutra linha\nterceira linha".into());
        assert!(autoajuste(&w,0,Dimensao::Coluna(0))>122);
        assert!(autoajuste(&w,0,Dimensao::Linha(0))>28);
    }
}
