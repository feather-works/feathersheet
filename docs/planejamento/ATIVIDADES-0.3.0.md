# Atividades até a v0.3.0 — FeatherSheet

> Documento interno de planejamento. Estado de referência: **30/10/2026**, depois da v0.2.2
> (publicada em 29/10/2026). Não é resumo: cada item é uma tarefa.
>
> **Como ler cada item**
> - **Onde:** caminhos conferidos no repositório (ou marcados "(a conferir)").
> - **Pronto:** critério verificável (teste, foto com `FEATHERSHEET_CAPTURAR`, número medido, issue fechada).
> - **Depende:** o que precisa vir antes (`ID` deste documento ou decisão `D*`).
> - **Issue:** `#N` existente, ou "criar" (regra 23: bug reportado vira issue; versão só em lote).
> - **Tamanho:** P (até meio dia), M (um a três dias), G (mais de três dias). Estimativa, não compromisso.
>
> **Escopo da 0.3.0 (proposta a confirmar, decisão D1):** robustez de dados, memória, tabela
> dinâmica (bugs e os buracos listados), gráficos pedidos em issue, Windows, segurança, Feather Sans
> e #90. **O FeatherQuery sai da 0.3.0** (ver "Fora do escopo"). O roteiro antigo em
> `docs/planejamento/METAS-VERSOES.md` numera "v0.2 = melhor tabela dinâmica" e "v0.3 = FeatherQuery";
> isso ficou para trás (a v0.2 saiu como macros do Excel) e é corrigido em DOC-4.
>
> **Regras que valem para todas as atividades:** nada de `cargo build`/`cargo test` da GUI na máquina
> do usuário sem o limite de memória da memória `limitar-memoria-builds` (testes da GUI só pelo nome
> do teste, nunca a suíte); números grandes (1 M, 14 M linhas) são medidos pelo usuário, e o Claude
> entrega o roteiro (memória `testes-dados-pequenos`); tudo que é texto novo sai em pt-BR, en-US e
> es-ES; toda mudança de interface atualiza a Ajuda F1 nos três idiomas (regra 24).

---

## Estado verificado hoje (fatos de partida)

- **Dependências do app** (`crates/feathersheet/Cargo.toml`): pelicano tag `v0.1.19`, feather-ui tag
  `v0.3.8`, feather-charts `v0.3.4`. Tags remotas existentes: pelicano até `v0.1.20`; feather-ui até
  `v0.3.9`. **Não existem ainda** pelicano `v0.1.21` nem feather-ui `v0.3.10`.
- **Mudança não commitada no app:** `crates/feathersheet/src/main.rs` chama
  `feather_ui::fonte::usar_na_interface()`. Essa função só existe no ramo local `feather-sans` da
  feather-ui (commit `0fa32b0`). Hoje o app só compila com o `[patch]` do fim do `Cargo.toml`.
- **`[patch]` provisório** (`Cargo.toml`, bloco "shortcut: provisório até as tags…") aponta para
  `/tmp/claude-1000/.../scratchpad/wt-pelicano` e `wt-feather-ui`. Essas pastas existem agora, mas em
  `/tmp`, que some no reinício. As cópias de `.wt/` (`pelicano-0119`, `feather-ui-038`) são outros
  caminhos (a conferir qual é o correto).
- **Commits locais sem push no `main`:** 7 (`v0.2.2..HEAD`: `c7dc116`, `0dce643`, `aaf7f11`,
  `97d2fa4`, `1bafeec`, `d7fac05`, `7fa0e43`).
