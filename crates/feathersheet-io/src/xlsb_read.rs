//! Abrir `.xlsb` (o formato binário do Excel) pelo crate `calamine`: valores, fórmulas (convertidas como as do
//! `.xlsx`) e nomes definidos. Não vem a formatação (cores, bordas, larguras) nem mesclas, gráficos e imagens:
//! o `calamine` não lê isso no `.xlsb`. Gravar `.xlsb` não existe: o app abre como "sem arquivo" e o Ctrl+S
//! pede onde salvar (`.xlsx` ou `.fshz`).
use calamine::{open_workbook_from_rs, Data, Reader, Xlsb};
use feathersheet_core::celulas::refs::{parse_range_ref, MAX_COLS, MAX_ROWS};
use feathersheet_core::celulas::values::{CellError, ErrorCode, Value};
use feathersheet_core::planilha::workbook::Workbook;

use crate::xlsx_read::formula::formula_from_excel;
use crate::xlsx_read::sheet::format_date_serial;

/// O `.xlsb` em `bytes` como pasta, já calculada.
pub fn ler_xlsb(bytes: Vec<u8>) -> Result<Workbook, String> {
    let mut xlsb: Xlsb<_> = open_workbook_from_rs(std::io::Cursor::new(bytes))
        .map_err(|e| feathersheet_core::trf!("XLSB: não deu para abrir ({e})."))?;
    let nomes = xlsb.sheet_names().to_vec();
    if nomes.is_empty() { return Err(feathersheet_core::tr!("XLSB sem abas.").into()); }
    let mut pasta = Workbook::new();
    for (i, nome) in nomes.iter().enumerate() {
        if i == 0 { pasta.renomear_aba(0, nome); } else { pasta.inserir_aba(nome.clone(), i); }
        let valores = xlsb.worksheet_range(nome).map_err(|e| feathersheet_core::trf!("XLSB: aba '{nome}': {e}"))?;
        let (r0, c0) = valores.start().unwrap_or((0, 0));
        let aba = &mut pasta.aba_mut(i).ok_or("XLSB")?.sheet;
        let mut datas: Vec<(u32, u32, String)> = Vec::new();
        for (r, c, v) in valores.used_cells() {
            let (r, c) = (r0 + r as u32, c0 + c as u32);
            if r >= MAX_ROWS || c >= MAX_COLS { continue; }
            match v {
                Data::Int(n) => aba.set_cell_valor(r, c, Value::Number(*n as f64)),
                Data::Float(n) => aba.set_cell_valor(r, c, Value::Number(*n)),
                Data::String(s) => aba.set_cell_valor(r, c, Value::Text(s.clone())),
                Data::Bool(b) => aba.set_cell_valor(r, c, Value::Bool(*b)),
                Data::Error(e) => if let Some(code) = ErrorCode::from_code(&e.to_string()) {
                    aba.set_cell_valor(r, c, Value::Error(CellError::new(code)));
                },
                // A data vai como o leitor de `.xlsx` a entrega: texto de data que o motor reconhece e formata.
                Data::DateTime(d) => {
                    // shortcut: o calamine não expõe a base 1904 (só no Debug); trocar quando houver método público.
                    let base_1904 = format!("{d:?}").contains("is_1904: true");
                    let n = d.as_f64() + if base_1904 { 1462.0 } else { 0.0 };
                    let so_hora = d.is_duration() || n < 1.0;
                    datas.push((r, c, format_date_serial(n, n.fract() != 0.0 && !so_hora, so_hora)));
                }
                Data::DateTimeIso(s) | Data::DurationIso(s) => aba.set_cell_valor(r, c, Value::Text(s.clone())),
                Data::Empty => {}
            }
        }
        // Entram como se digitadas: o motor reconhece "01/01/2021" e põe o formato de data.
        for (r, c, texto) in datas { pasta.escrever_celula(i, r, c, texto); }
        if let Ok(formulas) = xlsb.worksheet_formula(nome) {
            let (f0, g0) = formulas.start().unwrap_or((0, 0));
            for (r, c, f) in formulas.used_cells() {
                let (r, c) = (f0 + r as u32, g0 + c as u32);
                if f.is_empty() || r >= MAX_ROWS || c >= MAX_COLS { continue; }
                pasta.escrever_celula(i, r, c, formula_from_excel(&sem_user(f)));
            }
        }
    }
    for (nome, refere) in xlsb.defined_names() {
        if refere.contains("#REF!") || nome.starts_with("_xlfn.") { continue; }
        // `Plan1!$A$1:$A$6` ou `'Minha aba'!$A$1`: a aba vai no retângulo, o resto é o intervalo.
        let refere = refere.replace('$', "");
        let Some((aba, intervalo)) = refere.rsplit_once('!') else { continue };
        let aba = aba.trim_matches('\'').replace("''", "'");
        if let Some(mut area) = parse_range_ref(intervalo) {
            area.sheet = Some(aba);
            pasta.definir_nome_na_leitura(nome, area);
        }
    }
    pasta.calcular_tudo();
    Ok(pasta)
}

/// O `calamine` escreve função que não conhece como `User(_xlfn.CONCAT,"A","b")`: volta a `_xlfn.CONCAT("A","b")`.
fn sem_user(f: &str) -> String {
    let mut s = f.to_string();
    while let Some(i) = s.find("User(") {
        let depois = &s[i + 5..];
        let fim_nome = depois.find([',', ')']).unwrap_or(depois.len());
        let nome = depois[..fim_nome].to_string();
        let resto = &depois[fim_nome..];
        let resto = resto.strip_prefix(',').unwrap_or(resto);
        s = format!("{}{nome}({resto}", &s[..i]);
    }
    s
}

#[cfg(test)]
mod testes {
    #[test]
    fn user_volta_ao_nome_da_funcao() {
        assert_eq!(super::sem_user("User(_xlfn.CONCAT,\"A\",\"b\")"), "_xlfn.CONCAT(\"A\",\"b\")");
        assert_eq!(super::sem_user("1+User(X)"), "1+X()");
        assert_eq!(super::sem_user("SUM(A1:A3)"), "SUM(A1:A3)");
    }
}
