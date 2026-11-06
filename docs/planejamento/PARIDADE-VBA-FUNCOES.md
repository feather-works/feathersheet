# Paridade: funções do VBA × conversor VBA→JS do FeatherSheet

## Resumo

Atualizado em 02/11/2026 (a primeira avaliação foi em 15/10/2026, antes da v0.2.0).

- Total de funções avaliadas: **141**
- COBERTA: **116**
- PARCIAL: **1**
- FALTA: **0**
- NÃO SE APLICA: **24**

(Sem contar as que não se aplicam, 116 de 117 cobertas; a única parcial é o CDec, que vira número de 64 bits em vez do Decimal de 28 algarismos.)

Lista oficial: página "Functions (Visual Basic for Applications)" da Microsoft Learn (versão de 2024-02). Tipos de conversão foram incluídos pela família (CBool…CVDate), mesmo fora da página principal. Código conferido em `feathersheet/crates/feathersheet-vba/src/gerar/` (funcoes.rs, expr.rs, instr.rs) e no prelúdio `feathersheet-vba/src/gerar/ajuda.rs` (o bloco `const vba = {…}` que vai junto com o código gerado).

Como o conversor trata função desconhecida: `expr.rs:150` devolve o motivo "função X sem equivalente" (o VBA não é convertido e a pendência aparece no relatório). Funções recusadas de propósito (arquivo, Windows, automação) caem em `embutida_outras` (`funcoes.rs`). Os números de linha da tabela são da avaliação de 15/10 e podem ter andado.

Legenda: COBERTA = convertida e usada no código gerado. PARCIAL = convertida com argumentos ou comportamento faltando. FALTA = não convertida (gera pendência). NÃO SE APLICA = depende de Windows, Office, arquivo ou impressora; continua recusada.

