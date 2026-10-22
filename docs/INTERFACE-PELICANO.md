# Interface Pelicano

Migração iniciada em 22/10/2026, com Pelicano v0.1.9 por tag. A interface nova
roda em uma janela Pelicano; o estado e os comandos do aplicativo existente
continuam responsáveis pelo motor, validação, histórico e arquivos. A ponte
temporária `app/pelicano/tarefas.rs` executa as tarefas desses comandos sem
abrir um segundo runtime de janela.

## Executar

Na raiz do repositório:

```sh
cargo run --manifest-path crates/feathersheet-gui/Cargo.toml --features pelicano -- --pelicano
```

Um caminho de arquivo depois de `--pelicano` abre a pasta existente. Para
uma captura sem janela, acrescente `--capturar /tmp/feathersheet.png`.

## Implementado

- Janela com barra própria, arrasto, redimensionamento, minimizar, maximizar e fechar.
- Temas claro e escuro, faixa com Arquivo, Página Inicial, Inserir, Fórmulas, Dados e Exibir.
- Grade em canvas: virtualização nos dois eixos, dimensões variáveis, ocultação,
  painéis congelados, seleção com arrasto e barras de rolagem existentes.
- Barra de endereço e fórmulas, navegação pelo teclado, edição, copiar/recortar/colar,
  desfazer/refazer, formatação básica, ordenação, AutoSoma e inserção de linhas/colunas.
- Abas: selecionar, adicionar e duplicar. Zoom na barra de status e Ctrl+roda.
- Abrir arquivos pelos leitores existentes; salvar FSHZ e exportar Excel, CSV, TSV e JSON.
- Avisos bloqueantes de alterações não salvas e validação; salvar antes de fechar
  usa o mesmo fluxo assíncrono e a mesma gravação atômica da interface atual.

## Pendências para substituir a interface padrão

Esta etapa **não encerra a migração**. A interface padrão continua Iced.
Ainda precisam de desenho, integração e verificação em Pelicano:

- Tela inicial, recentes, modelos, configurações e assistente de recuperação.
- Demais diálogos de formatação, busca, filtros, tabelas, validação e proteção.
- Mesclas e bordas completas no desenho, gráficos, imagens, formas, SmartArt e tinta.
- Tabelas dinâmicas, OpenQuery, impressão, editor de macros, MCP e janelas auxiliares.
- Navegação nas sugestões pelo teclado e seleção de referências durante a edição da fórmula.
- Paridade de acessibilidade da grade, idiomas e aferição de desempenho em arquivos grandes.

Recursos não migrados não ganham botões sem ação. Os dados continuam no mesmo
Workbook, mas a grade nova ainda não representa todos os objetos e formatos.
O Iced segue como dependência enquanto o estado e os comandos forem compartilhados.

## Verificar

```sh
cargo check --manifest-path crates/feathersheet-gui/Cargo.toml --features pelicano --offline
FEATHERSHEET_PELICANO_PREVIAS=/tmp cargo test --manifest-path crates/feathersheet-gui/Cargo.toml --features pelicano --offline app::pelicano::testes
```

O teste de captura gera `/tmp/feathersheet-claro.png` e `/tmp/feathersheet-escuro.png`
quando a variável de prévias é definida. Sem ela, valida os PNGs em memória.
