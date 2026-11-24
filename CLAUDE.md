# FeatherSheet — guia de trabalho

Planilha em Rust. Interface no **pelicano + feather-ui** (`crates/feathersheet`, binário `feathersheet`).
O Iced e a UI TypeScript/Tauri foram aposentados (a TS histórica está em `descarte/feathersheet/legacy-frontend/`);
uma interface web futura usará os motores via WebAssembly.

**Histórico completo** (cada entrega, versão, medição e decisão até 20/11/2026): `docs/planejamento/HISTORICO.md`.
Antes de mexer num recurso, procure o nome dele lá (`grep`): quase tudo tem nota de onde mora, limites e teste.
Lista do que falta para a 0.4.0: `docs/planejamento/RUMO-0.4.0.md`. Outros planos: `docs/planejamento/` (ROADMAP, PARIDADE-EXCEL, AUDITORIA-2026-11-13, MCP-PENDENTE, METAS-VERSOES).

## Estado

- Última versão: **v0.3.5** (23/11/2026). O que entrar depois vai para a **0.4.0** (a 0.3.6 foi pulada): anotar no topo do `HISTORICO.md`. Versão só sai quando o usuário pedir.
- Dependências só por tag/crates.io: pelicano 0.1.34, feather-ui v0.3.16, feather-charts v0.3.5, feather-io, Falcão
  (motor das macros). Mexer numa biblioteca junto: `[patch]` local provisório, trocar pela tag antes de lançar.
- Línguas da interface: pt-BR, en-US, es-ES, fr-FR.

## Crates

| Crate | O quê |
|---|---|
| `feathersheet-core` | motor: células, fórmulas, cálculo, dinâmica, consultas. **Sem dependência nenhuma.** |
| `feathersheet-io` | `.fshz` (Arrow), `.xlsx`/`.xlsm`, CSV, Parquet, SQLite, senha |
| `feathersheet-macros` | API das macros (JS no Falcão), riscos, formulários |
| `feathersheet-vba` | conversor VBA → JavaScript |
| `feathersheet-mcp` | servidor MCP (`sheet_*`), avulso ou ao vivo pelo app |
| `feathersheet` | a interface |

## Comandos

```bash
cd crates/feathersheet-core && cargo test     # motor, compila em segundos
cargo clippy --all-targets
```
Builds pesados: limitar memória (ver memória "Limitar memória dos builds"); nada de suíte inteira da GUI.
Medir velocidade: perfil `medir` (release sem LTO). Exemplos de medição no `feathersheet-io/examples/`.

## Regras

1. **Português** em interface, comentários e erros. Commit com prefixo `claude:`.
2. **Nada de item de menu morto.** Teste `nenhum_item_dos_menus_fica_sem_acao`.
3. **Conferir que a função existe** antes de usá-la em fórmula gerada por código (regra que não analisa não pinta e não dá erro).
4. **`feathersheet-core` não ganha dependência.** O que precisa de crate vai para o `io` ou para o app.
5. **Nunca travar a thread da janela**: abrir, colar em massa, recalcular e gráfico pesado vão para fora dela.
6. **Bug reportado vira issue** em `feather-works/feathersheet`; `Fixes #N` no commit. Versões só em lote; avulsa só se crasha.
7. **Toda funcionalidade nova ou mudada atualiza a Ajuda F1** (`ajuda.comoUsar` nas quatro línguas, `TOPICOS` em
   `app/ajuda.rs`, caminho de menu conferido em `app/faixa_nativa_grupos.rs`; funções no catálogo, API de macro em
   `ajuda_macros.rs`, atalhos em `ajuda_atalhos.rs`).
8. **Novidade** de cada entrega vai em `VERSOES` (`app/novidades.rs`) nas quatro línguas.
9. **Texto novo nas quatro línguas.** No motor, mensagem nova nasce com `tr!`/`trf!` (chave = texto em português);
   o teste `todo_texto_pronto_do_motor_esta_nos_dicionarios` cobra. `tr!(variavel)` e literal passado a ajudante
   (`grupo("…")`) não são vistos pelo teste: conferir à mão. Antes de criar chave de uma palavra, ver se ela já existe
   com outro sentido. Nome visível é **FeatherQuery**, nunca OpenQuery (identificadores antigos ficam).
10. **Tela nova**: mirar numa referência (Excel/Sheets ou algo do próprio app), mesmo acabamento do resto,
    reaproveitar o diálogo mais parecido (`App::dialogo`, `feather_ui::dialogo`), descrever em 2-3 frases e o
    usuário confirma antes do código. Conferir por foto (abaixo) antes de devolver.
