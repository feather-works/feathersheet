//! O conversor por fora: VBA no padrão das planilhas reais (botão que valida CPF, evento de
//! mudança, formatação, laços) → o JavaScript esperado. O `.xlsm` do usuário que guiou isto
//! não entra no repositório (é dele); os trechos aqui imitam o que ele tinha.

use feathersheet_vba::converter_texto;

fn macro_(vba: &str, nome: &str) -> String {
    let c = converter_texto(&[("Módulo1", vba)], &[("Sheet8", "MAIN")]);
    c.macros.iter().find(|m| m.nome == nome).unwrap_or_else(|| panic!("sem a macro {nome}: {:#?}", c.macros.iter().map(|m| &m.nome).collect::<Vec<_>>())).codigo.clone()
}

#[test]
fn celulas_fixas_viram_endereco_e_abas_viram_constante() {
    let js = macro_("Sub Preencher()\n  Sheet8.Cells(3, 2) = \"x\"\n  Range(\"A3\").Value = 5\n  Worksheets(\"MAIN\").Range(\"B10\").Value = Cells(1, \"H\")\nEnd Sub\n", "Preencher");
    assert!(js.contains("const Sheet8 = wb.sheet(\"MAIN\");"), "{js}");
    assert!(js.contains("Sheet8.set(\"B3\", \"x\");"), "{js}");
    assert!(js.contains("sheet.set(\"A3\", 5);"), "{js}");
    assert!(js.contains("Sheet8.set(\"B10\", sheet.value(\"H1\"));"), "{js}");
    assert!(js.trim_end().ends_with("Preencher();"));
}

#[test]
fn laco_com_celula_calculada_e_texto_concatenado() {
    let js = macro_("Sub Laco()\n  Dim i As Long\n  For i = 2 To 10\n    Cells(i, 3).Value = \"Linha \" & i\n  Next i\nEnd Sub\n", "Laco");
    assert!(js.contains("for (i = 2; i <= 10; i++) {"), "{js}");
    assert!(js.contains("sheet.set(`C${i}`, `Linha ${i}`);"), "{js}");
    assert!(js.contains("let i = 0;"));
}

#[test]
fn variant_compara_e_soma_como_o_vba() {
    let js = macro_("Sub Comparar()\n  If Range(\"B19\") = \"NÃO\" Then x = x + 1\nEnd Sub\n", "Comparar");
    assert!(js.contains("if (vba.eq(sheet.value(\"B19\"), \"NÃO\")) {"), "{js}");
    assert!(js.contains("x = vba.soma(x, 1);"), "{js}");
    assert!(js.contains("let x = undefined;"), "variável implícita declarada: {js}");
    assert!(js.contains("eq(a, b)") && js.contains("cmp(a, b)"), "a ajuda vai junto: {js}");
}

#[test]
fn funcao_retorna_pelo_nome_e_mod_arredonda() {
    let vba = "Public Function Dobro(ByVal v As Double) As Long\n  Dobro = v * 2\n  If v Mod 2 = 0 Then Exit Function\nEnd Function\nSub Usar()\n  MsgBox Dobro(3)\nEnd Sub\n";
    let js = macro_(vba, "Usar");
    assert!(js.contains("function Dobro(v) {"), "{js}");
    assert!(js.contains("$r = vba.cint(v * 2);"), "retorno Long arredonda: {js}");
    assert!(js.contains("vba.mod(v, 2) === 0"), "{js}");
    assert!(js.contains("return $r;"));
    assert!(js.contains("vba.msgBox(Dobro(3));"), "{js}");
    let funcoes = macro_(vba, "Funções do VBA");
    assert!(funcoes.contains("sheet.defineFunction(\"DOBRO\", (...a) => Dobro(...a));"), "{funcoes}");
}

#[test]
fn evento_de_mudanca_com_intersect() {
    let c = converter_texto(&[("Sheet8", "Private Sub Worksheet_Change(ByVal Target As Range)\n  If Intersect(Target, Range(\"B3\")) Is Nothing Then Exit Sub\n  Application.EnableEvents = False\n  Range(\"A3\") = \"ok\"\n  Application.EnableEvents = True\nEnd Sub\n")], &[("Sheet8", "MAIN")]);
    let e = &c.eventos[0];
    assert_eq!((e.aba.as_deref(), e.evento), (Some("MAIN"), "aoMudar"));
    assert!(e.codigo.contains("if (Sheet8.intersect(Target, \"B3\") === null) {"), "{}", e.codigo);
    assert!(e.codigo.contains("app.setEventsEnabled(false);"));
    assert!(e.codigo.contains("Sheet8.set(\"A3\", \"ok\");"), "no módulo da aba, Range é a aba: {}", e.codigo);
    assert!(e.codigo.trim_end().ends_with("Worksheet_Change(valores.intervalo || Sheet8.selection().address);"), "{}", e.codigo);
}

