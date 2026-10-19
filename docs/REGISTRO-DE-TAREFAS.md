# Registro de tarefas

> Era o README de `feathersheet/` enquanto o produto morava no repositório de desenvolvimento da suíte (até 15/10/2026). Fica como histórico das entregas e validações; links para `featheroffice/` e outros produtos apontam para pastas que não existem neste repositório.


> **Produto independente — repositório planejado:** `feather-works/feathersheet`.
> O projeto ainda está nesta pasta do repositório de desenvolvimento; a separação Git não foi realizada.

**Planilha:** formatos `.fshz`, `.xlsx` e CSV; desenvolvimento e versões próprios.

**Arquitetura compartilhada:** utiliza a base de motores Rust em [`featheroffice/`](../../featheroffice/) e os componentes de interface Rust/Iced de [`feather-ui/`](../../feather-ui/) quando aplicáveis. As dependências compartilhadas serão versionadas sem acoplar um produto ao código interno de outro.

**Publicação prevista:** versão **alpha em 30/10/2026**. O produto terá distribuição própria e também poderá ser selecionado isoladamente ou junto aos demais em um **instalador geral planejado**. Isso não significa que a funcionalidade de instalação já esteja pronta.


📚 [Índice da documentação](../docs/README.md)

## Decisão arquitetural — 12/10/2026

**Fonte ativa: Rust/Iced nativo.** O frontend TypeScript/Tauri foi descontinuado e sua interface foi arquivada em [`descarte/feathersheet/legacy-frontend/src/`](../../descarte/feathersheet/legacy-frontend/src/). Não corrigir imports, telas, CSS nem o build Vite legado: o diretório existe apenas para consulta durante a migração de funcionalidades para Rust. Futuramente, a execução na web deverá reutilizar os crates de motor Rust compilados para **WebAssembly**, não reativar a antiga UI TypeScript.

Os arquivos `feathersheet/src/core/`, `modelo/`, `io/`, `ponte/`, `openquery/core/` e `src-tauri/` permanecem por enquanto como referência de paridade/compatibilidade: **não significam que o antigo frontend continua suportado**. Os JSON em `src/idiomas/` permanecem ativos porque o GUI Rust os incorpora via `include_str!`. Não assumir que mover a interface conclui a migração de todas as capacidades: lacunas Rust continuam no checklist.


## Auditoria de paridade do frontend TypeScript → Rust/Iced

[**Mapa detalhado: 89 capacidades, 150 arquivos frontend, 112 arquivos de suporte e classificação 3 aplicativos × exclusivo do FeatherSheet**](../docs/migracao/AUDITORIA-FRONTEND-TS-RUST-3APPS.md).

A auditoria cruza o código TS, os widgets/motores Rust já existentes e o
checklist `featheroffice/PLANO-INTEGRACAO-CHECKLIST.md`. Marca separadamente
**ativo no código**, **parcial**, **motor Rust sem GUI equivalente** e
**não localizado**, sem assumir que código publicado equivale a testes Cargo
ou aceite visual. Explica o que pertence ao `feather-ui`/motores comuns
para Slide, Docs e Sheet e o que deve continuar no `feathersheet-gui`.


## APP-016 — Menu contextual de abas reposicionado e cores FSHZ

O botão direito nas **abas inferiores** abre agora o menu compacto **acima da
barra de guias**, alinhado à posição horizontal real do ponteiro (inclusive após
rolagem). Foram mantidos **Nova aba**, **Renomear**, **Excluir** quando possível
e adicionada **Cor da aba…**, com dez cores e **Sem cor**. A guia mostra a cor
escolhida e grava pelo campo nativo `Sheet::tab_color`/`tc` do formato FSHZ
— sem modificar células nem o formato do documento.

Os testes em [`cores_aba.rs`](../crates/feathersheet-gui/src/app/cores_aba.rs)
cobrem geometria, seleção de aba inativa, paleta, cor e round-trip FSHZ.
**Cargo e verificação visual ainda pendentes — APP-016 `[/]`.**
[Detalhes e comandos](../../featheroffice/APP-016-ABAS-CORES-MENU.md).

## Avisos de operações na própria barra de abas

