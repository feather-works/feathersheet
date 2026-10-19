# Roadmap do FeatherSheet — até a 1.0 (100% funcional)

Este roteiro junta, num lugar só, tudo o que falta para o FeatherSheet ser
**100% funcional**: cada comando que o usuário de Excel procura existe e
funciona, cada fórmula comum calcula igual, cada arquivo `.xlsx` abre e grava
sem perder nada, e as macros do dia a dia rodam.

Os números vêm dos levantamentos detalhados (conferidos em 15/10/2026):

| Frente | Hoje | Levantamento |
|---|---|---|
| Comandos da faixa | **211 / 290** (72,8%) | [`PARIDADE-EXCEL.md`](docs/planejamento/PARIDADE-EXCEL.md) |
| Aba Desenvolvedor | **4 / 19** | idem, seção "Desenvolvedor" |
| Funções de planilha | **457 / 520** | [`FUNCOES-FALTANTES.md`](docs/planejamento/FUNCOES-FALTANTES.md) |
| Funções do VBA no conversor | **52** cobertas, **12** parciais, **53** faltando (de 117 aplicáveis) | [`PARIDADE-VBA-FUNCOES.md`](docs/planejamento/PARIDADE-VBA-FUNCOES.md) |

## O que conta como 1.0

1. **Abrir e gravar** `.xlsx`, `.xlsm`, `.csv`, `.parquet` e `.fshz` sem perder
   conteúdo, formato, gráficos, formas, nomes nem validações.