#[test]
fn formatacao_bordas_e_with() {
    let js = macro_(
        "Sub Formatar()\n  With Range(\"A1:C1\")\n    .Font.Bold = True\n    .Interior.Color = RGB(169, 208, 142)\n    .HorizontalAlignment = xlCenter\n    .Borders(xlEdgeBottom).LineStyle = xlContinuous\n    .Interior.Color = xlNone\n  End With\n  Selection.Borders.LineStyle = xlNone\nEnd Sub\n",
        "Formatar",
    );
    for esperado in [
        "sheet.setBold(\"A1:C1\", true);",
        "sheet.setBackground(\"A1:C1\", app.rgb(169, 208, 142));",
        "sheet.setAlignHorizontal(\"A1:C1\", 'center');",
        "sheet.setBorder(\"A1:C1\", { bottom: true });",
        "vba.semFundo(sheet, \"A1:C1\");",
        "sheet.setBorder(sheet.selection().address, { none: true });",
    ] {
        assert!(js.contains(esperado), "faltou {esperado}:\n{js}");
    }
}

#[test]
fn ultima_linha_usa_getlastrow() {
    let js = macro_("Sub Ultima()\n  n = Cells(Rows.Count, 1).End(xlUp).Row\n  m = Range(\"B1\").End(xlDown).Row\nEnd Sub\n", "Ultima");
    assert!(js.contains("sheet.getLastRow(\"A\")"), "{js}");
    assert!(js.contains("vba.fim(sheet, \"B1\", -4121)"), "{js}");
}

#[test]
fn o_que_nao_converte_vira_comentario_e_pendencia() {
    let vba = "Sub Gerar()\n  Dim Word As Object\n  Set Word = CreateObject(\"Word.Application\")\n  Word.Visible = True\n  Range(\"A1\") = 1\n  Kill \"c:\\x.docx\"\nEnd Sub\n";
    let c = converter_texto(&[("Módulo1", vba)], &[]);
    let js = &c.macros[0].codigo;
    assert!(js.contains("// VBA não convertido (automação de outro programa (CreateObject)):"), "{js}");
    assert!(js.contains("//   Set Word = CreateObject(\"Word.Application\")"));
    assert!(js.contains("sheet.set(\"A1\", 1);"), "o resto continua: {js}");
    let motivos: Vec<&str> = c.pendencias.iter().map(|p| p.motivo.as_str()).collect();
    assert_eq!(motivos, ["automação de outro programa (CreateObject)", "automação de outro programa (CreateObject)", "acesso a arquivo"], "{:#?}", c.pendencias);
}

#[test]
fn macro_leva_os_procedimentos_que_chama_de_outro_modulo() {
    let c = converter_texto(&[("Módulo1", "Sub Botao()\n  If Valida(\"1\") Then MsgBox \"ok\"\nEnd Sub\n"), ("Módulo2", "Public Function Valida(s As String) As Boolean\n  Valida = Len(s) > 0\nEnd Function\n")], &[]);
    let js = &c.macros.iter().find(|m| m.nome == "Botao").unwrap().codigo;
    assert!(js.contains("function Botao()") && js.contains("function Valida(s)"), "{js}");
    assert!(js.contains("if (Valida(\"1\")) {"), "Boolean não passa por vba.bool: {js}");
}

#[test]
fn select_case_exit_for_aninhado_e_nomes_reservados() {
    let js = macro_(
        "Sub Casos()\n  Dim new_ As Long\n  For i = 1 To 3\n    Do While True\n      Exit For\n    Loop\n  Next\n  Select Case i\n    Case 1, 2\n      x = 1\n    Case 3 To 5\n      x = 2\n    Case Else\n      x = 3\n  End Select\n  Dim delete\n  delete = 1\nEnd Sub\n",
        "Casos",
    );
    assert!(js.contains("L") && js.contains(": for (i = 1;") && js.contains("break L"), "Exit For de dentro do Do: {js}");
    assert!(js.contains("const $caso"), "{js}");
    assert!(js.contains("delete_ = 1;"), "nome reservado do JS ganha _: {js}");
}

