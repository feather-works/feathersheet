use crate::funcoes::function_help::FunctionHelp;

pub static HELP: &[(&str, FunctionHelp)] = &[
    // matemática
    (
        "SUBTOTAL",
        FunctionHelp {
            args: &["função", "ref1", "[ref2]", "…"],
            doc: "Subtotal de um intervalo ignorando subtotais aninhados e linhas ocultas (101–111 e 1–11; linhas ocultas ou filtradas ficam de fora). 9/109 soma, 1/101 média, 2/102 contar números, 3/103 contar valores, 4/104 máx, 5/105 mín, 7/107 desvio padrão, 10/110 variância.",
        },
    ),
    (
        "AGGREGATE",
        FunctionHelp {
            args: &["função", "opções", "ref1", "[k]"],
            doc: "Agrega com opções para ignorar linhas ocultas, erros e subtotais aninhados (opções 0–7). Funções 1–19; de 14 a 19 usa k (MAIOR, MENOR, PERCENTIL, QUARTIL).",
        },
    ),
    (
        "SUM",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Soma os números ou intervalos informados.",
        },
    ),
    (
        "PRODUCT",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Multiplica todos os números informados.",
        },
    ),
    (
        "AVERAGE",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Média aritmética dos números.",
        },
    ),
    (
        "MEDIAN",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Valor central do conjunto ordenado.",
        },
    ),
    (
        "MIN",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Menor número do conjunto.",
        },
    ),
    (
        "MAX",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Maior número do conjunto.",
        },
    ),
    (
        "COUNT",
        FunctionHelp {
            args: &["valor1", "[valor2]", "…"],
            doc: "Conta quantas células contêm números.",
        },
    ),
    (
        "COUNTA",
        FunctionHelp {
            args: &["valor1", "[valor2]", "…"],
            doc: "Conta as células que não estão vazias.",
        },
    ),
    (
        "COUNTBLANK",
        FunctionHelp {
            args: &["intervalo"],
            doc: "Conta as células vazias do intervalo.",
        },
    ),
    (
        "ABS",
        FunctionHelp {
            args: &["núm"],
            doc: "Valor absoluto (sem sinal).",
        },
    ),
    (
        "SQRT",
        FunctionHelp {
            args: &["núm"],
            doc: "Raiz quadrada.",
        },
    ),
    (
        "EXP",
        FunctionHelp {
            args: &["núm"],
            doc: "e elevado a núm.",
        },
    ),
    (
        "LN",
        FunctionHelp {
            args: &["núm"],
            doc: "Logaritmo natural.",
        },
    ),
    (
        "LOG",
        FunctionHelp {
            args: &["núm", "[base]"],
            doc: "Logaritmo na base indicada (padrão 10).",
        },
    ),
    (
        "LOG10",
        FunctionHelp {
            args: &["núm"],
            doc: "Logaritmo na base 10.",
        },
    ),
    (
        "SIGN",
        FunctionHelp {
            args: &["núm"],
            doc: "Devolve -1, 0 ou 1 conforme o sinal.",
        },
    ),
    (
        "SIN",
        FunctionHelp {
            args: &["ângulo"],
            doc: "Seno de um ângulo em radianos.",
        },
    ),
    (
        "COS",
        FunctionHelp {
            args: &["ângulo"],
            doc: "Cosseno de um ângulo em radianos.",
        },
    ),
    (
        "TAN",
        FunctionHelp {
            args: &["ângulo"],
            doc: "Tangente de um ângulo em radianos.",
        },
    ),
    (
        "ASIN",
        FunctionHelp {
            args: &["núm"],
            doc: "Arco seno, em radianos.",
        },
    ),
    (
        "ACOS",
        FunctionHelp {
            args: &["núm"],
            doc: "Arco cosseno, em radianos.",
        },
    ),
    (
        "ATAN",
        FunctionHelp {
            args: &["núm"],
            doc: "Arco tangente, em radianos.",
        },
    ),
    (
        "ATAN2",
        FunctionHelp {
            args: &["x", "y"],
            doc: "Arco tangente das coordenadas x e y, em radianos.",
        },
    ),
    (
        "SINH",
        FunctionHelp {
            args: &["núm"],
            doc: "Seno hiperbólico.",
        },
    ),
    (
        "COSH",
        FunctionHelp {
            args: &["núm"],
            doc: "Cosseno hiperbólico.",
        },
    ),
    (
        "TANH",
        FunctionHelp {
            args: &["núm"],
            doc: "Tangente hiperbólica.",
        },
    ),
    (
        "RADIANS",
        FunctionHelp {
            args: &["graus"],
            doc: "Converte graus em radianos.",
        },
    ),
    (
        "DEGREES",
        FunctionHelp {
            args: &["radianos"],
            doc: "Converte radianos em graus.",
        },
    ),
    (
        "INT",
        FunctionHelp {
            args: &["núm"],
            doc: "Arredonda para baixo até o inteiro mais próximo.",
        },
    ),
    (
        "ROUND",
        FunctionHelp {
            args: &["núm", "[dígitos]"],
            doc: "Arredonda para o número de casas decimais.",
        },
    ),
    (
        "ROUNDUP",
        FunctionHelp {
            args: &["núm", "[dígitos]"],
            doc: "Arredonda para cima, longe do zero.",
        },
    ),
    (
        "ROUNDDOWN",
        FunctionHelp {
            args: &["núm", "[dígitos]"],
            doc: "Arredonda para baixo, em direção ao zero.",
        },
    ),
    (
        "TRUNC",
        FunctionHelp {
            args: &["núm", "[dígitos]"],
            doc: "Corta as casas decimais sem arredondar.",
        },
    ),
    (
        "MOD",
        FunctionHelp {
            args: &["núm", "divisor"],
            doc: "Resto da divisão.",
        },
    ),
    (
        "ISEVEN",
        FunctionHelp {
            args: &["núm"],
            doc: "VERDADEIRO quando o número é par.",
        },
    ),
    (
        "ISODD",
        FunctionHelp {
            args: &["núm"],
            doc: "VERDADEIRO quando o número é ímpar.",
        },
    ),
    (
        "POWER",
        FunctionHelp {
            args: &["núm", "expoente"],
            doc: "Eleva núm ao expoente.",
        },
    ),
    (
        "CEILING",
        FunctionHelp {
            args: &["núm", "[múltiplo]"],
            doc: "Arredonda para cima até o múltiplo.",
        },
    ),
    (
        "FLOOR",
        FunctionHelp {
            args: &["núm", "[múltiplo]"],
            doc: "Arredonda para baixo até o múltiplo.",
        },
    ),
    (
        "PI",
        FunctionHelp {
            args: &[],
            doc: "O número π.",
        },
    ),
    (
        "RAND",
        FunctionHelp {
            args: &[],
            doc: "Número aleatório entre 0 e 1. Recalcula a cada alteração.",
        },
    ),
    (
        "RANDBETWEEN",
        FunctionHelp {
            args: &["mín", "máx"],
            doc: "Inteiro aleatório entre os limites.",
        },
    ),
    (
        "STDEV",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Desvio padrão de uma amostra.",
        },
    ),
    (
        "STDEV.P",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Desvio padrão da população inteira.",
        },
    ),
    (
        "VAR",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Variância de uma amostra.",
        },
    ),
    (
        "VAR.P",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Variância da população inteira.",
        },
    ),
    (
        "SUMPRODUCT",
        FunctionHelp {
            args: &["matriz1", "[matriz2]", "…"],
            doc: "Multiplica as matrizes item a item e soma os produtos.",
        },
    ),
    (
        "SUMIF",
        FunctionHelp {
            args: &["intervalo", "critério", "[soma_intervalo]"],
            doc: "Soma só as células que atendem ao critério.",
        },
    ),
    (
        "SUMIFS",
        FunctionHelp {
            args: &["soma_intervalo", "intervalo1", "critério1", "…"],
            doc: "Soma quando todos os critérios batem ao mesmo tempo.",
        },
    ),
    (
        "TEXTJOIN",
        FunctionHelp {
            args: &["delimitador", "ignorar_vazio", "texto1", "…"],
            doc: "Junta textos com um delimitador.",
        },
    ),
    (
        "TEXTSPLIT",
        FunctionHelp {
            args: &["texto", "delimitador_coluna", "[delimitador_linha]"],
            doc: "Divide o texto em colunas e linhas.",
        },
    ),
    (
        "JOIN",
        FunctionHelp {
            args: &["delimitador", "valor1", "…"],
            doc: "Junta valores com um delimitador.",
        },
    ),
    (
        "REGEXEXTRACT",
        FunctionHelp {
            args: &["texto", "expressao"],
            doc: "Extrai o trecho que casa com a expressão regular.",
        },
    ),
    (
        "REGEXMATCH",
        FunctionHelp {
            args: &["texto", "expressao"],
            doc: "VERDADEIRO se o texto casa com a expressão regular.",
        },
    ),
    (
        "REGEXREPLACE",
        FunctionHelp {
            args: &["texto", "expressao", "substituicao"],
            doc: "Substitui o padrão encontrado pela expressão regular.",
        },
    ),
    (
        "EOMONTH",
        FunctionHelp {
            args: &["data_inicio", "meses"],
            doc: "Devolve a data do último dia do mês.",
        },
    ),
    (
        "EDATE",
        FunctionHelp {
            args: &["data_inicio", "meses"],
            doc: "Adiciona ou subtrai meses a uma data.",
        },
    ),
    (
        "NETWORKDAYS",
        FunctionHelp {
            args: &["data_inicio", "data_fim", "[feriados]"],
            doc: "Conta os dias úteis entre duas datas.",
        },
    ),
    (
        "WORKDAY",
        FunctionHelp {
            args: &["data_inicio", "dias", "[feriados]"],
            doc: "Calcula a data após N dias úteis.",
        },
    ),
    (
        "WEEKNUM",
        FunctionHelp {
            args: &["data", "[tipo]"],
            doc: "Devolve o número da semana no ano.",
        },
    ),
    (
        "SWITCH",
        FunctionHelp {
            args: &["expressao", "caso1", "valor1", "…", "[padrao]"],
            doc: "Compara a expressão com opções e devolve o valor do caso.",
        },
    ),
    (
        "IFNA",
        FunctionHelp {
            args: &["valor", "valor_se_na"],
            doc: "Devolve valor_se_na se o resultado for #N/A.",
        },
    ),
    (
        "LAMBDA",
        FunctionHelp {
            args: &["param1", "…", "formula"],
            doc: "Cria uma função personalizada reutilizável.",
        },
    ),
    (
        "LET",
        FunctionHelp {
            args: &["nome1", "valor1", "…", "formula"],
            doc: "Define variáveis locais para uso na fórmula.",
        },
    ),
    (
        "MAP",
        FunctionHelp {
            args: &["matriz", "lambda"],
            doc: "Aplica a função lambda a cada elemento da matriz.",
        },
    ),
    (
        "REDUCE",
        FunctionHelp {
            args: &["inicial", "matriz", "lambda"],
            doc: "Reduz a matriz a um valor acumulado usando lambda.",
        },
    ),
    (
        "COUNTIFS",
        FunctionHelp {
            args: &["intervalo1", "critério1", "…"],
            doc: "Conta as linhas que atendem a todos os critérios.",
        },
    ),
    (
        "AVERAGEIFS",
        FunctionHelp {
            args: &["média_intervalo", "intervalo1", "critério1", "…"],
            doc: "Média das linhas que atendem a todos os critérios.",
        },
    ),
    (
        "MAXIFS",
        FunctionHelp {
            args: &["máx_intervalo", "intervalo1", "critério1", "…"],
            doc: "Maior valor entre as linhas que atendem aos critérios.",
        },
    ),
    (
        "MINIFS",
        FunctionHelp {
            args: &["mín_intervalo", "intervalo1", "critério1", "…"],
            doc: "Menor valor entre as linhas que atendem aos critérios.",
        },
    ),
    (
        "MODE",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Valor que mais se repete.",
        },
    ),
    (
        "LARGE",
        FunctionHelp {
            args: &["intervalo", "k"],
            doc: "O k-ésimo maior valor.",
        },
    ),
    (
        "SMALL",
        FunctionHelp {
            args: &["intervalo", "k"],
            doc: "O k-ésimo menor valor.",
        },
    ),
    (
        "PERCENTILE",
        FunctionHelp {
            args: &["intervalo", "p"],
            doc: "Valor abaixo do qual está a fração p dos dados (p entre 0 e 1).",
        },
    ),
    (
        "QUARTILE",
        FunctionHelp {
            args: &["intervalo", "quarto"],
            doc: "Quartil: 0 mínimo, 1, 2 mediana, 3, 4 máximo.",
        },
    ),
    (
        "RANK",
        FunctionHelp {
            args: &["valor", "intervalo", "[crescente]"],
            doc: "Posição do valor na lista ordenada.",
        },
    ),
    (
        "AVEDEV",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Desvio médio absoluto em relação à média.",
        },
    ),
    (
        "DEVSQ",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Soma dos quadrados dos desvios.",
        },
    ),
    (
        "GEOMEAN",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Média geométrica; serve para taxas de crescimento.",
        },
    ),
    (
        "HARMEAN",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Média harmônica; serve para médias de velocidade.",
        },
    ),
    (
        "SKEW",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Assimetria da distribuição.",
        },
    ),
    (
        "KURT",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Curtose: o quanto a distribuição é pontuda.",
        },
    ),
    (
        "SUMBYCOLOR",
        FunctionHelp {
            args: &["intervalo", "célula_cor", "[tipo]"],
            doc: "Soma os números do intervalo cuja cor é a da célula de amostra (ou um texto \"#RRGGBB\"). Tipo 0 (padrão) compara o preenchimento; 1, a cor da fonte. Não vê a formatação condicional. Não existe no Excel.",
        },
    ),
    (
        "COUNTBYCOLOR",
        FunctionHelp {
            args: &["intervalo", "célula_cor", "[tipo]"],
            doc: "Conta as células com conteúdo no intervalo cuja cor é a da célula de amostra (ou um texto \"#RRGGBB\"). Tipo 0 (padrão) compara o preenchimento; 1, a cor da fonte. Não vê a formatação condicional. Não existe no Excel.",
        },
    ),
    (
        "COUNTUNIQUE",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Quantos valores diferentes existem.",
        },
    ),
    (
        "CORREL",
        FunctionHelp {
            args: &["y", "x"],
            doc: "Correlação entre os dois conjuntos, de -1 a 1.",
        },
    ),
    (
        "COVAR",
        FunctionHelp {
            args: &["y", "x"],
            doc: "Covariância entre os dois conjuntos.",
        },
    ),
    (
        "SLOPE",
        FunctionHelp {
            args: &["y", "x"],
            doc: "Inclinação da reta que melhor descreve os pontos.",
        },
    ),
    (
        "INTERCEPT",
        FunctionHelp {
            args: &["y", "x"],
            doc: "Onde essa reta cruza o eixo vertical.",
        },
    ),
    (
        "RSQ",
        FunctionHelp {
            args: &["y", "x"],
            doc: "R² : o quanto a reta explica os dados.",
        },
    ),
    (
        "FORECAST",
        FunctionHelp {
            args: &["x", "y_conhecidos", "x_conhecidos"],
            doc: "Projeta um valor usando a reta de tendência.",
        },
    ),
    (
        "FORECAST.LINEAR",
        FunctionHelp {
            args: &["x", "y_conhecidos", "x_conhecidos"],
            doc: "Projeta um valor usando a reta de tendência.",
        },
    ),
    (
        "AVERAGEA",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Média, contando texto como 0 e VERDADEIRO como 1.",
        },
    ),
    (
        "MAXA",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Maior valor, contando texto como 0 e VERDADEIRO como 1.",
        },
    ),
    (
        "MINA",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Menor valor, contando texto como 0 e VERDADEIRO como 1.",
        },
    ),
    (
        "STDEVA",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Desvio padrão amostral, contando texto como 0 e lógicos como 1/0.",
        },
    ),
    (
        "STDEVPA",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Desvio padrão populacional, contando texto como 0 e lógicos como 1/0.",
        },
    ),
    (
        "VARA",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Variância amostral, contando texto como 0 e lógicos como 1/0.",
        },
    ),
    (
        "VARPA",
        FunctionHelp {
            args: &["valor1", "…"],
            doc: "Variância populacional, contando texto como 0 e lógicos como 1/0.",
        },
    ),
    (
        "RANK.EQ",
        FunctionHelp {
            args: &["núm", "ref", "[ordem]"],
            doc: "Posição de um número na lista; empates recebem a posição mais alta.",
        },
    ),
    (
        "RANK.AVG",
        FunctionHelp {
            args: &["núm", "ref", "[ordem]"],
            doc: "Posição de um número na lista; empates recebem a média das posições.",
        },
    ),
    (
        "PERCENTILE.EXC",
        FunctionHelp {
            args: &["matriz", "k"],
            doc: "k-ésimo percentil, com k estritamente entre 0 e 1.",
        },
    ),
    (
        "QUARTILE.EXC",
        FunctionHelp {
            args: &["matriz", "quarto"],
            doc: "Quartil (1 a 3) de um conjunto de dados, pelo método exclusivo.",
        },
    ),
    (
        "PERCENTRANK.INC",
        FunctionHelp {
            args: &["matriz", "x", "[signif]"],
            doc: "Posição relativa de um valor, de 0 a 1 inclusive.",
        },
    ),
    (
        "PERCENTRANK.EXC",
        FunctionHelp {
            args: &["matriz", "x", "[signif]"],
            doc: "Posição relativa de um valor, estritamente entre 0 e 1.",
        },
    ),
    (
        "COVARIANCE.P",
        FunctionHelp {
            args: &["matriz1", "matriz2"],
            doc: "Covariância populacional de pares de valores.",
        },
    ),
    (
        "COVARIANCE.S",
        FunctionHelp {
            args: &["matriz1", "matriz2"],
            doc: "Covariância amostral de pares de valores.",
        },
    ),
    (
        "PEARSON",
        FunctionHelp {
            args: &["matriz1", "matriz2"],
            doc: "Coeficiente de correlação de Pearson.",
        },
    ),
    (
        "STANDARDIZE",
        FunctionHelp {
            args: &["x", "média", "desv_padrão"],
            doc: "Valor padronizado (escore z).",
        },
    ),
    (
        "TRIMMEAN",
        FunctionHelp {
            args: &["matriz", "proporção"],
            doc: "Média descartando uma fração de pontos nas duas extremidades.",
        },
    ),
    (
        "SKEW.P",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Assimetria populacional da distribuição.",
        },
    ),
    (
        "MODE.MULT",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Matriz vertical com todas as modas (valores mais frequentes).",
        },
    ),
    (
        "FISHER",
        FunctionHelp {
            args: &["x"],
            doc: "Transformação de Fisher.",
        },
    ),
    (
        "DSUM",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Soma os números de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DAVERAGE",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Média dos números de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DCOUNT",
        FunctionHelp {
            args: &["banco", "[campo]", "critérios"],
            doc: "Conta as células numéricas de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DCOUNTA",
        FunctionHelp {
            args: &["banco", "[campo]", "critérios"],
            doc: "Conta as células não vazias de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DGET",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Devolve o valor do campo do único registro que atende aos critérios.",
        },
    ),
    (
        "DMAX",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Maior número de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DMIN",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Menor número de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DPRODUCT",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Multiplica os números de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DSTDEV",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Desvio padrão amostral de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DSTDEVP",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Desvio padrão populacional de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DVAR",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Variância amostral de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "DVARP",
        FunctionHelp {
            args: &["banco", "campo", "critérios"],
            doc: "Variância populacional de um campo dos registros que atendem aos critérios.",
        },
    ),
    (
        "FISHERINV",
        FunctionHelp {
            args: &["y"],
            doc: "Inversa da transformação de Fisher.",
        },
    ),
    (
        "EVEN",
        FunctionHelp {
            args: &["núm"],
            doc: "Arredonda para cima, afastando de zero, até o inteiro par mais próximo.",
        },
    ),
    (
        "ODD",
        FunctionHelp {
            args: &["núm"],
            doc: "Arredonda para cima, afastando de zero, até o inteiro ímpar mais próximo.",
        },
    ),
    (
        "GCD",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Máximo divisor comum dos números.",
        },
    ),
    (
        "LCM",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Mínimo múltiplo comum dos números.",
        },
    ),
    (
        "MROUND",
        FunctionHelp {
            args: &["núm", "múltiplo"],
            doc: "Arredonda para o múltiplo mais próximo.",
        },
    ),
    (
        "QUOTIENT",
        FunctionHelp {
            args: &["numerador", "denominador"],
            doc: "Parte inteira de uma divisão.",
        },
    ),
    (
        "SUMSQ",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Soma dos quadrados dos números.",
        },
    ),
    (
        "SQRTPI",
        FunctionHelp {
            args: &["núm"],
            doc: "Raiz quadrada de (núm × π).",
        },
    ),
    (
        "FACTDOUBLE",
        FunctionHelp {
            args: &["núm"],
            doc: "Fatorial duplo: núm × (núm − 2) × (núm − 4) …",
        },
    ),
    (
        "ROMAN",
        FunctionHelp {
            args: &["núm", "[forma]"],
            doc: "Converte um número em algarismos romanos (texto).",
        },
    ),
    (
        "ACOSH",
        FunctionHelp { args: &["núm"], doc: "Cosseno hiperbólico inverso." },
    ),
    (
        "ASINH",
        FunctionHelp { args: &["núm"], doc: "Seno hiperbólico inverso." },
    ),
    (
        "ATANH",
        FunctionHelp { args: &["núm"], doc: "Tangente hiperbólica inversa." },
    ),
    (
        "ACOT",
        FunctionHelp { args: &["núm"], doc: "Arco-cotangente em radianos (0 a pi)." },
    ),
    (
        "ACOTH",
        FunctionHelp { args: &["núm"], doc: "Cotangente hiperbólica inversa." },
    ),
    (
        "COT",
        FunctionHelp { args: &["núm"], doc: "Cotangente de um ângulo em radianos." },
    ),
    (
        "COTH",
        FunctionHelp { args: &["núm"], doc: "Cotangente hiperbólica." },
    ),
    (
        "CSC",
        FunctionHelp { args: &["núm"], doc: "Cossecante de um ângulo em radianos." },
    ),
    (
        "CSCH",
        FunctionHelp { args: &["núm"], doc: "Cossecante hiperbólica." },
    ),
    (
        "SEC",
        FunctionHelp { args: &["núm"], doc: "Secante de um ângulo em radianos." },
    ),
    (
        "SECH",
        FunctionHelp { args: &["núm"], doc: "Secante hiperbólica." },
    ),
    (
        "BASE",
        FunctionHelp {
            args: &["núm", "raiz", "[comprimento_mínimo]"],
            doc: "Escreve um número em outra base, com zeros à esquerda.",
        },
    ),
    (
        "DECIMAL",
        FunctionHelp {
            args: &["texto", "raiz"],
            doc: "Lê um texto numa base dada e devolve o número.",
        },
    ),
    (
        "ARABIC",
        FunctionHelp {
            args: &["texto"],
            doc: "Converte algarismos romanos em número.",
        },
    ),
    (
        "CEILING.MATH",
        FunctionHelp {
            args: &["núm", "[sig]", "[modo]"],
            doc: "Arredonda para cima até o inteiro ou múltiplo mais próximo.",
        },
    ),
    (
        "CEILING.PRECISE",
        FunctionHelp {
            args: &["núm", "[sig]"],
            doc: "Arredonda para cima, em direção a mais infinito, até um múltiplo.",
        },
    ),
    (
        "ISO.CEILING",
        FunctionHelp {
            args: &["núm", "[sig]"],
            doc: "Arredonda para cima, em direção a mais infinito, até um múltiplo (ISO).",
        },
    ),
    (
        "FLOOR.MATH",
        FunctionHelp {
            args: &["núm", "[sig]", "[modo]"],
            doc: "Arredonda para baixo até o inteiro ou múltiplo mais próximo.",
        },
    ),
    (
        "FLOOR.PRECISE",
        FunctionHelp {
            args: &["núm", "[sig]"],
            doc: "Arredonda para baixo, em direção a menos infinito, até um múltiplo.",
        },
    ),
    (
        "COMBINA",
        FunctionHelp {
            args: &["núm", "núm_escolhido"],
            doc: "Número de combinações com repetição.",
        },
    ),
    (
        "MULTINOMIAL",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Razão entre o fatorial de uma soma e o produto dos fatoriais.",
        },
    ),
    (
        "MUNIT",
        FunctionHelp {
            args: &["dimensão"],
            doc: "Matriz identidade da dimensão dada.",
        },
    ),
    (
        "SERIESSUM",
        FunctionHelp {
            args: &["x", "n", "m", "coeficientes"],
            doc: "Soma de uma série de potências.",
        },
    ),
    (
        "SUMX2MY2",
        FunctionHelp {
            args: &["matriz_x", "matriz_y"],
            doc: "Soma das diferenças dos quadrados de elementos correspondentes.",
        },
    ),
    (
        "SUMX2PY2",
        FunctionHelp {
            args: &["matriz_x", "matriz_y"],
            doc: "Soma das somas dos quadrados de elementos correspondentes.",
        },
    ),
    (
        "SUMXMY2",
        FunctionHelp {
            args: &["matriz_x", "matriz_y"],
            doc: "Soma dos quadrados das diferenças de elementos correspondentes.",
        },
    ),
    (
        "BIN2DEC",
        FunctionHelp {
            args: &["núm"],
            doc: "Converte um número binário em decimal.",
        },
    ),
    (
        "BIN2HEX",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número binário em hexadecimal.",
        },
    ),
    (
        "BIN2OCT",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número binário em octal.",
        },
    ),
    (
        "DEC2BIN",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número decimal em binário.",
        },
    ),
    (
        "DEC2HEX",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número decimal em hexadecimal.",
        },
    ),
    (
        "DEC2OCT",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número decimal em octal.",
        },
    ),
    (
        "HEX2BIN",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número hexadecimal em binário.",
        },
    ),
    (
        "HEX2DEC",
        FunctionHelp {
            args: &["núm"],
            doc: "Converte um número hexadecimal em decimal.",
        },
    ),
    (
        "HEX2OCT",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número hexadecimal em octal.",
        },
    ),
    (
        "OCT2BIN",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número octal em binário.",
        },
    ),
    (
        "OCT2DEC",
        FunctionHelp {
            args: &["núm"],
            doc: "Converte um número octal em decimal.",
        },
    ),
    (
        "OCT2HEX",
        FunctionHelp {
            args: &["núm", "[casas]"],
            doc: "Converte um número octal em hexadecimal.",
        },
    ),
    (
        "BITAND",
        FunctionHelp {
            args: &["núm1", "núm2"],
            doc: "E bit a bit de dois inteiros não negativos.",
        },
    ),
    (
        "BITOR",
        FunctionHelp {
            args: &["núm1", "núm2"],
            doc: "OU bit a bit de dois inteiros não negativos.",
        },
    ),
    (
        "BITXOR",
        FunctionHelp {
            args: &["núm1", "núm2"],
            doc: "OU exclusivo bit a bit de dois inteiros não negativos.",
        },
    ),
    (
        "BITLSHIFT",
        FunctionHelp {
            args: &["núm", "quant"],
            doc: "Desloca os bits de um número para a esquerda.",
        },
    ),
    (
        "BITRSHIFT",
        FunctionHelp {
            args: &["núm", "quant"],
            doc: "Desloca os bits de um número para a direita.",
        },
    ),
    (
        "DELTA",
        FunctionHelp {
            args: &["núm1", "[núm2]"],
            doc: "Retorna 1 quando os dois números são iguais, senão 0.",
        },
    ),
    (
        "GESTEP",
        FunctionHelp {
            args: &["núm", "[limite]"],
            doc: "Retorna 1 quando o número é maior ou igual ao limite, senão 0.",
        },
    ),
    (
        "HOUR",
        FunctionHelp {
            args: &["núm_série"],
            doc: "Devolve a hora (0 a 23) de um horário.",
        },
    ),
    (
        "MINUTE",
        FunctionHelp {
            args: &["núm_série"],
            doc: "Devolve o minuto (0 a 59) de um horário.",
        },
    ),
    (
        "SECOND",
        FunctionHelp {
            args: &["núm_série"],
            doc: "Devolve o segundo (0 a 59) de um horário.",
        },
    ),
    (
        "TIME",
        FunctionHelp {
            args: &["hora", "minuto", "segundo"],
            doc: "Monta um horário (fração do dia) a partir de hora, minuto e segundo.",
        },
    ),
    (
        "DATEVALUE",
        FunctionHelp {
            args: &["texto_data"],
            doc: "Converte uma data escrita como texto (dd/mm/aaaa) em número de série.",
        },
    ),
    (
        "TIMEVALUE",
        FunctionHelp {
            args: &["texto_hora"],
            doc: "Converte um horário escrito como texto (hh:mm:ss) em fração do dia.",
        },
    ),
    (
        "DAYS",
        FunctionHelp {
            args: &["data_final", "data_inicial"],
            doc: "Devolve o número de dias entre duas datas.",
        },
    ),
    (
        "DAYS360",
        FunctionHelp {
            args: &["data_inicial", "data_final", "[método]"],
            doc: "Conta os dias entre duas datas num ano de 360 dias (método americano ou europeu).",
        },
    ),
    (
        "ISOWEEKNUM",
        FunctionHelp {
            args: &["data"],
            doc: "Devolve o número da semana ISO 8601 de uma data.",
        },
    ),
    (
        "YEARFRAC",
        FunctionHelp {
            args: &["data_inicial", "data_final", "[base]"],
            doc: "Devolve a fração do ano entre duas datas, conforme a base de contagem (0 a 4).",
        },
    ),
    (
        "NETWORKDAYS.INTL",
        FunctionHelp {
            args: &["data_inicial", "data_final", "[fim_de_semana]", "[feriados]"],
            doc: "Conta os dias úteis entre duas datas, com fins de semana personalizáveis.",
        },
    ),
    (
        "WORKDAY.INTL",
        FunctionHelp {
            args: &["data_inicial", "dias", "[fim_de_semana]", "[feriados]"],
            doc: "Devolve a data após um número de dias úteis, com fins de semana personalizáveis.",
        },
    ),
    (
        "BETA.DIST",
        FunctionHelp {
            args: &["x", "alfa", "beta", "cumulativo", "[A]", "[B]"],
            doc: "Distribuição beta: densidade ou probabilidade acumulada.",
        },
    ),
    (
        "BETA.INV",
        FunctionHelp {
            args: &["prob", "alfa", "beta", "[A]", "[B]"],
            doc: "Inversa da distribuição beta acumulada.",
        },
    ),
    (
        "BINOM.DIST",
        FunctionHelp {
            args: &["núm_s", "tentativas", "prob_s", "cumulativo"],
            doc: "Probabilidade da distribuição binomial.",
        },
    ),
    (
        "BINOM.DIST.RANGE",
        FunctionHelp {
            args: &["tentativas", "prob_s", "núm_s", "[núm_s2]"],
            doc: "Probabilidade de o número de sucessos cair num intervalo.",
        },
    ),
    (
        "BINOM.INV",
        FunctionHelp {
            args: &["tentativas", "prob_s", "alfa"],
            doc: "Menor número de sucessos cuja probabilidade binomial acumulada atinge alfa.",
        },
    ),
    (
        "CHISQ.DIST.RT",
        FunctionHelp {
            args: &["x", "graus_liberdade"],
            doc: "Probabilidade qui-quadrado à direita.",
        },
    ),
    (
        "CHISQ.INV",
        FunctionHelp {
            args: &["prob", "graus_liberdade"],
            doc: "Inversa da distribuição qui-quadrado à esquerda.",
        },
    ),
    (
        "CHISQ.INV.RT",
        FunctionHelp {
            args: &["prob", "graus_liberdade"],
            doc: "Inversa da distribuição qui-quadrado à direita.",
        },
    ),
    (
        "EXPON.DIST",
        FunctionHelp {
            args: &["x", "lambda", "cumulativo"],
            doc: "Distribuição exponencial: densidade ou probabilidade acumulada.",
        },
    ),
    (
        "F.DIST",
        FunctionHelp {
            args: &["x", "graus_liberdade1", "graus_liberdade2", "cumulativo"],
            doc: "Distribuição F: densidade ou probabilidade à esquerda.",
        },
    ),
    (
        "F.DIST.RT",
        FunctionHelp {
            args: &["x", "graus_liberdade1", "graus_liberdade2"],
            doc: "Probabilidade F à direita.",
        },
    ),
    (
        "F.INV",
        FunctionHelp {
            args: &["prob", "graus_liberdade1", "graus_liberdade2"],
            doc: "Inversa da distribuição F à esquerda.",
        },
    ),
    (
        "F.INV.RT",
        FunctionHelp {
            args: &["prob", "graus_liberdade1", "graus_liberdade2"],
            doc: "Inversa da distribuição F à direita.",
        },
    ),
    (
        "GAMMA",
        FunctionHelp {
            args: &["núm"],
            doc: "A função gama.",
        },
    ),
    (
        "GAMMA.DIST",
        FunctionHelp {
            args: &["x", "alfa", "beta", "cumulativo"],
            doc: "Distribuição gama: densidade ou probabilidade acumulada.",
        },
    ),
    (
        "GAMMA.INV",
        FunctionHelp {
            args: &["prob", "alfa", "beta"],
            doc: "Inversa da distribuição gama acumulada.",
        },
    ),
    (
        "GAMMALN",
        FunctionHelp {
            args: &["x"],
            doc: "Logaritmo natural da função gama.",
        },
    ),
    (
        "GAMMALN.PRECISE",
        FunctionHelp {
            args: &["x"],
            doc: "Logaritmo natural da função gama.",
        },
    ),
    (
        "GAUSS",
        FunctionHelp {
            args: &["z"],
            doc: "Probabilidade de um valor normal padrão ficar entre a média e z.",
        },
    ),
    (
        "HYPGEOM.DIST",
        FunctionHelp {
            args: &["amostra_s", "núm_amostra", "pop_s", "núm_pop", "cumulativo"],
            doc: "Probabilidade hipergeométrica.",
        },
    ),
    (
        "LOGNORM.DIST",
        FunctionHelp {
            args: &["x", "média", "desv_pad", "cumulativo"],
            doc: "Distribuição lognormal: densidade ou probabilidade acumulada.",
        },
    ),
    (
        "LOGNORM.INV",
        FunctionHelp {
            args: &["prob", "média", "desv_pad"],
            doc: "Inversa da distribuição lognormal acumulada.",
        },
    ),
    (
        "NEGBINOM.DIST",
        FunctionHelp {
            args: &["núm_f", "núm_s", "prob_s", "cumulativo"],
            doc: "Probabilidade binomial negativa.",
        },
    ),
    (
        "NORM.S.INV",
        FunctionHelp {
            args: &["prob"],
            doc: "Inversa da distribuição normal padrão acumulada.",
        },
    ),
    (
        "PHI",
        FunctionHelp {
            args: &["x"],
            doc: "Densidade da distribuição normal padrão.",
        },
    ),
    (
        "POISSON.DIST",
        FunctionHelp {
            args: &["x", "média", "cumulativo"],
            doc: "Probabilidade de Poisson.",
        },
    ),
    (
        "T.DIST.2T",
        FunctionHelp {
            args: &["x", "graus_liberdade"],
            doc: "Probabilidade t de Student bicaudal.",
        },
    ),
    (
        "T.DIST.RT",
        FunctionHelp {
            args: &["x", "graus_liberdade"],
            doc: "Probabilidade t de Student à direita.",
        },
    ),
    (
        "T.INV",
        FunctionHelp {
            args: &["prob", "graus_liberdade"],
            doc: "Inversa da distribuição t de Student à esquerda.",
        },
    ),
    (
        "T.INV.2T",
        FunctionHelp {
            args: &["prob", "graus_liberdade"],
            doc: "Inversa da distribuição t de Student bicaudal.",
        },
    ),
    (
        "WEIBULL.DIST",
        FunctionHelp {
            args: &["x", "alfa", "beta", "cumulativo"],
            doc: "Distribuição de Weibull: densidade ou probabilidade acumulada.",
        },
    ),
    (
        "CONFIDENCE.NORM",
        FunctionHelp {
            args: &["alfa", "desv_pad", "tamanho"],
            doc: "Meia-largura do intervalo de confiança pela distribuição normal.",
        },
    ),
    (
        "CONFIDENCE.T",
        FunctionHelp {
            args: &["alfa", "desv_pad", "tamanho"],
            doc: "Meia-largura do intervalo de confiança pela distribuição t de Student.",
        },
    ),
    (
        "PERMUTATIONA",
        FunctionHelp {
            args: &["núm", "núm_escolhido"],
            doc: "Permutações com repetição.",
        },
    ),
    (
        "ERF",
        FunctionHelp {
            args: &["limite_inferior", "[limite_superior]"],
            doc: "Função de erro integrada de 0 a um limite, ou entre dois limites.",
        },
    ),
    (
        "ERF.PRECISE",
        FunctionHelp {
            args: &["x"],
            doc: "Função de erro integrada de 0 a x.",
        },
    ),
    (
        "ERFC",
        FunctionHelp {
            args: &["x"],
            doc: "Função de erro complementar, 1 - ERF(x).",
        },
    ),
    (
        "ERFC.PRECISE",
        FunctionHelp {
            args: &["x"],
            doc: "Função de erro complementar, 1 - ERF(x).",
        },
    ),
    (
        "BETADIST",
        FunctionHelp {
            args: &["x", "alfa", "beta", "[A]", "[B]"],
            doc: "Probabilidade beta acumulada (compatibilidade).",
        },
    ),
    (
        "HYPGEOMDIST",
        FunctionHelp {
            args: &["amostra_s", "núm_amostra", "pop_s", "núm_pop"],
            doc: "Probabilidade hipergeométrica de uma contagem exata (compatibilidade).",
        },
    ),
    (
        "LOGNORMDIST",
        FunctionHelp {
            args: &["x", "média", "desv_pad"],
            doc: "Probabilidade lognormal acumulada (compatibilidade).",
        },
    ),
    (
        "NEGBINOMDIST",
        FunctionHelp {
            args: &["núm_f", "núm_s", "prob_s"],
            doc: "Probabilidade binomial negativa (compatibilidade).",
        },
    ),
    (
        "NORMSDIST",
        FunctionHelp {
            args: &["z"],
            doc: "Probabilidade normal padrão acumulada (compatibilidade).",
        },
    ),
    (
        "TDIST",
        FunctionHelp {
            args: &["x", "graus_liberdade", "caudas"],
            doc: "Probabilidade t de Student com uma ou duas caudas (compatibilidade).",
        },
    ),
    (
        "BINOMDIST",
        FunctionHelp {
            args: &["núm_s", "tentativas", "prob_s", "cumulativo"],
            doc: "Probabilidade da distribuição binomial. (compatibilidade)",
        },
    ),
    (
        "CHIDIST",
        FunctionHelp {
            args: &["x", "graus_liberdade"],
            doc: "Probabilidade qui-quadrado à direita. (compatibilidade)",
        },
    ),
    (
        "CHIINV",
        FunctionHelp {
            args: &["prob", "graus_liberdade"],
            doc: "Inversa da distribuição qui-quadrado à direita. (compatibilidade)",
        },
    ),
    (
        "CRITBINOM",
        FunctionHelp {
            args: &["tentativas", "prob_s", "alfa"],
            doc: "Menor número de sucessos cuja probabilidade binomial acumulada atinge alfa. (compatibilidade)",
        },
    ),
    (
        "EXPONDIST",
        FunctionHelp {
            args: &["x", "lambda", "cumulativo"],
            doc: "Distribuição exponencial: densidade ou probabilidade acumulada. (compatibilidade)",
        },
    ),
    (
        "FDIST",
        FunctionHelp {
            args: &["x", "graus_liberdade1", "graus_liberdade2"],
            doc: "Probabilidade F à direita. (compatibilidade)",
        },
    ),
    (
        "FINV",
        FunctionHelp {
            args: &["prob", "graus_liberdade1", "graus_liberdade2"],
            doc: "Inversa da distribuição F à direita. (compatibilidade)",
        },
    ),
    (
        "GAMMADIST",
        FunctionHelp {
            args: &["x", "alfa", "beta", "cumulativo"],
            doc: "Distribuição gama: densidade ou probabilidade acumulada. (compatibilidade)",
        },
    ),
    (
        "GAMMAINV",
        FunctionHelp {
            args: &["prob", "alfa", "beta"],
            doc: "Inversa da distribuição gama acumulada. (compatibilidade)",
        },
    ),
    (
        "LOGINV",
        FunctionHelp {
            args: &["prob", "média", "desv_pad"],
            doc: "Inversa da distribuição lognormal acumulada. (compatibilidade)",
        },
    ),
    (
        "NORMSINV",
        FunctionHelp {
            args: &["prob"],
            doc: "Inversa da distribuição normal padrão acumulada. (compatibilidade)",
        },
    ),
    (
        "POISSON",
        FunctionHelp {
            args: &["x", "média", "cumulativo"],
            doc: "Probabilidade de Poisson. (compatibilidade)",
        },
    ),
    (
        "TINV",
        FunctionHelp {
            args: &["prob", "graus_liberdade"],
            doc: "Inversa da distribuição t de Student bicaudal. (compatibilidade)",
        },
    ),
    (
        "WEIBULL",
        FunctionHelp {
            args: &["x", "alfa", "beta", "cumulativo"],
            doc: "Distribuição de Weibull: densidade ou probabilidade acumulada. (compatibilidade)",
        },
    ),
    (
        "BETAINV",
        FunctionHelp {
            args: &["prob", "alfa", "beta", "[A]", "[B]"],
            doc: "Inversa da distribuição beta acumulada. (compatibilidade)",
        },
    ),
    (
        "CONFIDENCE",
        FunctionHelp {
            args: &["alfa", "desv_pad", "tamanho"],
            doc: "Meia-largura do intervalo de confiança pela distribuição normal. (compatibilidade)",
        },
    ),
    (
        "FACT",
        FunctionHelp {
            args: &["núm"],
            doc: "Fatorial.",
        },
    ),
    (
        "COMBIN",
        FunctionHelp {
            args: &["total", "escolhidos"],
            doc: "Quantas combinações sem repetição.",
        },
    ),
    (
        "PERMUT",
        FunctionHelp {
            args: &["total", "escolhidos"],
            doc: "Quantos arranjos, considerando a ordem.",
        },
    ),
    (
        "NORM.DIST",
        FunctionHelp {
            args: &["x", "média", "desv_padrão", "cumulativo"],
            doc: "Distribuição normal (densidade ou cumulativa).",
        },
    ),
    (
        "NORM.S.DIST",
        FunctionHelp {
            args: &["z", "cumulativo"],
            doc: "Distribuição normal padrão (média 0 e desvio padrão 1).",
        },
    ),
    (
        "NORM.INV",
        FunctionHelp {
            args: &["probabilidade", "média", "desv_padrão"],
            doc: "Inverso da distribuição normal para a probabilidade.",
        },
    ),
    (
        "T.DIST",
        FunctionHelp {
            args: &["x", "graus_liberdade", "cumulativo"],
            doc: "Distribuição t de Student.",
        },
    ),
    (
        "T.TEST",
        FunctionHelp {
            args: &["matriz1", "matriz2", "caudas", "tipo"],
            doc: "Devolve a probabilidade associada ao teste t de Student.",
        },
    ),
    (
        "CHISQ.DIST",
        FunctionHelp {
            args: &["x", "graus_liberdade", "cumulativo"],
            doc: "Distribuição qui-quadrado.",
        },
    ),
    (
        "CHISQ.TEST",
        FunctionHelp {
            args: &["matriz_real", "matriz_esperada"],
            doc: "Devolve a probabilidade do teste qui-quadrado de independência.",
        },
    ),
    (
        "FILTER",
        FunctionHelp {
            args: &["intervalo", "condição", "[se_vazio]"],
            doc: "Devolve só as linhas (ou colunas) em que a condição é verdadeira. O resultado derrama nas células vizinhas.",
        },
    ),
    (
        "UNIQUE",
        FunctionHelp {
            args: &["intervalo", "[por_coluna]"],
            doc: "Remove as repetições, mantendo a primeira ocorrência.",
        },
    ),
    (
        "SORT",
        FunctionHelp {
            args: &["intervalo", "[coluna]", "[crescente]"],
            doc: "Ordena as linhas do intervalo por uma coluna.",
        },
    ),
    (
        "TRANSPOSE",
        FunctionHelp {
            args: &["intervalo"],
            doc: "Troca linhas por colunas.",
        },
    ),
    (
        "SEQUENCE",
        FunctionHelp {
            args: &["linhas", "[colunas]", "[início]", "[passo]"],
            doc: "Gera uma sequência de números.",
        },
    ),
    (
        "COUNTIF",
        FunctionHelp {
            args: &["intervalo", "critério"],
            doc: "Conta as células que atendem ao critério.",
        },
    ),
    (
        "AVERAGEIF",
        FunctionHelp {
            args: &["intervalo", "critério", "[média_intervalo]"],
            doc: "Média das células que atendem ao critério.",
        },
    ),
    (
        "IF",
        FunctionHelp {
            args: &["teste", "se_verdadeiro", "[se_falso]"],
            doc: "Devolve um valor ou outro conforme o teste.",
        },
    ),
    (
        "IFS",
        FunctionHelp {
            args: &["teste1", "valor1", "…"],
            doc: "Testa condições em ordem e devolve a do primeiro verdadeiro.",
        },
    ),
    (
        "IFERROR",
        FunctionHelp {
            args: &["valor", "se_erro"],
            doc: "Substitui o resultado quando ele é um erro.",
        },
    ),
    (
        "AND",
        FunctionHelp {
            args: &["lógico1", "…"],
            doc: "Verdadeiro só se todos forem verdadeiros.",
        },
    ),
    (
        "OR",
        FunctionHelp {
            args: &["lógico1", "…"],
            doc: "Verdadeiro se pelo menos um for verdadeiro.",
        },
    ),
    (
        "NOT",
        FunctionHelp {
            args: &["lógico"],
            doc: "Inverte verdadeiro e falso.",
        },
    ),
    (
        "XOR",
        FunctionHelp {
            args: &["lógico1", "…"],
            doc: "Verdadeiro quando a quantidade de verdadeiros é ímpar.",
        },
    ),
    (
        "TRUE",
        FunctionHelp {
            args: &[],
            doc: "O valor lógico verdadeiro.",
        },
    ),
    (
        "FALSE",
        FunctionHelp {
            args: &[],
            doc: "O valor lógico falso.",
        },
    ),
    (
        "ISERROR",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se o valor for um erro.",
        },
    ),
    (
        "ISBLANK",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se a célula estiver vazia.",
        },
    ),
    (
        "ISNUMBER",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se o valor for número.",
        },
    ),
    (
        "ISTEXT",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se o valor for texto.",
        },
    ),
    (
        "ISERR",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se o valor for qualquer erro, exceto #N/D.",
        },
    ),
    (
        "ISNA",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se o valor for o erro #N/D.",
        },
    ),
    (
        "ISLOGICAL",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se o valor for VERDADEIRO ou FALSO.",
        },
    ),
    (
        "ISNONTEXT",
        FunctionHelp {
            args: &["valor"],
            doc: "Verdadeiro se o valor não for texto (inclusive vazio).",
        },
    ),
    (
        "ERROR.TYPE",
        FunctionHelp {
            args: &["valor_erro"],
            doc: "Número que identifica o tipo de erro (1 a 9); #N/D se não for erro.",
        },
    ),
    (
        "TYPE",
        FunctionHelp {
            args: &["valor"],
            doc: "Tipo do valor: 1 número, 2 texto, 4 lógico, 16 erro, 64 matriz.",
        },
    ),
    (
        "N",
        FunctionHelp {
            args: &["valor"],
            doc: "Converte o valor em número: número fica, VERDADEIRO vira 1, o resto vira 0.",
        },
    ),
    (
        "IMAGE",
        FunctionHelp {
            args: &["base64"],
            doc: "Mostra na célula a imagem guardada em base64 (ou num data URL).",
        },
    ),
    (
        "IMAGE.BASE64",
        FunctionHelp {
            args: &["imagem"],
            doc: "O base64 puro de uma imagem (da célula com IMAGEM ou de um data URL).",
        },
    ),
    (
        "CONCAT",
        FunctionHelp {
            args: &["texto1", "…"],
            doc: "Junta os textos em um só.",
        },
    ),
    (
        "CONCATENATE",
        FunctionHelp {
            args: &["texto1", "…"],
            doc: "Junta os argumentos em um só texto (função antiga: não aceita intervalo; use CONCAT).",
        },
    ),
    (
        "LEN",
        FunctionHelp {
            args: &["texto"],
            doc: "Quantidade de caracteres.",
        },
    ),
    (
        "UPPER",
        FunctionHelp {
            args: &["texto"],
            doc: "Converte para maiúsculas.",
        },
    ),
    (
        "LOWER",
        FunctionHelp {
            args: &["texto"],
            doc: "Converte para minúsculas.",
        },
    ),
    (
        "CHAR",
        FunctionHelp {
            args: &["núm"],
            doc: "Caractere do código (conjunto Windows-1252).",
        },
    ),
    (
        "CODE",
        FunctionHelp {
            args: &["texto"],
            doc: "Código Windows-1252 do primeiro caractere.",
        },
    ),
    (
        "CLEAN",
        FunctionHelp {
            args: &["texto"],
            doc: "Tira os caracteres de controle não imprimíveis do texto.",
        },
    ),
    (
        "EXACT",
        FunctionHelp {
            args: &["texto1", "texto2"],
            doc: "VERDADEIRO quando os dois textos são iguais, com maiúsculas e minúsculas.",
        },
    ),
    (
        "T",
        FunctionHelp {
            args: &["valor"],
            doc: "Devolve o valor se for texto; senão, texto vazio.",
        },
    ),
    (
        "REPLACE",
        FunctionHelp {
            args: &["texto", "início", "núm_caract", "novo_texto"],
            doc: "Troca um trecho do texto pela posição.",
        },
    ),
    (
        "UNICHAR",
        FunctionHelp {
            args: &["núm"],
            doc: "Caractere do ponto de código Unicode.",
        },
    ),
    (
        "UNICODE",
        FunctionHelp {
            args: &["texto"],
            doc: "Ponto de código Unicode do primeiro caractere.",
        },
    ),
    (
        "NUMBERVALUE",
        FunctionHelp {
            args: &["texto", "[sep_decimal]", "[sep_milhar]"],
            doc: "Converte texto em número, com separadores escolhidos (padrão pt-BR: vírgula decimal, ponto de milhar).",
        },
    ),
    (
        "FIXED",
        FunctionHelp {
            args: &["núm", "[decimais]", "[sem_milhar]"],
            doc: "Arredonda o número e o devolve como texto com casas decimais e separador de milhar.",
        },
    ),
    (
        "DOLLAR",
        FunctionHelp {
            args: &["núm", "[decimais]"],
            doc: "Formata o número como moeda em texto (R$).",
        },
    ),
    (
        "TEXTBEFORE",
        FunctionHelp {
            args: &["texto", "delim", "[inst]", "[modo]", "[fim]", "[se_não_achar]"],
            doc: "Devolve o texto antes do delimitador indicado.",
        },
    ),
    (
        "TEXTAFTER",
        FunctionHelp {
            args: &["texto", "delim", "[inst]", "[modo]", "[fim]", "[se_não_achar]"],
            doc: "Devolve o texto depois do delimitador indicado.",
        },
    ),
    (
        "VALUETOTEXT",
        FunctionHelp {
            args: &["valor", "[formato]"],
            doc: "Converte qualquer valor em texto; com formato 1, em estilo estrito (texto entre aspas).",
        },
    ),
    (
        "ARRAYTOTEXT",
        FunctionHelp {
            args: &["matriz", "[formato]"],
            doc: "Converte uma matriz em texto; com formato 1, em estilo estrito de matriz.",
        },
    ),
    (
        "REGEXTEST",
        FunctionHelp {
            args: &["texto", "padrão", "[sensível]"],
            doc: "VERDADEIRO quando a expressão regular encontra correspondência no texto.",
        },
    ),
    (
        "PROPER",
        FunctionHelp {
            args: &["texto"],
            doc: "Deixa a primeira letra de cada palavra maiúscula.",
        },
    ),
    (
        "TRIM",
        FunctionHelp {
            args: &["texto"],
            doc: "Remove espaços sobrando.",
        },
    ),
    (
        "LEFT",
        FunctionHelp {
            args: &["texto", "[qtd]"],
            doc: "Primeiros caracteres do texto.",
        },
    ),
    (
        "RIGHT",
        FunctionHelp {
            args: &["texto", "[qtd]"],
            doc: "Últimos caracteres do texto.",
        },
    ),
    (
        "MID",
        FunctionHelp {
            args: &["texto", "início", "qtd"],
            doc: "Trecho do texto a partir de uma posição.",
        },
    ),
    (
        "REPT",
        FunctionHelp {
            args: &["texto", "vezes"],
            doc: "Repete o texto n vezes.",
        },
    ),
    (
        "SUBSTITUTE",
        FunctionHelp {
            args: &["texto", "antigo", "novo", "[ocorrência]"],
            doc: "Troca um trecho por outro.",
        },
    ),
    (
        "FIND",
        FunctionHelp {
            args: &["procurado", "texto", "[início]"],
            doc: "Posição do trecho, diferenciando maiúsculas e sem curingas.",
        },
    ),
    (
        "SEARCH",
        FunctionHelp {
            args: &["procurado", "texto", "[início]"],
            doc: "Posição do trecho, ignorando maiúsculas; aceita ? e * como curingas.",
        },
    ),
    (
        "VALUE",
        FunctionHelp {
            args: &["texto"],
            doc: "Converte texto em número.",
        },
    ),
    (
        "TEXT",
        FunctionHelp {
            args: &["valor", "formato"],
            doc: "Formata um número como texto (ex.: \"R$ #.##0,00\").",
        },
    ),
    (
        "ROW",
        FunctionHelp {
            args: &["[ref]"],
            doc: "Número da linha da referência.",
        },
    ),
    (
        "COLUMN",
        FunctionHelp {
            args: &["[ref]"],
            doc: "Número da coluna da referência.",
        },
    ),
    (
        "ROWS",
        FunctionHelp {
            args: &["intervalo"],
            doc: "Quantidade de linhas do intervalo.",
        },
    ),
    (
        "COLUMNS",
        FunctionHelp {
            args: &["intervalo"],
            doc: "Quantidade de colunas do intervalo.",
        },
    ),
    (
        "CHOOSE",
        FunctionHelp {
            args: &["índice", "valor1", "…"],
            doc: "Escolhe um valor da lista pelo índice.",
        },
    ),
    (
        "INDEX",
        FunctionHelp {
            args: &["intervalo", "linha", "[coluna]"],
            doc: "Valor na posição indicada dentro do intervalo.",
        },
    ),
    (
        "MATCH",
        FunctionHelp {
            args: &["procurado", "intervalo", "[tipo]"],
            doc: "Posição do valor dentro do intervalo. Use 0 para exato.",
        },
    ),
    (
        "VLOOKUP",
        FunctionHelp {
            args: &["procurado", "intervalo", "índice_coluna", "[aproximado]"],
            doc: "Procura na primeira coluna e devolve o valor da coluna indicada. Use FALSO para busca exata.",
        },
    ),
    (
        "HLOOKUP",
        FunctionHelp {
            args: &["procurado", "intervalo", "índice_linha", "[aproximado]"],
            doc: "Procura na primeira linha e devolve o valor da linha indicada.",
        },
    ),
    (
        "XLOOKUP",
        FunctionHelp {
            args: &["procurado", "onde_procurar", "o_que_devolver", "[se_não_achar]", "[modo]"],
            doc: "Busca moderna: intervalos separados, sem índice numérico. Modo 0 exato, -1 menor mais próximo, 1 maior mais próximo, 2 curinga.",
        },
    ),
    (
        "FREQUENCY",
        FunctionHelp {
            args: &["dados", "classes"],
            doc: "Quantos dados caem em cada faixa. Derrama uma coluna com uma linha a mais que as classes: a última conta o que passou do maior limite.",
        },
    ),
    (
        "MMULT",
        FunctionHelp {
            args: &["matriz1", "matriz2"],
            doc: "Produto de duas matrizes. As colunas da primeira têm de bater com as linhas da segunda.",
        },
    ),
    (
        "MINVERSE",
        FunctionHelp {
            args: &["matriz"],
            doc: "Matriz inversa de uma matriz quadrada.",
        },
    ),
    (
        "MDETERM",
        FunctionHelp {
            args: &["matriz"],
            doc: "Determinante de uma matriz quadrada.",
        },
    ),
    (
        "TODAY",
        FunctionHelp {
            args: &[],
            doc: "A data de hoje.",
        },
    ),
    (
        "NOW",
        FunctionHelp {
            args: &[],
            doc: "Data e hora atuais.",
        },
    ),
    (
        "DATE",
        FunctionHelp {
            args: &["ano", "mês", "dia"],
            doc: "Monta uma data a partir das partes.",
        },
    ),
    (
        "DATEDIF",
        FunctionHelp {
            args: &["data_início", "data_fim", "unidade"],
            doc: "Diferença entre duas datas. Unidade: \"A\" anos, \"M\" meses, \"D\" dias, \"AM\" meses que sobram do ano, \"AD\" dias que sobram do ano, \"MD\" dias que sobram do mês.",
        },
    ),
    (
        "YEAR",
        FunctionHelp {
            args: &["data"],
            doc: "Ano de uma data.",
        },
    ),
    (
        "MONTH",
        FunctionHelp {
            args: &["data"],
            doc: "Mês de uma data (1 a 12).",
        },
    ),
    (
        "DAY",
        FunctionHelp {
            args: &["data"],
            doc: "Dia do mês de uma data.",
        },
    ),
    (
        "WEEKDAY",
        FunctionHelp {
            args: &["data"],
            doc: "Dia da semana (1 = domingo).",
        },
    ),
    (
        "IMPORTDATA",
        FunctionHelp {
            args: &["url", "[separador]"],
            doc: "Lê um CSV ou TSV de um endereço na internet e derrama a tabela na grade. O separador é descoberto sozinho, ou informe \";\" no segundo argumento.",
        },
    ),
    (
        "IMPORTJSON",
        FunctionHelp {
            args: &["url", "[caminho]"],
            doc: "Lê um JSON de um endereço e traz um pedaço dele. O caminho usa pontos e colchetes: \"dados.itens[0].nome\". Uma lista de objetos vira tabela com cabeçalho.",
        },
    ),
    (
        "FETCHTEXT",
        FunctionHelp {
            args: &["url"],
            doc: "Devolve o conteúdo de um endereço como texto cru, sem interpretar.",
        },
    ),
    // Financeiras (fn_fin.rs)
    (
        "PMT",
        FunctionHelp {
            args: &["taxa", "nper", "vp", "[vf]", "[tipo]"],
            doc: "Pagamento periódico de um empréstimo ou anuidade, com pagamentos e taxa constantes.",
        },
    ),
    (
        "FV",
        FunctionHelp {
            args: &["taxa", "nper", "pgto", "[vp]", "[tipo]"],
            doc: "Valor futuro de um investimento com pagamentos e taxa constantes.",
        },
    ),
    (
        "PV",
        FunctionHelp {
            args: &["taxa", "nper", "pgto", "[vf]", "[tipo]"],
            doc: "Valor presente de uma série de pagamentos iguais futuros.",
        },
    ),
    (
        "NPER",
        FunctionHelp {
            args: &["taxa", "pgto", "vp", "[vf]", "[tipo]"],
            doc: "Número de períodos necessários para quitar ou acumular um valor.",
        },
    ),
    (
        "IPMT",
        FunctionHelp {
            args: &["taxa", "per", "nper", "vp", "[vf]", "[tipo]"],
            doc: "Parte dos juros do pagamento de um período dado.",
        },
    ),
    (
        "PPMT",
        FunctionHelp {
            args: &["taxa", "per", "nper", "vp", "[vf]", "[tipo]"],
            doc: "Parte do principal do pagamento de um período dado.",
        },
    ),
    (
        "ISPMT",
        FunctionHelp {
            args: &["taxa", "per", "nper", "vp"],
            doc: "Juros pagos em um período de empréstimo com amortização de principal constante.",
        },
    ),
    (
        "SLN",
        FunctionHelp {
            args: &["custo", "valor_residual", "vida"],
            doc: "Depreciação linear de um período.",
        },
    ),
    (
        "SYD",
        FunctionHelp {
            args: &["custo", "valor_residual", "vida", "por"],
            doc: "Depreciação pela soma dos dígitos dos anos para um período.",
        },
    ),
    (
        "DB",
        FunctionHelp {
            args: &["custo", "valor_residual", "vida", "período", "[mês]"],
            doc: "Depreciação por saldo decrescente fixo para um período.",
        },
    ),
    (
        "DDB",
        FunctionHelp {
            args: &["custo", "valor_residual", "vida", "período", "[fator]"],
            doc: "Depreciação por saldo decrescente duplo (ou outro fator) para um período.",
        },
    ),
    (
        "VDB",
        FunctionHelp {
            args: &["custo", "valor_residual", "vida", "início", "fim", "[fator]", "[sem_troca]"],
            doc: "Depreciação por saldo decrescente em qualquer intervalo de períodos, trocando para linear.",
        },
    ),
    (
        "EFFECT",
        FunctionHelp {
            args: &["taxa_nominal", "npery"],
            doc: "Taxa anual efetiva a partir de uma taxa nominal e de períodos de capitalização.",
        },
    ),
    (
        "NOMINAL",
        FunctionHelp {
            args: &["taxa_efetiva", "npery"],
            doc: "Taxa anual nominal a partir de uma taxa efetiva e de períodos de capitalização.",
        },
    ),
    (
        "DOLLARDE",
        FunctionHelp {
            args: &["preço_fracionário", "fração"],
            doc: "Converte uma cotação em notação fracionária para número decimal.",
        },
    ),
    (
        "DOLLARFR",
        FunctionHelp {
            args: &["preço_decimal", "fração"],
            doc: "Converte uma cotação decimal para notação fracionária.",
        },
    ),
    (
        "RRI",
        FunctionHelp {
            args: &["nper", "vp", "vf"],
            doc: "Taxa de juros equivalente para o crescimento de um investimento.",
        },
    ),
    (
        "PDURATION",
        FunctionHelp {
            args: &["taxa", "vp", "vf"],
            doc: "Períodos necessários para um investimento crescer até o valor-alvo.",
        },
    ),
    (
        "NPV",
        FunctionHelp {
            args: &["taxa", "valor1", "[valor2]", "…"],
            doc: "Valor líquido presente de fluxos de caixa periódicos a uma taxa de desconto.",
        },
    ),
    (
        "RATE",
        FunctionHelp {
            args: &["nper", "pgto", "vp", "[vf]", "[tipo]", "[estimativa]"],
            doc: "Taxa de juros por período de uma anuidade.",
        },
    ),
    (
        "CUMIPMT",
        FunctionHelp {
            args: &["taxa", "nper", "vp", "início", "fim", "tipo"],
            doc: "Juros acumulados pagos entre dois períodos.",
        },
    ),
    (
        "CUMPRINC",
        FunctionHelp {
            args: &["taxa", "nper", "vp", "início", "fim", "tipo"],
            doc: "Capital acumulado pago entre dois períodos.",
        },
    ),
    (
        "XNPV",
        FunctionHelp {
            args: &["taxa", "valores", "datas"],
            doc: "Valor líquido presente de fluxos de caixa em datas específicas.",
        },
    ),
    (
        "XIRR",
        FunctionHelp {
            args: &["valores", "datas", "[estimativa]"],
            doc: "Taxa interna de retorno de fluxos de caixa em datas específicas.",
        },
    ),
    (
        "IRR",
        FunctionHelp {
            args: &["valores", "[estimativa]"],
            doc: "Taxa interna de retorno de fluxos de caixa periódicos.",
        },
    ),
    (
        "MIRR",
        FunctionHelp {
            args: &["valores", "taxa_financ", "taxa_reinvest"],
            doc: "Taxa interna de retorno modificada, com custo de financiamento e taxa de reinvestimento.",
        },
    ),
    (
        "FVSCHEDULE",
        FunctionHelp {
            args: &["principal", "plano"],
            doc: "Valor futuro de um principal após uma série de taxas de juros compostos.",
        },
    ),
    (
        "CHITEST",
        FunctionHelp {
            args: &["matriz_real", "matriz_esperada"],
            doc: "Nome antigo de CHISQ.TEST (TESTE.QUI): probabilidade do teste qui-quadrado de independência.",
        },
    ),
    (
        "TTEST",
        FunctionHelp {
            args: &["matriz1", "matriz2", "caudas", "tipo"],
            doc: "Nome antigo de T.TEST (TESTE.T): probabilidade associada ao teste t de Student.",
        },
    ),
    (
        "PERCENTOF",
        FunctionHelp {
            args: &["dados_subconjunto", "dados_total"],
            doc: "Soma do subconjunto dividida pela soma do total (fração do total). Total zero resulta em #DIV/0!.",
        },
    ),
    (
        "COMPLEX",
        FunctionHelp {
            args: &["núm_real", "núm_imag", "[sufixo]"],
            doc: "Monta um número complexo no texto a+bi, com sufixo i (padrão) ou j. Ex.: =COMPLEXO(3;4) devolve 3+4i.",
        },
    ),
    (
        "IMABS",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Módulo de um número complexo. Ex.: =IMABS(\"3+4i\") devolve 5.",
        },
    ),
    (
        "IMAGINARY",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Parte imaginária de um número complexo. Ex.: =IMAGINÁRIO(\"3+4i\") devolve 4.",
        },
    ),
    (
        "IMARGUMENT",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Ângulo, em radianos, do número complexo no plano complexo. Zero resulta em #DIV/0!. Ex.: =IMARG(\"i\") devolve 1,5707963267949.",
        },
    ),
    (
        "IMCONJUGATE",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Conjugado de um número complexo (troca o sinal da parte imaginária). Ex.: =IMCONJ(\"3+4i\") devolve 3-4i.",
        },
    ),
    (
        "IMCOS",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Cosseno de um número complexo. Ex.: =IMCOS(\"0\") devolve 1.",
        },
    ),
    (
        "IMCOSH",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Cosseno hiperbólico de um número complexo. Ex.: =IMCOSH(\"0\") devolve 1.",
        },
    ),
    (
        "IMCOT",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Cotangente de um número complexo (cosseno sobre seno). Zero do seno resulta em #NÚM!. Ex.: =IMCOT(\"1+i\").",
        },
    ),
    (
        "IMCSC",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Cossecante de um número complexo. Ex.: =IMCOSEC(\"1+i\"). Zero do seno resulta em #NÚM!.",
        },
    ),
    (
        "IMCSCH",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Cossecante hiperbólica de um número complexo. Ex.: =IMCOSECH(\"1+i\"). Zero do seno hiperbólico resulta em #NÚM!.",
        },
    ),
    (
        "IMDIV",
        FunctionHelp {
            args: &["núm1", "núm2"],
            doc: "Quociente de dois números complexos. Divisor zero resulta em #NÚM!. Ex.: =IMDIV(\"-238+240i\";\"10+24i\") devolve 5+12i.",
        },
    ),
    (
        "IMEXP",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Exponencial de um número complexo. Ex.: =IMEXP(\"0\") devolve 1.",
        },
    ),
    (
        "IMLN",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Logaritmo natural de um número complexo. Zero resulta em #NÚM!. Ex.: =IMLN(\"1\") devolve 0.",
        },
    ),
    (
        "IMLOG10",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Logaritmo de base 10 de um número complexo. Zero resulta em #NÚM!. Ex.: =IMLOG10(\"10\") devolve 1.",
        },
    ),
    (
        "IMLOG2",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Logaritmo de base 2 de um número complexo. Zero resulta em #NÚM!. Ex.: =IMLOG2(\"8\") devolve 3.",
        },
    ),
    (
        "IMPOWER",
        FunctionHelp {
            args: &["núm_complexo", "potência"],
            doc: "Eleva um número complexo a uma potência. Ex.: =IMPOT(\"2+3i\";3) devolve -46+9i.",
        },
    ),
    (
        "IMPRODUCT",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Multiplica todos os números complexos informados. Ex.: =IMPROD(\"3+4i\";\"5-3i\") devolve 27+11i.",
        },
    ),
    (
        "IMREAL",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Parte real de um número complexo. Ex.: =IMREAL(\"3+4i\") devolve 3.",
        },
    ),
    (
        "IMSEC",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Secante de um número complexo. Ex.: =IMSEC(\"0\") devolve 1.",
        },
    ),
    (
        "IMSECH",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Secante hiperbólica de um número complexo. Ex.: =IMSECH(\"0\") devolve 1.",
        },
    ),
    (
        "IMSIN",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Seno de um número complexo. Ex.: =IMSENO(\"0\") devolve 0.",
        },
    ),
    (
        "IMSINH",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Seno hiperbólico de um número complexo. Ex.: =IMSENH(\"0\") devolve 0.",
        },
    ),
    (
        "IMSQRT",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Raiz quadrada principal de um número complexo. Ex.: =IMRAIZ(\"-4\") devolve 2i.",
        },
    ),
    (
        "IMSUB",
        FunctionHelp {
            args: &["núm1", "núm2"],
            doc: "Diferença entre dois números complexos. Ex.: =IMSUBTR(\"13+4i\";\"5+3i\") devolve 8+i.",
        },
    ),
    (
        "IMSUM",
        FunctionHelp {
            args: &["núm1", "[núm2]", "…"],
            doc: "Soma os números complexos informados, inclusive em intervalos. Ex.: =IMSOMA(\"3+4i\";\"5-3i\") devolve 8+i.",
        },
    ),
    (
        "IMTAN",
        FunctionHelp {
            args: &["núm_complexo"],
            doc: "Tangente de um número complexo. Ex.: =IMTAN(\"0\") devolve 0.",
        },
    ),
    (
        "VSTACK",
        FunctionHelp {
            args: &["matriz1", "[matriz2]", "…"],
            doc: "Empilha matrizes na vertical, uma abaixo da outra; o que faltar na largura vira #N/D. Ex.: =EMPILHARV(A1:B2;D1:E1).",
        },
    ),
    (
        "HSTACK",
        FunctionHelp {
            args: &["matriz1", "[matriz2]", "…"],
            doc: "Junta matrizes na horizontal, lado a lado; o que faltar na altura vira #N/D. Ex.: =EMPILHARH(A1:A3;C1:C3).",
        },
    ),
    (
        "TAKE",
        FunctionHelp {
            args: &["matriz", "[linhas]", "[colunas]"],
            doc: "Devolve as primeiras (positivo) ou últimas (negativo) linhas e colunas da matriz. Ex.: =PEGAR(A1:C5;2) devolve as 2 primeiras linhas.",
        },
    ),
    (
        "DROP",
        FunctionHelp {
            args: &["matriz", "[linhas]", "[colunas]"],
            doc: "Descarta as primeiras (positivo) ou últimas (negativo) linhas e colunas da matriz. Ex.: =DESCARTAR(A1:C5;1) tira a primeira linha.",
        },
    ),
    (
        "CHOOSECOLS",
        FunctionHelp {
            args: &["matriz", "núm_col1", "[núm_col2]", "…"],
            doc: "Devolve só as colunas indicadas, na ordem pedida; negativo conta a partir da última. Ex.: =ESCOLHERCOLS(A1:D5;1;3).",
        },
    ),
    (
        "CHOOSEROWS",
        FunctionHelp {
            args: &["matriz", "núm_lin1", "[núm_lin2]", "…"],
            doc: "Devolve só as linhas indicadas, na ordem pedida; negativo conta a partir da última. Ex.: =ESCOLHERLINS(A1:D5;1;-1).",
        },
    ),
    (
        "TOCOL",
        FunctionHelp {
            args: &["matriz", "[ignorar]", "[verificar_por_coluna]"],
            doc: "Transforma a matriz numa só coluna. Ignorar: 0 nada, 1 vazios, 2 erros, 3 os dois. Ex.: =PARACOL(A1:C3).",
        },
    ),
    (
        "TOROW",
        FunctionHelp {
            args: &["matriz", "[ignorar]", "[verificar_por_coluna]"],
            doc: "Transforma a matriz numa só linha. Ignorar: 0 nada, 1 vazios, 2 erros, 3 os dois. Ex.: =PARALIN(A1:C3).",
        },
    ),
    (
        "WRAPROWS",
        FunctionHelp {
            args: &["vetor", "contagem", "[preencher_com]"],
            doc: "Quebra uma linha ou coluna em linhas de N valores; o que sobrar é preenchido (padrão #N/D). Ex.: =QUEBRARLINS(A1:A6;2).",
        },
    ),
    (
        "WRAPCOLS",
        FunctionHelp {
            args: &["vetor", "contagem", "[preencher_com]"],
            doc: "Quebra uma linha ou coluna em colunas de N valores; o que sobrar é preenchido (padrão #N/D). Ex.: =QUEBRARCOLS(A1:A6;2).",
        },
    ),
    (
        "EXPAND",
        FunctionHelp {
            args: &["matriz", "linhas", "[colunas]", "[preencher_com]"],
            doc: "Aumenta a matriz até o tamanho pedido, preenchendo o novo espaço (padrão #N/D). Ex.: =EXPANDIR(A1:B2;3;3;0).",
        },
    ),
    (
        "SORTBY",
        FunctionHelp {
            args: &["matriz", "por_matriz1", "[ordem1]", "[por_matriz2]", "[ordem2]", "…"],
            doc: "Ordena a matriz pelos valores de outra matriz. Ordem: 1 crescente (padrão), -1 decrescente. Ex.: =CLASSIFICARPOR(A2:B9;B2:B9;-1).",
        },
    ),
    (
        "RANDARRAY",
        FunctionHelp {
            args: &["[linhas]", "[colunas]", "[mín]", "[máx]", "[inteiro]"],
            doc: "Matriz de números aleatórios; com inteiro VERDADEIRO sorteia números inteiros entre mín e máx. Ex.: =MATRIZALEATÓRIA(3;2;1;10;VERDADEIRO).",
        },
    ),
    (
        "LOOKUP",
        FunctionHelp {
            args: &["valor_procurado", "vetor_proc", "[vetor_result]"],
            doc: "Procura o valor num vetor em ordem crescente e devolve o item correspondente do vetor de resultado (aproximada). Ex.: =PROC(5;A1:A9;B1:B9).",
        },
    ),
    (
        "XMATCH",
        FunctionHelp {
            args: &["valor_procurado", "matriz_procurada", "[modo_correspondência]", "[modo_pesquisa]"],
            doc: "Posição do valor numa linha ou coluna. Correspondência: 0 exata, -1 menor mais próximo, 1 maior mais próximo, 2 curinga. Pesquisa: 1 do início, -1 do fim. Ex.: =CORRESPX(\"b\";A1:A5).",
        },
    ),
    (
        "ADDRESS",
        FunctionHelp {
            args: &["lin", "col", "[abs]", "[a1]", "[planilha]"],
            doc: "Monta o endereço de uma célula como texto. Abs: 1 $A$1, 2 A$1, 3 $A1, 4 A1; a1 FALSO dá o estilo L1C1. Ex.: =ENDEREÇO(2;3) devolve $C$2.",
        },
    ),
    (
        "HYPERLINK",
        FunctionHelp {
            args: &["local_vínculo", "[nome_amigável]"],
            doc: "Mostra o nome amigável (ou o próprio endereço) como vínculo. Ex.: =HIPERLINK(\"https://exemplo.com\";\"Abrir\").",
        },
    ),
    (
        "ISREF",
        FunctionHelp {
            args: &["valor"],
            doc: "VERDADEIRO se o argumento é uma referência de célula ou intervalo. Ex.: =ÉREF(A1) devolve VERDADEIRO.",
        },
    ),
    (
        "STEYX",
        FunctionHelp {
            args: &["y_conhecidos", "x_conhecidos"],
            doc: "Erro padrão do valor de y previsto para cada x na regressão linear. Precisa de 3 pares ou mais.",
        },
    ),
    (
        "PROB",
        FunctionHelp {
            args: &["intervalo_x", "intervalo_prob", "limite_inferior", "[limite_superior]"],
            doc: "Probabilidade de os valores de x ficarem entre os limites (ou serem iguais ao limite inferior). As probabilidades devem somar 1.",
        },
    ),
    (
        "F.TEST",
        FunctionHelp {
            args: &["matriz1", "matriz2"],
            doc: "Teste F bicaudal: probabilidade de as variâncias das duas matrizes não serem diferentes. Ex.: =TESTE.F(A1:A5;B1:B5).",
        },
    ),
    (
        "Z.TEST",
        FunctionHelp {
            args: &["matriz", "x", "[sigma]"],
            doc: "Valor P unicaudal do teste z. Sem sigma usa o desvio padrão da amostra. Ex.: =TESTE.Z(A1:A10;4).",
        },
    ),
    (
        "TREND",
        FunctionHelp {
            args: &["y_conhecidos", "[x_conhecidos]", "[novos_x]", "[constante]"],
            doc: "Valores ao longo da reta de tendência linear (mínimos quadrados) para os novos x. Aceita uma variável x.",
        },
    ),
    (
        "GROWTH",
        FunctionHelp {
            args: &["y_conhecidos", "[x_conhecidos]", "[novos_x]", "[constante]"],
            doc: "Valores ao longo da curva de crescimento exponencial y = b·m^x para os novos x. Aceita uma variável x.",
        },
    ),
    (
        "LENB",
        FunctionHelp {
            args: &["texto"],
            doc: "Número de bytes do texto; em idiomas de byte simples é igual a NÚM.CARACT.",
        },
    ),
    (
        "LEFTB",
        FunctionHelp {
            args: &["texto", "[núm_bytes]"],
            doc: "Primeiros bytes do texto; em idiomas de byte simples é igual a ESQUERDA.",
        },
    ),
    (
        "RIGHTB",
        FunctionHelp {
            args: &["texto", "[núm_bytes]"],
            doc: "Últimos bytes do texto; em idiomas de byte simples é igual a DIREITA.",
        },
    ),
    (
        "MIDB",
        FunctionHelp {
            args: &["texto", "núm_inicial", "núm_bytes"],
            doc: "Bytes do meio do texto; em idiomas de byte simples é igual a EXT.TEXTO.",
        },
    ),
    (
        "FINDB",
        FunctionHelp {
            args: &["texto_procurado", "no_texto", "[núm_inicial]"],
            doc: "Posição de um texto dentro de outro (diferencia maiúsculas); em idiomas de byte simples é igual a PROCURAR.",
        },
    ),
    (
        "SEARCHB",
        FunctionHelp {
            args: &["texto_procurado", "no_texto", "[núm_inicial]"],
            doc: "Posição de um texto dentro de outro (aceita curingas, ignora maiúsculas); em idiomas de byte simples é igual a LOCALIZAR.",
        },
    ),
    (
        "REPLACEB",
        FunctionHelp {
            args: &["texto_antigo", "núm_inicial", "núm_bytes", "novo_texto"],
            doc: "Troca parte do texto por outro; em idiomas de byte simples é igual a MUDAR.",
        },
    ),
    (
        "ENCODEURL",
        FunctionHelp {
            args: &["texto"],
            doc: "Codifica o texto para uso em endereço web (UTF-8, %XX). Ex.: =CODIFURL(\"a b\") devolve a%20b.",
        },
    ),
    (
        "CONVERT",
        FunctionHelp {
            args: &["núm", "de_unidade", "para_unidade"],
            doc: "Converte entre unidades de medida (massa, distância, tempo, pressão, força, energia, potência, temperatura, volume, área, velocidade, informação). Ex.: =CONVERTER(1;\"mi\";\"km\") devolve 1,609344.",
        },
    ),
    (
        "BESSELJ",
        FunctionHelp {
            args: &["x", "n"],
            doc: "Função de Bessel de primeira espécie Jn(x). Ex.: =BESSELJ(1,9;2) devolve 0,329926.",
        },
    ),
    (
        "BESSELI",
        FunctionHelp {
            args: &["x", "n"],
            doc: "Função de Bessel modificada In(x). Ex.: =BESSELI(1,5;1) devolve 0,981666.",
        },
    ),
    (
        "DISC",
        FunctionHelp {
            args: &["liquidação", "vencimento", "preço", "resgate", "[base]"],
            doc: "Taxa de desconto de um título. Ex.: =DESC(DATA(2018;1;25);DATA(2018;6;15);97,975;100;1) devolve 5,24%.",
        },
    ),
    (
        "PRICEDISC",
        FunctionHelp {
            args: &["liquidação", "vencimento", "desconto", "resgate", "[base]"],
            doc: "Preço por R$ 100 de valor nominal de um título descontado.",
        },
    ),
    (
        "YIELDDISC",
        FunctionHelp {
            args: &["liquidação", "vencimento", "preço", "resgate", "[base]"],
            doc: "Rendimento anual de um título descontado.",
        },
    ),
    (
        "INTRATE",
        FunctionHelp {
            args: &["liquidação", "vencimento", "investimento", "resgate", "[base]"],
            doc: "Taxa de juros de um título totalmente investido.",
        },
    ),
    (
        "RECEIVED",
        FunctionHelp {
            args: &["liquidação", "vencimento", "investimento", "desconto", "[base]"],
            doc: "Valor recebido no vencimento de um título totalmente investido.",
        },
    ),
    (
        "TBILLPRICE",
        FunctionHelp {
            args: &["liquidação", "vencimento", "desconto"],
            doc: "Preço por R$ 100 de uma letra do tesouro (vencimento em até um ano).",
        },
    ),
    (
        "TBILLYIELD",
        FunctionHelp {
            args: &["liquidação", "vencimento", "preço"],
            doc: "Rendimento de uma letra do tesouro.",
        },
    ),
    (
        "TBILLEQ",
        FunctionHelp {
            args: &["liquidação", "vencimento", "desconto"],
            doc: "Rendimento equivalente a título de uma letra do tesouro.",
        },
    ),
    (
        "PRICEMAT",
        FunctionHelp {
            args: &["liquidação", "vencimento", "emissão", "taxa", "lucro", "[base]"],
            doc: "Preço por R$ 100 de um título que paga juros no vencimento.",
        },
    ),
    (
        "YIELDMAT",
        FunctionHelp {
            args: &["liquidação", "vencimento", "emissão", "taxa", "preço", "[base]"],
            doc: "Rendimento anual de um título que paga juros no vencimento.",
        },
    ),
    (
        "ACCRINTM",
        FunctionHelp {
            args: &["emissão", "liquidação", "taxa", "valor_nominal", "[base]"],
            doc: "Juros acumulados de um título que paga juros no vencimento.",
        },
    ),
    (
        "OFFSET",
        FunctionHelp {
            args: &["ref", "linhas", "colunas", "[altura]", "[largura]"],
            doc: "Referência deslocada de linhas e colunas a partir de ref, com a altura e a largura pedidas. Ex.: =SOMA(DESLOC(A1;0;0;3)) soma A1:A3.",
        },
    ),
    (
        "INDIRECT",
        FunctionHelp {
            args: &["texto_ref", "[a1]"],
            doc: "Referência escrita como texto (\"B7\", \"Dados!A1:C9\" ou um nome); a1 FALSO lê o estilo L1C1. Ex.: =INDIRETO(\"A\"&B1).",
        },
    ),
    (
        "TRIMRANGE",
        FunctionHelp {
            args: &["intervalo", "[aparar_linhas]", "[aparar_colunas]"],
            doc: "O intervalo sem as linhas e colunas vazias das bordas. 0 não apara, 1 só no começo, 2 só no fim, 3 nos dois (padrão). Ex.: =AJUSTARINTERVALO(A:A).",
        },
    ),
    (
        "AREAS",
        FunctionHelp {
            args: &["ref"],
            doc: "Quantas áreas a referência tem.",
        },
    ),
    (
        "FORMULATEXT",
        FunctionHelp {
            args: &["ref"],
            doc: "O texto da fórmula da célula; sem fórmula, #N/D.",
        },
    ),
    (
        "ISFORMULA",
        FunctionHelp {
            args: &["ref"],
            doc: "VERDADEIRO se a célula tem fórmula.",
        },
    ),
    (
        "SHEET",
        FunctionHelp {
            args: &["[valor]"],
            doc: "Número da aba (1 = a primeira guia) da referência, do nome em texto ou, sem argumento, da aba da fórmula.",
        },
    ),
    (
        "SHEETS",
        FunctionHelp {
            args: &["[ref]"],
            doc: "Quantas abas a pasta tem (sem argumento) ou a referência cobre.",
        },
    ),
    (
        "CELL",
        FunctionHelp {
            args: &["tipo_info", "[ref]"],
            doc: "Informação da célula: \"endereço\", \"col\", \"lin\", \"conteúdo\", \"tipo\" (b vazia, l texto, v valor), \"prefixo\", \"formato\", \"largura\", \"proteger\".",
        },
    ),
    (
        "INFO",
        FunctionHelp {
            args: &["tipo"],
            doc: "Informação do ambiente: \"numarquivo\" (abas), \"origem\", \"versãoso\", \"recalc\", \"versão\", \"sistema\", \"diretório\".",
        },
    ),
    (
        "GETPIVOTDATA",
        FunctionHelp {
            args: &["campo_dados", "tabela", "[campo1]", "[item1]", "…"],
            doc: "Valor de uma tabela dinâmica desenhada na planilha: o campo de dados, uma célula da tabela e pares campo/item. Sem pares, o Total Geral.",
        },
    ),
    (
        "BYROW",
        FunctionHelp {
            args: &["matriz", "lambda"],
            doc: "Aplica a LAMBDA (ou o nome de uma função) a cada linha e devolve uma coluna. Ex.: =PORLINHA(A1:C3;LAMBDA(l;SOMA(l))).",
        },
    ),
    (
        "BYCOL",
        FunctionHelp {
            args: &["matriz", "lambda"],
            doc: "Aplica a LAMBDA (ou o nome de uma função) a cada coluna e devolve uma linha. Ex.: =PORCOL(A1:C3;MÁXIMO).",
        },
    ),
    (
        "SCAN",
        FunctionHelp {
            args: &["[valor_inicial]", "matriz", "lambda"],
            doc: "Como REDUZIR, mas devolve cada passo do acumulador. Ex.: =VARRER(0;A1:A5;LAMBDA(a;v;a+v)) dá a soma acumulada.",
        },
    ),
    (
        "MAKEARRAY",
        FunctionHelp {
            args: &["linhas", "colunas", "lambda"],
            doc: "Monta uma matriz chamando a LAMBDA(linha; coluna) em cada posição. Ex.: =CRIARMATRIZ(3;3;LAMBDA(l;c;l*c)).",
        },
    ),
    (
        "ISOMITTED",
        FunctionHelp {
            args: &["argumento"],
            doc: "Dentro de uma LAMBDA, VERDADEIRO se o parâmetro não foi passado.",
        },
    ),
    (
        "GROUPBY",
        FunctionHelp {
            args: &["campos_linha", "valores", "função", "[cabeçalhos]", "[profundidade_total]", "[ordem]", "[filtro]", "[relação]"],
            doc: "Agrupa os valores pelos campos e aplica a função a cada grupo, com total. Ex.: =AGRUPARPOR(A2:A9;C2:C9;SOMA).",
        },
    ),
    (
        "PIVOTBY",
        FunctionHelp {
            args: &["campos_linha", "campos_coluna", "valores", "função", "[cabeçalhos]", "[total_linhas]", "[ordem_linhas]", "[total_colunas]", "[ordem_colunas]", "[filtro]", "[relativo_a]"],
            doc: "Tabela cruzada: linhas por um campo, colunas por outro, a função em cada cruzamento e os totais. Ex.: =PIVOTARPOR(A2:A9;B2:B9;C2:C9;SOMA).",
        },
    ),
    (
        "LINEST",
        FunctionHelp {
            args: &["y_conhecidos", "[x_conhecidos]", "[constante]", "[estatísticas]"],
            doc: "Regressão linear por mínimos quadrados (uma ou várias variáveis): coeficientes mₙ…m₁ e b; com estatísticas, também erros padrão, R², F e somas de quadrados.",
        },
    ),
    (
        "LOGEST",
        FunctionHelp {
            args: &["y_conhecidos", "[x_conhecidos]", "[constante]", "[estatísticas]"],
            doc: "Curva exponencial y = b·m₁^x₁·…: devolve mₙ…m₁ e b (e as estatísticas, na escala do logaritmo).",
        },
    ),
    (
        "FORECAST.ETS",
        FunctionHelp {
            args: &["data_alvo", "valores", "linha_do_tempo", "[sazonalidade]", "[conclusão_dados]", "[agregação]"],
            doc: "Previsão por suavização exponencial tripla (tendência e sazonalidade aditivas). Sazonalidade 1 detecta sozinha, 0 desliga.",
        },
    ),
    (
        "FORECAST.ETS.CONFINT",
        FunctionHelp {
            args: &["data_alvo", "valores", "linha_do_tempo", "[nível_confiança]", "[sazonalidade]", "[conclusão_dados]", "[agregação]"],
            doc: "Meia largura do intervalo de confiança da PREVISÃO.ETS na data alvo (95% por padrão).",
        },
    ),
    (
        "FORECAST.ETS.SEASONALITY",
        FunctionHelp {
            args: &["valores", "linha_do_tempo", "[conclusão_dados]", "[agregação]"],
            doc: "Tamanho da estação que a PREVISÃO.ETS detecta (0 = sem sazonalidade).",
        },
    ),
    (
        "FORECAST.ETS.STAT",
        FunctionHelp {
            args: &["valores", "linha_do_tempo", "tipo_estatística", "[sazonalidade]", "[conclusão_dados]", "[agregação]"],
            doc: "Estatística do modelo da PREVISÃO.ETS: 1 alfa, 2 beta, 3 gama, 4 MASE, 5 SMAPE, 6 MAE, 7 RMSE, 8 passo.",
        },
    ),
    (
        "ACCRINT",
        FunctionHelp {
            args: &["emissão", "primeiro_juro", "liquidação", "taxa", "valor_nominal", "frequência", "[base]", "[método_cálculo]"],
            doc: "Juros acumulados de um título que paga juros periódicos.",
        },
    ),
    (
        "AMORDEGRC",
        FunctionHelp {
            args: &["custo", "data_compra", "primeiro_período", "residual", "período", "taxa", "[base]"],
            doc: "Depreciação de cada período no sistema francês, com coeficiente pela vida útil.",
        },
    ),
    (
        "AMORLINC",
        FunctionHelp {
            args: &["custo", "data_compra", "primeiro_período", "residual", "período", "taxa", "[base]"],
            doc: "Depreciação linear de cada período no sistema francês (primeiro período proporcional).",
        },
    ),
    (
        "COUPDAYBS",
        FunctionHelp {
            args: &["liquidação", "vencimento", "frequência", "[base]"],
            doc: "Dias do início do período de cupom até a liquidação.",
        },
    ),
    (
        "COUPDAYS",
        FunctionHelp {
            args: &["liquidação", "vencimento", "frequência", "[base]"],
            doc: "Dias do período de cupom que contém a liquidação.",
        },
    ),
    (
        "COUPDAYSNC",
        FunctionHelp {
            args: &["liquidação", "vencimento", "frequência", "[base]"],
            doc: "Dias da liquidação até o próximo cupom.",
        },
    ),
    (
        "COUPNCD",
        FunctionHelp {
            args: &["liquidação", "vencimento", "frequência", "[base]"],
            doc: "Data do próximo cupom depois da liquidação.",
        },
    ),
    (
        "COUPNUM",
        FunctionHelp {
            args: &["liquidação", "vencimento", "frequência", "[base]"],
            doc: "Quantos cupons faltam pagar entre a liquidação e o vencimento.",
        },
    ),
    (
        "COUPPCD",
        FunctionHelp {
            args: &["liquidação", "vencimento", "frequência", "[base]"],
            doc: "Data do último cupom antes da liquidação.",
        },
    ),
    (
        "DURATION",
        FunctionHelp {
            args: &["liquidação", "vencimento", "cupom", "lucro", "frequência", "[base]"],
            doc: "Duração de Macaulay de um título com cupom, em anos.",
        },
    ),
    (
        "MDURATION",
        FunctionHelp {
            args: &["liquidação", "vencimento", "cupom", "lucro", "frequência", "[base]"],
            doc: "Duração modificada (Macaulay dividida por 1 + lucro/frequência).",
        },
    ),
    (
        "ODDFPRICE",
        FunctionHelp {
            args: &["liquidação", "vencimento", "emissão", "primeiro_cupom", "taxa", "lucro", "resgate", "frequência", "[base]"],
            doc: "Preço por 100 de um título com primeiro período irregular.",
        },
    ),
    (
        "ODDFYIELD",
        FunctionHelp {
            args: &["liquidação", "vencimento", "emissão", "primeiro_cupom", "taxa", "preço", "resgate", "frequência", "[base]"],
            doc: "Rendimento de um título com primeiro período irregular.",
        },
    ),
    (
        "ODDLPRICE",
        FunctionHelp {
            args: &["liquidação", "vencimento", "último_juro", "taxa", "lucro", "resgate", "frequência", "[base]"],
            doc: "Preço por 100 de um título com último período irregular.",
        },
    ),
    (
        "ODDLYIELD",
        FunctionHelp {
            args: &["liquidação", "vencimento", "último_juro", "taxa", "preço", "resgate", "frequência", "[base]"],
            doc: "Rendimento de um título com último período irregular.",
        },
    ),
    (
        "PRICE",
        FunctionHelp {
            args: &["liquidação", "vencimento", "taxa", "lucro", "resgate", "frequência", "[base]"],
            doc: "Preço por 100 de valor nominal de um título que paga juros periódicos. Ex.: =PREÇO(DATA(2008;2;15);DATA(2017;11;15);5,75%;6,5%;100;2) dá 94,63.",
        },
    ),
    (
        "YIELD",
        FunctionHelp {
            args: &["liquidação", "vencimento", "taxa", "preço", "resgate", "frequência", "[base]"],
            doc: "Rendimento de um título que paga juros periódicos.",
        },
    ),
    (
        "BESSELK",
        FunctionHelp {
            args: &["x", "n"],
            doc: "Função de Bessel modificada de segunda espécie Kn(x), x > 0. Ex.: =BESSELK(1,5;1) dá 0,277388.",
        },
    ),
    (
        "BESSELY",
        FunctionHelp {
            args: &["x", "n"],
            doc: "Função de Bessel de segunda espécie Yn(x), x > 0. Ex.: =BESSELY(2,5;1) dá 0,145918.",
        },
    ),
    (
        "ASC",
        FunctionHelp {
            args: &["texto"],
            doc: "Troca letras, dígitos e símbolos de largura inteira pelos de meia largura.",
        },
    ),
    (
        "DBCS",
        FunctionHelp {
            args: &["texto"],
            doc: "Troca letras, dígitos e símbolos de meia largura pelos de largura inteira.",
        },
    ),
    (
        "BAHTTEXT",
        FunctionHelp {
            args: &["núm"],
            doc: "O valor em baht por extenso, em tailandês.",
        },
    ),
    (
        "PHONETIC",
        FunctionHelp {
            args: &["ref"],
            doc: "Texto fonético (furigana) da célula; sem ele, o próprio texto.",
        },
    ),
    (
        "EUROCONVERT",
        FunctionHelp {
            args: &["núm", "origem", "destino", "[precisão_total]", "[precisão_triangulação]"],
            doc: "Converte entre o euro e as moedas antigas da zona do euro pelas taxas fixas (\"DEM\", \"FRF\", \"PTE\"…).",
        },
    ),
    (
        "FILTERXML",
        FunctionHelp {
            args: &["xml", "xpath"],
            doc: "Valores de um texto XML pelo caminho XPath (/a/b, //b, [1], [@id='x'], /@atributo, text()). Ex.: =FILTROXML(A1;\"//item\").",
        },
    ),
    (
        "WEBSERVICE",
        FunctionHelp {
            args: &["url"],
            doc: "O texto que a URL devolve (só pedido explícito, de mão única: nada da planilha é enviado).",
        },
    ),
    (
        "CUBEKPIMEMBER",
        FunctionHelp {
            args: &["conexão", "nome_kpi", "propriedade_kpi", "[legenda]"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "CUBEMEMBER",
        FunctionHelp {
            args: &["conexão", "expressão_membro", "[legenda]"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "CUBEMEMBERPROPERTY",
        FunctionHelp {
            args: &["conexão", "expressão_membro", "propriedade"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "CUBERANKEDMEMBER",
        FunctionHelp {
            args: &["conexão", "expressão_conjunto", "classificação", "[legenda]"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "CUBESET",
        FunctionHelp {
            args: &["conexão", "expressão_conjunto", "[legenda]", "[ordem]", "[classificar_por]"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "CUBESETCOUNT",
        FunctionHelp {
            args: &["conjunto"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "CUBEVALUE",
        FunctionHelp {
            args: &["conexão", "[expressão_membro1]", "…"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "RTD",
        FunctionHelp {
            args: &["id_prog", "servidor", "tópico1", "[tópico2]", "…"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "CALL",
        FunctionHelp {
            args: &["…"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "REGISTER.ID",
        FunctionHelp {
            args: &["…"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "STOCKHISTORY",
        FunctionHelp {
            args: &["ação", "data_inicial", "[data_final]", "[intervalo]", "[cabeçalhos]", "…"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "TRANSLATE",
        FunctionHelp {
            args: &["texto", "[idioma_origem]", "[idioma_destino]"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "DETECTLANGUAGE",
        FunctionHelp {
            args: &["texto"],
            doc: "Reconhecida para abrir arquivos do Excel; devolve #N/D, porque precisa de servidor, DLL ou serviço na internet, e o FeatherSheet roda tudo no computador.",
        },
    ),
    (
        "MODE.SNGL",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Igual a MODO. Valor que mais se repete.",
        },
    ),
    (
        "NORMDIST",
        FunctionHelp {
            args: &["x", "média", "desv_padrão", "cumulativo"],
            doc: "Igual a DIST.NORM.N. Distribuição normal (densidade ou cumulativa).",
        },
    ),
    (
        "NORMINV",
        FunctionHelp {
            args: &["probabilidade", "média", "desv_padrão"],
            doc: "Igual a INV.NORM.N. Inverso da distribuição normal para a probabilidade.",
        },
    ),
    (
        "PERCENTILE.INC",
        FunctionHelp {
            args: &["intervalo", "p"],
            doc: "Igual a PERCENTIL. Valor abaixo do qual está a fração p dos dados (p entre 0 e 1).",
        },
    ),
    (
        "PERCENTRANK",
        FunctionHelp {
            args: &["matriz", "x", "[signif]"],
            doc: "Igual a ORDEM.PORCENTUAL.INC. Posição relativa de um valor, de 0 a 1 inclusive.",
        },
    ),
    (
        "QUARTILE.INC",
        FunctionHelp {
            args: &["intervalo", "quarto"],
            doc: "Igual a QUARTIL. Quartil: 0 mínimo, 1, 2 mediana, 3, 4 máximo.",
        },
    ),
    (
        "STDEV.S",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Igual a DESVPAD. Desvio padrão de uma amostra.",
        },
    ),
    (
        "STDEVP",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Igual a DESVPAD.P. Desvio padrão da população inteira.",
        },
    ),
    (
        "VAR.S",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Igual a VAR. Variância de uma amostra.",
        },
    ),
    (
        "VARP",
        FunctionHelp {
            args: &["núm1", "…"],
            doc: "Igual a VAR.P. Variância da população inteira.",
        },
    ),
    (
        "NA",
        FunctionHelp {
            args: &[],
            doc: "Devolve o erro #N/D (valor não disponível). Ex.: =SE(A1=\"\";NÃO.DISP();A1).",
        },
    ),
];
