# Atividades até a v0.3.0 — FeatherSheet

> Documento interno de planejamento. Estado de referência: **30/10/2026**, depois da v0.2.2
> (publicada em 29/10/2026). Não é resumo: cada item é uma tarefa.
> **Atualizado em 08/11/2026** (depois da v0.2.9 e do lote seguinte): cada item tem a linha
> `**Situação (08/11):**` (✅ feito, 🟡 parcial, ❌ falta, ⛔ obsoleto); o resumo vem logo abaixo.
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

## Corte de escopo (08/11/2026, pedido do usuário: "falta 64 ainda")

48 itens marcados **⏭️ 0.4.0**: memória (MEM-1–5, 7, 8, 11; MEM-6, 9 e 10 voltaram a pedido do usuário), dinâmica avançada (DIN-6–11, 14, 16–19), interface grande
(INT-1, 2, 4–6, 8, 9, 11, 14), Windows que pede o notebook (WIN-2–4), paridade (GRI-1–3), gráficos (GRA-5, 9),
MCP-6/7, DOC-3, ROB-6/7/10, Feather Sans (PUB-4–8) e MAC-2. Ficam na 0.3.0: 29 ✅, 15 🟡 (quase todos só
conferir) e 20 ❌, dos quais 9 são o lançamento (LAN-1–9). Os ⏭️ não entram na contagem da 0.3.0.

## Resumo em 08/11/2026

**Contagem (115 atividades):** ✅ feito 21 · 🟡 parcial 21 · ❌ falta 72 · ⛔ obsoleto 1.

