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

#[test]
fn intervalo_com_varias_areas_repete_a_instrucao_em_cada_uma() {
    let wb = roda("Sub T()\n  Dim u As Long\n  u = 3\n  Range(\"A1:A\" & u & \",C2\").Value = 5\n  Range(\"A1,C2\").Font.Bold = True\nEnd Sub\n", "T");
    for (l, c) in [(0, 0), (2, 0), (1, 2)] {
        assert_eq!(wb.valor(0, l, c), Value::Number(5.0));
    }
    assert_eq!(wb.valor(0, 1, 1), Value::Empty);
}

#[test]
fn shell_vira_app_shell_e_so_anota_o_comando() {
    let vba = "Sub T()\n  Dim id As Double\n  id = Shell(\"notepad.exe \" & \"a.txt\", vbNormalFocus)\n  Shell \"calc.exe\"\n  Call Shell(\"cmd /c dir\")\n  Range(\"A1\").Value = id\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").expect("macro convertida").codigo.clone();
    assert!(!js.contains("Shell"), "nada fica como VBA não convertido: {js}");
    let mut wb = Workbook::new();
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    assert_eq!(r.programas, ["notepad.exe a.txt", "calc.exe", "cmd /c dir"]);
    assert_eq!(wb.valor(0, 0, 0), Value::Number(0.0));
}

#[test]
fn choose_switch_strcomp_e_formatos_rodam_no_motor() {
    let wb = roda(
        "Function Opc(Optional x As Variant) As Boolean\n  Opc = IsMissing(x)\nEnd Function\nSub T()\n  Dim n As Long: n = 75\n  Range(\"A1\").Value = Choose(2, \"a\", \"b\", \"c\")\n  Range(\"A2\").Value = IsNull(Choose(9, \"a\"))\n  Range(\"A3\").Value = Switch(n < 50, \"baixo\", n < 100, \"médio\", True, \"alto\")\n  Range(\"A4\").Value = StrComp(\"abc\", \"ABC\") & \"|\" & StrComp(\"abc\", \"ABC\", vbTextCompare) & \"|\" & StrComp(\"a\", \"b\")\n  Range(\"A5\").Value = IsDate(\"31/12/2025\") & \"|\" & IsDate(\"xyz\") & \"|\" & IsDate(\"12\")\n  Range(\"A6\").Value = Opc() & \"|\" & Opc(1)\n  Range(\"A7\").Value = Hex(255) & \"|\" & Hex(-1) & \"|\" & Oct(8)\n  Range(\"A8\").Value = MonthName(3) & \"|\" & MonthName(12, True) & \"|\" & WeekdayName(1) & \"|\" & WeekdayName(1, True, vbMonday)\n  Range(\"A9\").Value = FormatNumber(1234.567) & \"|\" & FormatNumber(1234.5, 0, , , vbFalse) & \"|\" & FormatCurrency(10) & \"|\" & FormatPercent(0.256, 1)\n  Range(\"A10\").Value = FormatDateTime(DateSerial(2026, 11, 1), vbLongDate) & \"|\" & FormatDateTime(DateSerial(2026, 11, 1), vbShortDate) & \"|\" & FormatDateTime(TimeSerial(9, 5, 0), vbShortTime)\nEnd Sub\n",
        "T",
    );
    let t = |l| match wb.valor(0, l, 0) { Value::Text(s) => s.to_string(), v => format!("{v:?}") };
    assert_eq!(t(0), "b");
    assert_eq!(t(2), "médio");
    assert_eq!(t(3), "1|0|-1");
    assert_eq!(t(4), "True|False|False");
    assert_eq!(t(5), "True|False");
    assert_eq!(t(6), "FF|FFFF|10");
    assert_eq!(t(7), "março|dez|domingo|seg");
    assert_eq!(t(8), "1.234,57|1235|R$ 10,00|25,6%");
    assert_eq!(t(9), "domingo, 1 de novembro de 2026|01/11/2026|09:05");
    assert!(matches!(wb.valor(0, 1, 0), Value::Bool(true)), "{:?}", wb.valor(0, 1, 0));
}

