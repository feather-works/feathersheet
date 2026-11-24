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

- [x] Caminho de volta JS → VBA gravado no `.xlsm` e teste de ida e volta (24/11: compressão, gravador do
      `vbaProject.bin`, tradutor, Salvar `.xlsm`, eventos); conferido no LibreOffice e no olevba (24/11). Excel de
      verdade: só se o usuário tiver um à mão.
- [x] Relatório de conversão numa tela própria (24/11): diálogo no editor de macros, referência o Verificador de
      Compatibilidade do Excel; o módulo de pendências continua.
- [ ] Bateria com pelo menos 50 `.xlsm` reais (#143); meta de 95% rodando igual.
- [x] Integração com o FeatherDocs: `docs.fill` (modelo → `.docx`) e o conversor do `Word.Application` (mala
      direta do VBA), 24/11.
- [ ] Caso da "Planilha de Informações" com o `.xlsm` real do usuário (conferir o que mais do Word ele usa).

## Falta — interface (telas novas: confirmar antes)

- [ ] #83: divisão em quatro, Nova janela, Organizar tudo, Alternar janelas.
- [x] #79: girar e esticar o grupo inteiro no app (24/11); o `grpSp` já ia e voltava no `.xlsx`. Issue completa.
- [ ] #80: Temas da página (cores, fontes, efeitos).
- [x] Gerenciador de nomes: escolher a aba (Escopo) ao criar um nome (24/11).
- [ ] INT-11: ícone da seta no menu Orientação — depende de publicar a feather-ui nova (**pede ordem**).

## Falta — dinâmica e gráficos

- [ ] Gráfico dinâmico completo: botões de campo, todos os tipos, ida e volta no `.xlsx`.
- [ ] #145: segmentação ligada a várias dinâmicas e linha do tempo feitas no app (24/11); falta só a ida e volta no
      `.xlsx` (slicerCache/timelineCache do Excel; sem arquivo do Excel de exemplo para conferir).
- [ ] Atualizar só o grupo que mudou; arrastar campo ≤ 100 ms com 1 milhão de linhas.
- [ ] Metas de número da dinâmica (o usuário mede).

## Falta — memória e desempenho

- [ ] Resultado direto na coluna (sem `calc.cache`), `.xlsx` com dados tipados, abrir sob demanda (`mmap`).
- [ ] #142: recálculo.
- [x] Excluir milhares de trechos filtrados sem parar a janela (em fundo, 24/11). Falta só ficar rápido numa
      pasta enorme (vários cortes numa operação do motor).

## Falta — FeatherQuery e MCP

- [x] Escolher um arquivo direto como segunda fonte de Juntar/Acrescentar (24/11).
- [x] #94: tipo clicável, script legível, menu parado, nome/descrição das etapas, divisões da faixa e prévia
      fixa atrás do modal (`2974de4`) já estavam; Moeda e Percentual entraram em 24/11. Faixa conferida por foto.
- [x] MCP: PDF, imagens, tabelas, proteger aba e pasta (com portão), links, congelar/zoom/grade, configurar página: feitos.

## Para decidir (usuário)

- Decimal em fórmula digitada em inglês aparece com ponto em português (`=A1*0.25`): padronizar o texto
  guardado com vírgula muda como todo arquivo grava fórmula.
- #27: só fecha com um Excel de verdade. UserForm de um `.xlsm` do usuário. Comparação com o Excel no README.
