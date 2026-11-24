# Rumo à 0.4.0

A 0.3.6 foi pulada (decisão de 24/11/2026): o que entra depois da 0.3.5 sai na **0.4.0**, e a versão só é
lançada quando o usuário pedir. Esta é a lista viva do que falta; o detalhe de cada entrega fica no topo do
`HISTORICO.md`. Os planos antigos (`ATIVIDADES-0.3.0.md`, os ⏭️ de 08/11) estão desatualizados: muito do que
eles marcam como falta saiu entre a 0.3.1 e a 0.3.5.

Regras desta fase: toda mudança com teste que confira o comportamento; cuidado com a língua (moeda, decimal,
milhar, datas, nomes de função) nas quatro línguas.

## Feito na fila da 0.4.0

- [x] Eventos da pasta: Workbook_Open (no "Ativar macros"), BeforeSave e BeforeClose com Cancel; os módulos
      "Executar ao abrir" passaram a rodar; eventos do VBA importado valem depois de reabrir.
- [x] Nomes de valor/fórmula e nomes de uma aba só (motor, `.xlsx`, `.fshz`, `.xlsb`, Gerenciador, MCP).
- [x] Largura de antes de ocultar no `.fshz`; `enxugar.rs` conhece os campos novos da aba.
- [x] Grupo de formas gravado no `.xlsx` (#79, parte).
- [x] FeatherQuery: consulta que mescla/acrescenta outra consulta reabre e atualiza.
- [x] Excluir linhas com filtro e muitos trechos vai para outra thread (a janela não para).

## Falta — macros (o tema da 0.4)

- [ ] Caminho de volta JS → VBA gravado no `.xlsm` (o Excel abre e roda) e teste de ida e volta.
- [ ] Relatório de conversão numa tela própria (hoje é o módulo de pendências). **Tela nova: confirmar.**
- [ ] Bateria com pelo menos 50 `.xlsm` reais (#143); meta de 95% rodando igual.
- [ ] Integração com o FeatherDocs: API de documentos nas macros (`docs.open/replace/saveAs`), conversor do
      `Word.Application`, mala direta, caso da "Planilha de Informações".

## Falta — interface (telas novas: confirmar antes)

- [ ] #83: divisão em quatro, Nova janela, Organizar tudo, Alternar janelas.
- [ ] #79: girar e esticar o grupo inteiro no app.
- [ ] #80: Temas da página (cores, fontes, efeitos).
- [ ] Gerenciador de nomes: escolher a aba (Escopo) ao criar um nome.
- [ ] INT-11: ícone da seta no menu Orientação — depende de publicar a feather-ui nova (**pede ordem**).

## Falta — dinâmica e gráficos

- [ ] Gráfico dinâmico completo: botões de campo, todos os tipos, ida e volta no `.xlsx`.
- [ ] #145: segmentação ligada a várias dinâmicas e linha do tempo.
- [ ] Atualizar só o grupo que mudou; arrastar campo ≤ 100 ms com 1 milhão de linhas.
- [ ] Metas de número da dinâmica (o usuário mede).

## Falta — memória e desempenho

- [ ] Resultado direto na coluna (sem `calc.cache`), `.xlsx` com dados tipados, abrir sob demanda (`mmap`).
- [ ] #142: recálculo.
- [x] Excluir milhares de trechos filtrados sem parar a janela (em fundo, 24/11). Falta só ficar rápido numa
      pasta enorme (vários cortes numa operação do motor).

## Falta — FeatherQuery e MCP

- [ ] Escolher um arquivo direto como segunda fonte de Juntar/Acrescentar.
- [ ] #94: tipo de dado clicável e script legível.
- [ ] MCP: tabelas, links, imagens, congelar/zoom, impressão, proteção (`MCP-PENDENTE.md`).

## Para decidir (usuário)

- Decimal em fórmula digitada em inglês aparece com ponto em português (`=A1*0.25`): padronizar o texto
  guardado com vírgula muda como todo arquivo grava fórmula.
- #27: só fecha com um Excel de verdade. UserForm de um `.xlsm` do usuário. Comparação com o Excel no README.