Mensagens transitórias como **“Aba excluída”**, salvar, erros e operações
recentes agora aparecem como um aviso centralizado **na mesma linha das
guias**, sem criar uma linha de texto permanente abaixo. O aviso some
automaticamente após cerca de **5 segundos**, reutilizando o pulso
`MonitorPulso` já existente (1 Hz), e uma nova ação reinicia o prazo.
Duas exclusões consecutivas também renovam o aviso, mesmo com texto igual.

O status histórico permanece armazenado em `App::status`, mas o rodapé
fica reservado a célula, quantidade de abas e controles de zoom.
Não muda o formato `.fshz`, a Ribbon, nomes das guias ou a estrutura da
planilha. Foram incluídos três testes Rust de ciclo de vida do aviso.

```bash
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml aviso_aba_ -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml exclusoes_seguidas -j 2
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

**Compilação/inspeção Iced ainda não validadas neste ambiente.**


## UI-013 — Ajuda completa migrada para Rust/Iced (validação pendente)

A Ajuda F1 substituiu a página estática nativa pelo conteúdo do
TypeScript: **11 guias “Como usar”**, **161 funções de planilha com
assinaturas/descrições/aliases**, **128 verbetes de macro** e **36 atalhos**.
O menu lateral possui quatro seções com subseções, introduções e
anterior/próximo. A busca ignora acentos; **Ctrl+F** busca na própria Ajuda.
F1 no editor de fórmulas pesquisa a função em edição. Os textos pt-BR/en-US
continuam nos JSON originais; teste inglês com
`FEATHEROFFICE_LANG=en-US`.

**Limites:** os atalhos configuráveis mostrados são os padrões TS, pois a
interface Iced ainda não tem seu editor de atalhos; o foco exato numa linha
de função e a seleção de idioma nas preferências também faltam.
**Cargo/GUI pendentes; UI-013 `[/]`.**
[Guia e comandos](../../featheroffice/UI-013-AJUDA-RUST.md).

## Menu TypeScript recuperado na interface Rust/Iced

O botão **Menu** ao lado das abas da Ribbon abre os **nove grupos do
TypeScript**: Arquivo, Editar, Ver, Inserir, Formatar, Dados, Macros,
Ferramentas e Ajuda. Os submenus mantêm a ordem herdada, têm breadcrumb
e botão Voltar, e exibem o estado atual de Zoom, tema, barra de fórmulas,
linhas de grade e Pintar linha/coluna. O painel é compacto e possui
scroll interno, sem ocupar toda a largura da planilha.

Ações que já existiam no Rust, mas não estavam conectadas ao catálogo,
foram habilitadas: Colar somente valores e somente formatação, Inserir
Tabela e Imagem, Limpar formato, Cores alternadas, Ajustar largura,
Configuração de formatação condicional, Catálogo de funções e Sobre.
A Ribbon também agora executa **Colar valores**. Comandos TypeScript
ainda não implementados permanecem visíveis como **em migração**,
sem executarem mensagens vazias.

[Comparativo TS × Rust, limites e comandos de teste](../docs/migracao/MENU-TYPESCRIPT-RUST.md).
**Validação Cargo e revisão visual em Linux/Windows pendentes.**


## Editor visual de macros VBE em Rust/Iced (MAC-002)

O painel auxiliar **Macros** reproduz agora a organização do editor TypeScript:
menus, barra Executar/Depurar, Explorador de projeto (abas e módulos reais),
Propriedades, abas de código, designer de componentes (prévia com Desenho/HTML/CSS),
Janela Imediata, Locais, Pilha de chamadas e Inspeção. **A prioridade é a estrutura
visual**: comandos TS ainda não portados são exibidos desativados; o motor
`featheroffice-macros` e as rotinas JS já existentes continuam funcionais.
[Comparativo TS ↔ Iced, comandos e testes](../../featheroffice/MAC-002-EDITOR-VISUAL-ICED.md).

## Macros nomeadas usando o motor comum do FeatherOffice

O editor **Macros** agora usa `featheroffice-macros`, um runtime JS compartilhado. Permite guardar, abrir e excluir módulos nomeados, importar o catálogo JSON `office.macros.v1` do TypeScript e exportá-lo. Os módulos ficam na configuração geral do FeatherOffice, fora do documento. O atributo `auto` legado é preservado, mas nenhuma macro executa automaticamente. Operações básicas da planilha e rollback em caso de erro continuam funcionando; componentes HTML/depurador/eventos ainda pendentes. [Manual de migração e testes](../../featheroffice/MAC-001-MOTOR-MACROS-COMUM.md).


## Criar Tabela (Ctrl+T) — Rust/Iced, em validação

O botão **Inserir → Tabela** e o atalho **Ctrl+T** criam uma tabela nomeada
sobre o intervalo selecionado, com detecção automática de região e cabeçalho,
nome único, cor azul no cabeçalho e linhas alternadas. Na célula vazia, o
FeatherSheet sugere uma tabela 6 × 3 com títulos Coluna1, Coluna2 e Coluna3.
A criação preserva valores/fórmulas existentes e bloqueia sobreposições.
Metadados são guardados em `Sheet.tb` no `.fshz`; a pintura é virtual
no Canvas, não copia milhões de células.

**Paridade das tabelas TypeScript, em validação:** após a criação foram migrados
o painel de Design, renomear, converter em intervalo, paletas claro/escuro,
faixas por linha e coluna, borda, totais (soma, média, contagens, extremos e
desvio), filtros por valores com pesquisa/multisseleção e por regras
texto/número, ordenação, AutoFiltro (Ctrl+Shift+L), crescimento por digitação
e colagem e reajuste após alterar a estrutura. O motor colunar original e
`Sheet.tb` permanecem responsáveis pelos dados e metadados.

**Não é paridade final homologada:** os popovers TS avançados ainda diferem,
totais em milhões de linhas precisam de cache por revisão, e desfazer/refazer
de metadados continua pendente. Os testes Cargo/GUI ainda precisam ser
executados no Linux.

[Guia Criar Tabela](../docs/funcionalidades/CRIAR-TABELA-RUST.md) ·
[Paridade das tabelas TypeScript → Rust](../docs/funcionalidades/TABELAS-TS-PARA-RUST.md).


## Divisórias fixas entre cabeçalhos e células (estilo Google Sheets)

A grade Iced exibe uma linha de separação contínua entre a numeração de linhas
e as células e outra entre as letras das colunas e as células, além de traços
finos para separar os números e as letras vizinhos. São independentes da opção
**Exibir → Linhas de grade**, permanecem visíveis no zoom e na rolagem e não
alteram o arquivo nem os estilos. [Detalhes e testes](../docs/interface/GUI-DIVISORIAS-CABECALHOS.md).


## Pintar linha/coluna da célula ativa (visual, sem alterar dados)

No FeatherSheet nativo, **Exibir → Pintar linha/coluna** vem ativado na primeira execução. A última escolha (ligado/desligado) é gravada automaticamente nas preferências locais (`config-iced.json`) e recuperada ao reabrir o aplicativo. Selecionar
**D4** realça toda a parte visível da linha 4 e da coluna D; a interseção
fica mais forte. Ao navegar para **F9**, a cruz acompanha a célula ativa.
O botão da aba Exibir desliga/religa o destaque; zoom, temas e rolagem são
respeitados. É um overlay apenas do Canvas: **não altera estilos nem FSHZ**.

Os testes de seleção e não persistência foram acrescentados, mas dependem
de execução Cargo no computador do mantenedor.
[Comandos de teste e detalhes](../docs/interface/PINTAR-LINHA-COLUNA.md).


## Inserir imagem PNG/JPEG na planilha (GUI Rust/Iced)

Na aba **Inserir → Imagem**, um clique abre diretamente o seletor de arquivo.
A imagem escolhida aparece **sobre a grade** em tamanho inicial contido
(até 320 × 220 pixels), preservando proporções. O objeto fica no metadado
`Sheet.im` do FSHZ, **sem escrever valores nas células nem alterar fórmulas**.
Ao salvar/reabrir `.fshz`, os bytes PNG/JPEG e a posição do objeto são
recuperados no painel. Limite atual: 16 MiB / 4 milhões de pixels,
até 64 imagens por aba.

A antiga tela de prévia/efeitos foi **retirada do botão Imagem**,
conforme solicitação de UX: esse botão abre apenas o seletor e insere o
objeto. Para editar depois, use o botão separado **Inserir → Efeitos
(última imagem)**. O painel flutuante usa `feather-ui` e
`featheroffice-image` para Original, Cinza, Desfocar e Nitidez, com
opção de Exportar PNG. Ao aplicar um efeito, o PNG atualizado é salvo no
`src` da imagem em `Sheet.im`, preservando ID, posição, tamanho e demais
objetos; filtros não se acumulam enquanto o editor permanece aberto.
Arrastar/redimensionar/excluir imagens e integrá-las ao XLSX seguem
pendentes.
A renderização usa coordenadas lógicas aproximadas da grade para posicionar
as imagens; layout com alturas e larguras de células personalizadas ainda
precisa de refinamento. Testes Rust foram adicionados, mas Cargo/smoke Iced
não foram executados neste ambiente.

[Detalhes da integração dos efeitos IMG-014](../../featheroffice/IMG-014-EFEITOS-DOCS-SHEET.md).


### Mover e redimensionar imagens na grade

Clique na imagem importada para selecioná-la: aparece uma **borda azul** e
uma alça **↘ no canto inferior direito**. Com o botão esquerdo pressionado:

- **Arraste pelo corpo da imagem:** mova-a sobre a planilha.
- **Arraste pela alça ↘:** aumente ou reduza a largura e a altura, mantendo
  a proporção original e tamanho mínimo de 24 pixels por lado.
- **Solte o mouse:** o FeatherSheet grava a posição e as dimensões em
  `Sheet.im`; salvar/reabrir `.fshz` mantém os ajustes.
- **Esc durante o arraste:** cancela sem persistir a posição provisória.
  Clicar na grade desfaz apenas a seleção visual.

A prévia do movimento fica na UI e **não reserializa o PNG/JPEG a cada
movimento do cursor**. A seleção respeita zoom e, quando há sobreposição, a
imagem selecionada é desenhada por último. A forma de posicionamento atual
ainda considera largura/altura padrão das células; larguras de colunas
personalizadas precisam de um refinamento de geometria da grade.

**Testes Rust adicionados, mas a execução com Cargo e o teste visual
do arraste na GUI real ainda estão pendentes.**





## Testes dos codecs .fshz (IO-009 / IO-013)

O FeatherSheet conserva o formato `FSH2` + GZip/Arrow. A bateria
**IO-009** testa round-trip de valores, fórmulas, Unicode e estilos;
**IO-013** testa colunas densas e extensas com numéricos, dicionários,
bitmap, fórmulas, metadados e arquivo truncado. Não modifica o
armazenamento colunar nem substitui as agregações/fórmulas existentes.
Ambas as demandas permanecem `[/]` até teste Cargo.

```bash
cargo run --manifest-path feathersheet/crates/feathersheet-io/Cargo.toml --example io009_roundtrip -j 2 -- /tmp/io009-sheet.fshz
cargo run --manifest-path feathersheet/crates/feathersheet-io/Cargo.toml --example io013_extremos -j 2 -- /tmp/io013-sheet.fshz
```

[Corpus IO-009](../../featheroffice/fixtures/io009/README.md) ·
[Estresse IO-013](../../featheroffice/fixtures/io013/README.md)


> **Migração para Rust/Iced:** veja o [plano completo de paridade do frontend](../docs/migracao/PLANO-MIGRACAO-ICED.md) (mouse, teclado, menus, ferramentas, arquivos, gráficos e desativação do Tauri).


## TBL-013 — Tipografia e estilos comuns somente no viewport (em validação)

A grade do FeatherSheet agora chama
`estilos_comuns::projetar_estilo` para cada **célula visível** no Canvas.
O adaptador recebe `feathersheet-core::Style` já resolvido e as sobreposições
de formatação condicional, produzindo `featheroffice-tables::EstiloCompleto`
(fonte, tamanho, negrito, itálico, sublinhado, cores/fundo, quebra de linha
e alinhamento) e um indicador separado de tachado.

O desenho Iced usa de fato essa projeção para fonte, cores, preenchimento,
alinhamento vertical, sublinhado e tachado; bordas continuam no adaptador
compartilhado `bordas_comuns` (APP-010). Regras condicionais prevalecem
**somente no visual**, inclusive `false` explícito, sem modificar valores,
fórmulas, styles no núcleo ou persistência FSHZ/XLSX. Alinhamento horizontal
automático e formatos numéricos PT-BR continuam em `feathersheet-core`,
porque dependem do tipo e locale originais.

**Nada cria uma `Tabela` densa para Sheet.** A planilha continua com
`ColumnStore`, engine de fórmulas, 10 milhões de linhas e lazy recalc
próprios. O projetor não solicita outras células nem escaneia a planilha.
Sete testes Rust cobrem estilos, condições, cores inválidas, formatação
ausente e estilo real na última linha permitida preservando uma fórmula.

Valide a partir da raiz do repositório:

```bash
git pull --ff-only origin main
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml estilos_comuns -j 2 -- --nocapture
cargo check --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

