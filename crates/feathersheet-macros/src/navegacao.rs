//! Migração da API de navegação de macros (`api-navegacao.ts`).
//!
//! Não reimplementa armazenamento, parser A1 nem cálculo: consulta
//! `Sheet::chaves_no_retangulo` e `Workbook::valor` do core Rust.
use feathersheet_core::{
    celulas::{
        refs::{key_col, key_row, Rect},
        values::Value,
    },
    planilha::workbook::Workbook,
};

/// Retorna as posições (linha, coluna) em ordem de leitura, sem materializar uma matriz
/// de valores. A proteção de 500 mil posições fica no host de macros.
/// `visible` respeita o AutoFiltro pedido antes na mesma macro (`filtros`).
pub(crate) fn celulas_especiais(
    pasta: &Workbook, aba: usize, area: &Rect, tipo: &str, filtros: &[crate::pasta::PedidoFiltro],
) -> Result<Vec<(u32, u32)>, String> {
    if !matches!(tipo, "blanks" | "formulas" | "constants" | "errors" | "visible") {
        return Err(feathersheet_core::trf!("Tipo inválido: {tipo}. Use blanks, formulas, constants, errors ou visible"
        ));
    }
    let sheet = &pasta.aba(aba).ok_or("Aba inexistente")?.sheet;
    let mut resultado = Vec::new();
    if tipo == "visible" {
        let escondidas = crate::visiveis::linhas_escondidas(pasta, aba, area.r0, area.r1, filtros)?;
        let colunas: Vec<u32> = (area.c0..=area.c1).filter(|&c| !sheet.coluna_oculta(c)).collect();
        for (row, _) in (area.r0..=area.r1).zip(&escondidas).filter(|(_, e)| !**e) {
            resultado.extend(colunas.iter().map(|&col| (row, col)));
        }
        return Ok(resultado);
    }
    if tipo == "blanks" {
        // Incluir vazias com estilo, exatamente como o TS: o teste é
        // pelo texto bruto, não por presença na tabela colunar.
        for row in area.r0..=area.r1 {
            for col in area.c0..=area.c1 {
                if sheet.get_raw(row, col).is_empty() {
                    resultado.push((row, col));
                }
            }
        }
        return Ok(resultado);
    }

    // Esta enumeração já existe no core: cobre células normais,
    // armazenamento colunar e fórmulas compactadas, sem fazer outra
    // varredura da planilha inteira nem materializar as vazias.
    for chave in sheet.chaves_no_retangulo(area) {
        let row = key_row(chave);
        let col = key_col(chave);
        let bruto = sheet.get_raw(row, col);
        let corresponde = match tipo {
            "formulas" => bruto.starts_with('='),
            "constants" => !bruto.is_empty() && !bruto.starts_with('='),
            "errors" => matches!(pasta.valor(aba, row, col), Value::Error(_)),
            _ => unreachable!("tipo validado acima"),
        };
        if corresponde {
            resultado.push((row, col));
        }
    }
    Ok(resultado)
}

#[cfg(test)]
mod testes {
    use super::*;
    use feathersheet_core::celulas::refs::cell_name;

    fn celulas_especiais(pasta: &Workbook, aba: usize, area: &Rect, tipo: &str, f: &[crate::pasta::PedidoFiltro]) -> Result<Vec<String>, String> {
        super::celulas_especiais(pasta, aba, area, tipo, f).map(|v| v.into_iter().map(|(r, c)| cell_name(r, c)).collect())
    }

    #[test]
    fn seleciona_formulas_constantes_vazias_e_erros_sem_alterar_valores() {
        let mut pasta = Workbook::new();
        pasta.escrever_celula(0, 0, 0, "7".into());
        pasta.escrever_celula(0, 0, 1, "=A1+1".into());
        pasta.escrever_celula(0, 1, 0, "=1/0".into());
        let area = Rect::new(0, 0, 1, 2);
        assert_eq!(celulas_especiais(&pasta, 0, &area, "constants", &[]).unwrap(), vec!["A1".to_string()]);
        assert_eq!(celulas_especiais(&pasta, 0, &area, "formulas", &[]).unwrap(), vec!["B1".to_string(), "A2".to_string()]);
        assert_eq!(celulas_especiais(&pasta, 0, &area, "blanks", &[]).unwrap(), vec!["C1".to_string(), "B2".to_string(), "C2".to_string()]);
        assert_eq!(celulas_especiais(&pasta, 0, &area, "errors", &[]).unwrap(), vec!["A2".to_string()]);
        assert_eq!(pasta.aba(0).unwrap().sheet.get_raw(0, 0), "7");
    }

    #[test]
    fn retorna_resultados_ordenados_com_armazenamento_colunar() {
        let mut pasta = Workbook::new();
        pasta.escrever_celula(0, 500, 6, "z".into());
        pasta.escrever_celula(0, 10, 2, "a".into());
        pasta.escrever_celula(0, 20, 1, "b".into());
        let area = Rect::new(0, 0, 700, 6);
        assert_eq!(celulas_especiais(&pasta, 0, &area, "constants", &[]).unwrap(),
            vec!["C11".to_string(), "B21".to_string(), "G501".to_string()]);
        assert!(celulas_especiais(&pasta, 0, &area, "invalid", &[]).is_err());
    }
}
