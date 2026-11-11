# MCP do FeatherSheet — o que falta

> **Meta do usuário (17/10/2026): "o MCP tem que editar tudo, tudo mesmo".** Cada janela e cada
> comando do app precisa de ferramenta direta, com parâmetros. Esta lista é o caminho.

> Levantamento de 14/10/2026, logo depois da primeira versão (`d177e8f5`, `a4a15506`).
> Crate: `crates/feathersheet-mcp`; ponte ao vivo: `crates/feathersheet-gui/src/app/mcp_vivo.rs`.
> Regra que vale para tudo abaixo: **nunca despejar a planilha** — o motor calcula, a IA recebe
> resumo, amostra ou resultado; cada chamada que escreve é **um Ctrl+Z**.

## O que já existe

| Ferramenta | Arquivo (sem o app) | Ao vivo |
|---|---|---|
| `sheet_resumo`, `sheet_ler` (teto 2000), `sheet_amostra`, `sheet_estatistica`, `sheet_localizar` | sim | sim |
| `sheet_escrever` (valores, fórmula pt-BR, preencher fórmula), `sheet_limpar`, `sheet_desfazer` | sim | sim |
| `sheet_aba`, `sheet_nomes`, `sheet_formula` (precedentes, dependentes, erro, passo a passo) | sim | sim |
| `sheet_consulta` (passos do OpenQuery, grava em aba), `sheet_macro` (catálogo comum + executar) | sim | sim |
| `sheet_formatar`, `sheet_estrutura`, `sheet_mesclar` | sim | sim |
| `sheet_abrir`, `sheet_salvar` (.fshz, .xlsx, .csv/.tsv) | sim | sim |
| `sheet_selecionar`, `sheet_comando`, `sheet_tela`, `sheet_ver` (foto PNG) | — | sim |

Ao vivo, `sheet_comando` já alcança **qualquer item dos menus** (`menu` pelo rótulo) e **qualquer
atalho** (`tecla`). Isso cobre muita coisa de forma indireta, mas sem parâmetros: abre a janela
ou aplica o padrão, e a IA não consegue dizer "borda grossa vermelha" nem funciona sem o app.
Os itens abaixo são as ferramentas **diretas**, com parâmetros e sem depender da tela.

## Falta — formatação e estrutura (prioridade alta)

O que mais se pede depois de dados e fórmulas.

- [x] **`sheet_formatar`** (14/10, `src/formato.rs`): negrito/itálico/sublinhado/tachado, fonte e
      tamanho, cor do texto e do fundo, alinhamento (horizontal, vertical, quebra, rotação, recuo),
      formato de número (moeda, %, data… e casas decimais), bordas (todas, externas, nenhuma, um
      lado da beira, com cor) e `limpar`. Um Ctrl+Z. **Falta:** formato personalizado (o
      `customFormat` é guardado, mas o motor não formata por ele) e espessura/tracejado de borda
      (o motor só tem lado e cor).
- [x] **`sheet_estrutura`** (14/10): inserir/excluir linhas e colunas (fórmulas ajustadas pelo motor,
      mesclagens acompanham; **sem Ctrl+Z**, como no app), ocultar/reexibir, tamanho em pixels,
      autoajuste (ignora células mescladas, como o Excel). Sem o app, recusa inserir/excluir em
      aba com tabela/AutoFiltro: o reflow das tabelas ainda mora na interface (`tabelas/migracao.rs`).
- [x] **`sheet_mesclar`** (14/10): mesclar, centralizar, através, desmesclar. A regra e o JSON do
      `sheet.mc` saíram da interface para `feathersheet_io::mesclas`; o autoajuste foi para
      `feathersheet_core::operacoes::autoajuste`.
- [x] **`sheet_ordenar`** (16/10, `src/edicao.rs`): níveis por letra ou título do cabeçalho,
      decrescente, lista personalizada; um Ctrl+Z; funciona sem o app.