Status `[/]`: código publicado, ainda sem compilação ou verificação visual
efetiva neste ambiente.

## APP-010 — Bordas comuns no Canvas Iced (em validação)

A grade virtualizada do FeatherSheet reutiliza agora
`featheroffice-tables::BordaVisual` ao desenhar as bordas
`top/right/bottom/left` existentes. O adaptador é chamado
**somente para células visíveis**, conserva as cores RGB/RGBA,
a espessura de 1,5 px lógicos e os fallbacks do tema claro/escuro.
As fórmulas, o motor colunar e a serialização `.fshz` não foram
modificados. O formato de número e os demais estilos continuam
pertencendo ao motor Sheet.

```bash
cargo check --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml --all-targets -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml bordas_comuns::testes -j 2 -- --nocapture
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

Documentação de limites e matriz de testes:
[APP-010-BORDAS-FEATHERSHEET.md](../../featheroffice/APP-010-BORDAS-FEATHERSHEET.md).
**APP-010 permanece `[/]`** até smoke
Iced e verificação visual.

## Galeria de tipos compacta (Rust/Iced, em validação visual)

A janela **Escolha o tipo de gráfico** foi redesenhada com as medidas do
seletor TypeScript de `src/ui/grafico/chart-panel.ts` como referência.
No Iced, o popover diminuiu de **580×440 para 360×360 px**;
os cartões passaram a ser **horizontais**, com ícones vetoriais de
**36×24 px**, três opções por linha, legendas de 11 px e espaços reduzidos.
A seleção usa borda azul com fundo azul claro, no lugar de um grande bloco
azul. Opções sem renderizador permanecem visíveis, mas desativadas, com
opacidade menor e explicação no rodapé. A rolagem é restrita ao catálogo.

A mudança é **somente visual**: mantém as seis famílias, os 25 tipos
do catálogo, os sete tipos habilitados, callbacks, renderização e arquivos
`.fshz` como antes. Código: `src/app/graficos/seletor_tipo.rs` e
`src/app/graficos/editor.rs`.

Na raiz do repositório:

```bash
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml seletor_tipo -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml galeria_abre_fecha_e_seleciona_tipo_disponivel -j 2
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

