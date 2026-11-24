Sub Conferir(caminho As String, saida As String, nome_macro As String)
  Dim n As Integer : n = FreeFile
  Open saida For Output As #n
  On Error GoTo Falhou
  Dim args(1) As New com.sun.star.beans.PropertyValue
  args(0).Name = "MacroExecutionMode" : args(0).Value = 4
  args(1).Name = "Hidden" : args(1).Value = True
  Dim oDoc As Object
  oDoc = StarDesktop.loadComponentFromURL(ConvertToURL(caminho), "_blank", 0, args)
  Print #n, "aberto"
  Dim biblios As Variant, biblio As Variant, mods As Variant, modulo As Variant, alvo As String, codigo As String
  biblios = oDoc.BasicLibraries.ElementNames
  For Each biblio In biblios
    oDoc.BasicLibraries.loadLibrary(biblio)
    mods = oDoc.BasicLibraries.getByName(biblio).ElementNames
    For Each modulo In mods
      codigo = oDoc.BasicLibraries.getByName(biblio).getByName(modulo)
      Print #n, "modulo " & biblio & "." & modulo & " (" & Len(codigo) & " letras)"
      If InStr(codigo, "Sub " & nome_macro & "(") > 0 Then alvo = biblio & "." & modulo
    Next
  Next
  If alvo <> "" Then
    Print #n, "rodando " & alvo & "." & nome_macro
    Dim sc As Object
    sc = oDoc.getScriptProvider().getScript("vnd.sun.star.script:" & alvo & "." & nome_macro & "?language=Basic&location=document")
    Dim a1(), a2(), a3()
    sc.invoke(a1, a2, a3)
  End If
  Dim folha As Object : folha = oDoc.Sheets.getByIndex(0)
  folha.getCellRangeByName("C1").setValue(7)
  Dim c As Variant, cel As Object
  For Each c In Array("A1","A2","A3","B1","B2","B3","B4","B5","B6","D1","Z1","Z2")
    cel = folha.getCellRangeByName(c)
    Print #n, c & " | " & cel.getString() & " | " & cel.getFormula()
  Next
  oDoc.close(True)
  Print #n, "fim"
  Close #n
  Exit Sub
Falhou:
  Print #n, "ERRO " & Err & " linha " & Erl & ": " & Error$
  On Error Resume Next
  oDoc.close(True)
  Close #n
End Sub
