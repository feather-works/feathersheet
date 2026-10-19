# Metas por versão — FeatherSheet

> Roteiro combinado com o usuário em 16/10/2026. Cada item é uma meta a
> bater: só vira `[x]` quando estiver no app, testado e conferido na tela.
> Números de desempenho medidos na máquina de referência (AMD Ryzen 7 5700G,
> 16 threads, 32 GB), com o binário release, pelo console F12.
>
> **Regra que vale para todas:** tudo em Rust/Iced, nada do conteúdo sai da
> máquina, todo texto novo entra em pt-BR, en-US e es-ES, e cada versão tem
> o "O que há de novo" nos três idiomas.

**Em aberto (decidir):** se a alpha de 30/10/2026 é a v0.1.0; e se as versões saem por data (uma por semana) ou quando a lista fecha.

---

## v0.1.0 — parte 1: o que já está pronto desde a v0.0.5

> **Lançada em 17/10/2026**, a pedido do usuário, antes da alpha de 30/10.
> Os itens ainda sem `[x]` nas duas partes continuam abertos e passam para
> a versão seguinte.

> 16/10 noite: o usuário decidiu não lançar 0.0.6–0.0.9; tudo isto sai junto
> na **v0.1.0**, com a dinâmica e a referência entre arquivos abaixo, e entra
> todo no "O que há de novo" da 0.1.0.

- [x] Barra de status personalizável (botão direito): média, contagem,
      contagem numérica, mín, máx, soma, desvio padrão, vazias, erros.
- [x] Alt+F11: código de eventos da planilha (Ao mudar células, Ao ativar a
      aba) que sobrevive a salvar e reabrir; pastas de módulos; Renomear;
      divisórias arrastáveis; Guia/F1.
- [x] Espanhol completo + de-para dos nomes de função em JSON (pt, en, es).
- [x] Idioma do sistema na primeira abertura.
- [x] Abrir pela tela inicial mostra o carregamento.
- [ ] **Abrir CSV grande até ~10× mais rápido** (pôr no "O que há de novo"):
      `spotify_train.csv` (653 MB, 946 mil linhas, 70 milhões de células,
      separado por vírgula): **29,5 s → 3,2 s com 2 núcleos** (2,0 s com 16,
      6,0 s com 1), ~1 GB de RAM. Leitura por colunas, como o `.fshz`:
      divide o texto sem criar um texto por campo, classifica cada coluna
      (atalhos exatos para número e texto) e entrega blocos de 65.536 linhas
      prontos ao motor. Medido em 16/10 (`examples/medir_csv.rs`, `taskset`
      para os núcleos). Falta conferir no app (F12) antes de marcar.
- [ ] **Abrir .xlsx com fórmulas repetidas até ~4× mais rápido** (pôr no "O
      que há de novo"): `FeatherSheet_teste_pesado_1M.xlsx` (13 milhões de
      células, 7 milhões de fórmulas): **33,7 s → 8,4 s com 2 núcleos**
      (42,7 → 15,4 s com 1; 7,4 → 5,7 s com 16). A fórmula repetida linha a
      linha é traduzida do inglês e analisada uma vez só. Conferido célula
      por célula contra o `.fshz` do mesmo arquivo.
- [ ] **Gráfico com mais de 200 mil linhas, resumido** (soma, média,
      contagem, desvio padrão…, lendo todas as linhas): acima de 200 mil
      células o resumo liga sozinho e "Não resumir" sai da lista; lê até
      100 milhões de células.
- [x] **Colar até o fim da planilha**: um valor simples vai até a linha 100 milhões em 0,33 s (em blocos, um Ctrl+Z); fórmula para na última linha com dados (antes
      gravava 100 milhões de células e travava).
- [ ] **Gráfico novo aparece na parte visível** da planilha (antes, perto de A1).
- [ ] **MCP — aba "IA (MCP)" nas Configurações**: ligar/desligar (desligado de
      fábrica), somente leitura, estado e contador de pedidos, comando de
      conexão para copiar, lista do que a IA pode fazer.
- [ ] **MCP sem instalar nada a mais**: `feathersheet --mcp` (o instalador não
      leva o `feathersheet-mcp`).
