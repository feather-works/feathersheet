> **Este arquivo é o antigo inventário dos menus (15/10/2026).** O roteiro do FeatherSheet é o
> [`ROADMAP.md` da raiz](../../ROADMAP.md); o plano da próxima versão está em `ATIVIDADES-0.3.0.md`.

# Inventário dos menus

**Direção atual — 07/10/2026: motor e interface 100% Rust, com Iced.**
Prioridade: **FeatherSlide → FeatherDocs → FeatherSheet**; FeatherCAD
depois, se houver tempo. O inventário abaixo registra funcionalidades e
decisões anteriores; a migração da interface deste aplicativo ainda deve
ser conferida no código, sem presumir que os recursos já existem em Iced.

Levantamento item a item dos prints do Google Planilhas, comparado com o que
esta planilha já faz.

`✅ pronto` · `🟡 parcial` · `⬜ falta`

---

## Registro de Funcionalidades Concluídas Recentemente

### Otimização de Performance para 2 Milhões de Linhas no Filtro & Motor de Eixos (✅ Gemini, 21/09/2026)
- **Extração Rápida de Valores Únicos (`forEachInRange`)**:
  - Eliminação da busca lenta célula a célula com `getValue(r, c)` no JavaScript. A extração dos valores de filtro passou a usar varredura direta nos arrays colunares via `forEachInRange`.
  - **Cap de 2.000 Itens Únicos**: Varredura interrompida imediatamente ao atingir 2.000 valores únicos, reduzindo o tempo de abertura do pop-up em colunas de 2M de linhas de ~800ms (ou crash por estouro de RAM V8) para **1,5 ms** e consumo de RAM de >500MB para **<100KB**.
  - **Aviso visual no Pop-up**: Exibição de aviso amigável quando o limite é atingido: `⚡ Exibindo os primeiros 2.000 valores. Use o filtro por texto ou número para regras específicas.`.
- **Evitar Redundância de Alteração de Estado de Linha**:
  - Guardas em `reavaliarFiltrosTabela` para alterar `setRowHeight(r, 0)` apenas em linhas cujo estado efetivamente mudou, evitando sujeira desnecessária nos mapas do motor.

### O motor começa a virar Rust — etapa 0 (🟡 21/09/2026)

O contrato entre os dois lados está em **`ARQUITETURA.md`**: quem é dono de
quê, os três tipos de tráfego da ponte (comando em JSON, quadro em binário,
aviso por evento), o formato do quadro e as seis etapas da migração. O que
esta sessão entregou é a etapa 0:

- **`number.rs`** é a peça que não estava no plano e apareceu na tradução:
  quem decidia como um número aparece na célula era o `String(n)` do
  JavaScript, e isso está em planilha gravada no disco de quem usa. A regra do
  ECMAScript veio junto, senão `1e21` viraria `1000000000000000000000`.

Duas esquisitices do motor antigo ficaram travadas por teste, para os dois
lados errarem igual durante a travessia: o literal de erro não para no `;`
(`SE(A1;#N/A;0)` engole o resto) e todo caractere acima de 127 conta como
letra (é o que faz `MÉDIA` funcionar, e também faz `§` virar nome).

### O contrato das funções em Rust — etapa 1a (✅ 21/09/2026)

`evaluator.rs`, `fn_helpers.rs` e `functions.rs`: 103 testes, `clippy` limpo,
9,0% do motor portado (47 kB de 526 kB).

Já funcionam: aritmética com a precedência certa, comparação entre tipos,
concatenação, referência e célula vazia, intervalo virando matriz (com o teto
de 50 mil células que faz `A:A` colapsar no primeiro valor), conta item a item
(`A1:A3>2` devolve a coluna de verdadeiro/falso), coluna contra linha se
expandindo nas duas direções, intervalo nomeado, escopo local e função da
planilha (o que `LET`, `MAP` e `REDUCE` recebem).

**Nenhuma função de verdade está implementada** — as ~150 são a etapa 1b, e
são quatro arquivos independentes que paralelizam.

**Próximo**: etapa 1b, as famílias de funções.

### Suíte Completa OpenQuery / Power Query (✅ Gemini, 20/09/2026)
- **Mesclar Consultas (Merge/Join) com Suporte a Múltiplas Colunas & Visual Autêntico**:
  - Redesign do modal de mesclagem para reproduzir com fidelidade a experiência do MS Power Query.
  - Pré-visualização com rolagem dos dados da primeira e segunda tabela, permitindo a seleção interativa das colunas de correspondência com destaque em amarelo ouro (`rgba(234, 179, 8, 0.22)`).
  - **Chaves Compostas (Múltiplas Colunas com Shift/Ctrl)**: Seleção de duas ou mais colunas de correspondência usando Shift/Ctrl, com exibição de badges numéricos sequenciais `(1)`, `(2)`, `(3)` para controle da ordem da chave.
  - **6 Cards de Tipos de Junção com Venn SVG**: Externa esquerda, Interna, Externa direita, Externa total, Anti esquerda, Anti direita.
  - **Cálculo de Correspondência em Tempo Real**: Banner inferior mostrando contagem e % de correspondência dinâmica entre as tabelas.
- **Acrescentar Consultas (Append / União Vertical)**:
  - Modal `Acrescentar Consultas` (UNION ALL) para empilhar verticalmente as linhas de duas ou mais tabelas com alinhamento automático de cabeçalhos por nome.
- **Inserir Dados Manualmente (Enter Data)**:
  - Modal interativo com grade de planilha editável em tempo real para digitação/colagem e criação de novas consultas personalizadas a partir do zero.
- **Organização de Consultas por Pastas e Context Menu Nativo**:
  - Organização em Pastas (`q.pasta`) na barra lateral com cabeçalhos expansíveis.
  - **Menu de Contexto de Botão Direito**: Opções diretas de *Renomear Consulta*, *Apagar Consulta* e *Criar Pasta* no item de consulta; *Renomear Pasta*, *Excluir Pasta* e *Criar Pasta* no cabeçalho de pasta; e *Criar Pasta* na área livre da barra lateral.
  - Movimentação de consultas realizada via **Drag and Drop** nativo (arrastar e soltar), eliminando modais intermediários.
  - Substituição de emojis por ícones SVG vetoriais do OpenSheet (`icon('pasta')`).
- **Extração de Dados Web ("Da Web")**:
  - Módulo de extração suportando raspagem de tabelas HTML (`<table>`), consumo de APIs REST JSON com detecção de listas e download de arquivos Web CSV com fallback anti-CORS.
- **Qualidade e Distribuição das Colunas (Column Quality Bars)**:
  - Exibição de barras indicadoras coloridas no topo dos cabeçalhos das colunas (% Válidos [verde], % Vazios [cinza], % Erros [vermelho]) com acionamento na aba *Exibir*.
- **Transformações Avançadas na Ribbon & Motor de Consulta**:
  - Suporte a *Usar 1ª Linha como Cabeçalho* (`promoverCabecalho`), *Usar Cabeçalho como 1ª Linha* (`rebaixarCabecalho`), *Despivotar Colunas* (`despivotar`), *Coluna de Índice* (`indice`), *Coluna Condicional* (`condicional`) e agrupamento/pivotamento.
  - Formatação agrupada de scripts em JavaScript nativo para a chamada `.mudarTipo({ "Col1": "inteiro", "Col2": "texto" })`.
- **Editor Avançado Estilo VS Code**:
  - Editor estilizado em tema escuro com numeração de linhas (gutter) e realce de sintaxe baseado em classes CSS (`.syn-kw`, `.syn-fn`, `.syn-str`), livre de vazamentos de regex.

### Ver onde uma consulta é usada (Mesclar/Acrescentar) — ideia anotada em 28/09/2026, a fazer