#[test]
fn botoes_do_excel_sao_listados() {
    let c = converter_texto(&[("EstaPasta_de_trabalho", "Private Sub Workbook_Open()\n  ActiveSheet.Shapes.Range(Array(\"Shape 3\")).Select\n  Selection.OnAction = \"Gerar\"\nEnd Sub\n")], &[]);
    assert_eq!((c.botoes[0].forma.as_str(), c.botoes[0].macro_.as_str()), ("Shape 3", "Gerar"));
    assert_eq!(c.eventos[0].evento, "aoAbrir");
}

#[test]
fn on_error_goto_vira_try_catch_e_goto_para_frente_vira_break() {
    let js = macro_(concat!(
        "Sub Salvar()\n",
        "  Dim x As Long\n",
        "  On Error GoTo TrataErro\n",
        "  x = 1\n",
        "  If x = 1 Then GoTo Fim\n",
        "  x = 2\n",
        "Fim:\n",
        "  x = 3\n",
        "  Exit Sub\n",
        "TrataErro:\n",
        "  MsgBox \"Falhou: \" & Err.Description\n",
        "End Sub\n"), "Salvar");
    assert!(!js.contains("// VBA não convertido ("), "{js}");
    assert!(js.contains("try {") && js.contains("} catch ($erro) {") && js.contains("vba.erro($erro);"), "{js}");
    assert!(js.contains("rotulo_fim: {") && js.contains("break rotulo_fim;"), "{js}");
    assert!(js.contains("vba.erroTexto()"), "{js}");
    // Rótulo que ninguém chama some sem pendência; GoTo para trás continua pendente.
    let solto = macro_("Sub A()\nInicio:\n  x = 1\nEnd Sub\n", "A");
    assert!(!solto.contains("// VBA não convertido ("), "{solto}");
    let tras = macro_("Sub B()\nVolta:\n  x = 1\n  GoTo Volta\nEnd Sub\n", "B");
    assert!(tras.contains("GoTo para trás"), "{tras}");
}

/// O `salvar_enviar_PDF` da planilha real: PDF de uma aba e uma aba salva num `.xlsx` novo.
fn modulo5(vba: &str) -> (String, Vec<feathersheet_vba::gerar::Pendencia>) {
    let c = converter_texto(&[("Módulo5", vba)], &[("Sheet8", "MAIN"), ("Sheet7", "6 - AUTOCONSUMO REMOTO")]);
    let m = c.macros.iter().find(|m| m.nome == "Gerar").expect("macro Gerar");
    (m.codigo.clone(), c.pendencias.clone())
}

#[test]
fn exporta_pdf_da_aba() {
    let (js, pend) = modulo5("Sub Gerar()\n  strDir = ThisWorkbook.Path\n  Sheet8.ExportAsFixedFormat Type:=xlTypePDF, Filename:=(strDir & \"\\\" & \"01-FORMULARIO2.pdf\")\n  ActiveSheet.ExportAsFixedFormat xlTypePDF, \"b.pdf\"\nEnd Sub\n");
    assert!(js.contains("strDir = wb.path;"), "{js}");
    assert!(js.contains("Sheet8.exportPdf(vba.cat(strDir, \"\\\\\", \"01-FORMULARIO2.pdf\"));") || js.contains("Sheet8.exportPdf("), "{js}");
    assert!(js.contains("sheet.exportPdf(\"b.pdf\");"), "{js}");
    assert!(pend.iter().all(|p| p.aviso), "{pend:#?}");
    let (_, pend) = modulo5("Sub Gerar()\n  Sheet8.ExportAsFixedFormat Type:=xlTypeXPS, Filename:=\"a.xps\"\nEnd Sub\n");
    assert!(pend.iter().any(|p| p.motivo.contains("XPS")), "{pend:#?}");
}

