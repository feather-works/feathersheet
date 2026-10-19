# Funções do FeatherSheet

Catálogo das funções do motor Rust, a partir dos registros em `functions.rs`
e das descrições em `function_help_data.rs`.

Cada função pode ser digitada pelo nome em inglês ou por qualquer um dos apelidos em português listados (as duas grafias, com e sem acento, quando existirem).

⚠ 9 função(ões) ainda sem descrição em `function_help_data.rs` aparecem só com o nome.

---

## Matemática

### SOMA (`SUM`)

`SOMA(núm1; [núm2]; …)`

Soma os números ou intervalos informados.

### PRODUCT

`PRODUCT(núm1; [núm2]; …)`

Multiplica todos os números informados.

### ABS

`ABS(núm)`

Valor absoluto (sem sinal).

### RAIZ (`SQRT`)

`RAIZ(núm)`

Raiz quadrada.

### POTÊNCIA (`POWER`)

`POTÊNCIA(núm; expoente)`

Eleva núm ao expoente.

Outros apelidos aceitos: `POTENCIA`

### EXP

`EXP(núm)`

e elevado a núm.

### LN

`LN(núm)`

Logaritmo natural.

### LOG

`LOG(núm; [base])`

Logaritmo na base indicada (padrão 10).

### LOG10

`LOG10(núm)`

Logaritmo na base 10.

### SINAL (`SIGN`)

`SINAL(núm)`

Devolve -1, 0 ou 1 conforme o sinal.

### INT

`INT(núm)`

Arredonda para baixo até o inteiro mais próximo.

### ARRED (`ROUND`)

`ARRED(núm; [dígitos])`

Arredonda para o número de casas decimais.

### ARREDONDAR.PARA.CIMA (`ROUNDUP`)

`ARREDONDAR.PARA.CIMA(núm; [dígitos])`

Arredonda para cima, longe do zero.

### ARREDONDAR.PARA.BAIXO (`ROUNDDOWN`)

`ARREDONDAR.PARA.BAIXO(núm; [dígitos])`

Arredonda para baixo, em direção ao zero.

### TRUNC

`TRUNC(núm; [dígitos])`

Corta as casas decimais sem arredondar.

### RESTO (`MOD`)

`RESTO(núm; divisor)`

Resto da divisão.

### CEILING

`CEILING(núm; [múltiplo])`

Arredonda para cima até o múltiplo.

### FLOOR

`FLOOR(núm; [múltiplo])`

Arredonda para baixo até o múltiplo.

### PI

`PI()`

O número π.

### ALEATÓRIO (`RAND`)

`ALEATÓRIO()`

Número aleatório entre 0 e 1. Recalcula a cada alteração.

Outros apelidos aceitos: `ALEATORIO`

### ALEATÓRIOENTRE (`RANDBETWEEN`)

`ALEATÓRIOENTRE(mín; máx)`

Inteiro aleatório entre os limites.

### SOMARPRODUTO (`SUMPRODUCT`)

`SOMARPRODUTO(matriz1; [matriz2]; …)`

Multiplica as matrizes item a item e soma os produtos.

### FATORIAL (`FACT`)

`FATORIAL(núm)`

Fatorial.

### COMBINAÇÃO (`COMBIN`)

`COMBINAÇÃO(total; escolhidos)`

Quantas combinações sem repetição.

### PERMUTAÇÃO (`PERMUT`)

`PERMUTAÇÃO(total; escolhidos)`

Quantos arranjos, considerando a ordem.

### ÉPAR (`ISEVEN`)

`ÉPAR(núm)`

VERDADEIRO quando o número é par.

Outros apelidos aceitos: `EPAR`

### ÉÍMPAR (`ISODD`)

`ÉÍMPAR(núm)`

VERDADEIRO quando o número é ímpar.

Outros apelidos aceitos: `ÉIMPAR`, `EIMPAR`

### SEN (`SIN`)

`SEN(ângulo)`

Seno de um ângulo em radianos.

### COS

`COS(ângulo)`

Cosseno de um ângulo em radianos.

### TG (`TAN`)

`TG(ângulo)`

Tangente de um ângulo em radianos.

### ASEN (`ASIN`)

`ASEN(núm)`

Arco seno, em radianos.

### ACOS

`ACOS(núm)`

Arco cosseno, em radianos.

### ATG (`ATAN`)

`ATG(núm)`

Arco tangente, em radianos.

### ATAN2

`ATAN2(x; y)`

Arco tangente das coordenadas x e y, em radianos.

### SENH (`SINH`)

`SENH(núm)`

Seno hiperbólico.

### COSH

`COSH(núm)`

Cosseno hiperbólico.

### TGH (`TANH`)

`TGH(núm)`

Tangente hiperbólica.

### RADIANOS (`RADIANS`)

`RADIANOS(graus)`

Converte graus em radianos.

### GRAUS (`DEGREES`)

`GRAUS(radianos)`

Converte radianos em graus.

## Estatística

### MÉDIA (`AVERAGE`)