Teste visual: abrir **Inserir → Gráfico**, verificar a galeria menor e
rolagem, seleção com borda discreta, sete itens clicáveis e tipos futuros
desativados. **Cargo e aparência visual precisam ser validados na máquina.**

## APP-009 — Gráficos compartilhados na grade Iced (em validação)

A extração, os filtros e as agregações permanecem no `feathersheet-core`.
A prévia e os gráficos salvos da planilha compartilham agora a mesma
aplicação de cores e nomes antes da renderização por
`featheroffice-charts`. O resultado SVG é compatível com a paleta
antiga quando não existe o bloco `_featherNative`, sem alterar o
formato `.fshz`.

```bash
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml app009_ -j 2 -- --nocapture
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

[Contrato da APP-009](../../featheroffice/APP-009-CHARTS-FEATHERSHEET.md).
**Status `[/]`** até teste Cargo, abertura de gráficos salvos e
comparação visual da prévia com o embed em Linux/Windows.

## Limite inferior de 10 milhões de linhas e numeração legível (Rust/Iced)

Corrigido o Canvas da planilha em `crates/feathersheet-gui/src/grade.rs`:

- A coluna fixa dos números de linhas cresceu de **55 px para 90 px** em
  coordenadas lógicas, suficiente para mostrar **10.000.000** sem cortar
  os dígitos do lado esquerdo, inclusive com zoom entre 50% e 200%.
- Ao chegar à linha 10.000.000 (`MAX_ROWS`), o início da grade é
  limitado pela **altura real do Canvas, pelo zoom e pelas alturas das
  últimas linhas**. Assim, a última linha fica junto ao rodapé da grade,
  sem deixar um grande retângulo em branco abaixo dela.
- O scroll do mouse parte da origem **realmente desenhada**, para voltar
  da última linha no primeiro giro. Clique, seleção e contextos de linha
  usam exatamente a mesma origem visual: não há descompasso de células.
- A **ancoragem no rodapé** também é preservada quando o usuário modifica
  o zoom em 50–200%; rolar de volta para cima desativa a ancoragem,
  permitindo navegar normalmente sem ficar preso ao fim.
- O limite do motor continua sendo `MAX_ROWS = 10_000_000`, sem criar
  linhas adicionais nem materializar dez milhões de objetos.
- Resta um espaço inferior de no máximo uma linha em viewports cuja altura
  não é múltipla das alturas visíveis; isso permite exibir a linha final
  inteira, sem recortar o rótulo.

**Validar na raiz do FeatherOffice:**

```bash
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml limite_inferior_10_milhoes -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml zoom_mantem_linha_dez_milhoes -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml ponto_da_grade_respeita_deslocamento_e_largura -j 2
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

