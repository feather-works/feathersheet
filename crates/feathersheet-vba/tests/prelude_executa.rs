//! Executa de verdade o JavaScript convertido no motor embutido das macros, para pegar
//! métodos que o motor não tem (ex.: `String.prototype.substr` derrubava `vba.mid`).

use feathersheet_core::{celulas::values::Value, planilha::workbook::Workbook};
use feathersheet_vba::converter_texto;

fn roda(vba: &str, nome: &str) -> Workbook {
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == nome).expect("macro convertida").codigo.clone();
    let mut wb = Workbook::new();
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    wb
}

#[test]
fn mid_do_vba_funciona_no_motor_embutido() {
    let wb = roda(
        "Sub T()\n  Range(\"A1\").Value = Mid(\"abcdef\", 2, 3)\n  Range(\"A2\").Value = Mid(\"abc\", 5)\n  Range(\"A3\").Value = Mid(\"abc\", 2)\n  Range(\"A4\").Value = Mid(\"abc\", 2, 0)\nEnd Sub\n",
        "T",
    );
    assert_eq!(wb.valor(0, 0, 0), Value::Text("bcd".into()));
    assert_eq!(wb.valor(0, 1, 0), Value::Empty);
    assert_eq!(wb.valor(0, 2, 0), Value::Text("bc".into()));
    assert_eq!(wb.valor(0, 3, 0), Value::Empty);
}

#[test]
fn outras_funcoes_de_texto_e_format_rodam_no_motor() {
    let wb = roda(
        "Sub T()\n  Range(\"A1\").Value = Format(5, \"000\") & \"x\"\n  Range(\"A2\").Value = Format(1234.5, \"#,##0.00\")\n  Range(\"A3\").Value = Left(\"abcdef\", 2) & Right(\"abcdef\", 2)\n  Range(\"A4\").Value = Replace(Trim(\"  a-b \"), \"-\", \"+\")\n  Range(\"A5\").Value = InStr(\"abcdef\", \"cd\")\n  Range(\"A6\").Value = String(3, \"x\")\nEnd Sub\n",
        "T",
    );
    assert_eq!(wb.valor(0, 0, 0), Value::Text("005x".into()));
    assert_eq!(wb.valor(0, 2, 0), Value::Text("abef".into()));
    assert_eq!(wb.valor(0, 3, 0), Value::Text("a+b".into()));
    assert_eq!(wb.valor(0, 4, 0), Value::Number(3.0));
    assert_eq!(wb.valor(0, 5, 0), Value::Text("xxx".into()));
}

#[test]
fn range_value_le_e_escreve_matriz_do_1() {
    let wb = roda(
        "Sub T()\n  Dim d As Variant, s() As Variant, i As Long\n  Range(\"A1\").Value = 1: Range(\"A2\").Value = 2: Range(\"A3\").Value = 3\n  Range(\"B1\").Value = \"a\": Range(\"B2\").Value = \"b\": Range(\"B3\").Value = \"c\"\n  d = Range(\"A1:B3\").Value\n  For i = 1 To UBound(d, 1)\n    d(i, 1) = d(i, 1) * 10\n  Next i\n  Range(\"D1:E3\").Value = d\n  ReDim s(1 To 10, 1 To 3)\n  s(1, 1) = LBound(s, 1): s(1, 2) = UBound(d, 2): s(2, 3) = \"z\"\n  Range(\"G1\").Resize(2, 3).Value = s\n  Range(\"J1:L2\").Value = Array(\"x\", \"y\", \"w\")\nEnd Sub\n",
        "T",
    );
    assert_eq!(wb.valor(0, 0, 3), Value::Number(10.0));
    assert_eq!(wb.valor(0, 2, 3), Value::Number(30.0));
    assert_eq!(wb.valor(0, 2, 4), Value::Text("c".into()));
    assert_eq!(wb.valor(0, 0, 6), Value::Number(1.0));
    assert_eq!(wb.valor(0, 0, 7), Value::Number(2.0));
    assert_eq!(wb.valor(0, 1, 8), Value::Text("z".into()));
    // O recorte: a linha 3 da matriz (vazia) não entra.
    assert_eq!(wb.valor(0, 2, 6), Value::Empty);
    assert_eq!(wb.valor(0, 1, 11), Value::Text("w".into()));
}