`MÉDIA(núm1; [núm2]; …)`

Média aritmética dos números.

Outros apelidos aceitos: `MEDIA`

### MED (`MEDIAN`)

`MED(núm1; [núm2]; …)`

Valor central do conjunto ordenado.

### MODO (`MODE`)

`MODO(núm1; …)`

Valor que mais se repete.

### MÍNIMO (`MIN`)

`MÍNIMO(núm1; [núm2]; …)`

Menor número do conjunto.

Outros apelidos aceitos: `MINIMO`

### MÁXIMO (`MAX`)

`MÁXIMO(núm1; [núm2]; …)`

Maior número do conjunto.

Outros apelidos aceitos: `MAXIMO`

### MAIOR (`LARGE`)

`MAIOR(intervalo; k)`

O k-ésimo maior valor.

### MENOR (`SMALL`)

`MENOR(intervalo; k)`

O k-ésimo menor valor.

### CONT.NÚM (`COUNT`)

`CONT.NÚM(valor1; [valor2]; …)`

Conta quantas células contêm números.

Outros apelidos aceitos: `CONT.NUM`

### CONT.VALORES (`COUNTA`)

`CONT.VALORES(valor1; [valor2]; …)`

Conta as células que não estão vazias.

### CONT.VAZIO (`COUNTBLANK`)

`CONT.VAZIO(intervalo)`

Conta as células vazias do intervalo.

Outros apelidos aceitos: `CONTAR.VAZIO`, `CONTVAZIO`

### CONT.ÚNICO (`COUNTUNIQUE`)

`CONT.ÚNICO(valor1; …)`

Quantos valores diferentes existem.

Outros apelidos aceitos: `CONTAR.ÚNICO`, `CONTAR.UNICO`

### DESVPAD (`STDEV`)

`DESVPAD(núm1; …)`

Desvio padrão de uma amostra.

### STDEV.P

`STDEV.P(núm1; …)`

Desvio padrão da população inteira.

### VAR

`VAR(núm1; …)`

Variância de uma amostra.

### VAR.P

`VAR.P(núm1; …)`

Variância da população inteira.

### DESV.MÉDIO (`AVEDEV`)

`DESV.MÉDIO(núm1; …)`

Desvio médio absoluto em relação à média.

### DESVQ (`DEVSQ`)

`DESVQ(núm1; …)`

Soma dos quadrados dos desvios.

### MÉDIA.GEOMÉTRICA (`GEOMEAN`)

`MÉDIA.GEOMÉTRICA(núm1; …)`

Média geométrica; serve para taxas de crescimento.

### MÉDIA.HARMÔNICA (`HARMEAN`)

`MÉDIA.HARMÔNICA(núm1; …)`

Média harmônica; serve para médias de velocidade.

### DISTORÇÃO (`SKEW`)

`DISTORÇÃO(núm1; …)`

Assimetria da distribuição.

### CURT (`KURT`)

`CURT(núm1; …)`

Curtose: o quanto a distribuição é pontuda.

### PERCENTIL (`PERCENTILE`)

`PERCENTIL(intervalo; p)`

Valor abaixo do qual está a fração p dos dados (p entre 0 e 1).

### QUARTIL (`QUARTILE`)

`QUARTIL(intervalo; quarto)`

Quartil: 0 mínimo, 1, 2 mediana, 3, 4 máximo.

### ORDEM (`RANK`)

`ORDEM(valor; intervalo; [crescente])`

Posição do valor na lista ordenada.

### SOMASE (`SUMIF`)

`SOMASE(intervalo; critério; [soma_intervalo])`

Soma só as células que atendem ao critério.

### SOMASES (`SUMIFS`)

`SOMASES(soma_intervalo; intervalo1; critério1; …)`

Soma quando todos os critérios batem ao mesmo tempo.

### CONT.SE (`COUNTIF`)

`CONT.SE(intervalo; critério)`

Conta as células que atendem ao critério.

Outros apelidos aceitos: `CONTSE`

### CONT.SES (`COUNTIFS`)

`CONT.SES(intervalo1; critério1; …)`

Conta as linhas que atendem a todos os critérios.

Outros apelidos aceitos: `CONTSES`

### MÉDIASE (`AVERAGEIF`)

`MÉDIASE(intervalo; critério; [média_intervalo])`

Média das células que atendem ao critério.

Outros apelidos aceitos: `MEDIASE`

### MÉDIASES (`AVERAGEIFS`)

`MÉDIASES(média_intervalo; intervalo1; critério1; …)`

Média das linhas que atendem a todos os critérios.

Outros apelidos aceitos: `MEDIASES`

### MÁXIMOSES (`MAXIFS`)

`MÁXIMOSES(máx_intervalo; intervalo1; critério1; …)`

Maior valor entre as linhas que atendem aos critérios.

Outros apelidos aceitos: `MAXIMOSES`

### MÍNIMOSES (`MINIFS`)

`MÍNIMOSES(mín_intervalo; intervalo1; critério1; …)`

Menor valor entre as linhas que atendem aos critérios.