| função | categoria | estado | onde/observação |
|---|---|---|---|
| Asc | conversão | COBERTA | funcoes.rs:133 (`charCodeAt(0)`); só o primeiro caractere |
| Chr | conversão | COBERTA | funcoes.rs:132 (`String.fromCharCode`) |
| CVErr | conversão | COBERTA | `vba.cverr` → `app.error`; `#NULL!` entrou no motor (02/11/2026). Erro constante gravado numa célula fica como o texto do erro, como o `.xlsx` com erro digitado |
| Format | conversão | COBERTA | ajuda.rs (`format`, `_formatoNomeado`): máscaras e todos os formatos com nome (General Number, Currency, Fixed, Standard, Percent, Scientific, Yes/No, True/False, On/Off, as datas e horas com nome) |
| Hex | conversão | COBERTA | `vba.hex(v, 16)`, negativo em complemento de dois |
| Oct | conversão | COBERTA | `vba.hex(v, 8)` |
| Str | conversão | COBERTA | `vba.strNum`: espaço do sinal em número não negativo |
| Val | conversão | COBERTA | ajuda.rs:29, funcoes.rs:142 |
| Abs | matemática | COBERTA | funcoes.rs:158 |
| Atn | matemática | COBERTA | funcoes.rs:163 (`Math.atan`) |
| Cos | matemática | COBERTA | funcoes.rs:163 |
| Derived math (Sec, Csc, Cot, ArcSin…) | matemática | NÃO SE APLICA | não são funções do VBA: a página da Microsoft só ensina a conta com Sin/Cos/Atn, que já convertem |
| Exp | matemática | COBERTA | funcoes.rs:161 |
| Int | matemática | COBERTA | funcoes.rs:156 (`Math.floor`, certo para negativos) |
| Fix | matemática | COBERTA | funcoes.rs:157 (`Math.trunc`) |
| Log | matemática | COBERTA | funcoes.rs:162 |
| Rnd | matemática | COBERTA | `vba.rnd`/`vba.randomize`: o gerador do VBA (24 bits, semente 327680): a mesma sequência do Excel, Rnd(0), Rnd(negativo) e Randomize |
| Sgn | matemática | COBERTA | funcoes.rs:160 |
| Sin | matemática | COBERTA | funcoes.rs:163 |
| Sqr | matemática | COBERTA | funcoes.rs:159 |
| Tan | matemática | COBERTA | funcoes.rs:163 |
| CBool | conversão de tipo | COBERTA | funcoes.rs:152-155 (`como_bool`) |
| CByte | conversão de tipo | COBERTA | `vba.faixaInt`, Estouro fora de 0..255 |
| CCur | conversão de tipo | COBERTA | `vba.ccur`: 4 casas com o arredondamento do banqueiro |
| CDate | conversão de tipo | COBERTA | `vba.cdate` (ajuda.rs): número, dd/mm/aaaa, aaaa-mm-dd, com hh:mm[:ss], só hh:mm |
| CDbl | conversão de tipo | COBERTA | funcoes.rs:151 |
| CDec | conversão de tipo | PARCIAL | funcoes.rs:151 (número JS). Sem precisão decimal exata |
| CInt | conversão de tipo | COBERTA | `vba.faixaInt`: arredonda para o par e dá Estouro (erro 6) fora de -32768..32767 |
| CLng | conversão de tipo | COBERTA | `vba.faixaInt`, Estouro fora de ±2^31 |
| CLngLng | conversão de tipo | COBERTA | funcoes.rs:150 |
| CLngPtr | conversão de tipo | COBERTA | mesmo caminho do CLngLng |
| CSng | conversão de tipo | COBERTA | funcoes.rs:151 |
| CStr | conversão de tipo | COBERTA | funcoes.rs:129 |
| CVar | conversão de tipo | COBERTA | o próprio valor |
| CVDate | conversão de tipo | COBERTA | `vba.cdate` |
| Array | outras | COBERTA | funcoes.rs:179 |
| CallByName | outras | COBERTA | `call_by_name` (expr.rs): com o nome fixo, vira a chamada escrita direto (VbMethod, VbGet, VbLet, VbSet); nome calculado vira pendência |
| Choose | outras | COBERTA | `vba.choose`; fora da faixa é Null |
| Command | outras | NÃO SE APLICA | linha de comando do programa; sem equivalente no app |
| CreateObject | outras | NÃO SE APLICA | funcoes.rs:185 recusa (automação de outro programa) |
| CurDir | outras | NÃO SE APLICA | funcoes.rs:187 recusa (acesso a arquivo) |
| Date | outras | COBERTA | funcoes.rs:93 (`vba.hoje()`), ajuda.rs:122 |
| DateAdd | outras | COBERTA | `vba.dateadd`: d/y/w, ww, h, n, s; m, q, yyyy param no último dia do mês |
| DateDiff | outras | COBERTA | `vba.datediff`: yyyy, q, m, d/y, w, ww, h, n, s |
| DatePart | outras | COBERTA | `vba.datepart` |
| DateSerial | outras | COBERTA | `vba.serial` (dias desde 30/12/1899, como a planilha) |
| DateValue | outras | COBERTA | `Math.floor(vba.cdate(…))` |
| Day | outras | COBERTA | funcoes.rs:168 (`vba.dia`), ajuda.rs:125 |
| DDB | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| Dir | outras | NÃO SE APLICA | funcoes.rs:187 recusa (arquivo) |
| DoEvents | outras | COBERTA | funcoes.rs:97 (no-op, `undefined`) |
| Environ | outras | NÃO SE APLICA | funcoes.rs:188 recusa (variável do Windows) |
| EOF | outras | NÃO SE APLICA | funcoes.rs:187 recusa (arquivo) |
| Error | outras | COBERTA | `vba.textoErro`: os erros comuns em pt-BR; sem número, o último erro |
| FileAttr | outras | NÃO SE APLICA | arquivo; sem equivalente |
| FileDateTime | outras | NÃO SE APLICA | funcoes.rs:187 recusa (arquivo) |
| FileLen | outras | NÃO SE APLICA | funcoes.rs:187 recusa (arquivo) |
| Filter | outras | COBERTA | `vba.filtrar`: incluir/excluir e vbTextCompare |
| FormatCurrency | outras | COBERTA | `vba.formatNumero` com "R$ ": casas, zero à esquerda, negativo entre parênteses e agrupar |
| FormatDateTime | outras | COBERTA | `vba.formatDataHora`: os cinco formatos, em pt-BR |
| FormatNumber | outras | COBERTA | `vba.formatNumero`: casas, zero à esquerda, negativo entre parênteses e agrupar |
| FormatPercent | outras | COBERTA | `vba.formatNumero` com %: casas, zero à esquerda, negativo entre parênteses e agrupar |
| FreeFile | outras | NÃO SE APLICA | funcoes.rs:187 recusa (arquivo) |
| FV | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| GetAllSettings | outras | NÃO SE APLICA | registro do Windows; sem equivalente |
| GetAttr | outras | NÃO SE APLICA | arquivo; sem equivalente |
| GetObject | outras | NÃO SE APLICA | funcoes.rs:185 recusa (automação) |
| GetSetting | outras | NÃO SE APLICA | registro do Windows; sem equivalente |
| Hour | outras | COBERTA | `vba.hora` |
| IIf | outras | COBERTA | funcoes.rs:180-182 (só com 3 argumentos; as duas ramificações são avaliadas, como no VBA) |
| IMEStatus | outras | NÃO SE APLICA | teclado japonês do Windows |
| Input | outras | NÃO SE APLICA | instr.rs:44 recusa (acesso a arquivo) |
| InputBox | outras | COBERTA | `vba.inputBox` → `prompt`, que até 06/11/2026 não existia no motor (o InputBox quebrava ao rodar): agora o app pergunta numa caixa com campo e roda a macro de novo com o texto (`Contexto::textos`). `Application.InputBox` com `Type:=` 1/2/4/8 em `vba.inputBoxApp` (Cancelar dá False; no 8, erro 424) |
| InStr | outras | COBERTA | funcoes.rs:121, ajuda.rs:22 (início e texto), comparação binária |
| InStrRev | outras | COBERTA | `vba.instrRev`: início e vbTextCompare |
| IPmt | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| IRR | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| IsArray | outras | COBERTA | funcoes.rs:141 |
| IsDate | outras | COBERTA | `vba.isDate` para texto; variável `As Date` (guarda o serial) é data |
| IsEmpty | outras | COBERTA | funcoes.rs:139 (`vba.vazio`) |
| IsError | outras | COBERTA | `app.isError` (erro de célula da API) |
| IsMissing | outras | COBERTA | `=== undefined` (Optional sem padrão) |
| IsNull | outras | COBERTA | funcoes.rs:140 (`=== null`) |
| IsNumeric | outras | COBERTA | funcoes.rs:138, ajuda.rs:28 |
| IsObject | outras | COBERTA | conversão para objetos do Excel; `vba.isObject` para o resto |
| Join | outras | COBERTA | funcoes.rs:131 (separador padrão espaço, como o VBA) |
| LBound | outras | COBERTA | `vba.lbound(m, dim)`: respeita `Dim v(1 To n)` (limite em `lb`) |
| LCase | outras | COBERTA | funcoes.rs:128 |
| Left | outras | COBERTA | funcoes.rs:119, ajuda.rs:20 |
| Len | outras | COBERTA | funcoes.rs:117, ajuda.rs:18. Conta unidades UTF-16 (emoji conta 2) |
| Loc | outras | NÃO SE APLICA | arquivo aberto (posição); sem equivalente |
| LOF | outras | NÃO SE APLICA | arquivo aberto (tamanho); sem equivalente |
| LTrim | outras | COBERTA | funcoes.rs:125, ajuda.rs:26 |
| RTrim | outras | COBERTA | funcoes.rs:126, ajuda.rs:27 |
| Trim | outras | COBERTA | funcoes.rs:124, ajuda.rs:25 |
| MacID | outras | NÃO SE APLICA | específica do Mac |
| MacScript | outras | NÃO SE APLICA | específica do Mac |
| Mid | outras | COBERTA | funcoes.rs:118, ajuda.rs:19 (começa do 1). A instrução `Mid(s, i, n) = x` em `vba.midTroca` (06/11/2026) |
| Minute | outras | COBERTA | `vba.minuto` |
| MIRR | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| Month | outras | COBERTA | funcoes.rs:167 (`vba.mes`), ajuda.rs:124 |
| MonthName | outras | COBERTA | `vba.nomeMes` (pt-BR) |
| MsgBox | outras | COBERTA | `vba.msgBox` → `app.choose`: todos os conjuntos de botões, ícones e botão padrão; devolve vbOK…vbNo |
| Now | outras | COBERTA | `vba.agora()` (serial da data e hora locais) |
| NPer | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| NPV | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| Partition | outras | COBERTA | `vba.particao`, com a largura dos números do VBA |
| Pmt | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| PPmt | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| PV | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| QBColor | outras | COBERTA | `vba.qbcolor` (16 cores) |
| Rate | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| Replace | outras | COBERTA | `vba.replace`: início (o resultado começa nele, como no VBA), quantas e vbTextCompare |
| RGB | outras | COBERTA | funcoes.rs:165 (`app.rgb`) |
| Right | outras | COBERTA | funcoes.rs:120, ajuda.rs:21 |
| Round | outras | COBERTA | funcoes.rs:164, ajuda.rs:30 (arredonda para o par no empate) |
| Second | outras | COBERTA | `vba.segundo` |
| Seek | outras | NÃO SE APLICA | arquivo aberto; instr.rs:9 e instr.rs:44 recusam |
| Shell | outras | COBERTA | `app.shell(comando)` (01/11/2026): a macro só anota; o app mostra o comando e pergunta "Rodar?" depois que ela termina bem, e acusa programa escondido (Base64/hex/bytes/certutil) em vermelho. Devolve 0, não o número da tarefa |
| SLN | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| Space | outras | COBERTA | funcoes.rs:134 |
| Spc | outras | NÃO SE APLICA | só faz sentido com Print (saída de arquivo) |
| Split | outras | COBERTA | `vba.split`: limite e vbTextCompare; texto vazio dá matriz vazia (UBound -1) |
| StrComp | outras | COBERTA | `vba.strcomp`: binário e vbTextCompare |
| StrConv | outras | COBERTA | `vba.strconv`: maiúsculas, minúsculas, primeira maiúscula, vbWide/vbNarrow, vbKatakana/vbHiragana; vbUnicode/vbFromUnicode não mudam o texto (o JavaScript já é Unicode) |
| String | outras | COBERTA | funcoes.rs:135. Usa só o primeiro caractere do segundo argumento (String(n, 65) não converte) |
| StrReverse | outras | COBERTA | funcoes.rs:136 |
| Switch | outras | COBERTA | `vba.switch`; nenhuma verdadeira é Null |
| SYD | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| Tab | outras | NÃO SE APLICA | só faz sentido com Print (saída de arquivo) |
| Time | outras | COBERTA | `vba.agora() % 1` (só a hora) |
| Timer | outras | COBERTA | funcoes.rs:95 (segundos desde a meia-noite) |
| TimeSerial | outras | COBERTA | conta direta em fração do dia |
| TimeValue | outras | COBERTA | `vba.cdate(…) % 1` |
| TypeName | outras | COBERTA | intervalo/aba/pasta resolvidos na conversão (`tipo_de_objeto`, expr.rs); valores por `vba.typeName` (número é Double, como a célula do Excel) |
| UBound | outras | COBERTA | `vba.ubound(m, dim)`: qualquer dimensão, com limite de baixo |
| UCase | outras | COBERTA | funcoes.rs:127 |
| VarType | outras | COBERTA | mesma ideia do TypeName; matriz 8204, objeto 9 |
| Weekday | outras | COBERTA | `vba.weekday` com o primeiro dia da semana |
| WeekdayName | outras | COBERTA | `vba.nomeDia` (pt-BR, com primeiro dia da semana) |
| Year | outras | COBERTA | funcoes.rs:166 (`vba.ano`), ajuda.rs:123 |

