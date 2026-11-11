//! "O que há de novo": abre sozinha na primeira abertura depois de atualizar
//! (e em Ajuda ▸ O que há de novo). Segue a F1 (GUIA-JANELAS.md): lateral com
//! as versões, cabeçalho com a marca, cartões dos destaques e a lista do
//! resto. O conteúdo é embutido no app; nada é buscado na internet.
use super::*;
use feather_ui::consulta::Ilustracao;

pub use feather_ui::novidades::Versao;

/// Da mais nova para a mais antiga. A primeira é a desta compilação.
pub const VERSOES: &[Versao] = &[
    Versao {
        numero: "0.3.0",
        data: "08/11/2026",
        resumo: "Power Query, dinâmica mais esperta e arquivos mais leves",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("file-export", "Power Query no OpenQuery", "Cole o M do Excel e as etapas aparecem; Copiar como Power Query leva de volta."),
            ("table", "Dinâmica mais esperta", "Dinâmicas recomendadas, contagem de distintos, mediana, quartis e datas como data."),
            ("timeline", "Gantt com o dia de hoje", "Linha de hoje e tarefas pintadas como concluídas, em andamento ou futuras."),
            ("printer", "Página e propriedades no arquivo", "Margens, papel e área de impressão salvos; título, autor e empresa no .xlsx."),
        ],
        tambem: &[
            "OpenQuery com os passos do Power Query que faltavam: substituir, preencher, formatar texto, remover linhas, transpor, dividir por posição, coluna personalizada com fórmula, agrupar com várias contas.",
            "Consultas: Ctrl+Z nas etapas, duplicar e referenciar, juntar arquivos de uma pasta e nomes parecidos, atualizar ao abrir o arquivo.",
            "Abra as consultas do Power Query que o Excel guardou no .xlsx.",
            "Ctrl+Z desfaz inserir e excluir linhas e colunas, com as fórmulas que liam a área.",
            "AutoConclusão: ao digitar, o valor que já existe na coluna aparece e o Enter completa.",
            "Ctrl ao arrastar a alça conta 7, 8, 9; sem Ctrl o número é copiado, como no Excel.",
            "Editar uma célula centralizada mantém o texto no centro, como no Excel.",
            "Formatar células… e Copiar como imagem no botão direito da célula.",
            "Nome e empresa que vão nos arquivos agora também em Configurações ▸ Seus dados.",
            "Opção para o Salvar como… sugerir .xlsx, para quem troca arquivo com o Excel.",
            "Salvar em .xlsx até 3 vezes mais rápido em planilhas grandes.",
            "Salvar planilha grande gasta bem menos memória.",
            "Fórmulas SES com critério em branco ou zero muito mais rápidas em planilhas grandes.",
            "Macros do Excel: SpecialCells, AutoFilter com duas condições, Filtro Avançado, InputBox com caixa para digitar, CopyPicture e número com vírgula ao juntar texto.",
            "Tab passa pelos campos dos formulários das macros na ordem do Excel.",
            "Uma macro com defeito não fecha mais o FeatherSheet.",
            "Interface em inglês e espanhol sem sobras de português na faixa.",
            "O F12 mostra a mesma memória do Monitor do sistema.",
        ],
    },
    Versao {
        numero: "0.2.9",
        data: "04/11/2026",
        resumo: "Minigráficos na célula e arquivos mais fiéis",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("sparkline-line", "Minigráficos", "Inserir → Minigráficos desenha linha, coluna ou ganhos/perdas na célula."),
            ("arrow-back-up", "Desfazer exato", "Ctrl+Z devolve o número inteiro e o texto como era, mesmo com formatação."),
            ("file-export", "Ajuda em Markdown", "A ajuda (F1) pode ser exportada como um arquivo Markdown."),
            ("shield-lock", "Mais segurança", "Macros de evento de VBA importado começam bloqueadas; atualizador em pasta privada."),
        ],
        tambem: &[
            "Total de tabela e SUBTOTAL voltam a contar só as linhas visíveis quando o filtro muda.",
            "Texto com espaço nas pontas e números com formatação continuam iguais no .fshz e no .xlsx.",
            "Data, hora e porcentagem lidas do .xlsx guardam o valor exato, sem arredondar pela formatação.",
            "Salvar em .xlsx avisa quantas células passaram de 32.767 caracteres e foram cortadas.",
            "Pasta grande demais para a cópia de segurança automática avisa para salvar com Ctrl+S.",
            "Salvamento interrompido não deixa arquivo parcial para trás.",
            ".xlsx com XML aninhado demais ou gigante é recusado em vez de travar o app.",
            "Planilhas com milhares de alturas de linha diferentes rolam mais leve.",
        ],
    },
    Versao {
        numero: "0.2.8",
        data: "03/11/2026",
        resumo: "Controles de formulário do Excel e arquivos mais seguros",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("forms", "Controles de formulário", "Crie na aba Macros, ligue a uma célula e ajuste em Formatar controle."),
            ("text-wrap", "Texto passa para a vizinha", "Texto comprido aparece inteiro por cima das células vazias do lado, como no Excel."),
            ("device-floppy", "Nada se perde ao fechar", "Fechar no meio de um salvamento pergunta de novo pelo que foi digitado."),
            ("lock", "Arquivos baixados mais seguros", "Formulário de macro respeita o bloqueio; caminho de rede só com a sua permissão."),
        ],
        tambem: &[
            "F12 mostra o que o .xlsx tem e não foi convertido (ActiveX, segmentação de dados, minigráficos…).",
            "Ctrl+S num .xlsx com partes não convertidas pede onde salvar, para não apagá-las.",
            "Salvar em .xlsx com mais de 1.048.576 linhas avisa em vez de perder linhas.",
            "CSV com ponto e vírgula continua com ponto e vírgula ao salvar de novo.",
            "=(0,1+0,2)=0,3 dá VERDADEIRO, como no Excel; MÍNIMO(;5) e CONT.VALORES com texto vazio também.",
            "PROCV, CORRESP, PERCENTIL, MED e CONT.SE arrastado em milhares de linhas ficam bem mais rápidos.",
            "Recuperar uma cópia só a tira da lista depois que ela abre.",
            "Esta janela de novidades e os Primeiros passos não ficam mais escondidos atrás da planilha.",
        ],
    },
    Versao {
        numero: "0.2.7",
        data: "03/11/2026",
        resumo: "Macros do Excel preservadas e Windows mais leve",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("code", "Salvar como .xlsm com o VBA original", "As macros do arquivo do Excel continuam nele ao salvar."),
            ("bolt", "Mais leve sem placa de vídeo", "No Windows, o quadro desenhado pela CPU vai direto para a janela."),
        ],
        tambem: &[
            ".xlsx e .xlsm abrem como o arquivo de trabalho (Ctrl+S grava neles).",
            "Instalador do Windows para todos os usuários, em Arquivos de Programas\\FeatherWorks\\FeatherSheet.",
        ],
    },
    Versao {
        numero: "0.2.6",
        data: "02/11/2026",
        resumo: "Mais leve e mais certo no Windows",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("bolt", "Abrir .fshz grande sem pico de memória", "4 milhões de linhas: 1,1 GB em vez de 2,2 GB, e abre 40% mais rápido."),
            ("table", "Fórmulas da tela sempre calculadas", "Células com \"…\" calculam sozinhas, com filtro ou com a tela rolada."),
            ("code", "Macros do Excel mais completas", "Collection, Dictionary, ByRef, MsgBox com todos os botões, AutoFilter e formas."),
            ("pointer", "Macro ao clicar numa célula", "Evento Ao selecionar células (o Worksheet_SelectionChange do VBA)."),
        ],
        tambem: &[
            "O editor de macros (Alt+F11) abre vazio, sem a macro de exemplo (#139).",
            "Instalador do Windows: .xlsm no \"Abrir com\" e tamanho em Programas e Recursos.",
            "Arquivo .xlsx com vínculo externo: rolar e selecionar colunas sem atraso.",
        ],
    },
    Versao {
        numero: "0.2.5",
        data: "01/11/2026",
        resumo: "Excel com milhões de fórmulas abre rápido",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("bolt", "Abrir .xlsx pesado: de 10 min para 4 s", "Milhões de fórmulas com vínculo abrem com os valores do Excel (antes travava em 80%)."),
        ],
        tambem: &[
            "SUBTOTAL com DESLOC por linha (contar só as visíveis no SOMARPRODUTO) dá o valor do Excel.",
            "Ao abrir, avisa quando há versão nova (uma vez por dia; desligue em Configurações → Sobre).",
        ],
    },
    Versao {
        numero: "0.2.4",
        data: "01/11/2026",
        resumo: "Janela certa no Windows 10",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("bolt", "Janela no lugar no Windows 10", "Placas sem DirectX 12 (como a Intel HD 2500) deixavam a janela deslocada, com faixa preta."),
        ],
        tambem: &[],
    },
    Versao {
        numero: "0.2.3",
        data: "01/11/2026",
        resumo: "Macros do Excel quase completas",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("math-function", "Funções do VBA: 99%", "Datas, Format, Choose, Switch, financeiras (Pmt, NPV, IRR…), TypeName, Filter."),
            ("table", "Caixas de texto e proteção", "Macros mexem nas caixas de texto, protegem a aba e ordenam pelo filtro."),
            ("shield-lock", "Shell com pergunta", "Rodar programa sempre pergunta antes e avisa se o código traz um binário."),
            ("bolt", "Links e grade pela macro", "ShellExecute abre link no navegador; DisplayGridlines liga e desliga a grade."),
        ],
        tambem: &[
            "Windows 10: manifesto do Windows no .exe (escala da tela e compatibilidade), para a janela não sair deslocada (#135).",
            "Sort sem SetRange ordena a tabela inteira, não só a coluna da chave.",
            "Str(5) dá \" 5\" e CInt fora da faixa dá Estouro, como no VBA.",
        ],
    },
    Versao {
        numero: "0.2.2",
        data: "29/10/2026",
        resumo: "Menos memória e macros mais seguras",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("device-floppy", "Metade da memória em bases grandes", "Colunas de números compactadas sem perda ao abrir: 4,4 GB viraram 2,0 GB."),
            ("shield-lock", "Ativar macros?", "Todo arquivo com macro pergunta antes, avisando o que ela grava e o VBA perigoso."),
            ("bolt", "Gráficos só da aba aberta", "Os das outras abas são desenhados quando a aba aparece."),
            ("math-function", "Fórmulas longas como no Excel", "Até 4.096 operações numa fórmula; SOMARPRODUTO mais rápido."),
        ],
        tambem: &[
            "Arquivos maliciosos param em tetos: fórmula, .fshz, XML do .xlsx, VBA, MCP e atualização (#123–#130).",
            "Preencher ou colar fórmula na planilha inteira para com aviso, em vez de estourar a memória.",
            "Coluna de datas em texto vinda do Parquet vira data; data que não existe fica texto (#131).",
            "Gráfico numa aba com tabela dinâmica deixa o Total Geral de fora, como no Excel.",
            "Abrir: filtro por tipo de arquivo.",
        ],
    },
    Versao {
        numero: "0.2.1",
        data: "29/10/2026",
        resumo: "Abrir mais rápido, salvar mais leve",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("bolt", "Abrir .xlsx muito mais rápido", "Leitor próprio das linhas, vários núcleos e nada calculado durante a leitura."),
            ("chart-bar", "Gráficos com painel por tipo", "Campos de cada tipo, rosca de verdade e o intervalo ajustado arrastando na grade."),
            ("table", "OpenQuery como o Power Query", "Etapas num script editável, CSV em qualquer codificação e fonte sem limite."),
            ("math-function", "Fórmulas em milhões de linhas", "Calculam em fundo, com Parar; CORREL e afins certos quando há células vazias."),
            ("device-floppy", "Salvar sem dobrar a memória", "As colunas de números vão para o arquivo sem serem copiadas antes."),
            ("palette", "Tema escuro no Ubuntu", "O automático segue o sistema; células claras como no Excel e cantos arredondados."),
        ],
        tambem: &[
            "As listas suspensas perto do pé da janela (Eixo X do gráfico) voltaram a escolher o item (#122).",
            "Novos na faixa: Obter e Transformar Dados, Dimensionar para Ajustar e Proteger Pasta de Trabalho.",
            "Gráficos do .xlsx: séries que pulam colunas, linhas escondidas pelo filtro e rosca (#104, #109).",
            "AGORA() e HOJE() no fuso do sistema (#95); o rodapé soma as áreas do Ctrl+clique (#111).",
            "Ctrl+1 e a setinha ↘ dos grupos Fonte, Alinhamento e Número abrem Formatar células.",
            "O editor da célula usa a letra, a cor e o fundo dela; exportar .xlsx, JSON e CSV não trava a janela.",
            "A segmentação de dados fica no canto da grade, não mais por cima da barra de fórmulas.",
            "No Windows, o menu Iniciar mostra FeatherSheet (#102).",
        ],
    },
    Versao {
        numero: "0.2.0",
        data: "27/10/2026",
        resumo: "Macros do Excel, rápidas",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("bolt", "Motor JavaScript próprio", "As macros rodam no Falcão, o motor da suíte escrito em Rust: ~4× mais rápidas."),
            ("player-play", "Macro longa sem travar", "O botão mostra \"Executando… N s\" e tem Parar; o que a macro escreveu é desfeito."),
            ("code", "Conversor VBA mais completo", "Matrizes no Range.Value, datas, RemoveDuplicates, FormatConditions e On Error."),
            ("clock", "200 mil linhas em segundos", "A macro de fechamento do mês do vídeo roda em ~8 s; no Excel, 25 s."),
        ],
        tambem: &[
            "Macros novas: wb.addSheet, wb.deleteSheet, wb.renameSheet e sheet.autoFitColumns.",
            "Gravar matriz, formatar coluna e remover duplicadas em planilha grande: um Ctrl+Z só.",
            "Borda sem listra branca entre células (#69); \"VERDADEIRO\" continua texto (#66); célula mesclada não anda ao rolar (#36).",
            "Limites conhecidos: inserir/excluir linhas ainda não entra no Ctrl+Z (#65); no erro da macro, a coluna é a do começo da expressão.",
        ],
    },
    Versao {
        numero: "0.1.5",
        data: "26/10/2026",
        resumo: "Formulários nas macros",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("code", "Formulários nas macros", "O UserForm do VBA: designer no Alt+F11, janela própria e sheet.showComponent."),
            ("table", "UserForm do .xlsm convertido", "O formulário do Excel vem com o desenho, as listas e o código dos botões."),
            ("code", "Várias funções num módulo", "Botões e formulários chamam Módulo.função, como as Sub do VBA."),
            ("palette", "Formatação condicional em janela própria", "\"Aplica-se a\" com Selecionar pela grade e filtro por planilha ou seleção."),
            ("link", "Vínculos e macros mais seguros", "Vínculos do disco só depois de \"Atualizar valores\"; forma com macro respeita o bloqueio."),
            ("math-function", "Fórmulas e datas corrigidas", "CONT.SE com \"2\" e \"=2\", DATAM fora do ano 9999 e aba com nome de célula."),
        ],
        tambem: &[
            "Texto do .xlsx que parece número ou fórmula continua texto ao abrir.",
            "Abrir e salvar não troca mais as fórmulas de lugar.",
            "Ctrl+Z depois de ordenar devolve as fórmulas; inserir ou excluir linhas limpa o histórico em vez de desfazer errado.",
            "Macro presa num laço sem fim para sozinha em 30 segundos.",
            "Recuperação de cada janela aberta fica separada.",
            "Limites conhecidos: inserir/excluir linhas ainda não entra no Ctrl+Z (#65); o texto \"VERDADEIRO\" do .xlsx volta como lógico (#66).",
        ],
    },
    Versao {
        numero: "0.1.4",
        data: "22/10/2026",
        resumo: "Agora estamos com a biblioteca pelicano",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("bolt", "Interface nova sobre o pelicano", "A mesma tela de antes, agora desenhada pela biblioteca pelicano da FeatherWorks."),
            ("table", "Tudo no mesmo lugar", "Faixa, menus, diálogos, macros e ajuda continuam iguais aos da versão 0.1.3."),
            ("clock", "Mesmo desempenho", "Arquivos grandes abrem no mesmo tempo e com a mesma memória da 0.1.3."),
        ],
        tambem: &[
            "Barra fx: depois de inserir uma referência ou usar F4, o cursor ainda não vai para depois do trecho.",
            "Renomear aba: o nome antigo não vem selecionado; apague antes de digitar.",
        ],
    },
    Versao {
        numero: "0.1.3",
        data: "19/10/2026",
        resumo: "Excel e acabamento",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("link", "Vínculo com outro arquivo no .xlsx", "O Excel abre sem reparo e mostra os últimos valores da origem."),
            ("math-function", "Fórmulas novas no Excel sem #NOME?", "AGREGAR, MUNIT, REGEX e PERCENTOF chegam ao Excel com o nome certo."),
            ("clipboard", "Colar data até o fim da coluna", "A data vai até a última linha com o formato junto, num Ctrl+Z só."),
            ("pointer", "Clique com a caneta da mesa", "A ponta clica em botões, células e menus, sem pedir compartilhar a tela."),
            ("photo", "Imagem entra e sai da célula", "Botão direito: Colocar sobre as células ou Colocar na célula."),
            ("table", "Fórmulas corrigidas", "SOMARPRODUTO com matriz, DIA.DA.SEMANA com o tipo e nome de outra aba em SOMASES."),
            ("code", "Macros com laço grande", "Laços de até 20 milhões de voltas; o erro de laço sem fim sai em português."),
            ("bolt", "Borda do Ctrl+C animada", "A área copiada fica com o traço andando, como no Excel."),
            ("palette", "Formatar células mais compacto", "A aba Alinhamento cabe inteira; códigos de número sem rolagem."),
        ],
        tambem: &[
            "Tabela dinâmica gravada no .xlsx é reconhecida pelo Excel como dinâmica.",
            "Barra de status: as contas ficam junto do zoom e alinhadas.",
            "Caixa de nome mostra o nome definido da seleção e uma célula só sem repetir (G17).",
            "Formatação condicional sem o código hexa da cor à mostra.",
            "Seletor de tipo de gráfico abre no centro, sem opções em cinza.",
            "Editor de macros: divisórias da lateral e da Janela Imediata arrastam direito.",
            "Macros: toLocaleString(\"pt-BR\") segue a localidade.",
            "Configurações gravadas sem risco de perder tudo numa queda.",
            "MCP: sheet_tela mostra os painéis abertos; parâmetros e mensagens consistentes.",
            "Menu ▾ da AutoSoma mostra o atalho Alt+=.",
            "Bloquear seleção aparece aceso quando a célula está bloqueada (o clique desbloqueia).",
        ],
    },
    Versao {
        numero: "0.1.2",
        data: "18/10/2026",
        resumo: "Todas as funções do Excel, faixa completa e menos travadas",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("math-function", "Todas as 520 funções do Excel", "DESLOC, INDIRETO, LAMBDA, PROCX… e todas nos menus da aba Fórmulas."),
            ("bolt", "Apagar e desfazer sem travar", "Apagar uma fórmula e dar Ctrl+Z: de 3,4 s para um instante."),
            ("device-floppy", "Salvar arquivo enorme mais rápido", "Ctrl+S em 13,7 milhões de linhas: a tela para 0,7 s em vez de 3,8 s."),
            ("chart-line", "Rolar com gráficos sem tranco", "O gráfico é desenhado uma vez; a rolagem só recorta a imagem pronta."),
            ("chart-bar", "Gráfico com um clique", "Colunas, Linhas, Pizza… direto da faixa; Recomendados escolhe pelos dados."),
            ("printer", "Várias áreas de impressão", "A prévia mostra todas, uma por página; cabeçalho e rodapé próprios."),
            ("text-box", "Caixa de texto", "Inserir › Caixa de texto: clique ou arraste e já comece a digitar."),
            ("bolt", "Abrir .xlsx mais rápido", "As abas e os gráficos do arquivo são lidos em paralelo."),
            ("omega", "Símbolos e equações", "Moedas, setas, frações, letras gregas e sinais de conta num clique."),
        ],
        tambem: &[
            "Cópia de recuperação de uma pasta com muitos erros: de 0,64 s para 0,13 s.",
            "LET e LAMBDA: variável com intervalo funciona dentro de SOMA e das outras funções de lista.",
            "Funções que dependem de serviço de fora (cubo, RTD, STOCKHISTORY) dizem o motivo do #N/D.",
            "Macros › Componentes abre o editor no componente HTML/CSS.",
            "Formas: o botão Efeitos liga e desliga a sombra.",
            "Ferramentas › Proteção: Bloquear seleção e Proteger planilha.",
            "Inserir › Impressão ganhou Adicionar à área.",
            "Texto da forma fica escuro sobre preenchimento claro.",
            "Erro repetido numa coluna inteira ocupa uma entrada só no arquivo.",
        ],
    },
    Versao {
        numero: "0.1.1",
        data: "18/10/2026",
        resumo: "Milhões de linhas sem travar, imagem na célula e menos memória",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("pivot-table", "Tabela dinâmica em segundo plano", "Em milhões de linhas a janela não para: a barra embaixo mostra o andamento."),
            ("bolt", "Fórmula na coluna inteira", "Preencher ou colar em 13,7 milhões de linhas leva milissegundos."),
            ("bolt", "MÍNIMOSES e os *SES sem travar", "Com A:A e B:B na fórmula: preencher mil linhas caiu de 76 s para 12 ms."),
            ("table", "Ctrl+T e região atual instantâneos", "Em 13 milhões de linhas a área da tabela sai em 0,1 s (antes travava)."),
            ("arrows-horizontal", "Inserir e excluir coluna rápido", "Com milhares de fórmulas: de 3,3 s para 0,03 s, referências certas."),
            ("device-floppy", "Arquivo enorme com menos memória", "Abrir 13,7 milhões de linhas: de 13,3 GB para 4,5 GB; salvar: de 20 para 9 GB."),
            ("photo", "Imagem dentro da célula", "Inserir na célula, tirar dela, Alt+arrastar; GIF, BMP e WebP aparecem."),
            ("palette", "Painel Formatar imagem", "Altura, largura em cm, giro, escala, proporção e mover com as células."),
            ("terminal", "IA cria tabela dinâmica", "Pelo MCP: dinâmica, formatação condicional e fórmulas sem limite de linhas."),
        ],
        tambem: &[
            "Tabela dinâmica: célula vazia e texto não entram mais como 0 em Média, Desvio padrão, Mínimo e Moda.",
            "Tabela dinâmica: mudar os campos reaproveita a origem já lida e refaz só a conta.",
            "Colar uma fórmula desloca as referências, como no Excel (=A1 colada 4 linhas abaixo vira =A5).",
            "Inserir ou excluir coluna ajusta referências de coluna inteira, como B:B.",
            "PARÂMETRO compara texto e número como o Excel.",
            "Colar milhões de fórmulas não dá mais um tranco a cada segundo.",
            "Ctrl+Z e Ctrl+Y de muitas células de uma vez ficaram mais rápidos.",
            "Gráfico a partir de uma célula numa tabela enorme não trava mais.",
            "Monitor (F12): avisa quando a tela fica parada mais de 200 ms e mostra o tempo do Ctrl+T.",
            "Figuras do .xlsx não passam mais por cima dos cabeçalhos ao rolar.",
            "A barra de fórmulas não mostra o código da imagem guardada na célula.",
            "Botão direito na imagem abre o menu dela; com a célula em edição, abre o menu da célula.",
            "Um painel lateral por vez: gráfico, tabela dinâmica ou imagem.",
            "Menu Orientação do texto com as setas certas.",
            "Exportar CSV ou TSV sugere o nome do arquivo.",
            "Pasta nova em inglês começa em Sheet1; em espanhol, Hoja1.",
            "O arquivo .fshz grande é gravado em partes, sem montar tudo na memória.",
        ],
    },
    Versao {
        numero: "0.1.0",
        data: "17/10/2026",
        resumo: "Tabela dinâmica rápida, referência entre arquivos e espanhol",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("pivot-table", "Tabela dinâmica muito mais rápida", "Trocar um campo em 1 milhão de linhas leva ~0,2 s; criar é na hora."),
            ("link", "Referência entre arquivos", "=[Vendas.fshz]Dados!A1 fica viva com os dois abertos; clique na outra janela."),
            ("clipboard", "Colar em 100 milhões de linhas", "Um valor colado na coluna inteira vai até a última linha em menos de 1 s."),
            ("bolt", "CSV grande ~10× mais rápido", "653 MB e 70 milhões de células abrem em 3,2 s com 2 núcleos; antes, 29,5 s."),
            ("bolt", ".xlsx com fórmulas ~4× mais rápido", "A fórmula repetida é traduzida uma vez só: de 33,7 s para 8,4 s com 2 núcleos."),
            ("world", "Interface em espanhol", "Terceiro idioma, com as funções em espanhol; abre no idioma do sistema."),
            ("chart-pivot", "Mostrar valores como, gráfico e detalhes", "% do total, acumulado e mais; gráfico dinâmico; duplo clique abre as linhas."),
            ("terminal", "IA pelo MCP", "Ligue em Configurações: a IA ordena, filtra, substitui, move e copia."),
            ("code", "Eventos da planilha no Alt+F11", "Código ao mudar células e ao ativar a aba, pastas de módulos e divisórias."),
        ],
        tambem: &[
            "Tabela dinâmica: nasce vazia, como no Excel, e mostra \"Atualizando…\" na barra em origens grandes.",
            "Tabela dinâmica: aba própria na faixa, filtro de relatório na grade e arrastar para reordenar os campos.",
            "Tabela dinâmica: Soma, Média, Desvio, Moda… ao clicar no campo; duplo clique em 125 mil linhas em 0,4 s.",
            "Segmentação de dados e atualizar sozinha quando a origem muda.",
            "Ctrl+C marca a área copiada com borda tracejada, até Esc ou uma edição.",
            "Barra de status do seu jeito: botão direito escolhe média, contagem, mín, máx, soma e mais.",
            "Gráfico com milhões de linhas resume os dados (soma, média, contagem…) em vez de travar.",
            "Fórmula colada numa coluna inteira para na última linha com dados (antes travava).",
            "O gráfico novo aparece na parte visível da planilha.",
            "O menu do botão direito no gráfico não some mais ao passar o mouse.",
            "Somar dias a uma data (=A1+2) mantém o formato de data.",
            "Um painel lateral por vez (gráfico ou tabela dinâmica).",
            "Ocultar linhas ou colunas até o fim da planilha.",
            "Abrir pela tela inicial mostra o carregamento.",
            "feathersheet --mcp liga o servidor MCP sem instalar nada a mais.",
            "A interface em inglês sem sobras em português (borda, console F12, Configurações).",
            "No Windows, o app abre sem a janela de console (cmd) junto.",
            "No Windows, o monitor (F12) mostra a memória e o processador.",
            "Imagem dentro da célula: a fórmula IMAGEM e as figuras \"Colocar na célula\" do Excel aparecem.",
            "Várias áreas de impressão por aba, cada uma nas suas páginas.",
            "AutoSoma (e Média, Contagem, Máximo, Mínimo) já grava o resultado, sem esperar o Enter.",
            "Tab durante a edição confirma e anda para a direita.",
            "Excluir a primeira linha de um intervalo encolhe o intervalo, como no Excel (antes virava #REF!).",
            "Gantt com datas mostra as datas no eixo.",
            "Contar distintos nas séries de texto do gráfico, e trocar a categoria sem travar.",
        ],
    },
    Versao {
        numero: "0.0.5",
        data: "16/10/2026",
        resumo: "Interface em inglês, Layout da Página e .xlsx muito mais rápido",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("world", "Interface em inglês", "Menus, faixa, janelas, Ajuda e fórmulas mudam de idioma em Configurações."),
            ("printer", "Aba Layout da Página", "Margens, orientação, tamanho, área de impressão, quebras e escala na faixa."),
            ("palette", "Formatar células refeito", "Seis abas: número, alinhamento, fonte, borda, preenchimento e proteção."),
            ("bolt", "Abre .xlsx 3,6× mais rápido", "A Planilha Pesada (7 milhões de fórmulas) fica pronta em 9,7 s, antes 34 s."),
            ("table", "Somar e contar por cor", "SOMAR.POR.COR e CONT.SE.COR pela cor do fundo ou da fonte da célula."),
            ("chart-bar", "Dica do mouse por tipo", "Gantt mostra início, término e duração; velas, os quatro preços; cascata, o acumulado."),
        ],
        tambem: &[
            "Funções sem nome em português ganharam um (MULT, TETO e outras); o autocompletar segue o idioma.",
            "Seus dados (nome, iniciais e organização) em formulário alinhado.",
            "Delete apaga a forma ou o gráfico selecionado; a barra de fórmulas não fica com o foco.",
            "Barra de progresso por etapas ao abrir arquivos grandes.",
            "Menos memória ao abrir pastas grandes.",
            "O monitor (F12) e a barra de baixo também em inglês.",
        ],
    },
    Versao {
        numero: "0.0.4",
        data: "16/10/2026",
        resumo: "Macros dentro do arquivo e novidades ao atualizar",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("sparkles", "O que há de novo ao atualizar", "Quem vem de uma versão anterior vê esta janela na primeira abertura."),
            ("rocket", "Primeiros passos na instalação", "O assistente abre sozinho só numa instalação nova; depois, no menu Ajuda."),
            ("code", "Macros dentro do arquivo", "Cada planilha guarda as suas macros no .fshz; uma nova começa sem nenhuma."),
        ],
        tambem: &[
            "Na 0.0.3, quem atualizava da 0.0.2 recebia os Primeiros passos em vez das novidades.",
            "Atribuir macro e o editor (Alt+F11) não mostram mais macros de outros arquivos.",
        ],
    },
    Versao {
        numero: "0.0.3",
        data: "16/10/2026",
        resumo: "Gráficos no visual novo e recuperação automática",
        ilustracao: Ilustracao::Grafico,
        destaques: &[
            ("chart-bar", "25 tipos de gráfico", "Todos no visual novo, inclusive combinado, cascata, radar, Gantt e mapa de calor."),
            ("pointer", "Dica do mouse", "Passe o mouse num ponto para ver o valor; o texto da dica é personalizável."),
            ("list", "Legenda clicável", "Clique numa série da legenda para ocultá-la, com transição suave."),
            ("chart-line", "Elementos analíticos", "Linha de tendência, barras de erro, máximo, mínimo e média."),
            ("history", "Recuperação automática", "Se o computador desligar, a planilha alterada volta pela tela inicial."),
            ("arrow-back-up", "Ctrl+Z nos gráficos", "Mover e redimensionar um gráfico agora se desfaz como qualquer edição."),
            ("rocket", "Primeiros passos", "Assistente da primeira abertura e esta janela de novidades."),
            ("user", "Seus dados", "Seu nome assina os comentários e vai nas propriedades do .fshz e do .xlsx."),
        ],
        tambem: &[
            "Shift + roda do mouse rola a planilha na horizontal.",
            "Painel de formatação do gráfico com grupos separados por divisórias.",
            "Gráficos importados do .xlsx sem preenchimento voltam a ter fundo.",
            "A barra de abertura do .parquet conta linhas, não megabytes.",
            "O rodapé (Contagem, Soma) não para mais no meio da seleção em pastas novas.",
        ],
    },
    Versao {
        numero: "0.0.2",
        data: "16/10/2026",
        resumo: "Atualização pelo app e grade mais leve",
        ilustracao: Ilustracao::Atualizacao,
        destaques: &[
            ("refresh", "Procurar atualização", "Em Configurações, Sobre: consulta a versão nova e instala à vista, num terminal."),
            ("table", "Grade mais leve", "O desenho da grade fica guardado e só é refeito quando algo muda."),
            ("device-desktop", "Nome e logo no sistema", "O monitor do sistema do Linux mostra o FeatherSheet com nome e ícone."),
        ],
        tambem: &[
            "O cursor só é acompanhado na janela principal, sem lentidão com outra janela aberta.",
            "Identificação da janela e ícone corrigidos.",
        ],
    },
    Versao {
        numero: "0.0.1",
        data: "15/10/2026",
        resumo: "Primeira versão pública",
        ilustracao: Ilustracao::Planilha,
        destaques: &[
            ("table", "Planilha nativa", "Motor e interface em Rust, rápidos com arquivos grandes."),
            ("math-function", "Funções de planilha", "Nomes em português e em inglês, com ajuda e exemplos na F1."),
            ("code", "Macros", "Editor de macros em JavaScript (Alt+F11) e importação de VBA."),
            ("device-floppy", "Instaladores", "Pacote .deb, .tar.gz e instalador do Windows."),
        ],
        tambem: &[
            "Repositório próprio, com a paridade com o Excel descrita no README.",
        ],
    },
];