Outros apelidos aceitos: `MINIMOSES`

### CORREL

`CORREL(y; x)`

Correlação entre os dois conjuntos, de -1 a 1.

### COVAR

`COVAR(y; x)`

Covariância entre os dois conjuntos.

### INCLINAÇÃO (`SLOPE`)

`INCLINAÇÃO(y; x)`

Inclinação da reta que melhor descreve os pontos.

Outros apelidos aceitos: `INCLINACAO`

### INTERCEPÇÃO (`INTERCEPT`)

`INTERCEPÇÃO(y; x)`

Onde essa reta cruza o eixo vertical.

Outros apelidos aceitos: `INTERCEPCAO`

### RQUAD (`RSQ`)

`RQUAD(y; x)`

R² : o quanto a reta explica os dados.

### PREVISÃO (`FORECAST`)

`PREVISÃO(x; y_conhecidos; x_conhecidos)`

Projeta um valor usando a reta de tendência.

Outros apelidos aceitos: `PREVISAO`

### STDEV.S

*(sem descrição cadastrada ainda)*

### VAR.S

*(sem descrição cadastrada ainda)*

### MODE.SNGL

*(sem descrição cadastrada ainda)*

### PERCENTILE.INC

*(sem descrição cadastrada ainda)*

### QUARTILE.INC

*(sem descrição cadastrada ainda)*

### DIST.NORM (`NORM.DIST`)

`DIST.NORM(x; média; desv_padrão; cumulativo)`

Distribuição normal (densidade ou cumulativa).

Outros apelidos aceitos: `DISTNORM`

### DIST.NORMP (`NORM.S.DIST`)

`DIST.NORMP(z; cumulativo)`

Distribuição normal padrão (média 0 e desvio padrão 1).

Outros apelidos aceitos: `DISTNORMP`

### INV.NORM (`NORM.INV`)

`INV.NORM(probabilidade; média; desv_padrão)`

Inverso da distribuição normal para a probabilidade.

Outros apelidos aceitos: `INVNORM`

### DIST.T (`T.DIST`)

`DIST.T(x; graus_liberdade; cumulativo)`

Distribuição t de Student.

Outros apelidos aceitos: `DISTT`

### TESTE.T (`T.TEST`)

`TESTE.T(matriz1; matriz2; caudas; tipo)`

Devolve a probabilidade associada ao teste t de Student.

Outros apelidos aceitos: `TESTET`

### DIST.QUI (`CHISQ.DIST`)

`DIST.QUI(x; graus_liberdade; cumulativo)`

Distribuição qui-quadrado.

Outros apelidos aceitos: `DISTQUI`

### TESTE.QUI (`CHISQ.TEST`)

`TESTE.QUI(matriz_real; matriz_esperada)`

Devolve a probabilidade do teste qui-quadrado de independência.

Outros apelidos aceitos: `TESTEQUI`

### PROJ.LIN (`LINEST`)

`PROJ.LIN(y_conhecidos; [x_conhecidos]; [constante]; [estatísticas])`

Regressão linear por mínimos quadrados (uma ou várias variáveis): coeficientes mₙ…m₁ e b; com estatísticas, também erros padrão, R², F e somas de quadrados.

### PROJ.LOG (`LOGEST`)

`PROJ.LOG(y_conhecidos; [x_conhecidos]; [constante]; [estatísticas])`

Curva exponencial y = b·m₁^x₁·…: devolve mₙ…m₁ e b (e as estatísticas, na escala do logaritmo).

### PREVISÃO.ETS (`FORECAST.ETS`)

`PREVISÃO.ETS(data_alvo; valores; linha_do_tempo; [sazonalidade]; [conclusão_dados]; [agregação])`

Previsão por suavização exponencial tripla (tendência e sazonalidade aditivas). Sazonalidade 1 detecta sozinha, 0 desliga.

Outros apelidos aceitos: `PREVISAO.ETS`

### PREVISÃO.ETS.CONFINT (`FORECAST.ETS.CONFINT`)

`PREVISÃO.ETS.CONFINT(data_alvo; valores; linha_do_tempo; [nível_confiança]; [sazonalidade]; [conclusão_dados]; [agregação])`

Meia largura do intervalo de confiança da PREVISÃO.ETS na data alvo (95% por padrão).

Outros apelidos aceitos: `PREVISAO.ETS.CONFINT`

### PREVISÃO.ETS.SAZONALIDADE (`FORECAST.ETS.SEASONALITY`)

`PREVISÃO.ETS.SAZONALIDADE(valores; linha_do_tempo; [conclusão_dados]; [agregação])`

Tamanho da estação que a PREVISÃO.ETS detecta (0 = sem sazonalidade).

Outros apelidos aceitos: `PREVISAO.ETS.SAZONALIDADE`

### PREVISÃO.ETS.STAT (`FORECAST.ETS.STAT`)

`PREVISÃO.ETS.STAT(valores; linha_do_tempo; tipo_estatística; [sazonalidade]; [conclusão_dados]; [agregação])`