- **Ramo `editor-alinhado`:** 1 commit sobre o `main` (`e5bf09b`, correção do #90). Depende do pelicano
  `v0.1.20` (`align_x` no `text_input`, commit `10cc721`).
- **Arquivos soltos:** `crates/feathersheet/0` (texto de histograma, 32 linhas `n contagem`, gravado
  por engano: não é código); `feathersheet_0.2.2_amd64.snap` na raiz, não rastreado; `.claude/` não
  rastreado; `docs/planejamento/video-v0.2/banner-featheroffice.png` e `vendas-outubro.xlsx` não
  rastreados. Há 5 ramos locais `worktree-agent-*`.
- **Issues abertas (21):** #133, #121, #113, #108, #107, #106, #94, #93, #90, #83, #82, #80, #79, #72,
  #71, #70, #68, #65, #27, #9, #8. Fechadas e fora da lista: #100, #110, #132, #122, #123–#130.
- **Não existe ainda:** `#[global_allocator]` em qualquer crate (confirmado por busca): o alocador de
  teste que recusa pedido acima do teto não foi escrito.
- **`--version`:** `crates/feathersheet/src/main.rs` só trata `--mcp` (linha ~80); qualquer outro
  argumento abre a janela.
- **Pânicos:** `app/eventos_vba.rs` e `app/formulario_macro.rs` não têm nenhum `catch_unwind`
  (confirmado); `app/calculo_em_fundo.rs` e `app/macro_dialogo/em_fundo.rs` têm.
- **CI:** `.github/workflows/` tem só `macos.yml` e `testes.yml`. O `testes.yml` roda core, io, macros,
  vba e mcp, não a GUI. **Não há job de Windows.** O `.exe` sai de `pacote/gerar.sh` (mingw, na máquina).
- **Dinâmica (`feathersheet-core/src/dinamica/pivot.rs:13`):** `AgregacaoOp` tem Soma, Contagem, Media,
  Min, Max, DesvioPadrao, Moda. **Não tem** distintos, mediana nem percentil.
- **Dinâmicas recomendadas:** a flag `recomendacoes` (`app/dinamica_ui.rs:187`, ligada em `:532` no
  `Evento::Recomendadas`) é escrita e **nunca lida**. A Ajuda F1 (commit `c7dc116`) promete o recurso.
- **Limite de operações (conferido pela sessão principal):** são duas constantes diferentes com o mesmo
  nome. `formula/parser.rs:43` (`MAX_OPERACOES` 4.096 no release, 1.024 no debug) é o teto de
  operações de uma fórmula, o que o `CLAUDE.md` descreve; `planilha/sheet/edicao.rs:1659`
  (2.100.000) é o teto de linhas/colunas ocultadas de uma vez. Não há divergência.
- **Teto de célula em bloco:** `app/preenchimento_ui.rs:12` (`TETO_CELULAS_EM_BLOCO` = 250 mi). O teste
  `ctrl_d_de_formula_em_milhoes_de_linhas_vai_em_bloco` está em `app/preenchimento_ui.rs:216`.
- **Barra de rolagem:** `grade/barras.rs:62–90` (`cursos`): `ultima_coluna` é índice de coluna, enquanto
  `vis_colunas` conta só colunas visíveis. Suspeita para o bug de colunas ocultas (a reproduzir).
- **Ajuda F1:** `app/ajuda.rs`, `app/ajuda_macros.rs`, `app/ajuda_atalhos.rs`.
- **Novidades:** `app/novidades.rs` (testes `toda_versao_tem_traducao` em `:427` e
  `versao_desta_compilacao_e_a_primeira_e_todas_desenham` em `:441`).
- **MCP ao vivo:** `crates/feathersheet-mcp/src/ao_vivo.rs:8–9` diz que a primeira linha de cada conexão
  é o token (lado do app em `app/mcp_vivo.rs`: a conferir a validação).
- **VBA:** `feathersheet-vba/src/formulario.rs:88` `MAX_PROFUNDIDADE = 4` (UserForm);
  `feathersheet-vba/src/sintaxe/expr.rs:18` teto de nós (#129). Profundidade de bloco `If`/`For`
  aninhado: a conferir.
- **Configurações MCP:** `app/configuracoes_ui.rs:375` já grava `"args": ["--mcp"]`.

---

## Frente 1 — Publicação pendente e higiene do repositório

### PUB-1. Tirar os arquivos soltos da raiz e de `crates/feathersheet`
- **Onde:** `crates/feathersheet/0`; `feathersheet_0.2.2_amd64.snap` (raiz); `.claude/`;
  `docs/planejamento/video-v0.2/banner-featheroffice.png`, `vendas-outubro.xlsx`.
- **Pronto:** `git status` sem `??` indesejado. O `0` apagado; o `.snap` movido para `pacote/saida/`
  ou ignorado; `.claude/` decidido (D19); os dois arquivos de vídeo commitados ou ignorados (D19).
- **Depende:** D19.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-2. Limpar ramos e worktrees locais
- **Onde:** `git branch`, `git worktree list`; ramos `worktree-agent-a46ff…`, `…a4b03…`, `…a4d3…`,
  `…a6b02…`, `…a750d…`; worktrees em `/tmp/.../scratchpad/` e `.wt/`.
- **Pronto:** para cada ramo, `git branch --contains` confirma que o commit já está no `main` ou em outro
  ramo; ramos órfãos apagados; `git worktree list` só com os necessários.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-3. Enviar os 7 commits locais do `main`
- **Onde:** `git log origin/main..main` (7 commits, do `7fa0e43` ao `c7dc116`).
- **Pronto:** `git log origin/main..main` vazio. Antes, conferir que a Ajuda F1 (`c7dc116`) está
  coerente com o que o app faz (ver DIN-5 e DOC-1).
- **Depende:** D4 (o usuário autoriza o envio: a regra da memória é "publicar exige o suba do usuário").
- **Issue:** nenhuma.  **Tamanho:** P.

### PUB-4. Publicar pelicano `v0.1.21` (Feather Sans)
- **Onde:** repositório do pelicano, ramo `fonte-padrao`, commit `60b80f6` (`set_default_family`,
  `Font::SYSTEM_SANS`). Cópia de trabalho: `/run/media/maragon/Arquivos/GitHub/pelicano`.
- **Pronto:** tag `v0.1.21` no remoto; o teste `clique_na_caixinha_marca_e_o_cursor_fica` (quebrado pela
  mudança de fonte, memória de 28/10) corrigido e verde no próprio pelicano (rodado pelo usuário, com
  limite de memória).
- **Depende:** D3 (autorização de tag/push).  **Issue:** nenhuma.  **Tamanho:** M.

### PUB-5. Publicar feather-ui `v0.3.10` (módulo `fonte`)
- **Onde:** repositório da feather-ui, ramo `feather-sans`, commit `0fa32b0` (`fonte::usar_na_interface`).
  Cópias: `/run/media/maragon/Arquivos/GitHub/.wt/feather-ui-038`.
- **Pronto:** tag `v0.3.10` no remoto. Conferir a base do ramo (a conferir: é descendente de `v0.3.9`,
  `da8cb17`?) e se a feather-ui depende do pelicano por tag ou por caminho (a conferir), para que o
  `Cargo.toml` dela aponte para `v0.1.21`.
- **Depende:** PUB-4 (se a feather-ui pedir o pelicano novo); D3.  **Issue:** nenhuma.  **Tamanho:** P/M.

### PUB-6. Trocar o `[patch]` pelas tags e atualizar o `Cargo.lock`
- **Onde:** `crates/feathersheet/Cargo.toml` (linhas 12 e 16: pelicano `v0.1.19` → `v0.1.21`; feather-ui
  `v0.3.8` → `v0.3.10`; apagar o bloco `[patch]` e o comentário "shortcut: provisório…" no fim);
  `crates/feathersheet/Cargo.lock`.
- **Pronto:** `grep -n patch crates/feathersheet/Cargo.toml` vazio; `Cargo.lock` aponta para as tags;
  `clone` limpo da tag compila (o usuário confirma; o Claude não roda `cargo`). Esse passo também acaba
  com o risco de `/tmp`.
- **Depende:** PUB-4, PUB-5.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-7. Conferir a fonte na tela e nas células
- **Onde:** `crates/feathersheet/src/main.rs` (chamada de `usar_na_interface`); `src/grade.rs` e
  `src/estilos_comuns.rs` (`Some("sans-serif")` nas células, mudança não commitada).
- **Pronto:** foto da janela principal e de uma célula com texto (`FEATHERSHEET_CAPTURAR`, antes e
  depois); o `"sans-serif"` das células confirmado como coerente com `Font::SYSTEM_SANS` (a conferir se
  o valor deve ser `None`). Usuário confirma no Linux; o Windows vai em WIN-2.
- **Depende:** PUB-6.  **Issue:** nenhuma.  **Tamanho:** P/M.

### PUB-8. Commitar a Feather Sans no FeatherSheet
- **Onde:** os arquivos de PUB-6 e PUB-7; `CLAUDE.md` (nota na seção da 0.3.0).
- **Pronto:** commit `claude: Feather Sans como fonte da interface (pelicano v0.1.21, feather-ui v0.3.10)`;
  `cargo` sem `[patch]`.
- **Depende:** PUB-6, PUB-7.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-9. Fechar #90: editar célula mantendo a formatação
- **Onde:** ramo `editor-alinhado` (commit `e5bf09b`, sobre o `main`); `grade.rs` e o editor de célula
  (a conferir o arquivo exato com `grep -rn "text_input\|text_editor" crates/feathersheet/src/grade*`).
- **Pronto:** rebase sobre o `main` depois de PUB-8; pelicano `v0.1.20` (tag existe) com `align_x`;
  foto de `A1:C1` mesclada, negrito, centralizada, letra grande, editada no lugar com a mesma aparência;
  commit com `Fixes #90`.
- **Depende:** PUB-8 (ou, se o rebase for antes, PUB-6 só com `v0.1.20`).  **Issue:** #90.  **Tamanho:** M.

---

## Frente 2 — Robustez de dados e desfazer

### ROB-1. Reproduzir e corrigir: fórmulas de G–M trocadas de coluna
- **Onde:** `crates/feathersheet-core/src/planilha/workbook.rs` (`mover_estrutura`), `planilha/sheet/`
  (inserir/excluir); exemplo `examples/conferir_estrutura_pequena.rs` (a conferir o caminho exato).
- **Pronto:** operação + gravar + reabrir reproduzido em teste pequeno; corrigido; usuário confirma na
  "Planilha Pesada.fshz" (#REF! e Resumo zerado sumiram). Hoje o item está `[ ]` no METAS, sem issue.
- **Depende:** nada.  **Issue:** criar (bug).  **Tamanho:** M.

### ROB-2. Excluir a primeira linha de intervalo encolhe o intervalo
- **Onde:** `crates/feathersheet-core/src/planilha/mover.rs` (regra de reescrita) e a reescrita de
  referências (a conferir: `formula_text.rs`).
- **Pronto:** teste com `=SOMA($I$2:$I$2000)` ao excluir a linha 2 vira `$I$2:$I$1999` (como no Excel),
  nunca `#REF!:$I$2000`. Se o teste já passar, fechar com o teste novo.
- **Depende:** nada.  **Issue:** criar se reproduzir.  **Tamanho:** P/M.

### ROB-3. Ctrl+Z de inserir e excluir linhas e colunas (#65)
- **Onde:** `planilha/sheet/` (histórico: `Passo`, `esquecer_historico`); `planilha/workbook.rs`
  (`mover_estrutura`); tabelas, mesclas e nomes definidos andam junto (ver `planilha/mover.rs`).
- **Pronto:** teste: inserir e desfazer deixa células, fórmulas, estilos, tabelas e mesclas iguais ao
  estado anterior; excluir e desfazer devolve a fatia inteira (incluindo fórmulas de outras abas
  consertadas). Ctrl+Z não regrava célula a célula (ver ROB-4).
- **Depende:** ROB-1 (mesmo código).  **Issue:** #65.  **Tamanho:** G.

### ROB-4. Desfazer da estrutura sem regravar célula a célula
- **Onde:** o mesmo caminho de ROB-3 (o `Passo::Estrutura` guarda a fatia, não cada célula).
- **Pronto:** medido com 2 mil linhas com fórmula acumulada (`=K1+I2`): desfazer ≤ 100 ms (meta a
  confirmar na decisão D8). Número no F12 (`medir` no `feathersheet-core`).
- **Depende:** ROB-3.  **Issue:** #65.  **Tamanho:** incluído em ROB-3 (se passar de G, separar).

### ROB-5. Mesclar e desmesclar entram no Ctrl+Z
- **Onde:** `crates/feathersheet/src/app/mesclas.rs` e `app/mesclas/`; histórico em `planilha/sheet/`.
- **Pronto:** teste: mesclar, Ctrl+Z, desmesclar, Ctrl+Z restaura o estado; foto não necessária.
- **Depende:** nada.  **Issue:** criar (pendência de 28/10, sem issue).  **Tamanho:** M.

### ROB-6. Recálculo fora da thread da janela, com progresso e Esc
- **Onde:** `crates/feathersheet/src/app/calculo_em_fundo.rs` (existe desde a v0.2.1, com "Parar a cada
  256 linhas"); `app/macro_dialogo/em_fundo.rs` como referência. Cálculo no motor:
  `feathersheet-core/src/planilha/calculo_paralelo.rs`.
- **Pronto:** `MÍNIMOSES` em 13,7 milhões de linhas: a janela responde em ≤ 16 ms durante a conta
  (medido pelo usuário com F12), barra com progresso, Esc cancela. Confirmar primeiro se
  `calculo_em_fundo.rs` já cobre o caso (a conferir: o METAS v0.1.1 ainda marca `[ ]`).
- **Depende:** nada.  **Issue:** nenhuma (METAS v0.1.1); criar se o caso ainda travar.  **Tamanho:** M.

### ROB-7. Cópia para gravar não bloqueia a janela
- **Onde:** `crates/feathersheet-io/src/da_workbook.rs`; `app.rs` (salvar); `feathersheet-io/src/fshz_write.rs`.
- **Pronto:** F12 mostra que a parte na thread da janela fica ≤ 16 ms em `2026_03.fshz` (13,7 mi linhas).
  Se a v0.2.1 ("salvar sem copiar as colunas de número") já resolveu, só medir e fechar o `[ ]` do METAS.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P (medir) / M (se não resolveu).

### ROB-8. SES com critério vazio ou zero sem índice
- **Onde:** `crates/feathersheet-core/src/planilha/agregado.rs` (índice por grupo, `sheet_deps.rs`);
  o teste que trava: `crates/feathersheet/src/app/preenchimento_ui.rs:216`.
- **Pronto:** o teste `ctrl_d_de_formula_em_milhoes_de_linhas_vai_em_bloco` passa dentro do tempo do teste
  (a conferir o teto), e a versão do core (`agregado.rs`) tem teste com critério vazio.
- **Depende:** nada.  **Issue:** criar (bug, o teste já era vermelho em 29/10).  **Tamanho:** M.

### ROB-9. Decidir o nível de compressão do `.fshz`
- **Onde:** `crates/feathersheet-io/src/fshz_write.rs`.
- **Pronto:** D8 respondida; número medido no `2026_03.fshz` pelo usuário (METAS: nível 6 = 43 s e
  359 MB; nível 3 = 20 s e 458 MB; nível 1 = 13 s e 702 MB).
- **Depende:** D8.  **Issue:** nenhuma.  **Tamanho:** P.

### ROB-10. Arrastar fórmula para a planilha inteira (padrão por faixa)
- **Onde:** `planilha/sheet/edicao.rs` (`preencher_formula_faixa_vazia`), `planilha/modelo_padrao.rs`
  (`FaixaPadrao`), `crates/feathersheet/src/grade/alca.rs` (alça de preenchimento).
- **Pronto:** arrastar a alça de uma fórmula até a linha 1.048.576 responde em ≤ 1 s (medido) e usa um
  padrão de faixa, não 1 milhão de fórmulas; teste de tempo e de teto de memória (MEM-2).
- **Depende:** D11 (desenho aprovado pelo usuário); MEM-1.  **Issue:** criar (pedido do usuário, 29/10).  **Tamanho:** G.

### ROB-11. Dar nome próprio ao teto de dimensões ocultas
- **Onde:** `crates/feathersheet-core/src/planilha/sheet/edicao.rs:1659` (constante local `MAX_OPERACOES`
  = 2.100.000, teto de linhas/colunas ocultadas de uma vez; homônima de `formula/parser.rs:43`).
- **Pronto:** renomeada para `MAX_DIMENSOES_OCULTAS`; `cargo test` do core verde.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P (opcional, só clareza).

---

## Frente 3 — Memória (plano dos 400 MB e testes que pegam estouro)

### MEM-1. Alocador contador de teste que recusa pedido acima do teto
- **Onde:** novo módulo de teste (ex.: `crates/feathersheet-core/src/teste_memoria.rs`, `#[cfg(test)]`,
  com `#[global_allocator]`); replicado em `feathersheet-io` (a decidir: copiar ou um crate de teste
  comum, D20). Hoje não existe nenhum `global_allocator` no repositório.
- **Pronto:** um teste de exemplo com teto artificial falha em milissegundos e diz o nome do teste e o
  pedido recusado, sem encher a RAM. Só nos crates leves (regra da memória).
- **Depende:** nada.  **Issue:** criar (pedido do usuário, 29/10).  **Tamanho:** M.

### MEM-2. Tetos: arrastar/colar fórmula até o fim e tudo selecionado
- **Onde:** testes sobre `preencher_formula_faixa_vazia` e `Sheet::colar`; caso: 16.384 colunas ×
  1.048.576 linhas com a linha de fórmula (o caso que o usuário testou).
- **Pronto:** teste reprova se o pico passar do teto (MEM-1); medido pico exato.
- **Depende:** MEM-1, ROB-10.  **Issue:** criar.  **Tamanho:** M.

### MEM-3. Tetos: abrir `.xlsx` e `.fshz` gerados no teste (muitas colunas curtas)
- **Onde:** `crates/feathersheet-io/examples/` (os `medir_*` existem); testes novos em
  `feathersheet-io/src/` com geração sintética pequena e teto.
- **Pronto:** reprova o pico acima do teto; compara com o número medido no `2026_03.fshz` pelo usuário.
- **Depende:** MEM-1.  **Issue:** criar.  **Tamanho:** M.

### MEM-4. Tetos: dinâmica e gráfico em muitas linhas
- **Onde:** `feathersheet-core/src/dinamica/pivot.rs`, `dinamica_layout.rs`; `app/graficos/testes.rs`
  (bancada `medir_quadros_grafico_com_bloco`, ignorada).
- **Pronto:** teste com teto; a bancada ignorada vira teste normal com teto.
- **Depende:** MEM-1.  **Issue:** criar.  **Tamanho:** M.

### MEM-5. Tetos: compactar, salvar e reabrir
- **Onde:** `feathersheet-core/src/compactar` (a conferir: `ChunkedColumn::compactar`), `feathersheet-io/src/fshz_write.rs`,
  `fshz_read.rs`.
- **Pronto:** ciclo compactar + salvar + reabrir dentro do teto, com resultado igual bit a bit.
- **Depende:** MEM-1.  **Issue:** criar.  **Tamanho:** M.

### MEM-6. Medir o número atual no app (linha de partida da meta)
- **Onde:** F12 (`app/monitor.rs`) no app; arquivo `Planilha Pesada` e `2026_03.fshz` do usuário.
- **Pronto:** tabela com RAM do app vazio, do `2026_03.fshz` aberto e calculado, e da Planilha Pesada.
  Meta do plano: 400–500 MB (hoje a memória cita ~1.600 MB antes da v0.2.2; medir de novo).
- **Depende:** nada (usuário roda).  **Issue:** nenhuma.  **Tamanho:** P.

### MEM-7. Fase 5 do plano: resultado direto na coluna (sem `calc.cache`)
- **Onde:** `feathersheet-core/src/planilha/column_store.rs`, `planilha/calculo_paralelo.rs`,
  `planilha/textos_crus.rs`; `planilha/faixa_lida.rs` (a conferir).
- **Pronto:** pico de ~360 MB da Resumo recém-aberta do `.xlsx` e a consolidação de 1,5 s caem; teste
  de igualdade de resultado; medido pelo usuário.
- **Depende:** D9.  **Issue:** nenhuma.  **Tamanho:** G.

### MEM-8. Fase 3 do plano: XLSX com dados tipados (sem reserva de 2 milhões de textos)
- **Onde:** leitor de `.xlsx` em `feathersheet-io/src/` (a conferir o módulo: `arrow_fluxo.rs` ou
  `xlsx_read*`); a reserva de textos `"374.99"`, `"14/12/2026"`.
- **Pronto:** ganho medido no `.xlsx` de 13 mi células; mesma leitura célula a célula.
- **Depende:** D9.  **Issue:** nenhuma.  **Tamanho:** G.

### MEM-9. Gravar em fluxo (pico de +360 MB ao salvar)
- **Onde:** `feathersheet-io/src/fshz_write.rs`.
- **Pronto:** pico ao salvar medido no `2026_03.fshz` sem o acréscimo; arquivo igual ao atual
  (`examples/conferir_fshz_iguais.rs`).
- **Depende:** ROB-7, D9.  **Issue:** nenhuma.  **Tamanho:** G.

### MEM-10. F12 e Monitor do GNOME: mesmo critério de memória
- **Onde:** `crates/feathersheet/src/app/monitor.rs`; leitura de `VmRSS`/`VmHWM` (a conferir o que o F12 mostra).
- **Pronto:** o F12 mostra o mesmo número que o Monitor do sistema (diferença explicada ou corrigida;
  hoje ~300 MB).
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P/M.

### MEM-11. Abrir sob demanda (`mmap`) e fórmula compilada (fase 6 e V5)
- **Onde:** `feathersheet-io/src/fshz_read.rs`; `feathersheet-core` (bytecode).
- **Pronto:** fora da 0.3.0 se D9 disser assim; ver "Fora do escopo".
- **Depende:** D9, e o `.fshz` deixar de ser gzip único.  **Issue:** nenhuma.  **Tamanho:** G (fora).

---

## Frente 4 — Tabela dinâmica

### DIN-1. Bug: a coluna DATA sai como número no começo e como texto no fim
- **Onde:** `feathersheet-core/src/dinamica/pivot.rs` (`excel_serial_to_ymd`), `dinamica_layout.rs`;
  `crates/feathersheet/src/app/dinamica_ui.rs`.
- **Pronto:** teste com 3 datas de origem: a dinâmica mostra data em todas as linhas (formato único);
  foto de `FEATHERSHEET_CAPTURAR_DINAMICA`. Relato da memória (29/10): `46240` no começo e
  `7/30/2026` no fim.
- **Depende:** nada.  **Issue:** criar (bug).  **Tamanho:** M.

### DIN-2. Total Geral fora do gráfico dinâmico (conferir)
- **Onde:** commit `355e9dd` (já feito, segundo a memória); `app/graficos.rs`.
- **Pronto:** confirmar com foto e fechar a pendência da memória de 29/10.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### DIN-3. #106: filtro de relatório some quando o campo está em Filtros e em Linhas; a lista abre longe do botão
- **Onde:** `dinamica_layout.rs` (posição das células de filtro, A1:B1), `app/dinamica_ui.rs` (lista do
  filtro); "Execução" aparece duas vezes (provavelmente espaço no fim do item: `trim` ao comparar).
- **Pronto:** com STATUS em Filtros e Linhas, A1 = STATUS e B1 = (Tudo) com o botão; a lista abre logo
  abaixo do botão; filtrar não tira o foco da tabela; foto `FEATHERSHEET_CAPTURAR_DINAMICA=filtro`.
- **Depende:** nada.  **Issue:** #106.  **Tamanho:** M.

### DIN-4. #107: menu do campo com "Mover para ›" e "Resumir por ›" mais limpo
- **Onde:** `app/dinamica_ui.rs` (menu da setinha do campo); `FEATHERSHEET_CAPTURAR_DINAMICA=menu|resumir`.
- **Pronto:** um item "Mover para ›" com submenu (Para cima, Para baixo, Filtros, Colunas, Linhas,
  Valores); cada item do submenu move o campo (teste da ação); foto.
- **Depende:** nada.  **Issue:** #107.  **Tamanho:** P/M.

### DIN-5. Dinâmicas recomendadas: fazer ou retirar (menu morto)
- **Onde:** `app/dinamica_ui.rs:187`, `:491`, `:530–532` (`Evento::Recomendadas`); `faixa_nativa_grupos.rs`
  (o botão); `app/ajuda.rs` (o tópico que promete o recurso).
- **Pronto (opção A, D5):** sugere 3 a 5 resumos a partir dos dados (campo de texto + número, campo de
  data), mostra a prévia, o clique cria a dinâmica; teste com dados sintéticos. **Opção B:** botão e
  tópico da Ajuda saem; teste que falha se houver item de menu sem ação. Regra 3 proíbe o estado atual.
- **Depende:** D5.  **Issue:** criar (bug: menu morto).  **Tamanho:** M (A) / P (B).

### DIN-6. Itens agrupados à mão e itens calculados
- **Onde:** `dinamica/pivot.rs` (agrupar), `dinamica/exibicao.rs` (já tem agrupar em faixas numéricas e
  datas: reaproveitar), `app/dinamica_ui.rs` (seleção de itens → Agrupar).
- **Pronto:** selecionar 2 itens → Agrupar cria um grupo; a soma do grupo confere com a soma manual;
  teste de core; foto.
- **Depende:** D6.  **Issue:** criar (buraco listado no METAS v0.1.6/v0.2).  **Tamanho:** G.

### DIN-7. Campo calculado com editor de fórmula e autocompletar
- **Onde:** `feathersheet-core/src/dinamica/campo_calculado.rs` (hoje só soma, no painel avançado);
  `app/dinamica_ui/` (a conferir: onde mora o painel avançado).
- **Pronto:** campo `=Preço*Quantidade` com autocompletar dos nomes dos campos; teste de resultado;
  foto do editor.
- **Depende:** D6.  **Issue:** criar.  **Tamanho:** G.

### DIN-8. Subtotais em cima, embaixo ou sem
- **Onde:** `dinamica/dinamica_layout.rs`, `dinamica/exibicao.rs` (Extras); hoje só liga/desliga.
- **Pronto:** três opções por campo; teste do layout compacto e tabular com cada uma.
- **Depende:** D6.  **Issue:** criar.  **Tamanho:** M.

### DIN-9. Estrutura de tópicos e recolher/expandir por nível (conferir)
- **Onde:** `dinamica/dinamica_layout.rs`; `recolhidos` em `app/dinamica_ui.rs`.
- **Pronto:** o layout de tópicos tem recolher por nível e teste; hoje a memória diz que
  "recolher/expandir" existe: confirmar só a parte "por nível" (a conferir).
- **Depende:** DIN-8.  **Issue:** criar se faltar.  **Tamanho:** M.

### DIN-10. Mostrar páginas de filtro (uma aba por item)
- **Onde:** `app/dinamica_ui.rs` (ação), `feathersheet-core/src/planilha/workbook.rs` (criar abas).
- **Pronto:** a ação cria uma aba por item do campo de filtro, com o recorte de cada um; teste de
  quantas abas e conteúdo.
- **Depende:** DIN-3.  **Issue:** criar.  **Tamanho:** M.

### DIN-11. Origem: intervalo, tabela, outra aba e outro arquivo aberto (conferir)
- **Onde:** `app/dinamica_ui.rs` (criação); `planilha/vinculo.rs` (referência entre arquivos abertos,
  já existente desde a v0.1.0).
- **Pronto:** quadro de confere: intervalo e outra aba (a conferir se já funcionam), tabela (`regiao_da_tabela`),
  outro arquivo aberto (usa o vínculo). Teste de cada um; o que faltar vira atividade própria.
- **Depende:** nada.  **Issue:** criar (se faltar).  **Tamanho:** M.

### DIN-12. Agregações: contagem de distintos, mediana e percentil
- **Onde:** `feathersheet-core/src/dinamica/pivot.rs:13` (`AgregacaoOp`) e `do_str`; `dinamica_ui.rs`
  (seletor de agregação).
- **Pronto:** `Distintos`, `Mediana` e `Percentil` no motor, com teste contra valores conhecidos (texto
  e vazio como no Excel); no painel; escrita no `.xlsx` (a conferir se a dinâmica gravada aceita).
- **Depende:** D18 (sem o "modelo de dados" do Excel).  **Issue:** criar.  **Tamanho:** M.

### DIN-13. Ctrl+Z de cada mudança no painel
- **Onde:** `Passo::Dinamicas` (existe, a conferir se cada ação do painel gera um passo).
- **Pronto:** arrastar campo, mudar agregação, mudar filtro: cada um desfaz com um Ctrl+Z; teste.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### DIN-14. Estilos da dinâmica que sobrevivem ao Atualizar
- **Onde:** `dinamica/` (configuração JSON na chave `extras`); `app/dinamica_visual.rs`.
- **Pronto:** escolher um estilo, Atualizar, o estilo continua; ida e volta `.fshz` e `.xlsx`.
- **Depende:** D6.  **Issue:** criar.  **Tamanho:** G.

### DIN-15. Segmentação de dados completa
- **Onde:** `app/dinamica_ui/segmentacao.rs` (hoje ligada a uma dinâmica só); leitor de `.xlsx` ignora a
  segmentação (METAS v0.2).
- **Pronto:** segmentação ligada a duas dinâmicas, seleção múltipla, lida e gravada no `.xlsx`; teste de
  ida e volta; LibreOffice/Excel (usuário) abre sem "conteúdo removido".
- **Depende:** D6, GRA-9 (o gráfico dinâmico usa a mesma segmentação).  **Issue:** criar.  **Tamanho:** G.

### DIN-16. Metas de número da dinâmica (medir, sem mudar código)
- **Onde:** bancada `medir_etapas_da_troca` (memória de 15/10): `FEATHERSHEET_PESADO=…_1M.fshz
  FEATHERSHEET_ABA=1 FEATHERSHEET_CAMPOS="2,3;6" cargo test --profile medir ...` (rodar pelo usuário,
  com limite de memória).
- **Pronto:** 1 M linhas, 2 campos de linha + 1 de valor ≤ 200 ms; arrastar campo ≤ 100 ms; 10 M
  linhas, 3 + 2 ≤ 1 s; atualizar após uma célula ≤ 50 ms; memória extra ≤ 10% da origem. Número
  registrado no F12 e no README (DOC-3). Os números que não baterem viram atividades próprias.
- **Depende:** DIN-19.  **Issue:** nenhuma.  **Tamanho:** M (medir).

### DIN-17. Atualizar só o grupo que mudou
- **Onde:** `dinamica/pivot.rs` (somas por grupo, METAS v0.1.4), `app/dinamica_ui.rs`.
- **Pronto:** mudar uma célula da origem refaz só o grupo afetado; ≤ 50 ms (DIN-16).
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** G.

### DIN-18. Adiar atualização do painel
- **Onde:** `app/dinamica_ui.rs` (arrumar campos e calcular no fim).
- **Pronto:** com a opção ligada, arrastar 3 campos recalcula uma vez; teste de contagem de cálculos.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### DIN-19. Arrastar campo responde em ≤ 100 ms até 1 M linhas
- **Onde:** `app/dinamica_ui.rs` (evento de arrasto) + `dinamica_layout.rs`.
- **Pronto:** medido pela bancada (DIN-16).
- **Depende:** DIN-17 ou DIN-18.  **Issue:** criar se não bater.  **Tamanho:** M.

---

## Frente 5 — Gráficos

### GRA-1. Gantt: linha do dia de hoje e pintar concluídas, em andamento e futuras (#133)
- **Onde:** `feather-charts` (`personalizacao.rs`, `cor_do_ponto`; tag nova `v0.3.5`);
  `crates/feathersheet/src/app/graficos.rs` (painel); `app/graficos/`; MCP em `sheet_grafico` (MCP-10).
- **Pronto:** foto do Gantt com linha "Hoje" ligável no painel e três cores calculadas pela data (não
  fixas); cor por barra no painel; `sheet_grafico` aceita as opções. Tag do feather-charts publicada.
- **Depende:** D7; a tag do feather-charts.  **Issue:** #133.  **Tamanho:** G.

### GRA-2. Rosca de verdade (#113)
- **Onde:** `feather-charts` (`EstiloGrafico`: espessura do anel, já em `EstiloPizza::espessura_anel`);
  `feathersheet-io` (leitor/gravador de `doughnutChart`, `holeSize`; a conferir o arquivo
  `xlsx_read_graficos*`/`xlsx_write_graficos.rs`).
- **Pronto:** foto de rosca com `holeSize 50`; o `.xlsx` gravado volta com `doughnutChart`; teste de ida e
  volta; o paliativo (pizza cheia) sai.
- **Depende:** tag do feather-charts.  **Issue:** #113.  **Tamanho:** M.

### GRA-3. Painel próprio por tipo de gráfico (#108)
- **Onde:** `crates/feathersheet/src/app/graficos/` (pasta existe); `app/graficos.rs`.
- **Pronto:** pizza com uma série (bug relatado: duas séries selecionadas ao abrir); painéis de dispersão,
  cascata e os outros tipos com as opções de cada um; foto de `FEATHERSHEET_CAPTURAR_GRAFICO=painel:<tipo>`
  para cada tipo.
- **Depende:** nada (GRA-1 e GRA-2 usam o mesmo painel: fazer antes o painel genérico).  **Issue:** #108.  **Tamanho:** G.

### GRA-4. Contorno dos dados de origem e alças para ajustar o intervalo (#121)
- **Onde:** `app/graficos.rs` (seleção); desenho sobre a grade (`grade/desenho_auxiliar.rs`, `src/geometria_overlay.rs`).
- **Pronto:** com o gráfico selecionado, roxo nos rótulos, azul nos valores, vermelho no cabeçalho, alças
  nos cantos que mudam o intervalo do painel; foto. Desenho aprovado antes (regra 21: descrever antes).
- **Depende:** D10 (confirmar o desenho com o usuário).  **Issue:** #121.  **Tamanho:** G.

### GRA-5. Trocar a categoria sem travar; contas da série pelo tipo
- **Onde:** `app/graficos.rs`; `feather-charts` (a conferir).
- **Pronto:** número: Soma, Média etc.; texto: só Contar e Contar distintos (regra do METAS v0.1.0); troca de
  categoria mede ≤ 16 ms na bancada `medir_graficos_ao_abrir_e_trocar`.
- **Depende:** GRA-3.  **Issue:** criar.  **Tamanho:** M.

### GRA-6. Gráfico com mais de 200 mil linhas, resumido (conferir)
- **Onde:** `app/graficos.rs` (o liga automático e "Não resumir" na lista).
- **Pronto:** conferir se a regra do METAS v0.1.0 ("acima de 200 mil células o resumo liga sozinho; lê até
  100 mi") está no código; se não, implementar; foto.
- **Depende:** nada.  **Issue:** criar se faltar.  **Tamanho:** P/M.

### GRA-7. Gráfico novo aparece na parte visível da planilha (conferir)
- **Onde:** `app/graficos.rs` (posição inicial).
- **Pronto:** com a planilha rolada até a linha 10.000, o gráfico novo aparece na área visível; foto.
- **Depende:** nada.  **Issue:** criar se faltar.  **Tamanho:** P.

### GRA-8. BLOCO no Eixo X: fechar ou reabrir (conferir)
- **Onde:** commit `5239b08` (bancada `medir_quadros_grafico_com_bloco`, 18–21 ms com e sem gráfico).
- **Pronto:** a memória de 29/10 diz "medido"; decidir se a pendência fecha (sem issue) e registrar.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### GRA-9. Gráfico dinâmico completo: botões de campo, todos os tipos, `.xlsx`
- **Onde:** `app/graficos.rs` e `app/dinamica_ui.rs`; `feathersheet-io` (gravar o gráfico dinâmico no `.xlsx`).
- **Pronto:** botões de campo no próprio gráfico (filtrar pela dinâmica); os tipos de gráfico todos
  disponíveis; ida e volta `.xlsx`; foto.
- **Depende:** D6, DIN-15 (segmentação compartilhada).  **Issue:** criar.  **Tamanho:** G.

### GRA-10. Excel remove o desenho da aba de gráficos (histograma e cascata) (#27)
- **Onde:** `feathersheet-io/src/xlsx_write_graficos.rs` (a conferir: nome exato); os `chartEx`.
- **Pronto:** o usuário abre no Excel os três arquivos `teste-1`, `teste-2`, `teste-3` (o caminho
  `teste-*.xlsx` está no corpo da issue) e a aba mantém os gráficos. Antes: o usuário diz qual abriu
  sem o gráfico; depois: arquivo gravado de novo e conferido.
- **Depende:** o usuário abre no Excel.  **Issue:** #27.  **Tamanho:** M.

---

## Frente 6 — Editor de célula e grade

O editor de célula com formatação (#90) está em PUB-9, por depender da cadeia da Feather Sans e do pelicano `v0.1.20`.

### EDI-1. Barra de rolagem horizontal não reconhece o fim com colunas ocultas
- **Onde:** `crates/feathersheet/src/grade/barras.rs:62–90` (`cursos`: `ultima_coluna` por índice,
  `vis_colunas` por colunas visíveis); `grade/barras.rs:98` e `:145` (uso).
- **Pronto:** reproduzir com um `.xlsx` de teste com A e H ocultas (aba Geral); a barra chega ao fim e
  para na última coluna com dados; teste que conta colunas ocultas. Pergunta ao usuário: qual barra e o
  que aconteceu (ver D21).
- **Depende:** D21.  **Issue:** criar (bug relatado em 29/10).  **Tamanho:** M.

### EDI-2. Número digitado como texto (indicador)
- **Onde:** `grade/` (canto da célula); regra em `planilha/` (a conferir).
- **Pronto:** célula com "1" como texto mostra o indicador e o aviso ao passar o mouse (foto);
  desligável nas Configurações. Desenho aprovado antes (regra 21).
- **Depende:** D22.  **Issue:** criar (paridade).  **Tamanho:** M.

---

### EDI-3. Dividir texto em colunas: todas as colunas, seleção em massa e tipo para várias (#134)
- **Onde:** `crates/feathersheet/src/app/assistente_divisao.rs` (`selecionadas`, `clicar_coluna`,
  `aplicar_tipo`, `Evento::Modificadores`/`SelecionarTodas`), `assistente_divisao/visual.rs`
  (`COLUNAS_SEM_ROLAGEM`, rolagem nos dois sentidos, barra "N colunas selecionadas"), `app.rs`
  (repasse do `ModifiersChanged`).
- **Pronto:** com 83 colunas a prévia mostra todas (rolagem lateral); clique, Ctrl+clique e Shift+clique
  selecionam; trocar o tipo vale para todas as selecionadas; teste
  `ctrl_e_shift_selecionam_varias_e_o_tipo_vale_para_todas`; foto conferida; Ajuda F1 (Ferramentas de dados)
  em pt/en/es.
- **Depende:** nada.  **Issue:** #134.  **Tamanho:** M. *(Em andamento em 30/10.)*

### EDI-4. Otimizar a divisão de texto em colunas (#134)
- **Onde:** `assistente_divisao.rs` (`TrabalhoDivisao`, lotes por tick), `feathersheet-core/src/operacoes/ops`
  (`OpcoesDivisao`, `TipoColunaDivisao`, divisão por separador/largura fixa).
- **Pronto:** roteiro de medição entregue ao usuário (dividir uma coluna em ~80 num arquivo de ~1 milhão
  de linhas, tempo e pico de memória no F12); o gargalo medido atacado (candidatos: análise de cada parte
  célula a célula, escrita por célula em vez de por coluna, tamanho do lote na thread da janela); tempo
  antes/depois registrado; a janela nunca trava (memória `nunca-travar-thread-da-janela`).
- **Depende:** EDI-3 (mesmo arquivo); medição do usuário.  **Issue:** #134.  **Tamanho:** M–G.
- **Feito em 30/10 (c6e306d):** o pool de textos não dobra mais no meio da gravação (estimativa numa
  thread + `StringPool::reservar` num tique só); bancada `medir_divisao_larga` (20 mil × 83): gravação
  0,69 → 0,47 s, tiques de 20–72 ms viram um de ~25 ms. **Falta:** medir no arquivo real do usuário; a
  gravação ainda é célula a célula na thread da janela (próximo passo: montar as colunas de destino
  em threads e só encaixar na janela).

---

## Frente 7 — Windows

### WIN-1. Receber o `dano.txt` do notebook Windows
- **Onde:** instruções já na memória `windows-notebook-empresa` (`set PELICANO_DANO=1`, `2> "%TEMP%\dano.txt"`).
- **Pronto:** o usuário manda o arquivo da 0.1.9 ou confirma que o "meio bugado" passou. Sem isso não
  há o que corrigir.
- **Depende:** o usuário.  **Issue:** criar (se o problema persistir).  **Tamanho:** P.

### WIN-2. Testar a 0.3.0 no notebook (instalador)
- **Onde:** `pacote/gerar.sh windows`, `pacote/feathersheet.nsi`; checklist: instalar, abrir uma
  planilha, Feather Sans, cantos da janela, tema automático (`system_dark`, a conferir no Windows), abrir
  `.xlsx`, Ctrl+S.
- **Pronto:** lista de verificação preenchida pelo usuário, sem erro bloqueante.
- **Depende:** LAN-5, PUB-8.  **Issue:** nenhuma.  **Tamanho:** M (do usuário).

### WIN-3. Job de Windows no CI (compilar o app)
- **Onde:** `.github/workflows/` (novo `windows.yml`, ou job em `macos.yml`); o `testes.yml` continua
  sem a GUI (regra da memória).
- **Pronto:** push em `main` compila `feathersheet` no `windows-latest` (sem rodar a suíte da GUI); job verde.
- **Depende:** PUB-6 (sem `[patch]`).  **Issue:** criar.  **Tamanho:** M.

### WIN-4. Pressão da caneta no Windows
- **Onde:** `feather_ui::caneta` (WM_POINTER); a janela de teste no notebook.
- **Pronto:** traço com a mesa mostra pressão; "conferido por cargo check" (14/10) passa a "conferido na
  máquina". Se a mesa não estiver no notebook, o item fica fora da 0.3.0 (D14).
- **Depende:** WIN-2; D14.  **Issue:** nenhuma.  **Tamanho:** M.

---

## Frente 8 — Segurança

### SEG-1. Token do socket MCP validado no app
- **Onde:** `crates/feathersheet-mcp/src/ao_vivo.rs:8–9, :70, :83` (token na primeira linha); lado do app
  `crates/feathersheet/src/app/mcp_vivo.rs` (a conferir a validação e a permissão do arquivo `porta token`).
- **Pronto:** teste: conexão sem o token certo é recusada; o arquivo com o token só é lido pelo dono
  (permissão 0600, a conferir o que existe); nota no `FORMATO`/MCP.
- **Depende:** nada.  **Issue:** criar (pendência da revisão de 29/10).  **Tamanho:** M.

### SEG-2. Pânicos nos eventos de macro não derrubam o app
- **Onde:** `crates/feathersheet/src/app/eventos_vba.rs`, `app/formulario_macro.rs` (sem `catch_unwind`);
  padrão já usado em `app/calculo_em_fundo.rs` e `app/macro_dialogo/em_fundo.rs`.
- **Pronto:** entrada ruim no evento (macro que lança erro, formulário com valor estranho) vira mensagem na
  barra de status; teste do caminho com entrada ruim.
- **Depende:** nada.  **Issue:** criar (crash: correção avulsa permitida pela regra 23).  **Tamanho:** P/M.

### SEG-3. Teto do corpo HTTP do OpenQuery (conferir)
- **Onde:** (a conferir) onde está o cliente HTTP do OpenQuery (`app/openquery_ui*` e o crate de dados).
- **Pronto:** corpo acima do teto é recusado com erro claro; teste.
- **Depende:** nada.  **Issue:** criar se não existir.  **Tamanho:** P.

### SEG-4. Profundidade de bloco no parser VBA
- **Onde:** `feathersheet-vba/src/sintaxe/` (parser de blocos `If`/`For`/`Select` aninhados); o teto de
  nós já existe em `sintaxe/expr.rs:18` (#129).
- **Pronto:** arquivo com 10.000 `If` aninhados termina com erro de conversão (não pânico nem estouro de
  pilha); teste; a revisão de 29/10 apontava a parte de blocos (a conferir se já está fechada).
- **Depende:** nada.  **Issue:** criar se faltar.  **Tamanho:** P.

### SEG-5. Revisão de segurança das macros (fechamento)
- **Onde:** `feathersheet-macros/src/riscos.rs`; `app/macro_dialogo/`.
- **Pronto:** lista dos #123–#130 conferida contra o código e fechada; "Ativar macros?" testado para
  arquivo com macro em `.xlsm` e `.fshz`.
- **Depende:** nada.  **Issue:** #123–#130 (já fechadas: só conferir).  **Tamanho:** P.

---

## Frente 9 — Paridade com outros programas (teste de 28/10)

### GRI-1. Roteiro comparativo com outro programa com dados pequenos
- **Onde:** a planilha é montada uma vez (`.xlsx`) e aberta nos dois.
- **Pronto:** tabela com abrir, recalcular e editar uma célula da base, nos dois, mais a conferência dos
  resultados (FILTRO, desvio padrão, INCLINAÇÃO) entre os dois e com o Excel. O usuário roda o pesado.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** M.

### GRI-2. Cálculo iterativo (referência circular com iterações)
- **Onde:** `feathersheet-core/src/planilha/calculo_manual.rs` / detecção de circular (a conferir o
  módulo); opções nas Configurações.
- **Pronto:** opção "Iterações" (máximo e diferença), com teste de convergência em fórmula circular
  simples; desligada por padrão. Decisão D17.
- **Depende:** D17.  **Issue:** criar.  **Tamanho:** G.

### GRI-3. Abrir `.xlsx` protegido por senha
- **Onde:** `feathersheet-io/src/` (leitor de `.xlsx`; a conferir se já há tratamento de `EncryptionInfo`).
- **Pronto:** pede a senha e abre; sem crate com C; teste com arquivo de senha conhecida. Decisão D14
  (dependência pura Rust, custo medido).
- **Depende:** D14.  **Issue:** criar.  **Tamanho:** G.

---

## Frente 10 — Interface, faixa, menus e idioma

### INT-1. Faixa recolhendo grupo a grupo em todas as abas
- **Onde:** `crates/feathersheet/src/app/faixa_responsiva.rs`, `faixa_nativa.rs`, `faixa_nativa_grupos.rs`
  (hoje só a Início; as outras usam o "Mais", pendência de 28/10).
- **Pronto:** foto de Inserir, Fórmulas e Dados recolhidas: cada grupo recolhe sozinho; teste de
  quantos grupos cabem por largura.
- **Depende:** nada.  **Issue:** nenhuma (pendência da memória).  **Tamanho:** M.

### INT-2. #68: faixa em três níveis (aberta, recolhida, micro com os menus na barra de título)
- **Onde:** `app/faixa_nativa.rs`, `app/menu_principal.rs`, `app/configuracoes_ui.rs`.
- **Pronto:** a escolha guarda nas Configurações; foto dos três níveis; Ajuda atualizada (DOC-1). Desenho
  já descrito na issue: aprovar antes (regra 21, D12).
- **Depende:** D12; INT-1.  **Issue:** #68.  **Tamanho:** G.

### INT-3. Menu morto: auditoria de todos os rótulos
- **Onde:** `dados/menus.json` (a conferir: a issue #82 cita `dados/menus.json` no menu clássico);
  `app/menu_principal.rs`, `app/menu_nativo.rs`, `app/menu_contexto.rs`.
- **Pronto:** teste que percorre todos os itens e falha se algum não tem ação; "Proteger intervalo…" e
  "Intervalos protegidos…" (#82) saem ou são implementados; a lista de itens mortos fica vazia.
- **Depende:** D12.  **Issue:** #82 (e criar para o que mais aparecer).  **Tamanho:** M.

### INT-4. Exibir: Nova Janela, Organizar Tudo, Dividir, Alternar Janelas (#83)
- **Onde:** `app/menu_principal.rs`, `app/mover_abas.rs`; janelas auxiliares do pelicano.
- **Pronto:** ou implementado (duas abas visíveis) ou fora do menu. Decidido em D12.
- **Depende:** D12.  **Issue:** #83.  **Tamanho:** G (implementar) / P (tirar).

### INT-5. Grupo Organizar para gráficos e imagens (#79)
- **Onde:** `app/objetos.rs`, `app/objetos/interface.rs` (a conferir: o grupo hoje só existe para formas).
- **Pronto:** Avançar, Recuar, Painel de Seleção, Alinhar, Agrupar, Girar para gráficos e imagens; teste
  de ordem Z; foto.
- **Depende:** D12.  **Issue:** #79.  **Tamanho:** G.

### INT-6. Temas da página (#80) e grupo Revisão (#82)
- **Onde:** `app/menu_principal.rs`; `feathersheet-io/src/` (tema do `.xlsx`, `xl/theme`).
- **Pronto:** D12 decide: temas da pasta do Excel (cores e fontes) ou sair do menu; a "Revisão" sem os
  itens que não existem sai do menu.
- **Depende:** D12.  **Issue:** #80, #82.  **Tamanho:** G (temas) / P (tirar).

### INT-7. Interface em inglês sem sobras em português
- **Onde:** `crates/feathersheet/src/idioma.rs`; `trf!`/textos fixos em `app/` (grep de palavras em pt
  nas telas); itens do METAS: "salvo", "Colar", grupos da Página Inicial, abas Exibir/Macros/Ferramentas,
  espessura "Média" da borda, linhas do motor no F12.
- **Pronto:** com o idioma en-US, foto das telas principais sem português; lista de palavras pt (grep)
  vazia nos textos de interface.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### INT-8. Migração do idioma: mensagens do Rust e descrições das funções
- **Onde:** `function_help.rs` (a conferir o caminho no `feathersheet-core`); `explicar_erro` (mensagens
  de erro).
- **Pronto:** mensagens e a ajuda das funções em pt, en e es, pelo mesmo arquivo de idioma; teste de
  chave faltando.
- **Depende:** D15.  **Issue:** criar.  **Tamanho:** G.

### INT-9. Nomes das funções por idioma
- **Onde:** `feathersheet-core` (o motor reconhece os nomes); o de-para em JSON (pt, en, es, já existe).
- **Pronto:** D15 decide. Se entrar: nomes localizados sem quebrar arquivos antigos (teste com `.fshz`
  antigo).
- **Depende:** D15.  **Issue:** criar.  **Tamanho:** G (ou fora).

### INT-10. Botão direito na guia da aba não mostra nada (conferir)
- **Onde:** `app/mover_abas.rs`, `app/menu_contexto.rs`.
- **Pronto:** clique direito na guia abre o menu de aba (renomear, cor, ocultar, mover); foto.
- **Depende:** nada.  **Issue:** criar se ainda acontecer.  **Tamanho:** P/M.

### INT-11. Menu Orientação do texto: ícones e posição
- **Onde:** `app/menu_orientacao.rs`.
- **Pronto:** ícones de orientação legíveis e o menu abre junto ao botão; foto.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** P.

### INT-12. Barra de progresso ao abrir fica em 100% em "Montando… na memória" (conferir)
- **Onde:** `app/progresso_abertura.rs`.
- **Pronto:** o progresso só chega a 100% quando termina a montagem; teste de estado.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** P.

### INT-13. `feathersheet --version` imprime a versão e sai
- **Onde:** `crates/feathersheet/src/main.rs` (antes de abrir a janela, junto do `--mcp` da linha ~80);
  `app/atualizacao.rs:21` (`CARGO_PKG_VERSION`).
- **Pronto:** `feathersheet --version` imprime `feathersheet 0.3.0` e sai sem abrir janela; teste ou
  conferência manual (o usuário roda no terminal).
- **Depende:** nada.  **Issue:** criar (pedido de 29/10).  **Tamanho:** P.

### INT-14. Cursor do Claude ao vivo (#9)
- **Onde:** `app/mcp_vivo.rs` (cada pedido chega no `update`), `grade.rs` (borda colorida do intervalo),
  `app/` (balão e histórico no F12).
- **Pronto:** borda e etiqueta "Claude" no intervalo; balão de uma linha que some; `sheet_comando` pisca o
  botão; histórico no F12. Sem a seta de mouse (proposta). Foto com MCP.
- **Depende:** D10.  **Issue:** #9.  **Tamanho:** G.

### INT-15. Janela com barra própria não redimensiona (#72): conferir e fechar
- **Onde:** `feather_ui::cromo::moldura_redimensionavel` (feather-ui v0.3.3, já no app).
- **Pronto:** redimensionar pelas bordas funciona no Linux (foto ou teste manual do usuário); fechar com
  `Fixes #72`.
- **Depende:** nada.  **Issue:** #72.  **Tamanho:** P.

---

## Frente 11 — MCP e IA

### MCP-1. Aba "IA (MCP)" nas Configurações
- **Onde:** `app/configuracoes_ui.rs` (o JSON com `--mcp` já é gerado em `:375`); `app/mcp_vivo.rs`.
- **Pronto:** ligar e desligar (desligado de fábrica); somente leitura; estado e contador de pedidos;
  comando de conexão para copiar; lista do que a IA pode fazer. Foto da aba; com o canal desligado não
  abre socket (teste).
- **Depende:** SEG-1.  **Issue:** nenhuma (METAS v0.1.0).  **Tamanho:** M.

### MCP-2. Instalador leva o `--mcp` sem instalar nada a mais (conferir)
- **Onde:** `pacote/gerar.sh`; `main.rs:80` já trata `--mcp` no binário principal.
- **Pronto:** a instalação do `.deb` e do `.exe` sem o `feathersheet-mcp` ainda deixa a IA conectar (teste
  manual no Linux; no Windows com WIN-2).
- **Depende:** LAN-5.  **Issue:** nenhuma.  **Tamanho:** P.

### MCP-3. Conciliar os erros de parâmetro do MCP
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs` e as ferramentas: `sheet_filtrar` só aceita letra em
  `coluna`; `sheet_estatistica` recusa `coluna: "G"` (quer `G:G`); `sheet_substituir` usa `texto`/`por`;
  `sheet_consulta` pede `{"tipo":"agrupar",…}` mas o texto mostra `agrupar{…}`.
- **Pronto:** cada ferramenta aceita as formas que o próprio esquema mostra (ou o esquema é corrigido);
  teste por ferramenta.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### MCP-4. `sheet_tela` mostra os painéis abertos
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs`; `app/mcp_vivo.rs` (estado da tela).
- **Pronto:** `sheet_tela` lista o seletor de gráfico e a formatação condicional quando abertos.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** P/M.

### MCP-5. `sheet_condicional` ao vivo não abre diálogo sem pedido (conferir)
- **Onde:** `app/mcp_vivo.rs`.
- **Pronto:** a chamada de leitura não abre janela; teste.
- **Depende:** nada.  **Issue:** criar se ainda acontecer.  **Tamanho:** P.

### MCP-6. Cobertura do MCP para o que a 0.3.0 entrega
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs`, `permissoes.rs`; `docs/planejamento/MCP-PENDENTE.md`
  (124 linhas, lista do que falta).
- **Pronto:** cada feature nova da 0.3.0 (dinâmica, gráfico, segmentação) tem ferramenta ou o motivo
  escrito em `MCP-PENDENTE.md`; o subconjunto escolhido em D23 testado no MCP.
- **Depende:** D23; DIN, GRA.  **Issue:** nenhuma.  **Tamanho:** G.

### MCP-7. Ferramentas MCP do Gantt
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs` (`sheet_grafico`).
- **Pronto:** aceita as opções de GRA-1 (linha de hoje, cor por barra).
- **Depende:** GRA-1.  **Issue:** #133 (item 4 da issue).  **Tamanho:** P.

---

## Frente 12 — Macros (fora dos itens de segurança)

### MAC-1. Autocompletar no editor de macros (#8)
- **Onde:** `app/macros/` (editor Alt+F11), `feathersheet-macros` (lista da API em inglês, `sheet.*`).
- **Pronto:** digitar `sheet.` mostra os métodos com assinatura e descrição do guia; `sh` → `sheet`,
  `sheet.r` → `range`; teste da lista (a lista vem do mesmo arquivo do guia).
- **Depende:** nada.  **Issue:** #8.  **Tamanho:** M.

### MAC-2. UserForm de um `.xlsm` real do usuário
- **Onde:** `feathersheet-vba/src/formulario.rs`; `importar_vba.rs::formulario_da_conversao`.
- **Pronto:** um `.xlsm` real do usuário (pedido ao usuário) abre e o formulário aparece no designer.
  Hoje só testado com arquivos do LibreOffice (CLAUDE.md).
- **Depende:** o usuário manda o arquivo.  **Issue:** nenhuma.  **Tamanho:** M.

---

## Frente 13 — Documentação e Ajuda F1

### DOC-1. Ajuda F1 em dia com tudo que a 0.3.0 muda (pt, en, es)
- **Onde:** `app/ajuda.rs`, `app/ajuda_macros.rs`, `app/ajuda_atalhos.rs`; o tópico de dinâmicas
  recomendadas (DIN-5) tem de refletir a decisão.
- **Pronto:** cada atividade que mudou interface tem tópico novo ou atualizado nos três idiomas; nenhum
  tópico promete o que o app não faz; revisão por lista (uma linha por tópico).
- **Depende:** as atividades que mudam interface.  **Issue:** nenhuma (regra 24).  **Tamanho:** M.

### DOC-2. Exportar a Ajuda F1 como Markdown (#71)
- **Onde:** `app/ajuda.rs`; `feather_ui::ajuda` (catálogo).
- **Pronto:** botão grava um `.md` escolhido pela pessoa, com funções, macros e atalhos no idioma do app;
  teste de conteúdo mínimo.
- **Depende:** DOC-1.  **Issue:** #71.  **Tamanho:** M.

### DOC-3. Comparação com o Excel publicada no README (mesmo arquivo, script para repetir)
- **Onde:** `README.md` (em inglês, regra da memória); script em `scripts/` ou `crates/feathersheet/examples/` (a conferir).
- **Pronto:** tabela no README com abrir, recalcular e salvar, do mesmo arquivo, e o script que repete a conta.
- **Depende:** DIN-16, ROB-7 (números finais).  **Issue:** nenhuma (METAS v0.2.0).  **Tamanho:** M.

### DOC-4. Conciliar `docs/planejamento/METAS-VERSOES.md`
- **Onde:** `docs/planejamento/METAS-VERSOES.md`.
- **Pronto:** (a) a numeração das versões reflete o que saiu (a v0.2 foi macros; a 0.3.0 é esta lista;
  FeatherQuery na 0.4); (b) itens que já saíram marcados `[x]` (ex.: "Inserir/excluir coluna com muitas
  fórmulas trava" está `[ ]` na parte 1 e `[x]` na v0.1.1); (c) cada `[ ]` tem atividade aqui ou está
  fora de escopo.
- **Depende:** D1.  **Issue:** nenhuma.  **Tamanho:** M.

### DOC-5. Dois roteiros: raiz `ROADMAP.md` e `docs/planejamento/ROADMAP.md`
- **Onde:** `/ROADMAP.md` e `docs/planejamento/ROADMAP.md` (a conferir se o da raiz é o público).
- **Pronto:** um só fica como fonte; o outro aponta para ele.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### DOC-6. CLAUDE.md: fila de trabalho e a seção da 0.3.0
- **Onde:** `/CLAUDE.md` (fila de trabalho desatualizada; seções de 0.1.x).
- **Pronto:** seção `## v0.3.0` no topo (como a v0.2.2); a fila antiga substituída pelo que sobra depois
  da 0.3.0.
- **Depende:** LAN-1.  **Issue:** nenhuma.  **Tamanho:** P/M.

---

## Frente 14 — Lançamento da v0.3.0

### LAN-1. Versão e nome
- **Onde:** `crates/feathersheet/Cargo.toml` (`version`); `snap/snapcraft.yaml` (depois, LAN-7); `pacote/`.
- **Pronto:** `version = "0.3.0"`; `grep -rn 0.2.2` só em histórico.
- **Depende:** D1, D2 (o nome da versão).  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-2. "O que há de novo" em pt-BR, en-US e es-ES
- **Onde:** `crates/feathersheet/src/app/novidades.rs` (testes `toda_versao_tem_traducao` e
  `versao_desta_compilacao_e_a_primeira_e_todas_desenham`).
- **Pronto:** a entrada 0.3.0 existe nos três idiomas; os dois testes passam (o usuário roda só esses).
- **Depende:** as atividades que entram na 0.3.0.  **Issue:** nenhuma.  **Tamanho:** M.

### LAN-3. Notas da release e seção do CLAUDE.md
- **Onde:** `CLAUDE.md` (pt); notas de release (en, sem rodapé "Generated with Claude Code"); modelo em
  `pacote/PUBLICAR.md`.
- **Pronto:** notas em inglês prontas; CLAUDE.md com a seção; revisão com o usuário.
- **Depende:** DOC-6, LAN-2.  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-4. Testes verdes no CI
- **Onde:** `.github/workflows/testes.yml` (core, io, macros, vba, mcp); `macos.yml`.
- **Pronto:** todos os jobs verdes no commit de lançamento. A GUI fica só com os testes pelo nome que o
  usuário roda (regra da memória).
- **Depende:** todas as atividades de código.  **Issue:** nenhuma.  **Tamanho:** P (verificação).

### LAN-5. Gerar os pacotes de uma cópia limpa da tag
- **Onde:** `pacote/gerar.sh` (num `git worktree` da tag; pacote precisa de `pacote/PUBLICAR.md` passo 2).
- **Pronto:** `.deb`, `.tar.gz`, `.exe` em `pacote/saida/0.3.0/`; o `.dmg` sai do CI do macOS (`macos.yml`).
  Build com `systemd-run … MemoryMax=6G MemorySwapMax=0 nice -n 19 taskset -c 0-3` (memória de builds).
- **Depende:** LAN-4, WIN-3 (se entrar).  **Issue:** nenhuma.  **Tamanho:** M.

### LAN-6. Release no GitHub
- **Onde:** `gh release create v0.3.0` (nomes sem versão, como em PUBLICAR.md passo 3); `SHA256SUMS`.
- **Pronto:** release com .deb, .tar.gz, .exe, SHA256SUMS e o .dmg do CI; notas em inglês.
- **Depende:** LAN-3, LAN-5; D4 (autorização).  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-7. Snap: beta e depois stable
- **Onde:** `snap/snapcraft.yaml` (version e URL do .deb); `pacote/PUBLICAR.md` passo 4.
- **Pronto:** `snapcraft pack` com o `.deb` da release; instalado com `--dangerous`; enviado ao beta e
  depois ao stable.
- **Depende:** LAN-6.  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-8. Repositório apt (`sudo apt install feathersheet`)
- **Onde:** a decidir (repositório `feather-works/apt` ou pasta no feathersheet); chave GPG.
- **Pronto:** D13 decidido; repositório publicado; `apt install feathersheet` instala a versão 0.3.0 numa
  máquina limpa (teste do usuário).
- **Depende:** D13; LAN-6.  **Issue:** nenhuma.  **Tamanho:** G.

### LAN-9. Conferência de uso com os arquivos do usuário (roteiro)
- **Onde:** `Planilha Pesada.fshz`, `2026_03.fshz`, `Gráficos Relatório Anual_FSFA_202.xlsx`,
  `docs/planejamento/video-v0.2/vendas-outubro.xlsx`.
- **Pronto:** o Claude entrega o roteiro (abrir, editar, salvar, reabrir, dinâmica, gráfico); o usuário
  roda e manda "ok" ou o erro; nenhum arquivo sai da máquina.
- **Depende:** LAN-5 (pacote ou binário do commit).  **Issue:** nenhuma.  **Tamanho:** M (do usuário).

---

## Fora do escopo da 0.3.0 (registrado, não é atividade)

- **FeatherQuery (antigo OpenQuery):** renomear (`openquery/` → `featherquery/`), editor visual de passos,
  consultas guardadas no `.fshz`, `.fquz`, MCP `sheet_consulta` → `query`. Issues: #93, #94 (e a
  renomeação implícita no #70). Proposto para a **0.4**, como o METAS (v0.3 antigo).
- **#70 conectores de banco e arquivos (Google Drive, PostgreSQL, SQLite, etc.):** depende do FeatherQuery;
  fica para depois. Os itens de Parquet (ordem 1 da issue) e SQLite podem ser avaliados antes (D1).
- **Fase 6 (`mmap`) e V5 (fórmula compilada)** do plano de memória: MEM-11.
- **Dinâmica de dinâmica** (a saída vira origem), **medidas entre campos (DAX sem DAX)** e tabelas
  relacionadas: 0.4 proposto (METAS v0.1.8).
- **Temas da página (#80)**, **grupo Organizar (#79)** e **Exibir: janelas (#83)**: depende de D12.
- **Macros: VBA ⇄ JS de volta**, eventos automáticos do VBA, formulários de ida e volta (METAS v0.4).
- **Funções faltantes (457 → 510)** e REGEX, LAMBDA (METAS v0.5).
- **Assinatura de macOS e Windows, instalador da suíte, leitor de tela** (METAS v0.9).

---

## Ordem sugerida de execução

1. **Higiene (um dia):** PUB-1, PUB-2 (sem decisão necessária).
2. **Bugs de crash e pequenos, em lote (regra 23):** SEG-2, INT-13, INT-15, SEG-4, ROB-11, DIN-2.
3. **Publicar a Feather Sans** (depende de D3 e D4): PUB-4 → PUB-5 → PUB-6 → PUB-7 → PUB-8 → PUB-9 (#90).
4. **Dados e desfazer:** ROB-2 → ROB-1 → ROB-3 (#65) → ROB-4 → ROB-5 → ROB-8 → ROB-7.
5. **Memória, tetos primeiro (sem mexer no produto):** MEM-1 → MEM-2 → MEM-3 → MEM-4 → MEM-5 → MEM-6 (medir).
6. **Memória, ganhos (depende de D9):** ROB-6 → ROB-9 → MEM-7 → MEM-8 → MEM-9 → MEM-10.
7. **Tabela dinâmica, bugs:** DIN-1 → DIN-3 (#106) → DIN-4 (#107) → DIN-5 (D5).
8. **Tabela dinâmica, buracos (depende de D6):** DIN-12 (D18) → DIN-13 → DIN-6 → DIN-7 → DIN-8 → DIN-9 → DIN-10 → DIN-11 → DIN-14 → DIN-17 → DIN-18 → DIN-19 → DIN-16 (medir).
9. **Gráficos:** tags do feather-charts → GRA-2 (#113) → GRA-3 (#108, painel genérico) → GRA-5 → GRA-6 → GRA-7 → GRA-8 → GRA-1 (#133) → GRA-4 (#121, D10-anexa) → GRA-9 → GRA-10 (#27, usuário no Excel).
10. **Grade e faixa:** EDI-3 → EDI-4 (#134) → EDI-1 (barra de colunas ocultas, D21) → INT-1 → INT-2 (#68, D12) → INT-3 (#82) → INT-4 (#83) → INT-5 (#79) → INT-6 (#80).
11. **MCP e IA:** SEG-1 → MCP-1 → MCP-3 → MCP-4 → MCP-5 → MCP-6 → MCP-7 → INT-14 (#9, D10).
12. **Segurança restante e macros:** SEG-3 → SEG-5 → MAC-1 (#8).
13. **Paridade:** GRI-1 (roteiro, usuário) → GRI-2 (D17) → EDI-2 (D22) → GRI-3 (D14).
14. **Idioma e textos:** INT-7 → INT-8 e INT-9 (D15) → INT-10 a INT-12.
15. **Windows:** WIN-1 (usuário) → WIN-3 (job no CI) → WIN-2 (usuário) → WIN-4 (D14).
16. **Documentação:** DOC-4 (cedo, para o roteiro ficar certo) → DOC-5 → DOC-1 (durante cada item) → DOC-2 (#71) → DOC-3 (depois de DIN-16 e ROB-7).
17. **Lançamento:** DOC-6 → LAN-1 → LAN-2 → LAN-3 → LAN-4 → LAN-5 → WIN-2 e LAN-9 (usuário) → LAN-6 → LAN-7 → LAN-8 (D13, se entrar).

---

## Decisões que o usuário precisa tomar antes

- **D1 — Escopo da 0.3.0.** Confirmar a proposta (robustez, memória, dinâmica, gráficos pedidos, Windows,
  segurança, Feather Sans, #90) e que o FeatherQuery sai para a 0.4. Bloqueia DOC-4, LAN-1.
- **D2 — Número da versão.** 0.3.0 (como pedido) ou 0.2.3. Bloqueia LAN-1.
- **D3 — Publicar pelicano `v0.1.21` e feather-ui `v0.3.10` (tags e push)**. Bloqueia PUB-4 e PUB-5.
- **D4 — Enviar os 7 commits do `main` e o `editor-alinhado`.** A regra da memória é "publicar exige o suba
  do usuário". Bloqueia PUB-3, LAN-6.
- **D5 — Dinâmicas recomendadas: fazer (3 a 5 resumos) ou retirar o botão e o tópico da Ajuda.** Bloqueia DIN-5.
- **D6 — Quais buracos da dinâmica entram na 0.3.0.** Itens calculados e agrupamento à mão (DIN-6, DIN-7),
  subtotais em cima/embaixo (DIN-8), páginas de filtro (DIN-10), estilos (DIN-14), segmentação completa
  (DIN-15), gráfico dinâmico completo (GRA-9). Bloqueia DIN-6 a DIN-15 e GRA-9.
- **D7 — Gantt (#133) na 0.3.0?** Exige tag nova do feather-charts. Bloqueia GRA-1 e MCP-7.
- **D8 — Nível de compressão do `.fshz`.** Nível 6 (43 s, 359 MB), 3 (20 s, 458 MB) ou 1 (13 s, 702 MB).
  Também: qual o teto de tempo do desfazer da estrutura (ROB-4). Bloqueia ROB-9.
- **D9 — Quais fases do plano de memória entram na 0.3.0** (Fase 5, Fase 3 XLSX, gravar em fluxo, V4, Fase 6
  `mmap`, V5). A sugestão é entrar Fase 5, Fase 3 e gravar em fluxo; `mmap` e V5 ficam para a 0.4. Bloqueia
  MEM-7 a MEM-11.
- **D10 — "Cursor do Claude" (#9) e o destaque do gráfico (#121): aprovar o desenho.** A seta de mouse animada
  fica de fora (proposta sem resposta do usuário). Bloqueia INT-14, GRA-4.
- **D11 — "Arrastar fórmula para a planilha inteira" (padrão por faixa + valor sob demanda): aprovar o desenho.**
  Bloqueia ROB-10.
- **D12 — Faixa em três níveis (#68), menus de Exibir (#83), grupo Organizar (#79), temas (#80) e Revisão
  (#82): implementar ou tirar do menu.** Regra 3 pede a segunda opção quando não há ação. Bloqueia INT-2 a INT-6.
- **D13 — Repositório apt: separado (`feather-works/apt`) ou dentro do feathersheet, e chave GPG.** Bloqueia LAN-8.
- **D14 — Windows e dependências:** (a) o usuário manda o `dano.txt` (WIN-1); (b) CI com job de Windows
  (WIN-3); (c) senha de `.xlsx` (GRI-3) exige crate pura Rust, com custo medido; (d) pressão de caneta no
  Windows entra na 0.3.0 só se a mesa estiver no notebook (WIN-4).
- **D15 — Idioma: fases 5 e 6** (mensagens do Rust, descrições das funções, nomes localizados). Função de
  motor, a decisão é do usuário (memória e CLAUDE.md). Bloqueia INT-8, INT-9.
- **D16 — Snap: publicar na loja como stable na 0.3.0 ou só beta?** Bloqueia LAN-7.
- **D17 — Cálculo iterativo entra na 0.3.0?** Bloqueia GRI-2.
- **D18 — Distintos, mediana e percentil (além do Excel) entram na 0.3.0?** Bloqueia DIN-12.
- **D19 — Arquivos não rastreados:** `.claude/` (ignorar ou commitar o que for compartilhado), e os
  dois arquivos de `docs/planejamento/video-v0.2/` (commitar ou manter fora). Bloqueia PUB-1.
- **D20 — Alocador de teste:** copiar o módulo entre crates ou criar um crate de teste comum. Bloqueia MEM-1 (escolha de forma).
- **D21 — Barra de rolagem (EDI-1):** qual barra (horizontal da aba Geral?), e o que a pessoa fez antes
  (rolou até onde). Bloqueia EDI-1.
- **D22 — Número armazenado como texto (EDI-2):** entra na 0.3.0 e com que aviso (desenho, regra 21).
- **D23 — Subconjunto do MCP para a 0.3.0** (MCP-6): quais itens de `MCP-PENDENTE.md` entram.

---

## Itens que precisam de issue nova (regra 23), sem criar aqui

Conferir antes de abrir, e não duplicar issue existente (`gh issue list`):

| Assunto | Atividade | Motivo |
|---|---|---|
| Fórmulas G–M trocadas de coluna | ROB-1 | bug de 16/10 sem issue |
| Excluir a 1ª linha de intervalo | ROB-2 | bug de 16/10 sem issue (se reproduzir) |
| Mesclar/desmesclar fora do Ctrl+Z | ROB-5 | pendência de 28/10 |
| SES com critério vazio sem índice | ROB-8 | trava o teste de Ctrl+D |
| Arrastar fórmula para a planilha inteira | ROB-10 | pedido de 29/10 |
| Tetos de memória (alocador) | MEM-1 a MEM-5 | pedido de 29/10 |
| DATA da dinâmica (número/texto) | DIN-1 | relato de 29/10 |
| Agrupar itens e campo calculado | DIN-6, DIN-7 | buracos listados |
| Dinâmicas recomendadas sem ação | DIN-5 | menu morto (regra 3) |
| Ctrl+Z do painel e atualização por grupo | DIN-13, DIN-17 | buracos listados |
| Barra de rolagem com colunas ocultas | EDI-1 | relato de 30/10 |
| Pânicos em eventos de macro | SEG-2 | crash (correção avulsa) |
| Token do socket MCP | SEG-1 | revisão de 29/10 |
| `feathersheet --version` abre a janela | INT-13 | relato de 29/10 |
| Job de Windows no CI | WIN-3 | falta no CI |
| Contagem de distintos, mediana, percentil | DIN-12 | buraco listado |
| Número como texto | EDI-2 | paridade |
| Cálculo iterativo | GRI-2 | paridade |
| Senha em `.xlsx` | GRI-3 | paridade |
| Sincronia MCP: parâmetros e tela | MCP-3, MCP-4 | relato de 19/10 |
| Feather Sans: teste do pelicano | PUB-4 | teste quebrado pela mudança de fonte |

Issues existentes que esta lista fecha: #90 (PUB-9), #72 (INT-15), #65 (ROB-3), #106 (DIN-3), #107 (DIN-4),
#108 (GRA-3), #113 (GRA-2), #121 (GRA-4), #133 (GRA-1), #27 (GRA-10), #9 (INT-14), #8 (MAC-1), #68 (INT-2),
#71 (DOC-2), #79 (INT-5), #80 e #82 (INT-6, INT-3), #83 (INT-4). Fora da 0.3.0: #70, #93, #94.

---

## Pontos que ficaram sem confirmação (a conferir antes de usar)

- Base do ramo `feather-sans` da feather-ui e a dependência dela pelo pelicano (PUB-5).
- Qual dos caminhos de worktree o `[patch]` deve usar, e se ainda existe (PUB-6).
- Se a validação do token MCP já está no app (SEG-1) e a permissão do arquivo do token.
- Onde está o HTTP do OpenQuery e se o teto do corpo já existe (SEG-3).
- Se o parser VBA já limita a profundidade de blocos (SEG-4).
- Se `calculo_em_fundo.rs` já cobre o caso do `MÍNIMOSES` com Esc (ROB-6).
- Se a cópia para gravar já saiu da thread da janela na v0.2.1 (ROB-7).
- O que já existe de origem da dinâmica (DIN-11) e o "recolher por nível" (DIN-9).
- Se o gráfico novo e o resumo de 200 mil linhas estão no código (GRA-6, GRA-7).
- Se as metas `[ ]` do METAS v0.1.0 e v0.1.1 já saíram sem marcar (DOC-4).
- Se o instalador leva o `--mcp` sem o `feathersheet-mcp` (MCP-2).
- Os textos fixos em português ainda restantes (INT-7): a varredura não foi feita.
- Se o `system_dark` do pelicano funciona no Windows (WIN-2).
- Se o `.xlsx` com senha já tem tratamento (GRI-3).
- Caminho exato do exemplo `conferir_estrutura_pequena.rs` e do módulo de leitura do `.xlsx` (ROB-1, MEM-8).
- Não rodei nenhum `cargo`, teste ou o aplicativo; as metas de número dependem da medição do usuário.
