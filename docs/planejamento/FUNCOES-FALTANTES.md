# Funções do Excel que faltam no motor

> Levantado em 13/10/2026. Comparação nome por nome entre a tabela `FUNCOES`
> de `crates/feathersheet-core/src/funcoes/functions.rs` (mais `LET`,
> `LAMBDA`, `MAP` e `REDUCE`, que o avaliador resolve pelo nome) e a lista
> alfabética de funções do Excel 365, montada à mão por categoria (520 nomes).
> Apelidos em português não contam: só o nome canônico em inglês.

**Temos as 520 funções do Excel; não falta nenhuma.** (18/10/2026: entraram as 63 que
faltavam, ver "Lote de 18/10/2026" abaixo. 15/10: 48; 14/10: PERCENTOF, CHITEST, TTEST e os
26 números complexos IM*/COMPLEX.) Fora da lista do Excel, o motor tem ainda `COUNTUNIQUE`,
`JOIN`, `REGEXMATCH`, `IMPORTDATA`, `IMPORTJSON`, `FETCHTEXT` e `IMAGE.BASE64`.

## Lote de 18/10/2026 (63 funções)

Arquivos novos: `fn_referencia.rs`, `fn_lambda.rs`, `fn_regressao.rs`, `fn_cupom.rs` e
`fn_compat.rs`; BESSELK/BESSELY em `fn_conversao.rs`. Todas com nome em português, ajuda em
`function_help_data.rs` e descrição/nome em inglês e espanhol (`src/idiomas/funcoes.*.json`).

| Categoria | en / pt |
|---|---|
| Referência (11) | OFFSET/DESLOC, INDIRECT/INDIRETO, TRIMRANGE/AJUSTARINTERVALO, AREAS/ÁREAS, FORMULATEXT/FÓRMULATEXTO, ISFORMULA/ÉFÓRMULA, SHEET/PLAN, SHEETS/PLANS, CELL/CÉL, INFO/INFORMAÇÃO, GETPIVOTDATA/INFODADOSTABELADINÂMICA |
| LAMBDA e agrupamento (7) | BYROW/PORLINHA, BYCOL/PORCOL, SCAN/VARRER, MAKEARRAY/CRIARMATRIZ, ISOMITTED/ÉOMITIDO, GROUPBY/AGRUPARPOR, PIVOTBY/PIVOTARPOR |
| Estatística (6) | LINEST/PROJ.LIN, LOGEST/PROJ.LOG, FORECAST.ETS/PREVISÃO.ETS, FORECAST.ETS.CONFINT, FORECAST.ETS.SEASONALITY/PREVISÃO.ETS.SAZONALIDADE, FORECAST.ETS.STAT |
| Financeiras (18) | ACCRINT/JUROSACUM, AMORDEGRC, AMORLINC, COUPDAYBS/CUPDIASINLIQ, COUPDAYS/CUPDIAS, COUPDAYSNC/CUPDIASPRÓX, COUPNCD/CUPDATAPRÓX, COUPNUM/CUPNÚM, COUPPCD/CUPDATAANT, DURATION/DURAÇÃO, MDURATION/MDURAÇÃO, ODDFPRICE/PREÇOPRIMINC, ODDFYIELD/LUCROPRIMINC, ODDLPRICE/PREÇOÚLTINC, ODDLYIELD/LUCROÚLTINC, PRICE/PREÇO, YIELD/LUCRO, EUROCONVERT |
| Engenharia (2) | BESSELK, BESSELY |
| Texto (4) | ASC, DBCS, BAHTTEXT, PHONETIC/FONÉTICA |
| Web (2) | WEBSERVICE/SERVIÇOWEB (o mesmo pedido de mão única do BUSCAR.TEXTO), FILTERXML/FILTROXML |
| Só reconhecidas (13) | CUBE* (7), RTD, CALL/CHAMAR, REGISTER.ID/REGISTRO.ID, STOCKHISTORY/HISTÓRICODEAÇÕES, TRANSLATE/TRADUZIR, DETECTLANGUAGE/DETECTARIDIOMA |

