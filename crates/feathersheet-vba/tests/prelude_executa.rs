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

#[test]
fn byref_alterado_volta_para_quem_chamou() {
    // Variável volta alterada; expressão e ByVal não; global do módulo também volta.
    let wb = roda(
        "Dim g As Long\nSub T()\n  Dim a As Long, b As Long, s As String\n  a = 1: b = 1: s = \"x\"\n  Somar a, 5\n  Somar b + 0, 5\n  Dobrar a\n  Juntar s\n  Somar g, 7\n  Range(\"A1\").Value = a\n  Range(\"A2\").Value = b\n  Range(\"A3\").Value = s\n  Range(\"A4\").Value = g\n  Range(\"A5\").Value = Contar(a)\n  Range(\"A6\").Value = a\nEnd Sub\nSub Somar(total As Long, ByVal n As Long)\n  total = total + n\n  n = 0\nEnd Sub\nSub Dobrar(ByRef x As Long)\n  Dim i As Long\n  For i = 1 To 1\n    x = x * 2\n  Next i\nEnd Sub\nSub Juntar(t As String)\n  t = t & \"y\"\nEnd Sub\nFunction Contar(ByRef k As Long) As Long\n  k = k + 1\n  Contar = k * 10\nEnd Function\n",
        "T",
    );
    assert_eq!(wb.valor(0, 0, 0), Value::Number(12.0), "(1 + 5) * 2");
    assert_eq!(wb.valor(0, 1, 0), Value::Number(1.0), "expressão vai por valor");
    assert_eq!(wb.valor(0, 2, 0), Value::Text("xy".into()));
    assert_eq!(wb.valor(0, 3, 0), Value::Number(7.0));
    assert_eq!(wb.valor(0, 4, 0), Value::Number(130.0));
    assert_eq!(wb.valor(0, 5, 0), Value::Number(13.0));
}