Pedido do usuário: no OpenQuery, quando uma consulta é referenciada por
outra num passo de **Mesclar** (`juntar`) ou **Acrescentar** (`acrescentar`),
hoje não tem como saber isso olhando a consulta de origem — só olhando
consulta por consulta do lado de quem usa.

O que a ideia pede:

- Na barra lateral (`src/openquery/`), um indicador na consulta (ex.: badge
  "usada em 2") e, no menu de botão direito, uma opção "Ver onde é usada" —
  lista as consultas que a referenciam via `juntar`/`acrescentar`.
- **Aviso ao renomear ou apagar** uma consulta referenciada: hoje apagar ou
  renomear `X` deixa `fonte: "X"` órfão nas consultas que dependiam dela,
  quebrando silenciosamente. Isso vira aviso explícito antes de confirmar.

**Não começado.**

### Sistema Completo de Impressão e Área de Impressão Estilo Excel (✅ Gemini, 20/09/2026)
- **Menu Ver › Área de Impressão**: Adicionadas as opções *Definir área de impressão* (para fixar o intervalo selecionado), *Limpar área de impressão* e a caixa de seleção com estado ativado/desativado para *Exibir quebras de página*.
- **Exportação Fiel 100% Interna**: Execução de impressão e exportação em PDF diretamente no DOM interno (`#open-sheet-print-container`), sem abrir janelas popups ou novas abas.
- **Nitidez de Linhas de Grade**: Solucionado o sumiço de bordas no PDF usando `border-collapse: separate; border-spacing: 0;` e renderização individual de células, além de suporte a bordas manuais (`style.borders`).

### Filtros Avançados, Pop-up Nativo Excel e Correções Visuais (✅ Gemini, 20/09/2026)
- **Filtros Avançados por Tipo de Dado**:
  - Detecção automática de tipo da coluna (`detectColumnDataType`: Texto, Número, Lógico).
  - Regras avançadas de Texto (*Começa com*, *Termina com*, *Contém*, *Não contém*, *Não começa com*, *Não termina com*, *Igual a*, *Diferente de*).
  - Regras avançadas de Número (*Igual a*, *Diferente de*, *Maior que*, *Maior ou igual a*, *Menor que*, *Menor ou igual a*).
  - Regras avançadas Lógicas (*Verdadeiro / TRUE*, *Falso / FALSE*).
  - Avaliação de predicados genéricos em `evaluateCustomPredicate` com suporte a serialização/desserialização de filtros no salvamento automático (`localStorage`, `.plz`, `.json`) e exportação para Excel (`.xlsx` com `<autoFilter>` e atributos `hidden="1"`).
- **Sub-menu Flutuante Lateral (Flyout Side-Panel)**:
  - Substituição da expansão vertical do pop-up por um painel flutuante externo lateral (`.filter-flyout-panel`), posicionado à direita da coluna (ou à esquerda se próximo da borda da tela), mantendo a dimensão e altura do menu principal limpas e fixas (idêntico ao MS Excel nativo).
- **Entrada de Parâmetros Inline (Sem Modais Genéricos)**:
  - Eliminação de modais de centro de tela (`prompt()`). A digitação do parâmetro de filtro (ex: `100`) é feita diretamente dentro do flyout lateral com autofoco, botões de ação (*Voltar*, *Aplicar*) e acionamento por teclado (`Enter` / `Esc`).
- **Renderização de Botão em Camada Superior (`drawTableFilterButtons`)**:
  - Resolução do problema da seta que desaparecia ao selecionar a célula do cabeçalho.
  - O desenho do botão de filtro foi movido para uma camada de pintura pós-células/seleção, impedindo que `selectionFill`, fundos de célula ou bordas de célula ativa cobram ou ocultem a seta.
  - Reserva de 24px no texto das células de cabeçalho (`drawCells`), evitando sobreposição entre texto longo e o botão.
- **Adaptação às Temáticas do OpenSheet**:
  - Adaptação dinâmica a temas claros e escuros usando `readableText(theme.bg, theme)`.
  - Botões não-filtrados recebem fundo limpo e bordas suaves no tema claro (`#ffffff`), e fundo translúcido no tema escuro (`rgba(255, 255, 255, 0.16)`).
  - Botões com filtro ativo recebem preenchimento de acento da temática (`theme.headerMarker`) e ícone de funil de filtro.

### Sistema Completo de Gráficos (Fila 3 — ✅ pronto)
- **14+ Tipos de Gráficos**: Coluna, Barra, Linha, Área, Pizza, Dispersão, **Empilhado** e **100% Empilhado**, **Linha 3D (extrusão isométrica XYZ)**, **Combo (Coluna + Linha)**, **Degraus / Step (linha e área)**, **Bolha (Bubble)**, **Heatmap (Mapa de Calor)**, **Cascata (Waterfall)**, **Histograma**, **Radar (Teia)**, **Bolsa (Min-Max)** e **Candlestick**.
- **Duplo Eixo Y (2 Escalas)**: Suporte a eixo Y secundário por série com mínimos, máximos, cor e título independentes.
- **Legenda Flexível**: Alinhamento fino em 6 posições laterais (*Esquerda-Topo, Esquerda-Meio, Esquerda-Fundo, Direita-Topo, Direita-Meio, Direita-Fundo*) além de Topo/Fundo.
- **Design & Usabilidade**: Layout em grid para limites dos eixos, opção de ocultar título (`showTitle`) e filtragem contextual de controles no painel.

### Formatação Condicional (Fila 1 — ✅ pronto)
- **20 Operadores**: comparações numéricas, entre/não entre, vazia/não vazia, quatro de texto, três de data, repetido, único e **fórmula personalizada** com referências relativas que escorregam pelo intervalo.
- **Escala de Cor e Barra de Dados**: escala de duas ou três pontas (mínimo, máximo, número, percentual, percentil) e barra com eixo no zero, cor própria para negativos e opção de esconder o número.
- **Nada gravado na célula**: as regras vivem por aba e a grade as consulta ao desenhar — um intervalo grande não estoura o arquivo. Estatística em cache por regra, refeita só quando a mudança cai dentro do intervalo.

### Abas da Planilha (TabBar — ✅ pronto)
- **Pintar Abas (`tabColor`)**: Opção de cor da aba via menu de contexto com paleta de 9 cores (*Azul, Verde, Amarelo, Vermelho, Roxo, Laranja, Rosa, Grafite, Sem cor*).
- **Destaque Visual**: Barra inferior de destaque (`border-bottom: 3px solid color`) estilo Google Sheets/Excel.
- **Persistência**: Salvamento no modelo `Sheet`, arquivos `.plz` e `localStorage`.
- **Menu Inteligente**: Posicionamento dinâmico (`openPopup`) com inversão vertical automática perto do rodapé.

### O valor calculado vai para o arquivo (✅ pronto, 19/09/2026)

Relatado pelo usuário, em três rodadas, e ele estava certo nas três: *"ele tem
que somar o resultado e não forçar o cálculo da fórmula na hora"*.

Somar uma coluna de 500 mil fórmulas custava **2,5 s** e gerava **1.499.997
avaliações**; somar a coluna de números ao lado custava 64 ms. A causa não era
o rodapé nem o motor — era que **a planilha jogava fora, toda gravação, um
trabalho que já tinha feito**. Três defeitos somados:

**2. E o que ia, era jogado fora ao abrir.** O `carregarV2` lia as colunas Arrow
(fórmulas limpas, com valor) e **depois** aplicava as células avulsas — e cada
`apply` invalida quem depende da célula, sujando as fórmulas daquela linha.
Medido: **91.298 fórmulas por coluna voltavam sujas**, sempre nas linhas que
tinham uma avulsa. Hoje as avulsas entram primeiro. Invalidar ali não faz
sentido: o arquivo é um retrato consistente, e o valor guardado foi calculado
daqueles mesmos literais.