2. **Todos os comandos** das abas Arquivo, Página Inicial, Inserir, Layout da
   Página, Fórmulas, Dados, Revisão, Exibir e das abas contextuais (tabela,
   gráfico, forma, tabela dinâmica) ligados a uma ação de verdade, exceto o que
   está em [Fora de escopo](#fora-de-escopo).
3. **Funções**: todas as do Excel 365 que não dependem de serviço on-line.
4. **Macros**: as funções do VBA aplicáveis convertidas, gravador de macros e
   macros de evento.
5. **Sem regressão**: testes do motor, do io e da interface verdes; arquivos
   grandes do usuário medidos a cada versão.

---

## v0.0.1 — primeira versão instalável (agora)

O estado atual, empacotado: Linux (`.deb` e `.tar.gz` portátil) e Windows
(instalador `.exe`), sem atualização automática. O executável Linux desta versão exige
glibc 2.43 (Ubuntu 26.04 ou mais novo).

- [ ] Compilar o Linux contra uma glibc antiga, para rodar em Ubuntu 22.04/24.04,
      Debian 12 e Mint.
- [ ] Outros formatos: AppImage, `.rpm` e macOS.

## Fase 1 — Confiança no uso diário

O que pode fazer alguém perder trabalho ou desistir no primeiro dia.

- [ ] Testes da interface (`feathersheet-gui`) todos verdes.
- [ ] `.fshz` muito grande: gravar acima de 4 GB e plano de memória de 400 MB
      ([`PLANO-MEMORIA-400MB.md`](docs/planejamento/PLANO-MEMORIA-400MB.md)).
- [ ] Abrir `.xlsx` grande mais rápido (medido com os arquivos reais).
- [ ] Fidelidade do `.xlsx`: linhas com altura zero, altura padrão por aba,
      bordas média e grossa, cabeçalhos de linha/coluna ocultos, zoom por aba,
      formatos `dxf` de tabela, título de gráfico em cascata duplicado.
- [ ] Nome definido usado como intervalo em `CONT.SE` e como lista em `CORRESP`.
- [ ] Título da janela ao abrir arquivo que não é `.fshz` (hoje "Sem título.fshz").
- [ ] Abrir e reparar arquivo corrompido.
- [ ] **Vários FeatherSheet abertos ao mesmo tempo trabalhando juntos** (hoje cada
      janela é isolada):
      - copiar e colar entre janelas mantendo fórmulas, formatos e validações
        (não só o texto);
      - referência a outra pasta aberta (`=[Vendas.fshz]Plan1!A1`), recalculando
        quando a outra muda, e vínculos quando ela está fechada;
      - mover ou copiar uma aba para outra pasta aberta;
      - Exibir → Alternar janelas, Organizar tudo e lado a lado com rolagem
        sincronizada entre as pastas abertas;
      - aviso ao abrir o mesmo arquivo duas vezes (somente leitura na segunda).

## Fase 2 — Completar a faixa (79 comandos)

Em ordem de quanto o usuário sente falta.

**Layout da Página (8 / 23)** — o que mais pesa para imprimir:
- [ ] Margens, Quebras, Imprimir títulos, Exibir títulos, Plano de fundo.
- [ ] Temas, Cores, Fontes e Efeitos do tema.
- [ ] Organizar objetos: Avançar, Recuar, Alinhar, Agrupar, Girar, Painel de seleção.

**Inserir (11 / 25):**
- [ ] Caixa de texto, Cabeçalho e rodapé, Símbolo, Equação.
- [ ] Minigráficos, Gráficos recomendados, Gráfico dinâmico.
- [ ] Segmentação de dados e Linha do tempo.
- [ ] SmartArt (o motor já existe no [`feather-shapes`](https://github.com/feather-works/feather-shapes)), Instantâneo, Mapas, Linha de assinatura, Objeto.

**Exibir (9 / 20):**
- [ ] Modos Normal e Layout da página, modos de exibição personalizados, Régua.
- [ ] Zoom na seleção.
- [ ] Janelas: Nova janela, Organizar tudo, Dividir, Ocultar, Reexibir, Alternar.

**Revisão (7 / 18):**
- [ ] Ortografia e Dicionário de sinônimos (dicionários Hunspell locais, como no FeatherSlide).
- [ ] Estatísticas da pasta, Verificar acessibilidade, Texto alternativo.
- [ ] Mostrar comentários, Converter em comentários.
- [ ] Proteger pasta de trabalho, Permitir edição de intervalos.
- [ ] Mostrar alterações.

**Dados (18 / 28):**
- [ ] Consolidar, Circular dados inválidos, Relações.
- [ ] Planilha de previsão, Solver.
- [ ] Atualizar tudo (consultas e conexões).
- [ ] Listas personalizadas, Formulário de dados.

**Abas contextuais:**
- [ ] Design da Tabela: Resumir com tabela dinâmica, Inserir segmentação,
      opções de estilo (linha de cabeçalho, primeira e última coluna).
- [ ] Design do Gráfico: Layout rápido, Estilos, Alternar linha/coluna, Mover gráfico.
- [ ] Formatar (forma): Formatar seleção, Preenchimento e Contorno da forma.
- [ ] Tabela dinâmica: Classificar, Limpar tudo, Totais gerais.

**Arquivo (10 / 11):**
- [ ] Exportar como página da Web.

## Fase 3 — Fórmulas (63 funções)

1. [ ] **Referência e lambdas**: `OFFSET`, `INDIRECT` (referência dinâmica no
       avaliador), `BYROW`, `BYCOL`, `SCAN`, `MAKEARRAY`, `ISFORMULA`,
       `FORMULATEXT`, `SHEET`, `SHEETS`, `GETPIVOTDATA`.
2. [ ] **Regressão**: `LINEST`, `LOGEST`.
3. [ ] **Financeiras de cupom** (`COUP*`, `PRICE`, `YIELD`, `DURATION`, `ODD*`)
       e **engenharia** (`BESSELK`, `BESSELY`).
4. [ ] **Cálculo**: referências 3D (`Jan:Dez!B5`), cálculo iterativo (referência
       circular intencional), estilo L1C1, precisão conforme exibido, datas 1904.
5. [ ] **Texto rico na célula** (negrito, cor ou sobrescrito numa palavra só).

A lista completa, por categoria, está em
[`FUNCOES-FALTANTES.md`](docs/planejamento/FUNCOES-FALTANTES.md).

## Fase 4 — Macros

- [ ] **Funções do VBA que faltam no conversor** (53), começando pelas mais usadas:
      datas (`DateAdd`, `DateDiff`, `DatePart`, `DateSerial`, `DateValue`,
      `TimeSerial`, `TimeValue`, `Hour`, `Minute`, `Second`, `Weekday`,
      `WeekdayName`, `MonthName`, `IsDate`, `CDate`), formatação
      (`FormatNumber`, `FormatCurrency`, `FormatPercent`, `FormatDateTime`),
      texto (`StrComp`, `StrConv`, `Filter`), lógica (`Choose`, `Switch`,
      `IsError`, `IsMissing`, `TypeName`, `VarType`), financeiras (`Pmt`, `PV`,
      `FV`, `NPV`, `IRR`, `Rate`, `NPer`, `IPmt`, `PPmt`, `MIRR`, `SLN`, `SYD`,
      `DDB`) e o restante da [lista](docs/planejamento/PARIDADE-VBA-FUNCOES.md).
- [ ] Completar as 12 parciais.
- [ ] **Gravador de macros** (gravar ações gerando JavaScript) e usar referências relativas.
- [ ] **Macros de evento** iguais às do Excel (abrir, alterar célula, trocar de aba…).
- [ ] Aba Desenvolvedor: segurança de macro, controles de formulário e modo de design.

## Fase 5 — Gráficos

O motor é o [`feather-charts`](https://github.com/feather-works/feather-charts).

- [ ] Todos os tipos no desenho formatado: degraus, áreas empilhadas, radar,
      combinado, bolhas, histograma, cascata, velas, Gantt, mapa de calor, linha 3D.
- [ ] Elementos analíticos: linha de tendência com equação e R², barras de erro,
      linhas de máximo/mínimo.
- [ ] Gráfico dinâmico (segue a tabela dinâmica), minigráficos, modelos de gráfico.
- [ ] Dica ao passar o mouse e arrastar título e legenda dentro do gráfico.

## Fase 6 — Dados avançados

- [ ] Conector SQLite (Rust puro, só leitura) e funções `SQL()` / `QUERY()`.
- [ ] Segmentação de dados e linha do tempo ligadas às tabelas dinâmicas.
- [ ] Modelo de dados com relacionamentos (Power Pivot), sobre o armazenamento
      por colunas que o motor já tem.
- [ ] Analisar dados por IA local (pelo servidor MCP, só por pedido).
- [ ] Pendências do servidor MCP ([`MCP-PENDENTE.md`](docs/planejamento/MCP-PENDENTE.md)).

## Fase 7 — Polimento para a 1.0

- [ ] Área de transferência com vários itens.
- [ ] AutoCorreção.
- [ ] Modo de toque (faixa espaçada).
- [ ] Planilha da direita para a esquerda.
- [ ] Inquire: comparar pastas e limpar formatação em excesso.
- [ ] Instalador geral da suíte (escolher todos os aplicativos ou só alguns).

---

## Fora de escopo

Ficam de fora por dependerem de nuvem, conta ou do Windows, o que contraria a
regra de privacidade da suíte (tudo local, sem conta, sem telemetria):

- Coautoria em tempo real, publicar no Power BI ou SharePoint.
- Tipos de dados vinculados (Ações, Geografia), `STOCKHISTORY`, tradutor e
  pesquisa inteligente on-line, `TRANSLATE`/`DETECTLANGUAGE`.
- OLE, DDE, RTD, suplementos COM e do Excel, funções de cubo OLAP.
- Data Streamer (porta serial) e mapeamento XML com esquema, até alguém pedir.

Funções que buscam dados de fora (`WEBSERVICE`, `FILTERXML`) só entram como
conector de mão única, por pedido explícito do usuário.
