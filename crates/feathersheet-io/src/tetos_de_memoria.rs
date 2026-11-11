//! MEM-3 e MEM-5: abrir, compactar, salvar e reabrir dentro do teto de memória
//! ([`crate::teste_memoria::Teto`], o mesmo alocador do motor).
//!
//! shortcut: o `.xlsx` lê as abas em threads (`std::thread::scope`), e o alocador conta só a
//! thread do teste; o teto do `.xlsx` vale para a montagem, não para a descompressão das abas.

use crate::teste_memoria::Teto;
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::workbook::Workbook;

/// O relatório de muitas colunas curtas: 9 abas × 200 colunas × 30 linhas, número e texto.
/// Era o caso que reservava 65.536 linhas por coluna (~70 MB, 29/10/2026) e, até 10/11/2026,
/// 8 KB de máscara por coluna (27 MB para abrir; hoje 2,2 MB).
fn colunas_curtas() -> Workbook {
    let mut wb = Workbook::vazia();
    for a in 0..9 {
        let i = wb.criar_aba(format!("Aba{a}"));
        let s = &mut wb.aba_mut(i).unwrap().sheet;
        for c in 0..200u32 {
            s.set_cell(0, c, &format!("Col{c}"));
            for r in 1..30u32 {
                let v = if c % 4 == 0 { format!("t{}", r % 7) } else { ((r * 31 + c) % 1000).to_string() };
                s.set_cell(r, c, &v);
            }
        }
    }
    wb
}

#[test]
fn abrir_fshz_de_colunas_curtas_cabe_no_teto() {
    let bytes = crate::da_workbook::pasta_gravar_bytes(&colunas_curtas(), 0).unwrap();
    let teto = Teto::novo("abrir_fshz_de_colunas_curtas", 8 << 20);
    let volta = crate::para_workbook::para_workbook_soltando(crate::fshz_read::ler_fshz(&bytes).unwrap());
    eprintln!("MEM-3 .fshz colunas curtas ({} KB): pico {} KB", bytes.len() >> 10, teto.pico() >> 10);
    assert_eq!(volta.valor(8, 29, 199), Value::Number(((29 * 31 + 199) % 1000) as f64));
}

#[test]
fn abrir_xlsx_de_colunas_curtas_cabe_no_teto() {
    let bytes = crate::xlsx_write::gerar_xlsx(&colunas_curtas()).unwrap();
    let teto = Teto::novo("abrir_xlsx_de_colunas_curtas", 8 << 20);
    let (volta, _) = crate::xlsx_read::ler_xlsx_workbook(bytes).unwrap();
    eprintln!("MEM-3 .xlsx colunas curtas: pico {} KB", teto.pico() >> 10);
    assert_eq!(volta.valor(8, 29, 199), Value::Number(((29 * 31 + 199) % 1000) as f64));
}

/// MEM-5: 200 mil linhas (número, texto repetido e fórmula), compactar, salvar e reabrir
/// dentro do teto, com todos os valores iguais.
#[test]
fn compactar_salvar_e_reabrir_cabe_no_teto() {
    const N: u32 = 200_000;
    let mut wb = Workbook::vazia();
    let i = wb.criar_aba("Dados");
    {
        let s = &mut wb.aba_mut(i).unwrap().sheet;
        let linhas: Vec<Vec<String>> = (0..N).map(|r| vec![(r % 5000).to_string(), format!("k{}", r % 13)]).collect();
        s.gravar_bloco_literais(0, 0, linhas);
        s.set_cell(0, 2, "=A1*2");
        assert!(s.preencher_formula_faixa_vazia(2, 1, N - 1, "=A2*2"));
    }
    let teto = Teto::novo("compactar_salvar_e_reabrir", 32 << 20);
    wb.compactar_colunas();
    let bytes = crate::da_workbook::pasta_gravar_bytes(&wb, 0).unwrap();
    let volta = crate::para_workbook::para_workbook_soltando(crate::fshz_read::ler_fshz(&bytes).unwrap());
    eprintln!("MEM-5 compactar + salvar + reabrir {N} linhas ({} KB): pico {} KB", bytes.len() >> 10, teto.pico() >> 10);
    drop(teto);
    for r in (0..N).step_by(997).chain([N - 1]) {
        for c in 0..2 {
            assert_eq!(volta.valor(0, r, c), wb.valor(0, r, c), "linha {r}, coluna {c}");
        }
        assert_eq!(volta.aba(0).unwrap().sheet.get_raw(r, 2), format!("=A{}*2", r + 1));
    }
}