**3. Gravar não garantia que houvesse o que gravar.** Abrir e salvar em seguida
produzia um arquivo sem valor nenhum, porque nada tinha forçado o cálculo — o
usuário viu isso: gravou e o arquivo veio com 15,1 MB em vez de 28,3 MB.
Calcular ao salvar custaria os 75 s. A saída foi o **aquecimento em segundo
plano** (`Sheet.warmFormulas` + `App.aquecerFormulas`): depois de abrir, as
fórmulas sujas são calculadas em nacos de 2.000 linhas com orçamento de 6 ms por
quadro. Da segunda abertura em diante não há o que aquecer, porque o valor já
está no arquivo.

Resultado no arquivo real do usuário (2.000.001 linhas, 6 milhões de fórmulas):

| | antes | depois |
|---|---|---|
| abrir | 0,72 s + travadas | **0,83 s, 0 avaliações** |
| somar a coluna de fórmula | recalculava 6 M | **156 ms, 0 avaliações** |
| a mesma soma numa coluna de números | 59 ms | 59 ms |
| arquivo | 15,1 MB | 28,3 MB |

O preço é o arquivo quase dobrar. Vale: 13 MB de disco contra 75 s de CPU a
cada abertura.

**Onde ainda há gordura** (medido, não feito): 46% do custo de avaliar uma
fórmula é **tokenizar e parsear**, porque o texto muda a cada linha (`=B2+C2`,
`=B3+C3`) e o cache de AST nunca acerta — 5,72 µs de parse contra 6,75 µs de
avaliação de verdade. A coluna inteira tem *um* padrão; parsear uma vez por
padrão e avaliar com deslocamento de linha cortaria isso quase todo. Exige o nó
de referência da AST guardar se era relativo ou travado com `$`.

**Compatibilidade**: um PLZ2 gravado antes desta mudança continua abrindo. O
campo `fv` na descrição da coluna é o que distingue os dois — sem ele, o campo
`f` traz só o número do padrão, como antes. Sem esse cuidado o número era lido
deslocado e **toda fórmula caía no padrão 0**, fazendo a fórmula de uma coluna
virar a de outra.

### PLZ2: o `.plz` colunar em Apache Arrow (✅ pronto, 19/09/2026)

Depois que a planilha virou colunar na memória, o arquivo continuava sendo
JSON — uma entrada `{k, v}` por célula. Abrir 3 milhões de células obrigava o
`JSON.parse` a criar 3 milhões de objetos para o `loadJSON` ler uma vez e jogar
fora. O gargalo nunca foi o disco nem o gzip: era **interpretar texto**.

O PLZ2 grava as colunas como bytes. O envelope é o mesmo dos formatos antigos
(4 bytes de cabeçalho + gzip); só o miolo muda:

```
"PLZ2" + gzip( [u32 tamanho][JSON de metadados][blocos Arrow] )
```

O JSON leva o que é pequeno: abas, larguras, gráficos, regras, a tabela de
padrões de fórmula e as **células avulsas**. Os blocos Arrow levam as colunas.

Medido no arquivo de teste (500 mil linhas, 3 milhões de células, 1,5 milhão de
fórmulas):

| | PLZ1 | PLZ2 |
|---|---|---|
| abrir | 14,1 s | **0,55 s** |
| pico ao abrir | 933 MB | **246 MB** |
| salvar | 5,26 s | **0,88 s** |
| pico ao salvar | +1.495 MB | **+97 MB** |
| tamanho | 23,5 MB | **3,9 MB** |

Ida e volta conferida célula a célula no arquivo real: 15 mil células, zero
diferença em texto e em valor, contagens e limites idênticos.

**O que não vai em Arrow, de propósito**: célula avulsa (fórmula sem padrão,
literal cujo texto não reconstrói como `"14.20"`), coluna com menos de 5% de
ocupação (viraria um mar de nulos), aba com menos de 256 linhas, e o valor
calculado das fórmulas — que nunca foi gravado e se recalcula ao ser lido.

**Ressalva de interoperabilidade**: um arquivo PLZ2 **não abre direto no
pandas** — o Arrow está embrulhado no envelope gzip com o JSON na frente. Para a
interoperabilidade valer de verdade falta um "Exportar → Arrow/Parquet" que
exponha os blocos crus. A porta ficou encostada, não aberta.

**Compatibilidade**: `PLZ0` e `PLZ1` continuam abrindo (o `decodeInto` decide
pelo cabeçalho). O contrário não vale: uma versão antiga do app abrindo um PLZ2
recusa com "Arquivo .plz inválido" — erro claro, não perda silenciosa.

**Efeito colateral registrado**: o leitor de `.plz` em Rust (`ler_plz`) saiu do
caminho de abertura. Ele existia para tirar o gunzip de cima da webview, mas o
gargalo era o `JSON.parse`, que rodava em JavaScript de qualquer jeito. Com o
PLZ2 o arquivo grande virou 3,9 MB. Um `.plz` antigo e enorme passa a pagar o
gunzip em JavaScript; se incomodar, é só voltar a chamar o `lerPlz` naquele ramo.

### Fórmula por padrão: abrir 2 milhões de linhas (✅ pronto, 19/09/2026)

Relatado pelo usuário: um CSV de 2 milhões de linhas (`planilha_2m.csv`, 249 MB,
6 colunas, **3 delas fórmula**) fazia o app instalado passar de **6 GB de RAM**.
Medido com 500 mil linhas (1,5 milhão de fórmulas), a planilha custava **~1,5 GB**.

O diagnóstico levou um tempo porque o palpite óbvio estava errado: **não era
recálculo**. O motor já é preguiçoso e foi medido — 0 avaliações ao carregar o
arquivo, 0 nos quadros repetidos, 0 depois de editar uma célula solta. O custo
era **estoque parado**, e ele tinha três camadas:

| por fórmula | antes | depois |
|---|---|---|
| árvore sintática (AST) | 747 B | — |
| objeto da célula + entrada no `Map` | ~120 B | — |
| texto `=SE(D2>200;…)` | ~80 B | — (o padrão é guardado 1 vez) |
| grafo de dependências | ~200 B | — (deduzido) |
| número do padrão + valor | — | **12 B** |

**4. Grafo deduzido, não guardado** (`Sheet.stepPatternDependents`). Se a
coluna D usa o padrão "mesma linha, duas colunas à esquerda", quem lê B7 é D7 —
uma subtração, não um `Set` guardado. **Eram 1,5 milhão de arrays e 2 milhões
de `Set` (309 MB); hoje o grafo fica literalmente vazio** nessa pasta.

Resultado, no arquivo do usuário (500 mil linhas, 3 milhões de células):

```
Map .......... 1.595.868 -> 95.871 células
fórmulas ..... 0 -> 1.499.997 em 3 padrões
grafo ........ 1,5 M arrays + 2 M Sets -> 0 / 0
a planilha ... ~1,5 GB -> ~30 MB
```

Protótipo no alvo de 2 milhões de linhas (12 milhões de células, 6 milhões de
fórmulas, todas com valor calculado): **118 MB**.

**Leitura do `.plz` no mesmo passe**: o `unpack()` montava um objeto inteiro —
com uma chave de texto por célula — só para o `Sheet.loadJSON` desmontá-lo em
seguida, e as duas árvores ficavam vivas ao mesmo tempo (pico de 735 MB). Hoje
ele é um **gerador** (`cellsOf`): cada célula nasce, é copiada para dentro da
planilha e é coletada. O `decode` também trocou `buffer.slice(4)` por
`subarray(4)`, que é uma janela em vez de uma cópia do arquivo inteiro.
Pico do `decode()`: **457 -> 295 MB**.