**Como funcionam as de referência:** DESLOC, INDIRETO e AJUSTARINTERVALO devolvem referência.
O avaliador troca a chamada pelo `Ref`/`Range` antes de passar à função de fora
(`Evaluator::resolver_referencias`), então `SOMA(DESLOC(A1;0;0;1000000))`, `LIN(INDIRETO("B7"))`
e `ÉREF(DESLOC(…))` funcionam como no Excel. DESLOC, INDIRETO, CÉL e INFORMAÇÃO entram como
voláteis (o grafo de dependências não sabe o que elas leem). O cálculo paralelo volta para o
serial quando a fórmula pede o texto da fórmula ou a lista de abas.

**Só reconhecidas:** abrem sem `#NOME?` num arquivo do Excel e devolvem `#N/D` com o motivo.
Cubo OLAP e RTD precisam de servidor; CHAMAR e REGISTRO.ID executariam código de DLL;
HISTÓRICODEAÇÕES, TRADUZIR e DETECTARIDIOMA dependem de serviço na internet, contra a regra de
privacidade (o conteúdo não sai do computador).

**Limites conhecidos:**
- ÁREAS dá sempre 1 (o analisador não tem a união `(A1;C3:D4)`); PLANS(ref) dá 1 (sem referência 3D).
- CÉL: "formato" é sempre "G" e "largura" é 8 (o motor não vê formato nem largura de coluna);
  "arquivo" é vazio. INFORMAÇÃO("diretório") é vazio.
- INFODADOSTABELADINÂMICA lê a dinâmica desenhada na planilha (cabeçalho em cima, "Total Geral"
  embaixo, itens de coluna como `item (valor)`); o par campo/item precisa achar uma linha só.
- AGRUPARPOR faz subtotal só no primeiro nível de campos; PIVOTARPOR só total geral (profundidade 2
  vale como 1), ordena só pelas chaves e ignora `relativo_a`.
- PREVISÃO.ETS é Holt-Winters aditivo com α, β, γ pelo menor erro quadrático (grade + refino). O
  Excel não publica a otimização dele: os valores ficam perto, não idênticos. A data alvo é um
  valor só (não matriz).
- ASC/DBCS convertem letras, dígitos e símbolos latinos; o katakana de meia largura não.
- As financeiras batem com os exemplos da ajuda do Excel (testes em `fn_cupom.rs`); CUPDIASPRÓX na
  base 0 é CUPDIAS − CUPDIASINLIQ, como no Excel.
- Nomes em português sem tradução oficial conhecida, escolhidos aqui: AJUSTARINTERVALO, PORLINHA,
  PORCOL, VARRER, CRIARMATRIZ, ÉOMITIDO, PIVOTARPOR. Em espanhol: RECORTARRANGO. Conferir com o
  Excel em português/espanhol quando houver.

## Lote de 15/10/2026 (48 funções, em `fn_extra.rs`, `fn_conversao.rs` e `fn_titulos.rs`)

Todas com nome em português e em inglês (tabela de apelidos de `functions.rs`) e ajuda em
`function_help_data.rs`.