## O que falta (02/11/2026)

- Nenhuma função falta. Parcial: CDec (sem o Decimal de 28 algarismos do VBA).

## Medido em .xlsm reais (01/11/2026)

`cargo run --release --example converter_vba -- arquivo.xlsm [pasta]` (a pasta recebe os `.js` e o
`pendencias.txt`). Três arquivos da máquina do usuário:

| arquivo | linhas VBA | não convertidos antes | depois | o que sobrou |
|---|---|---|---|---|
| Fluxo de caixa.xlsm | 634 | 209 | **0** | — |
| precioso.xlsm | 469 | 12 | 1 | `Planilha1.num` (variável que o próprio VBA não declara) |
| Planilha de Informações v6.0a.xlsm | 2.304 | 62 | 51 | automação do Word (26), formas (7), arquivo (6), xlwings (5), CreateObject (2) |

Feito nesta rodada: `Protect`/`Unprotect` (com senha e `Allow*`), `ActiveWindow.DisplayGridlines`
(`DisplayHeadings` sem efeito: os cabeçalhos ficam sempre à mostra), `Selection.AutoFilter` sem
critério e `ws.AutoFilter.Sort` (ordena a tabela do filtro; os botões de filtro ainda não aparecem),
`ShellExecute(0, "open", link)` → `app.openUrl` (só link web, abre sem perguntar como o hiperlink),
`Shapes("…")`/`Shapes.Range(Array("…"))`, `.Select` + `Selection.ShapeRange` e
`.TextFrame2.TextRange.Font` (negrito, itálico, tamanho, cor) e `.Text` → `sheet.shape(nome)`
(o `.xlsx` agora guarda o nome da forma em `nome`).