- [x] **MCP — ferramentas novas** (16/10, testadas no MCP e ao vivo no app):
      `sheet_ordenar`, `sheet_filtrar` (só ao vivo), `sheet_substituir`,
      `sheet_mover`, `sheet_copiar` (com Colar especial).
- [ ] Gráfico: trocar a categoria sem travar; contas da série pelo tipo
      (número: Soma, Média…; texto: só Contar e Contar distintos).
- [ ] Interface em inglês sem sobras em português (espessura "Média" da
      borda, linhas do motor no F12, descrição de processador e RAM).
- [ ] **Bug: inserir/excluir coluna com muitas fórmulas trava** (2 mil
      linhas = 3,3 s; 1 milhão > 13 min). `Workbook::mover_estrutura`.
- [ ] **Bug: excluir a 1ª linha de um intervalo** (`$I$2:$I$2000`, excluir
      a linha 2) vira `#REF!:$I$2000`; o Excel encolhe para `$I$2:$I$1999`.
- [ ] **Bug: fórmulas de G–M trocadas de coluna** na "Planilha Pesada.fshz"
      de 16/10 17:19 (#REF! e Resumo zerado). Reproduzir: operação +
      gravar + reabrir (`examples/conferir_estrutura_pequena.rs`).
- [x] Testes verdes em core, io, gui, macros, vba, mcp e ponte.
- [ ] Instalador para macOS (GitHub Actions: Intel e Apple Silicon, `.dmg`).
- [x] Remover a dependência por caminho do `feather-io` (o repositório público
      tem que compilar sozinho).

---

## v0.1.0 — parte 2: tabela dinâmica melhor + referência entre arquivos

### Tabela dinâmica (ligar na tela o que o motor já tem)
- [ ] (escrito 16/10, falta compilar e conferir) "Mostrar valores como" na tela: os 12 modos do Excel (% do total, da
      linha, da coluna, do pai, diferença de, % de diferença, acumulado,
      classificação, índice…). Motor pronto (`dinamica/exibicao.rs`).
- [ ] (escrito 16/10, falta compilar e conferir) Agrupar datas por ano/trimestre/mês/dia e números em faixas, pela tela.
- [ ] (escrito 16/10, falta compilar e conferir) Classificar itens (A→Z, Z→A, pelo valor) e totais gerais de linhas e de
      colunas ligados/desligados separadamente.
- [x] Duplo clique num valor abre uma aba com as linhas de origem (detalhar): 353 ms para 124 mil linhas (F12, 16/10).
- [ ] (escrito 16/10, falta compilar e conferir) Atualizar a dinâmica sozinha ao mudar a origem (opção) e botão Atualizar.
- [ ] Gravar a dinâmica no `.xlsx` (hoje só lê): o Excel abre e a reconhece
      como tabela dinâmica, não como valores soltos.
- [ ] (escrito 16/10, falta compilar e conferir) **Gráfico dinâmico (básico)**: Inserir → Gráfico dinâmico (hoje botão
      sem ação) cria um gráfico ligado à dinâmica; mudar filtro, campo ou
      Atualizar redesenha o gráfico.
- [ ] (escrito 16/10, falta compilar e conferir) **Segmentação de dados (básica)**: Inserir → Segmentação de dados (hoje
      "Em breve") cria um painel de botões de um campo, ligado a **uma**
      dinâmica; clicar filtra a dinâmica e o gráfico dela.

**Metas de número**
- [ ] 1 milhão de linhas, 2 campos de linha + 1 de valor: **≤ 200 ms** (16/10, no F12: 1 campo de linha + 1 de valor 130–205 ms; 2 campos de linha 350 ms; antes
      500 mil = 107 ms).
- [ ] Arrastar um campo no painel responde em **≤ 100 ms** até 1 milhão de linhas.

### Referência entre dois arquivos abertos
- [x] Fórmula `=[Vendas.fshz]Dados!A1` (e intervalos) aceita, com
      autocompletar do nome do arquivo e clique na outra janela para apontar.
- [x] Com os dois FeatherSheet abertos, a referência fica **viva**: quando a
      origem muda e é recalculada, o destino atualiza **em até 1 s**, só as
      células que dependem dela (sem recalcular a pasta inteira).
- [x] Origem fechada: fica o último valor guardado no arquivo, com aviso
      "valores de [Vendas.fshz] de 16/10 14:32" e botão Atualizar valores.
- [ ] Renomear ou mover o arquivo de origem: Editar links (trocar a origem).
- [x] O canal entre as duas janelas é local (mesmo esquema de chave do MCP);
      nada passa pela rede.
- [ ] `.xlsx`: grava e lê a referência externa no formato do Excel.

---

## v0.1.1 a v0.1.8 — o caminho até a v0.2 (combinado em 17/10/2026)

> Uma versão por tema, cada uma lançada quando a lista dela fecha. Itens
> marcados *(conferir)* podem já estar resolvidos: confirmar antes de mexer.

### v0.1.1 — correções
- [x] **CRÍTICO: salvar `.fshz` de 13 milhões de linhas (vindo de `.parquet`)
      bateu 20 GB de RAM** (17/10, usuário). Feito: o gravador
      (`fshz_write.rs`) não monta mais vetores Arrow nem bytes IPC da aba
      inteira; conta o tamanho numa passada e grava fatias de ~64 MB direto
      no gzip (o leitor, inclusive o da v0.1.0, já juntava vários blocos).
      Medido com `examples/medir_salvar_grande.rs` no `2026_03.fshz`: pico
      9,0 GB (pasta aberta 4,4 + cópia para gravar 4,4 + gravador ~0,1),
      arquivo idêntico (`examples/conferir_fshz_iguais.rs`).
- [ ] **A cópia para gravar** (`da_workbook`, 4,4 GB e 3,4 s **na thread da
      tela** a cada Ctrl+S no mesmo arquivo): blocos da coluna em `Arc` com
      cópia só ao editar, e o gravador lendo os blocos direto.
- [x] **Abrir esse arquivo tinha pico de 13,3 GB** para 4,4 GB residentes:
      bloco Arrow inteiro + `RecordBatch` + vetores. Agora `arrow_fluxo.rs`
      lê buffer a buffer do gzip direto para as colunas (também os arquivos
      da v0.1.0, de lote único) e `para_workbook_soltando` solta cada coluna
      ao carregar: pico 4,5 GB, 8,6 s (era 13 s). Abrir + `MÍNIMOSES` em AU +
      calcular tudo: pico 5,5 GB.
- [ ] Gravar leva 43 s (gzip nível 6, 359 MB). Nível 3: 20 s e 458 MB;
      nível 1: 13 s e 702 MB. Decidir com o usuário.
- [x] **Dinâmica: vazio e texto entravam como 0** em Média, DesvPad, Mín e
      Moda (`pivot.rs`/`dinamica_layout.rs`, `to_number(..).unwrap_or(0.)`).
      Agora como no Excel: vazio fica de fora; texto só conta na Contagem.
- [x] **Fórmula na coluna inteira** (análise de 13,7 milhões de linhas):
      a coluna empacotada aceita intervalo local (`FaixaPadrao` em
      `modelo_padrao.rs`; invalida por coluna quando o intervalo tem linhas
      fixas). `Sheet::preencher_formula_faixa_vazia` grava o padrão em bloco,
      um Ctrl+Z. Ctrl+D/alça/até o fim e colar usam o bloco quando a coluna
      abaixo está vazia. No `2026_03.fshz`: preencher AU2:AU13770159 com o
      `MÍNIMOSES` = 11 ms; calcular as 13,7 milhões = 26 s, +690 MB
      (`examples/medir_formula_coluna_inteira.rs`). Falta: `sheet_escrever`
      do MCP usar o mesmo caminho.
- [x] Colar não deslocava referência relativa (`=A1` de B1 em B5 ficava
      `=A1`): agora como no Excel; recortar mantém.
- [x] Inserir/excluir coluna não mexia em `B:B`/`5:7` (`ajustar_inteiras`
      em `formula_text.rs`).
- [ ] **Recálculo fora da thread da tela**, com progresso e Esc: `MÍNIMOSES`
      em 13,7 milhões de linhas travou o app inteiro (17/10).
- [x] **`*SES` em coluna inteira travando** (`MÍNIMOSES(AO:AO;A:A;A2;…)`):
      o `*SES` já usava o índice por grupo (`planilha/agregado.rs`); o que
      travava era o grafo de dependências, que indexava `A:A` em ~98 mil
      blocos de 1.024 linhas (até a linha 100 milhões): 70 ms **por fórmula
      escrita**. Faixa alta agora vai para um índice por coluna
      (`sheet_deps.rs`). Preencher 1.000 linhas: 76 s → 12,5 ms; 100 mil:
      1,3 s (`examples/medir_minimoses.rs`).
- [x] `PARÂMETRO`/`SWITCH` comparava o texto dos dois lados: agora como no
      Excel (texto sem ligar para maiúscula; número não casa com texto).
- [ ] **MCP edita tudo** (17/10, usuário: "tem que editar tudo tudo mesmo"):
      `sheet_dinamica` feito (listar/criar/editar/atualizar/excluir, ao vivo);
      `sheet_escrever` com fórmula vai em bloco sem teto. Seguir a lista de
      `MCP-PENDENTE.md` até cobrir cada janela do app.
- [x] Índice dos `*SES` por códigos (`agregado.rs`): 13,7 milhões de linhas
      3,3 s → 1,9 s na primeira célula.
- [ ] Teste de referência da v0.1.1: a análise do desvio padrão por bloco de
      200 m no `2026_03.fshz` (13,7 milhões de linhas) feita inteira no app.
- [x] Inserir/excluir coluna com muitas fórmulas trava (2 mil linhas = 3,3 s;
      1 milhão > 13 min). Causa: cada fórmula regravada disparava a cascata
      de "sujo" pela cadeia inteira (coluna acumulada `=K1+I2`): quadrático.
      Agora `Sheet::adiar_cascata`/`soltar_cascata` no `mover_estrutura`:
      2 mil linhas em 20–48 ms (`examples/conferir_estrutura_pequena.rs`).
      O Ctrl+Z dessas operações ainda regrava célula a célula.
- [ ] Fórmulas de G–M trocadas de coluna na "Planilha Pesada" (#REF! e Resumo
      zerado): reproduzir com `examples/conferir_estrutura_pequena.rs`.
- [ ] Botão direito na guia da aba não mostra nada.
- [ ] Menu Orientação do texto: ícones como quadradinhos e menu longe do botão.
- [x] Exportar sugere o nome do arquivo: o `.xlsx` já sugeria; o CSV/TSV
      sugeria o nome da aba ("Planilha1") e agora sugere o do documento.
- [ ] Barra de progresso ao abrir fica em 100% durante "Montando… na memória".
- [ ] Inglês sem sobras: status "salvo", Colar, grupos da Página Inicial,
      abas Exibir/Macros/Ferramentas *(conferir)*.
- [ ] `.dmg` para Mac Intel (o job `macos-13` ficou parado na fila).
- [x] Ctrl+T e Região atual em 13 milhões de linhas travavam: `regiao_atual`
      e `regiao_da_tabela` leem as máscaras de presença por bloco
      (`Sheet::sequencias_com_conteudo`). F12 mostra o tempo do Ctrl+T.
- [x] Colar 13 milhões de fórmulas: o pulso que guarda os resultados nas
      colunas percorria o cache inteiro a cada segundo (`retain`); agora
      `extract_if` com cota e orçamento de 30 ms por pulso.
- [x] Imagem na célula (17/10 noite): GIF/BMP/WebP na grade, barra de
      fórmulas vazia, Inserir imagem na célula, Colocar na célula / sobre as
      células, Alt+arrastar, painel Formatar imagem, menu do botão direito na
      imagem; figura do `.xlsx` recortada sob os cabeçalhos.
- [x] Só um painel lateral aberto por vez (gráfico, dinâmica, imagem).

### v0.1.2 — sem travadas
- [x] Selecionar 87 milhões de células trava ~5 s (rodapé, realce e alças fora
      da thread da tela).
- [x] Rolar a grade com `.parquet` aberto trava.
- [x] Lag ao rolar com gráficos na tela.
- [x] Recuperação automática prepara a cópia na thread da tela (~1 s em 70
      milhões de células): mover para fora.
- [x] Lentidão com outra janela aberta (Configurações) *(conferir)*.
- [x] Apagar uma fórmula na Planilha Pesada e dar Ctrl+Z trava um pouco
      (17/10, usuário): medir no F12 o desfazer e o recálculo que ele dispara.
      *18/10: editar sujava toda fórmula que lê outra aba; agora só quem lê a
      coluna tocada — apagar 3,4 s → 0,1 ms, Ctrl+Z 3,3 s → 0,13 s. Salvar e
      recuperação: erros repetidos guardados uma vez e colunas em paralelo
      (640 → 130 ms; Ctrl+S em 13,7 mi linhas 3,8 → 0,7 s). Testado pelo usuário.*

### v0.1.3 — Excel e acabamento *(lançada em 19/10/2026)*
- [x] Referência entre arquivos gravada como vínculo externo do `.xlsx`
      (`externalLink`, fórmula `[1]Dados!A1` com os últimos valores); lida de
      volta com o nome do arquivo *(conferido no LibreOffice: mostra os valores guardados)*.
- [x] Dinâmica gravada no `.xlsx` reconhecida pelo Excel como dinâmica *(conferido
      pelo usuário no Excel, 19/10)*.
- [x] Colar até o fim também para fórmula e data formatada *(data com o formato
      numa faixa de estilo; fórmula que cabe na coluna empacotada; teste da
      interface ainda não rodado)*.
- [x] Alças do gráfico: barra mais grossa no lugar dos quadradinhos *(já era
      assim desde a v0.0.3; conferido por foto)*.
- [x] Barrinha flutuante da forma encostada na forma *(desde a v0.0.5, logo
      acima da alça de giro; conferido por foto)*.
- [x] Barra de status: contas coladas no "−" do zoom, alinhadas na vertical.
- [x] Formatar células: códigos de número sem rolagem horizontal; abas Fonte,
      Borda e Preenchimento compactas; Alinhamento cabe inteira (Reduzir para caber
      e Mesclar lado a lado).
- [x] Borda do Ctrl+C animada, como a do Excel (traço andando a cada 100 ms).

### v0.1.4 — motor novo da dinâmica
- [ ] #27: o Excel remove o desenho da aba com histograma/cascata (`chartEx`);
      falta abrir os `teste-1/2/3` para achar qual gráfico.
- [ ] Ler as colunas inteiras de uma vez (dicionário de textos do motor, sem
      `Value` por célula) e agrupar por códigos em paralelo por bloco.
- [ ] Somas guardadas por grupo: mudar uma célula da origem refaz só o grupo.
- [ ] "Adiar atualização" no painel (arrumar campos e calcular no fim).
- [ ] Medir o caso do LinkedIn (20 arquivos × 850 mil linhas × 22 colunas =
      17 milhões de linhas): tempo, RAM e tamanho do `.fshz`, com script.

### v0.1.5 — memória
- [ ] A parte do plano dos 400 MB (`PLANO-MEMORIA-400MB.md`) que o caso de 17
      milhões de linhas exigir: RAM de abrir e da dinâmica medidas no F12.
- [ ] F12 × Monitor do GNOME (~300 MB de diferença): mesmo critério de memória.

### v0.1.6 — dinâmica igual à do Excel (parte 1)
- [ ] Painel e faixa no padrão do `GUIA-JANELAS.md` ("feia e não usual", 16/10).
- [ ] Campos calculados com editor de fórmula e autocompletar dos campos.
- [ ] Itens agrupados à mão (selecionar itens → Agrupar).
- [ ] Linha do tempo para campos de data.
- [ ] Layouts completos: estrutura de tópicos, subtotais em cima/embaixo,
      recolher e expandir por nível.
- [ ] Mostrar páginas de filtro (uma aba por item).

### v0.1.7 — dinâmica igual à do Excel (parte 2)
- [ ] Estilos de dinâmica e formatação que sobrevive ao Atualizar.
- [ ] Segmentação completa (várias dinâmicas, várias seleções, estilos, `.xlsx`).
- [ ] Gráfico dinâmico completo (botões de campo, todos os tipos, `.xlsx`).
- [ ] Dinâmicas recomendadas (3 a 5 a partir dos dados).
- [ ] Origem: outra aba, outro arquivo aberto e tabela.

### v0.1.8 — além do Excel
- [ ] Contagem de distintos sem "modelo de dados".
- [ ] Mediana e percentis como agregação.
- [ ] Ctrl+Z em cada mudança do painel.
- [ ] Dinâmica de dinâmica (a saída vira origem sem copiar valores).
- [ ] Medidas simples entre campos (o "DAX" sem DAX) e tabelas relacionadas
      por uma coluna em comum.

### v0.2.0 — fechamento
- [ ] As metas de número abaixo batidas e medidas no F12.
- [ ] Ajuda F1 com tudo o que o app faz, nos três idiomas.
- [ ] Comparação com o Excel publicada no README (mesmo arquivo, script para
      repetir).

---

## v0.2 — a melhor tabela dinâmica (melhor que a do Excel)

### Recursos
- [ ] Campos calculados (motor já tem `campo_calculado.rs`) com editor de
      fórmula e autocompletar dos nomes dos campos.
- [ ] Itens calculados e itens agrupados à mão (selecionar itens → Agrupar).
- [ ] Segmentação de dados completa (a básica sai na v0.1): ligada a **mais
      de uma** dinâmica, várias seleções, estilos, e lida/gravada no `.xlsx`
      (hoje o leitor a ignora).
- [ ] Linha do tempo para campos de data (anos, trimestres, meses, dias).
- [ ] Layouts compacto, estrutura de tópicos e tabular; repetir rótulos;
      subtotais em cima/embaixo/sem; recolher e expandir por nível.
- [ ] Estilos de dinâmica (temas da suíte) e formatação que sobrevive ao Atualizar.
- [ ] Gráfico dinâmico completo (o básico sai na v0.1): botões de campo no
      próprio gráfico, todos os tipos de gráfico, e lido/gravado no `.xlsx`.
- [ ] Mostrar páginas de filtro (uma aba por item).
- [ ] Dinâmicas recomendadas: sugere 3 a 5 a partir dos dados.
- [ ] Origem: intervalo, tabela, outra aba, outro arquivo aberto, consulta do
      FeatherQuery.

### Além do Excel
- [ ] Distinto (contagem de distintos) sem precisar do "modelo de dados".
- [ ] Mediana, percentis e moda como agregação.
- [ ] Desfazer cada mudança no painel (Ctrl+Z), o Excel não desfaz.
- [ ] Dinâmica de dinâmica: a saída vira origem de outra sem copiar valores.

### Metas de número
- [ ] 10 milhões de linhas, 3 campos de linha + 2 de valor: **≤ 1 s**.
- [ ] Atualizar depois de mudar **uma** célula da origem: **≤ 50 ms**
      (recalcular só o grupo afetado).
- [ ] Memória extra da dinâmica: **≤ 10%** da origem.
- [ ] Comparação registrada no README contra o Excel com o mesmo arquivo.

---

## v0.3 — FeatherQuery (o novo nome do OpenQuery)

### Renomear
- [ ] OpenQuery → FeatherQuery em tudo: pasta `openquery/` → `featherquery/`,
      crates, tela, menus, Ajuda, F12, MCP (`sheet_consulta` → `query`),
      formato `.fquz`. Feito como a renomeação do FeatherSheet: `git mv` puro
      num commit, conteúdo depois.

### Melhorias
- [ ] Editor de passos visual: lista de passos à direita, prévia que atualiza
      ao clicar num passo, editar/remover/reordenar passos.
- [ ] Consultas guardadas dentro do `.fshz`, com botão Atualizar e "atualizar
      ao abrir" (opção).
- [ ] Fontes: aba, tabela, outro arquivo (`.fshz`, `.xlsx`, `.csv`, `.parquet`),
      pasta inteira (junta os arquivos). Web/API só por pedido explícito.
- [ ] Juntar (todos os tipos de join) e acrescentar entre fontes diferentes.
- [ ] Tipos detectados por coluna, com aviso das linhas que não converteram.
- [ ] Desfazer por passo.

### Metas de número
- [ ] Prévia de 1 milhão de linhas com 5 passos: **≤ 500 ms**.
- [ ] Juntar duas fontes de 1 milhão de linhas: **≤ 2 s**.

---

## v0.4 — macros: VBA ⇄ JS sem problema

- [ ] Eventos automáticos do VBA convertidos e rodando (Worksheet_Change,
      SelectionChange, Activate, Workbook_Open com confirmação, BeforeSave).
- [ ] Formulários do VBA (UserForm) viram componentes do editor de macros.
- [ ] Caminho de volta: macro JS → VBA, gravada no `.xlsm` (o Excel abre e roda).
- [ ] Ida e volta sem perda: VBA → JS → VBA dá código equivalente (teste
      automático com o conjunto de arquivos).
- [ ] Relatório de conversão: o que converteu, o que ficou como comentário e por quê.
- [ ] Conjunto de teste com **pelo menos 50 `.xlsm` reais**.

### Metas de número
- [ ] **≥ 95%** das macros do conjunto convertem e rodam com o mesmo resultado
      do Excel (conferido célula a célula).
- [ ] **100%** das macros do conjunto abrem sem erro de conversão (as que não
      convertem inteiras dizem onde parou).

---

## v0.5 — mais funções e FeatherQuery nas fórmulas

- [ ] Funções: de **457** para **≥ 510** das ~520 do Excel
      (`FUNCOES-FALTANTES.md`), todas com ajuda e nome em pt, en e es.
- [ ] Toda função nova com teste contra o resultado do Excel.
- [ ] Consulta na célula: `=CONSULTA("nome_da_consulta")` derrama o resultado
      e atualiza com a origem.
- [ ] SQL na célula (`=SQL("SELECT … FROM Dados")`) sobre abas e tabelas.
- [ ] Funções de texto com expressão regular completas (REGEX*).
- [ ] LAMBDA com nome no Gerenciador de nomes e funções da macro chamáveis
      da planilha (`sheet.defineFunction`).

---

## v0.6 a v1.0 — proposta (confirmar)

### v0.6 — visual e impressão
- [ ] Minigráficos (sparklines) na célula.
- [ ] Temas de cor da pasta (como os temas do Excel) aplicados a tabelas,
      gráficos e dinâmicas.
- [ ] Impressão e PDF iguais à tela: cabeçalho/rodapé, repetir linhas,
      ajustar à página, quebras manuais.
- [ ] Gráficos de mapa e funil.

### v0.7 — desempenho e memória
- [ ] Plano dos 400 MB inteiro (`PLANO-MEMORIA-400MB.md`).
- [ ] 100 milhões de linhas abrem e rolam sem travar (rolagem ≥ 60 quadros/s).
- [ ] Planilha Pesada (7 milhões de fórmulas): abrir + calcular **≤ 5 s**
      (hoje 9,7 s).
- [ ] Abertura incremental: dá para ler a primeira tela antes de terminar de carregar.

### v0.8 — arquivo, histórico e revisão
- [ ] Janela Versões: o histórico Git embutido no `.fshz` (ver, comparar
      célula a célula, voltar).
- [ ] Comentários com respostas e revisão (controlar alterações).
- [ ] Proteger planilha e pasta com senha (cifrada de verdade).
- [ ] Referência a arquivo fechado lida direto do arquivo.

### v0.9 — plataformas e acesso
- [ ] macOS assinado e notarizado; Windows assinado.
- [ ] Instalador da suíte: escolher os apps; bibliotecas comuns automáticas.
- [ ] Teclado completo (todas as ações sem mouse) e leitor de tela.
- [ ] MCP com todas as ferramentas do `MCP-PENDENTE.md`.

### v1.0 — estabilidade
- [ ] Paridade com o Excel fechada na lista de `PARIDADE-EXCEL.md`.
- [ ] Nenhum bug grave conhecido; nenhuma perda de dado conhecida.
- [ ] Autocompletar no editor de macros (`sh` → `sheet`, `sheet.r` → `range`).
- [ ] Ajuda F1 cobrindo todas as janelas e funções, nos três idiomas.
- [ ] Teste de uso: abrir, editar e salvar 100 `.xlsx` reais sem diferença
      ao reabrir no Excel.