#[test]
fn financeiras_batem_com_o_excel() {
    let wb = roda(
        "Sub T()\n  Dim f(1 To 5) As Double\n  f(1) = -1000: f(2) = 300: f(3) = 400: f(4) = 400: f(5) = 300\n  Range(\"A1\").Value = Pmt(0.01, 12, 10000)\n  Range(\"A2\").Value = FV(0.01, 12, -100)\n  Range(\"A3\").Value = PV(0.01, 12, -100)\n  Range(\"A4\").Value = NPer(0.01, -100, 1000)\n  Range(\"A5\").Value = IPmt(0.01, 1, 12, 10000)\n  Range(\"A6\").Value = PPmt(0.01, 1, 12, 10000)\n  Range(\"A7\").Value = NPV(0.1, f)\n  Range(\"A8\").Value = IRR(f)\n  Range(\"A9\").Value = MIRR(f, 0.1, 0.12)\n  Range(\"A10\").Value = Rate(12, -888.49, 10000)\n  Range(\"A11\").Value = SLN(10000, 1000, 5)\n  Range(\"A12\").Value = SYD(10000, 1000, 5, 1)\n  Range(\"A13\").Value = DDB(10000, 1000, 5, 2)\nEnd Sub\n",
        "T",
    );
    // Conferidos à parte (Python) e com o Excel: PGTO, VF, VP, NPER, IPGTO, PPGTO, VPL, TIR, MTIR, TAXA, DPD, SDA, BDD.
    let esperado = [-888.4878867, 1268.250301, 1125.507747, 10.5886445, -100.0, -788.4878867,
        98.8506746, 0.1489503, 0.1369977, 0.0100002, 1800.0, 3000.0, 2400.0];
    for (l, e) in esperado.iter().enumerate() {
        let Value::Number(x) = wb.valor(0, l as u32, 0) else { panic!("A{} = {:?}", l + 1, wb.valor(0, l as u32, 0)) };
        assert!((x - e).abs() < 1e-4 * e.abs().max(1.0), "A{}: {x} ≠ {e}", l + 1);
    }
}