// Lento no debug (passa do prazo de 30 s das macros): `cargo test --release -- --ignored`.
#[test]
#[ignore]
fn range_value_acima_do_teto_de_uma_chamada_vai_em_blocos() {
    let t = std::time::Instant::now();
    let wb = roda(
        "Sub T()\n  Dim s() As Variant, d As Variant, i As Long, c As Long\n  ReDim s(1 To 60000, 1 To 9)\n  For i = 1 To 60000\n    For c = 1 To 9\n      s(i, c) = i * 10 + c\n    Next c\n  Next i\n  Range(\"A1:I60000\").Value = s\n  d = Range(\"A1:I60000\").Value\n  Range(\"K1\").Value = UBound(d, 1)\n  Range(\"K2\").Value = d(60000, 9)\nEnd Sub\n",
        "T",
    );
    eprintln!("60 mil × 9: {:?}", t.elapsed());
    assert_eq!(wb.valor(0, 59_999, 8), Value::Number(600_009.0));
    assert_eq!(wb.valor(0, 0, 10), Value::Number(60_000.0));
    assert_eq!(wb.valor(0, 1, 10), Value::Number(600_009.0));
}

#[test]
fn datas_do_vba_sao_seriais_como_na_planilha() {
    let wb = roda(
        "Sub T()\n  Dim d As Date\n  d = DateValue(\"15/01/2026\")\n  Range(\"A1\").Value = d\n  Range(\"A2\").Value = Weekday(d)\n  Range(\"A3\").Value = \"t \" & Format(d, \"dd/mm/yyyy\")\n  Range(\"A4\").Value = \"t \" & Format(DateAdd(\"m\", 1, #1/31/2026#), \"dd/mm/yyyy\")\n  Range(\"A5\").Value = DateDiff(\"d\", #1/1/2026#, d)\n  Range(\"A6\").Value = \"t \" & Day(d) & \"/\" & Month(d) & \"/\" & Year(d)\n  Range(\"A7\").Value = \"t \" & Format(DateSerial(2026, 13, 1) + TimeSerial(14, 5, 0), \"dd/mm/yyyy hh:mm\")\n  Range(\"A8\").Value = \"t \" & Hour(#2:30:00 PM#) & \":\" & Minute(#2:30:00 PM#)\n  Range(\"A9\").Value = StrConv(\"maria DA silva\", vbProperCase)\n  Range(\"A10\").Value = Weekday(d, vbMonday)\n  Range(\"A11\").Value = DatePart(\"y\", d)\n  Range(\"A12\").Value = DateSerial(2026, 1, 15)\n  Range(\"A13\").Value = \"t \" & Range(\"A12\").Text\nEnd Sub\n",
        "T",
    );
    assert_eq!(wb.valor(0, 0, 0), Value::Number(46037.0));
    assert_eq!(wb.valor(0, 1, 0), Value::Number(5.0)); // quinta-feira
    assert_eq!(wb.valor(0, 2, 0), Value::Text("t 15/01/2026".into()));
    assert_eq!(wb.valor(0, 3, 0), Value::Text("t 28/02/2026".into()));
    assert_eq!(wb.valor(0, 4, 0), Value::Number(14.0));
    assert_eq!(wb.valor(0, 5, 0), Value::Text("t 15/1/2026".into()));
    assert_eq!(wb.valor(0, 6, 0), Value::Text("t 01/01/2027 14:05".into()));
    assert_eq!(wb.valor(0, 7, 0), Value::Text("t 14:30".into()));
    assert_eq!(wb.valor(0, 8, 0), Value::Text("Maria Da Silva".into()));
    assert_eq!(wb.valor(0, 9, 0), Value::Number(4.0));
    assert_eq!(wb.valor(0, 10, 0), Value::Number(15.0));
    assert_eq!(wb.valor(0, 11, 0), Value::Number(46037.0));
    assert_eq!(wb.valor(0, 12, 0), Value::Text("t 15/01/2026".into()));
}

