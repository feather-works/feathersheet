# Paridade com o Excel — checklist de comandos

> Levantamento realizado em 13/10/2026, conferindo botão por botão da interface Rust/Iced (`feathersheet-gui`). O inventário contém 290 comandos para avaliar a cobertura funcional do FeatherSheet em relação ao Excel. Conta como **implementado** o comando que tem botão, menu, atalho ou botão direito ligado a uma ação de verdade; botão desabilitado ("Em breve") conta como **faltando**. Mede só **amplitude**, não profundidade nem fidelidade. A avaliação geral e o desempenho estão em [`PARIDADE-AVALIACAO.md`](PARIDADE-AVALIACAO.md).

**Geral: 212 / 290 comandos implementados (73,1%).** A aba Desenvolvedor (macros) não entra nesse total: fica à parte, em "Desenvolvedor: 4 / 19" (seção no fim do checklist). Atualizado em 15/10 (tarde): Página Inicial fechada, 93 / 94 — Sublinhado ▾ com sublinhado duplo, caneta de borda (Desenhar borda, grade, Apagar, cor e estilo da linha), recuo pela faixa e por Ctrl+Alt+Tab, Limpar tudo e Limpar hiperlinks no Limpar ▾, Reaplicar no Classificar e filtrar ▾ (6 comandos); Suplementos fica fora do escopo. Antes, no mesmo dia: Preenchimento relâmpago (Ctrl+E, Preencher ▾ e Dados > Ferramentas de dados; 2 comandos). Antes, no mesmo dia: comentários (anotações) na aba Revisão nova, em Inserir > Comentário e em Limpar ▾ > Limpar comentários (8 comandos). Antes, no mesmo dia, Inserir ganhou Formas (`c5543c9f`), Ícones (`01c9ed76`), Caixa de seleção (`316961b9`, `92dcf633`) e WordArt (`9970bdf2`). Em 14/10 (noite): aba Desenhar inteira (Caneta, Borracha, Laço, Tinta para forma). Antes, à tarde: Desproteger planilha (o mesmo botão de Proteger, contado à parte); Proteger planilha; Inserir/Excluir células, Novo estilo de célula, Largura padrão e a estrutura de tópicos da aba Dados (Agrupar, Desagrupar, Subtotal, Mostrar e Ocultar detalhe). Antes, no mesmo dia: aba Fórmulas na faixa (recentes, Financeira, Usar em fórmula, rastrear precedentes/dependentes, remover setas, opções de cálculo, F9 e Shift+F9).

> **Recontagem parcial em 02/11/2026** (leitura do código, sem rodar o app): Inserir 17/25 (gráficos
> recomendados, gráfico dinâmico, segmentação, caixa de texto, cabeçalho e rodapé, símbolo), Layout da
> Página 10/23 (margens, quebras), Dados 20/28 (da Web, circular dados inválidos), Revisão 8/18 (proteger
> pasta). Total estimado: **~222 / 290 (77%)**. As outras abas não foram recontadas; a tabela abaixo é a de 15/10.

| Aba | Implementados | Total | Paridade |
|---|---|---|---|
| Arquivo | 10 | 11 | 91% |
| Página Inicial | 93 | 94 | 99% |
| Inserir | 11 | 25 | 44% |
| Desenhar | 4 | 4 | 100% |
| Layout da Página | 8 | 23 | 35% |
| Fórmulas | 23 | 23 | 100% |
| Dados | 18 | 28 | 64% |
| Revisão | 7 | 18 | 39% |
| Exibir | 9 | 20 | 45% |
| Automatizar | 2 | 3 | 67% |
| Design da Tabela | 8 | 13 | 62% |
| Design do Gráfico | 4 | 8 | 50% |
| Formatar | 0 | 3 | 0% |
| Análise de Tabela Dinâmica | 11 | 13 | 85% |
| Design de Tabela Dinâmica | 3 | 4 | 75% |

## Arquivo (10 / 11, 91%)

Implementado:

- `file.new` Novo (Arquivo) — Menu → Novo · Ctrl+N
- `file.open` Abrir (Arquivo) — Menu → Abrir · Ctrl+O
- `file.save` Salvar (Arquivo) — Menu → Salvar · Ctrl+S
- `file.saveAs` Salvar como (Arquivo) — Menu → Salvar como
- `file.close` Fechar (Arquivo) — Menu → Fechar
- `file.print` Imprimir (Arquivo) — Menu → Imprimir
- `file.exportPdf` Exportar para PDF (Exportar) — Imprimir → PDF
- `file.exportCsv` CSV (Exportar) — Menu → Exportar → CSV/TSV
- `file.properties` Propriedades (Informações) — Menu → Informações
- `file.options` Opções (Opções) — Menu → Opções (Configurações)

Faltando:

- `file.exportHtml` Página da Web (Exportar)

## Página Inicial (93 / 94, 99%)

Implementado:

- `edit.paste` Colar (Área de transferência) — Ctrl+V · faixa
- `edit.pasteSpecial` Colar especial (Área de transferência) — Ctrl+Shift+V · menu Colar
- `edit.cut` Recortar (Área de transferência) — Ctrl+X · faixa
- `edit.copy` Copiar (Área de transferência) — Ctrl+C · faixa
- `edit.formatPainter` Pincel de formatação (Área de transferência) — Pincel
- `home.fontName` Fonte (Fonte) — lista Fonte
- `home.fontSize` Tamanho da fonte (Fonte) — lista Tamanho
- `home.increaseFontSize` Aumentar fonte (Fonte) — A+
- `home.decreaseFontSize` Diminuir fonte (Fonte) — A−
- `home.bold` Negrito (Fonte) — Ctrl+B
- `home.italic` Itálico (Fonte) — Ctrl+I
- `home.underline` Sublinhado (Fonte) — Ctrl+U
- `home.strikethrough` Tachado (Fonte) — faixa
- `home.superscript` Sobrescrito (Fonte) — Formatar células → Fonte
- `home.subscript` Subscrito (Fonte) — Formatar células → Fonte
- `home.borders` Bordas (Fonte) — menu Bordas
- `home.fillColor` Cor de preenchimento (Fonte) — balde
- `home.fontColor` Cor da fonte (Fonte) — A colorido
- `home.alignTop` Alinhar em cima (Alinhamento) — faixa
- `home.alignMiddle` Alinhar no meio (Alinhamento) — faixa
- `home.alignBottom` Alinhar embaixo (Alinhamento) — faixa
- `home.orientation` Orientação (Alinhamento) — menu Orientação
- `home.alignLeft` Alinhar à esquerda (Alinhamento) — faixa
- `home.alignCenter` Centralizar (Alinhamento) — faixa
- `home.alignRight` Alinhar à direita (Alinhamento) — faixa
- `home.wrapText` Quebrar texto (Alinhamento) — Quebrar
- `home.mergeCells` Mesclar células (Alinhamento) — Mesclar
- `home.unmergeCells` Desfazer mesclagem (Alinhamento) — Mesclar (alterna) · Mesclar ▾ → Desmesclar células
- `home.mergeCenter` Mesclar e centralizar (Alinhamento) — Mesclar ▾ (13/10)
- `home.mergeAcross` Mesclar através (Alinhamento) — Mesclar ▾ (13/10)
- `home.numberFormat` Formato de número (Número) — lista Formato
- `home.accounting` Formato de moeda (Número) — R$
- `home.percent` Estilo de porcentagem (Número) — % · Ctrl+Shift+%
- `home.comma` Separador de milhares (Número) — .000
- `home.increaseDecimal` Aumentar casas decimais (Número) — .0→
- `home.decreaseDecimal` Diminuir casas decimais (Número) — .0←
- `home.conditionalFormat` Formatação condicional (Estilos) — Formatação condicional
- `home.clearRules` Limpar regras (Estilos) — gerenciador → Excluir regra
- `home.manageRules` Gerenciar regras (Estilos) — Gerenciar regras…
- `home.formatAsTable` Formatar como tabela (Estilos) — Tabela ▾
- `home.cellStyle` Estilos de célula (Estilos) — Estilos de célula ▾
- `home.insertRows` Inserir linhas (Células) — Inserir linha
- `home.insertColumns` Inserir colunas (Células) — botão direito → Inserir coluna
- `home.insertSheet` Inserir planilha (Células) — Nova aba · Shift+F11
- `home.deleteRows` Excluir linhas (Células) — Excluir linha
- `home.deleteColumns` Excluir colunas (Células) — botão direito → Excluir coluna
- `home.deleteSheet` Excluir planilha (Células) — botão direito na aba
- `home.autofitRowHeight` AutoAjuste da altura (Células) — Formatar ▾ · duplo clique na divisória
- `home.autofitColumnWidth` AutoAjuste da largura (Células) — Formatar ▾ · duplo clique na divisória
- `home.rowHeight` Altura da linha (Células) — Formatar ▾ → Altura da linha… (13/10)
- `home.columnWidth` Largura da coluna (Células) — Formatar ▾ → Largura da coluna… (13/10)
- `home.hideRows` Ocultar linhas (Células) — Exibir → Ocultar linhas
- `home.hideColumns` Ocultar colunas (Células) — Exibir → Ocultar colunas
- `home.unhideRows` Reexibir linhas (Células) — Exibir → Reexibir linhas
- `home.unhideColumns` Reexibir colunas (Células) — Exibir → Reexibir colunas
- `sheet.hide` Ocultar planilha (Células) — botão direito na aba
- `sheet.unhide` Reexibir planilha (Células) — botão direito → Reexibir abas
- `sheet.rename` Renomear planilha (Células) — botão direito → Renomear
- `sheet.move` Mover ou copiar planilha (Células) — botão direito → Mover / Duplicar
- `sheet.tabColor` Cor da guia (Células) — botão direito → Cor da aba
- `home.lockCell` Bloquear célula (Células) — Formatar células → Proteção
- `home.formatCells` Formatar células (Células) — Formatar células
- `formulas.autoSum` AutoSoma (Edição) — Σ · Alt+=
- `edit.fillDown` Preencher para baixo (Edição) — Preencher · Ctrl+D
- `edit.fillRight` Preencher à direita (Edição) — Preencher · Ctrl+R
- `edit.fillUp` Preencher para cima (Edição) — Preencher
- `edit.fillLeft` Preencher à esquerda (Edição) — Preencher
- `edit.fillSeries` Série (Edição) — Preencher → Continuar até o fim · alça
- `edit.flashFill` Preenchimento relâmpago (Edição) — Preencher ▾ → Preenchimento relâmpago · Ctrl+E
- `edit.clearFormats` Limpar formatos (Edição) — Limpar formato
- `edit.clearContents` Limpar conteúdo (Edição) — Delete · botão direito
- `edit.clearHyperlinks` Limpar hiperlinks (Edição) — Remover link
- `edit.clearComments` Limpar comentários (Edição) — Limpar ▾ → Limpar comentários (15/10)
- `data.sortAscending` Classificar de A a Z (Edição) — A–Z
- `data.sortDescending` Classificar de Z a A (Edição) — Z–A
- `data.filter` Filtro (Edição) — AutoFiltro
- `data.clearFilter` Limpar filtro (Edição) — Alternar / Remover filtros
- `edit.find` Localizar (Edição) — Ctrl+F
- `edit.replace` Substituir (Edição) — Localizar e substituir
- `edit.goTo` Ir para (Edição) — caixa de nome · F5 / Ctrl+G
- `data.sort` Classificação personalizada (Edição) — Classificar e filtrar → Classificação personalizada…
- `edit.goToSpecial` Ir para especial (Edição) — F5 / Ctrl+G → opções do Especial
- `home.newCellStyle` Novo estilo de célula (Estilos) — Estilos de célula ▾ → Novo estilo de célula…
- `home.insertCells` Inserir células (Células) — Ctrl++ · Inserir ▾ · botão direito
- `home.deleteCells` Excluir células (Células) — Ctrl+- · Excluir ▾ · botão direito
- `home.defaultWidth` Largura padrão (Células) — Formatar ▾ → Largura padrão
- `review.protectSheet` Proteger planilha (Células) — Formatar ▾ → Proteger ou desproteger planilha · Bloquear/desbloquear célula
- `home.doubleUnderline` Sublinhado duplo (Fonte) — Sublinhado ▾ → Sublinhado duplo · Formatar células → Fonte (15/10; `.fshz` e `.xlsx` `<u val="double"/>`)
- `home.drawBorder` Desenhar borda (Fonte) — Bordas ▾ → Desenhar borda / Desenhar grade de borda / Apagar borda, cor e estilo da linha; arrastar na grade, um Ctrl+Z por traço, Esc sai (15/10; estilo da linha também no `.xlsx`)
- `home.decreaseIndent` Diminuir recuo (Alinhamento) — faixa · Ctrl+Alt+Shift+Tab (c6113971; atalho 15/10)
- `home.increaseIndent` Aumentar recuo (Alinhamento) — faixa · Ctrl+Alt+Tab (c6113971; atalho 15/10)
- `edit.clearAll` Limpar tudo (Edição) — Limpar ▾ → Limpar tudo (junto: Limpar hiperlinks da seleção, 15/10)
- `data.reapply` Reaplicar (Edição) — Classificar e filtrar ▾ → Reaplicar: ordenação e filtros da tabela/AutoFiltro de novo, um Ctrl+Z (15/10)