Teste visual: usar a caixa de nome para ir a `A10000000`; conferir a
numeração lateral completa, o rodapé sem vazio extenso, a roda do mouse
subindo imediatamente e o mesmo comportamento em zoom 50%, 100% e 200%.
**Ainda requer execução de Cargo e validação visual na máquina do mantenedor.**

## UI-009 — Scroll e menu contextual responsivo (validação pendente)

A grade Iced calcula o popup contextual usando a posição do cursor e
as dimensões reais do viewport, preferindo abrir abaixo/à direita ou
acima/à esquerda quando falta espaço. Janelas pequenas limitam o menu
ao espaço disponível e mantêm o `scrollable` interno. Scroll da grade
fecha o popup antigo sem alterar seleção, valores ou fórmulas. O zoom
não modifica as coordenadas lógicas do overlay.

Os oito testes (seis de geometria e dois de eventos) foram integrados
ao smoke da GUI. Para conferir:

```bash
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml geometria_overlay::testes -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml ui009_ -j 2
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

Guia de teste visual e limitações:
[UI-009-SCROLL-OVERLAYS.md](../../featheroffice/UI-009-SCROLL-OVERLAYS.md).
O status global permanece `[/]` até executar o Cargo, validar Linux/Windows
e conferir overlays correspondentes nos demais apps.

## Ctrl+Shift+V — colagem especial (validação pendente)

No FeatherSheet Iced, **Ctrl+V** mantém a colagem completa (valores,
fórmulas e formatação resolvida da cópia interna). **Ctrl+Shift+V** cola
**somente valores**, preservando a formatação já existente no destino.
Ao copiar uma fórmula na própria planilha, o atalho cola o **resultado
calculado no momento da cópia**, não a fórmula nem o estilo de origem.

O **menu de contexto (botão direito)** também oferece:
- **Colar somente valores:** mesmo comportamento do Ctrl+Shift+V.
- **Colar somente formatação:** aplica os estilos das células copiadas
  **sem substituir conteúdo, fórmulas ou valores** do destino. Essa opção
  requer uma cópia interna reconhecida do FeatherSheet; conteúdos externos
  em TSV não transportam estilos.
  
Os dois modos reutilizam o plano da colagem normal: blocos retangulares,
repetição em múltiplos da origem, limite de 100 mil células, seleção
atualizada e **um único passo de desfazer/refazer**. Valores que chegam
de outra aplicação podem ser colados como texto TSV, mas sem metadados de
formatação de origem.

**Validar na raiz do repositório:**

```bash
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml colar_valores_de_formula -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml colar_somente_formatacao -j 2
cargo test --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml ctrl_shift_v -j 2
cargo run --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