#[test]
fn remove_duplicates_conta_as_colunas_do_intervalo() {
    let wb = roda(
        "Sub T()\n  Range(\"B1\").Value = \"id\": Range(\"C1\").Value = \"x\"\n  Range(\"B2\").Value = 1: Range(\"C2\").Value = \"a\"\n  Range(\"B3\").Value = 1: Range(\"C3\").Value = \"b\"\n  Range(\"B4\").Value = 2: Range(\"C4\").Value = \"c\"\n  Range(\"B1:C4\").RemoveDuplicates Columns:=1, Header:=xlYes\nEnd Sub\n",
        "T",
    );
    assert_eq!(wb.valor(0, 2, 1), Value::Number(2.0));
    assert_eq!(wb.valor(0, 2, 2), Value::Text("c".into()));
    assert_eq!(wb.valor(0, 3, 1), Value::Empty);
}

#[test]
fn format_conditions_add_com_estilo_vira_regra_da_planilha() {
    let wb = roda(
        "Sub T()\n  With Range(\"M2:M9\").FormatConditions\n    .Delete\n    With .Add(Type:=xlCellValue, Operator:=xlEqual, Formula1:=\"=\"\"Meta batida\"\"\")\n      .Interior.Color = RGB(198, 239, 206)\n      .Font.Color = RGB(0, 97, 0)\n      .Font.Bold = True\n    End With\n    With .Add(Type:=xlExpression, Formula1:=\"=AND($M2<>\"\"\"\",$M2<>\"\"x\"\")\")\n      .Interior.Color = RGB(255, 199, 206)\n    End With\n  End With\n  Range(\"A1:A3\").FormatConditions.Add Type:=xlCellValue, Operator:=xlGreater, Formula1:=\"=10\"\nEnd Sub\n",
        "T",
    );
    let r = wb.aba(0).unwrap().sheet.r.clone().expect("regras");
    assert_eq!(r.matches("cf_macro_").count(), 3, "{r}");
    let txt = r.to_lowercase();
    assert!(txt.contains("meta batida") && txt.contains("#c6efce") && txt.contains("and($m2"), "{r}");
}

#[test]
fn on_error_resume_next_pula_a_linha_que_falha_ate_o_goto_0() {
    let wb = roda(
        "Sub T()\n  Dim rel As Worksheet\n  Application.DisplayAlerts = False\n  On Error Resume Next\n  Worksheets(\"Nao existe\").Delete\n  Range(\"A1\").Value = Err.Number <> 0\n  On Error GoTo 0\n  Set rel = Worksheets.Add(After:=ActiveSheet)\n  rel.Name = \"Fechamento\"\n  rel.Range(\"A1\").Value = \"novo\"\nEnd Sub\n",
        "T",
    );
    assert_eq!(wb.valor(0, 0, 0), Value::Bool(true));
    assert_eq!(wb.nomes()[1], "Fechamento");
    assert_eq!(wb.valor(1, 0, 0), Value::Text("novo".into()));
}

#[test]
fn sem_resume_next_o_erro_para_a_macro() {
    let c = converter_texto(&[("Módulo1", "Sub T()\n  Worksheets(\"Nao existe\").Delete\n  Range(\"A1\").Value = 1\nEnd Sub\n")], &[]);
    let mut wb = Workbook::new();
    let r = feathersheet_macros::executar(&mut wb, 0, &c.macros[0].codigo);
    assert!(r.erro.is_some());
    assert_eq!(wb.valor(0, 0, 0), Value::Empty);
}