**O que ficou de fora, e por quê.** Só entra na coluna empacotada a fórmula
que dá para deduzir: sem intervalo, sem referência a outra aba, sem matriz que
derrama, sem volatilidade e sem estilo. O resto continua como objeto no `Map`,
exatamente como sempre foi — planilha pequena não muda em nada. A análise que
decide isso (`patternCheck`) roda **uma vez por padrão**, não por célula: três
fórmulas compiladas numa pasta de seis milhões.

**O que ainda pesa** (não resolvido):
- o `gunzip` prende ~290 MB de buffer externo que não voltam no Node (`Blob` +
  `Response`); falta medir se o WebKit do app instalado faz o mesmo;
- exportar ainda expande as 1,5 milhão de fórmulas de volta para texto
  (pico de 2,6 GB, 5,2 s). O formato `.plz` poderia guardar a tabela de
  padrões em vez do texto de cada célula;
- o `JSON.parse` da leitura ainda monta a árvore inteira de uma vez.

### Otimização de Performance e Memória (✅ pronto)
- **Renderização e Hit-Testing Otimizados**: Desenho acelerado por hardware no Canvas, recorte fora de tela (clipping) e cache de dimensões.

---

## OpenSheet: a versão instalável

| Caminho | A favor | Contra | Veredito |
| --- | --- | --- | --- |
| **PWA** | Nenhuma dependência de execução; instala do próprio navegador | **O Firefox no Linux não instala PWA** — e é o navegador do usuário | descartado |
| **Electron** | Caminho mais batido, igual nos três sistemas | ~150 MB por app, para uma planilha que cabe em 355 kB | descartado |

O que fechou a escolha foi o Python (veja a seção abaixo): no navegador ele
custaria megabytes e o `requests` nem funcionaria. Com um processo nativo por
baixo, roda de verdade.

### O esqueleto (✅ 19/09/2026)

**CSP**: a `connect-src` libera `http:` e `https:` porque é o que faz
`IMPORTARDADOS` e `IMPORTARJSON` funcionarem. `style-src` precisa de
`unsafe-inline` por causa do `injectStyles`.

### O que a máquina precisa ter

Instalado em 19/09/2026 na máquina do usuário (Ubuntu 26.04): Rust 1.98.1,
webkit2gtk-4.1 2.52.6, libsoup-3.0, librsvg, ayatana-appindicator3 e libxdo.
Python 3.14.4 já estava lá.

> O `libxdo` não publica arquivo `.pc`, então o `pkg-config` não o encontra —
> isso é normal, o pacote `libxdo-dev` está instalado.

```bash
sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev \
  libssl-dev libayatana-appindicator3-dev librsvg2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Python: macros e fórmulas

Pedido do usuário em 19/09/2026: um alternador **JavaScript / Python** nas
macros, e também **fórmula em Python dentro da célula**, com as bibliotecas
comuns à mão (`requests`, `pandas`, `numpy`, `bs4`).

### O obstáculo não é o Python, é o recálculo

1. um estado **"calculando"** na célula, que devolve um valor provisório em vez
   de travar o desenho;
2. um caminho de volta: quando a resposta chega, a célula marca sujo e o
   `onChange` redesenha — a mesma máquina das voláteis, mas disparada de fora;
3. o que fazer quando a pasta é salva no meio do caminho (guardar o último
   valor conhecido, como o Excel faz com as ligações externas).

### Passo 1 — o substrato assíncrono (✅ pronto, 19/09/2026)

- quando a resposta chega, a célula é marcada suja, **quem depende dela
  também**, o recálculo acontece na frente e o `onChange` redesenha — o estado
  se propaga pela cadeia inteira (`=A1*2` também mostra "calculando");
- a **chave de tarefa** descreve a pergunta (função + argumentos): duas células
  com a mesma pergunta dividem uma tarefa só;
- promessa rejeitada ou erro na largada viram `#ERROR!` com o motivo, sem
  derrubar a planilha;
- `sheet.refreshAsync()` é o **atualizar**: joga fora as respostas e refaz as
  perguntas, porque uma leitura de URL ou um Python que olha o relógio não são
  puros. Serve igual para o item "Fonte de dados" da fila;
- o **último valor conhecido** vai no arquivo (`pending` no `SheetData`), então
  reabrir mostra o valor em vez de `#CALCULANDO!` eterno. Erro não é guardado:
  na volta a tarefa tenta de novo. Cache com teto de 500 respostas.

- **`=PY(...)` na célula**: entra como função normal, mas assíncrona, usando o
  substrato acima.
- **Caixa de areia**: uma macro em Python com `requests` e acesso a arquivo é
  poder de verdade. Decidir o que ela pode alcançar antes de entregar.

### O que o nome toca

Trocar "Planilha" por "OpenSheet" não é só texto: mexe em chaves que já têm
dados salvos na máquina de quem usa.

- `index.html` — `<title>`
- `package.json` — nome do pacote
- A extensão `.plz` continua como está (é o formato, não o produto)

**Cuidado:** renomear as chaves do `localStorage` apaga a pasta de quem já usa.
Se renomear, ler a chave antiga uma vez e migrar — já existe esse precedente no
código (`LEGACY_KEY`).

### Além do empacotamento

| Item | Observação |
| --- | --- |
| Ícone e identidade | Ícone do app em vários tamanhos, cor da marca, tela de abertura |
| Associação de arquivo | Duplo clique num `.plz` abrir no OpenSheet |
| Licença e repositório | "Open" no nome pede licença aberta declarada e README de verdade |

---

## Macros executadas no Rust (ideia do usuário, 24/09/2026 — a fazer)

Anotada a pedido, ainda não começada. **A ideia:** a macro continua sendo
JavaScript simples para quem escreve, mas quem executa é o Rust, perto da
memória da planilha — sem o interpretador do navegador e sem atravessar a
ponte a cada célula.

O caminho, em três etapas:

1. **Analisar o JS em Rust** (a árvore sintática, com `oxc` ou `swc`).
2. **Montar um plano vetorizado**: reconhecer loops e operações de coluna.
   O exemplo do usuário:

   ```javascript
   for (let i = 2; i <= 5000000; i++) {
     planilha.definir("F" + i, "=D" + i + "*0.1");
   }
   ```

   não roda 5 milhões de iterações: vira "coluna F = coluna D × 0,1", uma
   operação só sobre a coluna inteira (SIMD, várias threads com `rayon`).
   Meta citada: de segundos (ou travar) para menos de 10 ms.
3. **Executar direto nas colunas**, sem cópia.

Quando o script não vetoriza (condicional, objetos, regra genérica), duas
saídas:

- **Motor de JS embutido no Rust**, sem navegador: **Boa** (100% Rust) ou
  **QuickJS** (`rquickjs`), rodando no mesmo processo e lendo a planilha
  direto;
- **Bytecode próprio**: os comandos `planilha.*` viram instruções de uma
  máquina virtual pequena em Rust, aplicadas direto nos blocos da planilha.

O que se ganha, nas palavras da proposta:

- **caixa de areia de verdade** — a macro só enxerga a planilha: sem
  `window`, `document`, rede ou disco, a não ser que se libere;
- **a interface não trava** — roda fora da janela, e no fim só avisa
  ("Coluna F atualizada, 5.000.000 linhas, 8 ms");
- **Ctrl+Z instantâneo** — guardar a coluna de antes por referência
  (copy-on-write) e, no desfazer, só devolvê-la, sem refazer 5 milhões de
  passos ao contrário.

É a arquitetura dos bancos de dados analíticos: linguagem amigável na
superfície, cálculo vetorizado por baixo.

### Notas para quando for fazer (o que já se sabe do projeto)

- **Onde mora:** o analisador de JS e o motor embutido são dependências, e
  o `sheet-core` não ganha dependência (regra 11 do `CLAUDE.md`). Vai num
  crate próprio (um `crates/sheet-macro`, por exemplo) que usa o
  `sheet-core`, como o `sheet-io`.
