//! Abrir `.xlsb` (o formato binário do Excel) pelo crate `calamine`: valores, fórmulas (convertidas como as do
//! `.xlsx`) e nomes definidos. Não vem a formatação (cores, bordas, larguras) nem mesclas, gráficos e imagens:
//! o `calamine` não lê isso no `.xlsb`. Gravar `.xlsb` não existe: o app abre como "sem arquivo" e o Ctrl+S
//! pede onde salvar (`.xlsx` ou `.fshz`).
//!
//! shortcut: fórmula compartilhada (coluna preenchida) e algumas referências de tabela não vêm do calamine: a
//! célula fica com o valor. O status conta as fórmulas que não vieram.
use calamine::{open_workbook_from_rs, DataRef, Reader, SheetType, Xlsb};
use feathersheet_core::celulas::refs::{parse_range_ref, quote_sheet, MAX_COLS, MAX_ROWS};
use feathersheet_core::celulas::values::{CellError, ErrorCode, Value};
use feathersheet_core::planilha::workbook::Workbook;

use crate::xlsx_read::formula::formula_from_excel;
use crate::xlsx_read::sheet::format_date_serial;

/// O `.xlsb` em `bytes` como pasta já calculada, e quantas fórmulas não deu para ler.
pub fn ler_xlsb(bytes: Vec<u8>) -> Result<(Workbook, usize), String> {
    // Arquivo malformado faz o calamine fatiar fora do buffer (pânico): vira erro, não derruba o app.
    std::panic::catch_unwind(move || ler(bytes))
        .unwrap_or_else(|_| Err(feathersheet_core::tr!("XLSB: o arquivo está danificado.").into()))
}

