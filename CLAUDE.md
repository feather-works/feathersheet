# FeatherSheet — estado do projeto

> **Decisão atual (12/10/2026):** não desenvolver nem manter o frontend TypeScript/Tauri. A interface oficial é Rust/Iced; uma interface web futura utilizará os motores Rust via WebAssembly. A UI TS histórica está em `descarte/feathersheet/legacy-frontend/src/`. Entradas antigas neste documento servem como referência de migração, não como estado vigente.

## v0.1.0 lançada — 17/10/2026

- **Dependências só por tag:** feather-io v0.1.1, feather-charts v0.3.1,
  feather-ui v0.2.9 (mais feather-draw/feather-shapes). Nenhum `[patch]` nem
  caminho `../../../`: o repositório compila sozinho (o job do macOS depende
  disso). Para mexer numa biblioteca junto, `[patch]` local provisório e
  trocar pela tag nova antes de lançar.
- **Medir velocidade:** perfil `medir` na GUI (release sem LTO, incremental:
  ~1,5 min por mudança contra ~6 do release). Bancada da dinâmica:
  `FEATHERSHEET_PESADO=…_1M.fshz FEATHERSHEET_ABA=1 FEATHERSHEET_CAMPOS="2,3;6"
  cargo test --profile medir --bin feathersheet medir_etapas_da_troca -- --ignored --nocapture`
  (`FEATHERSHEET_COMPACTO=1` mede o caminho da árvore de grupos).
- **Dinâmica:** o caminho rápido (`pivot::montar_dinamica_com_chaves`) agrupa
  por códigos e devolve as chaves; `montar_com_extras` monta `caminhos` nos
  dois caminhos; Detalhes usa o atalho guardado (`avancado::caminhos_guardados`)
  e grava a aba por coluna. Nasce vazia; origem > 100 mil linhas adia a conta
  para a barra mostrar "Atualizando…". F12 anota o tempo de cada ação.
- **Colar até o fim:** valor simples em ≥ 100 mil linhas vai em blocos
  (`Sheet::preencher_faixa_vazia`, Ctrl+Z por `Passo::Divisao`); fórmula,
  data, estilo, validação e bloco de várias linhas ainda param na última
  linha com dados.

## Interface no pelicano (22/10/2026); Iced aposentado em 26/10/2026

- `crates/feathersheet` (binário `feathersheet`; até 26/10/2026 se chamava `feathersheet-pelicano`) é a interface, portada
  1 pra 1 do antigo `feathersheet-gui` (Iced) para o pelicano v0.1.11 + feather-ui v0.3.2
  (por git + tag). `pacote/gerar.sh` e o CI do macOS montam o pacote a partir dela,
  instalando o binário como `feathersheet`. **O `feathersheet-gui` (Iced) foi apagado em
  26/10/2026** (fica no histórico do git); referências a ele abaixo são registro histórico.
- Pacote: gerar de uma cópia limpa do commit (`git worktree`), porque o crate usa o
  `feathersheet-core` por caminho e pegaria mudança não commitada.
- `src/compat.rs` imita o canvas do Iced 0.13 (`Frame`, `Text`, `Path`, `Stroke`,
  `with_clip`…) e os módulos `keyboard`/`mouse`/`window` sobre o pelicano, para o desenho
  ficar linha a linha igual ao do Iced. `compat::pick_list_vec` é o `pick_list` com a lista
  por valor (o do pelicano só empresta). O `listen_with` do Iced virou `App::evento_janela`.
- Compila (`cargo check` e `check --tests`); **ainda não foi rodado nem comparado na tela**.
  Os limites ficaram como `// shortcut:` (31). Os que pedem versão nova do pelicano: pôr o
  cursor ou selecionar tudo num `text_input` por tarefa (inserir referência na barra fx,
  F4, renomear aba), cor de seleção e modo só leitura no `text_editor`, `align_x` no
  `text_input`, cor do trilho na `progress_bar`, tamanho da letra na dica do `tooltip`.

## Verificação antes do commit geral — 07/10/2026

As alterações existentes de macros foram preservadas no commit geral pedido
pelo usuário. Corrigidos o fechamento da ação de limpar breakpoints e a ordem
de fechamento dos blocos gerados na instrumentação do depurador. O guia da
API agora descreve as propriedades de valor/texto do intervalo e os aliases
de células e acesso a abas. Essas correções estabilizam o trabalho existente;
o foco da suíte continua no FeatherSlide.

**Direção atual — 07/10/2026: motor e interface 100% Rust, com Iced.**
Prioridade: **FeatherSlide → FeatherDocs → FeatherSheet**; FeatherCAD
depois, se houver tempo. O inventário abaixo registra funcionalidades e
decisões anteriores; a migração da interface deste aplicativo ainda deve
ser conferida no código, sem presumir que os recursos já existem em Iced.

**Renomeado de OpenSheet para FeatherSheet em 28/09/2026** (registro completo
em `/CLAUDE.md` da raiz, seção "Renomeação para FeatherOffice"): pasta
`opensheet/` → `feathersheet/`, crates `sheet-core`/`sheet-io`/`sheet-ponte` →
`feathersheet-core`/`feathersheet-io`/`feathersheet-ponte`, extensão `.oplz` →
`.fshz`. Os relatos de sessão abaixo, de antes dessa data, continuam citando
os nomes antigos — são registro histórico, não o estado atual.

## Biblioteca de UI do FeatherOffice (08/10/2026)

O crate compartilhado `/feather-ui/` contém os estilos do FeatherSlide e
componentes Iced independentes de motor. A interface atual do FeatherSheet
ainda é TypeScript/Tauri; não adicionar Iced ao `src-tauri` só para fazer
o link do tema. Quando iniciar `feathersheet-gui` nativo, ele consumirá
`feather-ui` e manterá seu motor de planilhas Rust separado.

## Comandos

```bash
cd crates/feathersheet-core
cargo test       # o motor novo, sozinho (compila em segundos: não tem dependência)
cargo clippy --all-targets
```

## Regras de trabalho neste projeto

1. **Responder em português.** Toda a interface, os comentários e as mensagens
   de erro são em pt-BR.
3. **Nada de item de menu morto.** Se não está implementado, não entra no menu.
8. **Conferir que a função existe** antes de usá-la numa fórmula gerada por
   código. "Cores alternadas" nasceu morto usando `=ÉPAR(LIN())` com a `ÉPAR`
   inexistente: regra que não analisa não pinta e **não dá erro**.
9. O usuário roda Firefox no Linux — atenção a diferenças (área de
   transferência, limite de altura de elemento).
11. **O `sheet-core` não ganha dependência.** Ele compila em segundos porque
    não tem nenhuma, e é isso que faz rodar o teste a cada mudança ser
    barato. O que precisa de crate (Arrow, `zip`, `quick-xml`) vai para o
    `sheet-io` ou para a camada de comandos.
14. **Ao final de cada entrega/tarefa concluída, realizar o commit no Git com o prefixo obrigatorio `gemini: <descrição do que foi feito>`.**
20. **Passo de consolidação depois de mesclar trabalho de agente.** Um
    agente que termina isolado (testes verdes na própria worktree) não viu o
    conjunto: nomes inconsistentes entre módulos parecidos, lógica duplicada
    entre dois agentes que não se enxergaram, doc que um atualizou e outro
    não. Antes de considerar uma leva "pronta", uma revisão dedicada ao
    conjunto mesclado — não mais um teste isolado — checando duplicação,
    inconsistência de nome/padrão e documentação desatualizada.
21. **Modelo de detalhamento, antes de construir qualquer tela nova**
    (28/09/2026, depois de entregar um diálogo genérico demais e fora do
    padrão visual do resto do app):
    1. **Tem referência?** Já existe em algum programa conhecido (Excel,
       Sheets, Word) ou algo parecido dentro do próprio OpenSheet? Se sim,
       mirar nessa referência, não numa leitura mínima do pedido.
    2. **Nível de acabamento**: sempre o padrão de qualidade do que já está
       construído no app. Não é uma escolha por tarefa — não existe "faz
       simples por agora".
    3. **Buscar semelhante antes de construir**: olhar o que já existe de
       parecido — dentro do app ou fora (Excel, Sheets, outro programa
       conhecido) — para trazer ideia a mais, não só atender o pedido ao pé
       da letra.
    4. **O que reaproveitar do app**: qual o diálogo/painel mais parecido
       que já existe, para puxar o mesmo padrão visual em vez de inventar
       um novo do zero.
    5. **Descrever antes de construir**: 2-3 frases do que vai ser feito,
       o usuário confirma ou ajusta, só depois vem o código.

23. **Bug reportado vira issue** (19/10/2026) em `feather-works/feathersheet`
    (`gh issue create`, label `bug`/`enhancement`). A correção fica com a issue
    aberta até o commit (`Fixes #N`). **Versões só em lote**; correção avulsa
    só quando algo crasha.

## Editor de macros (Alt+F11) — 05/10/2026

- `sheet.showComponent("nome", dados?)` abre o componente num iframe
  `sandbox="allow-scripts"`; `data-macro="modulo"` roda o módulo ao clicar
  (campos com `name` chegam em `valores`), `data-fechar` fecha.
- Também no editor: seletores Objeto/Procedimento acima do código,
  Propriedades em tabela (alfabética/por categoria), Janela de Inspeção
  (avalia expressões na planilha), Depurar → Verificar sintaxe, Ferramentas →
  Opções do editor. **Não existem (precisam de decisão):** pausar/parar,
  passo a passo, breakpoints, Locais e Pilha de chamadas — exigem controlar a
  execução do JavaScript.

## Decisões que não são óbvias

- **Layout em coluna flexível**, não grid: com `grid-template-rows` seria
  preciso uma faixa por filho, e acrescentar uma barra roubava a altura da
  grade. Só `.grid-host` cresce. Há teste travando isso.
- **CSS injetado por JavaScript** (`injectStyles`): um `.css` separado depende
  do bundler entregar o arquivo, e na máquina do usuário isso falhou — a página
  aparecia crua.