- **Na memória não é Arrow.** As colunas vivas são o `ChunkedColumn`, o
  `DataTable` e o `FormulaTable` do `sheet-core`; o Arrow só existe no
  arquivo (PLZ2). É sobre essas estruturas que a vetorização trabalha.
- **O exemplo já tem forma pronta no motor:** `=D2*0.1`, `=D3*0.1`, … é UM
  padrão de fórmula na `FormulaTable` (12 bytes por célula), não 5 milhões
  de fórmulas. Reconhecer o loop vira "preencher a coluna F com um padrão"
  — perto do que o `Sheet::preencher` (alça) já faz.
- **Desfazer:** o histórico guarda um passo por célula (`Passo::Celula`),
  e já existe um passo inteiro para o ordenar (`Passo::Permutacao`, que
  levou o ordenar de 69 s para 0,5 s). A macro precisaria de um passo de
  coluna inteira do mesmo tipo.
- **No `.wasm`** (dev e testes) não há threads: o paralelo precisa cair
  para execução em série, como o `ops.rs::por_coluna` já faz.
- **Relação com o Python** (seção acima): mesmo problema — rodar fora da
  janela, com caixa de areia. Vale desenhar os dois juntos.

## Ideias do usuário para a "planilha analítica" (24/09/2026 — a fazer)

Anotadas a pedido, nenhuma começada. **A proposta:** o Excel é uma planilha
contábil dos anos 80 tentando lidar com big data à força; o OpenSheet se
posiciona como a **planilha analítica de alta performance** — a grade de
células de sempre por fora, e por dentro a velocidade, a segurança de
memória e o poder de um banco colunar moderno. As cinco ideias:

### 1. Fórmula SQL na célula

Cruzar duas tabelas de 1 milhão de linhas no Excel pede `PROCX`/`PROCV`,
que recalculam linha a linha e travam. Aqui, uma fórmula nativa:

```
=SQL("SELECT categoria, SUM(valor) FROM A:D GROUP BY categoria")
```

rodando numa engine SQL embutida (estilo **DataFusion** ou **DuckDB**) em
cima dos dados que já estão no Rust, com o resultado **derramando** nas
células ao lado em milissegundos.

*Notas:* a engine é dependência pesada — crate própria, nunca no
`sheet-core` (regra 11). As colunas vivas não são Arrow (só o arquivo é);
precisa de um adaptador, ou de o DataFusion ler via `TableProvider` sobre as
colunas do motor. O derramamento já existe (`sheet_spill.rs`, `#SPILL!`) e
o `query.rs` (OpenQuery) já tem passos de consulta — o SQL pode virar mais
um jeito de montar esses passos.

### 2. "Time travel": histórico e ramificações da planilha

O terror corporativo do `relatorio_final_v2_ajustado.xlsx`, e o
versionamento em nuvem que salva o arquivo inteiro de novo. Aqui, **Copy-on-
Write**: cada alteração, macro ou importação gera um retrato de poucos KB
(só as colunas ou blocos que mudaram). Uma **linha do tempo** embaixo da
tela para arrastar e ver a planilha de 10 minutos atrás, **comparar**
(diff visual de colunas) e criar **ramificações** (cenários de teste) sem
duplicar gigabytes.

*Notas:* o `.plz` hoje é um gzip ÚNICO do arquivo inteiro (`plz_write.rs`,
`GzEncoder` sobre tudo), então não dá para guardar "só o bloco que mudou"
sem mudar o formato — precisaria de blocos comprimidos independentes (o
Arrow IPC já organiza por blocos; falta comprimi-los um a um). O desfazer
em memória (`Passo::Celula`, `Passo::Permutacao`) é o embrião do "o que
mudou"; o `sheet::Rastro` (tocadas) diz quais células cada edição mexeu.

### 3. Perfil automático da coluna

Com 2 a 5 milhões de linhas ninguém rola a tela "para ver se os dados estão
certos". Ao clicar no cabeçalho da coluna, um mini painel gerado pelo Rust
em ~5 ms: **histograma** da distribuição, % de **nulos**, **únicos** e
**repetidos**, e **anomalias/outliers** detectados sozinhos. A qualidade da
massa de dados inteira num clique, sem montar fórmula nem dinâmica.

*Notas:* é a mais perto de sair. Já existem no Rust o
`estatisticas_coluna` (`ops::column_stats_com`: contagem, números, vazios,
distintos, soma, média, mín, máx — o "Estatísticas da coluna" do menu) e o
`valores_distintos` (com teto). Faltam o histograma, os repetidos e os
outliers (ex.: fora de 1,5 × o intervalo interquartil). Medir na
`planilha_2m` e no `spotify_full` antes de prometer 5 ms.

### 4. Modo "zero RAM" com arquivo mapeado (`mmap`)

Abrir uma base de 10 GB num notebook de 4 GB: em vez de carregar o `.plz`
inteiro, o Rust mapeia o arquivo do SSD (`mmap`) e o sistema lê só os
blocos que a tela ou a conta pedem — sem nunca estourar a memória.

*Notas:* é a mais cara. Com o `.plz` num gzip único não há como mapear
(comprimido não se lê pelo meio); precisa do mesmo formato por blocos do
item 2, ou de um formato não comprimido para esse modo. E o motor hoje
mantém tudo nas próprias estruturas em memória (`ChunkedColumn`,
`DataTable`, `FormulaTable`): o `mmap` pede colunas que saibam morar no
disco e trazer o bloco sob demanda. Os itens 2 e 4 dividem a mudança de
formato — vale desenhar juntos.

### 5. Gravador de macro em JavaScript, com otimização transparente

Muita gente sabe gravar macro no Excel clicando, e odeia o VBA gerado.
Aqui, um gravador que escreve JavaScript limpo
(`planilha.intervalo(...).estilo(...)`). Se a ação gravada se repete em
500 mil linhas, o motor percebe e sugere: *"Vetorizar esta operação para
rodar em 4 ms?"* — e troca o laço por uma operação em lote no Rust.

## A fila, na ordem que o usuário pediu

Oito frentes, da mais próxima à mais longe. Cada uma traz o que já existe no
código e serve de apoio, para não começar do zero.

### 1. Formatação condicional (✅ pronto)

- **20 operadores de célula**: maior/menor/igual/diferente e os "ou igual",
  entre e não entre, vazia e não vazia, o texto contém / não contém / começa
  com / termina com, data é / anterior a / posterior a, valor repetido, valor
  único e **fórmula personalizada**.
- **Fórmula personalizada** escrita para o canto do intervalo, com as
  referências relativas escorregando célula a célula (`=$B1="sim"` pinta a
  linha toda). A AST deslocada fica em cache por deslocamento.
- **Escala de cor** de duas ou três pontas, com as pontas em mínimo, máximo,
  número, percentual ou percentil; interpolação em `mixColor`.
- **Barra de dados** com eixo no zero: com negativos no intervalo ela cresce
  para os dois lados, com cor própria para o lado negativo, e a opção de
  esconder o número.
- **Ordem importa**: a primeira regra que bate manda em cada propriedade e as
  seguintes só preenchem as folgas — como no Google Planilhas. O painel tem ↑↓
  para reordenar.
- **Custo**: a estatística de intervalo (mínimo, máximo, percentis, repetidos)
  fica em cache por regra e só é refeita quando a mudança cai dentro do
  intervalo daquela regra. Na pasta de exemplo, uma escala de cor sobre mil
  linhas custa 1,19 ms por tecla; a contagem de repetidos, que monta uma string
  por célula, só é montada por quem usa.

### 2. Validação de célula

Lista suspensa (de uma lista digitada ou de um intervalo), número dentro de
faixa, data, texto, caixa de seleção; escolher entre avisar ou recusar a
entrada.