`ByRef` (02/11/2026): o parâmetro de valor simples que o procedimento altera vira caixa (`p.v`, `gerar/por_ref.rs`); variável passada volta alterada, expressão não; valor solto (evento, fórmula) é embrulhado na entrada. Limite: `Foo (x)` com parênteses ainda vai por referência.

`CreateObject("WScript.Shell").Run` (02/11/2026) → `app.shell` (direto ou com `Set`).
`Collection` e `Scripting.Dictionary` (02/11/2026, `gerar/colecao.rs`) → `vba.colecao()`/`vba.dicionario()`:
`Add` (com chave, `Before`/`After`), `Item`/`c(i)`/`d(k)`, `d(k) = v`, `Count`, `Exists`, `Keys`, `Items`,
`Remove`, `RemoveAll`, `For Each` (itens da coleção, chaves do dicionário); erros 5, 9, 457 e 32811 como no VBA.
`Dim c As New Collection` também. Ainda não: `As New` em variável de módulo, `CompareMode`, `d.Key(k) = novo`. `AutoFilter`
com critério (02/11/2026): `sheet.autoFilter(endereço, coluna, critério)`, `showAllData()` e
`autoFilterOff()` anotam o pedido e o app filtra com o AutoFiltro da tela no fim da macro
(`tabelas::aplicar_filtros_da_macro`); `vba.filtroCrit` leva `Criteria1` (texto do CONT.SE),
`xlFilterValues`/`Array`, `xlOr` de dois valores e top 10. Ainda não: `xlAnd`/`xlOr` de comparação, cor,
filtro dinâmico, e ler as linhas filtradas na mesma macro (`SpecialCells(xlCellTypeVisible)`):
as linhas só somem no fim. `ws.FilterMode`/`AutoFilterMode` lidos valem sempre `True`.