Na janela: digitar uma fórmula com estilo na origem; Ctrl+C; selecionar
destino com estilo distinto; Ctrl+Shift+V; confirmar resultado sem
formatação de origem; Ctrl+Z e Ctrl+Y. Depois, copiar uma célula estilizada
e usar o botão direito → **Colar somente formatação** sobre uma fórmula
existente: o texto da fórmula deve permanecer idêntico.

## GUI 100% Rust/Iced (08/10/2026)

A interface nativa agora tem um crate separado:

```bash
cargo run --release --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml
```

Sem npm, Node, TypeScript, WebKit nem Tauri para executar este binário.
Depende de `feather-ui`, `feathersheet-core`, `feathersheet-io`, `iced` e `rfd`.

**Já implementado na GUI Iced inicial:**
- Template global com barra própria, controles do sistema, menu Arquivos,
  Configurações/Ajuda/Monitor em janelas nativas independentes;
- **Grade Canvas virtualizada** em Iced, desenhando apenas a área visível,
  com clique, rolagem por mouse, larguras e alturas definidas no motor;
- Edição pela barra de fórmulas, digitação direta e navegação com setas;
- **Copiar/recortar/colar áreas retangulares** pela seleção com mouse ou Shift+setas:
  Ctrl+C exporta TSV para outros aplicativos; Ctrl+V numa célula insere o bloco
  inteiro; Ctrl+V numa área com dimensões múltiplas do bloco preenche a seleção
  por repetição (ex.: copiar A1:B2 e colar em D4:G7). Cópia interna preserva
  fórmula e estilos; texto externo usa TSV sem importar formatação. A colagem
  é atômica para desfazer/refazer, limitada a 100.000 células e recusa área
  incompatível sem gravação parcial;