#[test]
fn tipos_erros_e_opcionais_de_texto_rodam_no_motor() {
    let wb = roda(
        "Sub T()\n  Dim r As Range, v As Variant, m As Variant, x As Variant\n  Set r = Range(\"B1:B3\")\n  m = Array(\"maçã\", \"Banana\", \"uva\")\n  Range(\"A1\").Value = TypeName(r) & \"|\" & TypeName(ActiveSheet) & \"|\" & TypeName(v) & \"|\" & TypeName(1.5) & \"|\" & TypeName(\"a\") & \"|\" & TypeName(m) & \"|\" & TypeName(True)\n  Range(\"A2\").Value = VarType(v) & \"|\" & VarType(\"a\") & \"|\" & VarType(m) & \"|\" & VarType(r) & \"|\" & IsObject(r) & \"|\" & IsObject(5)\n  x = CVErr(xlErrNA)\n  Range(\"A3\").Value = IsError(x) & \"|\" & IsError(5) & \"|\" & TypeName(x)\n  Range(\"A4\").Value = Join(Filter(m, \"an\"), \",\") & \"|\" & Join(Filter(m, \"A\", True, vbTextCompare), \",\") & \"|\" & Join(Filter(m, \"an\", False), \",\")\n  Range(\"A5\").Value = (Partition(25, 0, 99, 10) = \" 20: 29\") & \"|\" & (Partition(150, 0, 99, 10) = \"100:   \") & \"|\" & (Str(5) = \" 5\")\n  Range(\"A6\").Value = QBColor(12) & \"|\" & Error(11) & \"|\" & Str(-5)\n  Range(\"A7\").Value = Replace(\"aXbXcX\", \"X\", \"-\", 3, 1) & \"|\" & Replace(\"AxA\", \"a\", \"o\", 1, -1, vbTextCompare) & \"|\" & InStrRev(\"a-b-c\", \"-\") & \"|\" & InStrRev(\"a-b-c\", \"-\", 3)\n  Range(\"A8\").Value = Join(Split(\"a,b,c,d\", \",\", 2), \"|\") & \"/\" & UBound(Split(\"\", \",\"))\nEnd Sub\nSub Estouro()\n  Range(\"A1\").Value = CInt(40000)\nEnd Sub\n",
        "T",
    );
    let t = |l| match wb.valor(0, l, 0) { Value::Text(s) => s.to_string(), v => format!("{v:?}") };
    assert_eq!(t(0), "Range|Worksheet|Empty|Double|String|Variant()|Boolean");
    assert_eq!(t(1), "0|8|8204|9|True|False");
    assert_eq!(t(2), "True|False|Error");
    assert_eq!(t(3), "Banana|maçã,Banana,uva|maçã,uva");
    assert_eq!(t(4), "True|True|True");
    assert_eq!(t(5), "255|Divisão por zero|-5");
    assert_eq!(t(6), "b-cX|oxo|4|2");
    assert_eq!(t(7), "a|b,c,d/-1");
    let c = converter_texto(&[("Módulo1", "Sub E()\n  Range(\"A1\").Value = CInt(40000)\nEnd Sub\n")], &[]);
    let js = &c.macros.iter().find(|m| m.nome == "E").unwrap().codigo;
    let r = feathersheet_macros::executar(&mut Workbook::new(), 0, js);
    assert!(r.erro.as_deref().is_some_and(|e| e.contains("Estouro")), "{:?}", r.erro);
}

#[test]
fn protect_e_unprotect_viram_pedidos_de_protecao() {
    use feathersheet_core::planilha::sheet::protecao::Permissao;
    let vba = "Sub T()\n  ActiveSheet.Unprotect \"abc\"\n  Range(\"A1\").Value = 1\n  ActiveSheet.Protect Password:=\"abc\", DrawingObjects:=True, Contents:=True, Scenarios:=False, AllowFormattingCells:=True, AllowSorting:=False\n  ActiveSheet.Protect\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").unwrap().codigo.clone();
    assert!(!js.contains("não convertido (") && !js.contains("Protect"), "{js}");
    let r = feathersheet_macros::executar(&mut Workbook::new(), 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    let p = &r.protecoes;
    assert_eq!(p.len(), 3, "{p:?}");
    assert!(!p[0].proteger && p[0].senha.as_deref() == Some("abc"));
    let mut perm = p[1].permitidas.clone();
    perm.sort();
    assert_eq!(perm, [Permissao::FormatarCelulas, Permissao::EditarCenarios]);
    assert!(p[2].proteger && p[2].senha.is_none());
}

#[test]
fn autofilter_sort_ordena_a_tabela_inteira() {
    let vba = "Sub T()\n  Range(\"B2\").Value = \"Nome\": Range(\"C2\").Value = \"Qtd\"\n  Range(\"B3\").Value = \"c\": Range(\"C3\").Value = 3\n  Range(\"B4\").Value = \"a\": Range(\"C4\").Value = 1\n  Range(\"B5\").Value = \"b\": Range(\"C5\").Value = 2\n  Range(\"B2:C5\").Select\n  Selection.AutoFilter\n  ActiveSheet.AutoFilter.Sort.SortFields.Clear\n  ActiveSheet.AutoFilter.Sort.SortFields.Add Key:=Range(\"B3:B5\"), SortOn:=xlSortOnValues, Order:=xlAscending\n  With ActiveSheet.AutoFilter.Sort\n    .Header = xlYes\n    .Apply\n  End With\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").unwrap().codigo.clone();
    assert!(!js.contains("// VBA não convertido ("), "tudo convertido: {js}");
    let mut wb = Workbook::new();
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    let t = |l, c| wb.valor(0, l, c);
    assert_eq!(t(1, 1), Value::Text("Nome".into()));
    assert_eq!((t(2, 1), t(2, 2)), (Value::Text("a".into()), Value::Number(1.0)), "a linha inteira anda junto");
    assert_eq!((t(4, 1), t(4, 2)), (Value::Text("c".into()), Value::Number(3.0)));
}