Estatística do modelo da PREVISÃO.ETS: 1 alfa, 2 beta, 3 gama, 4 MASE, 5 SMAPE, 6 MAE, 7 RMSE, 8 passo.

Outros apelidos aceitos: `PREVISAO.ETS.STAT`

## Matriz dinâmica

### FILTRO (`FILTER`)

`FILTRO(intervalo; condição; [se_vazio])`

Devolve só as linhas (ou colunas) em que a condição é verdadeira. O resultado derrama nas células vizinhas.

### ÚNICO (`UNIQUE`)

`ÚNICO(intervalo; [por_coluna])`

Remove as repetições, mantendo a primeira ocorrência.

Outros apelidos aceitos: `UNICO`

### CLASSIFICAR (`SORT`)

`CLASSIFICAR(intervalo; [coluna]; [crescente])`

Ordena as linhas do intervalo por uma coluna.

Outros apelidos aceitos: `ORDENAR`

### TRANSPOR (`TRANSPOSE`)

`TRANSPOR(intervalo)`

Troca linhas por colunas.

Outros apelidos aceitos: `MATRIZ.TRANSPOSTA`

### SEQUÊNCIA (`SEQUENCE`)

`SEQUÊNCIA(linhas; [colunas]; [início]; [passo])`

Gera uma sequência de números.

Outros apelidos aceitos: `SEQUENCIA`

### FREQÜÊNCIA (`FREQUENCY`)

`FREQÜÊNCIA(dados; classes)`

Quantos dados caem em cada faixa. Derrama uma coluna com uma linha a mais que as classes: a última conta o que passou do maior limite.

Outros apelidos aceitos: `FREQUÊNCIA`, `FREQUENCIA`

### MATRIZ.MULT (`MMULT`)

`MATRIZ.MULT(matriz1; matriz2)`

Produto de duas matrizes. As colunas da primeira têm de bater com as linhas da segunda.

Outros apelidos aceitos: `MATRIZ.MULTIPLICAR`

### MATRIZ.INVERSO (`MINVERSE`)

`MATRIZ.INVERSO(matriz)`

Matriz inversa de uma matriz quadrada.

### MATRIZ.DETERM (`MDETERM`)

`MATRIZ.DETERM(matriz)`

Determinante de uma matriz quadrada.

### AGRUPARPOR (`GROUPBY`)

`AGRUPARPOR(campos_linha; valores; função; [cabeçalhos]; [profundidade_total]; [ordem]; [filtro]; [relação])`

Agrupa os valores pelos campos e aplica a função a cada grupo, com total. Ex.: =AGRUPARPOR(A2:A9;C2:C9;SOMA).

### PIVOTARPOR (`PIVOTBY`)

`PIVOTARPOR(campos_linha; campos_coluna; valores; função; [cabeçalhos]; [total_linhas]; [ordem_linhas]; [total_colunas]; [ordem_colunas]; [filtro]; [relativo_a])`

Tabela cruzada: linhas por um campo, colunas por outro, a função em cada cruzamento e os totais. Ex.: =PIVOTARPOR(A2:A9;B2:B9;C2:C9;SOMA).

## Lógica

### SE (`IF`)

`SE(teste; se_verdadeiro; [se_falso])`

Devolve um valor ou outro conforme o teste.

### SES (`IFS`)

`SES(teste1; valor1; …)`

Testa condições em ordem e devolve a do primeiro verdadeiro.

### SEERRO (`IFERROR`)

`SEERRO(valor; se_erro)`

Substitui o resultado quando ele é um erro.

### E (`AND`)

`E(lógico1; …)`

Verdadeiro só se todos forem verdadeiros.

### OU (`OR`)

`OU(lógico1; …)`

Verdadeiro se pelo menos um for verdadeiro.

### NÃO (`NOT`)

`NÃO(lógico)`

Inverte verdadeiro e falso.

Outros apelidos aceitos: `NAO`

### XOR

`XOR(lógico1; …)`

Verdadeiro quando a quantidade de verdadeiros é ímpar.

### VERDADEIRO (`TRUE`)

`VERDADEIRO()`

O valor lógico verdadeiro.

### FALSO (`FALSE`)

`FALSO()`

O valor lógico falso.

### É.ERRO (`ISERROR`)

`É.ERRO(valor)`

Verdadeiro se o valor for um erro.

Outros apelidos aceitos: `ÉERRO`

### ÉCÉL (`ISBLANK`)

`ÉCÉL(valor)`

Verdadeiro se a célula estiver vazia.

### ÉNÚM (`ISNUMBER`)

`ÉNÚM(valor)`

Verdadeiro se o valor for número.

### ÉTEXTO (`ISTEXT`)

`ÉTEXTO(valor)`

Verdadeiro se o valor for texto.

### SENÃODISP (`IFNA`)

`SENÃODISP(valor; valor_se_na)`

Devolve valor_se_na se o resultado for #N/A.

Outros apelidos aceitos: `SENAODISP`

### PARÂMETRO (`SWITCH`)