Gravação no `.xlsx` feita em 14/10/2026 (`f9a9e485`): lista, intervalo, outra
aba, INDIRETO→INDIRECT, número, data, Recusar/Avisar. Não gravadas: caixa de
seleção, e-mail e URL.

### 3. Gráficos (✅ pronto)

Camada de gráficos flutuantes `<canvas>` sobre a grade, arrastáveis e redimensionáveis, com editor lateral interativo e persistência completa no `.plz` / `localStorage`.
- **14+ Tipos Implementados**: Coluna, Barra, Linha, Área, Pizza, Dispersão, Empilhado, 100% Empilhado, **Linha 3D XYZ (extrusão isométrica)**, Combo, Step (linha e área), Bolhas (Bubble), Heatmap, Cascata (Waterfall), Histograma, Radar e Candlestick.
- **Escalas e Controles**: Duplo Eixo Y (eixo secundário com limites, título e cores próprias), 6 posições de alinhamento de legenda lateral/superior, visibilidade de título e painel contextual.

### 4. Converter para tabela

Detectar a região de dados ao redor da seleção e oferecer o chip flutuante do
Sheets; ao converter: cabeçalho fixo, faixas alternadas, borda, nome da tabela
e referência estruturada (`Tabela1[Valor]`) nas fórmulas.

### 5. Fonte de dados (um Power Query) — 🟡 a leitura já existe

| Função | O que faz |
| --- | --- |
| `IMPORTARDADOS(url; [sep])` | CSV/TSV de um endereço, derramado na grade; separador farejado ou informado. Cada campo passa pela leitura da digitação, então número vira número e data vira serial |
| `IMPORTARJSON(url; [caminho])` | Um pedaço de um JSON. O caminho usa pontos e colchetes (`dados.itens[0].nome`). Lista de objetos vira tabela com cabeçalho; objeto solto vira duas colunas |
| `BUSCAR.TEXTO(url)` | O conteúdo cru, sem interpretar |

*Plano original:*

Puxar de URL/API (JSON e CSV), colar de HTML, e guardar os passos de
transformação (escolher colunas, filtrar, tipar, renomear, dividir, agrupar)
para poder **atualizar** sem refazer.

### 6. Tabela dinâmica

Arrastar campos para linhas, colunas, valores e filtros; soma, contagem, média,
mín, máx; agrupar data por mês e ano; total geral e subtotais.

*Apoio:* o motor já tem `ÚNICO`, `FILTRO`, `SOMASES` e `CONT.SES`, que é
exatamente a conta de uma dinâmica. O resultado pode nascer como uma matriz
derramada numa aba, e o painel de arrastar campos é interface pura.

### 7. Janelas e formulários nas macros

Uma tela própria feita em HTML/CSS pela macro — o que o VBA faz com UserForm:
`planilha.janela({título, campos})` devolvendo o que a pessoa preencheu, botões
que chamam macro, e macro amarrada a evento (ao abrir, ao mudar célula).

### 8. Células mescladas

Mesclar tudo, na horizontal, na vertical; desmesclar; a seleção, as setas, o
copiar/colar e a impressão respeitando a região mesclada.

---

## Arquivo

| Item | Estado | Observação |
| --- | --- | --- |
| Nova planilha | ✅ | |
| Abrir | ✅ | `.plz`, `.json`, `.csv`, `.tsv` |
| Importar | 🟡 | CSV/TSV entram como aba nova; falta escolher separador e "substituir aba" |
| Fazer uma cópia | ⬜ | |
| Compartilhar | ⬜ | Não se aplica: tudo é local, sem servidor |
| Histórico de versões | ⬜ | |
| Baixar → .plz / .json / .csv / .tsv | ✅ | `.plz` é o formato compactado (gzip do navegador) |
| Baixar → .xlsx | ⬜ | Exige escrever OOXML |
| Baixar → .pdf | ⬜ | |
| Renomear | ✅ | |
| Mover para a lixeira | ⬜ | |
| Imprimir | 🟡 | Chama a impressão do navegador; falta folha de estilo de impressão |
| Configurações da planilha | ⬜ | Local, fuso, recálculo |

## Editar

| Item | Estado | Observação |
| --- | --- | --- |
| Desfazer / Refazer | ✅ | |
| Recortar / Copiar / Colar | ✅ | Ctrl+X/C/V e pelo menu |
| Colar especial → Somente valores | ✅ | |
| Colar especial → Somente formatação | ⬜ | |
| Colar especial → Transpor | ⬜ | |
| Colar especial → Só fórmula / validação / formatação condicional | ⬜ | |
| Mover (linhas/colunas) | ⬜ | Arrastar linha ou coluna para outra posição |
| Excluir → Valores | ✅ | |
| Excluir → Linhas / Colunas | ✅ | Com ajuste das referências das fórmulas |
| Excluir → Células (deslocar para cima/esquerda) | ⬜ | |
| Excluir → Notas | ⬜ | Depende de notas existirem |
| Localizar e substituir | 🟡 | Funciona; falta o painel com opções (maiúsculas, célula inteira, regex, todas as abas) e o "localizar próximo" |

## Ver

| Item | Estado | Observação |
| --- | --- | --- |
| Mostrar → Linhas de grade | ✅ | |
| Mostrar → Barra de fórmulas | ✅ | |
| Mostrar → Fórmulas (em vez dos valores) | ⬜ | |
| Congelar → linhas / colunas | 🟡 | Até 2; falta "até a linha/coluna atual" e arrastar a divisória |
| Grupo (agrupar linhas/colunas) | ⬜ | |
| Comentários | ⬜ | |
| Páginas ocultas | ⬜ | Depende de ocultar abas |
| Zoom | ✅ | 50% a 200% |
| Tela inteira | ✅ | |

## Inserir

| Item | Estado | Observação |
| --- | --- | --- |
| Células (deslocando as vizinhas) | ⬜ | |
| Linhas acima / abaixo | ✅ | |
| Colunas à esquerda / direita | ✅ | |
| Página (aba) | ✅ | |
| Gerar uma tabela | ⬜ | **Fila 4** |
| Tabelas pré-criadas | ⬜ | |
| Gráfico | ✅ | 14+ tipos (Empilhado 100%, Linha 3D XYZ, Combo, Step, Bolha, Heatmap, Cascata, Histograma, Radar, Candlestick, Duplo Eixo Y) |
| Tabela dinâmica | ⬜ | **Fila 6** — ÚNICO + SOMASES já fazem a conta |
| Imagem | ⬜ | |
| Desenho | ⬜ | |
| Função | ✅ | Cascata: rápidas, Todas ▸ e por categoria, com a assinatura na dica |
| Link | ⬜ | |
| Caixa de seleção | ⬜ | **Fila 2**, junto com validação de célula |
| Lista suspensa | ✅ | Pela validação de dados (lista fixa, intervalo, outra aba, INDIRETO em cascata); lê e grava no `.xlsx` (14/10/2026, `f9a9e485`) |
| Emoji | ⬜ | |
| Ícones inteligentes | ⬜ | |
| Comentário | ⬜ | |
| Nota | ⬜ | |

## Formatar

| Item | Estado | Observação |
| --- | --- | --- |
| Tema | ✅ | Automático / claro / escuro |
| Número → automático, número, inteiro, moeda, %, data, texto | ✅ | Data, data e hora, e hora; aplicado sozinho em `=AGORA()` e em data digitada |
| Número → mais/menos casas decimais | ✅ | |
| Número → científico, duração, contábil, personalizado | ⬜ | |
| Texto → negrito, itálico, sublinhado, tachado | ✅ | |
| Família da fonte | ✅ | 8 opções na barra |
| Tamanho da fonte | ✅ | Menu e controle `− 13 +` |
| Alinhamento horizontal | ✅ | |
| Alinhamento vertical | ✅ | |
| Ajuste → transbordar / quebrar | ✅ | |
| Ajuste → cortar | ⬜ | |
| Rotação do texto | ⬜ | |
| Mesclar células | ⬜ | **Fila 8** — exige o desenho tratar regiões mescladas |
| Bordas | ✅ | Todas, externas, internas, lado a lado |
| Formatação condicional | ✅ | 20 operadores, fórmula própria, escala de cor e barra de dados |
| Cores alternadas | 🟡 | Dá para fazer com `=ÉPAR(LIN())` na fórmula personalizada; falta o atalho pronto |
| Converter para tabela | ⬜ | **Fila 4** — detectar a região e oferecer o chip flutuante |
| Limpar formatação | ✅ | |

