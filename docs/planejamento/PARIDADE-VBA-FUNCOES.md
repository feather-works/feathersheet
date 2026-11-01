# Paridade: funções do VBA × conversor VBA→JS do FeatherSheet

## Resumo

Atualizado em 01/11/2026 (a primeira avaliação foi em 15/10/2026, antes da v0.2.0).

- Total de funções avaliadas: **141**
- COBERTA: **105**
- PARCIAL: **11**
- FALTA: **1**
- NÃO SE APLICA: **24**

(Cobertura útil, COBERTA + PARCIAL, de 141: 116. Sem contar as que não se aplicam, de 117: 116, cerca de 99%.)

Lista oficial: página "Functions (Visual Basic for Applications)" da Microsoft Learn (versão de 2024-02). Tipos de conversão foram incluídos pela família (CBool…CVDate), mesmo fora da página principal. Código conferido em `feathersheet/crates/feathersheet-vba/src/gerar/` (funcoes.rs, expr.rs, instr.rs) e no prelúdio `feathersheet-vba/src/gerar/ajuda.rs` (o bloco `const vba = {…}` que vai junto com o código gerado).

Como o conversor trata função desconhecida: `expr.rs:150` devolve o motivo "função X sem equivalente" (o VBA não é convertido e a pendência aparece no relatório). Funções recusadas de propósito (arquivo, Windows, automação) caem em `embutida_outras` (`funcoes.rs`). Os números de linha da tabela são da avaliação de 15/10 e podem ter andado.

Legenda: COBERTA = convertida e usada no código gerado. PARCIAL = convertida com argumentos ou comportamento faltando. FALTA = não convertida (gera pendência). NÃO SE APLICA = depende de Windows, Office, arquivo ou impressora; continua recusada.

| função | categoria | estado | onde/observação |
|---|---|---|---|
| Asc | conversão | COBERTA | funcoes.rs:133 (`charCodeAt(0)`); só o primeiro caractere |
| Chr | conversão | COBERTA | funcoes.rs:132 (`String.fromCharCode`) |
| CVErr | conversão | PARCIAL | `vba.cverr` → `app.error`; xlErrNull vira #ERROR! (a planilha não tem #NULL!) |
| Format | conversão | PARCIAL | ajuda.rs:34-80, funcoes.rs:137. Faltam os formatos nomeados General Number, Scientific, Yes/No, On/Off e datas nomeadas |
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
| Rnd | matemática | PARCIAL | funcoes.rs:96 (`Math.random()`). Falta Rnd(-1)/Randomize e semente reproduzível |
| Sgn | matemática | COBERTA | funcoes.rs:160 |
| Sin | matemática | COBERTA | funcoes.rs:163 |
| Sqr | matemática | COBERTA | funcoes.rs:159 |
| Tan | matemática | COBERTA | funcoes.rs:163 |
| CBool | conversão de tipo | COBERTA | funcoes.rs:152-155 (`como_bool`) |
| CByte | conversão de tipo | COBERTA | `vba.faixaInt`, Estouro fora de 0..255 |
| CCur | conversão de tipo | PARCIAL | funcoes.rs:151 (número JS). Sem precisão fixa de 4 casas |
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
| CallByName | outras | FALTA | não mapeada (chamada dinâmica de objeto) |
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
| FormatCurrency | outras | PARCIAL | `vba.formatNumero` com "R$ "; mesmos limites do FormatNumber |
| FormatDateTime | outras | COBERTA | `vba.formatDataHora`: os cinco formatos, em pt-BR |
| FormatNumber | outras | PARCIAL | `vba.formatNumero`: casas e agrupar; zero à esquerda e negativo entre parênteses ficam no padrão |
| FormatPercent | outras | PARCIAL | `vba.formatNumero` com %; mesmos limites do FormatNumber |
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
| InputBox | outras | COBERTA | ajuda.rs:98 (`prompt`), funcoes.rs:178. Sem tipo nem validação do VBA |
| InStr | outras | COBERTA | funcoes.rs:121, ajuda.rs:22 (início e texto), comparação binária |
| InStrRev | outras | COBERTA | `vba.instrRev`: início e vbTextCompare |
| IPmt | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| IRR | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| IsArray | outras | COBERTA | funcoes.rs:141 |
| IsDate | outras | PARCIAL | `vba.isDate`: texto que o CDate entende; data guardada como número numa variável dá False |
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
| Mid | outras | COBERTA | funcoes.rs:118, ajuda.rs:19 (começa do 1). A forma de instrução `Mid(...) = x` não foi conferida |
| Minute | outras | COBERTA | `vba.minuto` |
| MIRR | outras | COBERTA | bloco `fin` (ajuda.rs): mesma conta do Excel; IRR/Rate por Newton (erro se não convergir); teste `financeiras_batem_com_o_excel` |
| Month | outras | COBERTA | funcoes.rs:167 (`vba.mes`), ajuda.rs:124 |
| MonthName | outras | COBERTA | `vba.nomeMes` (pt-BR) |
| MsgBox | outras | PARCIAL | funcoes.rs:177, ajuda.rs:97. Só Sim/Não e OK/Cancelar (via confirm/alert). Faltam Abortar/Repetir/Ignorar e ícones |
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
| StrConv | outras | PARCIAL | `vba.strconv`: vbUpperCase, vbLowerCase, vbProperCase; as conversões de Unicode/japonês não |
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

## O que falta (01/11/2026)

- CallByName (não mapeada (chamada dinâmica de objeto))

Parciais: CVErr, Format, Rnd, CCur, CDec, FormatCurrency, FormatNumber, FormatPercent, IsDate, MsgBox, StrConv (detalhe na tabela).