- [x] **`sheet_filtrar`** (16/10): aplicar/limpar/remover/listar o AutoFiltro, com os critérios do
      `filtro_json` (lista, CONT.SE, regras, 10 primeiros, dinâmico, cor). **Só ao vivo**: a regra de
      tabelas e filtros ainda mora em `app/tabelas/migracao.rs`; sem o app, explica e indica
      `sheet_consulta`. Levar `migracao.rs` para o io destrava o modo arquivo (e o `sheet_estrutura`).
- [x] **`sheet_mover`** (16/10): mesma aba, pelo `mover_intervalo` (fórmulas da pasta seguem o bloco).
- [x] **`sheet_copiar`** (16/10): entre abas, Colar especial (tudo, valores, formatos, fórmulas,
      valores e formatos), transpor, operação (`colagem::combinar`), pular vazias; matriz derramada
      sem a âncora vai como valor, como no app.
- [x] **`sheet_substituir`** (16/10): no intervalo, na aba ou em todas; nos valores ou nas fórmulas;
      um Ctrl+Z por aba.

## Falta — objetos da planilha (prioridade média)

- [ ] **Tabelas** (Ctrl+T): criar, renomear, estilo, faixas, linha de totais, redimensionar.
      Conferir as ações de `app/tabelas.rs` e mover a regra para o motor onde ainda estiver na interface.
- [x] **`sheet_dinamica`** (17/10, ao vivo): listar, criar (origem + linhas/colunas/valores pelo
      título ou letra, nova aba por padrão), editar, atualizar (uma ou todas) e excluir, pelo mesmo
      caminho do painel (`app/dinamica_ui/mcp.rs`). Sem o app explica e indica `sheet_consulta`.
      10/11 (MCP-6): criar/editar aceitam `tema`, `subtotais` (sem/em_cima/embaixo), `totais_gerais`
      (`linha_embaixo`, `coluna_direita`), `campos_calculados`, `itens_calculados`, `grupos`, `filtros`
      (mostrar/ocultar itens), `ordenar` (A→Z, Z→A, por valor) e `mostrar_como` (percentuais, índice,
      acumulado, classificação); o listar mostra tudo isso. Teste `opcoes_da_dinamica_pelo_mcp`.
      Também "% de"/"diferença de" com item base e `recolher` (tudo, nada, por nível ou a lista).
      Também `agrupar` (datas por ano, trimestre, mês ou dia; faixas numéricas).
      **Falta:** páginas do filtro, segmentação e gráfico dinâmico (esses dois esperam GRA-9/DIN-15); modo arquivo
      (a regra mora na interface).
- [x] **Gráficos**: `sheet_grafico` (listar, criar, editar, remover). Conferido em 10/11 na lista de ferramentas.
- [x] **Formatação condicional**: `sheet_condicional` (listar, criar, remover, limpar).
- [x] **Validação de dados**: `sheet_validacao` (listar, criar, remover, limpar).
- [ ] **Links** em células: inserir, remover, listar.
- [ ] **Imagens**: inserir imagem (arquivo ou `=IMAGEM`). Botões de macro já em `sheet_botao`.
- [x] **Formulários das macros**: `sheet_formulario` (listar, ler, escrever, remover).

## Falta — dados e análise (prioridade média)

- [x] **Remover duplicados** (10/11): `sheet_duplicados` (colunas por letra ou título, `contar` só
      conta). Teste `duplicados_conta_e_remove_pela_coluna_escolhida`.
- [ ] **Dividir texto em colunas**: a regra mora na interface (`app/assistente_divisao`); mover para o
      motor antes de dar ferramenta. Até lá, o passo `dividirColunas` do `sheet_consulta` faz o mesmo
      numa aba de destino.
- [x] **Atingir meta** (10/11): `sheet_atingir_meta` (definir, para, alterando; grava só se achar).
      Teste `atingir_meta_acha_o_valor_e_grava`.
- [ ] **Tabela de dados** e **cenários**: o JSON dos cenários (`Sheet::ce`) é lido e gravado só pela
      interface (`app/cenarios_ui.rs`); levar o formato para o motor antes de dar ferramenta, para não
      haver duas cópias da regra.