fn ler(bytes: Vec<u8>) -> Result<(Workbook, usize), String> {
    let mut xlsb: Xlsb<_> = open_workbook_from_rs(std::io::Cursor::new(bytes))
        .map_err(|e| feathersheet_core::trf!("XLSB: não deu para abrir ({e})."))?;
    let abas: Vec<(String, bool)> = xlsb.sheets_metadata().iter().map(|s| (s.name.clone(), s.typ == SheetType::WorkSheet)).collect();
    if abas.is_empty() { return Err(feathersheet_core::tr!("XLSB sem abas.").into()); }
    // O calamine não põe aspas no nome da aba (`Vendas 2024!A1`): as que precisam ganham, da mais longa à mais curta.
    let mut com_aspas: Vec<(String, String)> = abas.iter()
        .map(|(n, _)| (format!("{n}!"), format!("{}!", quote_sheet(n))))
        .filter(|(a, b)| a != b).collect();
    com_aspas.sort_by_key(|(a, _)| std::cmp::Reverse(a.len()));
    let mut pasta = Workbook::new();
    let mut perdidas = 0;
    for (i, (nome, planilha)) in abas.iter().enumerate() {
        if i == 0 { pasta.renomear_aba(0, nome); } else { pasta.inserir_aba(nome.clone(), i); }
        // Aba de gráfico ou de diálogo: fica a aba, vazia.
        if !planilha { continue; }
        let aba = &mut pasta.aba_mut(i).ok_or("XLSB")?.sheet;
        let Ok(mut leitor) = xlsb.worksheet_cells_reader(nome) else { continue };
        while let Ok(Some(celula)) = leitor.next_cell() {
            let (r, c) = celula.get_position();
            if r >= MAX_ROWS || c >= MAX_COLS { continue; }
            match celula.get_value() {
                DataRef::Int(n) => aba.set_cell_valor(r, c, Value::Number(*n as f64)),
                DataRef::Float(n) => aba.set_cell_valor(r, c, Value::Number(*n)),
                DataRef::String(s) => aba.set_cell_valor(r, c, Value::Text(s.clone())),
                DataRef::SharedString(s) => aba.set_cell_valor(r, c, Value::Text((*s).to_string())),
                DataRef::Bool(b) => aba.set_cell_valor(r, c, Value::Bool(*b)),
                DataRef::Error(e) => if let Some(code) = ErrorCode::from_code(&e.to_string()) {
                    aba.set_cell_valor(r, c, Value::Error(CellError::new(code)));
                },
                DataRef::DateTime(d) => {
                    // shortcut: o calamine não expõe a base 1904 (só no Debug); trocar quando houver método público.
                    let n = d.as_f64() + if format!("{d:?}").contains("is_1904: true") { 1462.0 } else { 0.0 };
                    if d.is_duration() || n < 0.0 {
                        // [h]:mm de 36:00 é 1,5: como texto de hora perderia o dia.
                        aba.set_cell_valor(r, c, Value::Number(n));
                    } else {
                        // Entra como se digitada: o motor reconhece "01/01/2021" e põe o formato de data.
                        let so_hora = n < 1.0;
                        aba.set_cell(r, c, &format_date_serial(n, n.fract() != 0.0 && !so_hora, so_hora));
                    }
                }
                DataRef::DateTimeIso(s) | DataRef::DurationIso(s) => aba.set_cell_valor(r, c, Value::Text(s.clone())),
                DataRef::Empty => {}
            }
        }
        drop(leitor);
        let Ok(mut leitor) = xlsb.worksheet_cells_reader(nome) else { continue };
        loop {
            match leitor.next_formula() {
                Ok(Some(f)) => {
                    let (r, c) = f.get_position();
                    let texto = f.get_value();
                    if texto.is_empty() { perdidas += 1; continue; }
                    if r >= MAX_ROWS || c >= MAX_COLS { continue; }
                    let mut texto = relativas(&sem_user(texto));
                    for (sem, com) in &com_aspas { texto = com_aspas_no_nome(&texto, sem, com); }
                    aba.set_cell(r, c, &formula_from_excel(&texto));
                }
                // Aba cortada (sem o registro de fim): a leitura falha de novo a cada volta.
                Ok(None) | Err(calamine::XlsbError::Io(_)) => break,
                // O registro da fórmula já foi consumido: a próxima célula segue.
                Err(_) => perdidas += 1,
            }
        }
    }
    for (nome, refere) in xlsb.defined_names() {
        if refere.contains("#REF!") || nome.starts_with("_xlfn.") { continue; }
        // `Plan1!$A$1:$A$6` ou `'Minha aba'!$A$1`: a aba vai no retângulo, o resto é o intervalo.
        let limpo = refere.replace('$', "");
        let area = limpo.rsplit_once('!').and_then(|(aba, intervalo)| {
            let mut area = parse_range_ref(intervalo)?;
            area.sheet = Some(aba.trim_matches('\'').replace("''", "'"));
            Some(area)
        });
        match area {
            Some(area) => pasta.definir_nome_na_leitura(nome, area),
            // Valor ou fórmula (`0.1`, `Plan1!$A$1*2`). shortcut: o calamine não diz o escopo (aba) do nome.
            None => pasta.definir_nome_formula_na_leitura(nome, &formula_from_excel(refere), None),
        }
    }
    pasta.calcular_tudo();
    Ok((pasta, perdidas))
}

/// Troca `sem` (`2024!`) por `com` (`'2024'!`) só onde ele começa uma referência: não dentro de
/// `Vendas2024!` nem de texto entre aspas duplas.
fn com_aspas_no_nome(f: &str, sem: &str, com: &str) -> String {
    let mut saida = String::with_capacity(f.len() + 8);
    let (mut i, mut aspas) = (0, false);
    while i < f.len() {
        let resto = &f[i..];
        let antes = saida.chars().last();
        let livre = !antes.is_some_and(|a| a.is_alphanumeric() || a == '_' || a == '.' || a == '\'');
        if !aspas && livre && resto.starts_with(sem) {
            saida.push_str(com);
            i += sem.len();
            continue;
        }
        let ch = resto.chars().next().unwrap_or(' ');
        if ch == '"' { aspas = !aspas; }
        saida.push(ch);
        i += ch.len_utf8();
    }
    saida
}