Fora do escopo:

- `home.addIns` Suplementos (Suplementos) — a suíte não tem loja nem extensões de terceiros (privacidade por padrão); não entra.

## Inserir (12 / 25, 48%)

Implementado:

- `insert.pivotTable` Tabela dinâmica (Tabelas) — Tabela dinâmica
- `insert.recommendedPivotTables` Tabelas dinâmicas recomendadas (Tabelas) — Dinâmicas recomendadas
- `insert.table` Tabela (Tabelas) — Tabela · Ctrl+T
- `insert.picture` Imagens (Ilustrações) — Imagens
- `insert.chart` Gráficos (Gráficos) — Gráfico ▾
- `insert.link` Link (Links) — Inserir link
- `insert.shape` Formas (Ilustrações) — Formas ▾: camada de formas sobre a grade (`feather-shapes`), mover, alças, girar, barra flutuante (`c5543c9f`, 14/10)
- `insert.icons` Ícones (Ilustrações) — galeria Tabler com busca e rolagem virtualizada; o ícone é objeto da camada (`01c9ed76`, 14/10)
- `insert.checkbox` Caixa de seleção (Controles de célula) — Checkbox: caixas na seleção (VERDADEIRO/FALSO), clique e Espaço marcam, o botão aceso remove (`316961b9`, `92dcf633`, 15/10)
- `insert.wordArt` WordArt (Texto) — Inserir > WordArt: 4 estilos, deformação, sombra, texto editável (`9970bdf2`, 15/10)
- `review.newComment` Comentário (Comentários) — Inserir > Comentário abre o editor de comentário da célula ativa (15/10)
- `insert.sparkline` Minigráficos (Minigráficos) — Linha, Coluna e Ganhos/Perdas: diálogo Criar Minigráficos, aba contextual Minigráfico (tipo, pontos em destaque, estilo, mesmo eixo, limpar), `.fshz` e `.xlsx` (04/11)

Faltando:

- `insert.recommendedCharts` Gráficos recomendados (Gráficos)
- `insert.pivotChart` Gráfico dinâmico (Gráficos)
- `insert.slicer` Segmentação de dados (Filtros) — botão desabilitado com a dica "Em breve"
- `insert.textBox` Caixa de texto (Texto)
- `pageLayout.headerFooter` Cabeçalho e rodapé (Texto)
- `insert.symbol` Símbolo (Símbolos)
- `insert.smartArt` SmartArt (Ilustrações)
- `insert.screenshot` Instantâneo (Ilustrações)
- `insert.map` Mapas (Gráficos)
- `insert.timeline` Linha do tempo (Filtros) — botão desabilitado com a dica "Em breve"
- `insert.signatureLine` Linha de assinatura (Texto)
- `insert.object` Objeto (Texto)
- `insert.equation` Equação (Símbolos)

## Desenhar (4 / 4, 100%)

Aba **Desenhar** na faixa desde 14/10 (`app/desenhar.rs`). Os controles e a camada de tinta são do
`feather-ui` (`tinta`, v0.2.5) e o motor dos traços é do `feather-draw` (`tinta`), para o
FeatherSlide e o FeatherDocs usarem igual. A tinta fica presa às células (âncora + deslocamento),
acompanha rolagem, zoom e largura/altura de coluna/linha, vai no `.fshz` (campo `tn` da aba) e sai no
`.xlsx` como forma livre DrawingML. Cada traço, apagar ou forma é um Ctrl+Z; Esc sai do modo desenho.

Implementado:

- `draw.pen` Caneta (Ferramentas) — três canetas e marca-texto, com menu de cor e espessura (14/10)
- `draw.eraser` Borracha (Ferramentas) — de traço e de ponto (divide o traço) (14/10)
- `draw.lasso` Seleção com laço (Ferramentas) — seleciona os traços cercados; Delete apaga (14/10)
- `draw.inkToShape` Tinta para forma (Converter) — linha, seta, retângulo, triângulo, elipse e círculo (14/10)

Faltando (fora da contagem do Excel): mover/redimensionar a seleção do laço, pressão da caneta
(o Iced 0.13 não entrega), tinta para texto/matemática, reproduzir tinta e importar a tinta do `.xlsx`.

## Layout da Página (8 / 23, 35%)

Implementado:

- `pageLayout.orientation` Orientação (Configurar página) — Imprimir → Paisagem/Retrato
- `pageLayout.size` Tamanho (Configurar página) — Imprimir → A4/Carta/Ofício
- `pageLayout.printArea` Área de impressão (Configurar página) — Exibir → Definir/Limpar área
- `pageLayout.pageSetup` Configurar página (Configurar página) — Exibir → Visualizar
- `pageLayout.scaleToFit` Dimensionar para ajustar (Dimensionar) — Imprimir → Ajustar à 1 página
- `view.gridlines` Exibir linhas de grade (Opções de planilha) — Exibir → Linhas de grade
- `pageLayout.printGridlines` Imprimir linhas de grade (Opções de planilha) — Imprimir → linhas de grade
- `pageLayout.printHeadings` Imprimir títulos (Opções de planilha) — Imprimir → títulos A,B,C e 1,2,3

Faltando:

- `pageLayout.theme` Temas (Temas)
- `pageLayout.themeColors` Cores (Temas)
- `pageLayout.themeFonts` Fontes (Temas)
- `pageLayout.themeEffects` Efeitos (Temas)
- `pageLayout.margins` Margens (Configurar página)
- `pageLayout.breaks` Quebras (Configurar página)
- `pageLayout.background` Plano de fundo (Configurar página)
- `pageLayout.printTitles` Imprimir títulos (Configurar página)
- `view.headings` Exibir títulos (Opções de planilha)
- `arrange.bringForward` Avançar (Organizar)
- `arrange.sendBackward` Recuar (Organizar)
- `arrange.selectionPane` Painel de seleção (Organizar)
- `arrange.align` Alinhar (Organizar)
- `arrange.group` Agrupar (Organizar)
- `arrange.rotate` Girar (Organizar)

## Fórmulas (23 / 23, 100%)

Aba **Fórmulas** própria na faixa desde 14/10 (`app/formulas_ui.rs`); as setas são desenhadas na
grade por `grade/setas.rs`, e as contas são do motor (`planilha/sheet/auditoria.rs`,
`planilha/calculo_manual.rs`).

Implementado:

- `formulas.insertFunction` Inserir função (Biblioteca de funções) — catálogo de funções (Inserir) e menus por categoria
- `formulas.recentlyUsed` Usadas recentemente (Biblioteca de funções) — Recentes ▾, as 10 últimas inseridas (guardadas na configuração)
- `formulas.financial` Financeira (Biblioteca de funções) — Financeira ▾
- `formulas.logical` Lógica (Biblioteca de funções) — Lógica ▾
- `formulas.text` Texto (Biblioteca de funções) — Texto ▾
- `formulas.dateTime` Data e hora (Biblioteca de funções) — Data e hora ▾
- `formulas.lookupReference` Pesquisa e referência (Biblioteca de funções) — Pesquisa e referência ▾
- `formulas.mathTrig` Matemática e trigonometria (Biblioteca de funções) — Matemática ▾
- `formulas.moreFunctions` Mais funções (Biblioteca de funções) — Mais ▾: Estatística, Banco de dados, Matriz dinâmica, Engenharia, Fonte de dados
- `formulas.nameManager` Gerenciador de nomes (Nomes definidos)
- `formulas.defineName` Definir nome (Nomes definidos)
- `formulas.useInFormula` Usar em fórmula (Nomes definidos) — insere o nome na fórmula em edição
- `formulas.createFromSelection` Criar a partir da seleção (Nomes definidos) — títulos das bordas viram nomes (regra em `planilha/sheet/nomes_selecao.rs`)
- `formulas.tracePrecedents` Rastrear precedentes (Auditoria de fórmulas) — um nível por clique; intervalo contornado; outra aba tracejada; erro em vermelho
- `formulas.traceDependents` Rastrear dependentes (Auditoria de fórmulas) — um nível por clique
- `formulas.removeArrows` Remover setas (Auditoria de fórmulas) — as setas também somem quando a planilha muda
- `formulas.showFormulas` Mostrar fórmulas (Auditoria de fórmulas)
- `formulas.evaluateFormula` Avaliar fórmula (Auditoria de fórmulas) — passo a passo no motor (`formula/avaliar_passos.rs`), SE/SEERRO só no ramo escolhido, Entrar/Sair (`app/avaliar_ui.rs`)
- `formulas.watchWindow` Janela de inspeção (Auditoria de fórmulas) — janela própria (`app/inspecao_ui.rs`); lista vale para a sessão
- `formulas.errorChecking` Verificação de erros (Auditoria de fórmulas) — Explicar erro
- `formulas.calculationOptions` Opções de cálculo (Cálculo) — Automático/Manual; com conta esperando, o rodapé mostra "Calcular"
- `formulas.calculateNow` Calcular agora (Cálculo) — F9
- `formulas.calculateSheet` Calcular planilha (Cálculo) — Shift+F9

Faltando: nada.