- **Faixa de opções funcional**: negrito/itálico, moeda/porcentagem,
  alinhamento, desfazer/refazer, inserir/excluir linha e coluna;
- Múltiplas abas, renomear e excluir aba;
- Abrir/gravar .fshz, importar CSV/TSV e **abrir/exportar .xlsx** pelos
  módulos Rust existentes; exportar .xlsx não substitui o documento .fshz;
- Parser CSV/TSV em lotes migrado para `featheroffice-io::csv_lotes` (IO-006):
  `feathersheet-io::csv` preserva a API por reexportação. Mantém detecção de
  delimitador, aspas, BOM UTF-8, CRLF, multilinhas e texto PT-BR sem conversão
  prematura, com opção de lotes para reduzir a memória na importação.
  Smoke: `cargo run --manifest-path feathersheet/crates/feathersheet-io/Cargo.toml --example validacao_csv_lotes -j 2`;
- Tema claro/escuro, cor de destaque e janelas próprias para F1/F12/configuração.

### APP-011 — importação CSV/TSV comum, codecs nativos preservados (08/10/2026)

A janela Iced agora encaminha CSV/TSV para
`feathersheet-io::csv_workbook::importar_csv_tsv`, que consome
`featheroffice-io::csv_lotes` por meio de `feathersheet-io::csv`.
A leitura mantém lotes de até 50 mil linhas, BOM UTF-8, delimitadores
detectados (TSV força tabulação), CRLF, campos citados e valores PT-BR
como texto no motor colunar; evita a conversão para tabela densa comum.
O trabalho continua na task de importação fora da thread de UI.

A migração **não substituiu** os codecs de `.xlsx` e `.fshz`, nem o
salvamento atômico do FSHZ. Foram acrescentados testes do adaptador e
smoke executável cobrindo CSV, TSV, XLSX e FSHZ.

```bash
cargo test --manifest-path feathersheet/crates/feathersheet-io/Cargo.toml csv_workbook -j 2
cargo run --manifest-path feathersheet/crates/feathersheet-io/Cargo.toml --example validacao_app_io -j 2
cargo check --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml -j 2
```

**Status:** APP-011 `[/]` (publicada, testes reais e GUI ainda pendentes).

**Ainda faltando na GUI Iced:** importação de formatação rica de
aplicativos externos (o clipboard interoperável é TSV), barras de rolagem no estilo Excel,
pintura completa das fontes/estilos e objetos na grade, filtros, tabelas
dinâmicas, gráficos, macros, impressão, menus e atalhos completos,
e outras ferramentas da interface Tauri legada. O fato de o motor suportar um recurso não significa que a
GUI nativa já o expõe. O frontend Tauri legado permanece no repositório
temporariamente para não perder funcionalidade enquanto esses recursos
são migrados. Não faz parte do executável Iced.

Para verificar: `cargo check --manifest-path feathersheet/crates/feathersheet-gui/Cargo.toml`.
O workflow `feathersheet-rust.yml` testa a GUI nativa em GitHub Actions.


**Direção atual — 07/10/2026: motor e interface 100% Rust, com Iced.**
Prioridade: **FeatherSlide → FeatherDocs → FeatherSheet**; FeatherCAD
depois, se houver tempo. O inventário abaixo registra funcionalidades e
decisões anteriores; a migração da interface deste aplicativo ainda deve
ser conferida no código, sem presumir que os recursos já existem em Iced.