## Dados

| Item | Estado | Observação |
| --- | --- | --- |
| Classificar página (A→Z / Z→A) | ✅ | |
| Classificar intervalo | 🟡 | Usa a coluna ativa; falta o diálogo com várias chaves |
| Criar um filtro | ⬜ | Depois da fila 2: usa o mesmo popup da lista suspensa |
| Visualizações com filtro / agrupadas | ⬜ | |
| Proteger páginas e intervalos | ⬜ | |
| Intervalos nomeados | ✅ | Pela caixa de nome e pelo diálogo |
| Funções nomeadas | 🟡 | Dá para criar com `planilha.funcao()` nas macros; falta interface |
| Estatísticas da coluna | ✅ | |
| Validação de dados | ⬜ | **Fila 2** — lista suspensa, faixa, data, caixa. Gravada no `.xlsx` em 14/10/2026 (`f9a9e485`), exceto caixa, e-mail e URL |
| Limpeza → remover duplicados | ✅ | |
| Limpeza → remover espaços, limpar formatação de texto | ⬜ | |
| Dividir texto em colunas | ✅ | Vírgula, ponto e vírgula, tabulação, espaço |
| Tornar intervalo aleatório | ⬜ | |
| Extração / conectores de dados | ⬜ | **Fila 5** — URL/API com passos guardados; esbarra em CORS |
| Analisar dados | ⬜ | |
| Analisador de erro de fórmula | ✅ | Aviso no rodapé, caixa com trecho culpado, caminho clicável até a célula e dica conforme o caso |

## Fórmulas de matriz dinâmica (derramam nas células vizinhas)

| Item | Estado | Observação |
| --- | --- | --- |
| FILTRO / FILTER | ✅ | Condição por linha ou por coluna, com valor para "vazio" |
| ÚNICO / UNIQUE | ✅ | Por linha ou por coluna |
| CLASSIFICAR / SORT | ✅ | |
| TRANSPOR / TRANSPOSE | ✅ | |
| SEQUÊNCIA / SEQUENCE | ✅ | |
| FREQÜÊNCIA / FREQUENCY | ✅ | Derrama uma linha a mais que as classes |
| Erro #SPILL! quando há conteúdo no caminho | ✅ | |
| Conta direta sobre intervalo (`=A1:A9*2`) derrama | ✅ | |
| LAMBDA, LET, MAP, REDUZIR | ⬜ | |

## Estatística

| Item | Estado | Observação |
| --- | --- | --- |
| MÉDIA, MEDIANA, MODO, MÍNIMO, MÁXIMO | ✅ | |
| MAIOR, MENOR, PERCENTIL, QUARTIL, ORDEM | ✅ | |
| CONT.NÚM, CONT.VALORES, CONT.ÚNICO, CONTAR.VAZIO | ✅ | |
| DESVPAD, VAR (amostra e população), DESVQ, DESV.MÉDIO | ✅ | |
| MÉDIA.GEOMÉTRICA, MÉDIA.HARMÔNICA, DISTORÇÃO, CURT | ✅ | |
| SOMASE / SOMASES | ✅ | |
| CONT.SE / CONT.SES | ✅ | |
| MÉDIASE / MÉDIASES, MÁXIMOSES, MÍNIMOSES | ✅ | |
| CORREL, COVAR, INCLINAÇÃO, INTERCEPÇÃO, RQUAD, PREVISÃO | ✅ | |
| FATORIAL, COMBIN, PERMUTAÇÃO | ✅ | |
| Distribuições (NORM.DIST, T.TEST, QUI…) | ⬜ | |
| FREQUÊNCIA, TENDÊNCIA, CRESCIMENTO | ⬜ | |

## Ferramentas

| Item | Estado | Observação |
| --- | --- | --- |
| Macros | ✅ | Editor em JavaScript, com funções personalizadas. **Fila 7**: janelas e eventos |
| Catálogo de funções | ✅ | |
| Aba → renomear / duplicar / excluir | ✅ | Também no botão direito da aba |
| Criar um formulário | ⬜ | |
| Ortografia | ⬜ | |
| Controles de sugestão | ⬜ | |
| Configurações de notificação | ⬜ | Não se aplica |
| Acessibilidade | ⬜ | |

## Ajuda

| Item | Estado | Observação |
| --- | --- | --- |
| Lista de funções | ✅ | Mesma cascata do menu Inserir |
| Atalhos do teclado | ✅ | |
| Sobre | ✅ | |
| Pesquisar nos menus | ✅ | Campo "Menus" na barra |
| Treinamento / atualizações / termos | ⬜ | Não se aplica |

## Barra de ferramentas

| Item | Estado | Observação |
| --- | --- | --- |
| Desfazer / refazer | ✅ | |
| Imprimir | ✅ | Menu e barra |
| Pincel de formatação | ✅ | Copia o estilo da célula ativa para a próxima seleção |
| Zoom | ✅ | Menu e barra |
| R$ / % | ✅ | |
| Diminuir / aumentar casas decimais | ✅ | `.0−` e `.00+` |
| 123 (formatos) | ✅ | |
| Família e tamanho da fonte | ✅ | 8 famílias e o controle `− 13 +` |
| N / I / S / cor do texto | ✅ | |
| Cor de preenchimento | ✅ | |
| Bordas | ✅ | Todas, externas, internas, lado a lado |
| Mesclar | ⬜ | |
| Alinhamento horizontal / vertical / quebra | ✅ | Todos na barra; rotação ainda não |
| Inserir link / imagem / gráfico | ⬜ | |
| Filtro | ⬜ | |
| Σ (funções) | ✅ | Abre a mesma cascata |
| Busca "Menus" | ✅ | Encontra qualquer comando dos menus |

---

## Edição de fórmulas

| Item | Estado | Observação |
| --- | --- | --- |
| Autocompletar o nome da função | ✅ | Com assinatura e descrição |
| Dica do argumento atual | ✅ | Destaca em qual argumento o cursor está |
| Referência clicando na célula | ✅ | Arrastar seleciona intervalo |
| **Referência pelas setas** | ✅ | Shift estende, Ctrl salta, F2 volta ao texto |
| Referências coloridas na grade | ✅ | Cada referência com a sua cor |
| Alça de preenchimento | ✅ | Sequências, textos numerados e fórmulas |

## Menu do botão direito

| Item | Estado | Observação |
| --- | --- | --- |
| Na célula: recortar, copiar, colar, colar valores | ✅ | |
| Na célula: inserir/excluir linha e coluna | ✅ | |
| Na célula: bordas, limpar formatação | ✅ | |
| Na célula: intervalo nomeado, estatísticas | ✅ | |
| No cabeçalho da coluna: inserir/excluir/ajustar largura | ✅ | |
| No cabeçalho da linha: inserir/excluir/limpar | ✅ | |
| Na aba: renomear, duplicar, cor da aba (pintar), excluir | ✅ | Paleta de 9 cores, destaque de borda inferior e popup posicionado automaticamente |
| Ocultar linha/coluna, redimensionar pelo menu | ⬜ | |

## Fora dos prints, mas conhecido