/// O calamine 0.38 deixa os bits de "relativo" (0x4000 coluna, 0x8000 linha) no número da coluna dos intervalos e
/// das referências entre abas, e escreve tudo com `$`: `SOMA(A1:B2)` chega `SOMA($BTRM$1:$BTRN$2)`. Coluna acima de
/// XFD (16384) só pode ser isso: desfaz. Só no formato `$COL$LIN`, que nome definido não tem.
fn relativas(f: &str) -> String {
    let b = f.as_bytes();
    let mut saida = String::with_capacity(f.len());
    let (mut i, mut aspas) = (0, false);
    while i < b.len() {
        if b[i] == b'"' { aspas = !aspas; }
        if !aspas && b[i] == b'$' && (i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b'.')) {
            let letras = b[i + 1..].iter().take_while(|c| c.is_ascii_uppercase()).count();
            let j = i + 1 + letras;
            if (1..=4).contains(&letras) && b.get(j) == Some(&b'$') {
                let digitos = b[j + 1..].iter().take_while(|c| c.is_ascii_digit()).count();
                let fim = j + 1 + digitos;
                let livre = b.get(fim).is_none_or(|c| !(c.is_ascii_alphanumeric() || *c == b'_' || *c == b'('));
                let coluna = f[i + 1..j].bytes().fold(0u32, |n, c| n * 26 + u32::from(c - b'A' + 1));
                if digitos > 0 && livre && coluna > 16384 {
                    let v = coluna - 1;
                    let nome = feathersheet_core::celulas::refs::col_name(v & 0x3FFF);
                    let (col_rel, lin_rel) = (v & 0x4000 != 0, v & 0x8000 != 0);
                    saida.push_str(&format!("{}{nome}{}{}", if col_rel { "" } else { "$" }, if lin_rel { "" } else { "$" }, &f[j + 1..fim]));
                    i = fim;
                    continue;
                }
            }
        }
        let ch = f[i..].chars().next().unwrap_or(' ');
        saida.push(ch);
        i += ch.len_utf8();
    }
    saida
}

/// O `calamine` escreve função que não conhece como `User(_xlfn.CONCAT,"A","b")`: volta a `_xlfn.CONCAT("A","b")`.
/// Só o `User(` que começa um nome (não o fim de `GetUser(`) e fora de texto entre aspas.
fn sem_user(f: &str) -> String {
    let mut saida = String::with_capacity(f.len());
    let mut aspas = false;
    let mut resto = f;
    while let Some(ch) = resto.chars().next() {
        let antes = saida.chars().last();
        let comeca_nome = !antes.is_some_and(|a| a.is_alphanumeric() || a == '_' || a == '.');
        if ch == '"' { aspas = !aspas; }
        if !aspas && comeca_nome && resto.starts_with("User(") {
            let depois = &resto[5..];
            let fim = depois.find([',', ')']).unwrap_or(depois.len());
            saida.push_str(&depois[..fim]);
            saida.push('(');
            resto = &depois[fim..];
            resto = resto.strip_prefix(',').unwrap_or(resto);
            continue;
        }
        saida.push(ch);
        resto = &resto[ch.len_utf8()..];
    }
    saida
}

#[cfg(test)]
mod testes {
    #[test]
    fn bits_de_relativo_saem_da_coluna() {
        assert_eq!(super::relativas("SUM($BTRM$1:$BTRN$2)"), "SUM(A1:B2)");
        assert_eq!(super::relativas("SUM($AVLI$1:$XFF$2)"), "SUM($A1:B$2)");
        assert_eq!(super::relativas("Sheet2!$BTRM$1+$A$1"), "Sheet2!A1+$A$1", "coluna válida fica");
        assert_eq!(super::relativas("\"$BTRM$1\""), "\"$BTRM$1\"", "texto fica");
    }

    #[test]
    fn aspas_so_no_nome_inteiro() {
        assert_eq!(super::com_aspas_no_nome("2024!A1+Vendas2024!A1", "2024!", "'2024'!"), "'2024'!A1+Vendas2024!A1");
        assert_eq!(super::com_aspas_no_nome("\"2024!\"&2024!B2", "2024!", "'2024'!"), "\"2024!\"&'2024'!B2");
    }

    #[test]
    fn user_volta_ao_nome_da_funcao() {
        assert_eq!(super::sem_user("User(_xlfn.CONCAT,\"A\",\"b\")"), "_xlfn.CONCAT(\"A\",\"b\")");
        assert_eq!(super::sem_user("1+User(X)"), "1+X()");
        assert_eq!(super::sem_user("SUM(A1:A3)"), "SUM(A1:A3)");
        assert_eq!(super::sem_user("User(GetUser,A1)"), "GetUser(A1)", "nome que termina em User fica");
        assert_eq!(super::sem_user("\"User(\"&A1"), "\"User(\"&A1", "dentro de aspas fica");
    }
}