`PARÂMETRO(expressao; caso1; valor1; …; [padrao])`

Compara a expressão com opções e devolve o valor do caso.

Outros apelidos aceitos: `PARAMETRO`

### NÃODISP (`NA`)

*(sem descrição cadastrada ainda)*

Outros apelidos aceitos: `NAODISP`

### PORLINHA (`BYROW`)

`PORLINHA(matriz; lambda)`

Aplica a LAMBDA (ou o nome de uma função) a cada linha e devolve uma coluna. Ex.: =PORLINHA(A1:C3;LAMBDA(l;SOMA(l))).

### PORCOL (`BYCOL`)

`PORCOL(matriz; lambda)`

Aplica a LAMBDA (ou o nome de uma função) a cada coluna e devolve uma linha. Ex.: =PORCOL(A1:C3;MÁXIMO).

### VARRER (`SCAN`)

`VARRER([valor_inicial]; matriz; lambda)`

Como REDUZIR, mas devolve cada passo do acumulador. Ex.: =VARRER(0;A1:A5;LAMBDA(a;v;a+v)) dá a soma acumulada.

### CRIARMATRIZ (`MAKEARRAY`)

`CRIARMATRIZ(linhas; colunas; lambda)`

Monta uma matriz chamando a LAMBDA(linha; coluna) em cada posição. Ex.: =CRIARMATRIZ(3;3;LAMBDA(l;c;l*c)).

### ÉOMITIDO (`ISOMITTED`)

`ÉOMITIDO(argumento)`

Dentro de uma LAMBDA, VERDADEIRO se o parâmetro não foi passado.

Outros apelidos aceitos: `EOMITIDO`

## Texto

### CONCATENAR (`CONCAT`)

`CONCATENAR(texto1; …)`

Junta os textos em um só.

Outros apelidos aceitos: `CONCATENATE`

### NÚM.CARACT (`LEN`)

`NÚM.CARACT(texto)`

Quantidade de caracteres.

Outros apelidos aceitos: `NUM.CARACT`

### MAIÚSCULA (`UPPER`)

`MAIÚSCULA(texto)`

Converte para maiúsculas.

Outros apelidos aceitos: `MAIUSCULA`

### MINÚSCULA (`LOWER`)

`MINÚSCULA(texto)`

Converte para minúsculas.

Outros apelidos aceitos: `MINUSCULA`

### PRI.MAIÚSCULA (`PROPER`)

`PRI.MAIÚSCULA(texto)`

Deixa a primeira letra de cada palavra maiúscula.

### ARRUMAR (`TRIM`)

`ARRUMAR(texto)`

Remove espaços sobrando.

### ESQUERDA (`LEFT`)

`ESQUERDA(texto; [qtd])`

Primeiros caracteres do texto.

### DIREITA (`RIGHT`)

`DIREITA(texto; [qtd])`

Últimos caracteres do texto.

### EXT (`MID`)

`EXT(texto; início; qtd)`

Trecho do texto a partir de uma posição.

Outros apelidos aceitos: `EXT.TEXTO`

### REPETIR (`REPT`)

`REPETIR(texto; vezes)`

Repete o texto n vezes.

### SUBSTITUIR (`SUBSTITUTE`)

`SUBSTITUIR(texto; antigo; novo; [ocorrência])`

Troca um trecho por outro.

### PROCURAR (`FIND`)

`PROCURAR(procurado; texto; [início])`

Posição do trecho, diferenciando maiúsculas e sem curingas.

### LOCALIZAR (`SEARCH`)

`LOCALIZAR(procurado; texto; [início])`

Posição do trecho, ignorando maiúsculas; aceita ? e * como curingas.

### VALOR (`VALUE`)

`VALOR(texto)`

Converte texto em número.

### TEXTO (`TEXT`)

`TEXTO(valor; formato)`

Formata um número como texto (ex.: "R$ #.##0,00").

### CONCATENATE

*(sem descrição cadastrada ainda)*

### UNIRTEXTO (`TEXTJOIN`)

`UNIRTEXTO(delimitador; ignorar_vazio; texto1; …)`

Junta textos com um delimitador.

### DIVIDIRTEXTO (`TEXTSPLIT`)

`DIVIDIRTEXTO(texto; delimitador_coluna; [delimitador_linha])`

Divide o texto em colunas e linhas.

### JUNTAR (`JOIN`)

`JUNTAR(delimitador; valor1; …)`

Junta valores com um delimitador.

### ASC

`ASC(texto)`

Troca letras, dígitos e símbolos de largura inteira pelos de meia largura.

### DBCS

`DBCS(texto)`

Troca letras, dígitos e símbolos de meia largura pelos de largura inteira.

### BAHTTEXT

`BAHTTEXT(núm)`

O valor em baht por extenso, em tailandês.

### FONÉTICA (`PHONETIC`)

`FONÉTICA(ref)`

Texto fonético (furigana) da célula; sem ele, o próprio texto.

Outros apelidos aceitos: `FONETICA`

## Pesquisa e referência