| Categoria | en / pt |
|---|---|
| Matriz dinâmica (13) | VSTACK/EMPILHARV, HSTACK/EMPILHARH, TAKE/PEGAR, DROP/DESCARTAR, CHOOSECOLS/ESCOLHERCOLS, CHOOSEROWS/ESCOLHERLINS, TOCOL/PARACOL, TOROW/PARALIN, WRAPROWS/QUEBRARLINS, WRAPCOLS/QUEBRARCOLS, EXPAND/EXPANDIR, SORTBY/CLASSIFICARPOR, RANDARRAY/MATRIZALEATÓRIA |
| Pesquisa e informação (5) | LOOKUP/PROC, XMATCH/CORRESPX, ADDRESS/ENDEREÇO, HYPERLINK/HIPERLINK, ISREF/ÉREF |
| Estatística (8) | STEYX/EPADYX, PROB/PROB, F.TEST/TESTE.F, Z.TEST/TESTE.Z, TREND/TENDÊNCIA, GROWTH/CRESCIMENTO, e os nomes de compatibilidade FTEST/TESTEF, ZTEST/TESTEZ |
| Texto (8) | LENB/NÚM.CARACTB, LEFTB/ESQUERDAB, RIGHTB/DIREITAB, MIDB/EXT.TEXTOB, FINDB/PROCURARB, SEARCHB/LOCALIZARB, REPLACEB/MUDARB, ENCODEURL/CODIFURL |
| Engenharia (3) | CONVERT/CONVERTER, BESSELJ/BESSELJ, BESSELI/BESSELI |
| Financeiras (11) | DISC/DESC, PRICEDISC/PREÇODESC, YIELDDISC/LUCRODESC, INTRATE/TAXAJUROS, RECEIVED/RECEBER, TBILLPRICE/OTNVALOR, TBILLYIELD/OTNLUCRO, TBILLEQ/OTN, PRICEMAT/PREÇOVENC, YIELDMAT/LUCROVENC, ACCRINTM/JUROSACUMV |

Limites conhecidos: TENDÊNCIA/CRESCIMENTO aceitam uma só variável X (várias colunas de X dão
`#VALOR!`); CORRESPX trata os modos de pesquisa binária (2 e -2) como pesquisa linear; PROC
supõe o vetor em ordem crescente; as funções `*B` valem como as sem B (só idiomas de byte duplo
contam bytes); HIPERLINK devolve o texto, abrir o vínculo é da interface; ÉREF não reconhece
nome definido como referência.

## Conferência com a página oficial (14/10/2026)

Comparada a lista acima com a página da Microsoft "Funções do Excel (por categoria)"
(pt-BR e en-US): **520 funções; o motor tem 409; as 111 que faltam estão todas nesta
lista** (nenhuma falta fora dela; contagem de 14/10, antes do lote de 15/10). O motor tem ainda 7 fora da página (COUNTUNIQUE,
FETCHTEXT, IMAGE.BASE64, IMPORTDATA, IMPORTJSON, JOIN, REGEXMATCH).

### Funções que temos sem o nome em português

Regra do usuário: com a interface em português, a fórmula aparece só em português. As
funções abaixo existem no motor, mas sem apelido pt-BR. **Cuidado:** a coluna com o nome
em português vem da página da Microsoft, que é tradução automática e erra vários nomes
(ex.: POWER aparece como "ENERGIA", o Excel usa POTÊNCIA; COLUMN como "COLUNA", o Excel
usa COL; TIME como "HORÁRIO", o Excel usa TEMPO; SWITCH como "MUDAR", o Excel usa
PARÂMETRO). Conferir cada nome com o Excel em português antes de criar o apelido.