Formas (02/11/2026): `Shapes`/`Buttons`/`Pictures("…")` com `.Visible` (guarda `oculto` na forma; a
camada da grade não desenha nem clica), `.OnAction` (lido e escrito) e `.Left`/`.Top`/`.Width`/`.Height`
(pontos; o app reancora a forma quando a macro termina, `objetos::aplicar_formas_da_macro`). Ler a
posição ainda não converte.

Próximos: automação do Word (fica para quando o FeatherDocs tiver API), `CallByName`, MsgBox com
Abortar/Repetir/Ignorar e ícones.

`SpecialCells` e AutoFilter (06/11/2026): `SpecialCells(xlCellTypeVisible | Blanks | Constants | Formulas)`
(com `Value` 23 ou, em fórmulas, `xlErrors`) vira `vba.especiais` → `aba.specialAreas`, um endereço de várias
áreas (`"A1:C2,A4:C4"`); sem célula, o erro 1004, pego pelo `On Error Resume Next` como no Excel.
`xlCellTypeLastCell` é a célula de `lastRow`/`lastColumn`. As visíveis respeitam o AutoFiltro pedido antes na
mesma macro (`feathersheet-macros/src/visiveis.rs` repete os pedidos sobre as tabelas da aba sem mexer nela; o
app filtra de verdade no fim). A API aceita intervalo de várias áreas (`varias_areas.rs`): escrever, limpar e
formatar em cada área, `copy` juntando as áreas no destino (Copy das visíveis), leitura pela primeira área;
`.Count`, `.Address`, `.EntireRow`/`.EntireColumn`, `For Each` e `.EntireRow.Delete` (de baixo para cima)
contam todas as áreas. `AutoFilter` com dois critérios de comparação (`xlAnd`, o padrão, e `xlOr`) vira o
critério `{op: "e"|"ou"}` (`Criterio::Duas` no motor). Corrigido junto: `rng Is Nothing` com `rng As Range`
lia o valor da célula. Ainda não: SpecialCells de comentários/validação/formatação condicional, constantes só
de números/textos, filtro por cor/dinâmico pela macro, `Areas(i)`; o `.xlsx` não grava o critério de duas
condições (as linhas escondidas ficam).