11. **Depois de mesclar trabalho de agente**, uma revisão do conjunto: duplicação, nomes inconsistentes, doc desatualizada.
12. Limite conhecido de um atalho fica no código como `// shortcut: <limite>`.

## Cuidados ao mexer

- **Campo novo na aba** (`Sheet`, JSON opaco: `pg`, `mg`, `tn`, `bt`…): incluir no `feathersheet-io/src/enxugar.rs`
  (`deny_unknown_fields` falha alto ao abrir arquivo real) e na ida e volta `.fshz`/`.xlsx`.
- **Inserir/excluir linhas ou colunas** esquece o histórico das abas mexidas; o Ctrl+Z da operação é uma foto da pasta
  (`app/desfazer_estrutura.rs`). Lugar novo que insere/exclui: `foto_antes_da_estrutura` antes e
  `guardar_foto_da_estrutura` depois; minigráficos: `refluir_minigraficos`.
- **Macros guardam o id da aba, não o índice** (`__sheet(i)`), porque `addSheet`/`deleteSheet` mudam os índices.
- **`parse_literal` não muda com a língua**: o `.fshz` guarda o texto digitado. Separador de milhar sai só na
  digitação/colagem (`input::sem_milhar_digitado`).
- **`to_text` do lógico fica em português** (chave de coluna e filtros salvos); o que aparece usa `to_text_formula`
  e `ErrorCode::na_lingua()`. `as_str()` dos erros é o canônico em inglês.
- **Função que devolve matriz** precisa ser reconhecida como tal (`maybe_matrix`) para derramar; o derramamento mora em
  `planilha/sheet/derramamento.rs` e o `.fshz` guarda só a âncora.
- **Fórmula por padrão**: coluna preenchida para baixo é um padrão, não N fórmulas; o `pattern_check` decide (uma vez
  por padrão) o que vai para a coluna empacotada.
- **Ordem de carga do `.fshz`**: avulsas antes das colunas, senão sujam as fórmulas e o valor gravado se perde.
- **Gravação atômica** (`.parcial` + renomear). Cópia de recuperação ≠ o arquivo; o Ctrl+S de `.xlsx` com partes não
  convertidas (`nao_convertidos`) pede "Salvar como".
- **Texto do `.xlsx` entra com `MARCA_TEXTO`** e não passa pelo `parse_literal`.
- **Vínculo externo** de pasta aberta não lê nada sozinho até "Atualizar valores".
- Conta de "abaixo das abas" usa `App::topo_abaixo_abas`, nunca número fixo. Ícone novo: conferir com
  `feather_ui::icones::existe` (inexistente vira "+").
- Opções globais do motor (idioma, listas da alça, cálculo iterativo): teste em arquivo próprio em `tests/`.

## Conferir a tela sem o usuário

`FEATHERSHEET_CAPTURAR=/tmp/tela.png ./target/release/feathersheet [arquivo]` abre, fotografa (1280×800) e fecha.
Modificadores mais usados (lista completa no `HISTORICO.md`, seção "Conferir a tela sem o usuário", e no código,
`grep FEATHERSHEET_CAPTURAR_`):

- `_CELULA=B5:D9`, `_CELULAS='A1==SOMA(B1:B3)|E1=30'`, `_ABA=2`, `_ZOOM=75`, `_CLIQUE=A1`
- `_FAIXA=formulas|exibir|micro`, `_MENU_CLASSICO=2`, `_MENU=Lógica`
- `_DIALOGO=<nome>` (diálogos internos: `colar_especial`, `filtro_avancado`, `validacao`…), `_JANELA=<nome>`
  (`informacoes`, `condicional`, `openquery[:etapa]`, `busca_macros`…)
- `_GRAFICO=painel:<tipo>`, `_DINAMICA=filtro|menu|resumir`, `_AJUDA=SOMA`, `_TINTA=1`, `_COMENTARIOS=…`
- `_FICAR=1` deixa a janela aberta para um `PELICANO_ROTEIRO` clicar.

## MCP

`feathersheet-mcp` (stdio) e ao vivo pelo app em `$XDG_RUNTIME_DIR/feathersheet/mcp.sock` (`FEATHERSHEET_MCP=0`
desliga). Mudou ferramenta: recompilar o MCP **e** o app. Regra de volume: nunca despejar a planilha, só estrutura,
amostra e estatística; cada escrita = um Ctrl+Z.