- **Campo invisível `.grid-keyboard`** recebe o foco: o Firefox só dispara
  copiar/colar em elemento editável. O `mousedown` da grade faz
  `preventDefault()` para não perder esse foco (era a causa das setas paradas).
- **Recálculo preguiçoso** com propagação de "sujo" e poda; fórmulas de matriz
  são a exceção: precisam ser avaliadas na frente (`arrayCells`), porque uma
  célula vizinha pode ser lida antes da âncora. Mas só as que têm motivo —
  sujas, em `#SPILL!`, ou com a área derramada mexida (`spillNeedsReview`).
  Recalcular todas a cada tecla custava 8,7 ms na pasta de exemplo, contra
  0,7 ms agora.
- **Fórmula não guarda árvore nem texto** (19/09/2026): uma coluna preenchida
  para baixo é *um* padrão, não um milhão de fórmulas. O que fica por célula
  são 12 bytes — número do padrão e valor calculado. A árvore é solta assim
  que a célula calcula e reparseia sob demanda; o texto se reconstrói do
  padrão. Detalhe e números em `docs/planejamento/ROADMAP.md` → "Fórmula por padrão".
- **O grafo de dependências das fórmulas colunares não existe**: é deduzido.
  Se a coluna D lê "mesma linha, duas colunas à esquerda", quem lê B7 é D7 —
  uma subtração (`stepPatternDependents`). Eram 309 MB de `Set` e array.
- **Eixos com padrão + exceções**: 1.048.576 linhas sem alocar nada por linha.
- **Escala da barra de rolagem**: acima de 15 milhões de px o navegador trunca
  o elemento, então a barra passa a trabalhar em escala e a roda do mouse rola
  em pixels reais.
- **Referências entre abas** são revalidadas a cada mudança (são poucas), em
  vez de entrar no grafo de dependências.
- **Vírgula é decimal** quando está entre dígitos (`0,5`); fora disso separa
  argumentos. `;` é o separador recomendado.
- **O Rust escreve número como o JavaScript escrevia** (`crates/sheet-core/
  src/number.rs`, 21/09/2026): quem decidia como um número aparece na célula
  era o `String(n)`, e isso está em planilha que já existe no disco de quem
  usa. Trocar pelo `{}` do Rust mudaria resultado calado — `1e21` viraria
  `1000000000000000000000` e `0,0000001` viraria `0.0000001`. Então a regra do
  ECMAScript veio junto: `to_js_string` escolhe entre notação comum e
  científica nos mesmos limites (abaixo de 1e-6, a partir de 1e21), e
  `from_js_string` recusa o que o `Number()` recusava e o `parse` do Rust
  aceitaria (`inf`, `nan`, `1_0`).

## O que já funciona