#[test]
fn copia_a_aba_para_pasta_nova_e_salva() {
    let vba = "Sub Gerar()\n  strDir = ThisWorkbook.Path\n  Set newBook = Application.Workbooks.Add\n  Sheet7.Copy Before:=newBook.Sheets(1)\n  For i = 2 To newBook.Worksheets.Count\n    newBook.Worksheets(2).Delete\n  Next i\n  newBook.SaveAs (strDir & \"\\\" & \"09-UNIDADES_PARTICIPANTES\" & \".xlsx\")\n  newBook.Close\n  Workbooks.Open Filename:=\"C:\\x.xlsm\"\nEnd Sub\n";
    let (js, pend) = modulo5(vba);
    assert!(js.contains("newBook = vba.novaPasta();"), "{js}");
    assert!(js.contains("newBook.abas.splice(1 - 1, 0, \"6 - AUTOCONSUMO REMOTO\");"), "{js}");
    assert!(js.contains("newBook.abas.length"), "{js}");
    assert!(js.contains("newBook.abas.splice(2 - 1, 1);"), "{js}");
    assert!(js.contains("wb.saveSheetsAs(newBook.abas.filter((a) => a !== null), "), "{js}");
    assert!(js.contains("novaPasta() { return { abas: [null] }; }"), "{js}");
    // Abrir outra pasta continua pendente.
    let abrir: Vec<_> = pend.iter().filter(|p| !p.aviso).collect();
    assert_eq!(abrir.len(), 1, "{pend:#?}");
    assert!(abrir[0].motivo.contains("Workbooks.Open"), "{pend:#?}");
}

#[test]
fn workbook_declarado_e_copia_depois() {
    let (js, pend) = modulo5("Sub Gerar()\n  Dim nb As Workbook\n  Set nb = Workbooks.Add\n  Sheet8.Copy After:=nb.Sheets(1)\n  nb.SaveAs Filename:=\"saida.fshz\"\nEnd Sub\n");
    assert!(js.contains("nb.abas.splice(1, 0, \"MAIN\");"), "{js}");
    assert!(js.contains("wb.saveSheetsAs(nb.abas.filter((a) => a !== null), \"saida.fshz\");"), "{js}");
    assert!(pend.iter().all(|p| p.aviso), "{pend:#?}");
}

#[test]
fn texto_para_colunas_gravado_pelo_excel() {
    // O formato do gravador de macros (precioso.xlsm, Macro3).
    let vba = "Sub Separar()\n  num = 2\n  Range(\"A\" & num).Select\n  Selection.TextToColumns Destination:=Cells(num, 1), DataType:=xlDelimited, _\n    TextQualifier:=xlDoubleQuote, ConsecutiveDelimiter:=False, Tab:=True, _\n    Semicolon:=False, Comma:=False, Space:=False, Other:=True, OtherChar _\n    :=\"-\", FieldInfo:=Array(Array(1, 1), Array(2, 1)), TrailingMinusNumbers:=True\n  Columns(\"B:B\").TextToColumns DataType:=xlFixedWidth\nEnd Sub\n";
    let js = macro_(vba, "Separar");
    assert!(js.contains(".textToColumns("), "{js}");
    assert!(js.contains("delimiters: ['\\t', String(\"-\").charAt(0)]"), "{js}");
    assert!(!js.contains("VBA não convertido (Range.texttocolumns)"), "{js}");
    assert!(js.contains("largura fixa"), "largura fixa fica marcada: {js}");
}

/// #129: VBA montado para estourar a pilha vira "não convertido", sem derrubar o app.
#[test]
fn vba_aninhado_demais_nao_derruba_e_o_normal_continua() {
    let n = 50_000;
    let soma = format!("x = 1{}", " + 1".repeat(n));
    let menos = format!("x = {}1", "-".repeat(n));
    let nao = format!("x = {}True", "Not ".repeat(n));
    let parenteses = format!("x = {}1{}", "(".repeat(n), ")".repeat(n));
    let membros = format!("x = a{}", ".b".repeat(n));
    let blocos = format!("{}x = 1\n{}", "If True Then\n".repeat(5_000), "End If\n".repeat(5_000));
    for corpo in [soma, menos, nao, parenteses, membros, blocos] {
        let vba = format!("Sub Bomba()\n{corpo}\nEnd Sub\nSub Normal()\n  Range(\"A1\").Value = 1 + 2\nEnd Sub\n");
        // Numa thread de 2 MB, como as de trabalho: com recursão sem teto, estoura.
        let js = std::thread::Builder::new().stack_size(2 << 20)
            .spawn(move || converter_texto(&[("Módulo1", &vba)], &[])).unwrap().join()
            .expect("a conversão estourou a pilha");
        let normal = js.macros.iter().find(|m| m.nome == "Normal").expect("Normal").codigo.clone();
        assert!(normal.contains("sheet.set(\"A1\", 1 + 2);"), "{normal}");
        assert!(!js.pendencias.is_empty(), "o excesso deve virar pendência");
    }
}