### PROCV (`VLOOKUP`)

`PROCV(procurado; intervalo; índice_coluna; [aproximado])`

Procura na primeira coluna e devolve o valor da coluna indicada. Use FALSO para busca exata.

### PROCH (`HLOOKUP`)

`PROCH(procurado; intervalo; índice_linha; [aproximado])`

Procura na primeira linha e devolve o valor da linha indicada.

### PROCX (`XLOOKUP`)

`PROCX(procurado; onde_procurar; o_que_devolver; [se_não_achar]; [modo])`

Busca moderna: intervalos separados, sem índice numérico. Modo 0 exato, -1 menor mais próximo, 1 maior mais próximo, 2 curinga.

### ÍNDICE (`INDEX`)

`ÍNDICE(intervalo; linha; [coluna])`

Valor na posição indicada dentro do intervalo.

Outros apelidos aceitos: `INDICE`

### CORRESP (`MATCH`)

`CORRESP(procurado; intervalo; [tipo])`

Posição do valor dentro do intervalo. Use 0 para exato.

### ESCOLHER (`CHOOSE`)

`ESCOLHER(índice; valor1; …)`

Escolhe um valor da lista pelo índice.

### LIN (`ROW`)

`LIN([ref])`

Número da linha da referência.

### COL (`COLUMN`)

`COL([ref])`

Número da coluna da referência.

### LINS (`ROWS`)

`LINS(intervalo)`

Quantidade de linhas do intervalo.

### COLS (`COLUMNS`)

`COLS(intervalo)`

Quantidade de colunas do intervalo.

### DESLOC (`OFFSET`)

`DESLOC(ref; linhas; colunas; [altura]; [largura])`

Referência deslocada de linhas e colunas a partir de ref, com a altura e a largura pedidas. Ex.: =SOMA(DESLOC(A1;0;0;3)) soma A1:A3.

### INDIRETO (`INDIRECT`)

`INDIRETO(texto_ref; [a1])`

Referência escrita como texto ("B7", "Dados!A1:C9" ou um nome); a1 FALSO lê o estilo L1C1. Ex.: =INDIRETO("A"&B1).

### AJUSTARINTERVALO (`TRIMRANGE`)

`AJUSTARINTERVALO(intervalo; [aparar_linhas]; [aparar_colunas])`

O intervalo sem as linhas e colunas vazias das bordas. 0 não apara, 1 só no começo, 2 só no fim, 3 nos dois (padrão). Ex.: =AJUSTARINTERVALO(A:A).

### ÁREAS (`AREAS`)

`ÁREAS(ref)`

Quantas áreas a referência tem.

### FÓRMULATEXTO (`FORMULATEXT`)

`FÓRMULATEXTO(ref)`

O texto da fórmula da célula; sem fórmula, #N/D.

Outros apelidos aceitos: `FORMULATEXTO`

### ÉFÓRMULA (`ISFORMULA`)

`ÉFÓRMULA(ref)`

VERDADEIRO se a célula tem fórmula.

Outros apelidos aceitos: `EFORMULA`, `ÉFORMULA`

### PLAN (`SHEET`)

`PLAN([valor])`

Número da aba (1 = a primeira guia) da referência, do nome em texto ou, sem argumento, da aba da fórmula.

### PLANS (`SHEETS`)

`PLANS([ref])`

Quantas abas a pasta tem (sem argumento) ou a referência cobre.

### CÉL (`CELL`)

`CÉL(tipo_info; [ref])`

Informação da célula: "endereço", "col", "lin", "conteúdo", "tipo" (b vazia, l texto, v valor), "prefixo", "formato", "largura", "proteger".

Outros apelidos aceitos: `CEL`

### INFORMAÇÃO (`INFO`)

`INFORMAÇÃO(tipo)`

Informação do ambiente: "numarquivo" (abas), "origem", "versãoso", "recalc", "versão", "sistema", "diretório".

Outros apelidos aceitos: `INFORMACAO`

### INFODADOSTABELADINÂMICA (`GETPIVOTDATA`)

`INFODADOSTABELADINÂMICA(campo_dados; tabela; [campo1]; [item1]; …)`

Valor de uma tabela dinâmica desenhada na planilha: o campo de dados, uma célula da tabela e pares campo/item. Sem pares, o Total Geral.

Outros apelidos aceitos: `INFODADOSTABELADINAMICA`

## Data e hora

### HOJE (`TODAY`)

`HOJE()`

A data de hoje.

### AGORA (`NOW`)

`AGORA()`

Data e hora atuais.

### DATA (`DATE`)

`DATA(ano; mês; dia)`

Monta uma data a partir das partes.

### DATADIF (`DATEDIF`)

`DATADIF(data_início; data_fim; unidade)`

Diferença entre duas datas. Unidade: "A" anos, "M" meses, "D" dias, "AM" meses que sobram do ano, "AD" dias que sobram do ano, "MD" dias que sobram do mês.

Outros apelidos aceitos: `DATEDIFF`, `DATA.DIF`