| Função (en) | Nome pt-BR na página | Apelidos pt no motor |
|---|---|---|
| CHIDIST | QUIDIST | - |
| CHIINV | QUIINV | INV.QUI |
| CHITEST | QUITEST | - |
| CONFIDENCE | CONFIANÇA | INT.CONFIANCA, INT.CONFIANÇA |
| RANK | CLASSIFICAÇÃO | ORDEM |
| CONCATENATE | CONCATENAR | - |
| TIME | HORÁRIO | TEMPO |
| WORKDAY.INTL | DIA DE TRABALHO.INTL | DIATRABALHO.INTL |
| BITLSHIFT | DESLOCAÇÃO RÍGIDA | DESLOCESQBIT |
| BITRSHIFT | DESLOCAÇÃO DE BITS | DESLOCDIRBIT |
| ERF.PRECISE | ERF.PRECISO | FUNERRO.PRECISO |
| ERFC.PRECISE | FUNERRO.PRECISO | FUNERROCOMPL.PRECISO |
| PDURATION | DURAÇÃO PA | DURACAOP, DURAÇÃOP |
| ISBLANK | ÉCÉL.VAZIA | ÉCÉL |
| ISERR | ÉERRO | EERR, ÉERR |
| ISERROR | ÉERROS | É.ERRO, ÉERRO |
| ISNUMBER | ÉNÚMERO | ÉNÚM |
| NA | N/D | NA, NAO.DISP, NAODISP, NÃO.DISP, NÃODISP |
| MAP | MAPA | MAPEAR |
| SWITCH | MUDAR | PARAMETRO, PARÂMETRO |
| CHOOSE | ESCOLHA | ESCOLHER |
| COLUMN | COLUNA | COL |
| COLUMNS | COLUNAS | COLS |
| FILTER | FILTRAR | FILTRO |
| ROW | LINHA | LIN |
| AGGREGATE | AGREGADO | AGREGAR |
| ARABIC | ÁRABE | ARABICO, ARÁBICO |
| CEILING | TETO | - |
| CEILING.MATH | TETO.MATEMÁTICA | TETO.MAT |
| COMBIN | COMBINAR | COMBIN, COMBINAÇÃO |
| FLOOR.MATH | CHÃO.MATEMÁTICA | ARREDMULTB.MAT |
| PERCENTOF | PORCENTAGEM DE | - |
| POWER | ENERGIA | POTENCIA, POTÊNCIA |
| PRODUCT | PRODUTO | - |
| AVERAGEIF | MÉDIAIF | MEDIASE, MÉDIASE |
| BETA.DIST | VERSÃO BETA.DIST | DIST.BETA.N |
| BETA.INV | VERSÃO BETA.INV. | INV.BETA.N |
| BINOM.DIST.RANGE | BINOM.DIST.INTERVALO | DIST.BINOM.INTERVALO |
| BINOM.INV | BINOM.INV. | INV.BINOM |
| CHISQ.INV | CHISQ.INV. | INV.QUIQUA |
| CHISQ.TEST | CHISQ.TESTE | CHITEST, TESTE.QUI, TESTEQUI |
| CONFIDENCE.NORM | CONFIANÇA.NORMA | INT.CONFIANCA.NORM, INT.CONFIANÇA.NORM |
| CONFIDENCE.T | CONFIANÇA.T | INT.CONFIANCA.T, INT.CONFIANÇA.T |
| GAMMA.DIST | GAMA.DIST | DIST.GAMA |
| GAMMA.INV | GAMA.INV. | INV.GAMA |
| LOGNORM.INV | LOGNORM.INV. | INV.LOGNORMAL.N |
| MODE.SNGL | MODO.ÚNICO | - |
| NORM.DIST | NORMA.DIST | DIST.NORM, DISTNORM |
| NORM.INV | NORMA.INV. | INV.NORM, INVNORM |
| NORM.S.DIST | NORMA.S.DIST | DIST.NORMP, DISTNORMP |
| NORM.S.INV | NORMA.S.INV | INV.NORMP.N |
| PERCENTILE.INC | PERCENTIL.INC | - |
| PERCENTRANK.EXC | CLASSIFICAÇÃO PERCENTUAL.EXC | ORDEM.PORCENTUAL.EXC |
| PERCENTRANK.INC | CLASSIFICAÇÃO PERCENTUAL.INC | ORDEM.PORCENTUAL.INC |
| PERMUTATIONA | PERMUTAS | - |
| QUARTILE.INC | QUARTIL.INC | - |
| RANK.AVG | CLASSIFICAÇÃO.MÉD | ORDEM.MED, ORDEM.MÉD |
| STDEV.S | DESVPAD.A | - |
| VAR.S | VAR.A | - |
| CLEAN | LIMPAR | TIRAR |
| DOLLAR | DÓLAR | MOEDA |
| FIND | PROCURAR | LOCALIZAR |
| FIXED | CORRIGIDO | DEF.NUM.DEC, DEF.NÚM.DEC |
| REGEXEXTRACT | EXTRATO REGEX | REGEXEXTRAIR |
| REGEXTEST | TESTE DE REGISTRO | REGEXTESTE |
| SEARCH | LOCALIZAR | PROCURAR |
| UNICHAR | CARACT UNICHAR | CARACTUNICODE |