- Arrastar as abas para reordenar (o motor já tem `moveSheet`, falta a interface)
- Ocultar aba e cor da aba (✅ Cor da aba implementada com paleta de 9 cores e salvamento no .plz)
- Seleção múltipla com Ctrl
- Ajustar altura da linha pelo conteúdo (a largura da coluna já tem)
- Alça de preenchimento com padrões de data e dias da semana
- Recálculo em Web Worker para planilhas muito grandes
- Vírgula decimal já funciona (`=1,5+2,25`); `;` separa argumentos
- Impressão com quebra de página e cabeçalho repetido

## Fórmulas de matriz ao estilo Ctrl+Shift+Enter

| Item | Estado | Observação |
| --- | --- | --- |
| Literal `{1;2;3}` | ✅ | `;` separa linhas, `\` separa colunas (pt-BR) |
| `SE` item a item | ✅ | Condição em matriz avalia os dois ramos e combina |
| `ANO`, `MÊS`, `DIA`, `DIA.DA.SEMANA` item a item | ✅ | |
| `MAIOR`/`MENOR` com k em matriz | ✅ | `MAIOR(A1:A9;{1;2;3})` |
| Aritmética entre intervalos | ✅ | `(A2:A9="x")*(ANO(C2:C9)=2024)` |
| Texto e lógicos ignorados dentro de matriz | ✅ | Faz `MAIOR(SE(…))` funcionar |
| Argumento em branco (`SEQUÊNCIA(3;;;0)`) | ✅ | Vale o padrão da função |
| `LIN()` / `COL()` sem argumento | ✅ | A célula que está calculando |
| Trigonometria (SEN, COS, TG, GRAUS, RADIANOS…) | ✅ | Com os nomes em português |
| `MATRIZ.MULT` / `MATRIZ.INVERSO` / `MATRIZ.DETERM` | ✅ | Gauss-Jordan com pivô parcial |
| `ÍNDICE` sobre matriz calculada | ✅ | Não só sobre intervalo |
| Resto das funções escalares item a item | ⬜ | Ampliar `ARRAY_LIFTING` |

## Já pronto e que o Sheets não tem

- Macros em JavaScript, com `planilha.funcao()` criando funções de célula
- Formato `.plz`: pasta inteira comprimida com gzip nativo do navegador
- Arquivo único: `dist/index.html` abre sem servidor, com duplo clique

---

## Armazenamento por coluna (memória) — ✅ pronto (19/09/2026)

Um objeto por célula num `Map` custava ~179 bytes para guardar 8 bytes de
dado — numa planilha de milhões de linhas isso ia para a faixa de GB (o
tamanho em que o Google Planilhas já proíbe, teto de 10 milhões de células).
A análise original (trazida do Gemini) propunha um armazenamento colunar
(`Float64Array`/`Uint8Array` por coluna) em 4 passos.

**O que não valia a pena** (mantido como registro): comprimir em memória
(guardar tudo em `.plz` e descomprimir ao ler) — o desenho lê milhares de
células por quadro, e descomprimir nesse caminho mataria a fluidez.

---

### Fila de Refatoração e Modularização (Arquivos > 500 linhas):

Caso 1: O preenchimento é derivado de uma Regra/Fórmula

(Ex: C2:C5000000 = "=A2 * 1.1" ou C2:C5000000 = "Pendente")

Se o dado foi gerado por uma fórmula homogênea, um valor constante ou um incremento sequencial, você não precisa materializar nem salvar 5 milhões de valores individuais no disco no histórico do Git.

    Compressão por Metadados (Armazenamento Virtual):

        Em vez de salvar 5 milhões de floats no .plz, o Rust grava apenas a definição do bloco:

            Tipo: Virtual / Expression

            Expressão: "=A * 1.1" ou Valor: "Pendente"

            Intervalo: [Linha 2 até Linha 5.000.000]

        Custo em disco no commit: Poucos bytes.

    Materialização sob Demanda (Lazy):

        O motor em Rust só calcula o valor numérico quando a tela pede para renderizar aquele trecho ou quando uma agregação/tabela dinâmica precisa ler a coluna.

        Se o dado precisa ir para memória de cálculo, ele gera o array Arrow em tempo de execução via SIMD em milissegundos.

Caso 2: Os dados são Arbitrários / Novos

(Ex: O usuário importou um CSV novo, colou 5 milhões de linhas de fora ou uma macro gerou valores aleatórios não previsíveis)

Se os 5 milhões de dados realmente precisam existir fisicamente como valores distintos, você não tem como fugir de gravá-los, mas a sua arquitetura colunar com o .plz continua dando uma vantagem esmagadora sobre o Excel:

    Apenas a Coluna Afetada é Gravada:

        No Excel (.xlsx), que é um XML gigantesco ou binário misturado por linha, alterar a coluna C obriga o Excel a reescrever todas as outras colunas (A, B, D, E, F...) no salvamento.

        No seu ecossistema colunar, se você tem 50 colunas e alterou 5 milhões de linhas da coluna C:

            As outras 49 colunas não são tocadas. O Rust nem lê nem grava nada delas.

            Apenas os chunks da Coluna C são comprimidos e adicionados ao arquivo .plz.

    Compressão Colunar Contígua (Arrow + ZSTD/LZ4):

        5 milhões de números inteiros ou floats em formato colunar comprimem com facilidade absurda (técnicas como Run-Length Encoding, Bit-Packing ou Delta Encoding antes do ZSTD).

        Uma coluna de 5 milhões de números de ponto flutuante de 64-bit ocupa 40 MB de RAM bruta. Com compressão colunar, isso cai para menos de 8 a 15 MB em disco.

        Gravar 15 MB em um SSD NVMe moderno leva menos de 10 milissegundos.

    Escrita em Streaming por Chunks (Zero Bloqueio de RAM):

        O Rust comprime e faz o flush dos chunks em blocos de 500 mil linhas diretamente para o arquivo temporário do .plz.

        Não precisa alocar um megabuffer gigante em memória para depois cuspir no disco; o pipeline consome o stream enquanto o usuário continua interagindo com a interface.

Como Fica no "Git da Planilha":

    Se alterou C2: O commit grava ~1 MB (apenas o chunk que contém a linha 2 da coluna C) e atualiza o manifesto.

    Se preencheu C2:C5.000.000 com regra/fórmula: O commit grava alguns bytes de metadado de expressão.

    Se colou 5 milhões de valores brutos novos em C: O commit grava apenas os ~10-15 MB comprimidos daquela coluna específica. O restante do arquivo (seja ele de 300 MB, 1 GB ou 10 GB) permanece exatamente no mesmo lugar, apenas referenciado pelo novo manifesto.

## Idioma da interface (i18n) — ideia anotada em 28/09/2026, a fazer

O que a proposta sugere, resumido:

- **Arquivo de tradução por idioma** (`locales/pt-BR.json`, `locales/en-US.json`),
  chave → texto, com plural/variável no padrão ICU MessageFormat quando
  precisar (`"Exibindo {count} linhas"`).

**Por que isto é maior do que parece.** Não é só criar o dicionário — é trocar
TODO texto hoje escrito direto (o projeto inteiro, motor incluído, porque as
mensagens de erro do Rust também são texto fixo em português) por uma chamada
`t('chave')`. É um trabalho transversal, não uma tarefa de uma tarde. Também
esbarra na regra 15 (funcionalidade nova só no Rust): a troca de idioma em si
é estado de interface, mas o texto que hoje "mora" no motor (`explain.rs`,
`function_help.rs`, mensagens de validação) precisaria de uma decisão de para
onde vai o catálogo de strings de cada um.

**Não começado.** Anotado a pedido do usuário; nenhuma decisão de produto
tomada (nem se/quando o app vira multilíngue de verdade, isso muda a régua
"responder em português" da regra 1 do `CLAUDE.md`).