### ANO (`YEAR`)

`ANO(data)`

Ano de uma data.

### MÊS (`MONTH`)

`MÊS(data)`

Mês de uma data (1 a 12).

Outros apelidos aceitos: `MES`

### DIA (`DAY`)

`DIA(data)`

Dia do mês de uma data.

### WEEKDAY

`WEEKDAY(data)`

Dia da semana (1 = domingo).

### NÚMSEMANA (`WEEKNUM`)

`NÚMSEMANA(data; [tipo])`

Devolve o número da semana no ano.

Outros apelidos aceitos: `NUMSEMANA`

### DATAM (`EDATE`)

`DATAM(data_inicio; meses)`

Adiciona ou subtrai meses a uma data.

### FIMMÊS (`EOMONTH`)

`FIMMÊS(data_inicio; meses)`

Devolve a data do último dia do mês.

Outros apelidos aceitos: `FIMMES`

### DIATRABALHO (`WORKDAY`)

`DIATRABALHO(data_inicio; dias; [feriados])`

Calcula a data após N dias úteis.

### DIATRABALHOTOTAL (`NETWORKDAYS`)

`DIATRABALHOTOTAL(data_inicio; data_fim; [feriados])`

Conta os dias úteis entre duas datas.

## Fonte de dados

### IMPORTARDADOS (`IMPORTDATA`)

`IMPORTARDADOS(url; [separador])`

Lê um CSV ou TSV de um endereço na internet e derrama a tabela na grade. O separador é descoberto sozinho, ou informe ";" no segundo argumento.

### IMPORTARJSON (`IMPORTJSON`)

`IMPORTARJSON(url; [caminho])`

Lê um JSON de um endereço e traz um pedaço dele. O caminho usa pontos e colchetes: "dados.itens[0].nome". Uma lista de objetos vira tabela com cabeçalho.

### BUSCAR.TEXTO (`FETCHTEXT`)

`BUSCAR.TEXTO(url)`

Devolve o conteúdo de um endereço como texto cru, sem interpretar.

Outros apelidos aceitos: `BUSCARTEXTO`

### FILTROXML (`FILTERXML`)

`FILTROXML(xml; xpath)`