Lacunas da linguagem (06/11/2026): `Foo (x)` sem `Call` passa `x` por valor (o parser guarda o
`Expr::Paren`); `Dim c As New Collection`/`Dictionary` no módulo nasce pronta (`Macro::novas`);
`d.CompareMode = vbTextCompare` (só com o dicionário vazio, erro 5 como no VBA) e `d.Key(k) = nova`;
ler `.Left/.Top/.Width/.Height` da forma (`ReguaSimples` do `objetos_aba`, a mesma medida da grade).
Ainda não: `&` de número escreve com ponto (`91.5`), o Excel em pt-BR com vírgula.

### Rodando de verdade (01/11/2026)

`cargo run --release --example rodar_xlsm -- arquivo.xlsm [filtro]` roda cada macro e evento
convertido numa cópia da pasta lida do próprio arquivo (respondendo Sim às perguntas):

- precioso.xlsm: 18 de 18 rodam; Planilha de Informações: 30 de 30.
- Fluxo de caixa.xlsm: 27 de 28. O Excel em português chama a caixa de "CaixaDeTexto 39" e a
  macro gravada pede "TextBox 39": `sheet.shape` aceita o nome padrão em qualquer língua quando o
  número bate. A que falha (Macro6) usa `Selection.ShapeRange` antes de selecionar forma — no
  Excel também daria erro com uma célula selecionada.