A maior parte do que andou desde 30/10 veio de outro lado: macros do Excel (v0.2.3–v0.2.8), memória ao abrir
(v0.2.5–v0.2.6), minigráficos e auditoria (v0.2.9) e, depois da v0.2.9, o OpenQuery com Power Query, a
AutoConclusão, as propriedades e a configuração de página. Das frentes deste plano, saíram o desfazer da
estrutura (#65), o editor alinhado (#90), os quatro gráficos pedidos em issue (#108, #113, #121), os dois bugs
da dinâmica (#106, #107) e o Dividir texto (#134). Memória, dinâmica (buracos), Windows, segurança e
lançamento quase não andaram.

**Atenção para o escopo (D1):** o plano tirava o FeatherQuery da 0.3.0, mas o lote depois da v0.2.9 já é,
em boa parte, OpenQuery/Power Query. A próxima versão sai com ele de qualquer jeito.

### O que falta, por valor para quem usa

Do mais valioso para o menos (P até meio dia, M um a três dias, G mais de três dias):

1. **SEG-2** (P/M): pânico num evento de macro ou formulário derruba o app e perde o trabalho.
2. **ROB-2** (P/M): excluir a 1ª linha de um intervalo pode estragar a fórmula (`#REF!`); dado errado calado.
3. **ROB-1** (P, se for o #45): confirmar que as fórmulas G–M não trocam mais de coluna no arquivo do usuário.
4. **DIN-1** (M): a coluna DATA da dinâmica sai como número no começo e como texto no fim.
5. **DIN-5** (P para tirar, M para fazer): botão "Dinâmicas recomendadas" que não faz nada, e a Ajuda o promete.
6. **ROB-8** (M): SES com critério vazio ou zero fica sem índice e trava em milhões de linhas.
7. **EDI-1** (M): a barra de rolagem não chega ao fim com colunas ocultas.
8. **SEG-3** (P): fonte Web do OpenQuery sem teto de tamanho (pode esgotar a memória).
9. **GRA-1** (G): Gantt com linha de hoje e cores por situação (#133, pedido do usuário).
10. **DIN-15** (G): segmentação de dados completa e linha do tempo (#145).
11. **MAC-1** (M): autocompletar no editor de macros (#8).
12. **ROB-10** (G): arrastar fórmula para a planilha inteira sem travar (pedido de 29/10; D11).
13. **INT-7** (M): interface em inglês sem sobras em português.
14. **PUB-4 → PUB-5 → PUB-7 → PUB-8** (M no conjunto): Feather Sans na interface.
15. **DIN-12** (M): contagem de distintos, mediana e percentil na dinâmica.
16. **INT-1** (M): faixa recolhendo grupo a grupo em todas as abas.
17. **ROB-6** (M; P se só medir): recálculo grande em fundo com Esc, a conferir com o `MÍNIMOSES`.
18. **ROB-7** (P): medir a cópia para gravar no `2026_03.fshz`.
19. **EDI-4** (M–G): Dividir texto: medir no arquivo real e montar as colunas fora da thread da janela.
20. **GRI-2** (G): cálculo iterativo (D17).
21. **EDI-2** (M): indicador de número digitado como texto (D22).
22. **DIN-13** (M): conferir um Ctrl+Z por ação no painel da dinâmica.
23. **DIN-8** (M): subtotais em cima, embaixo ou sem.
24. **DIN-6** (G): agrupar itens à mão.
25. **DIN-7** (G): campo calculado com editor e autocompletar.
26. **DIN-10** (M): mostrar páginas de filtro.
27. **DIN-9** (M): recolher/expandir por nível.
28. **DIN-11** (M): conferir cada origem da dinâmica.
29. **DIN-14** (G): estilos da dinâmica que sobrevivem ao Atualizar.
30. **DIN-18** (M): adiar atualização do painel.
31. **DIN-17** (G): atualizar só o grupo que mudou.
32. **DIN-19** (M): arrastar campo em ≤ 100 ms até 1 M linhas.
33. **GRA-9** (G): gráfico dinâmico completo (botões de campo, `.xlsx`).
34. **GRA-5** (M): trocar categoria sem travar; contas pela natureza da série.
35. **GRA-10** (M): #27, o usuário abre no Excel e confirma.
36. **GRI-3** (G): abrir `.xlsx` com senha (D14).
37. **MEM-7** (G): resultado direto na coluna, sem `calc.cache` (D9).
38. **MEM-8** (G): `.xlsx` com dados tipados (D9).
39. **MEM-9** (G): gravar em fluxo, sem o pico de +360 MB (D9).
40. **MEM-1** (M): alocador de teste com teto (base de MEM-2 a MEM-5).
41. **MEM-2** (M): teto em arrastar/colar fórmula até o fim.
42. **MEM-3** (M): teto ao abrir `.xlsx`/`.fshz`.
43. **MEM-4** (M): teto em dinâmica e gráfico.
44. **MEM-5** (M): teto em compactar, salvar e reabrir.
45. **MEM-10** (P/M): F12 e Monitor do GNOME com o mesmo número.
46. **MEM-6** (P, do usuário): medir a memória atual no app.
47. **SEG-1** (P): arquivo do token com 0600 e teste de conexão recusada.
48. **SEG-5** (P): conferência escrita das #123–#130 e teste do "Ativar macros?".
49. **INT-3** (M): teste que acha item de menu sem ação em todos os menus.
50. **INT-6** (G temas / P tirar): Temas (#80) e o resto da Revisão (#82) (D12).
51. **INT-4** (G / P): Exibir: Nova Janela, Organizar Tudo, Dividir (#83) (D12).
52. **INT-5** (G): grupo Organizar para gráficos e imagens (#79) (D12).
53. **INT-2** (G): faixa em três níveis (#68) (D12).
54. **INT-14** (G): cursor do Claude ao vivo (#9) (D10).
55. **INT-8** (G): mensagens do Rust em en/es (D15).
56. **INT-9** (G): nomes das funções em espanhol (D15).
57. **INT-10** (P): conferir o botão direito na guia da aba.
58. **INT-11** (P): ícones e posição do menu Orientação do texto.
59. **INT-12** (P): conferir a barra de progresso ao abrir.
60. **INT-13** (P): `feathersheet --version`.
61. **WIN-1** (P, do usuário): confirmar que o problema do notebook era o #135.
62. **WIN-3** (M): job de Windows no CI.
63. **WIN-2** (M, do usuário): testar a versão nova no notebook.
64. **WIN-4** (M): pressão da caneta no Windows (D14).
65. **MAC-2** (M): UserForm de um `.xlsm` real do usuário.
66. **MCP-6** (G): MCP para o que a versão nova entrega (D23).
67. **MCP-7** (P): opções do Gantt no `sheet_grafico` (depois de GRA-1).
68. **MCP-2** (P): conferir o `--mcp` no `.deb` e no `.exe` instalados.
69. **GRA-8** (P): registrar o fechamento do BLOCO no Eixo X.
70. **GRI-1** (M): roteiro comparativo com outro programa.
71. **ROB-9** (P): nível de compressão do `.fshz` (D8).
72. **ROB-11** (P): renomear a constante de dimensões ocultas.
73. **DIN-16** (M): medir as metas de número da dinâmica.
74. **DOC-1** (M): F1 com AutoConclusão e sem prometer as recomendadas.
75. **DOC-4** (M): METAS com a numeração certa.
76. **DOC-3** (M): comparação com o Excel no README.
77. **DOC-5** (P): apontar o `docs/planejamento/ROADMAP.md` para o da raiz.
78. **DOC-6** (P/M): seção da versão nova no CLAUDE.md.
79. **PUB-1** (P): tirar os arquivos soltos.
80. **PUB-2** (P): apagar os ramos já no `main`.
81. **LAN-1 → LAN-9** (P a G): lançamento (versão, novidades, notas, CI, pacotes, release, snap, apt, conferência).
82. **MEM-11** (G): `mmap` e fórmula compilada (fora da 0.3.0).

### Decisões que o usuário precisa tomar antes (ainda abertas)

- **D1 — Escopo:** precisa ser revista, porque o OpenQuery/Power Query já entrou no lote depois da v0.2.9.
- **D2 — Número da versão:** 0.3.0 ou mais uma 0.2.x (0.2.10).
- **D3 — (reformulada) Feather Sans:** publicar no pelicano do crates.io (0.1.3x) e na feather-ui v0.3.16, e
  decidir se o app sobe para o pelicano 0.1.31 (muda o Tab entre campos).
- **D5** dinâmicas recomendadas (fazer ou tirar) · **D6** buracos da dinâmica · **D7** Gantt (#133) ·
  **D8** compressão do `.fshz` · **D9** fases do plano de memória.
- **D10 — só a parte do cursor do Claude (#9):** o destaque do gráfico (#121) já foi feito.
- **D11** arrastar fórmula para a planilha inteira · **D12** faixa em 3 níveis, Exibir, Organizar, Temas e o
  resto da Revisão (a parte "Proteger intervalo" já saiu do menu) · **D13** repositório apt (e agora também
  se o Flathub entra, há manifesto pronto).
- **D14 — (b) CI de Windows, (c) senha de `.xlsx`, (d) caneta no Windows.** A (a) parece resolvida pelo #135.
- **D15** idioma (mensagens do Rust, nomes em espanhol) · **D17** cálculo iterativo · **D18** distintos,
  mediana e percentil.
- **D19 — arquivos não rastreados:** os de `video-v0.2/`, os `.snap` da raiz e o `pacote/flatpak/cargo-sources.json`
  (o `.claude/` já não aparece).
- **D20** alocador de teste · **D21** qual barra de rolagem · **D22** número como texto · **D23** subconjunto do MCP.

**Não valem mais:** D4 (tudo enviado; o `editor-alinhado` foi superado pelo `d697473`) e D16 (o snap já vai
para stable desde a 0.2.3, pelo `PUBLICAR.md`).

---

## Estado verificado hoje (fatos de partida)

> Reescrito em 08/11/2026. O estado de 30/10 (o `[patch]` para `/tmp`, os 7 commits sem push, o ramo
> `feather-sans` no app) não vale mais.

- **Versão:** `crates/feathersheet/Cargo.toml` diz `0.2.9`. Tags v0.2.3 a v0.2.9 publicadas, e o snap acompanha
  (0.2.8, 0.2.9). O `main` local é igual ao `origin/main` (nada sem push).
- **Dependências:** pelicano `0.1.30` do **crates.io** (travado no `Cargo.lock`; a 0.1.31 muda o Tab entre
  campos, conferir na grade antes de subir); feather-ui tag `v0.3.15`; feather-charts `v0.3.4`; feather-io
  `v0.2.1`. **Nenhum `[patch]`.**
- **Feather Sans:** não está no app. A API não existe no pelicano 0.1.30 nem no 0.1.31, e o ramo `feather-sans`
  da feather-ui segue só local. A tentativa antiga está no stash `e7a5c3e`.
- **No `main` depois da v0.2.9, ainda sem versão:** Ctrl+Z de inserir/excluir linhas e colunas (#65,
  `549db94`), editor com o alinhamento da célula (#90, `d697473`), AutoConclusão (`3223ff5`),
  configuração de página no arquivo (`1c75649`), "Salvar como .xlsx" (`47fa907`), Configurações ▸ Seus dados
  (`5666adc`), Propriedades em Menu ▸ Informações (`c6c06ce`), Ctrl na alça e "Formatar células…"
  (`e0ac596`), macros (SpecialCells, InputBox, AdvancedFilter, CopyPicture, Tab do formulário), o
  **OpenQuery bem mais completo, com Power Query ↔ OpenQuery** (`9700ea5`, `28fcf67` e anteriores) e as
  correções #147–#153. Linux compilado no Debian 12 (`93e9b24`).
- **Não commitado agora:** `feathersheet-io/src/xlsx_write.rs`, `examples/medir_exportar.rs` e o manifesto do
  Flatpak (trabalho de outra sessão: não mexer).
- **Issues abertas (18):** #8, #9, #27, #68, #70, #79, #80, #82, #83, #93, #94, #133 e as novas #141–#146
  (idiomas, recálculo, bateria de macros, integrações, segmentação/linha do tempo, complementos).
  Fechadas desde 30/10: #65, #71, #72, #90, #106, #107, #108, #113, #121, #134–#140, #147–#153.
- **Ainda verdade:** sem `#[global_allocator]`; `--version` não tratado; `eventos_vba.rs` e
  `formulario_macro.rs` sem `catch_unwind`; CI só com `macos.yml` e `testes.yml`; `AgregacaoOp` sem
  distintos/mediana/percentil; a flag `recomendacoes` nunca lida; `MAX_OPERACOES` local em `edicao.rs:1718`.
- **Novo achado:** a fonte Web do OpenQuery (`openquery_ui/importador.rs::ler_web`) baixa sem teto de tamanho
  (SEG-3).

---

## Frente 1 — Publicação pendente e higiene do repositório

### PUB-1. Tirar os arquivos soltos da raiz e de `crates/feathersheet`
**Situação (08/11):** ❌ falta. Continuam soltos: `crates/feathersheet/0`, `feathersheet_0.2.2/0.2.8/0.2.9_amd64.snap` na raiz, os dois arquivos de `video-v0.2/` e, novo, `pacote/flatpak/cargo-sources.json`. O `.claude/` já não aparece no `git status`.
- **Onde:** `crates/feathersheet/0`; `feathersheet_0.2.2_amd64.snap` (raiz); `.claude/`;
  `docs/planejamento/video-v0.2/banner-featheroffice.png`, `vendas-outubro.xlsx`.
- **Pronto:** `git status` sem `??` indesejado. O `0` apagado; o `.snap` movido para `pacote/saida/`
  ou ignorado; `.claude/` decidido (D19); os dois arquivos de vídeo commitados ou ignorados (D19).
- **Depende:** D19.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-2. Limpar ramos e worktrees locais
**Situação (08/11):** ❌ falta. São 9 ramos locais. Sem nada fora do `main` (podem ser apagados): `claude-pelicano-0127`, `rel-0.2.4` e 4 `worktree-agent-*` (`a46ff…`, `a4b03…`, `a6b02…`, `a750d…`). Com 1 commit fora: `editor-alinhado` (`e5bf09b`, superado por `d697473`) e `worktree-agent-a4d3…` (a conferir). Worktree solta: `.wt/sheet-0127` (parada em `2f8581d`).
- **Onde:** `git branch`, `git worktree list`; ramos `worktree-agent-a46ff…`, `…a4b03…`, `…a4d3…`,
  `…a6b02…`, `…a750d…`; worktrees em `/tmp/.../scratchpad/` e `.wt/`.
- **Pronto:** para cada ramo, `git branch --contains` confirma que o commit já está no `main` ou em outro
  ramo; ramos órfãos apagados; `git worktree list` só com os necessários.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-3. Enviar os 7 commits locais do `main`
**Situação (08/11):** ✅ feito. `git log origin/main..main` está vazio; tudo enviado, com as releases v0.2.3 a v0.2.9.
- **Onde:** `git log origin/main..main` (7 commits, do `7fa0e43` ao `c7dc116`).
- **Pronto:** `git log origin/main..main` vazio. Antes, conferir que a Ajuda F1 (`c7dc116`) está
  coerente com o que o app faz (ver DIN-5 e DOC-1).
- **Depende:** D4 (o usuário autoriza o envio: a regra da memória é "publicar exige o suba do usuário").
- **Issue:** nenhuma.  **Tamanho:** P.

### PUB-4. Publicar pelicano `v0.1.21` (Feather Sans)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. A Feather Sans não chegou ao pelicano publicado: nem a 0.1.30 nem a 0.1.31 do crates.io têm `set_default_family`/`Font::SYSTEM_SANS`. O número `v0.1.21` já passou: agora seria uma 0.1.3x no crates.io (ver D3).
**Situação (11/11):** ✅ feito. pelicano **0.1.33** no crates.io (tag `v0.1.33`), com `set_default_family` e `Font::SYSTEM_SANS`; suíte do pelicano verde.
- **Onde:** repositório do pelicano, ramo `fonte-padrao`, commit `60b80f6` (`set_default_family`,
  `Font::SYSTEM_SANS`). Cópia de trabalho: `/run/media/maragon/Arquivos/GitHub/pelicano`.
- **Pronto:** tag `v0.1.21` no remoto; o teste `clique_na_caixinha_marca_e_o_cursor_fica` (quebrado pela
  mudança de fonte, memória de 28/10) corrigido e verde no próprio pelicano (rodado pelo usuário, com
  limite de memória).
- **Depende:** D3 (autorização de tag/push).  **Issue:** nenhuma.  **Tamanho:** M.

### PUB-5. Publicar feather-ui `v0.3.10` (módulo `fonte`)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. O ramo `feather-sans` continua só local na feather-ui; a v0.3.15 (usada hoje) não tem o módulo `fonte`. A próxima seria a v0.3.16.
**Situação (11/11):** ✅ feito. feather-ui **v0.3.16** (tag), módulo `fonte`, com o D redesenhado a pedido do usuário; 105 testes.
- **Onde:** repositório da feather-ui, ramo `feather-sans`, commit `0fa32b0` (`fonte::usar_na_interface`).
  Cópias: `/run/media/maragon/Arquivos/GitHub/.wt/feather-ui-038`.
- **Pronto:** tag `v0.3.10` no remoto. Conferir a base do ramo (a conferir: é descendente de `v0.3.9`,
  `da8cb17`?) e se a feather-ui depende do pelicano por tag ou por caminho (a conferir), para que o
  `Cargo.toml` dela aponte para `v0.1.21`.
- **Depende:** PUB-4 (se a feather-ui pedir o pelicano novo); D3.  **Issue:** nenhuma.  **Tamanho:** P/M.

### PUB-6. Trocar o `[patch]` pelas tags e atualizar o `Cargo.lock`
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ⛔ obsoleto. Não há mais `[patch]`: o pelicano vem do crates.io (0.1.30) e a feather-ui pela tag v0.3.15 (`d697473`). A tentativa com `[patch]` para `/tmp` está guardada no stash `e7a5c3e`. Quando a fonte for publicada, basta subir as versões.
- **Onde:** `crates/feathersheet/Cargo.toml` (linhas 12 e 16: pelicano `v0.1.19` → `v0.1.21`; feather-ui
  `v0.3.8` → `v0.3.10`; apagar o bloco `[patch]` e o comentário "shortcut: provisório…" no fim);
  `crates/feathersheet/Cargo.lock`.
- **Pronto:** `grep -n patch crates/feathersheet/Cargo.toml` vazio; `Cargo.lock` aponta para as tags;
  `clone` limpo da tag compila (o usuário confirma; o Claude não roda `cargo`). Esse passo também acaba
  com o risco de `/tmp`.
- **Depende:** PUB-4, PUB-5.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-7. Conferir a fonte na tela e nas células
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. O `main.rs` não chama `usar_na_interface` e as células não usam `"sans-serif"` (essa mudança ficou no stash `e7a5c3e`).
**Situação (11/11):** ✅ feito no Linux, por foto: interface em Feather Sans, células com a letra do sistema (`Some("sans-serif")`). Falta o Windows (WIN-2).
- **Onde:** `crates/feathersheet/src/main.rs` (chamada de `usar_na_interface`); `src/grade.rs` e
  `src/estilos_comuns.rs` (`Some("sans-serif")` nas células, mudança não commitada).
- **Pronto:** foto da janela principal e de uma célula com texto (`FEATHERSHEET_CAPTURAR`, antes e
  depois); o `"sans-serif"` das células confirmado como coerente com `Font::SYSTEM_SANS` (a conferir se
  o valor deve ser `None`). Usuário confirma no Linux; o Windows vai em WIN-2.
- **Depende:** PUB-6.  **Issue:** nenhuma.  **Tamanho:** P/M.

### PUB-8. Commitar a Feather Sans no FeatherSheet
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Depende de PUB-4 e PUB-5.
**Situação (11/11):** ✅ feito. No `main`, com pelicano 0.1.33 e feather-ui v0.3.16, sem `[patch]`; 1059 testes.
- **Onde:** os arquivos de PUB-6 e PUB-7; `CLAUDE.md` (nota na seção da 0.3.0).
- **Pronto:** commit `claude: Feather Sans como fonte da interface (pelicano v0.1.21, feather-ui v0.3.10)`;
  `cargo` sem `[patch]`.
- **Depende:** PUB-6, PUB-7.  **Issue:** nenhuma.  **Tamanho:** P.

### PUB-9. Fechar #90: editar célula mantendo a formatação
**Situação (08/11):** ✅ feito. #90 fechada em 08/11 pelo `d697473`: o editor usa o alinhamento horizontal da célula (`TextInput::align_x`, pelicano 0.1.30). Letra, tamanho, negrito, cor e fundo já vinham do `e076ed3`. Entra na próxima versão.
- **Onde:** ramo `editor-alinhado` (commit `e5bf09b`, sobre o `main`); `grade.rs` e o editor de célula
  (a conferir o arquivo exato com `grep -rn "text_input\|text_editor" crates/feathersheet/src/grade*`).
- **Pronto:** rebase sobre o `main` depois de PUB-8; pelicano `v0.1.20` (tag existe) com `align_x`;
  foto de `A1:C1` mesclada, negrito, centralizada, letra grande, editada no lugar com a mesma aparência;
  commit com `Fixes #90`.
- **Depende:** PUB-8 (ou, se o rebase for antes, PUB-6 só com `v0.1.20`).  **Issue:** #90.  **Tamanho:** M.

---

## Frente 2 — Robustez de dados e desfazer

### ROB-1. Reproduzir e corrigir: fórmulas de G–M trocadas de coluna
**Situação (08/11):** 🟡 parcial. Provavelmente é o #45 ("Reabrir .fshz troca fórmulas entre colunas", corrigido na v0.1.5). Falta o usuário confirmar na "Planilha Pesada.fshz" e marcar o item no METAS.
- **Onde:** `crates/feathersheet-core/src/planilha/workbook.rs` (`mover_estrutura`), `planilha/sheet/`
  (inserir/excluir); exemplo `examples/conferir_estrutura_pequena.rs` (a conferir o caminho exato).
- **Pronto:** operação + gravar + reabrir reproduzido em teste pequeno; corrigido; usuário confirma na
  "Planilha Pesada.fshz" (#REF! e Resumo zerado sumiram). Hoje o item está `[ ]` no METAS, sem issue.
- **Depende:** nada.  **Issue:** criar (bug).  **Tamanho:** M.

### ROB-2. Excluir a primeira linha de intervalo encolhe o intervalo
**Situação (08/11):** ✅ conferido em 08/11: não reproduz. O motor já encolhe o intervalo (avulsa, coluna empacotada e de outra aba); fica travado por `excluir_a_primeira_linha_encolhe_o_intervalo_das_formulas_da_aba` (`workbook/tests.rs`), além do teste de texto do `formula_text.rs`.
- **Onde:** `crates/feathersheet-core/src/planilha/mover.rs` (regra de reescrita) e a reescrita de
  referências (a conferir: `formula_text.rs`).
- **Pronto:** teste com `=SOMA($I$2:$I$2000)` ao excluir a linha 2 vira `$I$2:$I$1999` (como no Excel),
  nunca `#REF!:$I$2000`. Se o teste já passar, fechar com o teste novo.
- **Depende:** nada.  **Issue:** criar se reproduzir.  **Tamanho:** P/M.

### ROB-3. Ctrl+Z de inserir e excluir linhas e colunas (#65)
**Situação (08/11):** ✅ feito. `549db94` (08/11, #65 fechada): antes de inserir ou excluir, a pasta vira uma foto `.fshz` na memória (`app/desfazer_estrutura.rs`, `Workbook::restaurar_foto`); Ctrl+Y incluso. Entra na próxima versão.
- **Onde:** `planilha/sheet/` (histórico: `Passo`, `esquecer_historico`); `planilha/workbook.rs`
  (`mover_estrutura`); tabelas, mesclas e nomes definidos andam junto (ver `planilha/mover.rs`).
- **Pronto:** teste: inserir e desfazer deixa células, fórmulas, estilos, tabelas e mesclas iguais ao
  estado anterior; excluir e desfazer devolve a fatia inteira (incluindo fórmulas de outras abas
  consertadas). Ctrl+Z não regrava célula a célula (ver ROB-4).
- **Depende:** ROB-1 (mesmo código).  **Issue:** #65.  **Tamanho:** G.

### ROB-4. Desfazer da estrutura sem regravar célula a célula
**Situação (08/11):** ✅ feito no mesmo `549db94`: o Ctrl+Z devolve a foto, sem regravar célula a célula. Medido em 200 mil linhas × 3: inserir 68 ms, desfazer 34 ms (`medir_foto_da_estrutura`). Limites: pasta com mais de 5 milhões de células na área usada não ganha foto (só avisa); o caso de 2 mil linhas com `=K1+I2` não foi medido à parte.
- **Onde:** o mesmo caminho de ROB-3 (o `Passo::Estrutura` guarda a fatia, não cada célula).
- **Pronto:** medido com 2 mil linhas com fórmula acumulada (`=K1+I2`): desfazer ≤ 100 ms (meta a
  confirmar na decisão D8). Número no F12 (`medir` no `feathersheet-core`).
- **Depende:** ROB-3.  **Issue:** #65.  **Tamanho:** incluído em ROB-3 (se passar de G, separar).

### ROB-5. Mesclar e desmesclar entram no Ctrl+Z
**Situação (08/11):** ✅ já existia. `feathersheet_io::mesclas::mesclar`/`desmesclar` gravam por `Sheet::alterar_mesclas`, que vira `Passo::Mesclas` (um Ctrl+Z). A pendência de 28/10 estava desatualizada. Não procurei um teste dedicado de mesclar → Ctrl+Z → desmesclar → Ctrl+Z.
- **Onde:** `crates/feathersheet/src/app/mesclas.rs` e `app/mesclas/`; histórico em `planilha/sheet/`.
- **Pronto:** teste: mesclar, Ctrl+Z, desmesclar, Ctrl+Z restaura o estado; foto não necessária.
- **Depende:** nada.  **Issue:** criar (pendência de 28/10, sem issue).  **Tamanho:** M.

### ROB-6. Recálculo fora da thread da janela, com progresso e Esc
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — 🟡 parcial. O `calculo_em_fundo.rs` não mudou desde a v0.2.1 (29/10). Falta o usuário medir o `MÍNIMOSES` em 13,7 milhões de linhas.
- **Onde:** `crates/feathersheet/src/app/calculo_em_fundo.rs` (existe desde a v0.2.1, com "Parar a cada
  256 linhas"); `app/macro_dialogo/em_fundo.rs` como referência. Cálculo no motor:
  `feathersheet-core/src/planilha/calculo_paralelo.rs`.
- **Pronto:** `MÍNIMOSES` em 13,7 milhões de linhas: a janela responde em ≤ 16 ms durante a conta
  (medido pelo usuário com F12), barra com progresso, Esc cancela. Confirmar primeiro se
  `calculo_em_fundo.rs` já cobre o caso (a conferir: o METAS v0.1.1 ainda marca `[ ]`).
- **Depende:** nada.  **Issue:** nenhuma (METAS v0.1.1); criar se o caso ainda travar.  **Tamanho:** M.

### ROB-7. Cópia para gravar não bloqueia a janela
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — 🟡 parcial. Falta só medir: a v0.2.1 já salva sem copiar as colunas de número, mas não há número registrado.
- **Onde:** `crates/feathersheet-io/src/da_workbook.rs`; `app.rs` (salvar); `feathersheet-io/src/fshz_write.rs`.
- **Pronto:** F12 mostra que a parte na thread da janela fica ≤ 16 ms em `2026_03.fshz` (13,7 mi linhas).
  Se a v0.2.1 ("salvar sem copiar as colunas de número") já resolveu, só medir e fechar o `[ ]` do METAS.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P (medir) / M (se não resolveu).

### ROB-8. SES com critério vazio ou zero sem índice
**Situação (08/11):** ✅ feito em 08/11: o agregado ganhou chaves para vazia, texto vazio e FALSO (`ChaveCriterio::alternativas`, `Agregado::grupo_de`); critério zero, `""` e célula vazia juntam os grupos pela mesma regra do `make_predicate`, e o que fica além das linhas lidas conta no CONT.SES. O agregado para na última linha das colunas lidas (não da aba). Release: 173 ms → 12 µs por fórmula de critério vazio. Gabarito: `ses_com_criterio_vazio_zero_e_texto_vazio_da_o_mesmo_com_e_sem_agregado`. O teste do Ctrl+D passa (1,9 s em debug, com o cálculo em fundo das 3 milhões; teto subiu para 5 s porque o teste espera o cálculo).
- **Onde:** `crates/feathersheet-core/src/planilha/agregado.rs` (índice por grupo, `sheet_deps.rs`);
  o teste que trava: `crates/feathersheet/src/app/preenchimento_ui.rs:216`.
- **Pronto:** o teste `ctrl_d_de_formula_em_milhoes_de_linhas_vai_em_bloco` passa dentro do tempo do teste
  (a conferir o teto), e a versão do core (`agregado.rs`) tem teste com critério vazio.
- **Depende:** nada.  **Issue:** criar (bug, o teste já era vermelho em 29/10).  **Tamanho:** M.

### ROB-9. Decidir o nível de compressão do `.fshz`
**Situação (08/11):** ❌ falta. D8 continua aberta; o `fshz_write.rs` usa `Compression::default()` (nível 6) num caminho e `fast()` no outro.
- **Onde:** `crates/feathersheet-io/src/fshz_write.rs`.
- **Pronto:** D8 respondida; número medido no `2026_03.fshz` pelo usuário (METAS: nível 6 = 43 s e
  359 MB; nível 3 = 20 s e 458 MB; nível 1 = 13 s e 702 MB).
- **Depende:** D8.  **Issue:** nenhuma.  **Tamanho:** P.

### ROB-10. Arrastar fórmula para a planilha inteira (padrão por faixa)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. D11 continua aberta.
**Situação (10/11):** 🟡 a parte de engenharia feita: a fórmula levada até o fim em todas as 16.384 colunas (100 milhões de linhas) cabe em 714 MB (MEM-2) e leva **2,7 s em release** (antes, 136 s só em debug). Cada coluna refazia a invalidação olhando todas as outras; dentro de uma transação de fora (colar, arrastar) as colunas agora se juntam em `Historico::invalidar_ao_fechar` e invalidam uma vez no `fechar_transacao`. Testes: `formulas_em_bloco_numa_transacao_invalidam_ao_fechar` e `formula_na_planilha_inteira_cabe_no_teto` (ignorado, release). Falta a D11 (o padrão por faixa para fórmula que não cabe em bloco) e o usuário conferir no app.
- **Onde:** `planilha/sheet/edicao.rs` (`preencher_formula_faixa_vazia`), `planilha/modelo_padrao.rs`
  (`FaixaPadrao`), `crates/feathersheet/src/grade/alca.rs` (alça de preenchimento).
- **Pronto:** arrastar a alça de uma fórmula até a linha 1.048.576 responde em ≤ 1 s (medido) e usa um
  padrão de faixa, não 1 milhão de fórmulas; teste de tempo e de teto de memória (MEM-2).
- **Depende:** D11 (desenho aprovado pelo usuário); MEM-1.  **Issue:** criar (pedido do usuário, 29/10).  **Tamanho:** G.

### ROB-11. Dar nome próprio ao teto de dimensões ocultas
**Situação (08/11):** ❌ falta. Continua `const MAX_OPERACOES: u64 = 2_100_000` em `edicao.rs:1718`.
**Situação (10/11):** ✅ feito: renomeada para `MAX_DIMENSOES_OCULTAS`.
- **Onde:** `crates/feathersheet-core/src/planilha/sheet/edicao.rs:1659` (constante local `MAX_OPERACOES`
  = 2.100.000, teto de linhas/colunas ocultadas de uma vez; homônima de `formula/parser.rs:43`).
- **Pronto:** renomeada para `MAX_DIMENSOES_OCULTAS`; `cargo test` do core verde.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P (opcional, só clareza).

---

## Frente 3 — Memória (plano dos 400 MB e testes que pegam estouro)

### MEM-1. Alocador contador de teste que recusa pedido acima do teto
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Não há `#[global_allocator]` em nenhum crate.
**Situação (10/11):** ✅ feito: `feathersheet-core/src/teste_memoria.rs` (`#[cfg(test)]`, `#[global_allocator]`). Conta por thread (o `cargo test` roda em paralelo); `Teto::novo("rótulo", bytes)` limita a thread e `pico()` diz o maior uso. Pedido acima do teto: escreve `[teto de memoria] rótulo: pedido de N bytes recusado (em uso X, teto Y)` sem alocar e aborta (conferido uma vez). Os 984 testes do motor passam com ele. Limite: trabalho mandado ao rayon não conta (comentário `shortcut:`). Só no core por ora; no `feathersheet-io` entra junto com o MEM-3.
- **Onde:** novo módulo de teste (ex.: `crates/feathersheet-core/src/teste_memoria.rs`, `#[cfg(test)]`,
  com `#[global_allocator]`); replicado em `feathersheet-io` (a decidir: copiar ou um crate de teste
  comum, D20). Hoje não existe nenhum `global_allocator` no repositório.
- **Pronto:** um teste de exemplo com teto artificial falha em milissegundos e diz o nome do teste e o
  pedido recusado, sem encher a RAM. Só nos crates leves (regra da memória).
- **Depende:** nada.  **Issue:** criar (pedido do usuário, 29/10).  **Tamanho:** M.

### MEM-2. Tetos: arrastar/colar fórmula até o fim e tudo selecionado
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Depende de MEM-1.
**Situação (10/11):** ✅ feito, com dois bugs achados pelo teto:
1. Levar uma fórmula até a linha 100 milhões numa coluna gastava **403 MB** (um bloco cru de 256 KB por bloco de 65.536 linhas, todos iguais). Novo `Bloco::Constante` em `column_store.rs` (bloco coberto inteiro, a ponta final e o bloco 0 que completa igual) e a máscara cheia comum (`mascara_cheia`): **43 KB** por coluna, pico 302 KB.
2. O contador de fórmulas (`FormulaTable::count`, u32) dava a volta acima de 4,29 bilhões (fórmula em todas as colunas até o fim): agora u64.
Testes em `feathersheet-core/src/teste_memoria.rs`: `formula_ate_o_fim_da_coluna_cabe_no_teto` (8 MB), `formula_em_mil_colunas_ate_o_fim_cabe_no_teto` (1.024 colunas, 64 MB, mede 44 MB) e, ignorado por ser lento, `formula_na_planilha_inteira_cabe_no_teto` (16.384 colunas: 711 MB, antes passava de 6 GB). Fica para o ROB-10: o tempo cresce mais que linear (136 s em debug) porque cada coluna refaz a invalidação olhando todas as outras.
- **Onde:** testes sobre `preencher_formula_faixa_vazia` e `Sheet::colar`; caso: 16.384 colunas ×
  1.048.576 linhas com a linha de fórmula (o caso que o usuário testou).
- **Pronto:** teste reprova se o pico passar do teto (MEM-1); medido pico exato.
- **Depende:** MEM-1, ROB-10.  **Issue:** criar.  **Tamanho:** M.

### MEM-3. Tetos: abrir `.xlsx` e `.fshz` gerados no teste (muitas colunas curtas)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Depende de MEM-1.
**Situação (10/11):** ✅ feito, com um ganho achado pelo teto: cada bloco reservava 8 KB de máscara de presença logo na 1ª célula. Abrir um `.fshz` de 9 abas × 200 colunas × 30 linhas (54 mil células) montava **27 MB**; a máscara agora cresce como os valores (`mascara_ate`, `bit` em `column_store.rs`) e são **2,2 MB**. Testes em `feathersheet-io/src/tetos_de_memoria.rs` (o alocador vem do core por `#[path]`): `.fshz` e `.xlsx` de colunas curtas com teto de 8 MB. Limite: o `.xlsx` lê as abas em threads, que o alocador não conta.
- **Onde:** `crates/feathersheet-io/examples/` (os `medir_*` existem); testes novos em
  `feathersheet-io/src/` com geração sintética pequena e teto.
- **Pronto:** reprova o pico acima do teto; compara com o número medido no `2026_03.fshz` pelo usuário.
- **Depende:** MEM-1.  **Issue:** criar.  **Tamanho:** M.

### MEM-4. Tetos: dinâmica e gráfico em muitas linhas
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Depende de MEM-1.
**Situação (10/11):** 🟡 dinâmica ✅: `dinamica_de_um_milhao_de_linhas_cabe_no_teto` (1 M linhas, 5 regiões × 12 meses, teto 1 MB): caminho antigo 46 KB, árvore de grupos 72 KB, ou seja, guarda por grupo e não por linha. Falta o gráfico: a leitura dos dados do gráfico mora no crate da interface, que ainda não tem o alocador de teste.
- **Onde:** `feathersheet-core/src/dinamica/pivot.rs`, `dinamica_layout.rs`; `app/graficos/testes.rs`
  (bancada `medir_quadros_grafico_com_bloco`, ignorada).
- **Pronto:** teste com teto; a bancada ignorada vira teste normal com teto.
- **Depende:** MEM-1.  **Issue:** criar.  **Tamanho:** M.

### MEM-5. Tetos: compactar, salvar e reabrir
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Depende de MEM-1.
**Situação (10/11):** ✅ feito: `compactar_salvar_e_reabrir_cabe_no_teto` (200 mil linhas com número, texto e fórmula; compactar + salvar + reabrir com teto de 32 MB, mede 17 MB) e confere os valores e as fórmulas de volta.
- **Onde:** `feathersheet-core/src/compactar` (a conferir: `ChunkedColumn::compactar`), `feathersheet-io/src/fshz_write.rs`,
  `fshz_read.rs`.
- **Pronto:** ciclo compactar + salvar + reabrir dentro do teto, com resultado igual bit a bit.
- **Depende:** MEM-1.  **Issue:** criar.  **Tamanho:** M.

### MEM-6. Medir o número atual no app (linha de partida da meta)
**Situação (08/11):** ✅ medido em 08/11 no app em release (modo captura, memória lida do processo depois de abrir): app vazio 154 MB residente / 43 MB própria (o número do Monitor do sistema); Planilha Pesada.fshz 252 / 140 MB (pico 661 MB ao abrir); 2026_03.fshz (13,7 mi de linhas × 46 colunas) abre em 9,9 s com 2.174 / 2.064 MB (pico 2.187 MB). A Planilha Pesada do disco tem as fórmulas da aba Dados deslocadas uma coluna (`Mês` = `MÊS(C2)` lê a Região): é o sintoma do ROB-1 gravado no arquivo, e o Resumo sai zerado; não é do motor.
- **Onde:** F12 (`app/monitor.rs`) no app; arquivo `Planilha Pesada` e `2026_03.fshz` do usuário.
- **Pronto:** tabela com RAM do app vazio, do `2026_03.fshz` aberto e calculado, e da Planilha Pesada.
  Meta do plano: 400–500 MB (hoje a memória cita ~1.600 MB antes da v0.2.2; medir de novo).
- **Depende:** nada (usuário roda).  **Issue:** nenhuma.  **Tamanho:** P.

### MEM-7. Fase 5 do plano: resultado direto na coluna (sem `calc.cache`)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. D9 continua aberta.
- **Onde:** `feathersheet-core/src/planilha/column_store.rs`, `planilha/calculo_paralelo.rs`,
  `planilha/textos_crus.rs`; `planilha/faixa_lida.rs` (a conferir).
- **Pronto:** pico de ~360 MB da Resumo recém-aberta do `.xlsx` e a consolidação de 1,5 s caem; teste
  de igualdade de resultado; medido pelo usuário.
- **Depende:** D9.  **Issue:** nenhuma.  **Tamanho:** G.

### MEM-8. Fase 3 do plano: XLSX com dados tipados (sem reserva de 2 milhões de textos)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. A v0.2.6 compactou a abertura do `.fshz`, não a do `.xlsx`.
- **Onde:** leitor de `.xlsx` em `feathersheet-io/src/` (a conferir o módulo: `arrow_fluxo.rs` ou
  `xlsx_read*`); a reserva de textos `"374.99"`, `"14/12/2026"`.
- **Pronto:** ganho medido no `.xlsx` de 13 mi células; mesma leitura célula a célula.
- **Depende:** D9.  **Issue:** nenhuma.  **Tamanho:** G.

### MEM-9. Gravar em fluxo (pico de +360 MB ao salvar)
**Situação (08/11):** ✅ feito em 08/11: a cópia para gravar materializava um `i32` por linha de cada coluna de texto. Agora vai pelos blocos da coluna viva (`DataTable::retrato_txt`, `ColunaTxtBruta::blocos`/`indice`, `da_workbook::coluna_txt_sem_copia`), como os números. `medir_salvar_grande` no 2026_03.fshz (pelo leitor compacto, o do app): além da pasta aberta 605 → 134 MB; arquivo idêntico byte a byte ao de antes (`cmp`).
- **Onde:** `feathersheet-io/src/fshz_write.rs`.
- **Pronto:** pico ao salvar medido no `2026_03.fshz` sem o acréscimo; arquivo igual ao atual
  (`examples/conferir_fshz_iguais.rs`).
- **Depende:** ROB-7, D9.  **Issue:** nenhuma.  **Tamanho:** G.

### MEM-10. F12 e Monitor do GNOME: mesmo critério de memória
**Situação (08/11):** ✅ feito em 08/11: a diferença é de critério. O Monitor do GNOME mostra residente menos compartilhada (`RssAnon`); a "Memória em uso" do F12 (feather-ui) é o `VmRSS`, que soma bibliotecas, fontes e o driver de vídeo. O F12 do app ganhou o cartão "Memória própria" (o número do Monitor do sistema) e a linha "Bibliotecas e fontes compartilhadas" com a diferença (`monitor::campo_status_kb`). Trocar o cartão principal na feather-ui fica para quando o `monitor.rs` de lá estiver livre (há trabalho de outro chat nele).
- **Onde:** `crates/feathersheet/src/app/monitor.rs`; leitura de `VmRSS`/`VmHWM` (a conferir o que o F12 mostra).
- **Pronto:** o F12 mostra o mesmo número que o Monitor do sistema (diferença explicada ou corrigida;
  hoje ~300 MB).
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P/M.

### MEM-11. Abrir sob demanda (`mmap`) e fórmula compilada (fase 6 e V5)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Já marcada como fora da 0.3.0.
- **Onde:** `feathersheet-io/src/fshz_read.rs`; `feathersheet-core` (bytecode).
- **Pronto:** fora da 0.3.0 se D9 disser assim; ver "Fora do escopo".
- **Depende:** D9, e o `.fshz` deixar de ser gzip único.  **Issue:** nenhuma.  **Tamanho:** G (fora).

---

## Frente 4 — Tabela dinâmica

### DIN-1. Bug: a coluna DATA sai como número no começo e como texto no fim
**Situação (08/11):** ✅ feito em 08/11: o rótulo numérico de um campo de Linhas cuja coluna de origem tem formato de data recebe esse formato (`config_campo::formatos_de_data`), nos dois caminhos (criar e editar). Teste `campo_de_data_em_linhas_sai_como_data_em_todas_as_linhas`. Limite: data guardada como texto na origem (`7/30/2026` de CSV americano) continua texto e vai para o fim; isso é da importação.
- **Onde:** `feathersheet-core/src/dinamica/pivot.rs` (`excel_serial_to_ymd`), `dinamica_layout.rs`;
  `crates/feathersheet/src/app/dinamica_ui.rs`.
- **Pronto:** teste com 3 datas de origem: a dinâmica mostra data em todas as linhas (formato único);
  foto de `FEATHERSHEET_CAPTURAR_DINAMICA`. Relato da memória (29/10): `46240` no começo e
  `7/30/2026` no fim.
- **Depende:** nada.  **Issue:** criar (bug).  **Tamanho:** M.

### DIN-2. Total Geral fora do gráfico dinâmico (conferir)
**Situação (08/11):** ✅ feito. `355e9dd` (29/10), já nas versões publicadas. Falta só a foto para encerrar a pendência.
- **Onde:** commit `355e9dd` (já feito, segundo a memória); `app/graficos.rs`.
- **Pronto:** confirmar com foto e fechar a pendência da memória de 29/10.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### DIN-3. #106: filtro de relatório some quando o campo está em Filtros e em Linhas; a lista abre longe do botão
**Situação (08/11):** ✅ feito. `268012a` (28/10); #106 fechada em 04/11.
- **Onde:** `dinamica_layout.rs` (posição das células de filtro, A1:B1), `app/dinamica_ui.rs` (lista do
  filtro); "Execução" aparece duas vezes (provavelmente espaço no fim do item: `trim` ao comparar).
- **Pronto:** com STATUS em Filtros e Linhas, A1 = STATUS e B1 = (Tudo) com o botão; a lista abre logo
  abaixo do botão; filtrar não tira o foco da tabela; foto `FEATHERSHEET_CAPTURAR_DINAMICA=filtro`.
- **Depende:** nada.  **Issue:** #106.  **Tamanho:** M.

### DIN-4. #107: menu do campo com "Mover para ›" e "Resumir por ›" mais limpo
**Situação (08/11):** ✅ feito. `268012a`; #107 fechada em 04/11.
- **Onde:** `app/dinamica_ui.rs` (menu da setinha do campo); `FEATHERSHEET_CAPTURAR_DINAMICA=menu|resumir`.
- **Pronto:** um item "Mover para ›" com submenu (Para cima, Para baixo, Filtros, Colunas, Linhas,
  Valores); cada item do submenu move o campo (teste da ação); foto.
- **Depende:** nada.  **Issue:** #107.  **Tamanho:** P/M.

### DIN-5. Dinâmicas recomendadas: fazer ou retirar (menu morto)
**Situação (08/11):** ✅ feito em 08/11 (opção A): `dinamica::campos::sugestoes` (soma de número por categoria, por duas, outro número, contagem; até 5) e os botões no primeiro passo; o clique cria a dinâmica.
- **Onde:** `app/dinamica_ui.rs:187`, `:491`, `:530–532` (`Evento::Recomendadas`); `faixa_nativa_grupos.rs`
  (o botão); `app/ajuda.rs` (o tópico que promete o recurso).
- **Pronto (opção A, D5):** sugere 3 a 5 resumos a partir dos dados (campo de texto + número, campo de
  data), mostra a prévia, o clique cria a dinâmica; teste com dados sintéticos. **Opção B:** botão e
  tópico da Ajuda saem; teste que falha se houver item de menu sem ação. Regra 3 proíbe o estado atual.
- **Depende:** D5.  **Issue:** criar (bug: menu morto).  **Tamanho:** M (A) / P (B).

### DIN-6. Itens agrupados à mão e itens calculados
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
**Situação (10/11):** 🟡 a parte "itens agrupados à mão" feita: aba Tabela Dinâmica ▸ Agrupar seleção ▸ Agrupar / Desagrupar. Os itens de Linhas selecionados na grade viram um item só ("Grupo1"), que soma todos; no tabular o nível é o da coluna selecionada (como no Excel), no compacto o da linha; agrupar um grupo com outros itens junta tudo. Motor: `Extras::grupos_de_itens` (`GrupoDeItens { coluna, nome, itens }`, pela coluna da origem, gravado no `.fshz` com a dinâmica) e `Extras::juntar` nos quatro pontos que calculam a chave do item. Testes: `itens_agrupados_a_mao_somam_juntos_e_detalham` (core; o Detalhes do grupo acha as linhas dos dois), `agrupar_selecao_junta_itens_e_desagrupar_desfaz` e `agrupar_no_tabular_usa_o_nivel_da_coluna_selecionada` (GUI); foto `FEATHERSHEET_CAPTURAR_DINAMICA=agrupar`. Também em Colunas (seleção no cabeçalho; com dois campos vale o último nível) e renomear digitando por cima do rótulo (`renomear_grupo_na_celula`, chamado no `confirmar`); testes `agrupar_no_cabecalho_junta_itens_de_colunas` e o renomear dentro de `agrupar_selecao_junta_itens_e_desagrupar_desfaz`. Falta: itens calculados; o `.xlsx` não leva os grupos (o Excel mostra os itens soltos).
**Situação (10/11, fim):** itens calculados feitos: painel ▸ Origem e campos calculados ▸ ITENS CALCULADOS — escolhe-se um campo de Linhas ou Colunas, nome e fórmula sobre os outros itens (`FECAM - TCA`, gramática do campo calculado, com as sugestões do DIN-7). Motor: `Extras::itens_calculados` (`ItemCalculado { coluna, nome, formula }`); em cada cruzamento a conta usa os valores já resumidos dos irmãos (`com_calculados` em `dinamica_layout.rs`), em Linhas, em Colunas e em qualquer nível (só aparece embaixo do pai que tem os itens usados), no fim da lista. Testes `item_calculado_faz_a_conta_sobre_os_irmaos` (core) e `item_calculado_pelo_painel_entra_e_sai` (GUI); foto `FEATHERSHEET_CAPTURAR_DINAMICA=item` conferida. Diferença do Excel (comentário `shortcut:`): totais e subtotais não somam o item calculado. Falta só o `.xlsx` (grupos e itens calculados não vão ao arquivo).
- **Onde:** `dinamica/pivot.rs` (agrupar), `dinamica/exibicao.rs` (já tem agrupar em faixas numéricas e
  datas: reaproveitar), `app/dinamica_ui.rs` (seleção de itens → Agrupar).
- **Pronto:** selecionar 2 itens → Agrupar cria um grupo; a soma do grupo confere com a soma manual;
  teste de core; foto.
- **Depende:** D6.  **Issue:** criar (buraco listado no METAS v0.1.6/v0.2).  **Tamanho:** G.

### DIN-7. Campo calculado com editor de fórmula e autocompletar
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. O campo calculado existe no painel avançado, sem editor com autocompletar.
**Situação (10/11):** ✅ feito. Embaixo do campo da fórmula aparecem os campos que completam a palavra em digitação (`campo_calculado::sugestoes`); um clique troca a palavra pelo nome, entre colchetes quando tem espaço ou começa por número (`completar`), e devolve o foco ao campo. A gramática já tinha `+ - * /` e parênteses. Testes `sugere_e_completa_o_nome_do_campo` e `sugestao_completa_o_campo_da_formula`; foto com `FEATHERSHEET_CAPTURAR_DINAMICA=calculado`. F1 nos três idiomas.
- **Onde:** `feathersheet-core/src/dinamica/campo_calculado.rs` (hoje só soma, no painel avançado);
  `app/dinamica_ui/` (a conferir: onde mora o painel avançado).
- **Pronto:** campo `=Preço*Quantidade` com autocompletar dos nomes dos campos; teste de resultado;
  foto do editor.
- **Depende:** D6.  **Issue:** criar.  **Tamanho:** G.

### DIN-8. Subtotais em cima, embaixo ou sem
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
**Situação (10/11):** ✅ feito: botão da faixa "Subtotais: Sem / Em cima / Embaixo" (como o do Excel, para a dinâmica toda, não por campo); `Opcoes::subtotal_embaixo` no compacto põe a linha "a Total" depois dos itens (recolhido volta a contar na linha do grupo; o tabular já é sempre embaixo). Gravado como `subtotaisEmbaixo`; `.xlsx` lê e grava `subtotalTop="0"`. Teste `compacto_com_subtotal_embaixo_do_grupo`.
- **Onde:** `dinamica/dinamica_layout.rs`, `dinamica/exibicao.rs` (Extras); hoje só liga/desliga.
- **Pronto:** três opções por campo; teste do layout compacto e tabular com cada uma.
- **Depende:** D6.  **Issue:** criar.  **Tamanho:** M.

### DIN-9. Estrutura de tópicos e recolher/expandir por nível (conferir)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — 🟡 parcial. Já existem "Recolher tudo" e "Expandir tudo" (`dinamica_ui/faixa.rs:154`) e recolher por item; o "por nível" não encontrei.
**Situação (10/11):** ✅ feito: "Recolher nível" e "Expandir nível" na faixa (um nível por clique, como os botões de tópicos: recolhe o mais fundo ainda aberto, expande o mais raso ainda fechado). `dinamica_layout::passo_de_nivel`, teste `nivel_recolhe_do_mais_fundo_e_expande_do_mais_raso`. Ajuda F1 nos três idiomas.
- **Onde:** `dinamica/dinamica_layout.rs`; `recolhidos` em `app/dinamica_ui.rs`.
- **Pronto:** o layout de tópicos tem recolher por nível e teste; hoje a memória diz que
  "recolher/expandir" existe: confirmar só a parte "por nível" (a conferir).
- **Depende:** DIN-8.  **Issue:** criar se faltar.  **Tamanho:** M.

### DIN-10. Mostrar páginas de filtro (uma aba por item)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
**Situação (10/11):** ✅ feito: botão "Páginas do filtro" na faixa da dinâmica; uma aba por item do primeiro campo em Filtros (pula os que o filtro já tira; recusa se a lista passa de 160 itens), cada aba com o nome do item. Teste `paginas_do_filtro_criam_uma_aba_por_item`.
- **Onde:** `app/dinamica_ui.rs` (ação), `feathersheet-core/src/planilha/workbook.rs` (criar abas).
- **Pronto:** a ação cria uma aba por item do campo de filtro, com o recorte de cada um; teste de
  quantas abas e conteúdo.
- **Depende:** DIN-3.  **Issue:** criar.  **Tamanho:** M.

### DIN-11. Origem: intervalo, tabela, outra aba e outro arquivo aberto (conferir)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — 🟡 parcial. Existe `dinamica_ui/origem.rs`; o quadro de conferência de cada origem não foi feito.
**Situação (10/11):** 🟡 quase: intervalo (`A1:D9`, seleção ao abrir) ✅; outra aba (`Dados!A1:D9`, `'Minha aba'!A:D`) ✅; tabela ou nome definido (`Vendas`) ✅ — a caixa de origem resolve os três (`origem::resolver`, teste `origem_aceita_outra_aba_aspas_e_nome_de_tabela`). Falta: outro arquivo aberto (`[Arq.fshz]Aba!A1:D9`), que precisa ler pelo vínculo em vez de `workbook.valor`; vira atividade própria.
- **Onde:** `app/dinamica_ui.rs` (criação); `planilha/vinculo.rs` (referência entre arquivos abertos,
  já existente desde a v0.1.0).
- **Pronto:** quadro de confere: intervalo e outra aba (a conferir se já funcionam), tabela (`regiao_da_tabela`),
  outro arquivo aberto (usa o vínculo). Teste de cada um; o que faltar vira atividade própria.
- **Depende:** nada.  **Issue:** criar (se faltar).  **Tamanho:** M.

### DIN-12. Agregações: contagem de distintos, mediana e percentil
**Situação (08/11):** ✅ feito em 08/11: `AgregacaoOp::Distintos`, `Mediana` e `Percentil(p)` (PERCENTIL.INC; o painel oferece 1º quartil, 3º quartil e percentil 90) no `Acumulador`; distintos conta número e texto sem ligar para maiúscula, vazio fora. Gravados como `distintos`/`mediana`/`percentilNN`. No `.xlsx` vão como soma (a dinâmica clássica do Excel não tem essas contas; os valores das células valem até atualizar lá). Testes `distintos_mediana_e_percentil_como_no_excel` (motor) e `distintos_e_mediana_na_dinamica_e_guardados_no_arquivo` (app).
- **Onde:** `feathersheet-core/src/dinamica/pivot.rs:13` (`AgregacaoOp`) e `do_str`; `dinamica_ui.rs`
  (seletor de agregação).
- **Pronto:** `Distintos`, `Mediana` e `Percentil` no motor, com teste contra valores conhecidos (texto
  e vazio como no Excel); no painel; escrita no `.xlsx` (a conferir se a dinâmica gravada aceita).
- **Depende:** D18 (sem o "modelo de dados" do Excel).  **Issue:** criar.  **Tamanho:** M.

### DIN-13. Ctrl+Z de cada mudança no painel
**Situação (08/11):** ✅ feito em 08/11: teste `cada_mudanca_do_painel_desfaz_com_um_ctrl_z` (marcar campo, outro campo, trocar a conta, filtrar: cada um volta com um Ctrl+Z, na ordem).
- **Onde:** `Passo::Dinamicas` (existe, a conferir se cada ação do painel gera um passo).
- **Pronto:** arrastar campo, mudar agregação, mudar filtro: cada um desfaz com um Ctrl+Z; teste.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### DIN-14. Estilos da dinâmica que sobrevivem ao Atualizar
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
**Situação (10/11):** ✅ feito. O tema e o formato posto à mão (moeda, negrito) já sobreviviam ao Atualizar — agora com teste (`tema_e_formato_a_mao_sobrevivem_ao_atualizar`). O `.fshz` grava o tema no JSON da dinâmica. O `.xlsx` gravava sempre `PivotStyleLight16`: agora o tema vai como `PivotStyle…` (`ESTILOS_DA_DINAMICA`, nos dois sentidos) e estilos de arquivos do Excel são lidos pela família de cor, como nas tabelas. Teste `tema_da_dinamica_vai_e_volta`. Limite: o tema padrão do Office não tem vermelho nem roxo; no Excel saem laranja e dourado (voltam certos no FeatherSheet).
- **Onde:** `dinamica/` (configuração JSON na chave `extras`); `app/dinamica_visual.rs`.
- **Pronto:** escolher um estilo, Atualizar, o estilo continua; ida e volta `.fshz` e `.xlsx`.
- **Depende:** D6.  **Issue:** criar.  **Tamanho:** G.

### DIN-15. Segmentação de dados completa
**Situação (08/11):** ❌ falta. #145 aberta (segmentação e linha do tempo).
- **Onde:** `app/dinamica_ui/segmentacao.rs` (hoje ligada a uma dinâmica só); leitor de `.xlsx` ignora a
  segmentação (METAS v0.2).
- **Pronto:** segmentação ligada a duas dinâmicas, seleção múltipla, lida e gravada no `.xlsx`; teste de
  ida e volta; LibreOffice/Excel (usuário) abre sem "conteúdo removido".
- **Depende:** D6, GRA-9 (o gráfico dinâmico usa a mesma segmentação).  **Issue:** criar.  **Tamanho:** G.

### DIN-16. Metas de número da dinâmica (medir, sem mudar código)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta (o usuário mede).
- **Onde:** bancada `medir_etapas_da_troca` (memória de 15/10): `FEATHERSHEET_PESADO=…_1M.fshz
  FEATHERSHEET_ABA=1 FEATHERSHEET_CAMPOS="2,3;6" cargo test --profile medir ...` (rodar pelo usuário,
  com limite de memória).
- **Pronto:** 1 M linhas, 2 campos de linha + 1 de valor ≤ 200 ms; arrastar campo ≤ 100 ms; 10 M
  linhas, 3 + 2 ≤ 1 s; atualizar após uma célula ≤ 50 ms; memória extra ≤ 10% da origem. Número
  registrado no F12 e no README (DOC-3). Os números que não baterem viram atividades próprias.
- **Depende:** DIN-19.  **Issue:** nenhuma.  **Tamanho:** M (medir).

### DIN-17. Atualizar só o grupo que mudou
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
- **Onde:** `dinamica/pivot.rs` (somas por grupo, METAS v0.1.4), `app/dinamica_ui.rs`.
- **Pronto:** mudar uma célula da origem refaz só o grupo afetado; ≤ 50 ms (DIN-16).
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** G.

### DIN-18. Adiar atualização do painel
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
**Situação (10/11):** ✅ feito: "Adiar atualização" e "Atualizar" no pé do painel de campos; adiado, as mudanças ficam pendentes e o botão aplica tudo numa conta só; desligar com pendência aplica. Teste `adiar_atualizacao_so_aplica_no_botao`.
- **Onde:** `app/dinamica_ui.rs` (arrumar campos e calcular no fim).
- **Pronto:** com a opção ligada, arrastar 3 campos recalcula uma vez; teste de contagem de cálculos.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### DIN-19. Arrastar campo responde em ≤ 100 ms até 1 M linhas
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
- **Onde:** `app/dinamica_ui.rs` (evento de arrasto) + `dinamica_layout.rs`.
- **Pronto:** medido pela bancada (DIN-16).
- **Depende:** DIN-17 ou DIN-18.  **Issue:** criar se não bater.  **Tamanho:** M.

---

## Frente 5 — Gráficos

### GRA-1. Gantt: linha do dia de hoje e pintar concluídas, em andamento e futuras (#133)
**Situação (08/11):** ✅ feito em 08/11: feather-charts v0.3.5 (`EstiloGantt`: linha de hoje e cores pela situação, data em `PedidoAvancado::hoje`; commit `02bead2`). Painel: Séries ▸ Cronograma (liga, cores); a cor por barra já existia em Pontos. MCP: `sheet_grafico` com `gantt {hoje, situacao, cores}`. **Pendente:** publicar a tag `v0.3.5` da feather-charts (o push da tag pediu permissão) e trocar o `rev` pela tag nos dois `Cargo.toml`. "% concluído" preenchendo a barra ficou de fora.
- **Onde:** `feather-charts` (`personalizacao.rs`, `cor_do_ponto`; tag nova `v0.3.5`);
  `crates/feathersheet/src/app/graficos.rs` (painel); `app/graficos/`; MCP em `sheet_grafico` (MCP-10).
- **Pronto:** foto do Gantt com linha "Hoje" ligável no painel e três cores calculadas pela data (não
  fixas); cor por barra no painel; `sheet_grafico` aceita as opções. Tag do feather-charts publicada.
- **Depende:** D7; a tag do feather-charts.  **Issue:** #133.  **Tamanho:** G.

### GRA-2. Rosca de verdade (#113)
**Situação (08/11):** ✅ feito. `e8c8ea6`: rosca com tamanho do furo, um anel por série, `doughnutChart`/`holeSize` no `.xlsx`; #113 fechada em 04/11.
- **Onde:** `feather-charts` (`EstiloGrafico`: espessura do anel, já em `EstiloPizza::espessura_anel`);
  `feathersheet-io` (leitor/gravador de `doughnutChart`, `holeSize`; a conferir o arquivo
  `xlsx_read_graficos*`/`xlsx_write_graficos.rs`).
- **Pronto:** foto de rosca com `holeSize 50`; o `.xlsx` gravado volta com `doughnutChart`; teste de ida e
  volta; o paliativo (pizza cheia) sai.
- **Depende:** tag do feather-charts.  **Issue:** #113.  **Tamanho:** M.

### GRA-3. Painel próprio por tipo de gráfico (#108)
**Situação (08/11):** ✅ feito. `252a93f`: campos de cada tipo em Configurar e Personalizar, e a pizza abre com uma série; #108 fechada em 04/11.
- **Onde:** `crates/feathersheet/src/app/graficos/` (pasta existe); `app/graficos.rs`.
- **Pronto:** pizza com uma série (bug relatado: duas séries selecionadas ao abrir); painéis de dispersão,
  cascata e os outros tipos com as opções de cada um; foto de `FEATHERSHEET_CAPTURAR_GRAFICO=painel:<tipo>`
  para cada tipo.
- **Depende:** nada (GRA-1 e GRA-2 usam o mesmo painel: fazer antes o painel genérico).  **Issue:** #108.  **Tamanho:** G.

### GRA-4. Contorno dos dados de origem e alças para ajustar o intervalo (#121)
**Situação (08/11):** ✅ feito. `642cc82`, `f722375`, `ac2c8c8`, `389bafd`, `d8198ff`: origem contornada na grade, borda e quinas que esticam, num Ctrl+Z; #121 fechada em 04/11.
- **Onde:** `app/graficos.rs` (seleção); desenho sobre a grade (`grade/desenho_auxiliar.rs`, `src/geometria_overlay.rs`).
- **Pronto:** com o gráfico selecionado, roxo nos rótulos, azul nos valores, vermelho no cabeçalho, alças
  nos cantos que mudam o intervalo do painel; foto. Desenho aprovado antes (regra 21: descrever antes).
- **Depende:** D10 (confirmar o desenho com o usuário).  **Issue:** #121.  **Tamanho:** G.

### GRA-5. Trocar a categoria sem travar; contas da série pelo tipo
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta (a conferir; nenhum commit sobre isso).
**Situação (10/11):** 🟡 conferido no código: o gráfico **não agrega** por categoria (`graficos/editor.rs`: "não agrega"; `dados_do_grafico` lê as células como estão, igual ao Excel). A regra "texto só Contar" era do gráfico antigo em TypeScript e não se aplica; quem quer conta por categoria usa o gráfico dinâmico. Resta só medir a troca de categoria com o relatório do usuário (`FEATHERSHEET_XLSX=… cargo test --profile medir --bin feathersheet medir_graficos_ao_abrir_e_trocar -- --ignored --nocapture`).
- **Onde:** `app/graficos.rs`; `feather-charts` (a conferir).
- **Pronto:** número: Soma, Média etc.; texto: só Contar e Contar distintos (regra do METAS v0.1.0); troca de
  categoria mede ≤ 16 ms na bancada `medir_graficos_ao_abrir_e_trocar`.
- **Depende:** GRA-3.  **Issue:** criar.  **Tamanho:** M.

### GRA-6. Gráfico com mais de 200 mil linhas, resumido (conferir)
**Situação (08/11):** ✅ feito. `MAX_CELULAS_GRAFICO = 200_000` (`graficos/editor.rs:95`): acima disso o gráfico só sai resumido e o "Não resumir" some (`graficos.rs:371`). Falta a foto.
- **Onde:** `app/graficos.rs` (o liga automático e "Não resumir" na lista).
- **Pronto:** conferir se a regra do METAS v0.1.0 ("acima de 200 mil células o resumo liga sozinho; lê até
  100 mi") está no código; se não, implementar; foto.
- **Depende:** nada.  **Issue:** criar se faltar.  **Tamanho:** P/M.

### GRA-7. Gráfico novo aparece na parte visível da planilha (conferir)
**Situação (08/11):** ✅ feito. Teste `grafico_novo_nasce_na_parte_visivel_da_folha` (`graficos/testes.rs:340`, desde 17/10).
- **Onde:** `app/graficos.rs` (posição inicial).
- **Pronto:** com a planilha rolada até a linha 10.000, o gráfico novo aparece na área visível; foto.
- **Depende:** nada.  **Issue:** criar se faltar.  **Tamanho:** P.

### GRA-8. BLOCO no Eixo X: fechar ou reabrir (conferir)
**Situação (08/11):** ✅ fechado em 08/11: medido em `5239b08` (bancada `medir_quadros_grafico_com_bloco`, 18–21 ms com e sem gráfico); não há pendência.
- **Onde:** commit `5239b08` (bancada `medir_quadros_grafico_com_bloco`, 18–21 ms com e sem gráfico).
- **Pronto:** a memória de 29/10 diz "medido"; decidir se a pendência fecha (sem issue) e registrar.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### GRA-9. Gráfico dinâmico completo: botões de campo, todos os tipos, `.xlsx`
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Há um gráfico dinâmico básico (`dinamica_ui/grafico_dinamico.rs`); os botões de campo e o `.xlsx` não existem.
- **Onde:** `app/graficos.rs` e `app/dinamica_ui.rs`; `feathersheet-io` (gravar o gráfico dinâmico no `.xlsx`).
- **Pronto:** botões de campo no próprio gráfico (filtrar pela dinâmica); os tipos de gráfico todos
  disponíveis; ida e volta `.xlsx`; foto.
- **Depende:** D6, DIN-15 (segmentação compartilhada).  **Issue:** criar.  **Tamanho:** G.

### GRA-10. Excel remove o desenho da aba de gráficos (histograma e cascata) (#27)
**Situação (08/11):** 🟡 parcial. `0773b71` (28/10) declara os namespaces da cascata e do histograma como o Excel. #27 segue aberta até o usuário abrir no Excel.
- **Onde:** `feathersheet-io/src/xlsx_write_graficos.rs` (a conferir: nome exato); os `chartEx`.
- **Pronto:** o usuário abre no Excel os três arquivos `teste-1`, `teste-2`, `teste-3` (o caminho
  `teste-*.xlsx` está no corpo da issue) e a aba mantém os gráficos. Antes: o usuário diz qual abriu
  sem o gráfico; depois: arquivo gravado de novo e conferido.
- **Depende:** o usuário abre no Excel.  **Issue:** #27.  **Tamanho:** M.

---

## Frente 6 — Editor de célula e grade

O editor de célula com formatação (#90) está em PUB-9, por depender da cadeia da Feather Sans e do pelicano `v0.1.20`.

### EDI-1. Barra de rolagem horizontal não reconhece o fim com colunas ocultas
**Situação (08/11):** ❌ falta. O `barras.rs` só mudou pela largura da coluna de números (`af06557`, #148). D21 continua aberta.
- **Onde:** `crates/feathersheet/src/grade/barras.rs:62–90` (`cursos`: `ultima_coluna` por índice,
  `vis_colunas` por colunas visíveis); `grade/barras.rs:98` e `:145` (uso).
- **Pronto:** reproduzir com um `.xlsx` de teste com A e H ocultas (aba Geral); a barra chega ao fim e
  para na última coluna com dados; teste que conta colunas ocultas. Pergunta ao usuário: qual barra e o
  que aconteceu (ver D21).
- **Depende:** D21.  **Issue:** criar (bug relatado em 29/10).  **Tamanho:** M.

### EDI-2. Número digitado como texto (indicador)
**Situação (08/11):** ❌ falta.
- **Onde:** `grade/` (canto da célula); regra em `planilha/` (a conferir).
- **Pronto:** célula com "1" como texto mostra o indicador e o aviso ao passar o mouse (foto);
  desligável nas Configurações. Desenho aprovado antes (regra 21).
- **Depende:** D22.  **Issue:** criar (paridade).  **Tamanho:** M.

---

### EDI-3. Dividir texto em colunas: todas as colunas, seleção em massa e tipo para várias (#134)
**Situação (08/11):** ✅ feito. `090148c`; #134 fechada em 04/11.
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
**Situação (08/11):** 🟡 parcial. Feito o `c6e306d` e a #134 foi fechada. Ainda falta medir no arquivo real, e a gravação continua célula a célula na thread da janela.
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
**Situação (08/11):** 🟡 parcial. Provavelmente era o #135 (Windows 10 + Intel HD 2500: borda preta e imagem deslocada), resolvido na v0.2.4 (pelicano v0.1.22, janela pela CPU) e conferido na máquina do #135. Falta o usuário confirmar que é o mesmo notebook.
- **Onde:** instruções já na memória `windows-notebook-empresa` (`set PELICANO_DANO=1`, `2> "%TEMP%\dano.txt"`).
- **Pronto:** o usuário manda o arquivo da 0.1.9 ou confirma que o "meio bugado" passou. Sem isso não
  há o que corrigir.
- **Depende:** o usuário.  **Issue:** criar (se o problema persistir).  **Tamanho:** P.

### WIN-2. Testar a 0.3.0 no notebook (instalador)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta (depende da versão nova). Desde a v0.2.7 o instalador vai para Program Files.
- **Onde:** `pacote/gerar.sh windows`, `pacote/feathersheet.nsi`; checklist: instalar, abrir uma
  planilha, Feather Sans, cantos da janela, tema automático (`system_dark`, a conferir no Windows), abrir
  `.xlsx`, Ctrl+S.
- **Pronto:** lista de verificação preenchida pelo usuário, sem erro bloqueante.
- **Depende:** LAN-5, PUB-8.  **Issue:** nenhuma.  **Tamanho:** M (do usuário).

### WIN-3. Job de Windows no CI (compilar o app)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. `.github/workflows/` continua só com `macos.yml` e `testes.yml`.
**Situação (10/11):** 🟡 escrito: `.github/workflows/windows.yml` compila `crates/feathersheet` no `windows-latest` a cada push no `main` (e à mão), com `--locked` e sem informação de depuração. A dependência do PUB-6 caiu (não há mais `[patch]`). Conferido aqui com `cargo check --locked --target x86_64-pc-windows-gnu`: compila, só avisos antigos. Falta ver o job verde no GitHub depois do próximo push.
- **Onde:** `.github/workflows/` (novo `windows.yml`, ou job em `macos.yml`); o `testes.yml` continua
  sem a GUI (regra da memória).
- **Pronto:** push em `main` compila `feathersheet` no `windows-latest` (sem rodar a suíte da GUI); job verde.
- **Depende:** PUB-6 (sem `[patch]`).  **Issue:** criar.  **Tamanho:** M.

### WIN-4. Pressão da caneta no Windows
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
- **Onde:** `feather_ui::caneta` (WM_POINTER); a janela de teste no notebook.
- **Pronto:** traço com a mesa mostra pressão; "conferido por cargo check" (14/10) passa a "conferido na
  máquina". Se a mesa não estiver no notebook, o item fica fora da 0.3.0 (D14).
- **Depende:** WIN-2; D14.  **Issue:** nenhuma.  **Tamanho:** M.

---

## Frente 8 — Segurança

### SEG-1. Token do socket MCP validado no app
**Situação (08/11):** ✅ feito em 08/11: o arquivo `porta token` nasce 0600 (`gravar_so_do_dono`, `OpenOptions::mode` onde há Unix; no Windows fica na pasta do usuário); o teste `transporte_tcp_exige_o_token` confere a permissão e que conexão sem o token certo é largada. No Linux o socket já era 0600 numa pasta 0700.
- **Onde:** `crates/feathersheet-mcp/src/ao_vivo.rs:8–9, :70, :83` (token na primeira linha); lado do app
  `crates/feathersheet/src/app/mcp_vivo.rs` (a conferir a validação e a permissão do arquivo `porta token`).
- **Pronto:** teste: conexão sem o token certo é recusada; o arquivo com o token só é lido pelo dono
  (permissão 0600, a conferir o que existe); nota no `FORMATO`/MCP.
- **Depende:** nada.  **Issue:** criar (pendência da revisão de 29/10).  **Tamanho:** M.

### SEG-2. Pânicos nos eventos de macro não derrubam o app
**Situação (08/11):** ✅ feito em 08/11: `macro_dialogo::sem_derrubar` envolve toda execução de macro (eventos, formulário, Executar, em fundo); pânico vira "a macro travou o interpretador".
- **Onde:** `crates/feathersheet/src/app/eventos_vba.rs`, `app/formulario_macro.rs` (sem `catch_unwind`);
  padrão já usado em `app/calculo_em_fundo.rs` e `app/macro_dialogo/em_fundo.rs`.
- **Pronto:** entrada ruim no evento (macro que lança erro, formulário com valor estranho) vira mensagem na
  barra de status; teste do caminho com entrada ruim.
- **Depende:** nada.  **Issue:** criar (crash: correção avulsa permitida pela regra 23).  **Tamanho:** P/M.

### SEG-3. Teto do corpo HTTP do OpenQuery (conferir)
**Situação (08/11):** ✅ feito em 08/11: `importador::ler_com_teto` (512 MB) na fonte Web; acima disso, erro pedindo para baixar o arquivo.
- **Onde:** (a conferir) onde está o cliente HTTP do OpenQuery (`app/openquery_ui*` e o crate de dados).
- **Pronto:** corpo acima do teto é recusado com erro claro; teste.
- **Depende:** nada.  **Issue:** criar se não existir.  **Tamanho:** P.

### SEG-4. Profundidade de bloco no parser VBA
**Situação (08/11):** ✅ feito. `MAX_BLOCOS` em `feathersheet-vba/src/sintaxe/instr.rs:30` (#129): bloco além do teto vira `Desconhecida`, sem estourar a pilha. Não conferi se há teste com 10 mil `If`.
- **Onde:** `feathersheet-vba/src/sintaxe/` (parser de blocos `If`/`For`/`Select` aninhados); o teto de
  nós já existe em `sintaxe/expr.rs:18` (#129).
- **Pronto:** arquivo com 10.000 `If` aninhados termina com erro de conversão (não pânico nem estouro de
  pilha); teste; a revisão de 29/10 apontava a parte de blocos (a conferir se já está fechada).
- **Depende:** nada.  **Issue:** criar se faltar.  **Tamanho:** P.

### SEG-5. Revisão de segurança das macros (fechamento)
**Situação (08/11):** ✅ conferido em 08/11: #123 `MAX_OPERACOES` (parser), #124 teto de pilha da LAMBDA (`evaluator.rs`), #125 REPT (`fn_text.rs`), #126 teto de descompactação do .fshz, #127 `macros_liberadas` no MCP, #128 versão validada antes do roteiro de atualização, #129 teto de nós do VBA, #130 `MAX_PROFUNDIDADE` do XML; todos com teste. Teste novo `reabrir_fshz_e_xlsm_com_macro_pede_para_ativar` (salva .fshz com módulo, reabre pelo `instalar`: bloqueado, e o mesmo com a extensão .xlsm; sem macro, não pergunta).
- **Onde:** `feathersheet-macros/src/riscos.rs`; `app/macro_dialogo/`.
- **Pronto:** lista dos #123–#130 conferida contra o código e fechada; "Ativar macros?" testado para
  arquivo com macro em `.xlsm` e `.fshz`.
- **Depende:** nada.  **Issue:** #123–#130 (já fechadas: só conferir).  **Tamanho:** P.

---

## Frente 9 — Paridade com outros programas (teste de 28/10)

### GRI-1. Roteiro comparativo com outro programa com dados pequenos
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
- **Onde:** a planilha é montada uma vez (`.xlsx`) e aberta nos dois.
- **Pronto:** tabela com abrir, recalcular e editar uma célula da base, nos dois, mais a conferência dos
  resultados (FILTRO, desvio padrão, INCLINAÇÃO) entre os dois e com o Excel. O usuário roda o pesado.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** M.

### GRI-2. Cálculo iterativo (referência circular com iterações)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta.
**Situação (11/11):** ✅ feito. `feathersheet-core/src/planilha/iterativo.rs` (opção global, como o idioma) e `Sheet::valor_da_volta`/`fechar_ciclo`: a célula a que a conta voltou devolve o valor da rodada anterior e repete o ciclo até o máximo ou até mudar menos que a diferença; o F9 e a troca da opção sujam os ciclos (`sujar_ciclos`). Configurações › Planilha › Cálculo iterativo (desligado por padrão; 100 e 0,001). Teste `tests/iterativo.rs` (convergência, contador, F9, ligar e desligar). Limites: ciclo que passa por outra aba anda uma iteração por recálculo; a opção não é lida nem gravada no `.xlsx`; fórmula empacotada em ciclo não tem teste próprio.
- **Onde:** `feathersheet-core/src/planilha/calculo_manual.rs` / detecção de circular (a conferir o
  módulo); opções nas Configurações.
- **Pronto:** opção "Iterações" (máximo e diferença), com teste de convergência em fórmula circular
  simples; desligada por padrão. Decisão D17.
- **Depende:** D17.  **Issue:** criar.  **Tamanho:** G.

### GRI-3. Abrir `.xlsx` protegido por senha
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Não há `EncryptionInfo` no `feathersheet-io`.
**Situação (10/11):** ✅ feito, falta o usuário abrir um arquivo cifrado pelo Excel. `feathersheet-io/src/xlsx_senha.rs` decifra as duas criptografias AES do Office: a Standard (3.2/4.2, LibreOffice e Excel 2007; conferida com `tests/arquivos/senha123_libreoffice.xlsx`, gravado pelo LibreOffice 26.8) e a Agile (4.4, Excel 2010+; conferida só com um cifrador de teste escrito pela especificação, porque não há no disco arquivo cifrado pelo Excel). Ao abrir, a leitura devolve `PEDE_SENHA` e a janela "Arquivo protegido por senha" (`app/senha_abrir.rs`) pede a senha; errada, pede de novo com aviso. A senha vai à thread da leitura e é tirada de lá ao usar. Custo (D14): `cfb`, `aes`, `cbc`, `sha1`, `sha2` no io; no app entram só `aes`, `cbc`, `cipher`, `inout` e `block-padding` (RustCrypto, Rust puro). Locks de io, app, macros, mcp e vba atualizados. Testes: 4 em `xlsx_senha` e `xlsx_com_senha_pede_a_senha_recusa_a_errada_e_abre_com_a_certa` (GUI); foto `FEATHERSHEET_CAPTURAR_DIALOGO=senha`. F1 nos três idiomas.
**Gravar com senha (10/11):** `xlsx_senha::cifrar` grava Agile como o Excel 2010+ (SHA-512, AES-256, 100 mil voltas, sal e chave do `getrandom`, HMAC do `dataIntegrity`); `decifrar` agora confere esse HMAC e recusa arquivo adulterado. O app guarda a senha da pasta aberta (`senha_documento`) e o `.xlsx` sai cifrado com ela; Revisão › Proteger › "Criptografar com senha" define, troca ou tira (duas caixas que têm de conferir); o `.fshz` não leva senha (aviso no status). Prova de fora: o LibreOffice abriu com a senha um arquivo cifrado pelo FeatherSheet e leu os dados. Testes `agile_do_excel_abre_com_a_senha` (sal novo, adulteração) e `xlsx_com_senha_salva_cifrado_e_a_senha_troca_ou_sai`; foto `FEATHERSHEET_CAPTURAR_DIALOGO=criptografar`. Falta conferir no Excel (`~/fs_senha_teste/saida/cifrado_fs.xlsx`, senha `Açaí 2026`).
- **Onde:** `feathersheet-io/src/` (leitor de `.xlsx`; a conferir se já há tratamento de `EncryptionInfo`).
- **Pronto:** pede a senha e abre; sem crate com C; teste com arquivo de senha conhecida. Decisão D14
  (dependência pura Rust, custo medido).
- **Depende:** D14.  **Issue:** criar.  **Tamanho:** G.

---

## Frente 10 — Interface, faixa, menus e idioma

### INT-1. Faixa recolhendo grupo a grupo em todas as abas
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Nada novo de recolher grupo a grupo nas outras abas.
**Situação (10/11):** ✅ feito. Fora da Página Inicial, os grupos que não cabem viram um botão (ícone, seta e o nome do grupo) que abre o grupo inteiro num painel sob ele; o que nem recolhido cabe vai ao "Mais". Plano em `plano_recolhido` (`faixa_nativa.rs`): primeiro mostra o máximo de grupos recolhidos e, da esquerda para a direita, abre inteiro cada um que ainda cabe. Os títulos são anotados no `faixa_grupo` (`TITULOS_GRUPOS`); a Desenhar, que vem pronta da feather-ui, continua com o "Mais". Fotos a 900 px: Inserir (10 recolhidos + Mais, antes 1 grupo + Mais), Fórmulas e Dados (inteiros e recolhidos misturados). Testes `plano_recolhido_abre_o_que_cabe_e_manda_o_resto_ao_mais` e `dados_em_janela_estreita_recolhe_grupo_e_abre_o_painel`; Ajuda F1 nos três idiomas. Limite: o botão recolhido usa o mesmo ícone (`layout-grid`) em todos os grupos.
- **Onde:** `crates/feathersheet/src/app/faixa_responsiva.rs`, `faixa_nativa.rs`, `faixa_nativa_grupos.rs`
  (hoje só a Início; as outras usam o "Mais", pendência de 28/10).
- **Pronto:** foto de Inserir, Fórmulas e Dados recolhidas: cada grupo recolhe sozinho; teste de
  quantos grupos cabem por largura.
- **Depende:** nada.  **Issue:** nenhuma (pendência da memória).  **Tamanho:** M.

### INT-2. #68: faixa em três níveis (aberta, recolhida, micro com os menus na barra de título)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. #68 aberta.
- **Onde:** `app/faixa_nativa.rs`, `app/menu_principal.rs`, `app/configuracoes_ui.rs`.
- **Pronto:** a escolha guarda nas Configurações; foto dos três níveis; Ajuda atualizada (DOC-1). Desenho
  já descrito na issue: aprovar antes (regra 21, D12).
- **Depende:** D12; INT-1.  **Issue:** #68.  **Tamanho:** G.

### INT-3. Menu morto: auditoria de todos os rótulos
**Situação (08/11):** ✅ feito em 08/11: teste `nenhum_item_dos_menus_fica_sem_acao` percorre todos os menus clássicos numa pasta com dados; só ficam desligados os do momento (Desfazer mesclagem sem mescla, listas vazias de macros, Excluir aba com uma aba só). Na faixa cada botão já nasce com a mensagem dele.
- **Onde:** `dados/menus.json` (a conferir: a issue #82 cita `dados/menus.json` no menu clássico);
  `app/menu_principal.rs`, `app/menu_nativo.rs`, `app/menu_contexto.rs`.
- **Pronto:** teste que percorre todos os itens e falha se algum não tem ação; "Proteger intervalo…" e
  "Intervalos protegidos…" (#82) saem ou são implementados; a lista de itens mortos fica vazia.
- **Depende:** D12.  **Issue:** #82 (e criar para o que mais aparecer).  **Tamanho:** M.

### INT-4. Exibir: Nova Janela, Organizar Tudo, Dividir, Alternar Janelas (#83)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. #83 aberta.
- **Onde:** `app/menu_principal.rs`, `app/mover_abas.rs`; janelas auxiliares do pelicano.
- **Pronto:** ou implementado (duas abas visíveis) ou fora do menu. Decidido em D12.
- **Depende:** D12.  **Issue:** #83.  **Tamanho:** G (implementar) / P (tirar).

### INT-5. Grupo Organizar para gráficos e imagens (#79)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. #79 aberta.
- **Onde:** `app/objetos.rs`, `app/objetos/interface.rs` (a conferir: o grupo hoje só existe para formas).
- **Pronto:** Avançar, Recuar, Painel de Seleção, Alinhar, Agrupar, Girar para gráficos e imagens; teste
  de ordem Z; foto.
- **Depende:** D12.  **Issue:** #79.  **Tamanho:** G.

### INT-6. Temas da página (#80) e grupo Revisão (#82)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. #80 aberta. #82 aberta: Proteger Pasta já existe; faltam Ortografia, Mostrar Alterações e Permitir Edição de Intervalos.
- **Onde:** `app/menu_principal.rs`; `feathersheet-io/src/` (tema do `.xlsx`, `xl/theme`).
- **Pronto:** D12 decide: temas da pasta do Excel (cores e fontes) ou sair do menu; a "Revisão" sem os
  itens que não existem sai do menu.
- **Depende:** D12.  **Issue:** #80, #82.  **Tamanho:** G (temas) / P (tirar).

### INT-7. Interface em inglês sem sobras em português
**Situação (08/11):** ✅ feito em 08/11: em teste o `tr` anota o que não acha (`idioma::tirar_faltas`), e `com_ingles_as_telas_principais_nao_pedem_texto_sem_traducao` desenha em inglês as 10 abas da faixa, 21 diálogos internos, a barra de status com seleção, as 10 seções das Configurações, as 6 abas do Formatar células e as 4 do OpenQuery: sobrou só a lista Função da faixa (`SOMA`/`MÉDIA`), já traduzida. As chaves literais seguem pelos testes de sempre do `idioma.rs`. Fica de fora: texto do motor (mensagens do Rust no F12, INT-8, 0.4.0); sem foto.
- **Onde:** `crates/feathersheet/src/idioma.rs`; `trf!`/textos fixos em `app/` (grep de palavras em pt
  nas telas); itens do METAS: "salvo", "Colar", grupos da Página Inicial, abas Exibir/Macros/Ferramentas,
  espessura "Média" da borda, linhas do motor no F12.
- **Pronto:** com o idioma en-US, foto das telas principais sem português; lista de palavras pt (grep)
  vazia nos textos de interface.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### INT-8. Migração do idioma: mensagens do Rust e descrições das funções
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — 🟡 parcial. Descrições, argumentos e categorias das funções já existem em en e es (`src/idiomas/funcoes.*.json`, desde 16/10). Faltam as mensagens do Rust (`explicar_erro`), a conferir.
**Situação (10/11):** 🟡 medido. A interface está traduzida: dos 3.606 textos de `tr!`/`trf!` faltam só 4 (do próprio `idioma.rs`). O que sai em português é o que nasce no motor: 105 textos (46 motivos de erro `CellError::com_motivo`, as mensagens do leitor de fórmulas e ~40 da explicação de erro em `formula/explicar.rs`), lista em `scratchpad/motor_falta.json` da sessão. Os fixos dá para traduzir na tela com `tr!`; o resumo do diagnóstico é montado no motor encaixando pedaços variáveis (`{trecho}{arg}: {d}`) e não se traduz depois de pronto. Proposta para a D15: o motor passa a montar esses textos por um gancho de tradução (função que a tela registra; sem ela, português), com modelos de marcadores nomeados no lugar do `format!`. Assim o MCP continua recebendo português.
**Situação (11/11):** ✅ feito. Os textos prontos do motor (core, io, macros, vba) passam pelo dicionário da interface (`feathersheet_core::tr!`/`trf!`), com teste de guarda; as 875 traduções foram lidas uma a uma (34 corrigidas). Varredura da interface atrás de literal fora do `tr!`: 21 títulos e 1 mensagem corrigidos (painel da dinâmica, Colar especial, Ir para, estilos de tabela, cenários, histórico de macros, faixa do FeatherQuery, editor de macros). Ficam em português de propósito os itens listados no `CLAUDE.md`.
- **Onde:** `function_help.rs` (a conferir o caminho no `feathersheet-core`); `explicar_erro` (mensagens
  de erro).
- **Pronto:** mensagens e a ajuda das funções em pt, en e es, pelo mesmo arquivo de idioma; teste de
  chave faltando.
- **Depende:** D15.  **Issue:** criar.  **Tamanho:** G.

### INT-9. Nomes das funções por idioma
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — 🟡 parcial (a conferir). O README diz que os nomes em português ou inglês seguem o idioma da interface. Faltam o espanhol e a D15.
**Situação (11/11):** 🟡 quase. O de-para de nomes existe em espanhol e francês (`funcoes.es-ES.json` e `funcoes.fr-FR.json`, 527 nomes cada) e as mensagens do motor citam a função pelo nome da língua. Não conferido nesta data: o teste com `.fshz` antigo pedido no "Pronto".
- **Onde:** `feathersheet-core` (o motor reconhece os nomes); o de-para em JSON (pt, en, es, já existe).
- **Pronto:** D15 decide. Se entrar: nomes localizados sem quebrar arquivos antigos (teste com `.fshz`
  antigo).
- **Depende:** D15.  **Issue:** criar.  **Tamanho:** G (ou fora).

### INT-10. Botão direito na guia da aba não mostra nada (conferir)
**Situação (08/11):** ✅ conferido em 08/11 por foto (`FEATHERSHEET_CAPTURAR_MENU_ABA=1`): o menu abre logo acima da guia (Nova, Renomear, Cor, Duplicar; com mais abas, Ocultar/Mover/Excluir). Teste antigo em `testes.rs` (abrir o contexto da aba e os rótulos).
- **Onde:** `app/mover_abas.rs`, `app/menu_contexto.rs`.
- **Pronto:** clique direito na guia abre o menu de aba (renomear, cor, ocultar, mover); foto.
- **Depende:** nada.  **Issue:** criar se ainda acontecer.  **Tamanho:** P/M.

### INT-11. Menu Orientação do texto: ícones e posição
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. `menu_orientacao.rs` não muda desde 26/10.
**Situação (10/11):** 🟡 conferido por foto (`FEATHERSHEET_CAPTURAR_JANELA=menu_orientacao`): abre junto ao grupo Alinhamento, ícones Tabler de traço coerentes. Corrigido: "Formatar células…" desalinhado (outra coluna de ícone e outro espaçamento) e a altura usada para não sair da tela (334 → 270, a real). Falta: "Girar texto para baixo" usa `arrow-down`, que no catálogo é um chevron antigo do FeatherSheet; a seta do Tabler (`arrow-narrow-up`/`arrow-narrow-down`) foi acrescentada ao `tabler-acoes.tsv` da feather-ui local, — entra com a próxima versão publicada da feather-ui (pede confirmação).
- **Onde:** `app/menu_orientacao.rs`.
- **Pronto:** ícones de orientação legíveis e o menu abre junto ao botão; foto.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** P.

### INT-12. Barra de progresso ao abrir fica em 100% em "Montando… na memória" (conferir)
**Situação (08/11):** ✅ feito em 08/11: a conta virou `porcentagem_de` (sem estado global) e o teste `montar_para_em_80_e_so_o_calculo_chega_a_100` trava: ler 0–40, montar até 80 (mesmo com a etapa contada a mais), calcular 80–100.
- **Onde:** `app/progresso_abertura.rs`.
- **Pronto:** o progresso só chega a 100% quando termina a montagem; teste de estado.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** P.

### INT-13. `feathersheet --version` imprime a versão e sai
**Situação (08/11):** ✅ feito em 08/11: `feathersheet --version` (ou `-V`) imprime `feathersheet <versão do Cargo.toml>` e sai sem janela; conferido no terminal.
- **Onde:** `crates/feathersheet/src/main.rs` (antes de abrir a janela, junto do `--mcp` da linha ~80);
  `app/atualizacao.rs:21` (`CARGO_PKG_VERSION`).
- **Pronto:** `feathersheet --version` imprime `feathersheet 0.3.0` e sai sem abrir janela; teste ou
  conferência manual (o usuário roda no terminal).
- **Depende:** nada.  **Issue:** criar (pedido de 29/10).  **Tamanho:** P.

### INT-14. Cursor do Claude ao vivo (#9)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. #9 aberta.
- **Onde:** `app/mcp_vivo.rs` (cada pedido chega no `update`), `grade.rs` (borda colorida do intervalo),
  `app/` (balão e histórico no F12).
- **Pronto:** borda e etiqueta "Claude" no intervalo; balão de uma linha que some; `sheet_comando` pisca o
  botão; histórico no F12. Sem a seta de mouse (proposta). Foto com MCP.
- **Depende:** D10.  **Issue:** #9.  **Tamanho:** G.

### INT-15. Janela com barra própria não redimensiona (#72): conferir e fechar
**Situação (08/11):** ✅ feito. #72 fechada em 04/11: já resolvido pela `moldura_redimensionavel_com` na `view`.
- **Onde:** `feather_ui::cromo::moldura_redimensionavel` (feather-ui v0.3.3, já no app).
- **Pronto:** redimensionar pelas bordas funciona no Linux (foto ou teste manual do usuário); fechar com
  `Fixes #72`.
- **Depende:** nada.  **Issue:** #72.  **Tamanho:** P.

---

## Frente 11 — MCP e IA

### MCP-1. Aba "IA (MCP)" nas Configurações
**Situação (08/11):** ✅ já existia (desde 26/10): Configurações ▸ IA (MCP) (`configuracoes_ui.rs:399`) com ligar/desligar, somente leitura, estado com contador de pedidos, comandos para copiar e a lista do que a IA pode fazer. Não conferi o teste "desligado não abre socket".
- **Onde:** `app/configuracoes_ui.rs` (o JSON com `--mcp` já é gerado em `:375`); `app/mcp_vivo.rs`.
- **Pronto:** ligar e desligar (desligado de fábrica); somente leitura; estado e contador de pedidos;
  comando de conexão para copiar; lista do que a IA pode fazer. Foto da aba; com o canal desligado não
  abre socket (teste).
- **Depende:** SEG-1.  **Issue:** nenhuma (METAS v0.1.0).  **Tamanho:** M.

### MCP-2. Instalador leva o `--mcp` sem instalar nada a mais (conferir)
**Situação (08/11):** ✅ conferido em 08/11: o pacote (`gerar.sh`) não leva nem pede o `feathersheet-mcp`; o binário do app com `--mcp` respondeu `initialize` e `tools/list` (34 ferramentas) por stdio; Configurações ▸ IA mostra o comando com o próprio executável. Instalar o `.deb`/`.exe` de verdade fica para o LAN-5/WIN-2.
- **Onde:** `pacote/gerar.sh`; `main.rs:80` já trata `--mcp` no binário principal.
- **Pronto:** a instalação do `.deb` e do `.exe` sem o `feathersheet-mcp` ainda deixa a IA conectar (teste
  manual no Linux; no Windows com WIN-2).
- **Depende:** LAN-5.  **Issue:** nenhuma.  **Tamanho:** P.

### MCP-3. Conciliar os erros de parâmetro do MCP
**Situação (08/11):** ✅ feito. #13 (v0.1.3): `texto::intervalo` aceita `G` e `3`, e as mensagens de erro são próprias de cada ferramenta.
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs` e as ferramentas: `sheet_filtrar` só aceita letra em
  `coluna`; `sheet_estatistica` recusa `coluna: "G"` (quer `G:G`); `sheet_substituir` usa `texto`/`por`;
  `sheet_consulta` pede `{"tipo":"agrupar",…}` mas o texto mostra `agrupar{…}`.
- **Pronto:** cada ferramenta aceita as formas que o próprio esquema mostra (ou o esquema é corrigido);
  teste por ferramenta.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** M.

### MCP-4. `sheet_tela` mostra os painéis abertos
**Situação (08/11):** ✅ feito. #12 (v0.1.3): `sheet_tela` lista a janela e o menu de formatação condicional e o seletor de gráfico (`mcp_vivo.rs:524–526`).
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs`; `app/mcp_vivo.rs` (estado da tela).
- **Pronto:** `sheet_tela` lista o seletor de gráfico e a formatação condicional quando abertos.
- **Depende:** nada.  **Issue:** criar.  **Tamanho:** P/M.

### MCP-5. `sheet_condicional` ao vivo não abre diálogo sem pedido (conferir)
**Situação (08/11):** ✅ feito. #11 fechada em 27/10 (não se reproduziu; teste em `mcp_vivo.rs:743`).
- **Onde:** `app/mcp_vivo.rs`.
- **Pronto:** a chamada de leitura não abre janela; teste.
- **Depende:** nada.  **Issue:** criar se ainda acontecer.  **Tamanho:** P.

### MCP-6. Cobertura do MCP para o que a 0.3.0 entrega
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. `MCP-PENDENTE.md` não muda desde 17/10; D23 continua aberta.
**Situação (10/11):** 🟡 a dinâmica coberta: `sheet_dinamica` criar/editar ganhou tema, subtotais, totais gerais, campos calculados, itens calculados, grupos, filtros, ordenação e "mostrar valores como", e o listar os descreve (teste `opcoes_da_dinamica_pelo_mcp`). O que falta na dinâmica, e por quê, está em `MCP-PENDENTE.md`. Gráfico e segmentação seguem com D23/GRA-9/DIN-15.
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs`, `permissoes.rs`; `docs/planejamento/MCP-PENDENTE.md`
  (124 linhas, lista do que falta).
- **Pronto:** cada feature nova da 0.3.0 (dinâmica, gráfico, segmentação) tem ferramenta ou o motivo
  escrito em `MCP-PENDENTE.md`; o subconjunto escolhido em D23 testado no MCP.
- **Depende:** D23; DIN, GRA.  **Issue:** nenhuma.  **Tamanho:** G.

### MCP-7. Ferramentas MCP do Gantt
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Depende de GRA-1.
**Situação (10/11):** ✅ já estava feito junto com o GRA-1: `sheet_grafico` aceita `gantt {hoje, situacao, concluida, andamento, futura, cor_hoje}` (`feathersheet-mcp/src/grafico.rs`, esquema em `esquemas.rs`).
- **Onde:** `crates/feathersheet-mcp/src/esquemas.rs` (`sheet_grafico`).
- **Pronto:** aceita as opções de GRA-1 (linha de hoje, cor por barra).
- **Depende:** GRA-1.  **Issue:** #133 (item 4 da issue).  **Tamanho:** P.

---

## Frente 12 — Macros (fora dos itens de segurança)

### MAC-1. Autocompletar no editor de macros (#8)
**Situação (08/11):** ❌ falta. #8 aberta.
- **Onde:** `app/macros/` (editor Alt+F11), `feathersheet-macros` (lista da API em inglês, `sheet.*`).
- **Pronto:** digitar `sheet.` mostra os métodos com assinatura e descrição do guia; `sh` → `sheet`,
  `sheet.r` → `range`; teste da lista (a lista vem do mesmo arquivo do guia).
- **Depende:** nada.  **Issue:** #8.  **Tamanho:** M.

### MAC-2. UserForm de um `.xlsm` real do usuário
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. Esperando o arquivo do usuário.
- **Onde:** `feathersheet-vba/src/formulario.rs`; `importar_vba.rs::formulario_da_conversao`.
- **Pronto:** um `.xlsm` real do usuário (pedido ao usuário) abre e o formulário aparece no designer.
  Hoje só testado com arquivos do LibreOffice (CLAUDE.md).
- **Depende:** o usuário manda o arquivo.  **Issue:** nenhuma.  **Tamanho:** M.

---

## Frente 13 — Documentação e Ajuda F1

### DOC-1. Ajuda F1 em dia com tudo que a 0.3.0 muda (pt, en, es)
**Situação (08/11):** ✅ conferido em 08/11: os 20 tópicos do Como usar têm o mesmo número de linhas em pt/en/es; AutoConclusão já estava em Primeiros passos; Dinâmicas recomendadas agora existem (DIN-5); Gantt e as contas novas da dinâmica entraram hoje.
- **Onde:** `app/ajuda.rs`, `app/ajuda_macros.rs`, `app/ajuda_atalhos.rs`; o tópico de dinâmicas
  recomendadas (DIN-5) tem de refletir a decisão.
- **Pronto:** cada atividade que mudou interface tem tópico novo ou atualizado nos três idiomas; nenhum
  tópico promete o que o app não faz; revisão por lista (uma linha por tópico).
- **Depende:** as atividades que mudam interface.  **Issue:** nenhuma (regra 24).  **Tamanho:** M.

### DOC-2. Exportar a Ajuda F1 como Markdown (#71)
**Situação (08/11):** ✅ feito. #71, v0.2.9 (`ajuda::markdown`).
- **Onde:** `app/ajuda.rs`; `feather_ui::ajuda` (catálogo).
- **Pronto:** botão grava um `.md` escolhido pela pessoa, com funções, macros e atalhos no idioma do app;
  teste de conteúdo mínimo.
- **Depende:** DOC-1.  **Issue:** #71.  **Tamanho:** M.

### DOC-3. Comparação com o Excel publicada no README (mesmo arquivo, script para repetir)
**Situação (08/11):**  ⏭️ 0.4.0 (corte de 08/11) — ❌ falta. O README compara versões do próprio app, sem a coluna do Excel.
- **Onde:** `README.md` (em inglês, regra da memória); script em `scripts/` ou `crates/feathersheet/examples/` (a conferir).
- **Pronto:** tabela no README com abrir, recalcular e salvar, do mesmo arquivo, e o script que repete a conta.
- **Depende:** DIN-16, ROB-7 (números finais).  **Issue:** nenhuma (METAS v0.2.0).  **Tamanho:** M.

### DOC-4. Conciliar `docs/planejamento/METAS-VERSOES.md`
**Situação (08/11):** 🟡 parcial em 08/11: aviso no topo do METAS com a numeração real (v0.2 = macros; 0.3.0 = este plano; o resto da 'v0.2 dinâmica' e da 'v0.3 FeatherQuery' para a 0.4.0) e as seções antigas marcadas. Falta marcar `[x]` item a item o que já saiu.
- **Onde:** `docs/planejamento/METAS-VERSOES.md`.
- **Pronto:** (a) a numeração das versões reflete o que saiu (a v0.2 foi macros; a 0.3.0 é esta lista;
  FeatherQuery na 0.4); (b) itens que já saíram marcados `[x]` (ex.: "Inserir/excluir coluna com muitas
  fórmulas trava" está `[ ]` na parte 1 e `[x]` na v0.1.1); (c) cada `[ ]` tem atividade aqui ou está
  fora de escopo.
- **Depende:** D1.  **Issue:** nenhuma.  **Tamanho:** M.

### DOC-5. Dois roteiros: raiz `ROADMAP.md` e `docs/planejamento/ROADMAP.md`
**Situação (08/11):** ✅ feito em 08/11: o `docs/planejamento/ROADMAP.md` (inventário antigo dos menus) abre apontando para o `ROADMAP.md` da raiz e para este plano.
- **Onde:** `/ROADMAP.md` e `docs/planejamento/ROADMAP.md` (a conferir se o da raiz é o público).
- **Pronto:** um só fica como fonte; o outro aponta para ele.
- **Depende:** nada.  **Issue:** nenhuma.  **Tamanho:** P.

### DOC-6. CLAUDE.md: fila de trabalho e a seção da 0.3.0
**Situação (08/11):** ❌ falta. O CLAUDE.md tem as seções v0.2.3–v0.2.9 e "Depois da v0.2.9"; não tem `## v0.3.0`, e a fila antiga está intacta.
- **Onde:** `/CLAUDE.md` (fila de trabalho desatualizada; seções de 0.1.x).
- **Pronto:** seção `## v0.3.0` no topo (como a v0.2.2); a fila antiga substituída pelo que sobra depois
  da 0.3.0.
- **Depende:** LAN-1.  **Issue:** nenhuma.  **Tamanho:** P/M.

---

## Frente 14 — Lançamento da v0.3.0

### LAN-1. Versão e nome
**Situação (08/11):** ✅ `version = "0.3.0"` (o snap anda no LAN-7).
- **Onde:** `crates/feathersheet/Cargo.toml` (`version`); `snap/snapcraft.yaml` (depois, LAN-7); `pacote/`.
- **Pronto:** `version = "0.3.0"`; `grep -rn 0.2.2` só em histórico.
- **Depende:** D1, D2 (o nome da versão).  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-2. "O que há de novo" em pt-BR, en-US e es-ES
**Situação (08/11):** ✅ entrada 0.3.0 no `VERSOES` (4 destaques + 18 linhas) em pt-BR, en-US e es-ES; os testes das novidades passam.
- **Onde:** `crates/feathersheet/src/app/novidades.rs` (testes `toda_versao_tem_traducao` e
  `versao_desta_compilacao_e_a_primeira_e_todas_desenham`).
- **Pronto:** a entrada 0.3.0 existe nos três idiomas; os dois testes passam (o usuário roda só esses).
- **Depende:** as atividades que entram na 0.3.0.  **Issue:** nenhuma.  **Tamanho:** M.

### LAN-3. Notas da release e seção do CLAUDE.md
**Situação (08/11):** ✅ seção `## v0.3.0` no CLAUDE.md, README com "New in v0.3.0"; notas em inglês na release.
- **Onde:** `CLAUDE.md` (pt); notas de release (en, sem rodapé "Generated with Claude Code"); modelo em
  `pacote/PUBLICAR.md`.
- **Pronto:** notas em inglês prontas; CLAUDE.md com a seção; revisão com o usuário.
- **Depende:** DOC-6, LAN-2.  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-4. Testes verdes no CI
**Situação (08/11):** ✅ Testes verdes no `3087115`; o macOS roda pela tag.
- **Onde:** `.github/workflows/testes.yml` (core, io, macros, vba, mcp); `macos.yml`.
- **Pronto:** todos os jobs verdes no commit de lançamento. A GUI fica só com os testes pelo nome que o
  usuário roda (regra da memória).
- **Depende:** todas as atividades de código.  **Issue:** nenhuma.  **Tamanho:** P (verificação).

### LAN-5. Gerar os pacotes de uma cópia limpa da tag
**Situação (08/11):** ✅ `.deb`, `.tar.gz` e `.exe` de um worktree da tag (binário Linux pede glibc ≤ 2.35).
- **Onde:** `pacote/gerar.sh` (num `git worktree` da tag; pacote precisa de `pacote/PUBLICAR.md` passo 2).
- **Pronto:** `.deb`, `.tar.gz`, `.exe` em `pacote/saida/0.3.0/`; o `.dmg` sai do CI do macOS (`macos.yml`).
  Build com `systemd-run … MemoryMax=6G MemorySwapMax=0 nice -n 19 taskset -c 0-3` (memória de builds).
- **Depende:** LAN-4, WIN-3 (se entrar).  **Issue:** nenhuma.  **Tamanho:** M.

### LAN-6. Release no GitHub
**Situação (08/11):** ✅ release v0.3.0 publicada com os 3 pacotes e o SHA256SUMS; o `.dmg` vem do CI. VirusTotal do `.exe` a conferir.
- **Onde:** `gh release create v0.3.0` (nomes sem versão, como em PUBLICAR.md passo 3); `SHA256SUMS`.
- **Pronto:** release com .deb, .tar.gz, .exe, SHA256SUMS e o .dmg do CI; notas em inglês.
- **Depende:** LAN-3, LAN-5; D4 (autorização).  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-7. Snap: beta e depois stable
**Situação (08/11):** ✅ snap 0.3.0 no stable (revisão 4).
- **Onde:** `snap/snapcraft.yaml` (version e URL do .deb); `pacote/PUBLICAR.md` passo 4.
- **Pronto:** `snapcraft pack` com o `.deb` da release; instalado com `--dangerous`; enviado ao beta e
  depois ao stable.
- **Depende:** LAN-6.  **Issue:** nenhuma.  **Tamanho:** P.

### LAN-8. Repositório apt (`sudo apt install feathersheet`)
**Situação (08/11):** ❌ falta. D13 continua aberta. Novo: manifesto Flatpak e AppStream para o Flathub (`c3305f2`).
- **Onde:** a decidir (repositório `feather-works/apt` ou pasta no feathersheet); chave GPG.
- **Pronto:** D13 decidido; repositório publicado; `apt install feathersheet` instala a versão 0.3.0 numa
  máquina limpa (teste do usuário).
- **Depende:** D13; LAN-6.  **Issue:** nenhuma.  **Tamanho:** G.

### LAN-9. Conferência de uso com os arquivos do usuário (roteiro)
**Situação (08/11):** ❌ falta.
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