Valores de um texto XML pelo caminho XPath (/a/b, //b, [1], [@id='x'], /@atributo, text()). Ex.: =FILTROXML(A1;"//item").

### SERVIÇOWEB (`WEBSERVICE`)

`SERVIÇOWEB(url)`

O texto que a URL devolve (só pedido explícito, de mão única: nada da planilha é enviado).

Outros apelidos aceitos: `SERVICOWEB`

### MEMBROKPICUBO (`CUBEKPIMEMBER`)

`MEMBROKPICUBO(conexão; nome_kpi; propriedade_kpi; [legenda])`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### MEMBROCUBO (`CUBEMEMBER`)

`MEMBROCUBO(conexão; expressão_membro; [legenda])`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### PROPRIEDADEMEMBROCUBO (`CUBEMEMBERPROPERTY`)

`PROPRIEDADEMEMBROCUBO(conexão; expressão_membro; propriedade)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### MEMBROCLASSIFICADOCUBO (`CUBERANKEDMEMBER`)

`MEMBROCLASSIFICADOCUBO(conexão; expressão_conjunto; classificação; [legenda])`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### CONJUNTOCUBO (`CUBESET`)

`CONJUNTOCUBO(conexão; expressão_conjunto; [legenda]; [ordem]; [classificar_por])`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### CONTAGEMCONJUNTOCUBO (`CUBESETCOUNT`)

`CONTAGEMCONJUNTOCUBO(conjunto)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### VALORCUBO (`CUBEVALUE`)

`VALORCUBO(conexão; [expressão_membro1]; …)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### RTD

`RTD(id_prog; servidor; tópico1; [tópico2]; …)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### CHAMAR (`CALL`)

`CHAMAR(…)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### REGISTRO.ID (`REGISTER.ID`)

`REGISTRO.ID(…)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### HISTÓRICODEAÇÕES (`STOCKHISTORY`)

`HISTÓRICODEAÇÕES(ação; data_inicial; [data_final]; [intervalo]; [cabeçalhos]; …)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

Outros apelidos aceitos: `HISTORICODEACOES`

### TRADUZIR (`TRANSLATE`)

`TRADUZIR(texto; [idioma_origem]; [idioma_destino])`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

### DETECTARIDIOMA (`DETECTLANGUAGE`)

`DETECTARIDIOMA(texto)`

Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.

## Imagem

### IMAGEM (`IMAGE`)

*(sem descrição cadastrada ainda)*

### IMAGEM.BASE64 (`IMAGE.BASE64`)

*(sem descrição cadastrada ainda)*

## Texto (expressão regular)

### REGEXCORRESP (`REGEXMATCH`)

`REGEXCORRESP(texto; expressao)`

VERDADEIRO se o texto casa com a expressão regular.

### REGEXEXTRAIR (`REGEXEXTRACT`)

`REGEXEXTRAIR(texto; expressao)`

Extrai o trecho que casa com a expressão regular.

### REGEXSUBSTITUIR (`REGEXREPLACE`)

`REGEXSUBSTITUIR(texto; expressao; substituicao)`

Substitui o padrão encontrado pela expressão regular.

## Engenharia

### BESSELK

`BESSELK(x; n)`

Função de Bessel modificada de segunda espécie Kn(x), x > 0. Ex.: =BESSELK(1,5;1) dá 0,277388.

### BESSELY

`BESSELY(x; n)`

Função de Bessel de segunda espécie Yn(x), x > 0. Ex.: =BESSELY(2,5;1) dá 0,145918.

## Financeiras

### JUROSACUM (`ACCRINT`)

`JUROSACUM(emissão; primeiro_juro; liquidação; taxa; valor_nominal; frequência; [base]; [método_cálculo])`

Juros acumulados de um título que paga juros periódicos.

### AMORDEGRC

`AMORDEGRC(custo; data_compra; primeiro_período; residual; período; taxa; [base])`

Depreciação de cada período no sistema francês, com coeficiente pela vida útil.

### AMORLINC

`AMORLINC(custo; data_compra; primeiro_período; residual; período; taxa; [base])`

Depreciação linear de cada período no sistema francês (primeiro período proporcional).

### CUPDIASINLIQ (`COUPDAYBS`)

`CUPDIASINLIQ(liquidação; vencimento; frequência; [base])`

Dias do início do período de cupom até a liquidação.

### CUPDIAS (`COUPDAYS`)

`CUPDIAS(liquidação; vencimento; frequência; [base])`

Dias do período de cupom que contém a liquidação.

### CUPDIASPRÓX (`COUPDAYSNC`)

`CUPDIASPRÓX(liquidação; vencimento; frequência; [base])`

Dias da liquidação até o próximo cupom.

Outros apelidos aceitos: `CUPDIASPROX`

### CUPDATAPRÓX (`COUPNCD`)

`CUPDATAPRÓX(liquidação; vencimento; frequência; [base])`

Data do próximo cupom depois da liquidação.

Outros apelidos aceitos: `CUPDATAPROX`

### CUPNÚM (`COUPNUM`)

`CUPNÚM(liquidação; vencimento; frequência; [base])`

Quantos cupons faltam pagar entre a liquidação e o vencimento.

Outros apelidos aceitos: `CUPNUM`

### CUPDATAANT (`COUPPCD`)

`CUPDATAANT(liquidação; vencimento; frequência; [base])`

Data do último cupom antes da liquidação.

### DURAÇÃO (`DURATION`)

`DURAÇÃO(liquidação; vencimento; cupom; lucro; frequência; [base])`

Duração de Macaulay de um título com cupom, em anos.

Outros apelidos aceitos: `DURACAO`

### MDURAÇÃO (`MDURATION`)

`MDURAÇÃO(liquidação; vencimento; cupom; lucro; frequência; [base])`

Duração modificada (Macaulay dividida por 1 + lucro/frequência).

Outros apelidos aceitos: `MDURACAO`

### PREÇOPRIMINC (`ODDFPRICE`)

`PREÇOPRIMINC(liquidação; vencimento; emissão; primeiro_cupom; taxa; lucro; resgate; frequência; [base])`

Preço por 100 de um título com primeiro período irregular.

Outros apelidos aceitos: `PRECOPRIMINC`

### LUCROPRIMINC (`ODDFYIELD`)

`LUCROPRIMINC(liquidação; vencimento; emissão; primeiro_cupom; taxa; preço; resgate; frequência; [base])`

Rendimento de um título com primeiro período irregular.

### PREÇOÚLTINC (`ODDLPRICE`)

`PREÇOÚLTINC(liquidação; vencimento; último_juro; taxa; lucro; resgate; frequência; [base])`

Preço por 100 de um título com último período irregular.

Outros apelidos aceitos: `PRECOULTINC`

### LUCROÚLTINC (`ODDLYIELD`)

`LUCROÚLTINC(liquidação; vencimento; último_juro; taxa; preço; resgate; frequência; [base])`

Rendimento de um título com último período irregular.

Outros apelidos aceitos: `LUCROULTINC`

### PREÇO (`PRICE`)

`PREÇO(liquidação; vencimento; taxa; lucro; resgate; frequência; [base])`

Preço por 100 de valor nominal de um título que paga juros periódicos. Ex.: =PREÇO(DATA(2008;2;15);DATA(2017;11;15);5,75%;6,5%;100;2) dá 94,63.

Outros apelidos aceitos: `PRECO`

### LUCRO (`YIELD`)

`LUCRO(liquidação; vencimento; taxa; preço; resgate; frequência; [base])`

Rendimento de um título que paga juros periódicos.

### EUROCONVERT

`EUROCONVERT(núm; origem; destino; [precisão_total]; [precisão_triangulação])`

Converte entre o euro e as moedas antigas da zona do euro pelas taxas fixas ("DEM", "FRF", "PTE"…).