Também na rodada: **Ctrl+/** seleciona a matriz derramada da célula ativa; o contorno da área
derramada passou a usar a cor da seleção da grade.

## Dados (18 / 28, 64%)

Implementado:

- `data.getData` Obter dados (Obter e transformar) — OpenQuery
- `data.fromTextCsv` De texto/CSV (Obter e transformar) — Menu → Importar · Abrir CSV/TSV
- `data.fromTableRange` Da tabela/intervalo (Obter e transformar) — OpenQuery
- `data.queriesConnections` Consultas e conexões (Consultas e conexões) — OpenQuery
- `data.advancedFilter` Avançado (Classificar e filtrar) — Filtrar por condição
- `data.textToColumns` Texto para colunas (Ferramentas de dados) — Dividir texto
- `data.removeDuplicates` Remover duplicatas (Ferramentas de dados) — Duplicados
- `data.flashFill` Preenchimento relâmpago (Ferramentas de dados) — Dados → Relâmpago · Ctrl+E
- `data.validation` Validação de dados (Ferramentas de dados) — Validação de dados
- `data.analysisToolPak` Análise de dados (Análise) — Estatísticas · Perfil da coluna
- `data.scenarioManager` Gerenciador de cenários (Previsão) — Dados → Cenários
- `data.goalSeek` Atingir meta (Previsão) — Dados → Atingir meta…
- `data.dataTable` Tabela de dados (Previsão) — Dados → Tabela de dados… (valores fixos)
- `data.group` Agrupar (Estrutura de tópicos) — Dados → Agrupar · Alt+Shift+→
- `data.ungroup` Desagrupar (Estrutura de tópicos) — Dados → Desagrupar · Alt+Shift+←
- `data.subtotal` Subtotal (Estrutura de tópicos) — Dados → Subtotal (janela, Remover todos)
- `data.showDetail` Mostrar detalhe (Estrutura de tópicos) — Dados → Mostrar detalhe · [+] na grade
- `data.hideDetail` Ocultar detalhe (Estrutura de tópicos) — Dados → Ocultar detalhe · [−] na grade

Faltando:

- `data.refreshAll` Atualizar tudo (Consultas e conexões)
- `data.circleInvalid` Circular dados inválidos (Ferramentas de dados)
- `data.consolidate` Consolidar (Ferramentas de dados)
- `data.forecastSheet` Planilha de previsão (Previsão)
- `data.solver` Solver (Análise)
- `data.fromWeb` Da Web (Obter e transformar)
- `data.stocks` Ações (Tipos de dados)
- `data.geography` Geografia (Tipos de dados)
- `data.relationships` Relações (Ferramentas de dados)
- `data.analyzeData` Analisar dados (Análise)

## Revisão (7 / 18, 39%)

Implementado:

- `review.unprotectSheet` Desproteger planilha (Proteger) — Formatar ▾ → Proteger ou desproteger planilha (mesmo botão de `review.protectSheet`, contado à parte); também em Revisão → Proteger planilha
- `review.newNote` Nova anotação (Anotações) — Revisão → Novo comentário (vira "Editar comentário" quando a célula já tem); botão direito → Inserir/Editar comentário. Autor = nome do usuário do sistema; um Ctrl+Z (15/10)
- `review.deleteComment` Excluir comentário (Comentários) — Revisão → Excluir (comentários da seleção), botão direito, botão no editor
- `review.previousComment` Comentário anterior (Comentários) — Revisão → Anterior (dá a volta na aba)
- `review.nextComment` Próximo comentário (Comentários) — Revisão → Próximo
- `review.showNote` Mostrar/ocultar anotação (Anotações) — Revisão → Mostrar/ocultar comentário; botão direito (gravado no arquivo: `visivel`)
- `review.showAllNotes` Mostrar todas as anotações (Anotações) — Revisão → Mostrar todos os comentários (só a tela)

Faltando:

- `review.spelling` Ortografia (Revisão de texto)
- `review.thesaurus` Dicionário de sinônimos (Revisão de texto)
- `review.workbookStatistics` Estatísticas da pasta (Revisão de texto)
- `review.checkAccessibility` Verificar acessibilidade (Acessibilidade)
- `review.showComments` Mostrar comentários (Comentários) — o painel lateral de comentários encadeados não existe
- `review.convertToComments` Converter em comentários (Anotações)
- `review.protectWorkbook` Proteger pasta de trabalho (Proteger)
- `review.allowEditRanges` Permitir edição de intervalos (Proteger)
- `review.smartLookup` Pesquisa inteligente (Ideias)
- `review.translate` Traduzir (Idioma)
- `review.showChanges` Mostrar alterações (Alterações)

## Exibir (9 / 20, 45%)

Implementado:

- `view.pageBreakPreview` Visualização da quebra de página (Modos de exibição) — Exibir → Quebras de página
- `view.formulaBar` Barra de fórmulas (Mostrar) — Exibir → Barra de fórmulas
- `view.zoom` Zoom (Zoom) — lista Zoom · rodapé
- `view.zoom100` 100% (Zoom) — clicar no % do rodapé
- `view.freezePanes` Congelar painéis (Janela) — Congelar ▾
- `view.freezeTopRow` Congelar linha superior (Janela) — Congelar → Até a linha atual
- `view.freezeFirstColumn` Congelar primeira coluna (Janela) — Congelar → Até a coluna atual
- `view.unfreezePanes` Descongelar painéis (Janela) — Congelar → Descongelar
- `view.macros` Macros (Macros) — Macros → Editor · Alt+F11

Faltando:

- `view.normal` Normal (Modos de exibição)
- `view.pageLayout` Layout da página (Modos de exibição)
- `view.customViews` Modos de exibição personalizados (Modos de exibição)
- `view.ruler` Régua (Mostrar)
- `view.zoomToSelection` Zoom na seleção (Zoom)
- `view.newWindow` Nova janela (Janela)
- `view.arrangeAll` Organizar tudo (Janela)
- `view.split` Dividir (Janela)
- `view.hideWindow` Ocultar janela (Janela)
- `view.unhideWindow` Reexibir janela (Janela)
- `view.switchWindows` Alternar janelas (Janela)

## Automatizar (2 / 3, 67%)

Implementado:

- `automate.newScript` Novo script (Ferramentas de script) — Macros → Novo módulo
- `automate.allScripts` Todos os scripts (Ferramentas de script) — Editor de macros

Faltando:

- `automate.recordActions` Gravar ações (Ferramentas de script)

## Design da Tabela (8 / 13, 62%)

Implementado:

- `table.rename` Nome da tabela (Propriedades) — Renomear
- `table.resize` Redimensionar tabela (Propriedades) — Redimensionar
- `table.removeDuplicates` Remover duplicatas (Ferramentas) — Dados → Duplicados
- `table.convertToRange` Converter em intervalo (Ferramentas) — Converter
- `table.totalRow` Linha de totais (Opções de estilo) — Linha de totais
- `table.bandedRows` Linhas em tiras (Opções de estilo) — Faixas por linha
- `table.bandedColumns` Colunas em tiras (Opções de estilo) — Faixas por coluna
- `table.style` Estilos de tabela (Estilos de tabela) — Estilos de tabela ▾

Faltando:

- `table.summarizePivot` Resumir com tabela dinâmica (Ferramentas)
- `table.insertSlicer` Inserir segmentação (Ferramentas)
- `table.headerRow` Linha de cabeçalho (Opções de estilo)
- `table.firstColumn` Primeira coluna (Opções de estilo)
- `table.lastColumn` Última coluna (Opções de estilo)

## Design do Gráfico (4 / 8, 50%)

Implementado:

- `chart.addElement` Adicionar elemento (Layouts) — editor → título, legenda, eixos
- `chart.changeColors` Alterar cores (Estilos) — editor → Paleta
- `chart.selectData` Selecionar dados (Dados) — editor → intervalo
- `chart.changeType` Alterar tipo (Tipo) — editor → Tipo de gráfico

Faltando:

- `chart.quickLayout` Layout rápido (Layouts)
- `chart.set` Estilos de gráfico (Estilos)
- `chart.switchRowColumn` Alternar linha/coluna (Dados)
- `chart.move` Mover gráfico (Local)

## Formatar (0 / 3, 0%)

Faltando:

- `chart.formatSelection` Formatar seleção (Seleção atual)
- `chart.shapeFill` Preenchimento da forma (Estilos de forma)
- `chart.shapeOutline` Contorno da forma (Estilos de forma)

## Análise de Tabela Dinâmica (11 / 13, 85%)

Implementado:

- `pivot.fieldList` Lista de campos (Mostrar) — painel Campos
- `pivot.refresh` Atualizar (Dados) — Atualizar dinâmica
- `pivot.changeSource` Alterar fonte de dados (Dados) — painel → Intervalo de origem
- `pivot.calculatedField` Campos, itens e conjuntos (Cálculos) — Opções avançadas → campo calculado
- `pivot.addField` Adicionar campo (Campos) — painel → marcar campo
- `pivot.removeField` Remover campo (Campos) — painel → x
- `pivot.moveField` Mover campo (Campos) — painel → setas
- `pivot.valueSettings` Configurações do campo de valor (Campo ativo) — painel → Σ operação
- `pivot.filter` Filtro (Filtro) — painel → ícone de filtro
- `pivot.group` Agrupar campo (Agrupar) — Agrupar datas
- `pivot.collapse` Recolher campo (Campo ativo) — clicar no grupo · Recolher todos

Faltando:

- `pivot.sort` Classificar (Classificar)
- `pivot.delete` Limpar tudo (Ações)

## Design de Tabela Dinâmica (3 / 4, 75%)

Implementado:

- `pivot.subtotals` Subtotais (Layout) — Opções avançadas → Subtotais
- `pivot.reportLayout` Layout do relatório (Layout) — Opções avançadas → Layout
- `pivot.style` Estilos de tabela dinâmica (Estilos) — Opções avançadas → Tema

Faltando:

- `pivot.grandTotals` Totais gerais (Layout)

## Desenvolvedor (macros) (4 / 19, 21%)

Fora do "Geral": a aba Desenvolvedor do Excel é separada da contagem da faixa. No FeatherSheet as macros ficam na aba
**Macros** da faixa e no editor Alt+F11; os itens abaixo são conferidos contra essas peças.
Os controles de formulário e os XML não existem, e os suplementos também não.

Implementado:

- `developer.visualBasic` Visual Basic (Código) — aba Macros → Editor de macros · Alt+F11
- `developer.macros` Macros (Código) — aba Macros → Executar macro · Editar
- `developer.viewCode` Exibir código (Código) — aba Macros → Editar (mesmo editor de macros)
- `developer.properties` Propriedades (Controles) — editor Alt+F11 → Exibir → Propriedades

Faltando:

- `developer.recordMacro` Gravar macro (Código) — é `automate.recordActions`, a fazer
- `developer.relativeReferences` Usar referências relativas (Código)
- `developer.macroSecurity` Segurança de macro (Código) — não há central de confiança
- `developer.addIns` Suplementos (Suplementos)
- `developer.excelAddIns` Suplementos do Excel (Suplementos)
- `developer.comAddIns` Suplementos COM (Suplementos)
- `developer.insertControls` Inserir controles (Controles) — só o Botão de macro (aba Macros → Planilha); caixa de combinação, lista, opção etc. faltam
- `developer.designMode` Modo de design (Controles) — não há alternância; o designer só existe dentro do componente
- `developer.runDialog` Executar caixa de diálogo (Controles) — botão Componentes da aba Macros está desabilitado; o componente abre por macro (`sheet.showComponent`)
- `developer.xmlSource` Origem (XML)
- `developer.mapProperties` Propriedades do mapa (XML)
- `developer.expansionPacks` Pacotes de expansão (XML)
- `developer.refreshXml` Atualizar dados (XML)
- `developer.importXml` Importar (XML)
- `developer.exportXml` Exportar (XML)

---

# Paridade por funcionalidade

> Acrescentado em 13/10/2026 a partir da lista completa de funcionalidades do Excel que o
> usuário trouxe. Complementa o checklist de comandos acima (que mede botões da faixa): aqui
> cada linha é uma **funcionalidade**, conferida no código Rust/Iced. `[x]` feito · `[~]`
> parcial (existe, falta parte, ou só no motor sem interface) · `[ ]` falta. Itens de nuvem,
> conta ou servidor da Microsoft entram como falta, mas a regra de privacidade da suíte
> (nada sai da máquina sem pedido) decide se algum deles será feito.

**24 feitas, 41 parciais e 52 faltando, de 117 funcionalidades** (20,5% completas; 55,6% com alguma cobertura). Contagem refeita linha a linha em 15/10; Comentários passou a parcial no mesmo dia; Máscara de formato de número e Recuo continuam como estavam (em andamento). Preenchimento relâmpago passou a parcial em 15/10.

| Área | Feito | Parcial | Falta | Total |
|---|---|---|---|---|
| 1. Estruturação, entrada e manipulação de dados | 4 | 10 | 6 | 20 |
| 2. Formatação, tipografia e máscaras | 2 | 3 | 1 | 6 |
| 3. Engenharia de dados, ETL e modelagem | 0 | 3 | 4 | 7 |
| 4. Análise, estatística e simulação | 4 | 3 | 2 | 9 |
| 5. Motor de cálculo e endereçamento | 3 | 2 | 5 | 10 |
| 6. Visualização e elementos gráficos | 2 | 4 | 7 | 13 |
| 7. Automação, programação e extensibilidade | 1 | 3 | 3 | 7 |
| 8. Dados estruturados, XML, tempo real e hardware | 0 | 0 | 5 | 5 |
| 9. Auditoria, inspeção e governança | 5 | 2 | 2 | 9 |
| 10. Exibição, janelas, layout e impressão | 2 | 4 | 3 | 9 |
| 11. Colaboração, governança e segurança | 0 | 2 | 4 | 6 |
| 12. Desenho à tinta e dispositivos | 0 | 1 | 1 | 2 |
| 13. Acessibilidade, voz e idiomas | 0 | 0 | 7 | 7 |
| 14. Arquivos, recuperação e integração | 1 | 3 | 3 | 7 |
| **Total** | **24** | **41** | **52** | **117** |

## 1. Estruturação, entrada e manipulação de dados

- [x] Tabelas estruturadas: Ctrl+T, estilos, linha de totais, faixas, cresce sozinha, referência estruturada `Tabela1[Valor]`, nome da coluna no cabeçalho ao rolar.
- [~] Validação de dados: lista, intervalo, número, data, **hora**, texto e caixa (`operacoes/validation.rs`), com alerta Parar/Aviso/Informações (`QuandoValidacao::{Recusar,Avisar,Informar}`), título do alerta e **mensagem de entrada** (título + texto, `Validacao::mensagem_de_entrada`), e **Circular dados inválidos** no motor (`Workbook::circular_dados_invalidos`, 15/10/2026). `.xlsx` (e `.fshz`, mesmo JSON): lista, intervalo, outra aba, INDIRETO→INDIRECT, número, data, hora, `errorStyle`/`errorTitle`/`error`/`showInputMessage`/`promptTitle`/`prompt`; caixa, e-mail, URL, "contém" e "começa com" vão como `custom` com fórmula equivalente e voltam como a regra original (`xlsx_validacao_custom.rs`; e-mail/URL aproximados). GUI (15/10/2026): diálogo no componente comum (`app/validacao_dialogo.rs`) com abas Configurações (inclui Hora), Mensagem de entrada (título + texto) e Alerta de erro (Parar/Aviso/Informações, título e mensagem); o formulário preserva mensagens e a regra Hora; balão da mensagem de entrada ao selecionar a célula (`grade/validacao_visual.rs`); alerta ao digitar valor inválido com Repetir/Cancelar, Sim/Não/Cancelar ("Continuar?") e OK/Cancelar; Dados > Validação ▾ > Circular dados inválidos (elipses vermelhas, até 255, somem ao corrigir) e Limpar círculos. Falta: caixas "Ignorar em branco", "Mostrar mensagem de entrada" e "Mostrar alerta" (o motor não guarda os interruptores); mensagem multilinha; comprimento de texto como regra própria; personalizada de outra origem; a colagem ainda trata Informações como aviso na barra de status.
- [~] Preenchimento relâmpago (Flash Fill): Ctrl+E, Preencher ▾ e Dados > Ferramentas de dados (`operacoes/relampago.rs`, 15/10). Síntese por exemplos (1 a 3, mais os que não baterem, até 6): concatenação de constantes e subcadeias das colunas vizinhas, pontas por índice ou antes/depois de token (espaço, vírgula, @, ponto, dígitos, maiúsculas…), caixa MAIÚSCULA/minúscula/Inicial; vence o programa mais simples. Só preenche células vazias, para na primeira linha vazia do bloco, ignora cabeçalho, um Ctrl+Z; ambíguo ou sem padrão: não escreve e avisa na barra de status. Falta: a sugestão automática enquanto se digita (prévia em cinza do Excel), as opções de "Desfazer preenchimento relâmpago/aceitar sugestões" e transformações além de subcadeia (ex.: data por nome do mês).
- [~] AutoPreenchimento e séries: alça, Preencher para baixo/direita/cima/esquerda, série de números e datas (`operacoes/fill.rs`). Motor de Preencher > Série pronto (`operacoes/serie.rs`, `Sheet::preencher_serie`): linear, crescimento, datas por dia/dia útil/mês/ano, tendência linear e exponencial, limite, linhas ou colunas, um Ctrl+Z. Diálogo Preencher ▾ > Série… (`app/serie_ui.rs`, 15/10/2026): série em linhas/colunas, Linear, Crescimento, Data (Dia, Dia da semana, Mês, Ano), AutoPreenchimento (a regra da alça), Tendência, Incremento e Limite (aceita data); erros do motor no diálogo; um Ctrl+Z. Falta: Justificar.
- [ ] Listas personalizadas (para preencher e ordenar).
- [x] Texto para colunas: assistente "Dividir texto" com delimitadores e tipo por coluna (medido: 87,6 M células em 24 s). Largura fixa a conferir.
- [x] Remover duplicadas.
- [ ] Consolidar.
- [ ] Tipos de dados vinculados (Ações, Geografia…): dependem de serviço na nuvem.
- [ ] Dados da imagem (OCR de foto de tabela).
- [ ] Formulário de dados.
- [x] Escolher da lista suspensa: Alt+↓ lista os valores do bloco da coluna, em ordem alfabética (`app/entrada_rapida.rs`, 13/10).
- [~] Abas agrupadas e Ctrl+Enter: Ctrl+Enter preenche a seleção (todas as áreas), deslocando as referências da fórmula, num Ctrl+Z (13/10). Falta: abas agrupadas (`[Grupo]`).
- [~] Colar especial: janela Ctrl+Alt+V (`app/colar_especial.rs`, 13/10) com Tudo, Fórmulas, Valores, Formatos, Valores e formatos, Larguras das colunas; operações somar/subtrair/multiplicar/dividir (regra no motor, `operacoes/colagem.rs`); ignorar em branco; transpor; um Ctrl+Z. Falta: colar vínculo, validação, comentários, colar como imagem e imagem vinculada.
- [ ] Área de transferência com vários itens (até 24).
- [~] Localizar e substituir: janela flutuante e arrastável no estilo do Excel (13/10), abas Localizar/Substituir, Dentro (planilha/pasta), Pesquisar por linhas ou colunas, Examinar fórmulas/valores, diferenciar maiúsculas, célula inteira, Localizar tudo com Planilha/Célula/Valor/Fórmula; muito rápido em arquivo grande. Falta: buscar por formato e em comentários.
- [~] Ir para especial: janela F5/Ctrl+G (`app/ir_para.rs`, 13/10) com referência, em branco, constantes e fórmulas por tipo, diferenças na linha/coluna, região atual, última célula, somente visíveis, formatação condicional e validação; o resultado vira seleção de várias áreas (Ctrl+clique também, `app/selecao_multipla.rs`). Falta: precedentes, dependentes, objetos, comentários e matriz atual.
- [~] AutoFiltro: caixas de seleção, pesquisa e filtro por condição (`operacoes/filtro.rs`). **Motor pronto em 15/10/2026** (`filtro.rs`, `filtro_data.rs`; falta ligar na GUI): filtro por cor de preenchimento e da fonte, 10 primeiros/últimos (itens ou %), acima/abaixo da média e períodos de data (hoje, ontem, amanhã, semana, mês, trimestre, ano, acumulado no ano, todos de um mês/trimestre), com "hoje" injetável; lê e grava `filterColumn` (`colorFilter`+dxf, `top10`, `dynamicFilter`, `filters`) da tabela no .xlsx. **GUI ligada em 15/10/2026** (`app/tabelas/filtro_extra.rs`): no menu da seta da coluna, "Filtrar por cor ▸" (amostras de preenchimento e de fonte, "Sem preenchimento"/"Automático"), "Filtros de número ▸" (comparações, "10 primeiros…" com Primeiros/Últimos, N e Itens/Porcentagem, acima/abaixo da média) e "Filtros de data ▸" (hoje até acumulado no ano, "Todas as datas no período ▸" com trimestres e meses), só o submenu que combina com o tipo da coluna; item ativo marcado; "hoje" pela data local; filtro, alturas e JSON num só Ctrl+Z (até 200 mil linhas mudadas). Falta: filtro por ícone, "Entre…"/"Antes…"/"Depois…" de data, lista de valores agrupada por ano/mês nas datas, `customFilters` do .xlsx e `<autoFilter>` solto da aba (só o de tabela é lido).
- [~] Filtro avançado: "Filtrar por condição". Falta: tabela de critérios E/OU na planilha e extrair únicos para outro destino.
- [~] Classificação: A→Z, Z→A, pelo filtro e janela Classificar (`app/classificar.rs`, 13/10) com vários níveis (adicionar, copiar, mover, excluir), cabeçalho detectado e diferenciar maiúsculas (`ops::planejar_ordenacao_com`), num Ctrl+Z. **Motor pronto em 15/10/2026** (`ordenacao_ext.rs`; falta ligar na janela Classificar): por cor da célula/fonte (em cima ou embaixo), por lista personalizada (dias, meses pt-BR e lista dada) e da esquerda para a direita (por linha, valor ou cor), num Ctrl+Z; lê e grava `sortState`/`sortCondition` (`sortBy`, `customList`, `columnSort`) no .xlsx. **Menu da coluna ligado em 15/10/2026**: "Classificar por cor ▸" (cor da célula ou da fonte em cima) e "Classificar por lista ▸" (listas prontas e lista digitada), num Ctrl+Z com o refiltro; grava `ordenacao` na tabela. Falta: janela Classificar com os níveis novos, por ícone, listas personalizadas do usuário salvas.

## 2. Formatação, tipografia e máscaras

- [~] Formato de número: automático, número, inteiro, moeda, contábil, científico, porcentagem, data, data e hora, hora, duração e texto, com casas decimais. **Máscara personalizada feita (commit c6113971)**: até 4 seções (positivo; negativo; zero; texto), cores `[Vermelho]`/`[Red]`/`[CorN]`, condições `[>1000]`, `*` (preenchimento), `_` (espaço), frações, científica e escala por vírgula; ida e volta com o `numFmt` do .xlsx. Falta: o preenchimento `*` é aproximado (a largura vem de fonte proporcional, não medida em pixels) e conferir casos raros (calendários não gregorianos, `[DBNum]`, `[$-locale]` completos).
- [~] Alinhamento: esquerda, centro, direita, cima, meio, embaixo, quebrar texto, rotação de -90° a 90°, recuo de 0 a 15 (commit c6113971, grava `indent` no .xlsx), mesclar e centralizar, mesclar através, mesclar e desmesclar. Falta: reduzir para caber, Preencher, Justificar, Centralizar na seleção, Distribuído, texto vertical empilhado e direção do texto.
- [x] AutoAjuste de altura e largura (duplo clique e menu) e tamanho exato pelo diálogo.
- [x] Pincel de formatação.
- [~] Estilos de célula: menu de estilos prontos e Novo estilo de célula (14/10). Falta: temas do documento (fontes e paleta de 12 cores) trocando tudo de uma vez.
- [ ] Formatação por caractere dentro da célula (rich text): negrito, cor, sobrescrito numa palavra só. Hoje sobrescrito/subscrito valem para a célula inteira.

## 3. Engenharia de dados, ETL e modelagem

- [~] Power Query → **OpenQuery**: editor em janela própria, passos guardados no `.fshz`, mesclar (6 modos), acrescentar, dinamizar, importar CSV/TSV/TXT/JSON/XLSX, prévia por etapa, perfil e qualidade de coluna. Falta: conectores de banco SQL, Web/API, PDF, Parquet e pasta inteira; correspondência difusa; coluna de exemplos; editor avançado em linguagem própria; parâmetros e funções. Fontes externas só por pedido explícito (privacidade).
- [ ] Power Pivot (modelo de dados, relacionamentos, DAX, hierarquias, KPIs, perspectivas). O armazenamento colunar do motor já é a base.
- [~] Tabelas dinâmicas: arrastar campos, filtros, agrupar datas, campo calculado, layouts compacto/estrutura/tabular, repetir rótulos, subtotais, recolher, dinâmicas recomendadas. **Motor pronto em 15/10/2026, commit 4b29bab1** (`dinamica/exibicao.rs`, `Extras`; falta ligar na GUI): Mostrar valores como (12 modos, igual ao Excel), agrupar número em faixas e data por ano/trimestre/mês/dia, classificar itens (A→Z, Z→A, por valor) e totais gerais de linhas e colunas separados; lê `showDataAs`/`fieldGroup`/`sortType`/`rowGrandTotals`/`colGrandTotals` do .xlsx (a gravação de `pivotTable` não existe). Falta: itens manuais, itens calculados, mostrar páginas de filtro, detalhar com duplo clique, item base nomeado vindo do .xlsx, GUI para tudo isso.
- [ ] Segmentação de dados (slicers): botão na faixa desabilitado, com a dica "Em breve" (15/10).
- [ ] Linha do tempo (timeline).
- [ ] Funções de cubo (OLAP).
- [~] Consultas e conexões: atualizar a consulta do OpenQuery. Falta: atualizar tudo, atualização automática a cada X minutos e ao abrir, Editar Links (vínculos com outras pastas existem no motor, `planilha/vinculo.rs`).

## 4. Análise, estatística e simulação

- [x] Atingir meta: janela não modal (`app/atingir_meta.rs`, 13/10), clique na grade preenche o campo, busca secante + bissecção no motor (`operacoes/atingir_meta.rs`, 100 iterações, tolerância 0,001), Manter (um Ctrl+Z) ou Desfazer.
- [ ] Solver.
- [x] Gerenciador de cenários: `app/cenarios_ui.rs` + motor `operacoes/cenarios.rs` (13/10). Lista e formulário, até 32 células variáveis, Mostrar (um Ctrl+Z), Resumo numa aba nova; guardado no `.fshz` (campo `ce` da aba).
- [~] Tabela de dados (1 e 2 variáveis): `app/tabela_dados.rs` + motor `operacoes/tabela_dados.rs` (13/10), resultados gravados como valores num Ctrl+Z e botão "Atualizar tabela" (só na sessão). Falta: ser fórmula viva `{=TABELA()}` que recalcula sozinha e fica no arquivo.
- [~] Planilha de previsão: as funções `PREVISAO`/`FORECAST` existem; falta a planilha com sazonalidade, limites de confiança e gráfico.
- [~] Ferramentas de análise (ToolPak): "Estatísticas" e "Perfil da coluna" dão estatística descritiva e histograma; as funções estatísticas cobrem testes t, F, correlação e regressão. Falta o assistente de cada ferramenta (ANOVA, regressão com relatório, amostragem, gerador de aleatórios, Fourier, média móvel).
- [ ] Analisar dados (IA). Planejado como MCP do FeatherSheet, local e por pedido.
- [x] Subtotal automático (`8d1ffeb2`): janela Subtotal (coluna de grupo, 11 funções, colunas), `=SUBTOTAL(n;…)` por grupo e Total Geral, níveis 1–3, substituir e Remover todos, num Ctrl+Z (`planilha/subtotais.rs`, 14/10). As linhas abrem pelo mover, então as fórmulas acompanham.
- [x] Agrupar/desagrupar e estrutura de tópicos (`8d1ffeb2`, níveis 1 a 8): Agrupar/Desagrupar (Alt+Shift+→/←), [+]/[−] e botões de nível na grade, Mostrar/Ocultar detalhe, grupo de dentro continua recolhido ao abrir o de fora; gravado no `.fshz` e no `.xlsx` (`outlineLevel`/`collapsed`) (`planilha/sheet/topicos.rs`, 14/10). Linha recolhida conta para o `SUBTOTAL` 1–11, como no Excel.

## 5. Motor de cálculo e endereçamento

- [~] Modos de cálculo: Automático e Manual em Fórmulas > Opções de cálculo, F9 calcula a pasta e Shift+F9 a planilha (`app/formulas_ui.rs:139-146`, `:420-421`; motor `planilha/calculo_manual.rs:31-68`); voltar ao automático calcula o que esperava. Falta o modo "automático exceto tabelas de dados". Feito em 14/10.
- [ ] Cálculo iterativo (referência circular intencional). Hoje a circular vira `#CIRCULAR!` com explicação.
- [x] Matrizes dinâmicas com derramamento (`#SPILL!`, FILTRO, ÚNICO, CLASSIFICAR, SEQUÊNCIA…).
- [x] Marcar valores desatualizados no modo manual: com conta esperando, o rodapé mostra "Calcular" / "Há fórmulas por calcular (F9)" (`app/formulas_ui.rs:586-593`, `Workbook::calculo_pendente_manual`). A célula em si não é marcada.
- [x] Cálculo em várias threads: escolhe de 1 até todos os núcleos menos um (Configurações → Desempenho; padrão todos menos um). Calcula em paralelo, coluna por coluna, na ordem das dependências (`planilha/calculo_paralelo.rs`, 13/10: Planilha Pesada de 25,9 s para 5,5 s).
- [~] Gerenciador de nomes: definir, listar, usar em fórmula e **criar a partir da seleção** (títulos das bordas viram nomes: `app/formulas_ui.rs:170-202`, regra em `planilha/sheet/nomes_selecao.rs`). Falta: escopo por aba (o `Nomeado` só tem nome e área, e a pasta guarda um mapa único: `planilha/workbook.rs:63`, `:426`) e aplicar nomes.
- [ ] Estilo de referência L1C1/R1C1 (a API de macros aceita `setFormulaR1C1`, a grade não).
- [ ] Referências 3D (`Jan:Dez!B5`).
- [ ] Precisão conforme exibido.
- [ ] Sistema de datas 1904 (o motor usa só a base 1900, sem o 29/02/1900 fictício).

## 6. Visualização e elementos gráficos

- [~] Formatação condicional: maior, menor, entre, igual, texto contém/começa/termina, datas, duplicados, exclusivos, acima/abaixo da média, fórmula, escala de cor (número, porcentagem, percentil), barra de dados, gerenciador de regras. Falta: conjuntos de ícones, 10 primeiros/últimos, "Parar se verdadeiro", barra com eixo para negativos e "mostrar só a barra".
- [~] Gráficos: colunas, barras, linhas, degraus, área (empilhados e 100%), pizza, rosca, dispersão, bolhas, radar, cascata, histograma, cotação (candlestick), mapa de calor, Gantt e combinação, com agregação sobre milhões de linhas. Falta: 3D, pizza de pizza, superfície, funil, treemap, sunburst, caixa e bigodes, Pareto e eixo secundário.
- [ ] Elementos analíticos de gráfico: linha de tendência com equação e R², barras de erro, linhas de projeção, máximo/mínimo, barras superiores/inferiores.
- [ ] Modelos de gráfico e aba só de gráfico.
- [ ] Gráfico dinâmico.
- [x] Minigráficos (sparklines): Linha, Coluna e Ganhos/Perdas, com aba contextual, `.fshz` e `.xlsx` (04/11).
- [ ] Mapas coropléticos e Mapas 3D.
- [ ] Ferramenta Câmera.
- [x] Imagem na célula e imagem flutuante, com `=IMAGEM`, base64 e efeitos de imagem. Remover plano de fundo falta.
- [~] Formas (`c5543c9f`) e ícones (`01c9ed76`) feitos como objetos da camada sobre a grade. Falta: SmartArt (botão "Em breve") e modelos 3D.
- [~] WordArt feito (`9970bdf2`: 4 estilos, deformação, sombra, texto editável). Falta a caixa de texto ligada a uma célula.
- [ ] Captura de tela.
- [x] Caixa de seleção na célula: Inserir > Checkbox põe caixas na seleção (VERDADEIRO/FALSO), clique e Espaço marcam, o botão aceso remove (`316961b9`, `92dcf633`, 15/10). Não é gravada no `.xlsx` (14/10/2026, `f9a9e485`).

## 7. Automação, programação e extensibilidade

- [ ] Gravador de macros (gravar ações gerando JavaScript, a fazer).
- [~] VBA e VBE → macros em **JavaScript** com editor Alt+F11 estilo VBE (módulos, Objeto/Procedimento, Propriedades, Janela de Inspeção, verificar sintaxe), funções definidas pelo usuário e eventos. Conversor VBA → JavaScript com interface: Macros → Importar VBA do Excel (.xlsm) (`c195e9fd`); as macros importadas entram sozinhas no editor; eventos do VBA (`Worksheet_Change`, `Worksheet_Activate`) rodam sozinhos (`6edc796e`); `Workbook_Open` continua à mão. Falta no editor: depurador (pausar, passo a passo, Locais, Pilha) e cerca de 110 de 128 chamadas da API na interface Iced.
- [~] UserForms → componentes HTML+CSS com designer.
- [x] Office Scripts → as próprias macros JavaScript rodam na máquina, sem nuvem.
- [ ] Python na planilha (`=PY()`): anotado, não iniciado; seria local, nunca em nuvem.
- [~] Controles na grade: botão de macro (arrastar, redimensionar, rodar). Falta: caixa de combinação, opção, lista, barra de rolagem, botão de número, caixa de grupo.
- [ ] Suplementos (add-ins).

## 8. Dados estruturados, XML, tempo real e hardware

- [ ] Mapeamento e importação/exportação de XML com esquema.
- [ ] Data Streamer (porta serial, Arduino).
- [ ] RTD e DDE (cotações em tempo real).
- [ ] OLE (objetos de outros programas embutidos).
- [ ] Editor de equações.

## 9. Auditoria, inspeção e governança

- [x] Rastrear precedentes e dependentes: um nível por clique, setas desenhadas na grade (`grade/setas.rs`), intervalo contornado, outra aba tracejada, erro em vermelho, Remover setas (`app/formulas_ui.rs:133-138`, `:335`; motor `planilha/sheet/auditoria.rs:22,42`). Feito em 14/10.
- [x] Avaliar fórmula passo a passo: janela com Avaliar/Entrar/Sair, SE/SEERRO só no ramo escolhido (`app/avaliar_ui.rs:28`; motor `formula/avaliar_passos.rs:73-171`). Feito em 14/10 (`a4a15506`).
- [x] Janela de inspeção: janela própria com as células observadas (`app/inspecao_ui.rs`, botão Inspeção da aba Fórmulas, `app/formulas_ui.rs:455`); a lista vale só para a sessão. O editor de macros tem a sua, que avalia expressões.
- [x] Mostrar fórmulas.
- [~] Verificação de erros: "Explicar este erro" no rodapé e no botão direito, referência circular explicada. Falta: triângulos de aviso em segundo plano (número como texto, fórmula inconsistente).
- [ ] Inquire (relatório da pasta, diagramas, comparar arquivos, limpar formatação em excesso).
- [~] Estatísticas da pasta: o F12 → Métricas mostra abas e células ocupadas; falta por aba (fórmulas, tabelas, gráficos) e última célula.
- [x] Verificar desempenho → formatação fantasma é colapsada ao abrir (`feathersheet-io/src/enxugar.rs`).
- [ ] Painel de seleção (camadas dos objetos).

## 10. Exibição, janelas, layout e impressão

- [x] Congelar painéis (linha, coluna ou interseção).
- [ ] Dividir a janela em 2 ou 4 painéis.
- [ ] Nova janela, organizar tudo, lado a lado com rolagem sincronizada.
- [~] Modos de exibição: Normal e Visualização de quebra de página. Falta: Layout da página com régua e cabeçalho editável na folha.
- [ ] Modos de exibição de planilha (visões por pessoa) e personalizados.
- [x] Foco na célula → "Pintar linha/coluna" da célula ativa (Exibir).
- [~] Zoom e controles da interface: zoom, linhas de grade, barra de fórmulas. Falta: zoom na seleção, cor da grade, ocultar títulos e guias.
- [~] Ocultar linhas, colunas e abas. Falta: ocultar janela e aba "muito oculta" (o leitor do XLSX conhece `veryHidden`).
- [~] Impressão: área de impressão, orientação, papel, ajustar a 1 página, linhas de grade, títulos A/B/C e 1/2/3, PDF e quebras de página. O motor já guarda margens, cabeçalho, rodapé e quebras manuais (`planilha/impressao.rs`); falta a interface. Falta também: imprimir títulos (linhas que repetem), ordem das páginas, preto e branco, rascunho, notas e troca de erros por branco na impressão, variáveis `&[Página]` no cabeçalho.

## 11. Colaboração, governança e segurança

- [ ] Coautoria em tempo real (nuvem).
- [~] Comentários com respostas e notas clássicas (15/10): anotações completas — novo, editar, excluir, anterior/próximo, mostrar/ocultar, mostrar todos, triângulo vermelho no canto, balão ao passar o mouse, botão direito, Limpar comentários e Limpar tudo; no motor com desfazer, acompanham inserir/excluir linhas e colunas e ordenar; gravadas no `.fshz` (`cm`) e no `.xlsx` (`commentsN.xml` + `vmlDrawingN.vml` + `legacyDrawing`). Do `.xlsx` lê as anotações clássicas e os comentários encadeados do Excel 365 (`threadedComments` + `persons`): autor da primeira mensagem e respostas juntadas no texto (`Nome: resposta`), sem o aviso de compatibilidade em inglês. Falta: responder/encadear na interface, gravar encadeado (sai como anotação), copiar/colar comentário, buscar em comentários, tamanho/posição do balão.
- [~] Histórico de versões: o formato `.fshz` prevê Git embutido (`FORMATO.md`); no FeatherSheet ainda não está ligado. Falta "Mostrar alterações".
- [ ] Controlar alterações (legado) e comparar/mesclar pastas.
- [~] Proteção (`28f11abe`): Proteger planilha com senha (hash SHA-512 do Excel) e as 15 permissões, célula bloqueada e fórmula oculta valendo, Bloquear célula no Formatar ▾; gravado no `.fshz` e no `.xlsx` (`sheetProtection`, `<protection>` do estilo) (14/10). A ação proibida é recusada ou desfeita na hora. Intervalos protegidos por senha só guardados no arquivo. Falta: permitir edição de intervalos, proteger estrutura, criptografia do arquivo com senha, assinatura digital, marcar como final.
- [ ] Inspetor de documento.

## 12. Desenho à tinta e dispositivos

- [~] Desenhar com caneta, borracha, laço, tinta para forma (aba Desenhar, `app/desenhar.rs`, 14/10). Falta: tinta para texto ou matemática, reprodução, mover a seleção do laço e trazer a tinta de volta do `.xlsx`.
- [ ] Modo de toque (faixa espaçada).

## 13. Acessibilidade, voz e idiomas

- [ ] Verificador de acessibilidade.
- [ ] Texto alternativo em imagens e gráficos.
- [ ] Falar células (leitura em voz alta).
- [ ] Tradutor.
- [ ] Ortografia, sinônimos e pesquisa inteligente.
- [ ] AutoCorreção.
- [ ] Planilha da direita para a esquerda e fonética asiática.

## 14. Arquivos, recuperação e integração

- [~] Formatos: abre e grava `.fshz` (próprio, Arrow) e `.xlsx`; abre `.csv`/`.tsv`; exporta PDF, CSV/TSV e JSON; lê o VBA de `.xlsm` para converter. Falta: gravar `.xlsm`, `.xlsb`, modelos `.xltx`, HTML, XML, `.ods`.
- [x] Salvamento automático e recuperação depois de travar (cópia de recuperação, intervalo escolhido, gravação atômica em segundo plano).
- [ ] Abrir e reparar arquivo corrompido.
- [~] Barra de status com soma, média, contagem, mínimo, máximo e desvio padrão da seleção, ignorando linhas ocultas. Falta: copiar o valor clicando nele.
- [~] Hiperlinks: inserir, abrir e remover link na célula. Falta a conferir: link para outra aba/célula/nome, `mailto:` com assunto.
- [ ] Mala direta.
- [ ] Publicar no Power BI / SharePoint (nuvem).

---

# Além do Excel

> Acrescentado em 13/10/2026, a pedido do usuário. A paridade acima mede só o que o Excel já
> tem. Esta parte registra o que o FeatherSheet tem, ou terá, **que o Excel não tem**, mais as
> otimizações já feitas e o caminho a seguir. **Pronto** = existe no código Rust/Iced;
> **parcial** = existe, mas falta parte; **a fazer** = decidido ou anotado, não começado.
> Números marcados **medido** saíram de execução com os arquivos do usuário.

## 1. OpenQuery — o Power Query da suíte

Passos de consulta guardados no próprio `.fshz` (`Sheet.oq`) como dados legíveis (JSON), motor
em Rust (`feathersheet-core/src/consulta/query.rs`), sem servidor e sem conta. Detalhe em
[`OPENQUERY-RUST-PARIDADE.md`](../funcionalidades/OPENQUERY-RUST-PARIDADE.md).

Pronto:

- Editor em janela própria no padrão do Alt+F11 (`openquery_ui/janela.rs`).
- Assistentes: mesclar consultas (6 modos, chaves compostas), acrescentar alinhando por nome de
  coluna, dinamizar com agregação.
- Importar CSV, TSV, TXT, JSON e XLSX numa tarefa separada da interface, com limites de tamanho;
  reimportar a origem preservando as etapas.
- Entrada manual (colar CSV/TSV), sem executar fórmula importada.
- Atualizar sem duplicar aba; recusa crescer por cima de dado digitado à mão.
- Vínculo com tabela (`Sheet.tb` com `openQueryId`), prévia por etapa, reordenar, excluir,
  exportar e importar os passos em JSON.
- Proteção contra seleção enorme: recusa antes de materializar (`query/limites.rs`).
- Executor em fluxo no motor (`consulta/query/fluxo.rs`): passos locais sem carregar todas as
  linhas.

Melhorias do OpenQuery (a fazer, nesta ordem):

1. Ligar o executor em fluxo à grade Iced: milhões de linhas com progresso e cancelamento.
2. Transformações globais em fluxo (juntar, ordenar, dinamizar, agrupar, remover repetidas).
3. Editor avançado: árvore de etapas com argumentos, filtros avançados, visualizador estilo M.
4. Edição tabular célula a célula na entrada manual.
5. Fórmula `=SQL(...)` sobre as abas e consultas (ideia de 24/09).
6. Fontes externas só por pedido explícito (regra de privacidade): Web/API com proteção contra
   SSRF, limites de download e tempo; SQL somente leitura.
7. A IA monta os passos pelo MCP do FeatherSheet (item 5 abaixo) e a pessoa vê e edita depois.

## 2. VBA em JavaScript

O Excel só roda VBA (e Office Scripts na nuvem). Aqui a macro é JavaScript com a API `sheet` em
inglês, roda na máquina, e quem vem do Excel converte o VBA.

Pronto:

- Editor de macros Alt+F11 estilo VBE: módulos, seletores Objeto/Procedimento, Propriedades,
  Janela de Inspeção, Verificar sintaxe.
- Componentes HTML+CSS no lugar do UserForm, com designer, `data-macro` e `data-fechar`.
- Botão de macro na grade (arrastar, redimensionar, rodar).
- **Conversor VBA → JavaScript, motor pronto** (`crates/feathersheet-vba`, commit `9dd4bbf`):
  lê o `vbaProject.bin` do `.xlsm` (OLE + compressão MS-OVBA em Rust puro) e os `.bas`/`.cls`/
  `.frm`; o que não converte vira comentário `// VBA não convertido:` com a lista do que ficou.
  Testado com um `.xlsm` real.
- **Interface do conversor** (`c195e9fd`): Macros → Importar VBA do Excel (.xlsm), a do arquivo
  aberto ou outro escolhido. As macros entram no editor sem rodar sozinhas na importação.
- **Eventos do VBA rodando sozinhos** (`6edc796e`): `Worksheet_Change` (aoMudar), depois de cada
  mudança feita pela pessoa, e `Worksheet_Activate` (aoAtivar), ao trocar de aba. `Workbook_Open`
  continua à mão, de propósito: rodar código ao abrir o arquivo é o caso mais arriscado.

A fazer:

1. Completar a API de macros na interface Iced (em 12/10 cerca de 17 de 128 chamadas
   funcionavam).
2. Gravar ações (`automate.recordActions`) gerando JavaScript, com loops de coluna vetorizados.
3. Macros executadas no Rust: loop de coluna vira operação vetorizada; o resto num motor embutido.
4. Depurador: pausar, passo a passo, pontos de parada, Locais e Pilha de chamadas.
5. UserForm `.frm` convertido direto para componente pelo desenho do designer.

## 3. Arquivo grande, memória e velocidade

O que já foi otimizado, com os números de antes e depois. A avaliação geral do desempenho
está em [`PARIDADE-AVALIACAO.md`](PARIDADE-AVALIACAO.md).

| Onde | Antes | Agora | Como | Tipo |
|---|---|---|---|---|
| Mesmo CSV aberto no FeatherSheet × Excel | Excel: ~4.500 MB de RAM | **~800 MB** | armazenamento colunar | **medido pelo usuário** (13/10) |
| CSV do Spotify aberto + Texto para Colunas, FeatherSheet × Excel | Excel: ~4.500 MB, lento, pesando o computador | **1.623 MB**, mais rápido e sem travar | colunar + corte em threads + gravação em lotes | **medido pelo usuário** (13/10) |
| Tamanho máximo da planilha | Excel: 1.048.576 linhas | **100.000.000 × 16.384** | eixos com padrão + exceções, nada alocado por linha | código |
| CSV de 817 MB (1,18 M × 83, 87,6 M células) | Excel corta em 1.048.576 linhas | 51,5 s, pico 1,1 GB | leitura e colagem em lotes | **medido** |
| CSV de 800 MB | estourava 8 GB | ~1,8 GB | lotes de 50 mil linhas; só suja célula que alguma fórmula lê | medido 23/09 |
| CSV de 200 MB (1,3 M × 20) | 21,7 s, 2,29 GB | 8,8 s, 454 MB | idem | medido 23/09 |
| Fórmula por célula | árvore + texto por célula | **12 bytes** por célula | coluna preenchida = 1 padrão; árvore solta e reparseada sob demanda | código |
| Grafo de dependências das colunas de fórmula | 309 MB de conjuntos | deduzido por conta | quem lê B7 é D7: uma subtração | código |
| Ler `.fshz` de 126 MB (Spotify) | 10,3 s | **1,5 s** | gzip lido em blocos de 1 MB | **medido** 13/10 |
| Abrir o mesmo `.fshz` | — | ~4,6 s, ~0,73 GB | Arrow colunar no ZIP | **medido** |
| Abrir arquivo de 2 M linhas | 7.736 MB | 3.111 MB | células fantasma colapsadas em estilo de coluna | medido 22/09 |
| Salvar 2 M linhas | ~3,6 GB, 1.987 ms comprimindo | 1,5 GB, 371 ms | compressão rápida, mesmo tamanho no disco | medido 22–23/09 |
| Salvar com a janela livre | janela parada | 0,4 s na janela, 8 s em segundo plano | gravação em segundo plano, atômica (`.parcial` + renomear) | **medido** 13/10 |
| Abrir XLSX com fórmulas encadeadas (Congo) | 120 s | **0,18 s**, 39 MB | cascata de sujeira visita cada fórmula uma vez (`f6a08ab0`) | **medido** 13/10 |
| Texto para Colunas, 87,6 M células | 108 s | **24 s**, pico 2,8 GB | corte em várias threads, gravação em lotes, `malloc_trim` no fim | **medido** 13/10 |
| Desfazer do Texto para Colunas | foto da área inteira | 6 s | foto só do que existia; histórico grande vai para o disco (`~/.cache`) | **medido** |
| Localizar em 2 M linhas | 4 s | 0,09 s (texto), 0,23 s (número) | cada texto distinto conferido uma vez; colunas em paralelo | medido 23/09 |
| Localizar no Spotify (1,18 M × 82) | 18 s | 0,64 s; "Próxima" em 0 ms | idem + última lista guardada por aba e versão | medido 23/09 |
| Região atual (Ctrl+T, Ctrl+A), 552 mil × 83 | — | 0,012 s | pula blocos vazios | **medido** 13/10 |
| Recálculo por tecla (pasta de exemplo) | 8,7 ms | 0,7 ms | preguiçoso, com poda; matriz só quando precisa | medido |
| Grade | — | lê só o visível (76 células por quadro numa aba de 400 mil) | virtualizada; seleção arrastando não percorre as 10 M linhas | medido 22/09 |
| Gráfico sobre 2 M linhas | 9 GB, travava | teto + aviso; agrega lendo tudo e acumulando | — | medido 22/09 |
| Tabela dinâmica, 500 mil linhas | 555 ms | 107 ms | agrupamento numa passada | medido 23/09 |
| Formatação condicional, 1,3 M linhas | — | 2,8 ms por quadro | estatística da regra em cache | medido 23/09 |
| Calcular a Planilha Pesada inteira (`.xlsx`, 7 M fórmulas) | 25,9 s, uma thread | **5,5 s**, 15 threads | cálculo por coluna na ordem das dependências, todos os núcleos menos um | **medido** 13/10 |
| Idem a partir do `.fshz` | 13,9 s | **2,1 s** | idem | **medido** 13/10 |
| RAM depois de calcular tudo (Planilha Pesada) | 1.754 MiB | **~364 MiB** | PROCX na coluna compacta (Codex) + alocador ajustado | **medido** 13/10 |
| Editar uma célula com 1 M de PROCX em outra aba | recalculava todos | só quem lê a aba e as colunas tocadas | invalidação seletiva | código 13/10 |

Também conta como diferencial: **métricas em tudo**. Todo comando deixa número no terminal
(`[motor]`, `[ação]`, `[quadro]`), e a abertura mostra "carregado em X s · N MB" no rodapé.

Otimizações que faltam (a fazer):

1. Gravar coluna inteira de uma vez, em paralelo (hoje ~190 ns por célula; 16,6 s do Texto para
   Colunas são gravação).
2. Montar a pasta depois de ler o `.fshz` (`para_workbook`, 3 s: o maior pedaço da abertura).
3. Colar CSV: 36 → 24 s no ramo `csv-rapido` (parado, sem testes); ler CSV em fluxo em vez do
   arquivo inteiro na memória.
4. 19 milhões de textos originais guardados no Spotify (~400 MB): decidir com o usuário se vale
   guardar a grafia original.
5. Modo "zero RAM" com `mmap`.

## 4. Outros diferenciais que já existem

- **Explicar erro de fórmula**: aviso no rodapé e "Explicar este erro" no botão direito.
- **Perfil de coluna** e **estatísticas da seleção** (histograma, nulos, valores distintos).
- **Monitor (F12)**: memória, CPU, console e métricas do motor numa janela própria, a mesma
  para toda a suíte (`feather_ui::monitor`).
- **Pintar linha/coluna** da célula ativa (Exibir), só visual, sem tocar no arquivo.
- **Funções que o Excel não tem**: `COUNTUNIQUE`, `JOIN`, `REGEXMATCH`, `IMAGE.BASE64`,
  `IMPORTDATA`, `IMPORTJSON`, `FETCHTEXT`.
- **Imagem dentro da célula** e conversão imagem ↔ base64.
- **Nome da coluna da tabela no lugar da letra** quando o cabeçalho sai da tela.
- **Formato aberto**: `.fshz` é ZIP com dados em Arrow (lido por pandas, polars, DuckDB) e Git
  embutido para o histórico ([`FORMATO.md`](../../../FORMATO.md)).
- **Privacidade**: sem conta, sem login, sem telemetria; nada sai da máquina sem pedido.
- **pt-BR de verdade**: funções pelo nome em português e em inglês, vírgula decimal.

## 5. Planejado (a fazer)

- **MCP do FeatherSheet**: feito em 14/10 (`crates/feathersheet-mcp`, base `d177e8f5`, formatar/estrutura/mesclar `1d8a43b0`): a IA recebe estrutura, amostra e estatística, nunca a planilha inteira; cada ferramenta é um Ctrl+Z. Pendências em `MCP-PENDENTE.md`.
- **CONTAR.COR / SOMAR.COR** e a mesma coisa na API de macros (o VBA não tem).
- **Histórico e ramificações** da planilha (Copy-on-Write sobre o Git do `.fshz`).
- **Modo de página finita para relatório**, com grade e guias de alinhamento.
- **Cartão HTML personalizável** na grade, ligado a macro.
- **Edição leve de imagem embutida** (recortar, girar, brilho) pelo crate comum da suíte.
- **Dados de gráfico e tabela do FeatherSlide** editados e calculados no FeatherSheet.
- **Interface em várias línguas** (JSON por língua).

## 6. Caminho a seguir

Ordem sugerida, juntando paridade e diferenciais. Cada passo termina com teste e print do app.

1. **Fechar o que está aberto**: queda ao salvar (falta a linha `panicked` do terminal); prints do
   painel novo da tabela dinâmica; linhas de filtro em cima da dinâmica gerada; os 24 testes da
   interface que falham (contagens antigas de funções, divisão, salvamento automático, eixos);
   commit do que está pronto.
2. **Página Inicial: 93 / 94 em 15/10** (sublinhado duplo, caneta de borda, recuo com atalho, Limpar tudo/hiperlinks, Reaplicar). Só Suplementos fica de fora, por escopo.
3. **Fórmulas**: completa em 14/10 (23 / 23).
4. **Funções, lote 1**: referência e matriz dinâmica (`OFFSET`, `INDIRECT`, `XMATCH`, `LOOKUP`,
   `VSTACK`, `HSTACK`, `TAKE`, `DROP`, `BYROW`…). Lista completa em
   [`FUNCOES-FALTANTES.md`](FUNCOES-FALTANTES.md).
5. **Dados**: Agrupar/Desagrupar e Subtotal (`8d1ffeb2`), Atingir meta e Tabela de dados já feitos; falta Consolidar.
6. **Revisão**: proteger planilha feita (`28f11abe`, com senha e as 15 permissões) e anotações (15/10, aba Revisão); falta responder a comentário encadeado.
7. **Diferenciais**: macros VBA (conversor já na interface; falta o depurador); OpenQuery em fluxo para milhões de linhas. MCP do FeatherSheet feito em 14/10 (`crates/feathersheet-mcp`); falta o que está em `MCP-PENDENTE.md`.
8. **Otimização**: gravar coluna inteira, `para_workbook`, colar CSV.

### Feito em 14 e 15/10/2026

- **Inserir**: Formas (`c5543c9f`), Ícones (`01c9ed76`), Caixa de seleção (`316961b9`, `92dcf633`) e WordArt (`9970bdf2`). Minigráficos, Segmentação de dados e Linha do tempo seguem sem motor: botões desabilitados com a dica "Em breve".
- **Dados**: Agrupar, Desagrupar e Subtotal (`8d1ffeb2`). **Revisão**: Proteger planilha (`28f11abe`) e comentários/anotações (15/10: aba Revisão, Inserir > Comentário, botão direito, Limpar comentários; `.fshz` e `.xlsx`).
- **Fórmulas**: aba completa (modos de cálculo, F9, rastreio, Avaliar fórmula, Janela de inspeção, Criar a partir da seleção).
- **MCP do FeatherSheet** (`crates/feathersheet-mcp`).

### Feito na rodada de 13/10/2026

- Grupo **Células**: botão **Formatar ▾** (Altura da linha…, AutoAjuste da altura, Largura da
  coluna…, AutoAjuste da largura, Ocultar/Reexibir linhas e colunas, Formatar células…) e o
  diálogo de tamanho (`app/dimensao_dialogo.rs`): vale para a seleção toda, um Ctrl+Z.
- **Mesclar ▾** dividido: Mesclar e centralizar, Mesclar através, Mesclar, Desmesclar
  (`app/mesclas/modos.rs`), cada um numa transação só.
- Ícones `line-height`, `arrow-autofit-width` e `arrow-autofit-height` no catálogo Tabler.
- Testes da interface voltaram a compilar (`grade/testes.rs`, `openquery_ui/janela.rs`).
- **Cálculo em várias threads** (`planilha/calculo_paralelo.rs`): ao abrir, a planilha é
  calculada em paralelo fora da janela, coluna por coluna na ordem das dependências; resultado
  idêntico ao cálculo sob demanda, conferido célula por célula.
- **Edição não recalcula tudo**: escrever numa aba só suja as fórmulas de outras abas que leem
  aquela aba e aquelas colunas.
- **Alocador ajustado** (`paralelismo::ajustar_alocador`): a memória que as threads liberam volta
  ao sistema.
- **Monitor F12 no `feather-ui` v0.2.2**: o aplicativo só manda mensagens e métricas.
- Paridade por funcionalidade (117 itens, 14 áreas) acrescentada a este documento.