- [ ] **OpenQuery com fonte externa**: CSV/XLSX de outro arquivo, Web/API (só por pedido explícito,
      regra 5) e consultas guardadas no arquivo com atualização. Hoje `sheet_consulta` lê só
      intervalos da própria pasta e não guarda a consulta.
- [x] **Juntar duas fontes** (10/11): `sheet_consulta` com `fontes` ({nome: endereço}); passo que cita
      fonte ausente explica. Teste `consulta_junta_com_outra_aba`.
- [x] **Conversor VBA → JS** (10/11): `sheet_macro` acao `converter_vba` (`arquivo` .xlsm ou `vba`
      colado) devolve o JS e o que não converteu; `guardar` põe as macros na pasta. Os eventos são
      só listados (quem liga é o Importar VBA do app). Teste `converter_vba_devolve_o_js_e_guarda`.

## Falta — exibição e arquivo (prioridade baixa)

- [ ] **Congelar painéis**, zoom, linhas de grade, tema: hoje só ao vivo, por `menu`/`tecla`
      (são estado da janela, não do arquivo).
- [ ] **Impressão**: área de impressão, quebras, exportar PDF pelo produtor comum da suíte.
- [ ] **Proteção**: proteger aba/pasta com senha e bloquear células (existe no app e nas macros, `sheet.protect`; falta ferramenta).
- [ ] **Recentes e modelos**: abrir um recente, criar a partir de um modelo da tela inicial.

## Falta — no próprio MCP

- [ ] **Transporte comum da suíte**: `ao_vivo.rs` é cópia adaptada do `feathercad-mcp`.
      Pela regra 3 da raiz, deve virar um crate próprio usado pelos dois (e pelo Docs e Slide).
- [ ] **`sheet_ver` sem o app**: renderizar um intervalo em PNG fora da janela, para conferir o
      visual também no modo arquivo.
- [ ] **Desfazer entre abas**: uma chamada que escreve em duas abas gera dois Ctrl+Z (o histórico
      é por aba). Agrupar quando o motor tiver histórico da pasta.
- [ ] **`sheet_abrir` ao vivo esperando a carga**: hoje responde "abrindo" na hora; o ideal é
      responder quando o arquivo terminar de abrir.
- [ ] **Confirmação para ações destrutivas ao vivo** (excluir aba, limpar área grande): perguntar
      na tela antes, como o app faz com o usuário.
- [ ] **Guias (`prompts`)** como o `cad_projeto` do FeatherCAD: receitas para relatório,
      controle financeiro, cadastro com formulário e macro, painel com gráfico.
- [ ] **Teste automático do modo ao vivo**: hoje testado à mão; falta um teste que suba o app sem
      janela (ou o `HospedeApp` com socket) e converse pelo binário.
- [ ] **Clique real na faixa**: `sheet_comando` dispara as mesmas mensagens dos botões, mas não
      passa pelo botão. Um botão ligado errado ainda passaria despercebido; só o `sheet_ver` mostra.

## Ordem sugerida

1. ~~`sheet_formatar`, `sheet_estrutura`, `sheet_mesclar`~~ — feitos em 14/10.
2. ~~`sheet_ordenar`/`sheet_filtrar`, `sheet_substituir`, `sheet_mover`/`sheet_copiar`~~ — feitos
   em 16/10. Falta levar o reflow e o filtro das tabelas para fora da interface (destrava
   `sheet_filtrar` e `sheet_estrutura` sem o app em aba com tabela).
3. Gráfico, tabela dinâmica e formatação condicional (reaproveitam o motor que já existe).
4. Transporte comum da suíte (antes de o Docs/Slide ganharem MCP ao vivo).
5. O restante, conforme o uso pedir.

- 10/11: `sheet_consolidar` (Dados → Consolidar, por posição ou rótulos; motor `Workbook::consolidar`). Subtotais em cima/embaixo, recolher por nível, páginas do filtro e adiar atualização da dinâmica ainda sem ferramenta própria.