## Por que é rápido

| Decisão | Efeito |
| --- | --- |
| Grade desenhada em **canvas**, não em DOM | só as células visíveis são desenhadas; rolar 100 mil linhas custa o mesmo que rolar 10 |
| Armazenamento **esparso** (`Map<int, Cell>`) | uma planilha de 1M×16K linhas só ocupa memória onde há conteúdo; a chave é um inteiro, não string |
| **Grafo de dependências** com propagação de "sujo" | editar uma célula só invalida quem depende dela, em vez de recalcular a planilha inteira |
| Recálculo **preguiçoso** (pull) | uma fórmula suja fora da tela só é avaliada quando alguém pede seu valor |
| Índice em blocos de 1024 linhas para intervalos | `=SOMA(A:A)` não força varrer um milhão de dependências a cada edição |
| Varredura esparsa em intervalos grandes | quando o intervalo é maior que o número de células preenchidas, percorre o mapa em vez do retângulo |
| Prefix-sum nos eixos (`Axis`) | pixel → linha/coluna em O(log n), mesmo com alturas e larguras personalizadas |
| Um único `Path2D` por quadro para as linhas de grade | milhares de linhas em um `stroke()` |

## Arquitetura

O `Sheet` implementa `EvalContext`, então o motor não sabe que existe uma interface —
dá para usá-lo em um script Node, num worker ou em testes sem montar nada.

## O que já funciona

- Fórmulas com `+ - * / ^ & %`, comparações, parênteses, referências, intervalos (`A1:B9`, `A:A`)
- 95 funções (160 nomes contando os apelidos em português): SOMA, MÉDIA, SE, SEERRO, PROCV, PROCH, ÍNDICE, CORRESP, SOMASE, CONT.SE,
  texto (ESQUERDA, EXT.TEXTO, SUBSTITUIR…), datas (HOJE, DATA, ANO…), estatística (DESVPAD, MED…).
  Os nomes em inglês também valem.
- Detecção de referência circular, propagação de erros, funções voláteis (ALEATÓRIO, HOJE)
- Seleção por arraste, seleção de linha/coluna inteira, Ctrl+setas, Shift+setas
- Edição na célula ou na barra de fórmulas; clicar numa célula durante a edição insere a referência
- Formatação: negrito, itálico, alinhamento, cor, fundo, formatos numéricos pt-BR
- Redimensionar colunas e linhas (duplo clique na borda ajusta à largura ideal)
- Copiar/recortar/colar (TSV, compatível com Excel e Google Sheets)
- Desfazer/refazer em transações, importar/exportar CSV, salvar/abrir JSON
- Salvamento automático em `localStorage`, tema claro/escuro pelo sistema

## Próximos passos

- Congelar painéis, múltiplas abas, gráficos, `.xlsx` real (OOXML) e recálculo em Web Worker.

## Interface compartilhada (Tauri, 08/10/2026)

O frontend existente do FeatherSheet **ainda é Tauri + TypeScript**; o
editor inteiro ainda não foi refeito em Iced. Para não perder os recursos
da planilha durante essa migração, ele usa agora o adaptador web do
template FeatherOffice em `../feather-ui/web/cromo.ts`:

- Barra personalizada com minimizar, maximizar, arrastar e fechar,
  seguindo os controles e o tamanho do FeatherSlide;
- Configurações em **janela nativa própria**, com todas as seções
  existentes, sem reduzir a grade;
- O estado e as operações das configurações permanecem na janela
  principal via `BroadcastChannel` (nenhuma segunda pasta carregada);
- Ajuda (F1) e Monitor (F12) com a mesma barra e janelas independentes;
- Em `npm run dev` no navegador, mantém o painel de configurações
  anterior, sem controles nativos;
- `npm run build` gera também `configuracoes.html`; a checagem está
  em `.github/workflows/feathersheet-ui.yml`.

Para testar no Tauri: `cd feathersheet && npm ci && npm run app:dev`.
A mudança não afeta o motor de células Rust/WASM nem os formatos do arquivo.

A futura GUI totalmente Rust/Iced consumirá diretamente
`feather-ui::cromo` e `feather-ui::janelas`; a versão Tauri usa o
contrato web correspondente até a migração completa.