#[test]
fn display_gridlines_vira_show_gridlines() {
    let vba = "Sub T()\n  ActiveWindow.DisplayGridlines = False\n  ActiveWindow.DisplayHeadings = False\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").unwrap().codigo.clone();
    assert!(!js.contains("// VBA não convertido ("), "{js}");
    let r = feathersheet_macros::executar(&mut Workbook::new(), 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    assert_eq!(r.grades.len(), 1);
    assert!(!r.grades[0].1);
}

#[test]
fn shell_execute_open_vira_open_url() {
    let vba = "Private Declare PtrSafe Function ShellExecute Lib \"shell32.dll\" Alias \"ShellExecuteA\" (ByVal hwnd As LongPtr, ByVal lpOperation As String, ByVal lpFile As String, ByVal lpParameters As String, ByVal lpDirectory As String, ByVal nShowCmd As Long) As LongPtr\nSub T()\n  Dim lSuccess As LongPtr\n  lSuccess = ShellExecute(0, \"Open\", \"https://www.google.com/maps/search/?api=1&query=\" & Range(\"B1\").Value)\n  Range(\"A1\").Value = lSuccess\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").unwrap().codigo.clone();
    assert!(!js.contains("// VBA não convertido ("), "{js}");
    let mut wb = Workbook::new();
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    assert_eq!(r.links, ["https://www.google.com/maps/search/?api=1&query="]);
    assert_eq!(wb.valor(0, 0, 0), Value::Number(42.0));
}

#[test]
fn caixa_de_texto_do_fluxo_de_caixa_muda_de_cor_e_negrito() {
    // O padrão do "Fluxo de caixa.xlsm": selecionar a caixa e mexer na letra pelo Selection.
    let vba = "Sub T()\n  ActiveSheet.Unprotect\n  ActiveSheet.Shapes.Range(Array(\"TextBox 39\")).Select\n  Selection.ShapeRange.TextFrame2.TextRange.Font.Bold = msoFalse\n  Selection.ShapeRange.TextFrame2.TextRange.Font.Size = 14\n  With Selection.ShapeRange.TextFrame2.TextRange.Font.Fill\n    .Visible = msoTrue\n    .ForeColor.RGB = RGB(0, 176, 80)\n    .Transparency = 0\n    .Solid\n  End With\n  Selection.ShapeRange.TextFrame2.TextRange.Font.Bold = msoTrue\n  ActiveSheet.Shapes(\"TextBox 39\").TextFrame.Characters.Text = \"Lucro\"\n  ActiveSheet.Protect DrawingObjects:=True, Contents:=True, Scenarios:=True\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").unwrap().codigo.clone();
    assert!(!js.contains("// VBA não convertido ("), "{js}");
    let mut wb = Workbook::new();
    wb.editar(0, |s| { s.alterar_objetos(Some(r#"{"v":1,"objetos":[{"id":1,"forma":"rect","de":{},"ate":{},"nome":"TextBox 39","texto":"Saldo"}]}"#.into())); });
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    let ob = wb.aba(0).unwrap().sheet.ob.clone().unwrap();
    for trecho in [r#""texto":"Lucro""#, r#""negrito":true"#, r#""tamanho":14.0"#, r##""cor":"#00b050""##] {
        assert!(ob.contains(trecho), "{trecho} em {ob}");
    }
}