#[derive(Debug, Clone)]
pub enum Evento {
    Versao(usize),
    VerNoGithub,
    Fechar,
}

impl App {
    pub(super) fn evento_novidades(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Versao(i) => self.novidades_versao = i.min(VERSOES.len() - 1),
            Evento::VerNoGithub => {
                let v = &VERSOES[self.novidades_versao.min(VERSOES.len() - 1)];
                if !cfg!(test) {
                    super::atualizacao::abrir_endereco(&format!("{}/tag/v{}", super::atualizacao::PAGINA_VERSOES, v.numero));
                }
            }
            Evento::Fechar => {
                if let Some(id) = self.janelas.id(Auxiliar::Novidades) {
                    return self.update(Msg::Fechar(id));
                }
            }
        }
        Task::none()
    }

    pub(super) fn view_novidades(&self) -> Element<'_, Msg> {
        feather_ui::novidades::tela(VERSOES, self.novidades_versao, super::atualizacao::versao_atual(), 3, |k| tr!(k).to_owned(),
            feather_ui::novidades::Acoes { escolher: |i| Msg::Novidades(Evento::Versao(i)),
                ver_no_github: Msg::Novidades(Evento::VerNoGithub), fechar: Msg::Novidades(Evento::Fechar) })
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Os textos ficam nesta tabela e passam por `tr!` em tempo de execução: a varredura
    /// das chaves do `idioma` não os enxerga, então a 0.1.5 e a 0.2.0 saíram sem tradução.
    #[test]
    fn toda_versao_tem_traducao() {
        use crate::idioma::{dicionario_de, Lingua};
        let faltam: Vec<String> = VERSOES.iter().flat_map(|v| {
            std::iter::once(v.resumo)
                .chain(v.destaques.iter().flat_map(|(_, t, d)| [*t, *d]))
                .chain(v.tambem.iter().copied())
                .flat_map(move |k| [Lingua::En, Lingua::Es, Lingua::Fr].into_iter()
                    .filter(move |l| !dicionario_de(*l).contains_key(k))
                    .map(move |l| format!("{} {}: {k:?}", v.numero, l.codigo())))
        }).collect();
        assert!(faltam.is_empty(), "{} sem tradução:\n{}", faltam.len(), faltam.join("\n"));
    }

    #[test]
    fn versao_desta_compilacao_e_a_primeira_e_todas_desenham() {
        assert_eq!(VERSOES[0].numero, super::super::atualizacao::versao_atual(),
            "acrescente as novidades da versão nova em VERSOES");
        let mut app = App::default();
        for i in 0..VERSOES.len() + 1 {
            let _ = app.update(Msg::Novidades(Evento::Versao(i)));
            let _: Element<'_, Msg> = app.view_novidades();
        }
        assert_eq!(app.novidades_versao, VERSOES.len() - 1);
        for v in VERSOES {
            for (icone, _, descricao) in v.destaques {
                assert!(feather_ui::icones::existe(icone), "{icone}");
                assert!(descricao.chars().count() <= 90, "{descricao}");
            }
        }
    }
}