#[test]
fn autofilter_convertido_pede_o_filtro_certo() {
    let c = converter_texto(&[("Módulo1", "Sub T()\n  Range(\"A1:B5\").AutoFilter Field:=2, Criteria1:=\">=10\"\n  Range(\"A1:B5\").AutoFilter 1, Array(\"=x\", \"y\"), xlFilterValues\n  Range(\"A1:B5\").AutoFilter 2, \"=a\", xlOr, \"=b\"\n  Range(\"A1:B5\").AutoFilter 2, 5, xlTop10Items\nEnd Sub\n")], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").expect("macro").codigo.clone();
    let mut wb = Workbook::new();
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    let criterios: Vec<_> = r.filtros.iter().map(|p| match &p.acao {
        feathersheet_macros::AcaoFiltro::Aplicar { coluna, criterio, .. } => (*coluna, criterio.as_ref().map(|c| c.to_string())),
        outra => panic!("{outra:?}"),
    }).filter(|(c, _)| c.is_some()).collect();
    let t = |s: &str| Some(s.to_string());
    assert_eq!(criterios, vec![(Some(2), t(r#"">=10""#)), (Some(1), t(r#"["x","y"]"#)), (Some(2), t(r#"["a","b"]"#)),
        (Some(2), t(r#"{"op":"top10","topo":true,"n":5,"percentual":false}"#))]);
    let c = converter_texto(&[("Módulo1", "Sub T()\n  Range(\"A1:B5\").AutoFilter 2, \">1\", xlAnd, \"<9\"\nEnd Sub\n")], &[]);
    let r = feathersheet_macros::executar(&mut wb, 0, &c.macros[0].codigo);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let feathersheet_macros::AcaoFiltro::Aplicar { criterio, .. } = &r.filtros[0].acao else { panic!() };
    assert_eq!(criterio.as_ref().map(|c| c.to_string()), t(r#"{"op":"e","val":[">1","<9"]}"#), "xlAnd de comparação");
}

/// Os dados de exemplo: Nome / UF / Valor, cinco linhas.
const DADOS: &str = "  Range(\"A1:C1\").Value = Array(\"Nome\", \"UF\", \"Valor\")\n  Range(\"A2:C2\").Value = Array(\"Bia\", \"SP\", 50)\n  Range(\"A3:C3\").Value = Array(\"Ana\", \"RJ\", 150)\n  Range(\"A4:C4\").Value = Array(\"Caio\", \"SP\", 250)\n  Range(\"A5:C5\").Value = Array(\"Davi\", \"MG\", 90)\n  Range(\"A6:C6\").Value = Array(\"Eva\", \"SP\", 120)\n";

#[test]
fn specialcells_visivel_depois_do_autofilter() {
    let vba = format!("Sub T()\n{DADOS}  Dim rng As Range, c As Range, n As Long, s As Double\n  Range(\"A1:C6\").AutoFilter Field:=2, Criteria1:=\"SP\"\n  Set rng = Range(\"A1:C6\").SpecialCells(xlCellTypeVisible)\n  Range(\"E1\").Value = rng.Count\n  Range(\"E2\").Value = rng.Address\n  For Each c In Range(\"C2:C6\").SpecialCells(xlCellTypeVisible)\n    s = s + c.Value\n  Next c\n  Range(\"E3\").Value = s\n  rng.Copy Destination:=Range(\"G1\")\n  Range(\"A1:C6\").AutoFilter Field:=3, Criteria1:=\">1000\"\n  On Error Resume Next\n  Set rng = Nothing\n  Set rng = Range(\"A2:C6\").SpecialCells(xlCellTypeVisible)\n  On Error GoTo 0\n  If rng Is Nothing Then Range(\"E4\").Value = \"nenhuma\"\n  Range(\"E5\").Value = Cells.SpecialCells(xlCellTypeLastCell).Address\nEnd Sub\n");
    let c = converter_texto(&[("Módulo1", &vba)], &[]);
    assert!(c.pendencias.is_empty(), "{:#?}", c.pendencias);
    let wb = roda(&vba, "T");
    let t = |x: &str| Value::Text(x.into());
    assert_eq!(wb.valor(0, 0, 4), Value::Number(12.0), "4 linhas visíveis × 3 colunas");
    assert_eq!(wb.valor(0, 1, 4), t("$A$1:$C$2,$A$4:$C$4,$A$6:$C$6"));
    assert_eq!(wb.valor(0, 2, 4), Value::Number(420.0), "50 + 250 + 120");
    assert_eq!((0..5).map(|l| wb.valor(0, l, 6)).collect::<Vec<_>>(), vec![t("Nome"), t("Bia"), t("Caio"), t("Eva"), Value::Empty], "Copy cola só as visíveis");
    assert_eq!(wb.valor(0, 3, 4), t("nenhuma"), "sem visível: erro 1004, pego pelo On Error");
    assert_eq!(wb.valor(0, 4, 4), t("$I$6"), "a última célula usada");
}

#[test]
fn specialcells_excluir_as_linhas_filtradas_e_limpar_vazias() {
    let vba = format!("Sub T()\n{DADOS}  Range(\"B3\").ClearContents\n  Range(\"B2:B6\").SpecialCells(xlCellTypeBlanks).Value = \"?\"\n  Range(\"A1:C6\").AutoFilter 2, \"SP\"\n  Range(\"A2:C6\").SpecialCells(xlCellTypeVisible).EntireRow.Delete\n  ActiveSheet.AutoFilterMode = False\nEnd Sub\n");
    let c = converter_texto(&[("Módulo1", &vba)], &[]);
    assert!(c.pendencias.is_empty(), "{:#?}", c.pendencias);
    let wb = roda(&vba, "T");
    let t = |x: &str| Value::Text(x.into());
    assert_eq!((0..4).map(|l| wb.valor(0, l, 0)).collect::<Vec<_>>(), vec![t("Nome"), t("Ana"), t("Davi"), Value::Empty], "as de SP saíram");
    assert_eq!(wb.valor(0, 1, 1), t("?"));
}

#[test]
fn shapes_visible_onaction_e_posicao_rodam() {
    let c = converter_texto(&[("Módulo1", "Sub T()\n  With ActiveSheet.Shapes(\"Button 1\")\n    .Visible = msoFalse\n    .OnAction = \"Gravar\"\n    .Left = 72: .Top = 36: .Width = 90: .Height = 30\n  End With\n  If ActiveSheet.Shapes(\"Button 1\").Visible = False Then Range(\"A1\").Value = ActiveSheet.Shapes(\"Button 1\").OnAction\nEnd Sub\nSub Gravar()\nEnd Sub\n")], &[]);
    assert!(c.pendencias.is_empty(), "{:#?}", c.pendencias);
    let js = c.macros.iter().find(|m| m.nome == "T").expect("macro").codigo.clone();
    let mut wb = Workbook::new();
    wb.editar(0, |s| { s.alterar_objetos(Some(r##"{"v":1,"objetos":[{"id":1,"forma":"rect","de":{},"ate":{},"nome":"Botão 1"}]}"##.into())); });
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    assert_eq!(wb.valor(0, 0, 0), Value::Text("Gravar".into()));
    assert_eq!(r.formas.len(), 4, "{:?}", r.formas);
}

#[test]
fn paridade_rnd_ccur_format_strconv_isdate_e_callbyname() {
    let vba = "Sub T()\n  Dim d As Date, x As Double\n  Range(\"A1\").Value = Round(Rnd(), 7)\n  Range(\"A2\").Value = Round(Rnd(), 6)\n  Range(\"A3\").Value = Round(Rnd(-1), 6)\n  Range(\"A4\").Value = Round(Rnd(0), 6)\n  x = Rnd(-1): Randomize 5\n  x = Rnd(): Range(\"A6\").Value = Rnd(-1): Randomize 5\n  Range(\"A5\").Value = (x <> Rnd())\n  Range(\"B1\").Value = CCur(1.00005)\n  Range(\"B2\").Value = CCur(1.00015)\n  Range(\"C1\").Value = Format(1234.5, \"Scientific\")\n  Range(\"C2\").Value = Format(0, \"Yes/No\") & Format(3, \"On/Off\")\n  Range(\"C3\").Value = Format(1234.5, \"General Number\")\n  Range(\"C4\").Value = Format(0.75, \"Medium Time\")\n  Range(\"C5\").Value = FormatNumber(-0.5, 2, vbFalse, vbTrue)\n  Range(\"D1\").Value = StrConv(\"abc\", vbUpperCase + vbWide)\n  Range(\"D2\").Value = StrConv(\"\u{30AB}\u{30BF}\", vbHiragana)\n  d = 45000\n  Range(\"E1\").Value = IsDate(d)\n  Range(\"E2\").Value = IsDate(x)\n  CallByName Range(\"F1\"), \"Value\", VbLet, 7\n  Range(\"F2\").Value = CallByName(Range(\"F1\"), \"Value\", VbGet) * 2\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    assert!(c.pendencias.is_empty(), "{:#?}", c.pendencias);
    let wb = roda(vba, "T");
    let v = |l: u32, col: u32| wb.valor(0, l, col);
    assert_eq!(v(0, 0), Value::Number(0.7055475), "primeiro Rnd do VBA");
    assert_eq!(v(1, 0), Value::Number(0.533424));
    assert_eq!(v(2, 0), Value::Number(0.224007), "Rnd(-1)");
    assert_eq!(v(3, 0), Value::Number(0.224007), "Rnd(0) repete");
    assert_eq!(v(4, 0), Value::Bool(false), "Rnd(-1) + Randomize com a mesma semente repete a sequência, como no VBA");
    assert_eq!(v(0, 1), Value::Number(1.0), "CCur arredonda para o par");
    assert_eq!(v(1, 1), Value::Number(1.0002));
    assert_eq!(v(0, 2), Value::Text("1,23E+03".into()));
    assert_eq!(v(1, 2), Value::Text("NãoAtivado".into()));
    assert_eq!(v(2, 2), Value::Number(1234.5), "\"1234,5\" vira número na célula, como no Excel");
    assert_eq!(v(3, 2), Value::Text("06:00 PM".into()));
    assert_eq!(v(4, 2), Value::Text("(,50)".into()));
    assert_eq!(v(0, 3), Value::Text("ＡＢＣ".into()));
    assert_eq!(v(1, 3), Value::Text("\u{304B}\u{305F}".into()));
    assert_eq!(v(0, 4), Value::Bool(true));
    assert_eq!(v(1, 4), Value::Bool(false));
    assert_eq!(v(0, 5), Value::Number(7.0));
    assert_eq!(v(1, 5), Value::Number(14.0));
}

#[test]
fn msgbox_de_tres_botoes_pergunta_e_devolve_o_codigo_do_vba() {
    let vba = "Sub T()\n  Dim r As Integer\n  r = MsgBox(\"Arquivo em uso\", vbAbortRetryIgnore + vbCritical + vbDefaultButton2, \"Erro\")\n  Range(\"A1\").Value = r\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = c.macros.iter().find(|m| m.nome == "T").unwrap().codigo.clone();
    let mut wb = Workbook::new();
    let r = feathersheet_macros::executar(&mut wb, 0, &js);
    assert!(r.erro.is_none(), "{:?}\n{js}", r.erro);
    assert_eq!(r.escolha, Some(feathersheet_macros::Escolha { botoes: vec![3, 4, 5], icone: 16, padrao: 1 }));
    assert_eq!(r.pergunta.as_deref(), Some("Erro\n\nArquivo em uso"));
    let r = feathersheet_macros::executar_com(&mut wb, 0, &js, feathersheet_macros::Contexto { escolhas: vec![2], ..Default::default() });
    assert!(r.erro.is_none() && r.pergunta.is_none(), "{r:?}");
    assert_eq!(wb.valor(0, 0, 0), Value::Number(5.0), "vbIgnore");
}

#[test]
fn cverr_xlerrnull_da_null() {
    let wb = roda("Sub T()\n  Range(\"A1\").Value = IsError(CVErr(xlErrNull))\n  Range(\"A2\").Value = CVErr(xlErrNull)\n  Range(\"A3\").Value = CVErr(xlErrNA)\nEnd Sub\n", "T");
    assert_eq!(wb.valor(0, 0, 0), Value::Bool(true), "#NULL! é erro de verdade dentro da macro");
    // Erro constante numa célula fica como o texto do erro (é como o .xlsx com #N/A digitado abre).
    assert_eq!(wb.valor(0, 1, 0), Value::Text("#NULL!".into()));
    assert_eq!(wb.valor(0, 2, 0), Value::Text("#N/A".into()));
}

#[test]
fn parenteses_passam_por_valor_e_colecao_do_modulo_nasce_pronta() {
    let wb = roda("Dim gc As New Collection\nSub T()\n  Dim a As Long, b As Long\n  a = 1: b = 1\n  Dobrar (a)\n  Call Dobrar(b)\n  Range(\"A1\").Value = a\n  Range(\"A2\").Value = b\n  gc.Add 5\n  gc.Add 7\n  Range(\"A3\").Value = gc.Count\nEnd Sub\nSub Dobrar(n As Long)\n  n = n * 2\nEnd Sub\n", "T");
    assert_eq!(wb.valor(0, 0, 0), Value::Number(1.0), "Dobrar (a): por valor");
    assert_eq!(wb.valor(0, 1, 0), Value::Number(2.0), "Call Dobrar(b): por referência");
    assert_eq!(wb.valor(0, 2, 0), Value::Number(2.0));
}

#[test]
fn instrucao_mid_troca_no_lugar() {
    let wb = roda("Sub T()\n  Dim s As String\n  s = \"abcdef\"\n  Mid(s, 2, 2) = \"XYZ\"\n  Range(\"A1\").Value = s\n  Mid(s, 5) = \"123456\"\n  Range(\"A2\").Value = s\nEnd Sub\n", "T");
    assert_eq!(wb.valor(0, 0, 0), Value::Text("aXYdef".into()));
    assert_eq!(wb.valor(0, 1, 0), Value::Text("aXYd12".into()), "não passa do fim");
}

#[test]
fn dicionario_sem_maiusculas_e_troca_de_chave() {
    let wb = roda("Sub T()\n  Dim d As Object, k As Variant, s As String\n  Set d = CreateObject(\"Scripting.Dictionary\")\n  d.CompareMode = vbTextCompare\n  d.Add \"Ana\", 1\n  d(\"ANA\") = d(\"ana\") + 1\n  d.Add \"Bia\", 5\n  d.Key(\"bia\") = \"Beatriz\"\n  For Each k In d.Keys\n    s = s & k & \"=\" & d(k) & \";\"\n  Next k\n  Range(\"A1\").Value = s\n  Range(\"A2\").Value = d.Count\nEnd Sub\n", "T");
    assert_eq!(wb.valor(0, 0, 0), Value::Text("Ana=2;Beatriz=5;".into()));
    assert_eq!(wb.valor(0, 1, 0), Value::Number(2.0));
}

#[test]
fn posicao_da_forma_em_pontos() {
    let c = converter_texto(&[("Módulo1", "Sub T()\n  With ActiveSheet.Shapes(\"Botão 1\")\n    Range(\"A1\").Value = .Left: Range(\"B1\").Value = .Top: Range(\"C1\").Value = .Width: Range(\"D1\").Value = .Height\n    .Left = 10\n    Range(\"A2\").Value = .Left\n  End With\nEnd Sub\n")], &[]);
    assert!(c.pendencias.is_empty(), "{:#?}", c.pendencias);
    let mut wb = Workbook::new();
    // Coluna A com 96 px (72 pt); a forma vai de B2 a C4.
    wb.editar(0, |s| { s.col_widths.insert(0, 96); s.alterar_objetos(Some(r##"{"v":1,"objetos":[{"id":1,"forma":"rect","de":{"linha":1,"coluna":1},"ate":{"linha":3,"coluna":2},"nome":"Botão 1"}]}"##.into())); });
    let r = feathersheet_macros::executar(&mut wb, 0, &c.macros[0].codigo);
    assert!(r.erro.is_none(), "{:?}", r.erro);
    let lidos: Vec<_> = (0..4).map(|c| wb.valor(0, 0, c)).collect();
    assert_eq!(lidos, [72.0, 21.0, 91.5, 42.0].map(Value::Number), "pontos: 122 px = 91,5 pt; 28 px = 21 pt");
    assert_eq!(wb.valor(0, 1, 0), Value::Number(10.0), "o que a macro mudou vale na leitura seguinte");
}

#[test]
fn application_inputbox_converte() {
    let vba = "Sub T()\n  Dim n As Variant, r As Range\n  n = Application.InputBox(\"Quantos?\", \"Contagem\", 5, Type:=1)\n  If n = False Then Exit Sub\n  Range(\"A1\").Value = n * 2\n  Set r = Application.InputBox(\"Onde?\", Default:=\"$B$2:$B$3\", Type:=8)\n  r.Value = \"aqui\"\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    assert!(c.pendencias.is_empty(), "{:#?}", c.pendencias);
    let js = &c.macros[0].codigo;
    assert!(js.contains("vba.inputBoxApp(\"Quantos?\", \"Contagem\", 5, 1)") && js.contains("vba.inputBoxApp(\"Onde?\", undefined, \"$B$2:$B$3\", 8)"), "{js}");
}


#[test]
fn numero_vira_texto_como_o_cstr() {
    let wb = roda("Sub T()\n  Dim x As Double, i As Long\n  x = 0.1 + 0.2\n  i = 7\n  Range(\"A1\").Value = \"Total: \" & x\n  Range(\"A2\").Value = \"x\" & CStr(1 / 3)\n  Range(\"A3\").Value = \"L\" & i & \"/\" & 1E+15\n  Range(\"A4\").Value = \"x\" & CStr(-0.0000001)\n  Range(\"A5\").Value = Len(2.5)\nEnd Sub\n", "T");
    let t = |s: &str| Value::Text(s.into());
    assert_eq!(wb.valor(0, 0, 0), t("Total: 0,3"));
    assert_eq!(wb.valor(0, 1, 0), t("x0,333333333333333"));
    assert_eq!(wb.valor(0, 2, 0), t("L7/1E+15"));
    assert_eq!(wb.valor(0, 3, 0), t("x-1E-07"));
    assert_eq!(wb.valor(0, 4, 0), Value::Number(3.0));
}
