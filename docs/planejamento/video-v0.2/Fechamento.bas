Attribute VB_Name = "Fechamento"
Option Explicit

' Fechamento do mês: limpa a exportação do sistema, calcula total e comissão,
' marca metas e descontos fora da regra e monta o relatório por vendedor e região.
Sub FechamentoDoMes()
    Dim inicio As Double
    inicio = Timer
    Application.ScreenUpdating = False

    Dim ws As Worksheet
    Set ws = Worksheets("Vendas")

    ' 1. Tira os pedidos repetidos da exportação
    Dim ultima As Long
    ultima = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row
    ws.Range("A1:I" & ultima).RemoveDuplicates Columns:=1, Header:=xlYes
    ultima = ws.Cells(ws.Rows.Count, 1).End(xlUp).Row

    ' 2. Lê tudo de uma vez
    Dim dados As Variant
    dados = ws.Range("A2:I" & ultima).Value
    Dim n As Long
    n = UBound(dados, 1)

    Dim saida() As Variant
    ReDim saida(1 To n, 1 To 5)

    ' 3. Limpa e calcula linha a linha
    Dim i As Long, total As Double, comissao As Double, d As Date
    For i = 1 To n
        dados(i, 3) = StrConv(Trim(dados(i, 3)), vbProperCase)
        dados(i, 4) = UCase(Trim(dados(i, 4)))
        d = DateValue(dados(i, 2))
        dados(i, 2) = d

        total = dados(i, 7) * dados(i, 8) * (1 - dados(i, 9))
        Select Case total
            Case Is >= 10000
                comissao = total * 0.05
            Case Is >= 5000
                comissao = total * 0.03
            Case Else
                comissao = total * 0.01
        End Select

        saida(i, 1) = Format(d, "mm/yyyy")
        saida(i, 2) = Round(total, 2)
        saida(i, 3) = Round(comissao, 2)
        If dados(i, 9) > 0.15 Then
            saida(i, 4) = "Desconto acima"
        ElseIf total >= 10000 Then
            saida(i, 4) = "Meta batida"
        Else
            saida(i, 4) = ""
        End If
        saida(i, 5) = Weekday(d)
    Next i

    ws.Range("A2:I" & ultima).Value = dados
    ws.Range("J1:N1").Value = Array("Mês", "Total", "Comissão", "Situação", "Dia da semana")
    ws.Range("J2:N" & ultima).Value = saida
    ws.Range("B2:B" & ultima).NumberFormat = "dd/mm/yyyy"
    ws.Range("H2:H" & ultima & ",K2:L" & ultima).NumberFormat = "#,##0.00"
    ws.Range("I2:I" & ultima).NumberFormat = "0%"

    ' 4. Destaca meta batida (verde) e desconto fora da regra (vermelho)
    With ws.Range("M2:M" & ultima).FormatConditions
        .Delete
        With .Add(Type:=xlCellValue, Operator:=xlEqual, Formula1:="=""Meta batida""")
            .Interior.Color = RGB(198, 239, 206)
            .Font.Color = RGB(0, 97, 0)
        End With
        With .Add(Type:=xlCellValue, Operator:=xlEqual, Formula1:="=""Desconto acima""")
            .Interior.Color = RGB(255, 199, 206)
            .Font.Color = RGB(156, 0, 6)
        End With
    End With

    ' 5. Ordena por vendedor para somar em uma passada
    With ws.Sort
        .SortFields.Clear
        .SortFields.Add Key:=ws.Range("C2:C" & ultima), Order:=xlAscending
        .SetRange ws.Range("A1:N" & ultima)
        .Header = xlYes
        .Apply
    End With
    dados = ws.Range("A2:N" & ultima).Value

    ' 6. Ranking dos vendedores
    Dim rel As Worksheet
    Application.DisplayAlerts = False
    On Error Resume Next
    Worksheets("Fechamento").Delete
    On Error GoTo 0
    Application.DisplayAlerts = True
    Set rel = Worksheets.Add(After:=ws)
    rel.Name = "Fechamento"
    rel.Range("A1").Value = "Fechamento do mês"
    rel.Range("A1").Font.Bold = True
    rel.Range("A1").Font.Size = 16
    rel.Range("A3:F3").Value = Array("Vendedor", "Região", "Vendas", "Total", "Ticket médio", "Comissão")
    rel.Range("A3:F3").Font.Bold = True
    rel.Range("A3:F3").Interior.Color = RGB(31, 138, 76)
    rel.Range("A3:F3").Font.Color = RGB(255, 255, 255)

    Dim ranking() As Variant
    ReDim ranking(1 To 200, 1 To 6)
    Dim k As Long, atual As String, qtd As Long, soma As Double, somaCom As Double
    k = 0
    atual = ""
    For i = 1 To n
        If dados(i, 3) <> atual Then
            If atual <> "" Then
                k = k + 1
                ranking(k, 1) = atual
                ranking(k, 2) = dados(i - 1, 4)
                ranking(k, 3) = qtd
                ranking(k, 4) = Round(soma, 2)
                ranking(k, 5) = Round(soma / qtd, 2)
                ranking(k, 6) = Round(somaCom, 2)
            End If
            atual = dados(i, 3)
            qtd = 0
            soma = 0
            somaCom = 0
        End If
        qtd = qtd + 1
        soma = soma + dados(i, 11)
        somaCom = somaCom + dados(i, 12)
    Next i
    k = k + 1
    ranking(k, 1) = atual
    ranking(k, 2) = dados(n, 4)
    ranking(k, 3) = qtd
    ranking(k, 4) = Round(soma, 2)
    ranking(k, 5) = Round(soma / qtd, 2)
    ranking(k, 6) = Round(somaCom, 2)

    rel.Range("A4").Resize(k, 6).Value = ranking
    rel.Range("D4:F" & k + 3).NumberFormat = "#,##0.00"
    With rel.Sort
        .SortFields.Clear
        .SortFields.Add Key:=rel.Range("D4:D" & k + 3), Order:=xlDescending
        .SetRange rel.Range("A3:F" & k + 3)
        .Header = xlYes
        .Apply
    End With

    ' 7. Total por região
    Dim regioes As Variant
    regioes = Array("NORTE", "NORDESTE", "CENTRO-OESTE", "SUDESTE", "SUL")
    Dim r As Long, linha As Long
    linha = k + 6
    rel.Range("A" & linha).Value = "Total por região"
    rel.Range("A" & linha).Font.Bold = True
    For r = 0 To UBound(regioes)
        linha = linha + 1
        rel.Range("A" & linha).Value = regioes(r)
        rel.Range("B" & linha).Formula = "=SUMIF(Vendas!D:D,A" & linha & ",Vendas!K:K)"
        rel.Range("B" & linha).NumberFormat = "#,##0.00"
    Next r
    rel.Columns("A:F").AutoFit

    ' 8. Uma aba por região, para mandar a cada gerente
    Dim aba As Worksheet, parte() As Variant, j As Long, c As Long
    For r = 0 To UBound(regioes)
        Application.DisplayAlerts = False
        On Error Resume Next
        Worksheets(regioes(r)).Delete
        On Error GoTo 0
        Application.DisplayAlerts = True
        Set aba = Worksheets.Add(After:=Worksheets(Worksheets.Count))
        aba.Name = regioes(r)
        ReDim parte(1 To n, 1 To 14)
        j = 0
        For i = 1 To n
            If dados(i, 4) = regioes(r) Then
                j = j + 1
                For c = 1 To 14
                    parte(j, c) = dados(i, c)
                Next c
            End If
        Next i
        ws.Range("A1:N1").Copy aba.Range("A1")
        If j > 0 Then aba.Range("A2").Resize(j, 14).Value = parte
        aba.Range("B2:B" & j + 1).NumberFormat = "dd/mm/yyyy"
        aba.Columns("A:N").AutoFit
    Next r

    rel.Activate
    Application.ScreenUpdating = True
    MsgBox Format(n, "#,##0") & " vendas processadas em " & Format(Timer - inicio, "0.0") & " s", vbInformation, "Fechamento do mês"
End Sub