Motor: fórmulas com todos os operadores, ~130 funções (nomes em inglês e
português), referências entre abas (`Dados!A1`, `'Meu Plano'!B2`), intervalos
nomeados, detecção de referência circular, funções voláteis, **matrizes
dinâmicas com derramamento** (FILTRO, ÚNICO, CLASSIFICAR, TRANSPOR, SEQUÊNCIA,
#SPILL!), literais de matriz (`{1;2;3}`), trigonometria, `LIN()`/`COL()` sem
argumento, álgebra de matrizes (MATRIZ.MULT,
MATRIZ.INVERSO, MATRIZ.DETERM), estatística completa (família SES, correlação, tendência, percentis),
datas de verdade (`DATADIF`, data digitada vira serial com formato).

Interface: 8 menus com submenus e ícones, barra de ferramentas completa com
busca de comandos, menu de botão direito por alvo, abas, zoom, congelar
painéis, bordas, formatação de texto, alça de preenchimento, seleção de
referência pelas setas (modo Excel), macros em JavaScript, `.plz`, `.csv`,
`.json`, tema claro/escuro, **analisador de erro de fórmula** (aviso no rodapé
e "Explicar este erro" no botão direito), **gráficos** (17 tipos, camada
flutuante sobre a grade, editor lateral) e **formatação condicional** (20
operadores, fórmula personalizada, escala de cor e barra de dados).

**Documentos (19/09/2026):** página inicial com recentes, arquivos `.plz` de
verdade no disco, salvamento automático com tempo escolhível, recuperação
depois de travar, `Ctrl+S` / `Ctrl+Shift+S`, configurações em modal central
(tema, 6 esquemas de cor, cor de destaque, letra da grade, zoom, salvamento,
planilha), confirmações em modal próprio (nada de `confirm()` do navegador).

Inventário completo, item a item: **`docs/planejamento/ROADMAP.md`**.

## Fila de trabalho

### Referência entre arquivos abertos (v0.1.0, 16/10/2026) — feita e testada com duas janelas

`=[Vendas.fshz]Dados!A1`, `=SOMA([Vendas.fshz]Dados!A1:A10)` e
`='/home/x/[Vendas.fshz]Dados'!A1`. Motor em `planilha/vinculo.rs` e
`planilha/workbook/vinculos.rs` (mapa de valores externos na `Workbook`,
recálculo só de quem lê a célula que mudou); valores guardados no `.fshz`
(`ex`, `feathersheet-io/src/externos_guardados.rs`, contrato no `FORMATO.md`
da suíte); canal entre janelas, faixa "Valores de […] de …" e Editar links
em `feathersheet-gui/src/app/vinculos*`. **Para depois:** autocompletar o
nome do arquivo na fórmula; "clicar na outra janela para apontar" a
referência. **No `.xlsx` (19/10/2026):** `xlsx_write_vinculos.rs` grava `[N]Aba!A1` +
`xl/externalLinks/` com os valores guardados; `xlsx_read/vinculos.rs` lê de volta pelo
`trocar_origem_externa` (só o leitor nativo; o `ler_xlsx_bytes` em JSON não).

### Bugs e melhorias anotados em 19/10/2026 (montando o "Painel de Vendas" pelo MCP ao vivo)

**Pedidos do usuário:** feitos em 26/10/2026 — formatação condicional em janela própria
(`Auxiliar::Condicional`, aberta e fechada pelo `condicional_aberto` no `update` do `pelicano::App`),
cor só pela amostra, campo "Aplica-se a" com "Selecionar" pela grade e filtro "Esta planilha /
Seleção atual". O texto "em breve" do seletor de gráfico já tinha saído.

**Motor (achados pelo MCP):** resolvidos — nome de outra aba em `SOMASES`/`CORRESP` e
`SOMARPRODUTO` com matriz por dentro (testes no `1577c18`); `toLocaleString` nas macros (#7).

**Dificuldades com o MCP:**
1. Gráfico: `sheet_grafico` (26/10/2026) cria, edita e remove. Antes não havia jeito (nem `sheet_comando`, nem API das
   macros). O menu "Gráfico…" só abre o seletor, que pede clique.
2. Validação de dados: `sheet_validacao`; botão de macro: `sheet_botao`; formulário: `sheet_formulario` (26/10/2026).

**Formulários das macros (26/10/2026, o UserForm do VBA):** modelo em `feathersheet-macros/src/formularios.rs`
(10 tipos de controle, guardados em `Workbook::formularios` = `fm` no `.fshz`); `sheet.showComponent("nome", dados)`
abre a janela `Auxiliar::Formulario` (`app/formulario_macro.rs`); o botão roda a macro dele com `valores`
(`Contexto::valores`), e pode limpar/fechar. Designer no Alt+F11 (`app/macros/designer_formulario.rs`: caixa de
ferramentas, arrastar/redimensionar com grade de 4 px, propriedades, Guardar/Testar). A janela soma a barra de título
própria (`cromo::BARRA_ALTURA`) ao tamanho. Na macro, `sheet.field("nome").value/enabled/visible`
mexe no formulário aberto e `sheet.hideComponent()` fecha (`Resultado::campos`/`fechar_formulario`, aplicados depois
da execução); `macro` num campo roda ao mudar (texto/número: no Enter); botão `padrao` (Enter) e `cancelar` (Esc);
`desativado` esmaece, `oculto` some. Tipo `intervalo` = RefEdit (botão pega a seleção, valor `Aba!A1:B5`); lista com `fonte` = RowSource (lida ao abrir
pelo `opcoes_da_lista` da validação). `listagem` = ListBox (`multipla`: caixas de seleção, valor = lista de textos;
uma escolha: item em destaque sólido); lista com `livre` = ComboBox (campo + setinha). `sheet_tela` descreve o formulário aberto. **Falta para a paridade com o
UserForm** (levantamento de 26/10): ordem de tabulação; importar `.frm` solto. **UserForm do `.xlsm` (#67, 26/10):** `feathersheet-vba/src/formulario.rs` lê o desenho (fluxos `f`/`o` do MS-OFORMS, molduras em `i<ID>`) e o gerador converte o código do formulário: `UserForm1.Show` → `showComponent`, `TextBox1.Value` → `sheet.field`, `Unload Me` → `hideComponent`, `Controle_Click/_Change` → macro do controle (`UserForm1.função`), `AddItem`/valores fixos do `UserForm_Initialize` → opções e valores do desenho; `importar_vba.rs::formulario_da_conversao` grava em `Workbook::formularios`. Testado com os UserForms dos arquivos de teste do LibreOffice (`CheckOptionToggleValue.xls`, `MiscControlTests.xls`); falta um `.xlsm` real do usuário. **Aparência (26/10):** `fundo`/`cor`/`borda`
(#rrggbb), `raio`, `letra`, `negrito`/`italico`, `familia`, `alinhar` por controle e `fundo` do formulário
(`formulario_macro::aparencia`, usado pela janela e pela prévia do designer: painel APARÊNCIA com paleta); `max`,
`multilinha` (`text_editor` sobre `FormularioAberto::editores`), `dica` (balão); tipos `moldura` (arrastar no designer
leva junto o que está dentro) e `imagem` (base64; foto acima de 1,4 MB vira JPEG de 1024 px); `field(n).backColor/
foreColor/caption`. **Várias funções num módulo (26/10):** `Módulo.função` (ex. `macro_1.gravar`) roda o
módulo e chama só a função, como as Sub do VBA (`feathersheet_macros::procedimentos::codigo_da_macro`; botão,
forma, formulário e `sheet_macro executar`); as listas de escolha mostram cada módulo e cada função (`executaveis`).
3. Congelar painéis: `sheet_comando congelar {linhas, colunas}` (26/10/2026); os rótulos dos menus
   saem do `sheet_comando menus {filtro}`.
4. `sheet_tela` **não mostra os painéis abertos** (seletor de gráfico, diálogo de formatação
   condicional) — só descobri pela foto do `sheet_ver`.
5. `sheet_condicional` ao vivo **abriu o diálogo** de formatação condicional na tela.
6. Parâmetros inconsistentes entre ferramentas: `sheet_filtrar` só aceita letra em `coluna` e o
   erro fala em "Cada chave precisa de `coluna`" (mensagem do ordenar); `sheet_estatistica`
   recusa `coluna: "G"` (quer `G:G`); `sheet_substituir` usa `texto`/`por`; `sheet_consulta`
   descreve os passos como `agrupar{…}` mas exige `{"tipo":"agrupar",…}`.
7. Claude Code só carrega as ferramentas `sheet_*` numa sessão nova depois do `claude mcp add`;
   nesta sessão o MCP foi usado por stdio com um script (JSON-RPC à mão).

### Conversor de macro VBA → JavaScript (pedido do usuário, 07/10/2026) — motor pronto, falta a interface

Rascunho original (07/10, antes de começar):

Quem vem do Excel traz macro em VBA (`.xlsm`/`.xlsb`); as macros do FeatherSheet são
JavaScript com a API `sheet` em inglês (seção "API das macros em inglês" abaixo). O
conversor leva o VBA para essa API. Rascunho do desenho, para confirmar ao começar:

- **No Rust** (regra 2: tradução é lógica, não desenho), num crate próprio — candidato a
  comum se o FeatherDocs um dia importar macro do Word.
- **De onde vem o VBA:** o `xl/vbaProject.bin` do `.xlsm` é um arquivo composto OLE (CFB)
  com os módulos comprimidos (MS-OVBA, compressão RLE própria). Ler em Rust puro (o crate
  `cfb` é Rust puro; a descompressão do MS-OVBA é pequena, escrever à mão). Também aceitar
  o VBA colado como texto e os `.bas`/`.cls`/`.frm` exportados do editor do Excel.
- **O que traduzir primeiro (o que a macro gravada e a do dia a dia usam):** `Sub`/
  `Function`, `Dim`, `If`/`Select Case`, `For`/`For Each`/`Do While`, `Range("A1").Value`,
  `Cells(l, c)`, `Worksheets("x")`/`Sheets("x").Activate` → `goToSheet`, `.Formula`,
  `ClearContents` → `clear`, `End(xlUp).Row` → `lastRow`, `MsgBox`, `InputBox`,
  `Application.ScreenUpdating` (ignorar), `Worksheet_Change` → evento *Ao mudar células*.
  **UserForm (`.frm`) → componente HTML+CSS** (o editor já tem o designer; gerar pelo
  desenho dele).
- **O que não der para traduzir não some calado:** vira comentário `// VBA não convertido:`
  com a linha original, e o conversor devolve a lista do que ficou para trás (API do
  Windows, `CreateObject`, Outlook, `Declare`, acesso a arquivo). O resultado abre no
  editor (Alt+F11) para a pessoa revisar — nunca roda sozinho na conversão.
- **Liga com o MCP abaixo:** o conversor faz a parte mecânica; o que ficar marcado como
  não convertido a IA pode terminar pelo MCP.
- Testes com macros VBA reais (pedir ao usuário um `.xlsm` dele) — sintética esconde o
  que pega (regra "medir com os arquivos do usuário").

### MCP do FeatherSheet — feito em 14/10/2026 (`crates/feathersheet-mcp`)

Ferramentas `sheet_*` (resumo, ler com teto de 2000 células, escrever/preencher fórmula com um
Ctrl+Z por chamada, limpar, abas, nomes, desfazer, fórmula/auditoria, estatística, amostra,
localizar, consulta OpenQuery, macros, abrir/salvar) sobre o trait `Hospede`: `Avulso` (arquivo,
sem o app) ou `HospedeApp` (`feathersheet-gui/src/app/mcp_vivo.rs`, ao vivo). Ao vivo o app escuta
em `$XDG_RUNTIME_DIR/feathersheet/mcp.sock`; a thread do canal entrega o pedido ao `update` por
`Subscription::run` e espera a resposta. Só ao vivo: `sheet_selecionar`, `sheet_comando` (faixa,
tecla de atalho, menu por rótulo, editar pela barra fx, comandos da aba Fórmulas…), `sheet_tela`
(estado em texto) e `sheet_ver` (PNG da janela). `sheet_mouse` (26/10/2026, pelicano v0.1.11 + feather-ui v0.3.2) manda um roteiro do pelicano
(`mover x y`/`mover "rótulo"`, `clicar`, `duplo`, `apertar`/`soltar`, `digitar`, `tecla`, `rolar`, `esperar`)
para a janela principal ou uma auxiliar pelo `WindowCmd::Roteiro`: o cursor "Claude" anda e clica de verdade;
a resposta volta antes de o roteiro terminar. Rótulos da faixa são achados pelo nome; teclas F1–F12 ainda não. `FEATHERSHEET_MCP=0` desliga o canal; o modo de
captura não o abre. Registrado no Claude Code com `claude mcp add feathersheet --scope local`
apontando para `target/release/feathersheet-mcp`; **mudou ferramenta, recompilar o MCP e o app**
(o app embute a mesma biblioteca). Dívida (regra 3 da raiz): `ao_vivo.rs` é cópia adaptada do
`feathercad-mcp`; o transporte deve virar crate comum da suíte.

**A FAZER — "cursor do Claude" ao vivo (ideia do usuário, 19/10/2026; desenho proposto, falta
confirmar antes de construir):** mostrar na tela o que a IA está fazendo pelo MCP. Referência:
cursor de colaborador do Google Sheets/Figma. O `mcp_vivo.rs` já recebe cada pedido no `update`,
então o app sabe a ferramenta e o intervalo na hora.
1. Borda colorida (cor própria, ex. laranja) no intervalo que a IA mexe, com etiqueta "Claude"
   no canto; anda a cada pedido.
2. Balão de uma linha ao lado da etiqueta ("Escrevendo fórmula em C2:C200", "Rodando a macro
   Relatório"), some alguns segundos depois do último pedido.
3. `sheet_comando` (faixa/menu): o botão acionado pisca rapidamente.
4. Histórico das ações da IA no Console F12 (hora, ação, intervalo) — cada uma é um Ctrl+Z.
Fica de fora (proposto): seta de mouse animada voando pela tela — a IA não move o mouse, e
fingir só atrasaria e cobriria a grade. Perguntado ao usuário se quer mesmo a seta; sem resposta.

Rascunho original (07/10, antes de começar):

O usuário perguntou se o Sheet era o único app em que MCP não fazia sentido; a conclusão
(aprovada para anotar) é que **vale a pena**, mas desenhado diferente do MCP do FeatherCAD
e do FeatherSlide (`feathercad/crates/feathercad-mcp`, modelo para o resto):

- **Por que vale:** é onde mais se pede ajuda a uma IA (fórmula, duplicados, tabela
  dinâmica, "por que dá #N/D"); o OpenQuery combina — a IA monta os passos
  (filtrar/agrupar/mesclar) e a pessoa vê e edita depois; e é a peça que liga a suíte:
  fórmula e processamento só no FeatherSheet (decisão de 07/10, ver
  `featherslide/docs/arquitetura/ARQUITETURA.md`, "Editar os dados de gráfico e tabela"), então
  "atualiza o gráfico do slide com outubro" precisa do MCP do Sheet para calcular.
- **Volume — a regra central:** nunca despejar a planilha para a IA (a `planilha_2m` tem
  2 milhões de linhas). O MCP entrega estrutura (abas, colunas, tipos, nº de linhas),
  **amostra** e **estatística de coluna**; filtrar/agrupar/somar é do motor Rust, e a IA
  recebe só o resultado. O MCP do CAD manda o desenho inteiro — aqui isso não serve.
- **Privacidade (regra 5):** planilha costuma ter dado sensível. O app não manda nada; quem
  liga a IA é a pessoa. Mas o padrão é mandar resumo, e linhas de verdade só quando a IA
  pedir amostra explicitamente.
- **Desfazer:** cada ferramenta da IA = um Ctrl+Z, como no CAD (fórmula arrastada errada
  estraga muito sem ninguém ver).
- **O MCP também escreve macro** (o usuário apontou, 07/10): "faça uma macro que, ao
  clicar no botão, abre um formulário de cadastro e insere os dados em outra aba". Macro e
  MCP não são caminhos separados — o MCP é quem *gera* a automação que fica no arquivo e
  roda sem a IA depois. As peças já existem no editor (seção "Editor de macros" acima):
  módulo JS, componente HTML+CSS (o formulário; `data-macro`, campos com `name` →
  `valores`, `data-limpar` para cadastro em série), botão de macro, e o designer já sabe
  "Criar o módulo que grava na planilha". Ferramentas que o MCP precisa: listar/ler/
  escrever módulo, escrever componente (de preferência pelo **desenho** do designer, que
  gera HTML/CSS — menos erro que HTML solto), criar o botão ligado à macro, rodar a macro
  e ler o log/erro (para a IA corrigir sozinha). Pontos práticos:
  - **onde as macros moram:** hoje em `localStorage` (`office.macros.v1`,
    `office.componentes.v1`) — o MCP isolado (sem o app) não alcança; ou se resolve antes
    a pendência de levar macros/componentes para dentro do `.fshz` (decisão de formato,
    já anotada acima), ou essa parte só funciona **ao vivo**, com o app aberto;
  - **código que a IA escreveu roda na máquina da pessoa:** a macro gerada aparece no
    editor (Alt+F11) para ser lida, e não roda sozinha na criação — roda quando a pessoa
    clicar no botão (ou quando a IA pedir "rodar" e o app confirmar com a pessoa).
- **Macro/Python × MCP pedindo direto:** a IA pode fazer a tarefa uma vez (escrever nas
  células) ou deixar uma macro para repetir — perguntar qual a pessoa quer quando não
  estiver claro.

### Idiomas: migração do texto fixo (começou em 28/09/2026)

Pedido do usuário: o app inteiro em N línguas ("o aplicativo todo está em
pt-BR hardcode"). Formato: JSON, um arquivo por língua (regra 22). Fases,
um commit cada, o app funcionando entre elas:

4. Barras, painéis e caixas de diálogo (`src/ui/**`).
5. Mensagens que saem do Rust (erros, `explicar_erro`) e as descrições das
   funções da planilha (`function_help.rs`), hoje só em português.
6. Os nomes das funções de fórmula por língua (hoje: inglês + apelidos em
   português) — é motor, decidir com o usuário antes.

### A FAZER: abrir CSV grande ainda é lento (anotado em 28/09/2026)

Pedido do usuário: melhorar a leitura do CSV. **No app**, 1 milhão de linhas
levou ~74 s (log de 19:42 → 19:43; `vinculos_atualizar` esperando ~1,5 s
pela ponte a cada lote). Já consertado e commitado: a janela não congela
mais (`pasta://` assíncrono, a ponte solta entre lotes — `5411fe6`).

Medido fora do app com CSV sintético de 400 MB (3 M linhas × 20 colunas,
`crates/feathersheet-ponte/examples/medir_csv_partes.rs` no ramo abaixo):
ler 0,3 s, cortar 3,2 s, **colar 36 s** (~600 ns por célula no
`Sheet::paste`, `edicao.rs`), pico 1,2 GB. A primeira rodada de otimização
está no ramo **`csv-rapido`** (commit PARADO, sem testes): colar 36 → 24 s,
pulando mapas vazios no `soltar_para_literal` (~12 hashes SipHash por
célula à toa) e analisando cada literal uma vez só. Falta testar (colar
com data, %, moeda, " 5", "true", "="), medir no app, e atacar os 24 s que
sobram — candidatos: o `pool` do texto, o `set_cell_value` por célula,
colar por coluna em vez de célula a célula, e cortar/colar em paralelo
(cortar o lote seguinte enquanto o anterior cola).

### API das macros em inglês + guia por localidade (28/09/2026)

Decisão do usuário: **os nomes das funções das macros são em inglês** (fixos,
são código) e **o manual segue a localidade do app** (pt-BR / en-US). O objeto
é `sheet` (o `planilha` continua existindo como o mesmo objeto, mas os métodos
em português saíram, sem apelido: `value`, `formula`, `set`, `clear`,
`style`, `range`, `selection`, `select`, `lastRow`, `lastColumn`, `address`,
`defineFunction`, `log`, `status`, `name`, `sheets`, `goToSheet`; no
intervalo, `values`, `formulas`, `set`, `forEach`, `map`, `style`, `clear`,
`address`, `rows`, `columns`). Macro gravada com os nomes antigos precisa ser
reescrita. `goToSheet` é novo (pedido: botão de macro que navega entre abas).

### Botão de gatilho de macro — primeira fatia do item 5 (28/09/2026)

O item 5 da fila ("em Inserir faltam botões: gatilho de macro, e card de HTML
com macro; modo grid finito") tinha três pedaços. Este trabalho fez só o
**botão de gatilho de macro** — os outros dois (card de HTML personalizável,
modo grid finito para relatório) continuam pendentes, sem começar.

Menu Inserir → "Botão de macro…": cria um botão flutuante na grade (padrão do
Excel/Sheets, e o começo do UserForm do VBA, item 7 do `ROADMAP.md`). Clique
roda a macro escolhida; arrastar move o botão (o gesto decide pelo
deslocamento acumulado, não por um modo separado); alças de 8 pontos
redimensionam quando selecionado (botão direito → "Editar botão…" também
seleciona, para dar acesso às alças sem rodar a macro à toa).

- **Rust**: `crates/feathersheet-core/src/planilha/sheet.rs` (campo `bt`) e
  os pontos de round-trip que todo campo opaco precisa tocar —
  `feathersheet-ponte/src/lib.rs` (`metadados_para_json`/`definir_metadados`)
  e `feathersheet-io/src/{fshz_read,fshz_write,da_workbook,para_workbook,
  enxugar}.rs`. O `enxugar.rs` é o ponto crítico: `deny_unknown_fields` faz
  qualquer campo novo esquecido ali falhar alto ao abrir um arquivo real (é o
  comportamento de propósito, para não perder dado calado) — `bt` foi
  acrescentado lá também, com teste (`o_resto_dos_metadados_sobrevive`).

### Sessão de 23/09/2026 — a ponte revisada, e o `.plz` passa a morar no Rust

2. **`da_workbook` não gravava a coluna de fórmula empacotada**
   (`colunas_formula: vec![]`): abrir e salvar apagava as fórmulas.

### Abrir CSV grande: 8 GB -> ~1,8 GB (23/09/2026, noite)

Um CSV de 800 MB estourou 8 GB ao abrir. Duas causas, as duas no Rust:
o `dividir_csv` montava o arquivo inteiro num `Vec<Vec<String>>` antes de
colar (cada célula um `String`, 3 a 5 GB), e o colar rápido do `edicao.rs`
sujava CADA literal (`mark_dirty_cascade` duas vezes por célula, o conjunto
`dirty` com uma chave por célula). Agora corta e cola em lotes de 50 mil
linhas (`csv::dividir_csv_em_lotes`, `Ponte::inserir_aba_de_csv`) e só suja
quando existe fórmula que leia a célula. Medido com um CSV sintético de
200 MB (1,3 M linhas × 20 colunas, `crates/sheet-ponte/examples/medir_csv.rs`
com `/usr/bin/time -v`): 21,7 s e 2,29 GB de pico -> 8,8 s e 454 MB. Ainda não
medido no app com o arquivo do usuário. O texto do arquivo inteiro ainda fica
na memória durante a abertura (ler em fluxo seria o próximo passo).

### Migração, passo 1 — pronto (23/09/2026, noite)

Ordem combinada com o usuário (regra 15): 1) o que agarra vira comando Rust;
2) formatação condicional no Rust; 3) `PlanilhaRust` deixa de herdar do
`Sheet`; 4) `src/io/` para o `sheet-io`; 5) apagar o `src/core/`.

- a busca vê a coluna de fórmulas empacotadas;
- número escrito como o JS;
- substituir com desfazer;
- `replace_every` sem pânico.

`fill.rs` ganhou estilo, série de 12 algarismos e `Sheet::preencher`.
`regiao_da_tabela` (Ctrl+T, filtro, dinâmica). Ponte: `localizar`,
`substituir`, `estatisticas_coluna`, `regiao_atual`, `regiao_da_tabela`,
`autosoma`, `preencher`. **Na interface:**
- `grid.applyFill`, `app.autoSoma` e `app-dialogs.showColumnStats` chamam o
  Rust;
- o nome das séries no editor do gráfico vem do `grafico_dados`.

Todos com `sheet instanceof PlanilhaRust ? rust : antigo` até o passo 3.
Testes de interface novos: localizar/substituir, alça + estatísticas.

- a fórmula personalizada caía em `_ => false`, e as "Cores alternadas" nunca
  pintavam;
- o valor da regra era lido com o `parse` do Rust, e "1,5" e data não
  comparavam;
- texto vazio não contava como vazio.

`Sheet::avaliar_em`/`Workbook::avaliar_em` são o `evalAt`. A ponte tem o
`condicional_bloco` (retângulo visível → `[[l, c, formato]]`), com as
estatísticas em cache enquanto as regras e as versões das abas não mudam.
`PlanilhaRust.formatosCondicionais` manda as regras antes quando elas mudaram
(compara o JSON). A grade usa isso com a planilha no Rust. Medido em 1,3 M
linhas (`examples/medir_condicional.rs`): 2,8 ms por quadro, ~50 ms quando
uma edição refaz a escala. `serde_json` entrou no `sheet-core` só como
`dev-dependency`, para os testes montarem regra em JSON.

**Passo 3, terceira parte — pronto (23/09/2026, noite): a `PlanilhaRust`
não herda mais do `Sheet`.** Duas bases novas em `src/modelo/` (fora do
`core`, porque ficam depois do passo 5):

**Busca e ordenar na planilha de 2 milhões (23/09/2026, noite)** — relato do
usuário: "busca muito lerda", e ordenar pelo menu do filtro travou o app.
Medido com `examples/medir_busca_ordenar.rs` no `planilha_2m.plz`:

- **Localizar: 4 s -> 0,09 s** (texto) e **0,23 s** (número) na `planilha_2m`;
  no `spotify_full.plz` do usuário (1,18 M × 82, ~97 M células), "s" em
  fórmulas 18 s -> 0,64 s e "123" 13 s -> 1,4 s. O "Próxima" seguinte, 0 ms
  (a ponte guarda a última lista por aba e versão). Cada texto distinto do
  pool é conferido uma vez; a fórmula já calculada entrega o resultado cru
  (`FormulaTable::cru_at`); número só é escrito quando a consulta poderia
  bater num número; e as colunas são varridas em paralelo, uma thread por
  núcleo (`ops.rs::por_coluna` — só `std::thread::scope`, sem dependência;
  no `.wasm` roda em série). As threads só leem `DataTable`, `FormulaTable`,
  pool e `textos`: o `Sheet` tem o cálculo num `RefCell` e não atravessa
  thread, então a fórmula suja é calculada depois, em série.

**Próximo: passo 4, o resto** (`src/io/` para o `sheet-io`).

5. macros (projeto próprio no ROADMAP: "Macros executadas no Rust");

**Alça e tabela — prontos (27/09/2026)**, eram os itens 2 e 3 desta lista:

**Mover célula arrastando pela borda (24/09/2026, só no Rust).**
`crates/sheet-core/src/planilha/mover.rs` (a regra de reescrita, sensível a
intervalo) e `Workbook::mover_intervalo`; comando `mover_intervalo` na ponte
(sem `substituir`, destino com dado responde `ocupado` e a grade pergunta);
na grade, `hitBordaDaSelecao`/`concluirMover` e o arrasto `move-block`. As
fórmulas da pasta que liam o bloco acompanham, a referência ao destino
sobrescrito vira `#REF!`, e intervalo só anda inteiro (mover `A1` não
transforma `A1:A10` em `C1:A10` — o Excel estica, aqui fica). Um Ctrl+Z na
aba mexida. **Falta:** Ctrl+arrastar para copiar; mesclas, tabelas,
gráficos e regras não acompanham o bloco.

### Fila pedida pelo usuário em 23/09/2026 (tarde) — a fazer

Na numeração e nas palavras dele. Tudo testado na `planilha_2m` (2 milhões
de linhas) — "estou testando a ferramenta mesmo".

- [x] **(1) Tabela dinâmica ainda demora para pôr os valores** (23/09: agrupamento numa passada, 555 → 107 ms em 500 mil linhas — medir no app) com 2 milhões:
  "eu gostaria que fosse um pouco mais rápido na hora de remover e adicionar".
- [x] **(2) "Dá para melhorar mais ainda"** (ver 1) — a referência é o gráfico: "a
  parte de gráfico está quase instantânea, as trocas". A dinâmica tem que
  chegar lá.
- [x] **(4) Bolinha de loading na barra inferior** (23/09: bolinha girando; acende no filtro, na dinâmica e no salvamento automático) "em coisas que forem demorar
  muito mesmo".
- [x] **(5) Ajuste de visualização: os nomes das visões no menu estão comidos** (resolvido em 23/09: nome dos grupos da faixa)
  — "eu consigo ver OpenQuery e Tabela dinâmica, Consulta, mas em outras está
  comido".
- [x] **(6) Botão direito na coluna ou na linha: opção de ocultar** (23/09: Ocultar e Reexibir) (e
  reexibir).
- [x] **(7) Lag ao selecionar várias colunas arrastando** (resolvido em 23/09: a soma do rodapé espera o gesto parar) (de G até K, por
  exemplo): "ele força ir visualmente até os 10 milhões lá embaixo; os 10
  milhões no frontend não deveriam acontecer, só no backend, que é mais rápido;
  forçar isso graficamente é perda de desempenho".
- [x] **(8) Ainda há lag para trocar de aba quando a aba é muito pesada** (23/09: o filtro era reaplicado a cada troca e o eixo das linhas remontado a cada redesenho — medir no app):
  "forçar 10 milhões de dados é ineficiente; temos que mandar pro visual uma
  visão resumida — a tela vê 20, a gente faz +30 para conseguir ficar
  carregando — mas se eu quero somar tudo vai naturalmente, como se a gente
  tivesse carregado tudo; é só o visual que está limitado, o backend vai estar
  com tudo na memória e conseguindo fazer sem travar".
- [x] **(9) Formatação condicional** (resolvido em 23/09): "a parte de aplicar o intervalo está
  bugada"; "quando seleciona a coluna toda tem que mostrar só as letras"
  (`A:A`); "essa pré-seleção rápida tem que estar num dropdown, tipo para
  pintar as linhas, as células, aparecer as opções; ou criar uma regra
  específica; e gerenciar as regras da planilha; e a última, gerenciar regras
  da seleção".
- [x] **(10) Em Exibir, uma forma de tirar a formatação de cor alternada das
  células**, igual à linha de grade (liga/desliga).
- [x] **(11) Colar imagem via Ctrl+V não está funcionando mais.** (23/09: imagem não ia para o PLZ2; texto junto ganhava da imagem — conferir no app)
- [x] **(12) Fórmulas de imagem e base64** (23/09: =IMAGEM, =IMAGEM.BASE64, imagem na célula, botão direito na imagem): uma para converter imagem para
  base64, e vice-versa; uma para gerar imagem a partir de base64.
- [x] **(13) Acrescentar mais atalhos** (de teclado). (23/09: 17 atalhos + F4 na fórmula)
- [x] **(14) Juntar o monitor de memória com o que o F12 mostra** ("o atalho do
  F12 de ver as coisas") numa **página nova, igual ao OpenQuery**. (24/09:
  página Monitor, `monitor.html` + `src/monitor/`, janela própria pelo
  `abrir_janela_monitor`; abas Memória, Console e Métricas — o `[motor]` do
  Rust entra pelo `metricas_do_motor`. F12, Ctrl+Shift+M e o menu abrem ela
  no app; no navegador ficam os painéis de antes.)
- [ ] **(17) Em Inserir faltam botões**: para gatilho de macro, e para inserir
  card de HTML para personalização, que dê para usar macro também.
- [ ] **(18) Ativar um modo grid com página finita** para criação de
  relatório, com grade e guias de alinhamento.

- [ ] **(21) Macros executadas no Rust** (ideia do usuário, 24/09/2026 —
  só anotada): o JS da macro analisado em Rust, loops de coluna viram
  operação vetorizada, o resto num motor embutido (Boa/QuickJS) ou
  bytecode próprio. Detalhe e notas em `docs/planejamento/ROADMAP.md` → "Macros executadas
  no Rust".

- [ ] **(22) A "planilha analítica"** (ideias do usuário, 24/09/2026 — só
  anotadas): fórmula `=SQL(...)` com engine embutida, histórico/ramificações
  por Copy-on-Write, perfil automático da coluna (histograma, nulos,
  outliers), modo "zero RAM" com `mmap`, e gravador de macro em JS com
  vetorização. Detalhe e notas em `docs/planejamento/ROADMAP.md` → "Ideias do usuário para a
  planilha analítica". O perfil de coluna é o mais perto de sair; o
  histórico e o `mmap` dependem do `.plz` deixar de ser um gzip único.

**Pendentes de antes, na mesma sessão:**
- [ ] O `.xlsx` do usuário (`Custos_Armarios.xlsx`) ainda não traz o **painel
  congelado** (4 linhas fixas) nem o **gráfico** da aba Resumo.
- [ ] **Salvamento automático**: ele disse "salvamento automático é para
  recuperação caso seja fechado forçadamente". Pergunta aberta: num arquivo que
  já tem nome, o automático deve gravar só numa cópia de recuperação (como o
  Excel), e o arquivo mudar só no Ctrl+S? Hoje grava no próprio arquivo.
- [x] Salvamento automático pesado — medido (23/09): o retrato que segura a
  ponte leva ~50 ms; o resto roda fora da janela. O "pico de 2,3 GB" do log era
  o `VmHWM`, o maior pico DESDE QUE O APP ABRIU (da abertura), não da gravação,
  que fica em ~370 MB; o log agora diz isso. A compressão passou ao nível
  rápido: 1.987 → 371 ms para 2 milhões de linhas, mesmo tamanho no disco.
- [ ] Mesclar/desmesclar não entram no Ctrl+Z (como as tabelas).
- [x] O teste "formatação de planilha inteira / coluna inteira" (23/09: era
  bug de verdade; ver passo 3 acima).
- [ ] Gerar os pacotes (`.deb`/`.AppImage`/`.rpm`) com o app fechado — os de
  12:05 não têm o gráfico no Rust nem as caixas novas da dinâmica — e medir no
  app a troca de aba e a dinâmica depois dessas mudanças.

### Sessão de 22/09/2026 — memória, gráficos e filtros (✅ 12 itens)

Resumo para quem chega depois de um `/clear`. **Tudo medido no aplicativo
rodando**, não em teste sintético — foi a lição que fez esta sessão funcionar
onde a de 21/09 falhou.

| Problema | Antes | Agora |
|---|---|---|
| Abrir o `.plz` de 2 M linhas | 7.736 MB | **3.111 MB** |
| Salvar | triplicava (~3,6 GB) | **1,5 GB** |
| Gráfico sobre 2 M linhas | 9 GB, travava | teto + aviso |

Os consertos, e o que cada um ensinou:

2. **Células fantasma** — 9.999.881 células vazias numa coluna só, dizendo
   "coluna H é moeda". `crates/sheet-io/src/enxugar.rs` colapsa em `colStyles`
   antes de atravessar a ponte, e o gunzip saiu do JavaScript.
4. **`colStyles` não era gravado por gravador nenhum** — abrir e salvar
   apagava a formatação de coluna. Agora está nos dois gravadores PLZ2.
5. **`SOMA`/`PROCX` congelavam.** NÃO era o motor: a grade guarda o texto
   formatado no objeto da célula, e a fórmula que mora no `Map` é devolvida
   como o MESMO objeto quadro após quadro. Fórmula com intervalo é recusada
   pelo `patternCheck` e fica no `Map`; sem intervalo vai para a coluna
   empacotada, onde a célula é remontada sempre. Daí a assimetria.
   O `markDirty` agora limpa o cache.
7. **Gráfico agregado somava 489× menos** — agregava sobre a amostra. Agora
   quem agrega lê tudo e ACUMULA (soma por categoria, não entrada por linha).
8. Rodapé somava linha escondida; eixo Y cortado; séries escondidas ignoradas
   no agrupamento; seletor Soma/Média/Contagem/Mín/Máx/Desvio padrão.

**O que ficou aberto:**

- o **filtro reabrindo**: o critério agora sobrevive (conferido no arquivo do
  usuário: `criterio: ["NORMAL"]`, 1.747.769 linhas com altura 0), mas o
  usuário ainda relata comportamento estranho ao reabrir. É o próximo item;
- o gráfico **plota linha escondida** pelo filtro — Excel e Sheets não fazem
  isso. Não mexi porque mudaria todo gráfico existente;
- a **vírgula decimal na barra `fx`**: `getRaw` reconstrói número com
  `String(n)`, que usa ponto sempre — `1,1` digitado volta como `1.1`;
- o **autocompletar** deve seguir o ajuste de idioma (as duas grafias
  continuam válidas; só muda o que é EXIBIDO);

**Como medir, para não repetir o erro de 21/09:** abrir um arquivo imprime no
terminal `[abrir] etapa — total (app X · web Y) · pico Z · N ms`, e cada edição
imprime `[mudou] N célula(s): A1 C1`. Rodar com teto do sistema para não travar
a máquina:

### Sessão de 22/09/2026 (tarde) — o front travando: quatro causas medidas

4. **A legenda saía por cima do desenho**: era uma fileira só, sem quebra de
   linha. Com 5 séries de nome comprido num gráfico de 480 px, o último item
   ia de 386 a 547 px. Agora quebra linha (teto de 3 fileiras, o resto vira
   "+N"), e a legenda de pé também para de descer por baixo do gráfico.
5. **Cabeçalho de tabela no lugar da letra**, como no Excel: dentro de uma
   tabela com cabeçalho, quando a linha de cabeçalho já saiu da tela por
   cima, o nome do campo toma o lugar do `A`, `B`, `C`. Enquanto o cabeçalho
   está à vista (ou congelado), a letra fica.

**A grade É virtualizada**, respondendo à pergunta do usuário: medido numa
aba de 400 mil células roladas até a linha 25.000, um quadro lê **76**
células. O que travava nunca foi o desenho — eram as varreduras de planilha
inteira acima, em caminhos que ninguém imaginava caros (medir uma coluna,
listar o nome de uma série).

**O que ficou aberto:** "personalizar um pouco mais as tabelas" ainda não tem
escopo definido — falta o usuário dizer o que quer (estilo por faixa, linha de
totais, referência estruturada?).

### Sessão de 22/09/2026 (tarde, parte 2) — as tabelas, os quatro itens

- **Alça de redimensionar** no canto de baixo à direita, com prévia
  tracejada, e a tabela **cresce sozinha** quando se digita na linha logo
  abaixo. Para cima e para a esquerda não cresce: ali a linha nova seria o
  cabeçalho.
- **Referência estruturada** `=SOMA(Tabela1[Valor])`, com `[#Tudo]`,
  `[#Dados]` e `[#Cabeçalhos]`. Resolvida na hora, a cada avaliação — por isso
  acompanha a tabela crescendo e **não precisa** entrar no `mapReferences`.

⚠ **Duas dívidas com a regra "ninguém acrescenta funcionalidade em
`src/core/`"**, as duas conscientes e anotadas aqui para o lado Rust não ser
pego de surpresa:

Dois bugs da mesma família do `px()` foram consertados de passagem: o acerto
do **botão de filtro** no cabeçalho da tabela (errava até uma coluna quando
rolado) e, na parte 1 da sessão, a letra da coluna.

**Aberto:** o gráfico **pisca em branco ao arrastar** — diagnóstico conferido
e tarefa escrita para o Gemini no fim de `docs/planejamento/TAREFAS-GEMINI.md` (`setupDrag`
chama `render()`, que faz `replaceChildren()`, a cada `mousemove`).

### Decidido: um módulo de consulta, compartilhado

Ele é o meio do caminho entre "um intervalo de células" e "o que a pessoa quer
ver": uma **sequência de passos declarada em dados** (filtrar, ordenar,
agrupar, somar/contar/média, renomear, tipar coluna, remover repetidas,
dividir texto, juntar duas fontes), guardada no arquivo e executável de novo
quando os dados mudam.

Quem vai consumir, e por isso ele não pode nascer amarrado a nenhum:

- **Tabela dinâmica** (item 4): agrupar por campo e somar valores é um
  encadeamento desses passos; a interface só escolhe os campos.
- **Converter para tabela** (item 2): filtro e ordenação por coluna.
- **As matrizes dinâmicas** já existentes (`FILTRO`, `ÚNICO`, `CLASSIFICAR`)
  podem passar a compartilhar a mesma execução, em vez de cada função ter a
  sua.

Como fazer, para não repetir erros já cometidos aqui:

- **Puro, testável no Node, sem tocar em `Sheet`**: recebe linhas e devolve
  linhas. Quem lê da planilha e quem escreve o resultado é de fora.
- **Preguiçoso e ciente da área usada**: com o teto em 100 milhões de linhas,
  um passo que varra o intervalo declarado em vez dos dados existentes
  inutiliza o módulo inteiro. Ver o item da área usada nos cuidados abaixo.

### Fila do usuário, o que sobrou

1. **Validação de célula** — lista suspensa, faixa, data, caixa. Gravada no `.xlsx` em 14/10/2026 (`f9a9e485`), exceto caixa, e-mail e URL.
2. **Converter para tabela** — chip do Sheets, faixas, nome, referência
   estruturada.
4. **Tabela dinâmica** — campos em linhas/colunas/valores, agrupar por data.
5. **Janelas nas macros** — o UserForm do VBA.
6. **Células mescladas** — o desenho precisa tratar regiões mescladas.

### Git local (21/09/2026)

O projeto passou a ter `git` local. Eu (Claude) **não tenho terminal na
máquina** nesta sessão — só leio e escrevo arquivo por arquivo pela ponte —
então não consigo rodar `git commit` sozinho. O combinado: toda vez que eu
terminar uma mudança, devolvo a mensagem de commit pronta, sempre começando
com `claude:` e explicando o que foi feito e por quê, e o usuário (ou o
Gemini, que tem terminal) roda o commit. Serve pra distinguir no histórico o
que veio de mim do que veio do Gemini ou do usuário.

## Cuidados ao mexer

- **Inserir/excluir linhas ou colunas esquece o histórico** das abas mexidas
  (`Sheet::esquecer_historico`, auditoria de 26/10/2026, #46): o deslocamento
  não é um passo do desfazer, e um Ctrl+Z velho apagava a célula errada. Quem
  reajusta tabelas/mesclas depois (`refluir`) esquece de novo, senão o "antes"
  delas volta nas coordenadas velhas. Desfazer a estrutura de verdade pede um
  `Passo::Estrutura` com a fatia removida.
- **Vínculo externo de pasta aberta do disco não lê nada sozinho** (#50): nem do
  disco nem de outra janela, até "Atualizar valores" (`aguardando_permissao`).
- **Texto do `.xlsx` entra com a `MARCA_TEXTO`** (#48) e não passa pelo
  `parse_literal`; o texto "VERDADEIRO"/"FALSO" ainda volta lógico porque a
  coluna de dados guarda o lógico como esse texto no pool.
- Ao acrescentar uma barra na tela, conferir que `.grid-host` continua sendo o
  único item flexível.
- Ao acrescentar função que devolve matriz, incluir o nome em
  `DYNAMIC_FUNCTIONS` (senão o derramamento não aparece até a âncora ser lida).
  Se ela só vira matriz quando o argumento é matriz (como `SE` e `ANO`), o
  lugar é `ARRAY_LIFTING`, e o corpo usa `lift()` ou `broadcastAll()`.
- `forEachValue` entrega um segundo parâmetro dizendo se o valor veio de uma
  coleção (intervalo ou matriz). Texto e lógicos de dentro de coleção não
  viram número — é o que faz `MAIOR(SE(...))` ignorar os `FALSO`.
- A formatação condicional **não grava estilo em célula**: a grade consulta
  `sheet.conditionalAt(linha, coluna)` ao desenhar, uma vez por quadro, e
  guarda a resposta em `condFormats`. Quem mexer no laço de desenho precisa
  manter essa consulta única — chamá-la no fundo e de novo no texto dobra o
  custo.
- **Fórmula: quatro estados de `ast`, e a pergunta certa é pelo `raw`.**
  `Node` = árvore em mãos; `undefined` = **solta** (já calculou, `value` tem o
  resultado, reparseia quando sujar); `null` = não compila, o `#ERROR!` final
  está em `value`. Como célula literal guardada como objeto também tem `ast`
  indefinido, **nunca pergunte ao `ast` se aquilo é fórmula** — use
  `isFormulaRaw(raw)`. Foi exatamente isso que quebrou o `markDirty` na
  primeira tentativa: a célula deixava de ser sujada e congelava no valor
  antigo.
- **Fórmula nova que devolve matriz precisa entrar em `DYNAMIC_FUNCTIONS`** —
  a regra antiga, agora com uma segunda rede. O `patternCheck` usa o
  `maybeMatrix`, que só enxerga o que está na lista; se passar batido, a
  célula vai para a coluna empacotada e o derramamento se perderia. Hoje o
  `columnarValue` percebe a matriz na hora do cálculo, **migra a célula para o
  `Map`** e reprova o padrão para as irmãs dela. Funciona, mas é conserto de
  emergência: quem acrescenta função de matriz continua devendo a lista.
- **Quem decide se a fórmula vai para a coluna empacotada é o `patternCheck`**,
  e ele roda **uma vez por padrão**, não por célula. Recusa intervalo, outra
  aba, intervalo nomeado, matriz, volátil e fórmula com estilo — esses
  continuam como objeto no `Map`, com o grafo de sempre. Ao acrescentar um
  motivo novo de recusa, lembre que o padrão reprovado fica lembrado
  (`rejeitados`): a decisão vale para a coluna inteira.
- **`forEachInRange` escolhe entre dois caminhos** e isso importa: percorrer
  o `Map` inteiro é o certo para um pedido grande, mas custa `cells.size` *por
  chamada*. Quem pede em blocos pequenos (o rodapé soma de 512 em 512) fazia o
  `Map` de 96 mil células virar dezenas de milhões de iterações para ler 500.
  Hoje ele percorre o retângulo quando o retângulo é menor que o `Map`.
- **Nada de tocar na grade no construtor da `StatusBar`**: o campo `status` é
  criado antes do `grid`, e pedir `this.grid.zoom` ali derruba o app inteiro.
  O valor do zoom só é lido no primeiro `syncBars`. O teste "a interface monta
  sem lançar exceção" pega isso.
- **Fora da aba de origem, a caixa da célula some e a barra de fórmulas vira o
  editor** (como no Sheets). A caixa flutuando sobre outra planilha confunde, e
  simplesmente escondê-la deixava a fórmula sem lugar para digitar: o `;` não
  entrava e qualquer tecla caía no `startEdit`, que **reinicia a edição com
  aquela tecla** — era o "limpa tudo" relatado. Agora o `onEdit` da barra chama
  `grid.updateEditingText(texto, cursor)`, o editor escondido continua sendo a
  fonte da verdade (cursor e referências saem dele), e o `setFormulaWhileEditing`
  escreve na barra mesmo com o foco dentro, para a referência clicada aparecer
  na hora. O foco é pedido de novo no quadro seguinte, porque o clique na aba
  devolve o foco ao botão depois do nosso.
- **A barra de fórmulas espelha o editor durante a edição** (`onEditingChange`
  -> `syncBars`), em vez de mostrar o conteúdo da célula ativa. Sem isso,
  trocar de aba no meio de uma fórmula esvaziava a `fx`, porque a célula ativa
  da outra aba é outra.
- **O retângulo tracejado aparece na aba a que a referência pertence.** O
  `referenceBoxes` guarda o nome da aba do prefixo (`Dados!A1`, `'Meu
  Plano'!A1`) e o `boxesForSheet` escolhe o que cada aba mostra: as que citam
  essa aba, mais as sem prefixo quando a fórmula está sendo escrita nela. A cor
  é a mesma nas duas abas, porque a coloração acontece antes do filtro.
- **No jsdom o `requestAnimationFrame` falso devolve 0, não 1.** Ele chama a
  função na hora, e o id só é guardado depois; devolver um id verdadeiro
  deixava `this.frame` preso e a grade **nunca mais redesenhava** no teste. O
  canvas de mentira anota `strokeStyle`/`setLineDash`/`strokeRect` em
  `window.__DESENHO__`, que é como se testa o que foi desenhado.
- **As abas são recriadas a cada `renderTabs()`**: em teste, consultar `.tab`
  na hora do clique, senão o clique cai num nó solto do documento. O nome
  limpo está no `.tab-name`.
- **Trocar de aba no meio de uma fórmula não encerra a edição.** A grade
  guarda `editSheet`/`editCell` (onde a fórmula começou), o `setSheet` só
  comita se não houver fórmula aberta, o `writeReference` põe
  `quoteSheet(aba)!` quando a referência vem de outra aba, e o
  `commitEdit`/`cancelEdit` gravam na aba de origem e pedem a volta pelo
  `onReturnToSheet`. Era bug relatado em 19/09/2026.
- **Escape no editor é em dois tempos**: o primeiro fecha o autocompletar, o
  segundo fecha a edição. Teste que fecha com um Escape só deixa a edição
  aberta, e a tecla seguinte não faz nada (`if (this.editing) return`).
- **O tokenizador do motor não lê fórmula em inglês.** Ele é pt-BR: em
  `SUM(A1:A3,2.5)` a vírgula entre dígitos vira decimal e o texto vira
  `A30,20,5`. Por isso o `formulaFromExcel` tem varredor próprio, e só a ida
  (`formulaToExcel`) usa o `tokenize`. Na ida é preciso devolver o espaço em
  branco que o tokenizador engole, senão `=SOMA(A1; B1)` volta grudado.
- **Matriz troca de pontuação no `.xlsx`.** Aqui a mesma linha é `{1\2}`
  (porque `{1,2}` seria o número 1,2); no Excel é `{1,2}`. O `;` de mudar de
  linha é igual nos dois lados.
- **`.xlsx` aberto não vira o arquivo de trabalho**: o `caminho` fica nulo de
  propósito, senão o salvamento automático gravaria um `.plz` dentro de um
  arquivo com cara de Excel.
- **Uma dependência de execução, e só uma: o `apache-arrow`** (19/09/2026,
  decisão do usuário — "arrow é vida"). Entrou porque o `.plz` passa a ser
  colunar binário, e Arrow dá de graça o que a gente já tinha à mão (dicionário
  para a tabela de padrões, *validity bitmap* idêntico à nossa máscara de
  presença, bloco de 65.536 linhas) **mais** interoperabilidade com pandas,
  polars, DuckDB e Parquet — que é o que os itens "Python" e "Fonte de dados"
  do `docs/planejamento/ROADMAP.md` vão precisar. Custo medido: +223 kB no pacote, +52 kB
  gzipado. Não é porta aberta: continua valendo não acrescentar dependência
  sem medir o custo e sem um ganho desta ordem.
- **O valor calculado das fórmulas mora no arquivo, e a ordem de carga importa.**
  No `carregarV2`, as células avulsas entram **antes** das colunas Arrow: cada
  `applyLoose` passa pelo `apply`, que invalida quem depende da célula, e
  carregá-las depois sujava as fórmulas das mesmas linhas — jogando fora o valor
  recém-lido (91.298 por coluna, no arquivo de teste). Quem mexer no
  `carregarV2` precisa manter essa ordem; o teste "abrir um PLZ2 não recalcula
  fórmula nenhuma" trava isso.
- **Quem abre um arquivo chama `aquecerFormulas()`.** Um `.plz` pode vir sem os
  valores (versão antiga, ou gravado antes de a planilha calcular algo), e aí a
  primeira soma de uma coluna de fórmula paga tudo de uma vez. O aquecimento faz
  essa conta em segundo plano, e a próxima gravação a leva para o arquivo. Ao
  acrescentar um caminho novo de abertura, lembrar de chamá-lo — são os mesmos
  lugares que chamam `entrarNaPlanilha`.
- **Fórmula que gera texto ou erro não pode ir crua para o arquivo**: o
  `payload` dela guarda um id do pool global de strings (ou um índice na tabela
  de erros da aba), e os dois só existem nesta execução. O `saveColumn` os
  troca por índice num dicionário local que vai junto — se alguém acrescentar um
  tipo de valor novo, precisa fazer o mesmo.
- **Gravação de arquivo é atômica** (`gravar_arquivo` no Rust): escreve num
  `.parcial` e renomeia por cima. O salvamento automático dispara o tempo todo;
  uma queda no meio deixaria o arquivo da pessoa pela metade.
- **Dois salvamentos com ritmos diferentes**: a cópia de segurança vai para o
  `localStorage` a cada 0,8 s com um carimbo (qual arquivo, quando); o disco é
  gravado no intervalo escolhido. Se o carimbo for mais novo que o `mtime` do
  arquivo, sobrou trabalho — é assim que a recuperação sabe.

## Conferir a tela sem o usuário (14/10/2026)

`FEATHERSHEET_CAPTURAR=/caminho/tela.png ./target/release/feathersheet` abre uma planilha em
branco, fotografa a janela principal (1280×800) e fecha. `FEATHERSHEET_CAPTURAR_CELULA=XFD1`
seleciona a célula antes da foto. Usar antes de devolver mudança visual ao usuário.
`FEATHERSHEET_CAPTURAR_CELULAS='A1==SEQUÊNCIA(5)|E1=30'` escreve células antes da foto
(endereço, `=`, conteúdo; itens separados por `|`). `FEATHERSHEET_CAPTURAR_MENU_CLASSICO=2` recolhe a faixa e abre esse menu clássico; `FEATHERSHEET_CAPTURAR_JANELA=condicional` cria uma regra de exemplo e fotografa a janela de formatação condicional. `FEATHERSHEET_CAPTURAR_ABA=2` mostra essa aba (índice a partir de 0; só no app, útil com um arquivo na linha de comando). `FEATHERSHEET_CAPTURAR_CAIXAS=1` põe caixas de seleção (Inserir > Checkbox) na seleção de
`FEATHERSHEET_CAPTURAR_CELULA`, uma sim, uma não marcada. `FEATHERSHEET_CAPTURAR_FAIXA=formulas` abre
essa aba da faixa; `FEATHERSHEET_CAPTURAR_RASTREIO=ppd` traça níveis de precedentes (`p`) e
dependentes (`d`) na célula ativa; `FEATHERSHEET_CAPTURAR_MENU=Lógica` abre o menu dessa categoria
(ou `Mais`, `Recentes`, `Nomes`, `Cálculo`; `Criar` abre Criar a partir da seleção). A célula aceita
intervalo (`FEATHERSHEET_CAPTURAR_CELULA=A1:C3`). `FEATHERSHEET_CAPTURAR_JANELA=inspecao` põe a seleção na
Janela de inspeção e fotografa essa janela. `FEATHERSHEET_CAPTURAR_JANELA=historico_macros` roda seis macros de exemplo (à mão, botão,
evento aoMudar/aoAtivar, uma com erro, uma respondida com Não) e fotografa o editor Alt+F11 com o
Histórico aberto e a linha de erro escolhida. `FEATHERSHEET_CAPTURAR_JANELA=busca_macros` abre o editor com um
módulo de exemplo só na memória, a busca do Explorador com "TOTAL", Ctrl+H com "sheet" → "planilha" e a
calha de números; `FEATHERSHEET_CAPTURAR_BUSCA=total` troca o termo (`*total` liga "Todos os módulos",
`-` deixa a barra fechada). **Limite da foto:** o `window::screenshot` do Iced 0.13 não desenha o texto do
`text_editor` (vale também para o editor antigo, sem calha): a foto mostra calha, destaques e barra, mas o
código em branco. `FEATHERSHEET_CAPTURAR_AJUDA=SOMA` abre a Ajuda F1 na página
do primeiro tópico com exemplo contendo esse termo (função ou macro, ex. `sheet.set`) e fotografa a janela
da ajuda; `SOMA+copiado` mostra também o retorno "Copiado" do primeiro botão Copiar.
`FEATHERSHEET_CAPTURAR_INICIO='celula:A1,duplo,celula:B5:D6,grade,menu:bordas'` prepara a Página Inicial
(`app/desenho_borda_ui.rs`): `duplo`/`simples` (sublinhado na seleção), `recuo`, `grade`/`contorno`/`apagar`
(traço da caneta de borda), `modo` (caneta ligada), `menu:bordas|limpar|classificar|sublinhado|formatar`, `celula:B3[:D6]`.
`FEATHERSHEET_CAPTURAR_ZOOM=75` muda o zoom só nessa execução; `FEATHERSHEET_CAPTURAR_CLIQUE=ultima`
(ou um endereço; `ultima+seta` ainda aperta ↓ e →) clica depois do primeiro desenho da grade, e a barra
de status mostra zoom, célula e origem. `FEATHERSHEET_CAPTURAR_TINTA=1` abre a aba Desenhar com a
caneta ativa e traços de exemplo (à mão, marca-texto, e retângulo/elipse/seta pela "Tinta para forma");
`FEATHERSHEET_CAPTURAR_TINTA=menu` deixa aberto o menu de cor/espessura da Caneta 2.
`FEATHERSHEET_CAPTURAR_COMENTARIOS='B3=texto|D6!=à mostra\nsegunda linha|sobre:B3|todos|editor'` põe comentários (`!` = sempre à mostra), o balão do mouse, Mostrar todos ou abre o editor na célula ativa (o texto do `text_editor` não sai na foto).
`FEATHERSHEET_CAPTURAR_DIALOGO=nomes|nomes_exemplo|nomes_novo|erro|estatisticas|perfil|link|cenarios|ir_para|colar_especial|classificar|atingir_meta|tabela_dados|avaliar|celulas|dimensao|protecao|subtotal|novo_estilo|macro|serie|serie_data|validacao|validacao_entrada|validacao_alerta_aba|validacao_parar|validacao_aviso|validacao_info|validacao_balao|validacao_circulos|validacao_menu` abre esse diálogo interno (`app/dialogo_interno.rs`) antes da foto. Diálogo novo usa `App::dialogo(titulo, corpo)` (`feather_ui::dialogo`, guia em `feather-ui/GUIA-JANELAS.md` seção 5.1).

**Aba Desenhar (14/10/2026):** `app/desenhar.rs` só liga a planilha às peças comuns — motor em
`feather-draw::tinta`, camada e controles em `feather_ui::tinta`, âncoras/`.fshz`/`.xlsx` em
`feathersheet_io::tinta_aba`. A tinta mora no campo opaco `tn` da aba (JSON com âncora por traço +
texto `FTINTA`), gravado por `Sheet::alterar_tinta` (um Ctrl+Z). Campo novo na aba: lembrar do
`enxugar.rs` (`deny_unknown_fields`), como foi feito para o `tn`.

**Pressão da caneta (14/10/2026, ainda sem teste com a mesa real):**
- *Por que por fora do Iced:* Iced 0.13/winit 0.30 entregam a caneta como mouse, sem força, e
  o winit 0.30 não fala o protocolo tablet-v2 do Wayland. Decidido com o usuário: **não** ler
  `/dev/input` (exigiria o grupo `input` = todos os teclados) e **não** fazer fork do winit.
- *Camadas:* motor `feather-draw::tinta::pressao` (curva `CurvaPressao`, pressão
  sintética pela velocidade, `contorno_largura_variavel` = polígonos preenchidos que tela, PDF,
  SVG e `.xlsx` desenham igual; `Traco::pressao_sintetica`, gravado como `v` no texto
  `FTINTA`); `feather_ui::caneta` (`FontePressao`, casamento ponto↔pressão por tempo,
  XInput2 pelo `x11rb` em Rust puro lendo só os eventos crus dos dispositivos com eixo
  "Abs Pressure", WM_POINTER no Windows, diário de diagnóstico); `feather_ui::tinta::Camada`
  (campos `fonte_pressao`, `curva_previa`, `velocidade_previa`, `mostrar_pressao`;
  `TracoTela::contorno`; a ponta que chega como `mouse::Button::Other(n)` também risca).
- *Curva:* a pressão crua passa pela curva da pessoa **ao capturar** (gama das Configurações,
  padrão 0,75) e o arquivo guarda essa "pressão de resposta"; todo desenho usa
  `CurvaPressao::PADRAO` (mínimo 0,2, gama 1): espessura × (0,2 + 0,8 × pressão).
  Marca-texto não usa pressão. Sem mesa, com "espessura pela velocidade" (ligado por
  padrão), a pressão sintética fica entre 0,3 e 1 e o traço é marcado `v`.
- *Janela no Linux/Wayland (relato do usuário, 14/10):* numa janela Wayland nativa a ponta da
  XP-Pen move o cursor mas **não clica em nada** (nem botões, células ou menus): o winit 0.30
  não fala tablet-v2. Por isso `main.rs` decide antes de criar a janela: com **mesa
  conectada** (modo automático, padrão) a janela abre pelo **XWayland** (tira
  `WAYLAND_DISPLAY`/`WAYLAND_SOCKET`; o winit usa `DISPLAY`), e a caneta vira mouse no app
  inteiro. A mesa é detectada por `feather_ui::caneta::mesas_conectadas()`: só lê nome e
  capacidades em `/sys/class/input/input*/` (ABS_PRESSURE + BTN_TOOL_PEN), nunca abre
  `/dev/input`. Configurações > Planilha > "Janela no Linux (Wayland)": Automático / Sempre
  X11 / Sempre Wayland (`janela_linux` = `auto|x11|wayland`); teste sem mexer nas
  preferências: `FEATHERSHEET_JANELA=auto|x11|wayland`. O Console F12 diz onde a janela abriu
  e avisa se ficou no Wayland com mesa. O XWayland desta máquina tem `xwayland-native-scaling`
  ligado no mutter, então a janela X deve ficar nítida — **conferir na primeira execução**
  (a foto do modo captura sai do renderizador e não mostra a escala do compositor).
- *Clique da ponta (19/10/2026, testado com a mesa):* o winit descarta a maioria dos cliques da
  caneta no XWayland (13 de 15 no log). O widget `feather_ui::clique_caneta` embrulha a raiz da
  janela principal (`App::view`) e entrega o toque lido pelo XInput2 fora da tinta como
  `ButtonPressed`/`ButtonReleased(Left)` por dentro do app (arrasto incluso; clique do próprio Iced
  vence, sem duplicar). **Não voltar ao XTest:** simular mouse no sistema faz o GNOME pedir a
  permissão de área de trabalho remota, e o toque se perde enquanto ela não é dada.
- *Pressão:* Configurações > Planilha > "Caneta: usar pressão da mesa" (`caneta_pressao`,
  ligada por padrão; vale na próxima abertura). Só funciona com a janela em X11/XWayland
  (ou no Windows). `FEATHERSHEET_CANETA_X11=1` força pressão + X11; `=0` desliga a pressão.
- *Diagnóstico:* `FEATHERSHEET_CANETA_DIAG=1` lista os dispositivos XInput, anota os eventos
  crus de mouse/toque da camada e do XInput2 no Console F12 **e no terminal**, e liga o
  indicador "pressão 0,63" no canto da grade durante o traço. Cada traço sempre deixa uma
  linha `[caneta]` no Console F12 (pressão mín–máx, média, origem). Diagnóstico sem o app:
  `cargo run --example caneta_diag -- 15` no `feather-ui`.
- *Conferido aqui:* o XWayland desta máquina expõe `xwayland-tablet stylus` com
  `Abs Pressure 0..65535` e aceita a seleção dos eventos crus. Windows: só `cargo check
  --target x86_64-pc-windows-gnu` do `feather-ui` (sem rodar).
- *Histórico:* o `[patch]` provisório do `feather-ui` saiu; desde a v0.1.0 tudo vem por tag.
- *Roteiro de teste com a mesa:*
  1. Clique da caneta no app inteiro: `FEATHERSHEET_CANETA_DIAG=1 cargo run --release`
     (manifesto da GUI). O terminal deve mostrar `[janela] Janela pelo XWayland (preferência
     auto); mesa: UGTABLET Deco 01 V3 Pen.` Tocar com a ponta num botão da faixa (ex.: aba
     Fórmulas) e numa célula: devem responder como clique de mouse.
  2. Riscar: aba Desenhar, Caneta 1, encostar e arrastar. O terminal mostra
     `[camada] mouse ButtonPressed(…)` (qual botão chegou) e `[camada] fim do gesto …`.
     Se nada riscar, mandar essas linhas. Para comparar com o Wayland nativo:
     `FEATHERSHEET_JANELA=wayland FEATHERSHEET_CANETA_DIAG=1 cargo run --release`.
  3. Pressão: no mesmo traço, o canto da grade mostra "pressão 0,xx" mudando com a força, o
     traço engrossa apertando mais, e o terminal mostra `[caneta-x11] … pressão bruta N -> 0,xxx`
     e `fim do gesto … pressão em N de M pontos`. Se o fim do gesto disser "sem pressão real",
     mandar a saída (sinal de que os eventos crus do XInput2 não chegaram).
  4. Sem mesa (mouse): risco rápido sai mais fino que o lento.


## Matriz dinâmica no motor Rust (14/10/2026)

Até 14/10 o motor Rust calculava a matriz e mostrava só o primeiro item: o
`SpillManager` (tradução do TS, `sheet_spill.rs`) nunca tinha sido ligado. A
ligação é `planilha/sheet/derramamento.rs`:
- âncora = fórmula avulsa com `maybe_matrix` (`deps.link` a põe em
  `deps.array_cells`, que já anda com inserir/excluir linhas);
- vizinha vazia lê a matriz pelo índice (`vazia_ou_derramada`, função fria);
- leitura de FORA do avaliador (profundidade 0) garante as âncoras sujas antes
  (`garantir_derramamentos`) — dentro de outra conta, não, para não acusar
  circular onde não há;
- matriz que muda faz a cascata de "sujo" nas vizinhas; escrever na área
  (derramada ou bloqueada) suja a âncora; o `.fshz` guarda só a âncora, que
  recalcula ao abrir;
- o cálculo paralelo calcula as âncoras antes e deixa as colunas derramadas fora
  das threads (como as avulsas).
Medido nos arquivos do usuário (Planilha Pesada .fshz/.xlsx): abrir, Resumo e
calcular tudo iguais; a corrente de 1 milhão do .xlsx ficou 2–4% mais lenta
(8,85 → ~9,1 s), sem perf disponível para achar o resto.
