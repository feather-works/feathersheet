# FeatherSheet — histórico de entregas e decisões (antigo CLAUDE.md, até 20/11/2026)

> **Decisão atual (12/10/2026):** não desenvolver nem manter o frontend TypeScript/Tauri. A interface oficial é Rust/Iced; uma interface web futura utilizará os motores Rust via WebAssembly. A UI TS histórica está em `descarte/feathersheet/legacy-frontend/src/`. Entradas antigas neste documento servem como referência de migração, não como estado vigente.

## Depois da v0.3.5 (para a 0.4.0 — a 0.3.6 foi pulada, decisão de 24/11)

- **MCP: PDF da aba** (24/11/2026, `sheet_impressao` acao `pdf`, ao vivo): a página da aba (papel, orientação,
  escala, margens, áreas) vira `ConfigPagina` como no Salvar PDF; gravação atômica; `shortcut:` gera na thread da
  janela, por isso o teto de 300 mil células (acima, a IA é mandada para Arquivo ▸ Imprimir). Teste `pdf_pelo_mcp`.

- **MCP `sheet_imagem`** (24/11/2026, ao vivo): listar, inserir de arquivo (o mesmo `importar` do Inserir ▸
  Imagem: PNG/JPEG, 16 MiB conferidos antes de ler, 64 por aba) e remover. Teste `imagem_pelo_mcp`.

- **MCP `sheet_tabela`** (24/11/2026, ao vivo): listar, criar (nome sugerido se faltar), renomear, estilo,
  total por coluna (pela letra ou pelo título; liga a linha de totais e grava a conta num Ctrl+Z, como o botão),
  redimensionar e converter. `Hospede::tabela` (avulso explica que precisa do app); `tabelas::listar` virou
  `pub(super)`. Teste `tabela_pelo_mcp` (soma 31,75 na linha de totais). MCP também ganhou bloquear/desbloquear
  células no `sheet_proteger`.

- **MCP respeita a proteção** (24/11/2026; furo achado ao fazer o "proteger a pasta"): o `chamar` ganhou o portão da
  tela (marca o histórico das abas protegidas, roda, `examinar_protecao`; se tocou no proibido, `desfazer_ate` e a IA
  recebe o erro). Antes o avulso escrevia em célula bloqueada e o ao vivo respondia "feito" do que a tela desfazia.
  `sheet_estrutura` confere a permissão antes (inserir/excluir não entram no desfazer); `sheet_aba`
  criar/renomear/excluir recusa com a pasta protegida; `sheet_proteger` ganhou `alvo: "pasta"`. Teste
  `protecao_vale_para_a_ia`. Fica como está: as macros escrevem em aba protegida de propósito (o
  Unprotect → escrever → Protect do VBA convertido é aplicado no fim), e (mesmo dia) o portão também desfaz aba criada ou
  renomeada por outra ferramenta (`sheet_consulta` com destino novo, dinâmica) com a pasta protegida.

- **Dinâmica: botão direito no campo de uma área abre o menu dele** (#165, 24/11/2026; antes só a setinha ▾ abria):
  `Evento::MenuItemEm` com o ponto do `on_right_press_at`, que segura o clique (o painel de fora não o recebe).
  Teste `botao_direito_no_campo_abre_o_menu_onde_clicou`. Aberto junto: #166, a janela travou ~30 s no
  `2026_03 tabela dinamica.fshz` (esperando o registro do vigia, F12, para saber qual mensagem segurou a tela).

- **MCP `sheet_impressao`** (24/11/2026): listar/definir área de impressão (`""` limpa; coluna além da XFD é
  recusada), títulos que repetem, orientação, papel (A4, Carta, Ofício…) e ajustar a N páginas, no `pg` da aba (o
  mesmo que o `.xlsx` grava). Teste `configurar_pagina_pelo_mcp`.

- **MCP ao vivo: `zoom` e `grade`** (24/11/2026) no `sheet_comando` (o `congelar` já existia; o `MCP-PENDENTE`
  estava desatualizado). Zoom respeita os limites das Configurações. Teste `zoom_e_grade_pelo_mcp`.

- **VBA `Word.Application` → `docs.fill`** (24/11/2026): `CreateObject("Word.Application")`, `Documents.Open`/
  `Add(Template:=)`, `Content`/`Range.Find.Execute FindText:=, ReplaceWith:=` e o bloco `With .Find` (`.Text`,
  `.Replacement.Text`, `.Execute`), `SaveAs`/`SaveAs2`; `Visible`, `Close`, `Quit`, `ClearFormatting` e as opções da
  busca não fazem nada. Ajuda `vba.wordDoc` junta as trocas; o `SaveAs` chama `docs.fill`. O texto da troca passa
  pelo CStr do VBA (`vba.str`: vírgula em português). O resto do Word continua pendência. Teste
  `word_localizar_substituir_vira_docs_fill`; o exemplo de "não converte" do `converter.rs` passou a ser o Outlook.

- **Macro `docs.fill(modelo, trocas, destino)`** (24/11/2026, primeiro passo da integração com o FeatherDocs): um
  `.docx` novo a partir de um modelo do Word, trocando marcadores (`#NOME#`) como o Localizar e Substituir. A troca é
  no XML (`feathersheet_io::docx_modelo`): corpo, cabeçalhos, rodapés e notas; marcador que o Word quebrou em vários
  `<w:t>` é achado (dentro do parágrafo); o resto do arquivo sai byte a byte; teto de 64 MB. Modelo e destino na pasta
  da planilha (mesma gaiola do `exportPdf`), conta no aviso "Ativar macros" (`fill` em `GRAVA`). Moeda/data: a Ajuda
  manda usar `sheet.getText()` (o valor passado direto vira `String()`, com ponto). Ajuda F1 nas quatro línguas.
  Testes `docx_modelo::*`, `preencher_docx_fica_anotado_e_recusa_fora_da_pasta`, `macro_preenche_modelo_docx_na_pasta`.
  Falta: o conversor do `Word.Application` do VBA para `docs.fill` e o caso da "Planilha de Informações".
  Novidade para o `VERSOES` da 0.4.0: "Macros geram documentos do Word a partir de um modelo (docs.fill)".

- **MCP `sheet_link`** (24/11/2026): listar, definir (com texto, num Ctrl+Z) e remover hiperlinks de célula. A regra
  do que vira link (`normalizar_link`, `link_do_estilo`: só web, `mailto:` e `#Aba!A1`) saiu do app para
  `feathersheet_io::links`, para a IA não gravar `javascript:` nem `file:`. Testes `links_pelo_mcp`,
  `so_web_mailto_e_lugar_da_pasta`.

- **MCP `sheet_proteger`** (24/11/2026): listar, proteger a aba (senha opcional, permissões pelo nome do Excel:
  `sort`, `autoFilter`…; permissão desconhecida é recusada com a lista) e desproteger conferindo a senha. O hash da
  senha (SHA-512 com sal e giros do Excel) saiu do app para `feathersheet_io::senha_planilha` (o app reexporta; tirou
  `sha2`/`getrandom` do `Cargo.toml` dele). Teste `proteger_e_desproteger_pelo_mcp`. Nome de aba excluída: some da
  lista de nomes e do arquivo, volta com a aba no desfazer (`nome_de_aba_excluida_some_e_volta_com_a_aba`).

- **Excluir linhas com filtro em fundo** (24/11/2026; o "fica para depois" da 0.3.5, que parava a janela 2,5 s com
  5000 trechos): com 200 trechos ou mais (`TRECHOS_EM_FUNDO`), a exclusão vai para outra thread com a pasta, como o
  cálculo em fundo (espelho na tela, diálogo "Excluindo as linhas visíveis" com o andamento, sem Parar); a foto do
  Ctrl+Z é tirada antes, na janela. O laço dos trechos virou `celulas_dialogo::aplicar_trechos` (o mesmo na janela e
  na thread), e o reflow dos minigráficos, `refluir_minigraficos_na_pasta`. Teste
  `excluir_muitos_trechos_filtrados_vai_em_fundo_e_desfaz`. Continua um `excluir_linhas` por trecho (lento numa pasta
  enorme, mas sem travar); cortar várias faixas numa operação do motor fica anotado como `shortcut:`.

- **FeatherQuery: consulta que mescla/acrescenta outra consulta** (24/11/2026): a fonte adicional "Consulta: X" (o
  `mesclarConsultas` do Power Query importado) funcionava no editor, mas reabrir ou atualizar a consulta gravada dava
  "A fonte adicional 'Consulta: X' não existe mais": `fontes_extra_da_salva` só conhecia "Tabela: X" e abas. Agora
  calcula X na hora (`resultado_da_salva`, mesmo limite de profundidade). Teste
  `consulta_que_le_outra_consulta_reabre_e_atualiza`.
- **MCP `sheet_nomes`** (24/11/2026): lista os nomes de valor/fórmula e os de uma aba; `definir` com `formula` (pt-BR,
  conferida pelo parser) e `escopo`; `remover` com `escopo` tira o da aba. Teste `nomes_de_formula_e_de_aba_pelo_mcp`.

- **Grupo de formas no `.xlsx`** (24/11/2026; sobra da #79, "as formas saem soltas"): formas com o mesmo `grupo`
  vão num `xdr:grpSp` numa âncora só, a caixa do grupo cobrindo todas e cada forma com o `xfrm` dela em EMU (o espaço
  dos filhos igual ao da caixa); a leitura já desfazia o `grpSp`. `xlsx_write_graficos::grupo_xml`, teste
  `grupo_de_formas_vai_e_volta_pelo_xlsx`. Fica de fora (#79): girar e esticar o grupo como um todo no app.

- **Nomes de valor/fórmula e nomes de uma aba só** (24/11/2026; era limite anotado na 0.3.5, "o `Nomeado` só tem
  `area: Rect`"). Motor: `Workbook::nomes_formula` (`NomeFormula`: nome, fórmula na forma das células, aba de escopo,
  árvore compilada); `Hospedeiro::nome_na_aba`/`formula_do_nome` e `EvalContext::resolve_name_formula`, o nome da aba
  vence o da pasta; nome que é só referência (`=Plan2!$A$1:$A$3`) serve onde se pede referência (ÍNDICE, SUBTOTAL);
  nome que depende dele mesmo dá `#NOME?` (limite de 64, `NOMES_ABERTOS`). Recálculo: quem usa nome já era sujo a cada
  mudança (`sujar_externas_de`), então vale para os novos. Renomear aba e inserir/excluir linhas/colunas reescrevem o
  texto (`reescrever_aba`, `adjust_for_structure`); intervalo e fórmula da pasta com o mesmo nome não convivem.
  `.xlsx`: `read_formula_names` (o `read_names` deixou de ler os de `localSheetId`, que sobrescreviam o da pasta) e a
  gravação com `localSheetId`; saíram do `nao_convertidos` (o Ctrl+S não pede mais "Salvar como" por eles). `.fshz`:
  itens `{name, formula, scope}` na lista `names` (versão antiga ignora). `.xlsb`: os de fórmula entram (o calamine não
  diz o escopo). Gerenciador de nomes: lista os dois tipos com a coluna **Escopo** (como no Excel), "Refere-se a"
  aceita conta (`=0,1`, na língua da interface: `=0.1` em inglês), Aplicar mantém o escopo. A coluna Valor
  trocava ponto por vírgula sempre (em inglês saía `0,12`): agora segue a língua. A caixa de nome digitando um nome
  de fórmula ("Taxa") trocava-o por um intervalo da seleção: agora vai para o nome. Testes: `tests/nomes_formula.rs`
  (6), `nomes_de_formula_e_de_aba_vao_e_voltam` (io), `nome_guarda_a_aba_do_intervalo`, três em `nomes_ui`. Fica de
  fora: criar um nome de aba pelo Gerenciador (só os que vêm do Excel; editar mantém), referência relativa num nome
  (vale como absoluta), referência sem aba num nome da pasta não anda ao inserir linhas. **Achado para decidir:** o
  texto interno guarda o decimal como digitado (`=SUM(1,2,3.5)` → `=SUM(1;2;3.5)`, teste em `locale.rs`), e o
  português mostra `3.5` com ponto; vale para células e nomes. Novidade para o `VERSOES`: "Nomes com valor ou fórmula
  (Taxa = 0,1) e nomes de uma aba só, como no Excel: calculam, aparecem no Gerenciador e vão e voltam no .xlsx."
- **Largura de antes de ocultar no `.fshz`** (24/11/2026; limite anotado na 0.3.5): campo `dh` da aba
  (`(coluna?, índice, px)`), lido e gravado; reexibir depois de reabrir devolve a medida. Teste
  `medida_de_antes_de_ocultar_sobrevive_ao_fshz`. O `enxugar.rs` (leitor dos `.fshz` antigos) não conhecia `oc`, `wp`,
  `tp`, `pt`, `cm`, `lo` nem `co` e recusava esses arquivos (caíam no leitor lento, sem perda): entraram, com `dh`.

- **Eventos da pasta: Workbook_Open e Workbook_BeforeSave** (24/11/2026, item da v0.4 das metas). `aoAbrir` roda quando
  a pessoa clica em "Ativar macros" no arquivo recém-aberto (uma vez; a cópia de recuperação não conta,
  `App::ao_abrir_pendente`), seguido dos módulos "Executar ao abrir" (`auto`), que **antes não rodavam nunca** apesar
  de a tela e a Ajuda dizerem que sim. `antesDeSalvar` roda no começo do Salvar e do Salvar como
  (`valores.comoSalvar`); `wb.cancelSave()` (novo na API, `Resultado::cancelar_salvar`) não deixa gravar. Pergunta
  Sim/Não no evento: o Salvar espera e volta sozinho no "Sim" (`salvar_apos_pergunta`/`salvar_sem_evento`). O conversor
  VBA leva `Workbook_BeforeSave(SaveAsUI, Cancel)` para esse evento, com o `Cancel` numa caixa ByRef. Os eventos
  ficam em módulos `Eventos/<evento>` (pasta) e `Eventos/<aba>/<evento>`; o VBA importado agora usa esses nomes, então
  **o evento continua valendo depois de salvar e reabrir** (antes era `Evento_…`, ligado só na sessão da importação).
  No editor de macros, "Esta pasta de trabalho" deixou de ser um "Em breve" com seletor morto: é o editor dos dois
  eventos, como o das abas. Captura `FEATHERSHEET_CAPTURAR_JANELA=eventos_pasta`. Testes:
  `cancel_save_chega_ao_app` (macros), `before_save_com_cancel_cancela_a_gravacao` (vba),
  `ao_abrir_roda_ao_ativar_macros_uma_vez_e_depois_os_automaticos`, `antes_de_salvar_pode_cancelar_a_gravacao`,
  `antes_de_salvar_com_pergunta_salva_depois_da_resposta` (app). Fica de fora: Salvar cópia, o salvar do MCP e a
  cópia de recuperação não disparam o evento (o Excel também não dispara no SaveCopyAs). **BeforeClose** (mesmo dia):
  `antesDeFechar` roda em `App::proteger` (sair, Novo, abrir outro, modelo, recuperar) antes do "salvar alterações?";
  `wb.cancelClose()` mantém a pasta; com pergunta, fecha sozinho no "Sim"; a segunda passagem depois de gravar não
  roda de novo (`fechar_sem_evento`). O campo do motor virou `Resultado::cancelar` (vale para os dois). Testes
  `antes_de_fechar_pode_impedir_e_espera_a_resposta`, `before_close_com_cancel_impede_fechar`.
  Novidade para o `VERSOES` da 0.4.0: "Macros ao abrir, antes de salvar e antes de fechar — o Workbook_Open, o
  Workbook_BeforeSave e o Workbook_BeforeClose do Excel rodam aqui; Cancel = True não deixa gravar nem fechar."

- **Revisão do que entrou na 0.3.5** (23/11/2026, agente revisor): colar uma cópia filtrada deslocava as fórmulas
  e o "Colar vínculo" pela posição na cópia, não pela linha de onde vieram (`AreaCopiada.linhas`); valor derramado
  com a âncora numa linha escondida ia em branco; matriz bloqueada (#DESPEJO!) salva em `.xlsx` voltava embrulhada em
  SINGLE (agora vai com `cm`, e o `metadata.xml` sai também só com bloqueadas: `Sheet::tem_derrame_bloqueado`);
  `INT` voltou ao piso, só "encosta" no inteiro quando está a ~4 ulp dele (o arredondamento a 15 algarismos errava
  123456789012345,6); data com ponto sai só na digitação/colagem (`sem_milhar_digitado`), não no `parse_literal`, para
  um "05.03.2026" antigo do `.fshz` seguir texto; MCP ao vivo: o token tem teto de 256 bytes e a mensagem grande
  demais recebe erro -32600; excluir linhas com filtro: falha no meio deixa o Ctrl+Z, status conta as visíveis, tudo
  escondido não faz nada; o filtro barato da interseção legada exige ":" fora de parênteses. `.xlsb` refeito:
  leitura por streaming (`worksheet_cells_reader`, sem a grade densa que estourava a memória), aba de gráfico vem
  vazia, fórmula ilegível é contada (status) e não leva as outras, nome de aba ganha aspas, bits de "relativo"
  que o calamine deixa na coluna (`$BTRM$1` = A1) desfeitos, duração `[h]:mm` e data negativa ficam número,
  `sem_user` não corta `GetUser(`, pânico do calamine vira erro. Fica para depois: excluir milhares de trechos
  filtrados numa operação só (medido: 5000 trechos em 10 mil linhas com fórmulas = 2,5 s com a janela parada;
  pede `mover_estrutura` com vários cortes, que mexe em fórmulas, nomes, tabelas e mesclas de uma vez).
- **Revisão do que saiu sem revisão na 0.3.5** (23/11/2026): Recalcular tudo era N² (uma cascata por fórmula;
  `sujar_celulas` agora faz uma só) e rodava na janela (agora `sujar_tudo` + `calcular_em_fundo`, recusado com cálculo em
  fundo rodando); inserir que empurraria célula para fora (XFD) é recusado no `mover_estrutura` (a chave caía na linha de
  baixo); "Inserir linhas…" abaixo de um bloco entra depois dele todo, não no meio; grade limitada: coluna/linha inteira
  selecionada não a faz infinita (`ponta_da_selecao_para_a_grade`), o "+" pula linha/coluna oculta, clique direito depois
  do fim não abre menu; foto girada pelo EXIF guarda o perfil RGB, cai de qualidade antes de passar de 16 MiB e, a
  flutuante, gira fora da janela; fuso do Windows arredondado a 15 min (a virada do minuto entre as duas leituras dava
  UTC−03:01).
- **Revisão das correções de 23/11** (agente revisor): `.xlsb` cortado (sem `BrtEndSheetData`) girava para sempre no
  `next_formula` (erro de E/S agora encerra a aba); a aspa no nome da aba trocava `Vendas2024!` por `Vendas'2024'!`
  (`com_aspas_no_nome` só troca no começo da referência e fora de aspas); o filtro barato do ":" deixava `(A1:A3)*10`
  derramar (só parêntese de chamada conta); data com ponto voltou ao DATAVALOR, com hora (`05.03.2026 14:30`) e no CSV
  sem língua detectada (`input::data_com_ponto` público).
- **Interseção legada dentro de SE/ANO/MÊS/DIA/DIA.DA.SEMANA** (23/11/2026): `=SE(A1:A10>5;…)` de `.xlsx` antigo vale a
  linha da fórmula, como no Excel de antes do 365 (`functions::intersecao_nos_argumentos`; MAIOR/MENOR ficam de fora,
  querem o intervalo inteiro). O filtro barato da leitura deixa passar `IF(`/`YEAR(`… com ":".

- **Abrir `.xlsb`** (23/11/2026): `feathersheet-io/src/xlsb_read.rs` pelo crate `calamine` 0.38 (Rust puro, sem os
  recursos padrão). Vêm valores, datas (base 1904 convertida; entram como digitadas, com formato de data),
  fórmulas (convertidas pelo `formula_from_excel` do `.xlsx`; o `User(nome,…)` que o calamine escreve para função
  que não conhece volta a `nome(…)`) e nomes definidos (`Aba!$A$1:$B$2`, também de uma célula). Não vêm formatação,
  mesclas, larguras, gráficos nem imagens (o calamine não lê isso no `.xlsb`); o status diz isso ao abrir. Gravar
  `.xlsb` não existe: como o `.csv`, abre sem `caminho` e o Ctrl+S pede onde salvar. Filtro na janela de abrir,
  "Abrir com" no `.desktop` e no instalador do Windows, Ajuda F1. Teste `tests/xlsb_xlsx.rs` com três arquivos dos
  testes do calamine (MIT, `tests/arquivos/xlsb/LEIA-ME.md`); exemplo `examples/ver_xlsb.rs`.

## v0.3.5 (23/11/2026)

- O resto do relatório das issues do GridCraft (23/11/2026, entrou na 0.3.5 a pedido):
  **Limites do Excel na fórmula digitada** (`parser::fora_dos_limites_do_excel`, cobrado no `confirmar`, que
  mantém a edição e diz o motivo no status): mais de 64 funções uma dentro da outra, mais de 255 argumentos,
  texto entre aspas acima de 255 caracteres e número fora de 2,2E-308…9,99E+307. Arquivo aberto não passa por
  ela (o LibreOffice grava texto longo). Teste `recusa_o_que_o_excel_nao_abre`.
  **Excluir linhas inteiras com filtro** (Ctrl+- com linhas selecionadas) apaga só as visíveis, faixa por faixa
  de baixo para cima, num Ctrl+Z, como o Excel numa lista filtrada (`celulas_dialogo::aplicar_celulas`). Teste
  `excluir_linhas_com_filtro_apaga_so_as_visiveis`.
  **CSV**: a amostra que decide separador, codificação, decimal e ordem dia/mês passou de 64 KiB para 1 MiB
  (`csv_colunas::AMOSTRA_CSV`; umas 15 mil linhas em vez de 200). Teste
  `datas_mes_dia_decididas_mais_embaixo_que_os_64_kib_antigos`. Ler o arquivo duas vezes seria a correção completa.


- Comparação com o GridCraft (notas da 0.12 à 0.19 e títulos das 1.350 issues abertas), 23/11/2026, quatro correções:
  **Copiar com filtro** leva só as linhas que aparecem, como no Excel (`copiar_area(so_visiveis)`; o Recortar
  continua levando tudo, porque limpa a origem inteira; com linhas puladas a validação não vai junto, `shortcut:`);
  **Ctrl+Enter** não escreve nas linhas que o filtro escondeu (o mesmo `puladas_pelo_filtro` do Delete);
  **fórmula antiga do `.xlsx`** (sem `cm` nem `t="array"`, do Excel antes do 365 e do LibreOffice) com intervalo
  solto ou numa conta (`=A:A*2`) é embrulhada em `_xlfn.SINGLE` ao abrir e faz a interseção implícita (linha 5
  → `A5*2`) em vez de derramar; a função `SINGLE` (o `@` do Excel 365, que antes abria como `#NOME?`) existe no
  motor e volta como `_xlfn.SINGLE` no `.xlsx` (`fn_info::fn_single`, `xlsx_read/sheet.rs::intersecao_legada`;
  teste `tests/intersecao_legada_xlsx.rs`); **MCP** recusa mensagem acima de 16 MB sem guardá-la (`ler_linha`, stdio
  e conexão ao vivo). E o nome de aba conta o limite de 31 em UTF-16, como o Excel (emoji vale 2; passava e o
  Excel não abria o arquivo). Do mesmo relatório, feitos no dia: INT passa o número por 15 algarismos antes do piso (`=INT(0,1*3*10)` dava
  2; teste `int_logo_abaixo_de_inteiro_e_o_inteiro_como_no_excel`); data digitada com ponto e ano de 4 dígitos
  (`05.03.2026`, `input::parse_date_text`; `1.234.567` segue número); nome definido com cara de R1C1 (`R`, `RC`,
  `R1C1`, `R1x`, `C1_`) recusado em `nomes_ui::parece_r1c1`. Ficaram para depois: aninhamento acima de 64 no
  salvar, mais de 255 argumentos/caracteres em literal, excluir linhas de seleção filtrada (conferir o Excel), CSV
  que decide a ordem da data só pelo começo, literal 1E+308.
- AGORA()/HOJE() no Windows usam o fuso do sistema (hora local menos UTC por `GetLocalTime`/`GetSystemTime`, `app/fuso.rs`); antes era UTC e no Brasil o dia virava às 21 h. Conferido com `cargo check --target x86_64-pc-windows-gnu`, sem rodar num Windows.
- Auditoria do código de 14 a 23/11 (23/11/2026), oito correções:
  MCP não grava por cima do `.xlsx` aberto com partes não convertidas também quando o caminho vem escrito de
  outro jeito (`./a.xlsx`, atalho: `conferir_sobrescrita` compara o caminho real); ícones só do Excel 2010+
  (3 estrelas, 3 triângulos, 5 caixas) contam como não convertidos (sumiam no Ctrl+S) e o `trechos` pega o bloco
  `x14:conditionalFormattings` inteiro (o nome mais curto ganhava o empate); letra de 15 px volta 15 do `.xlsx`
  (saía 11 pt, que a leitura toma por padrão; o 11 saiu de `pt_da_fonte`); reexibir depois do Ctrl+Z ainda devolve
  a largura (`dimensoes_ocultadas` não é mais apagado ao reexibir); Grade limitada calcula o limite do outro
  painel pela aba dele (`limite_da_grade`); Colar vínculo recusa quando a aba copiada foi excluída (virava `=A1`
  desta aba); filtro avançado no lugar não mostra as linhas ocultas encostadas fora da lista
  (`Sheet::reexibir_somente`); o aviso de ícones ao salvar `.xlsx` não apaga mais o das macros VBA.
- Botão direito no número da linha ▸ "Inserir linhas…" (ou na letra da coluna ▸ "Inserir colunas…"): uma caixa pede
  quantas (1 a 10.000, vem com o tamanho da seleção) e onde (acima/abaixo, esquerda/direita), num Ctrl+Z
  (`app/inserir_varias.rs`, `inserir_menu_n`). Ideia do fadi123go (#164, discussão #790 do IronCalc). Teste
  `insere_quantas_linhas_pedir_abaixo_da_celula`; respeita a proteção da aba como o "Inserir linha".
- Exibir ▸ Grade limitada (#164, pedido do fadi123go, refeita no mesmo dia para bater com o desenho dele): a grade
  acaba na última linha/coluna, com fundo liso e sem letras nem números depois; um "+" no fim das letras e outro
  embaixo dos números acrescentam coluna/linha (`Msg::GradeCrescer`, `App::grade_extra`, só da tela); começa em
  A–K × 20 (o tamanho que ele propôs na discussão #790 do IronCalc) e cresce com os dados e com a seleção (Enter ou
  seta além da borda). Clique no vazio não seleciona nada (`Grade::clique_fora_da_grade`). Escolha também nos
  Primeiros passos (Aparência ▸ Grade: Infinita/Limitada) e em Configurações. Preferência da pessoa
  (`grade_limitada`), não do arquivo. Teste `grade_limitada_tem_minimo_e_cresce_com_dados_selecao_e_mais`; fotos
  conferidas (`FEATHERSHEET_CAPTURAR_GRADE_LIMITADA=1`). Limite: a rolagem continua livre além do fim (só fundo). Ajuste do mesmo dia (pedido do usuário): as barras de letras e de números seguem escuras até a borda, o "+" na cor de destaque do cabeçalho, e depois do fim não há borda para arrastar nem cursor de célula (`Grade::existe`; teste `grade_limitada_nao_tem_borda_depois_do_fim`).
- Reexibir coluna/linha devolve a largura/altura de antes de ocultar (voltava ao padrão; só o Ctrl+Z recuperava).
  `Sheet::dimensoes_ocultadas` (só na memória: inserir/excluir linhas ou colunas esquece; não vai para o `.fshz`,
  `// shortcut:`). No `.xlsx`, a coluna oculta sai com `hidden="1"` e a largura própria (saía `width="0"`), e a lida
  guarda a largura (`SheetJson::larguras_ocultas`). Testes `reexibir_devolve_a_largura_e_a_altura_de_antes` e
  `tests/coluna_oculta_xlsx.rs`.
- MCP (avulso e ao vivo) não grava `.xlsx` por cima do arquivo aberto quando ele tem `nao_convertidos`
  (`arquivo::conferir_sobrescrita`; a mensagem lista o que se perderia e pede outro `caminho`), como o Ctrl+S do app.
  O ao vivo passa as vistas da tela (`vistas_das_abas`) ao exportar `.xlsx`. Teste
  `nao_grava_por_cima_do_xlsx_com_parte_nao_convertida`. Conserta também a compilação do app, que a mudança de
  `arquivo::salvar` (vistas) da entrega anterior tinha quebrado no `mcp_vivo.rs`.
- `.xlsx` com nome definido que o motor não guarda (valor ou fórmula, `=0,1`, `=Vendas!A1*2`; ou o mesmo nome em duas
  abas pelo `localSheetId`): a leitura deixava de fora e o Ctrl+S gravava por cima sem avisar. Agora entram nos
  `nao_convertidos` ("Nome definido com valor ou fórmula", "Nome definido repetido em outra aba"); `#REF!` não conta.
  Suportar esses nomes de verdade fica para depois (o `Nomeado` só tem `area: Rect`).
- CSV com datas mês/dia (o americano): `02/13/2026` ficava texto e `01/02/2026` virava 1º de fevereiro. A amostra
  marca a coluna `mes_primeiro` quando algum segundo número passa de 12 e nenhum primeiro passa
  (`csv_colunas::ColunaCsv`); a data sai trocada para dia/mês nos dois caminhos. Teste
  `coluna_de_datas_mes_dia_vira_data_certa`; Ajuda F1 nas quatro línguas.
- MCP avulso: salvar `.xlsx` perdia o painel congelado, as linhas de grade e os zeros ocultos (o app guarda o
  `<sheetView>` ao abrir e devolve com `xlsx_write::com_vistas`; o MCP gravava sem). `Avulso::vistas` guarda pelo id
  da aba e `arquivo::salvar` passa por `com_vistas`. Teste `salvar_xlsx_pelo_mcp_mantem_painel_congelado_e_linhas_de_grade`.
- Formato personalizado (`0000` do CEP, `"Total: "0`) era ignorado na exportação CSV, na impressão em PDF e no
  texto do `.xlsx_write::texto_formatado`: os três usavam `format_number` (só o `format`), a grade usa
  `format_value` (o `code` manda). Agora os três passam por `format_value`. Teste
  `formato_personalizado_sai_no_csv_como_na_grade`. Os códigos em si já iam e voltavam iguais no `.xlsx`.
  Também o ajuste automático da largura (`operacoes/autoajuste.rs`, a coluna de `"Total: "0` ficava estreita) e o
  quadro (`planilha/frame.rs`); teste `formato_personalizado_conta_na_largura`.
- **Recalcular tudo (Ctrl+Alt+F9)**, em Fórmulas → Opções de cálculo: `Workbook::calcular_tudo` marca todas as
  fórmulas (`for_each_formula` + `sujar_celulas`) e calcula. O F9 só refaz o que está marcado: o valor guardado
  errado de um arquivo de outro programa (um `#NAME?` do LibreOffice) ficava até mexer na célula. Teste
  `tests/calcular_tudo.rs`; Ajuda F1 (atalhos F9/Shift+F9/Ctrl+Alt+F9 e o parágrafo de Nomes/Cálculo) nas quatro línguas.
- Salvar por um atalho (symlink) trocava o atalho por um arquivo comum e o original ficava velho:
  `arquivo::gravar_atomico_com` resolve o caminho real (`canonicalize`) antes. Teste
  `gravar_por_um_atalho_grava_no_original_e_o_atalho_continua`.
- Foto de celular entrava deitada (a orientação fica no EXIF e quem desenha só olha os pixels): ao inserir
  imagem solta (`imagens_importadas::importar`) ou na célula, o JPEG com orientação é regravado em pé
  (`girada_pelo_exif`, qualidade 92, sem EXIF). Só JPEG. Teste `foto_com_orientacao_no_exif_entra_em_pe`.
- CSV: código só de dígitos que começa com 0 (`007`, CEP, telefone) entra como texto e guarda o zero
  (`csv_colunas::codigo_com_zero`, nos três caminhos de importação; o `classificar` continua igual ao
  `parse_literal`, a regra fica no `campo`). `0`, `0.5` e `0,5` seguem número. Teste
  `codigo_com_zero_a_esquerda_fica_texto`; Ajuda F1 nas quatro línguas.
- MCP (`feathersheet-mcp/src/arquivo.rs`): salvar `.xlsx`/`.csv`/`.tsv` pelo `sheet_save` passou a ser atômico
  (`feathersheet_io::arquivo::gravar_atomico`), como o `.fshz` já era.
- Formas desenhadas no app iam embora ao salvar `.xlsx`, sem aviso (o aviso do Ctrl+S só olha o arquivo
  original). Agora `xlsx_write_graficos::forma_xml` grava cada forma como `xdr:sp`: `prstGeom` pelo
  `feather_shapes::TipoForma::prst` (dependência nova do `-io`), preenchimento, contorno, giro, espelho, texto
  com alinhamento e letra, macro e nome. Não vão: degradê, sombra, ajustes (o retângulo com cantos vira
  `roundRect`) e grupo (as formas saem soltas). Conferido no LibreOffice. Teste `forma_desenhada_vai_e_volta_pelo_xlsx`.
- Ícone, WordArt e SmartArt desenhados no app ainda não vão para o `.xlsx`: depois de salvar, o status avisa
  quantos ficaram de fora e sugere salvar também em `.fshz` (`App::objetos_sem_xlsx`).
- Ida e volta de um `.xlsx` do LibreOffice (20/11): a borda `hair` (0,5 pt contínua) voltava tracejada (agora
  fica Fina) e a letra encolhia (10 pt → 13 px → 9,75 pt): `xlsx_write::pt_da_fonte` leva ao tamanho padrão do
  Office a até 0,375 pt. O motor segue guardando px inteiros.

## v0.3.4 — 20/11/2026: Dividir janela, Inspecionar documento, Filtro avançado, cálculo mais rápido

Junta a seção "Depois da v0.3.3" abaixo: Dividir janela/Lado a lado (#83), Agrupar formas (#79), Inspecionar
documento, Filtro avançado, Exibir ▸ Zeros, Colar vínculo/Comentários/Validação, texto alternativo, alça de célula
vazia, copiar a conta do rodapé, avisos do Ctrl+S no `.xlsx`, ordenar 1 milhão de linhas (27 → 0,3 s), abas ligadas
3× mais rápidas, `.xlsx` ~40% menor. Novidades nas quatro línguas (traduções sem revisão humana). Mesmo pelicano
0.1.34 e feather-ui v0.3.16. `.exe` sem VirusTotal, como nas anteriores.

## Depois da v0.3.3 (entrou na 0.3.4)

**Dividir janela (#83, 16/11).** Exibir ▸ Janela ▸ Dividir janela / Lado a lado (`Msg::Dividir(lado)`,
`app/painel_dividido.rs`): dois painéis, um em cima do outro ou lado a lado, cada um com a sua aba e rolagem
(o outro botão muda só o sentido; o mesmo de novo desfaz). Só um é o ativo (o estado de sempre: `folha`,
`linha_inicio`, seleção); o outro fica em `App::painel_dividido` (id da aba, rolagem, célula e uma `EstadoEixos`
própria, trocada com `eixos_grade` por `RefCell::swap`). Ele é desenhado com as mesmas camadas da grade ativa
(`imagens_sobre`, `objetos_sobre`, `botoes_macro_sobre`, `tinta_sobre`), que recebem um `PainelVista` (aba,
rolagem, `ativo`); no inativo não há alças, seleção, inserção nem edição no lugar, e a tinta só desenha.
Segmentações de dados ficam só no ativo. Os objetos da aba do outro painel são relidos a cada quadro (o cache
de `objetos_de` guarda só a aba ativa). A grade do outro painel
é mapeada para `Msg::NoOutroPainel`: qualquer clique ou roda nele troca os papéis (por `Msg::Aba`, que confirma a
edição; recusa = nada muda) e repassa a mensagem; com fórmula aberta o clique aponta a referência na outra aba.
Os painéis não mudam de lugar (`ativo_embaixo`: o ativo é o segundo, embaixo ou à direita). **Não feito:**
divisão em quatro; Nova janela, Organizar tudo e Alternar janelas.
Teste: `painel_dividido::testes`. Foto: `FEATHERSHEET_CAPTURAR_DIVIDIR=1` (em cima/embaixo) e `=lado` (lado a
lado, com as formas de exemplo no painel inativo). "Lado a lado" é botão grande (ícone + rótulo) como os vizinhos
do grupo Janela (19/11; antes saía como botão pequeno solto, conferido por foto `FEATHERSHEET_CAPTURAR_FAIXA=exibir`).
Novidade: "Dividir janela (Exibir ▸ Janela): duas abas à vista, uma em cima da outra ou lado a lado, cada uma com
a sua rolagem, imagens e gráficos."

**Reduzir para caber e texto vertical no `.xlsx` (19/11).** O app já tinha os dois em Formatar células ▸ Alinhamento
(`shrinkToFit` e `textVertical` no `EstiloParcial`, desenhados pela grade), mas o `.xlsx` não lia nem gravava: o arquivo
do Excel perdia o efeito, e o vertical do Excel (`textRotation="255"`) virava giro de -165°. Agora `xlsx_read/styles.rs`
lê `shrinkToFit` e o 255 como `textVertical`, e o `xlsx_write` grava os dois. Testes: `reduzir_para_caber_e_texto_vertical`
(leitor) e `reduzir_para_caber_e_texto_vertical_voltam_do_xlsx` (`da_workbook`). Mesmo caminho para sobrescrito e
subscrito (`superscript`/`subscript` ↔ `<vertAlign>` da fonte; teste `sobrescrito_e_subscrito_da_fonte`). Novidade:
"Reduzir para caber, texto vertical, sobrescrito e subscrito do Excel agora abrem e salvam no .xlsx."

**Centralizar na seleção e outros alinhamentos do Excel guardados (19/11).** `horizontal` = `centerContinuous`,
`distributed`, `fill` ou `justify` era descartado ao ler o `.xlsx` (a célula ficava no Geral e o Ctrl+S apagava). Agora a
leitura põe o mais próximo em `align` (centro para os dois primeiros, esquerda para os outros) e guarda o original em
`alignExcel`; o `xlsx_write` devolve o original só se o `align` ainda é o que ele virou (trocado no app, vale o novo).
**Na tela** Centralizar na seleção centraliza só dentro da célula (`shortcut:` em `styles.rs`). Testes:
`centralizar_na_selecao_fica_guardado` e `centralizar_na_selecao_volta_ao_xlsx_so_se_nao_mudou`. Novidade: "Centralizar
na seleção, Preencher, Justificar e Distribuído do Excel não se perdem mais ao abrir e salvar o .xlsx."

**Cor da guia no `.xlsx` (19/11).** `Sheet::tab_color` ia e voltava do `.fshz`, mas o `.xlsx` não lia nem gravava o
`sheetPr/tabColor`: a aba colorida no Excel chegava sem cor e a colorida aqui saía sem cor. Leitura em `xlsx_read/sheet.rs`
(`cor_aba`, pelo `styles::cor_de`, que entende tema e tint) e gravação no `<sheetPr>` antes do `pageSetUpPr`. Teste
`cor_da_guia_vai_e_volta_do_xlsx`. Novidade: "A cor das guias das abas abre e salva no .xlsx do Excel."

**Degradê, borda diagonal e hachura avisados (19/11).** O estilo do app guarda só cor sólida e uma linha por lado:
`gradientFill`, `diagonalUp/Down` e `patternType` de hachura no `xl/styles.xml` sumiam no Ctrl+S calados. Agora entram no
`nao_convertidos` (do arquivo inteiro, contados pelo texto do `styles.xml`, `abre` aceita prefixo), então o F12 lista e o
Ctrl+S num `.xlsx` com eles pede "Salvar como" dizendo o que se perderia. Texto com parte formatada (`<rPr>` dentro de um `<si>` do
`sharedStrings.xml`: negrito numa palavra, cor num trecho) entrou no mesmo aviso, porque a célula guarda um formato só e
o trecho formatado sairia liso. Validação de tipo sem equivalente no motor (tamanho do texto, personalizada qualquer), que a leitura
deixava de fora só com nota no diário, também entra ("Validação de dados sem equivalente", por aba): `trechos` lê o XML da aba
aos pedaços e guarda só os blocos `<dataValidations>`/`<x14:dataValidations>`, passados ao `validacoes::ler` de sempre. O mesmo trecho traz a formatação condicional (`conditionalFormatting` e o
`x14:conditionalFormattings`): as regras que o `condicional::ler` deixa de fora (conjuntos de ícones, período de datas,
desvio padrão, 10% primeiros, barra de dados do x14) contam como "Formatação condicional sem equivalente". E o `<autoFilter>` da aba fora de tabela (o app só filtra tabela; os botões
sumiam e o Ctrl+S apagava) conta como "Filtro da planilha (fora de tabela)". Comentário em conversa do Excel 365 (`xl/threadedComments/`) também: o texto
não se perde (as respostas entram na nota), mas a conversa vira "anotação simples" ao gravar. O `trechos` trata `<tag …/>` sem fecho. Pode dar aviso por estilo definido e não usado
por célula nenhuma (fica do lado seguro). Novidade: "Salvar por cima de um .xlsx com degradê, borda diagonal ou hachura
avisa antes, em vez de perder esses formatos."

**Exibir ▸ Zeros, por aba (19/11).** O `showZeros="0"` do `<sheetView>` era ignorado (as células com zero apareciam)
e o Ctrl+S apagava. Agora `VistaAba::zeros` (lido em `vistas.rs`, gravado no `vista_xml`), `App::zeros_ocultos` (ids, como o
`grade_oculta`; só estado da janela, não vai no `.fshz`), `Grade::mostrar_zeros` e `Grade::e_zero` (só número 0; o "0" texto e a
célula em digitação aparecem). Botão Exibir ▸ Mostrar ▸ Zeros (`Msg::MostrarZeros`, marca alterado); o Monitor foi para a
mesma pilha nova (saía cortado embaixo da lista do Tema). Novidade: "Exibir ▸ Zeros: deixe em branco as células com zero
numa aba, como no Excel; a opção vem e volta no .xlsx." Foto: `FEATHERSHEET_CAPTURAR_ZEROS=0` (conferida: 0 digitado e fórmula que dá 0 em
branco, texto "0" aparece). Teste `testes_zeros` e `zeros_ocultos_vao_e_voltam_do_xlsx`.

**Colar vínculo (19/11).** Colar especial ganhou "Colar vínculo" (`QueColar::Vinculo`) e o menu Colar ▾ também
(`Evento::ColarVinculo`, executa direto): cada célula do destino vira `=A1` (mesma aba) ou `='Aba'!A1` (outra), relativa,
apontando para a célula copiada; o estilo do destino fica. Pede cópia feita no FeatherSheet (`AreaCopiada::aba_origem`,
novo). "Ignorar em branco" e "Transpor" valem; a operação não. Teste `colar_vinculo_aponta_para_a_origem_e_cita_a_outra_aba`.
Na mesma janela, "Comentários" (`QueColar::Comentarios`): só os comentários da cópia (`AreaCopiada::comentarios`, pela posição
na área) vão para o destino, que fica com o conteúdo e o formato; destino sem comentário na origem não muda. Teste
`colar_so_comentarios_mantem_o_conteudo_do_destino`. Foto da janela conferida (`FEATHERSHEET_CAPTURAR_DIALOGO=colar_especial`).
E "Validação" (`QueColar::Validacao`): a cópia guarda as regras recortadas na área (`AreaCopiada::validacoes`, pela posição);
colar tira o destino das regras dele (`caixas::subtrair`, agora `pub(crate)`) e põe as copiadas, num Ctrl+Z
(`alterar_validacoes`). **Limites** (`shortcut:` no código): vai uma vez no canto do destino, sem repetir pela área maior nem
transpor; fórmula personalizada não desloca as referências. Teste `colar_validacao_troca_a_do_destino_e_mantem_o_conteudo`.
De passagem, Limpar formatos passou a tirar também sobrescrito, subscrito, sublinhado duplo e o `alignExcel`.
Novidade: "Colar especial com Colar vínculo, Comentários e Validação, como no Excel."

**Texto alternativo nas imagens (19/11).** Botão direito na imagem ▸ Texto alternativo… (`Evento::TextoAlt*`, diálogo
`dialogo_texto_alt`, estado `EstadoObjetos::texto_alt`). Fica em `ImagemObjeto::texto_alt` (aparado; em branco = `None`),
vira `descr` no `cNvPr` do `xdr:pic` e é lido de volta (`desenho_objetos.rs`). Um Ctrl+Z. Só imagens: formas e
gráficos não (gráfico tem o seu no painel; controle no Formatar controle). A imagem inserida no app (bloco `im`) tem o campo no painel Formatar imagem (`Evento::TextoAlt`, chave `alt`, grava a cada tecla).
Testes `texto_alternativo_da_imagem` e `imagens_da_aba_vao_para_o_desenho_e_voltam`. Novidade: "Texto alternativo nas
imagens, para leitores de tela."

**Gráfico de colunas separadas com Ctrl (19/11, pedido do pai).** Coluna A e coluna D marcadas (`selecoes_extras`)
com as mesmas linhas e sem se cruzar: o intervalo é o texto `A1:A3;D1:D3`, como o Excel mostra (`intervalo_da_selecao`;
digitar assim também vale). `areas_do_intervalo`/`retangulo_do_intervalo` (graficos.rs) leem o texto em todo lugar
que lia o intervalo; o motor recebe o bloco A:D e `ocultar_colunas_fora` põe B e C em `ocultas`, na numeração que o
desenho usa (sem resumir, coluna sem número não conta). O texto vai no JSON como `_featherAreas` e volta ao reabrir;
a moldura na grade sai das referências do texto, então são duas caixas (antes uma A1:D3, pegando B e C, foto do pai).
O `.xlsx` filtra `hiddenSeries`, então o Excel recebe só A e D. Áreas de linhas diferentes: só a ativa. Teste
`colunas_separadas_com_ctrl_viram_categoria_e_serie`. Novidade: "Gráfico com colunas separadas (A e D com Ctrl), como no Excel."

**Nome da série sem cabeçalho como no Excel (19/11, pedido do pai depois de abrir no LibreOffice).** Era "Série 3"
(espaço e a posição da coluna). Agora o motor deixa o nome vazio e, depois de tirar as ocultas, dá
`nome_padrao_da_serie(i)` = "Série1", "Série2"… pela ordem no gráfico (também nos caminhos 3D e resumido). No `.xlsx` a
série sem cabeçalho e com o nome padrão sai sem `c:tx`, como o Excel grava (o LibreOffice/Excel dão o próprio nome).
Teste `serie_sem_cabecalho_se_chama_serie1_e_nao_vai_no_xlsx`.

**Pasta com abas ligadas (19/11).** 24 meses × 2 mil linhas (cada mês faz PROCV no anterior),
Cadastro, Parametros com o nome `Taxa`, Resumo com 3D (`Mes_01:Mes_24!`) e SOMARPRODUTO: 288.126 fórmulas, conferidas
por um verificador externo no arquivo que nós salvamos. Achou três erros nossos:
1. **Nome definido virava função ao salvar:** `Taxa` saía como `RATE` (`formula_to_excel` traduzia todo identificador);
   agora só o que vem antes de `(` e VERDADEIRO/FALSO.
2. **Cadeia entre abas de dois saltos ficava velha ao editar:** `depois_de_mudar_em` sujava só quem lia a célula escrita;
   agora repete com a aba suja inteira até parar (cada aba uma vez). Testes `cadeia_entre_abas_de_dois_saltos_recalcula`
   e `mudar_a_celula_do_nome_recalcula_quem_usa_o_nome`.
3. **Achar a aba pelo nome a cada célula era metade do cálculo:** memória de uma entrada em `SheetCtx::alvo` e
   `Contexto::indice` (8,4 → 2,8 s).

**Desempenho (19/11).** `.xlsx` neutro gerado à mão (fórmulas sem valor guardado: Total = Qtd×Preço, SE,
SOMA/SOMASE/CONT.SE no topo), `taskset -c 0-7`, pelo exemplo `comparar_desempenho`. Abrir+calcular 1 milhão de linhas:
1,8 s e 0,23 GB; gravar: 3,8 s; 50 mil PROCV+CONT.SE: 1,1 s. Abrir CSV de 1 milhão de linhas: 1,7 s e 0,2 GB. Ordenar
1 milhão de linhas levava 27 s só para mover: `permutar_sem_registro` tirava e punha o estilo célula por célula, partindo
a faixa do `EstilosCelula` a cada vez (memmove, quadrático; a coluna de datas do CSV tem estilo em toda célula). Agora
remonta as faixas por coluna: 0,3 s (abrir+ordenar 3,5 s). 100 abas × 10 mil linhas com resumo: 0,45 s e 130 MB. Editar
B2 em 1 milhão de linhas e recalcular: 0,21 s. Fórmula com aba de nome que parece endereço sem aspas (`S1!C2`, `ABA1!C2`)
dava #NOME?: o `tokenizer` pulava o `!` quando o nome `parece_referencia`; agora endereço seguido de `!` é aba
(teste `aba_com_nome_de_endereco_sem_aspas`; ao gravar sai `'S1'!`). O `.xlsx` saía grande (zip `fast`): `zip_write`
passou ao nível 4 (1 milhão de linhas: 71 → 44 MB, gravar 2,0 → 2,9 s). Novidade: ".xlsx salvo cerca de 40% menor."

**Conferência externa das contas (19/11)** sobre a `amostra_ouro.xlsx` (refaz as contas pelo valor guardado e testa ida
e volta). Achou a normal imprecisa: `fn_stats.rs` tinha cópia própria de `erf`/`norm_inv` (Abramowitz-Stegun, erro na
7ª casa); agora usa as de `fn_dist.rs` (teste `normal_com_precisao_do_excel`). E o `pt_name.rs` mandava NORM.S.DIST para
DIST.NORMP (= NORMSDIST); teste `todo_nome_portugues_volta_a_mesma_funcao` confere a tabela inteira. Novidade:
"Distribuição normal com a precisão do Excel."

**Alça de célula vazia apaga, cursor da alça (19/11, relato do pai).** Arrastar a alça a partir de origem sem
conteúdo limpa o conteúdo do destino como o Delete (formatação fica; `limpar_pela_alca`, um Ctrl+Z, só a parte
usada); antes recusava com "A origem está vazia". Ctrl+D/R de origem vazia continua recusando. Cursor: `Cell`
(cruz grossa) sobre as células e `Crosshair` (cruz fina) sobre a alça ou arrastando por ela (`icone_do_mouse`).
Testes `alca_de_celula_vazia_apaga_o_destino_como_no_excel` e `arrastar_divisoria_do_cabecalho_muda_a_largura_com_zoom`;
conferido no app por `PELICANO_ROTEIRO` gravado (B2 vazia arrastada sobre a A2). Novidade: "A alça de preenchimento
funciona como no Excel: cruz fina ao passar o mouse e, arrastada de uma célula vazia, apaga as células por onde passa."

**Clicar na conta do rodapé copia o valor (19/11).** `barra_status::pecas_contas` devolve as contas que cabem com a chave;
`view_resumo_selecao` faz de cada uma um botão sem moldura (dica "Clique para copiar") com `Evento::Copiar(chave)`, que
copia o valor como aparece (`1.234,5`). Somando ou fora da área continua texto. Teste `testes_copiar`; foto conferida
(`FEATHERSHEET_CAPTURAR_MODELO=queda-tensao FEATHERSHEET_CAPTURAR_CELULA=B5:D9`). Novidade: "Clique na soma, média ou
contagem do rodapé para copiar o valor."

**Inspecionar documento (19/11).** ☰ Menu ▸ Informações, acima das propriedades (`app/inspetor_documento.rs`,
`Msg::InspetorDocumento`): no clique (não a cada quadro) lista comentários, dados pessoais, abas ocultas, linhas/colunas
ocultas, cabeçalho e rodapé (texto de cada aba, do `pg`; só apontado, porque a página não tem Ctrl+Z), formas e imagens ocultas (`extras.oculto`), nomes,
vínculos externos (`areas_citadas`) e macros/VBA. Remove só o que é seguro: comentários (um Ctrl+Z por
aba, `shortcut:`) e dados pessoais (limpa autor, modificado por, empresa, gerente e o autor de cada comentário, e liga
`Propriedades::sem_dados_pessoais`, que faz `carimbar_propriedades` não pôr o nome de volta; "Voltar a gravar" desliga).
A marca vai no `.fshz` (`pr.semDadosPessoais`) e no `.xlsx` (`<workbookPr removePersonalInformation="1"/>`, junto do
`codeName` no `.xlsm`, um `workbookPr` só); com ela, comentário novo nasce sem autor. Abas e linhas ocultas são só
apontadas (Reexibir… abre o diálogo existente). Testes `inspetor_documento::tests`,
`testes_sem_dados_pessoais`; foto `FEATHERSHEET_CAPTURAR_JANELA=informacoes` conferida. Novidade: "Menu ▸ Informações ▸
Inspecionar documento: veja comentários, seu nome, abas ocultas e vínculos antes de mandar a planilha, e remova o que não
deve ir."

**Filtro avançado na faixa (19/11).** Dados ▸ Classificar e filtrar ▸ Avançado (`app/filtro_avancado_ui.rs`,
`Msg::FiltroAvancado`): lista (proposta pela região da célula ativa), critérios opcionais, filtrar no lugar (oculta as linhas
que não passam, `alterar_visibilidade_dimensoes`, num Ctrl+Z) ou copiar para (cabeçalho + linhas que passam; recusa destino
com dados ou em cima da lista/critérios: nada é sobrescrito), e somente registros exclusivos. A regra dos critérios é a das
macros (`feathersheet_macros::filtro_avancado::linhas_que_passam`, agora `pub`). **Limites:** lista e critérios só desta aba;
a cópia leva o valor das fórmulas (`shortcut:`); filtrar no lugar não grava o `_xlnm._FilterDatabase` (reexibir pelas linhas
ocultas ou Ctrl+Z). Testes `filtro_avancado_ui::testes`; foto `FEATHERSHEET_CAPTURAR_DIALOGO=filtro_avancado` conferida.
Novidade: "Filtro avançado do Excel em Dados ▸ Avançado: critérios E/OU na planilha, copiar o resultado e só registros exclusivos."

**Agrupar formas (#79, 19/11).** Shift+clique numa forma (`Evento::SelecionarMais`, só no gesto de mover; a camada não
arrasta) junta-a em `EstadoObjetos::mais`; Organizar ▸ Agrupar (`Evento::Agrupar`, ícone `link`, ativo com ≥2) grava
`extras.grupo` = número novo (maior da aba + 1) em todos, e quem já era de grupo leva o grupo inteiro; com forma de grupo
selecionada o botão vira Desagrupar (`link-off`). Sem campo novo: `extras` vai e volta no `.fshz`. Mover uma forma do
grupo (`Evento::Transformar` com gesto "mover") anda as outras pelo mesmo tanto; **girar e redimensionar mexem só na
forma** (`shortcut:` no código; o Excel estica o grupo inteiro). As outras do grupo e do Shift+clique ganham contorno
tracejado (`Camada::marcados`, `App::marcados`). Colar/duplicar tira o `grupo` da cópia. `.xlsx`: o `grpSp` lido marca os
filhos com o mesmo número (grupo dentro de grupo vira o de fora; `Leitura::grupos`); **gravar `.xlsx` não escreve o
`grpSp`** (as formas saem soltas). Testes: `objetos::testes::agrupar_move_junto_e_desagrupar_solta` e o do
`desenho_objetos`. Novidade: "Agrupar formas: Shift+clique para escolher várias e Organizar ▸ Agrupar para movê-las
juntas; grupos do Excel chegam agrupados." Com isto o #79 fica completo (menos girar/esticar o grupo inteiro).

## Depois da v0.3.2 (13/11/2026): auditoria de fórmulas, arquivos e do app

Três leituras do código acharam 31 problemas com caso concreto; a lista, com o que foi feito e o que
falta, está em `docs/planejamento/AUDITORIA-2026-11-13.md`. Feitos e testados (testes novos em
`feathersheet-core/src/planilha/workbook/tests_auditoria.rs` e `app/excluir_aba.rs`):
- **Fórmulas como no Excel:** número vira texto com a vírgula da língua; fórmula que aponta célula
  vazia vale 0 (antes "vazio": MÉDIA e CONT.NÚM erravam); `SOMA(A1;B1)` ignora texto e lógico da
  célula; curinga no critério só casa texto e vale com `<>`; critério 0 não casa vazia; erro na faixa
  de SOMASE/SOMASES/MÉDIASE(S)/MÁXIMOSES/MÍNIMOSES é o resultado; SEERRO sobre matriz; ÍNDICE com
  linha ou coluna 0; datas em texto e NÚMSEMANA com tipo; TEXTO com seções e 15 algarismos.
- **App:** Ctrl+Z depois de abrir outro arquivo não traz mais a pasta anterior; macro e cálculo em
  fundo esperam a gravação (o "salvando" ficava preso); a cópia de recuperação não sai mais vazia
  durante macro; Ctrl+C em coluna inteira copia só até a área usada; "Excluir aba" com dados pede
  confirmação (e não exclui com fórmula em edição); o Ctrl+Z de inserir linha não leva a macro salva
  depois; nome de aba que o Excel recusa é recusado ao renomear.
- **Arquivos:** intervalo de coluna inteira para no limite do Excel (antes linha 100 milhões e o
  Excel consertava o arquivo); `#CIRCULAR!`/`#ERROR!` vão só como fórmula; CSV grava data, % e moeda
  como a célula mostra; `_x000D_` do Excel vira a quebra de linha; linha ou célula sem `r` não some.
- **Ctrl+S de `.xlsx` sem perder:** formato de coluna inteira (cada célula com o estilo combinado,
  `<col style>` nas vazias), painéis congelados e linhas de grade (`xlsx_write::com_vistas`, a vista
  mora no app); imagens e formas do arquivo entram no aviso de "Salvar como" (`nao_convertidos`).
Depois (mesmo dia): M7 (copiar fórmula desloca `B:B`), M9 (PROCX com linha inteira, do fim e curinga) e
P9 (Delete na tabela filtrada não apaga as escondidas), A1 (formato da linha e da aba inteira no `.xlsx`), A2 (imagens gravadas no `.xlsx`), P7 (renomear aba esquece o Ctrl+Z das abas
reescritas) e A10 (link para dentro da pasta, `#Aba!A1`, lido, gravado e seguido pelo clique); e o cursor do Claude (`sheet_mouse`) agora tem
liga/desliga em Configurações ▸ IA (MCP) ▸ "Cursor da IA na tela" (`config.mcp_cursor`, desligado de fábrica).
Ficou de fora (anotado): `ANO(vazia)` dá 1899 porque o motor não tem o 29/02/1900 falso do Excel.
Novidades: "Fórmulas mais fiéis ao Excel: célula vazia vale 0, curingas e critério 0 nos CONT.SE e SOMASE, ÍNDICE com linha 0, TEXTO com seções."
"Ctrl+Z depois de abrir outro arquivo não traz mais o anterior; excluir aba com dados pede confirmação."
"CSV exportado com datas e porcentagens como aparecem na célula; .xlsx com coluna inteira abre no Excel sem reparo."
"Salvar .xlsx mantém o formato da coluna inteira, os painéis congelados, as linhas de grade e as imagens."
"PROCX traz a linha inteira, procura do fim e aceita curinga; copiar fórmula com B:B anda para C:C."
"Delete numa tabela filtrada só apaga as linhas visíveis, como no Excel."
"Links para outra aba do .xlsx funcionam e voltam para o Excel; no Inserir link, #Aba!A1 leva a uma célula da pasta."
"Cursor da IA na tela: liga e desliga em Configurações ▸ IA (MCP)."

**Cascata e histograma no `.xlsx` (#27, 16/11).** O Excel descartava o desenho inteiro da aba ("Parte removida:
drawingN.xml") porque o `cx:f` do `chartExN.xml` levava o intervalo direto (`'Aba'!$A$1:$A$5`); o Excel só aceita ali
um nome oculto `_xlchart.v1.N` (conferido num `.xlsx` gravado pelo Excel 16, que usa `_xlchart.v5.N`). Agora
`xlsx_write_graficos::nome_xlchart` junta os intervalos da pasta inteira e o `workbook.xml` declara cada um com
`hidden="1"`; o leitor já resolvia esses nomes. **Sem conferir num Excel de verdade.** Diferenças que ficaram (o
Excel grava, nós não): `style`/`colors` do chartEx e a reserva como forma de texto em vez de gráfico clássico.
Novidade: "Gráficos de cascata e histograma salvos em .xlsx abrem no Excel sem sumir o desenho da aba."

**FeatherQuery: renomear consulta (#93, 13/11).** Lápis na lista de consultas abre o campo do nome
(`Evento::PedirRenomear`, `Estado::renomeando`); `registro::renomear` recusa vazio e repetido e troca o
`Consulta: nome` em todo registro que lê a renomeada (origem, fontes extras, `fonte` das etapas). Pastas
(grupos) entraram depois, no mesmo dia: o ⋮ (ou botão direito) da consulta abre Renomear e Mover para pasta
(`Evento::AcoesConsulta`/`MoverParaPasta`); a pasta mora no registro (`_feather.pasta`, `registro::mover_para_pasta`,
mantida pelo `preparar_gravacao`) e o painel agrupa ao desenhar; pasta criada e ainda vazia não vai para o arquivo.
Etapas (#94): botão direito dá nome e descrição (`Estado::etapas_info`, `_feather.etapas`; acompanham remover e mover,
não o Ctrl+Z das etapas). Novidade: "FeatherQuery: renomeie consultas, organize em pastas e dê nome e descrição às etapas."

**SQL Server no FeatherQuery (#70, 13/11).** Terceiro tipo do mesmo modal. Crate `tiberius` (Rust puro, `rustls`; sem
`winauth`, só login do SQL Server; porta explícita, sem instância nomeada), assíncrono: `Cliente::Ms` leva um runtime
`current_thread` próprio. Sem sessão só de leitura no SQL Server: `SET IMPLICIT_TRANSACTIONS ON` na conexão e `IF @@TRANCOUNT >
0 ROLLBACK` depois de cada consulta (o DDL também volta), e `sql_da_pessoa` recusa `COMMIT`/`IMPLICIT_TRANSACTIONS` fora de
aspas (o T-SQL junta comandos sem `;`). Garantia de verdade só com usuário de leitura no servidor. Prévia com `TOP 8`
(`previa`), catálogo pelo `sys.objects` + `sys.partitions`, datas pelo `FromSql` do chrono (`valor_ms`). M: `Sql.Database("servidor,porta",
"banco")`. Testado num `mssql/server:2022` do podman, com e sem `?ssl`. Novidade: "FeatherQuery também lê do SQL Server."

**MySQL/MariaDB no FeatherQuery (#70, 13/11).** Mesmo formulário, escolha do tipo (`banco::Tipo`, `Evento::BancoTipo`; troca
porta e usuário se ainda estão no padrão do outro). Crate `mysql` (Rust puro, `rustls-tls-ring`), como o `mysql_banco.rs` do
FeatherDB; catálogo pelo `information_schema.tables` (só o banco escolhido, ou todos menos os do sistema). Só leitura pela
sessão (`SET SESSION TRANSACTION READ ONLY`): no MySQL o DDL fecha a transação sozinho. O SQL escrito vale para os dois
bancos só com um comando (`um_comando`; um `COMMIT; DROP` escaparia do modo leitura). Origem `Banco: mysql://…`, M
`MySQL.Database`. Testado num `mariadb:11` do podman (o mesmo teste `postgres_de_verdade` com `FEATHERSHEET_TESTE_PG=mysql://…`).
Novidade: "FeatherQuery também conecta no MySQL e no MariaDB."

**PostgreSQL no FeatherQuery (#70, 13/11).** `app/openquery_ui/banco.rs`, Obter dados ▸ Banco de dados PostgreSQL…
(`Evento::MostrarBanco`, formulário `Estado::banco`, num modal `janela::painel_banco`: configurar nunca vai para o centro onde ficam os dados, pedido do usuário; o endereço do "Da Web" também virou modal, `janela::painel_web`, e o centro só mostra a escolha de tabelas). Conexão igual à do FeatherDB (`featherdb-core/src/postgres_banco.rs`):
crate `postgres`, protocolo simples, tipo pela `prepare` (números, lógico; inteiro acima de 2^53 e datas como texto), SSL =
`sslmode=require` com `rustls` (criptografa, não confere o servidor). Copiado, não dependência: o `featherdb-core` traz
`rusqlite` com C, MySQL, Mongo e SSH. Tudo em `BEGIN TRANSACTION READ ONLY` … `ROLLBACK`. Navegador: até 200 tabelas/views
(`pg_class`, contagem estimada pelo `reltuples`), prévia `LIMIT 8` cada; com SQL escrito, um item "SQL: …". Teto de
1.048.576 linhas (`shortcut:` sem levar filtros ao servidor). Origem `Banco: postgres://usuario@servidor:porta/banco[?ssl] ▸
item` (sem senha), guardada como o Web (cópia na consulta; "Atualizar fonte" = `ReimportarBanco`, pega a senha do cofre ou
pede). Senha no cofre (`keyring` com `async-secret-service` pelo zbus, sem `libdbus`; chamado numa thread própria). Script
`lerBanco("postgres://…")` + `fonte.navegar(item)`; M: `PostgreSQL.Database("servidor:porta", "banco")` com
`[Schema, Item]` ou `[Query = …]`. Testes de verdade (ignorados): `FEATHERSHEET_TESTE_PG=postgres://u:s@h:p/db cargo test --
--ignored postgres_de_verdade` (passou num `postgres:16-alpine` do podman) e `cofre_ida_e_volta`. SSL não testado contra
servidor. Foto: `FEATHERSHEET_CAPTURAR_JANELA=openquery:banco`. Novidade: "FeatherQuery conecta no PostgreSQL: escolha a
tabela ou escreva um SQL; a senha fica no cofre do sistema."

**SQLite no FeatherQuery (#70, 13/11).** `feathersheet-io/src/sqlite_read.rs`: leitor próprio, só leitura, sem C (o
`rusqlite` traz C): cabeçalho, B-tree de tabela (folha 0x0d, interior 0x05), transbordo, registro (tipos 0–9, texto,
blob vira "[binário, N bytes]", inteiro acima de 2^53 vira texto), UTF-16, `INTEGER PRIMARY KEY` pelo rowid, colunas do
`CREATE TABLE` (`colunas_do_create`). Fora: views, `WITHOUT ROWID`, o que está só no `-wal`; lê o arquivo inteiro na memória
(`shortcut:`). No importador é "De arquivo" (`.sqlite`, `.sqlite3`, `.db`; fora do "Da pasta" por causa do `Thumbs.db`),
uma opção do Navegador por tabela (`FonteItem::Sqlite`). M exportado: `Odbc.DataSource` com o SQLite3 ODBC Driver.
Arquivo de teste `feathersheet-io/tests/arquivos/vendas.sqlite` (feito com o `sqlite3` do Python, páginas de 1 KB).
Novidade: "FeatherQuery lê bancos SQLite: escolha a tabela no Navegador." Parquet já estava no importador.

**Mostrar alterações (#82, 13/11).** O motor registra, ao fechar cada transação (`Historico::alteracoes`,
`Sheet::alteracoes`), o texto antes/depois das células que mudaram (estilo sozinho não conta); transação com mais de 50
células vira um registro com a área e a contagem; teto de 5000 (`shortcut:`). Desfazer/refazer, inserir/excluir linhas e
ordenar não entram. Só na sessão. Painel `app/alteracoes_ui.rs` (`alteracoes_painel: Option<bool>`, `Some(true)` = só a
seleção), Revisão ▸ Alterações. Foto: `FEATHERSHEET_CAPTURAR_ALTERACOES=1`. Novidade: "Revisão ▸ Mostrar alterações: veja o
que mudou em cada célula desde que abriu o arquivo." Com isto o #82 fica completo.

**Verificar Ortografia (#82, 13/11).** `app/ortografia_ui.rs`, Revisão ▸ Revisão de texto, F7 e Ferramentas ▸ Ortografia…
(`spellCheck`). Confere o bruto das células sem `=` a partir da ativa (só a seleção se tiver mais de uma célula), dá a volta
uma vez. Palavras: sem algarismo, e-mail, web, sigla e letra solta (`palavras`). Dicionário = `/usr/share/dict` do idioma
mais `dicionario-<idioma>.txt` e `dicionario-pessoal.txt` na pasta das preferências (`configuracao::caminho_config`), lido
uma vez por sessão; sem lista, avisa onde pôr (Windows e macOS não têm lista: `shortcut:` no módulo). Sugestões por
distância de edição ≤2 (≤1 até 4 letras). Alterar/Alterar todas numa transação (um Ctrl+Z). Foto:
`FEATHERSHEET_CAPTURAR_ORTOGRAFIA="texto"`. Novidade: "Revisão ▸ Ortografia (F7): ache e corrija palavras erradas, com
sugestões e dicionário pessoal." Ícones: `lock-open` e `abc` não existem no feather-ui (viram "+"); conferir com
`feather_ui::icones::existe`.

**Modo micro (#68, 13/11).** `config.faixa_micro` (Exibir ▸ Janela ou Configurações ▸ Planilha, `Msg::FaixaMicro`) vale com a
faixa recolhida (`App::faixa_micro`): `interface_editor::barra_micro` monta a barra de título (emblema, 5 ações, menus
clássicos com título de 88 px, nome do arquivo só com ≥1500 px, pesquisa, Expandir faixa, Configurações) e a linha das abas
some. Expandir volta às abas; recolher volta ao micro. Toda conta de "abaixo das abas" usa `App::topo_abaixo_abas` (43 ou
95), nunca `43.0 + 52.0`. Os resultados da pesquisa flutuam (`sobrepor_busca_micro`). Foto:
`FEATHERSHEET_CAPTURAR_FAIXA=micro` (`micro+mais` abre o menu Editar). Novidade: "Modo micro: menus e pesquisa na barra
de título, a planilha ganha a altura das abas."

**Permitir edição de intervalos (#82, 13/11).** Revisão ▸ Proteger: `app/intervalos_ui.rs` guarda os intervalos no `pr`
da aba (o campo antigo do TS, agora com a senha no formato do Excel: `{alg,hash,sal,giros}`), `.xlsx` em `<protectedRanges>`.
O motor só vê `Sheet::areas_liberadas` (não vai para o arquivo), posto por `App::liberar_areas` antes de cada ação conferida;
`celula_travada` = bloqueada no estilo e fora delas. Digitar em célula travada de intervalo com senha abre a caixa da senha
(`senha_intervalo`); a certa vale até fechar o arquivo (`intervalos_abertos`). Mudar a lista só com a aba desprotegida. Senha
PBKDF2 do app antigo não confere e não vai para o `.xlsx`. Novidade: "Revisão ▸ Permitir edição de intervalos: partes da
planilha protegida continuam editáveis, com senha se quiser; vai e volta do Excel."

**Organizar e Painel de Seleção (#79, 13/11).** `App::grupo_organizar` (Formato da forma e Layout da Página): trazer/
enviar, alinhar e girar o objeto selecionado (sem seleção ficam apagados) e o Painel de Seleção (`objetos.painel_selecao`,
encostado à direita): lista os objetos da aba (o da frente em cima), clique seleciona, olho oculta (`extras.oculto`, o
mesmo da macro), setas mudam a ordem (`Evento::OrdemDe`). Agrupar não entrou: pede seleção de vários objetos. Novidade:
"Layout da Página ▸ Organizar e Painel de Seleção: veja, oculte e reordene as formas e imagens da aba."

**Autocompletar no editor de macros (#8, 13/11).** `app/macros/autocompletar.rs`: o catálogo sai das assinaturas do
`ajuda_macros::GUIA` (o objeto antes do ponto: `sheet`, `range`, `wb`, `app`; solto a partir de 2 letras; depois de `)` vale
o intervalo; variável desconhecida mostra intervalo e aba). Lista abaixo do cursor (acima, sem espaço), 8 à vista, com a
assinatura e o exemplo; ↑/↓, Tab/Enter, Esc pelo `on_key` do editor. Não abre dentro de aspas nem de `//`. Foto:
`FEATHERSHEET_CAPTURAR_JANELA=busca_macros FEATHERSHEET_CAPTURAR_COMPLETAR=sheet.se`. Novidade: "Editor de macros com
autocompletar: digite sheet. e escolha o método, com a forma de usar e um exemplo."

**`.iqy` do Excel no FeatherQuery (13/11).** Obter dados ▾ ▸ Arquivo ▸ Consulta Web do Excel (`importador::ler_iqy`):
o endereço vira a fonte Web de sempre e o `Selection=N` escolhe a tabela sozinho (`Estado::selecao_iqy`). Recusa com aviso
o `.iqy` com POST e com parâmetros (`["Cidade","…"]`); formatação da página não vem. Não abre pelo Abrir do app, só pelo
FeatherQuery. Novidade: "Abra no FeatherQuery as consultas Web (.iqy) salvas pelo Excel."

## Depois da v0.3.2 (13/11/2026): o .dmg do macOS volta

O job `macos.yml` não gerava `.dmg` desde a v0.2.0: as máquinas `macos-13` e `macos-14` do GitHub foram
aposentadas (o job ficava na fila ou era cancelado sem rodar) e o `libc` só estava declarado para Linux,
mas o `app/fuso.rs` o usa também no macOS. Agora as duas entradas rodam em `macos-latest` (a Intel sai
dela com o alvo `x86_64-apple-darwin`) e o `libc` vale para Linux e macOS. Rodado à mão em 13/11: os dois
`.dmg` saíram (Intel 22 MB, Apple Silicon 20 MB), **sem abrir num Mac**. Na próxima tag eles anexam à
release sozinhos. Novidade: "Instalador para macOS de volta (Intel e Apple Silicon)."

## v0.3.3 — 16/11/2026: FeatherQuery em bancos de dados, Revisão completa, autocompletar nas macros

Junta o que entrou depois da v0.3.2: FeatherQuery lê SQLite, PostgreSQL, MySQL/MariaDB e SQL Server (#70), com a
conexão num modal, pastas e renomear consultas, nome e descrição nas etapas (#93, #94) e a consulta Web `.iqy`;
Revisão ▸ Ortografia (F7), Mostrar alterações e Permitir edição de intervalos (#82); Organizar e Painel de Seleção
(#79); modo micro (#68); autocompletar no editor de macros (#8); cursor da IA liga/desliga (#9); auditoria de 13/11
(fórmulas e arquivos); cascata e histograma por nomes `_xlchart` (#27, **sem conferir no Excel**); `.dmg` do macOS
de volta. Novidades da 0.3.3 nas quatro línguas. Mesmo pelicano 0.1.34 e feather-ui v0.3.16.

## v0.3.2 — 12/11/2026: abre em placa de vídeo antiga, cálculo iterativo, números como no Excel

Junta o que entrou depois da v0.3.1 (itens de 11/11 e 12/11 da "Fila de trabalho" abaixo): cálculo iterativo, milhar
digitado e colagem/CSV por coluna, textos do motor e códigos de erro na língua do app, Feather Sans. **pelicano 0.1.34**
(crates.io; feather-ui continua na v0.3.16): o app não fecha mais ao iniciar em placa sem Vulkan/OpenGL utilizável pelo
wgpu (Intel HD 4000 / Ivy Bridge, #163, relato de quem instalou pelo Snap): a janela abre desenhada pela CPU; e se a
placa falha com o app aberto, as janelas passam para a CPU. Conferido numa máquina real com HD 4000 rodando o binário
do `main` com o pelicano novo (abriu e desenhou a tela inicial). **Sem conferir:** o Snap naquela máquina e o Windows.
Release publicada em 12/11 (`.deb`, `.tar.gz`, `.exe`, `SHA256SUMS`); o `.exe` de novo **sem VirusTotal** (mesma decisão
da 0.3.1). Snap 0.3.2 publicado no canal estável da Snap Store (revisão 6), com o mesmo binário do `.deb`, sem o teste de
instalar aqui (pede `sudo`). O binário do `.deb` abriu nesta máquina (foto pelo `FEATHERSHEET_CAPTURAR`).

## v0.3.1 — 11/11/2026: Excel com senha, dinâmica mais completa, francês, FeatherQuery

Junta o "Lote de 10/11/2026" e as entregas de 11/11 (seção "Fila de trabalho" abaixo, item a item). Destaques no
`VERSOES`: `.xlsx` com senha, tabela dinâmica mais completa, Consolidar e referências 3D, interface em francês. O
editor de consultas passa a se chamar **FeatherQuery** em todo texto visível. **feather-charts por `tag = "v0.3.5"`**
(a tag foi publicada em 11/11; é o mesmo commit do `rev` de antes). Telas novas conferidas por foto (faixa de Exibir,
Dados, Inserir, Configurações ▸ Planilha, Revisão em francês, dinâmica, janela do FeatherQuery); o arrasto do campo para
duas áreas e o filtro que fecha ao clicar fora foram conferidos pelo usuário na tela. **Sem conferir:** a atualização
sem PowerShell numa máquina Windows e um `.xlsx` cifrado pelo Excel de verdade.
Release publicada em 11/11 (`.deb`, `.tar.gz`, `.exe`, `SHA256SUMS`); o `.exe` **não passou pelo VirusTotal** (decisão do usuário:
publicar sem conferir; se um antivírus acusar o PowerShell escondido da atualização, corrigir numa 0.3.2). Snap 0.3.1
publicado no canal estável da Snap Store (revisão 5), sem o teste de instalar aqui (pede `sudo`).

## v0.3.0 — 08/11/2026: Power Query, dinâmica, Gantt, página e propriedades no arquivo

Junta as seções "Depois da v0.2.9" abaixo (o que entrou desde 04/11). Destaques no `VERSOES`: Power Query no
OpenQuery, dinâmica mais esperta (recomendadas, distintos/mediana/quartis, data como data), Gantt com o dia de
hoje, página e propriedades no arquivo. Plano e o que ficou para a 0.4.0: `docs/planejamento/ATIVIDADES-0.3.0.md`
(corte de 08/11). Pacote Linux compilado num Debian 12 (o binário pede no máximo a glibc 2.35). **feather-charts por `rev = "02bead2…"`**
(a v0.3.5 está no main dela; falta a tag). Sem resolver nesta versão: ROB-1 (fórmulas trocadas de coluna, visto
gravado na Planilha Pesada), segmentação completa, número como texto, autocompletar das macros.

## Depois da v0.2.9 (04/11/2026): pedidos de quem vem do Excel — entram na próxima versão (lote)

Alça com Ctrl inverte cópia/sequência como no Excel (`plan_fill_ctrl`, `Sheet::preencher_com`;
`preencher_pela_alca` lê `ctrl_pressionado`; Ctrl+D e Série seguem sem inverter). Botão direito da
célula ganhou "Formatar células…" (aba Número) acima de "Limpar formatação". **Novidades da próxima
versão** (pôr em `VERSOES`): também "Ctrl ao arrastar a alça conta 7, 8, 9; sem Ctrl o número é
copiado, como no Excel." e "Formatar células… no botão direito da célula.".

Macros (06/11/2026): `SpecialCells` do VBA converte (visíveis, vazias, constantes, fórmulas, erros, última
célula) pela API nova `specialAreas` e pelo intervalo de várias áreas (`feathersheet-macros/src/varias_areas.rs`);
as visíveis já enxergam o AutoFiltro pedido na mesma macro (`visiveis.rs`); `AutoFilter` com `xlAnd`/`xlOr` de
comparação (`Criterio::Duas`, JSON `{op:"e"|"ou"}`). `feathersheet-macros` passou a depender do
`feathersheet-io` (só o `filtro_json`). Novidade: "Macros do Excel com SpecialCells (copiar só as linhas
visíveis do filtro, excluir as filtradas) e AutoFilter com duas condições agora convertem." Na mesma leva: `prompt` não existia
no motor (o `InputBox` convertido quebrava ao rodar); agora a pergunta de texto segue o esquema do
`confirm` (`Contexto::textos`, `Resultado::entrada`, campo na caixa da pergunta, `Msg::MacroDigitar`).
`Application.InputBox` com `Type:=`, instrução `Mid(...) = x`, `Foo (x)` por valor, `As New Collection` no
módulo, `CompareMode`/`Key` do Dictionary e ler `.Left/.Top` da forma (`objetos_aba::ReguaSimples`; as
medidas padrão da grade moram no `objetos_aba`). Novidade: "InputBox das macros do Excel volta a
funcionar, com caixa para digitar." Número vira texto pelo CStr (vírgula decimal, 15 algarismos: `"Total: " & 0,1+0,2` dá `Total: 0,3`; `app.decimalSeparator`). Novidade: "Macros do Excel escrevem número com vírgula ao juntar texto, como no Excel em português." `AdvancedFilter` (copiar e no lugar, `filtro_avancado.rs`, marca `_xlnm._FilterDatabase`; o `.xlsx` grava `_xlnm.*` com `localSheetId`). Novidade: "Filtro Avançado das macros do Excel (copiar para outro lugar e filtrar no lugar)." Formulário da macro: Tab/Shift+Tab na ordem do `TabIndex` (`Controle::ordem`, `Formulario::ordem_do_tab`, `Evento::Tab`) e foco no primeiro campo ao abrir. Novidade: "Tab passa pelos campos dos formulários das macros na ordem do Excel." `CopyPicture` → `sheet.pastePicture` (`impressao_pdf::svg_da_area` com o `trait Pincel` comum ao PDF; `PedidoImagem` aplicado no `concluir_macro` por `objetos::aplicar_imagens_da_macro`) e botão direito ▸ "Copiar como imagem" (`imagens_importadas::gravar_png_na_area_de_transferencia`, fora da thread da janela). Novidade: "Copiar como imagem no botão direito e CopyPicture das macros do Excel."

## Depois da v0.2.9 (08/11/2026): pelicano 0.1.30 do crates.io, feather-ui v0.3.15

O pelicano deixou de vir por git/tag: `pelicano = "0.1.30"` (crates.io, travado na 0.1.30 no Cargo.lock;
a 0.1.31 muda o Tab entre campos, conferir na grade antes de subir). feather-ui v0.3.15 é a v0.3.14 com
esse pelicano. Com isso o editor da célula usa o alinhamento horizontal dela (`TextInput::align_x`, #90):
centralizada edita no centro, à direita edita à direita. Novidade: "Editar uma célula centralizada mantém o
texto no centro, como no Excel."

AutoConclusão (como no Excel/Sheets): `App::completar_valor` (`entrada_rapida.rs`) acha o único texto do bloco
contínuo da coluna (até 10 mil linhas para cada lado, `valores_do_bloco`, o mesmo do Alt+↓) que começa com o
digitado; `sugestao_valor` (digitado, completo) é refeito no `Msg::Editar`/`Digitar` só quando o texto cresce,
mostrado embaixo da célula (editor na célula) ou da fx (digitando direto), e aceito no `confirmar` (Enter, Tab,
clique fora). Esc e o duplo clique limpam. shortcut: etiqueta embaixo, não o resto selecionado dentro da célula
como no Excel — o pelicano 0.1.30 não seleciona trecho por tarefa (a 0.1.31 tem `Task::select_range`).
Novidade: "AutoConclusão: ao digitar, o valor que já existe na coluna aparece e o Enter completa."

Ctrl+Z de inserir/excluir linhas e colunas (#65): ver "Cuidados ao mexer". Novidade: "Ctrl+Z desfaz
inserir e excluir linhas e colunas, com as fórmulas que liam a área."

Configuração de página no arquivo: campo `pg` da aba (JSON, `feathersheet-io/src/pagina_aba.rs`: papel pelo
código `paperSize` do Excel, orientação, escala/Dimensionar para Ajustar, margens em mm com as de
cabeçalho/rodapé, grade, títulos A/1, cabeçalho/rodapé, áreas de impressão, linhas/colunas que repetem).
O app (`impressao_ui.rs`) lê e grava só o `pg` (`pagina_aba`/`mudar_pagina_aba`, sem desfazer, marca
alterado); margem do arquivo que não bate com Normal/Estreita/Larga fica exata (`Pagina::margens`). `.xlsx`:
`xlsx_read/pagina.rs` (`printOptions`, `pageMargins`, `pageSetup`, `headerFooter`, `sheetPr/pageSetUpPr`) e
`_xlnm.Print_Area`/`Print_Titles` pelo `localSheetId` (`sheet::pagina_dos_nomes`; não viram nome da pasta);
o gravador escreve os mesmos (`pagina_aba::xml_da_aba`) e tira nome antigo igual da mesma aba. O cabeçalho do
Excel com códigos (`&C&P`) vira texto simples na tela e volta como veio se o texto não mudou
(`xlsx_cabecalho`). Campo novo na aba: lembrar do `enxugar.rs`. Novidade: "Margens, papel, área de
impressão e cabeçalho ficam salvos no arquivo e vêm do .xlsx do Excel."

Configurações ▸ "Salvar planilha nova como .xlsx" (`config.salvar_xlsx`, `Msg::SalvarXlsx`): o Salvar como…
de planilha sem arquivo (ou vinda de .csv/.parquet) sugere `.xlsx` (`App::formato_sugerido`). Novidade:
"Opção para o Salvar como… sugerir .xlsx, para quem troca arquivo com o Excel."

Configurações ▸ "Seus dados" (seção 2; `SECAO_MCP`/`SECAO_SOBRE` andaram para 8/9): nome, iniciais e organização,
os mesmos de Primeiros passos (`Msg::PrimeirosPassos(Evento::Nome…)`). Novidade: "Nome e empresa que vão nos
arquivos agora também em Configurações ▸ Seus dados."

Propriedades da planilha (☰ Menu ▸ Informações, como Arquivo ▸ Informações do Excel): `Workbook::propriedades`
ganhou título, assunto, palavras-chave, categoria, comentários, gerente, criado e modificado (W3CDTF UTC);
`.fshz` no `pr` (chaves só quando preenchidas), `.xlsx` no `docProps/core.xml`/`app.xml` (leitura e escrita).
`Msg::Propriedade(i, texto)` edita (marca alterado, sem desfazer, como no Excel); `carimbar_propriedades` põe
criado (se vazio) e modificado ao salvar. Foto: `FEATHERSHEET_CAPTURAR_JANELA=informacoes`. Novidade:
"Propriedades da planilha (título, assunto, autor, empresa, palavras-chave, comentários) em Menu ▸ Informações,
lidas e gravadas no .xlsx."

Salvar `.xlsx` mais rápido (08/11/2026, `medir_exportar` na Planilha Pesada, 13 mi de células): montar o XML
18,5 → 6,4 s, arquivo idêntico byte a byte. Três causas no `xlsx_write.rs`: fórmula empacotada com estilo direto
saía do atalho do padrão (agora o `xf` vem do estilo da célula, `Padrao::implicito`); padrão cuja primeira célula
tem `#REF!` montava o atalho errado e a conferência desligava o atalho para a coluna inteira
(`esperando_celula_valida`); `padroes` virou `Vec` pelo id (o `HashMap` custava 13%). Novidade: "Salvar em .xlsx
até 3 vezes mais rápido em planilhas grandes."

Itens do plano da 0.3.0 (08/11/2026): SEG-2 pânico de macro não derruba o app (`macro_dialogo::sem_derrubar`,
também no `em_fundo`); SEG-3 teto de 512 MB na fonte Web do OpenQuery (`importador::ler_com_teto`); DIN-5
Dinâmicas recomendadas de verdade (`feathersheet_core::dinamica::campos::sugestoes`, `Estado::sugestoes`,
`Evento::Sugestao`). Novidade: "Dinâmicas recomendadas: sugestões prontas (soma por região, contagem…) com um
clique." e "Uma macro com defeito não fecha mais o FeatherSheet."
ROB-8: SOMASES/CONT.SES/MÍNIMOSES… com critério zero, `""` ou célula vazia usam o agregado
(`ChaveCriterio::alternativas` + `Agregado::grupo_de`: vazia, texto vazio e FALSO viram chaves; o trecho além
das linhas lidas, `Agregado::fora`, conta quando todo critério aceita vazia) e o agregado para na última linha
das colunas lidas (`Sheet::agregado`, sem matriz derramada na aba). DIN-1: rótulo de data na dinâmica sai com
o formato de data da origem (`config_campo::formatos_de_data`). Novidades: "Fórmulas SES com critério em
branco ou zero muito mais rápidas em planilhas grandes." e "Datas na tabela dinâmica aparecem como data, não
como número."
DIN-12: a dinâmica ganhou Contagem de distintos, Mediana e Percentil (`AgregacaoOp::Distintos/Mediana/
Percentil(p)`, `acumulador::percentil`; no `.xlsx` vão como soma, como a Moda). Novidade: "Tabela dinâmica
com contagem de distintos, mediana e quartis."
Gantt (#133, GRA-1): linha do dia de hoje e barras pela situação (concluída/em andamento/futura) pela data de
hoje (`filtro_extra::serial_de_hoje`, passada no `PedidoAvancado::hoje`); painel Séries ▸ Cronograma
(`Mudanca::GanttHoje/GanttSituacao`, `AlvoCor::Gantt(k)`), MCP `sheet_grafico` com `gantt {…}`, estilo em
`_featherNative.estilo.gantt`. **feather-charts por `rev = "02bead2…"`** (a v0.3.5 está no main dela; falta
publicar a tag e trocar o `rev` pela tag antes do lançamento; a feather-io v0.2.1 ainda puxa a feather-charts v0.3.4, e as duas entram no binário até a feather-io subir de versão). Novidade: "Gantt com a linha de hoje e as tarefas
pintadas como concluídas, em andamento ou futuras."

Corte da 0.3.0 (08/11/2026): 51 itens do `ATIVIDADES-0.3.0.md` marcados ⏭️ 0.4.0 (memória, dinâmica avançada,
interface grande, Windows, Feather Sans…). INT-13: `feathersheet --version`/`-V`. INT-7: em teste o `tr` anota as
chaves sem tradução (`idioma::tirar_faltas`); o teste `com_ingles_as_telas_principais_nao_pedem_texto_sem_traducao`
desenha as telas principais em inglês. Novidade: "Interface em inglês e espanhol sem sobras de português na faixa."
MEM-10: F12 ▸ Métricas mostra "Memória própria" (`RssAnon`, o número do Monitor do GNOME) e a parte compartilhada
(`VmRSS − RssAnon`). Novidade: "O F12 mostra a mesma memória do Monitor do sistema."
MEM-9: salvar não copia mais as colunas de texto (`ColunaTxtBruta::blocos`, ids do pool pelos blocos da coluna viva
e a tabela id → índice local; id fora de `2×linhas+1024` volta à cópia). 2026_03.fshz: +605 → +134 MB ao salvar,
arquivo idêntico. MEM-6 medido (ver o plano): vazio 43 MB própria, 2026_03.fshz 2.064 MB. Novidade: "Salvar planilha
grande gasta bem menos memória."
Conferência da 0.3.0 (08/11/2026): o arquivo `porta token` do MCP nasce 0600 (`ao_vivo::gravar_so_do_dono`);
testes novos travam o "Ativar macros?" ao reabrir .fshz/.xlsm, nenhum item de menu clássico sem ação
(`nenhum_item_dos_menus_fica_sem_acao`, com as exceções do momento), a barra de abertura (montar ≤ 80%,
`porcentagem_de`) e um Ctrl+Z por ação do painel da dinâmica. METAS com a numeração real no topo.

## Depois da v0.2.9 (06/11/2026): OpenQuery (FeatherQuery) mais completo — entra na próxima versão (lote)

Motor (`feathersheet-core/src/consulta/query/extras.rs`): `Formatar`, `Preencher`, `RemoverLinhas`,
`Transpor`, `Inverter`, `DividirPosicoes`, `Personalizada` (os dados vão para uma `Sheet` de rascunho e a
fórmula é preenchida para baixo com `preencher_formula_faixa_vazia`; `[Coluna]` vira `B1` em
`traduzir_formula`), `Juntar.semelhanca` (chave normalizada + Levenshtein, teto de 20 mi de comparações),
`Tipar.erros` (o que não converte vira `#VALOR!`). Campos lógicos aceitam `null` (`bool_ou_nulo`: o script
corta argumento nulo no fim). Tela: modais no `feather_ui::dialogo` com prévia (`etapa::previa`, 200
linhas de amostra), Ctrl+Z das etapas (`historico`/`refazer` em JSON), Duplicar/Referenciar
(`Consulta: nome` como origem; `fonte_da_salva`/`resultado_da_salva`, ciclo até 8), aviso de tipo
(`aviso_tipo`), Da pasta (`importador::ler_pasta`, relida só pelo botão), `.fshz/.xlsm/.parquet` pelo
`App::decodificar_caminho`, Exibir ▸ Dependências (`registro::dependencias`), "Atualizar ao abrir"
(`_feather.atualizarAoAbrir`). A origem gravada não é mais traduzida (#151; `origem_interna` lê as antigas).
Foto: `FEATHERSHEET_CAPTURAR_JANELA=openquery[:etapa|transformar|adicionar|dependencias]`.
Novidades: "OpenQuery com os passos do Power Query que faltavam: substituir, preencher, formatar texto,
remover linhas, transpor, dividir por posição, coluna personalizada com fórmula, agrupar com várias contas."
e "Consultas: Ctrl+Z nas etapas, duplicar e referenciar, juntar arquivos de uma pasta e nomes parecidos,
atualizar ao abrir o arquivo."

**Power Query ↔ FeatherQuery (08/11/2026).** `openquery_ui/power_query.rs`: léxico e leitor do M
(`let … in`, `each`, `if`, `is`, registros, `#"nomes"`, `#(lf)`), cada etapa do `let` lida sozinha
(`ligacoes`: a que não entende vira `// M não convertido:` e não estraga as outras). `de_m` → linhas do
script (colunas pelo nome) que o `script::ler` de sempre confere; `para_m` → M com os nomes de etapa do
Power Query na língua do app. Cobre 39 `Table.*` (as que os botões do Excel geram), as fontes (Excel/CSV/
Parquet/pasta/Web/tabela da pasta/#table) e ~45 funções internas; coluna personalizada vai e volta pela
fórmula (`formula`/`de_formula`, só funções de `FUNCOES`). Atalhos novos no script, resolvidos pelo
cabeçalho daquele ponto: `removerColunas`, `pivotarColuna`, `despivotarSelecionadas`; `mesclarConsultas`
aceita a coluna da outra consulta pelo nome. Tela: colar M no Editor avançado e Aplicar converte (com
etapa sem equivalente, para e mostra o script comentado); "Copiar como Power Query" no rodapé. Limites:
a fonte do M vira comentário (a da consulta continua a de Obter dados); `contagem` (só números) vai como
`List.Count(List.Select(…, each _ is number))` e `Table.RowCount(_)` volta como contar valores da chave.
Consultas guardadas no `.xlsx`: `feathersheet-io/src/power_query.rs` tira o `Section1.m` do DataMashup
(`customXml/item*.xml`, UTF-16 ou UTF-8, base64 → u32 versão + u32 tamanho + zip OPC; teto de 32 MiB por
parte) e `consultas_da_secao` separa cada `shared Nome = …;`. Obter dados ▾ ▸ "Consultas do Power Query
deste arquivo…" (só com a pasta vinda de `.xlsx`/`.xlsm`, `App::arquivo_xlsx_aberto`) lista e abre a
escolhida no Editor avançado. Etapa que é só um valor (`col = "x"`) vira constante nas seguintes. Conferir
com arquivo real: `FEATHERSHEET_PQ_XLSX=x.xlsx cargo test consultas_de_um_xlsx_real -- --ignored --nocapture`
(nos dois `.xlsx` com Power Query desta máquina, o que sobra é M avançado: tabela dentro de agrupamento,
JSON em registros, `Table.TransformColumns` com função por subtabela).
Novidades: "Power Query do Excel no OpenQuery: cole o M e as etapas aparecem; Copiar como Power Query
leva de volta para o Excel." e "Abra as consultas do Power Query que o Excel guardou no .xlsx."

## v0.2.9 — 04/11/2026: minigráficos, desfazer exato, auditoria

Inserir ▸ Minigráficos (Linha, Coluna, Ganhos/Perdas): modelo e contas em
`feathersheet-core/src/operacoes/minigraficos.rs` (`criar`, `refluir`, `desenhar` em frações da
célula), campo `mg` da aba (JSON; `feathersheet-io/src/minigraficos_aba.rs`), `Passo::Minigraficos`
(um Ctrl+Z, `alterar_minigraficos`). `.xlsx`: `xlsx_read/minigraficos.rs` e
`xlsx_write_minigraficos.rs` (`extLst` x14 `sparklineGroups`; saíram do `nao_convertidos`).
Tela: `minigraficos_visual.rs` (desenho na célula, valores ao vivo) e `app/minigraficos_ui.rs`
(diálogo Criar Minigráficos, aba contextual Minigráfico, `refluir_minigraficos` nos 4 lugares de
inserir/excluir). Limites: cor por tema no `.xlsx` vira a padrão; faixa de dados de outra aba não
anda com inserir/excluir feito lá; eixo mín/máx personalizado e "vazias como zero" não existem.
F1 ▸ "Exportar como Markdown" (`ajuda::markdown`, #71). `.fshz` guarda texto com espaço nas pontas e
o texto do número (`tem_texto_ambiguo`, #138). Auditoria (ponytail-audit de 04/11): a foto do
desfazer (`snapshot`) guarda o número inteiro (`to_js_string`) e marca com `MARCA_TEXTO` o texto que o
analisador leria como outra coisa; `.xlsx` lido com formato guarda o `<v>` exato; Ctrl+S com erro ao
conferir o arquivo pede "Salvar como"; `de_rede` aceita `/\` misturados; parser rápido de XML com
teto de 64 MB e de profundidade; teto de células do `.fshz` vale para o arquivo inteiro; atualizador
em pasta privada (0700) e link de novidades só do repositório; VBA importado com eventos começa
bloqueado; parciais velhos de salvamento apagados; `//` dentro de aspas não é comentário nos riscos de
macro; inserir/excluir esquece o desfazer de todas as abas; MCP `sheet_mouse` recusa com aviso de
segurança aberto; aviso de cópia automática desligada; textos cortados no `.xlsx` contados
(`xlsx_write::textos_cortados`); tabela de alturas por impressão digital, não por mensagem; desfazer
com teto de 512 MB (`LIMITE_DO_DESFAZER_BYTES`); `bloco.rs` ficou só com `texto_formatado`.
SUBTOTAL/AGREGAR recalculam quando uma linha aparece ou some (`Sheet::visibilidade_mudou`, marcado em
`aplicar_dimensao`/`aplicar_ocultas_desde` e no filtro de tabela; consumido por `sujar_subtotais_se_mudou`
no fim da transação, no desfazer/refazer e no `editar`): o total de tabela com filtro somava as escondidas
desde o rastro de células do #136.
Não feito: compactar o `StringPool` na sessão (o arquivo já grava dicionário por coluna) e gerar o
`.xlsx` direto da pasta viva (travaria a tela).

## v0.2.8 — 03/11/2026: controles de formulário, texto que transborda, auditoria

Controles de formulário do Excel ligados na camada de objetos (`app/objetos/controles.rs`): botão
direito próprio, Formatar controle com as abas de cada tipo (`formatar_controle.rs`: Fonte,
Alinhamento, Cores e linhas, Tamanho, Proteção, Propriedades, Texto Alt., Controle) gravadas no
VML (`xlsx_write_controles.rs`/`xlsx_read/controles.rs`), grupo Controles na aba Macros, arrastar a
caixa da barra de rolagem. F12 lista o que o .xlsx tem e não foi convertido
(`xlsx_read/nao_convertidos.rs`); Ctrl+S num .xlsx com essas partes pede "Salvar como". Texto
comprido invade as vizinhas vazias (`grade.rs`, `transbordos`, desenhado depois das células).
Auditoria: fechar durante gravação/exportação volta a perguntar (`gravacao_terminou`/
`exportacao_terminou` chamam `proteger`); .xlsx acima de 1.048.576 linhas dá erro (`cabe_no_excel`);
texto >32.767 e número >9,99E+307 cortados; recuperar órfã só apaga depois de abrir; formulário de
macro respeita `macros_liberadas`; caminho de rede (`canal::de_rede`) não é tocado antes da
permissão; pasta de canais sem XDG_RUNTIME_DIR em `~/.cache`; cache de agregados com teto
(`LIMITE_GRUPOS`); PROCV/CORRESP cortam na última linha usada; PERCENTIL/MED/MAIOR/MENOR usam
`FaixaLida::numeros_ordenados`; `.fshz` não declara mais de `MAX_CELULAS_DECLARADAS`. Motor:
`compare` iguala números até 15 dígitos (finitos), `compare_exato` para ordenar; argumento em
branco vale 0 em `collect`; CONT.VALORES conta texto vazio. CSV regravado mantém o separador do
arquivo. Exemplo: `cargo run --release --example exemplo_controles -- saida.xlsx` no
`feathersheet-io`.

## v0.2.7 — 03/11/2026: .xlsm com o VBA original, pelicano v0.1.27

Salvar como `.xlsm` mantém o projeto VBA do arquivo original; `.xlsx`/`.xlsm` abrem como o arquivo
de trabalho. Instalador do Windows em Program Files\FeatherWorks\FeatherSheet para todos os usuários.
pelicano v0.1.27 e feather-ui v0.3.12 (no Windows sem pintura na placa, o quadro da CPU vai direto
para a janela).

## v0.2.6 — 02/11/2026: abrir sem pico de memória, "…" que não calculava, macros mais completas

`.fshz` aberto pelo app (`fshz_read::ler_fshz_arquivo_compacto`): colunas de número e texto em
blocos de 65.536 linhas compactados na hora (`arrow_fluxo::Plano`/`Montador`, `ChunkedColumn::
compactar_bloco`), textos direto no pool da aba (os dicionários estão nos metadados, lidos antes
do Arrow); `AbaLida::prontas` só vale com `para_workbook_soltando`. planilha-4M-EA (131 colunas de
texto × 4 M): pico 2,2 → 1,07 GB, abrir 12,7 → 7,5 s (`dicionario_local` sem SipHash: 28% do tempo).
"…" na tela: a grade anota em `CacheGrade::pendentes` as células que desenhou pendentes e o pulso
calcula essas primeiro (`calcular_visiveis`; para se não houver progresso, `vistos_parados`); a
estimativa por zoom pula linhas/colunas ocultas pelo eixo em cache (`EstadoEixos::visiveis_desde`, #140).
Conversor VBA: `Collection`/`Scripting.Dictionary` (`gerar/colecao.rs`, `vba.colecao()`/`vba.dicionario()`),
ByRef, MsgBox com todos os botões, AutoFilter, formas, `WScript.Shell`, evento Ao selecionar células.
Instalador: `.xlsm` no "Abrir com" e `EstimatedSize`. Padrão da suíte para o que aparece fora da
janela: `FeatherOffice/docs/PADRAO-SISTEMA-OPERACIONAL.md`.

## Depois da v0.2.5 (01/11/2026): vínculo externo empacotado e "Atualizar valores" em paralelo

Fórmula que lê outro arquivo (`[Arq.xlsx]Aba!A1`) passa no `pattern_check` e vai para a coluna
empacotada: no `.xlsx` do #136, 1,29 mi → 92 mil avulsas, abrir 4,4 → 2,3 s e 1,66 GB → 320 MB.
Quem acha essas fórmulas: `FormulaTable::padroes_com_vinculo` (por padrão: col, 1ª e última linha) e
`Sheet::vinculos_empacotados(_por_padrao)` (área pela análise da 1ª e da última linha);
`sujar_vinculo` suja a coluna (padrão ≥ 1.000 linhas) ou as linhas; `geracao_externas` mistura a
`assinatura_vinculos`. O cálculo paralelo lê as abas de fora (`Vista::com_nome`). **Planejador por
linha** (`calculo_paralelo::planejar`): avulsa só exclui quem lê a linha dela, e uma coluna só espera
outra se ler linha suja dela (totais na linha 2 lendo os dados e dados lendo a coluna dos totais na
própria linha era ciclo por coluna); avulsas já calculadas vão às threads por cópia
(`Sheet::avulsas_prontas`); `Workbook::calcular_em_paralelo` calcula até 1.000 avulsas sujas entre
rodadas (até 3). "Atualizar valores" nesse arquivo: 1,75 mi de fórmulas em paralelo, ~6 s (antes ~3 s
+ ~12 s sob demanda); conferido célula a célula, e na ida e volta `.xlsx`/`.fshz`. Seleção de colunas:
`SheetCtx::varrer` percorre as posições quando o retângulo é menor que as avulsas.

## v0.2.5 — 01/11/2026: .xlsx com milhões de fórmulas e vínculo externo (#136)

Um `.xlsx` de 54 mil linhas com 2,3 milhões de `{=SOMA(CONT.SE($O$8:$AC$8;…))}` lendo `[1]FILTROS!`
não abria (>10 min); agora 4,4 s (4 núcleos) e 1,4 GB. `DependencyGraph::range_index` é por (bloco de linhas,
bloco de 16 colunas) e agrupado pela faixa (`[r0, c0, r1, c1]`): milhões de fórmulas lendo a mesma
faixa custam uma conferência. `step_pattern_dependents` recebe o `columns_in_use()` montado uma vez
por cascata. A troca `[N]` → arquivo na leitura (`trocar_origem_externa_na_leitura`) não guarda
desfazer, e ela, `sujar_vinculo` e `sujar_externas(_de)` fazem uma cascata só (`adiar_cascata`).
**Abertura sem recálculo:** `[N]` vira o nome do arquivo no texto da fórmula ao ler a aba
(`xlsx_read/vinculos::trocar_numeros`; os livros são lidos antes das abas), e os valores guardados
dos vínculos (`instalar_externos_na_leitura`) e os nomes definidos (`definir_nome_na_leitura`) entram
sem sujar nada: o resultado que o Excel gravou vale (37,8 → 4,4 s; conferido célula a célula contra o
recálculo). `SUBTOTAL(3;DESLOC(faixa;LIN(faixa)-LIN(x);0;1))` dá um valor por linha
(`fn_subtotal::desloc_por_item`), como no Excel.
**Na janela, com a mesma pasta:** largura de coluna não suja nada (`sujar_externas_de` sai com
`rects` vazio: 360 → 0 ms); a lista de arquivos citados (`Workbook::areas_citadas`) fica guardada
pela `Sheet::geracao_externas` (sobe quando uma fórmula de vínculo entra ou sai) e é montada na
thread da abertura; o scroll usa a soma acumulada das alturas (`grade::modelo::alturas_proprias`,
refeita a cada `update`); recém-aberta não ganha cópia de recuperação, e a cópia automática só vale
até 1 milhão de células (`LIMITE_CELULAS_COPIA_AUTOMATICA`: a fotografia ainda é feita na thread da
janela, 2,8 s e +3 GB nessa pasta). Texto girado corta pela altura, não pela largura.
Medir: `cargo run --release --example medir_abrir_calcular -- arquivo.xlsx` no `feathersheet-io`.
**Aviso de versão nova** (`app/atualizacao.rs`): ao abrir, no máximo uma vez por dia
(`procurar_atualizacoes`/`ultima_procura_atualizacao` na configuração), lê `releases?per_page=15`
(pré-lançamentos inclusive, rascunho não) e mostra o cartão no meio da grade (Novidades / Depois /
Atualizar e reiniciar, que roda o roteiro de sempre e fecha o app no Windows/macOS). Foto:
`FEATHERSHEET_CAPTURAR_ATUALIZACAO=0.2.6`.

## v0.2.4 — 01/11/2026: janela certa no Windows 10 (#135)

pelicano v0.1.22 e feather-ui v0.3.10: no Windows, sem DX12/Vulkan (Intel HD 2500 no Windows 10),
o pelicano não cai mais no OpenGL, que desenhava a imagem deslocada com faixa preta; a janela
mostra pela CPU (o mesmo que `PELICANO_GPU=nenhuma`, conferido na máquina do #135). Diagnóstico
de janela sem o pelicano: `examples/janela_diag.rs` (só local, no repositório do pelicano).
Traduções en/es das novidades da 0.2.3, que tinham saído sem.

## v0.2.3 — 01/11/2026: paridade do VBA e manifesto do Windows

Conversor VBA com ~99% das funções (lista e medição em `docs/planejamento/PARIDADE-VBA-FUNCOES.md`;
`examples/rodar_xlsm.rs` e `rodar_bas.rs` rodam as macros convertidas). Novos na API das macros:
`app.shell` (pergunta "Rodar?" e acusa binário escondido, `riscos::binario_escondido`),
`app.openUrl`, `sheet.protect/unprotect`, `sheet.showGridlines`, `sheet.shape(nome)` (o `.xlsx`
guarda o nome da forma em `nome`; nome padrão vale em pt/en/es) e `range.getFormat()`.
`.exe` com manifesto (`pacote/feathersheet.manifest`: Windows 10/11, PerMonitorV2) — tentativa
para o #135 (Windows 10 + Intel HD 2500: imagem deslocada de onde os cliques caem; não é a GPU:
`PELICANO_GPU=0`/`nenhuma` dão igual). Mesmas bibliotecas da v0.2.2 (pelicano v0.1.19,
feather-ui v0.3.8); a Feather Sans (feather-ui 0.3.10) ficou para a próxima.

## v0.2.2 — 29/10/2026: menos memória e macros mais seguras

Colunas numéricas compactadas ao abrir (`ChunkedColumn::compactar`, `Bloco::Compacto`: base +
escala + 1/2/4 bytes, só quando volta bit a bit igual; blocos crus crescem de 64 a 65.536) e
`malloc_trim` (`paralelismo::devolver_memoria_ao_sistema`) depois de cada coluna, da abertura e dos
gráficos; rasters de gráfico só da aba aberta (`objetos::esquecer_rasteres` ao trocar). "Ativar
macros?" para todo arquivo com macro (`feathersheet-macros/src/riscos.rs`). Tetos de segurança
#123–#130; `MAX_OPERACOES` 4.096 no release (1.024 no debug, pilha menor); preencher/colar acima de
250 milhões de células avisa (`TETO_CELULAS_EM_BLOCO`). Mesmas bibliotecas da v0.2.1.

## v0.2.1 — 29/10/2026: correções

pelicano v0.1.19 (o evento usa o tamanho da janela que o recebe: a lista suspensa perto do
pé da janela perdia o clique, #122), feather-ui v0.3.8, feather-charts v0.3.4, todos por tag.
Salvar sem copiar as colunas de número, fórmula em milhões de linhas calculada em fundo
(`app/calculo_em_fundo.rs`), segmentação de dados ancorada na grade
(`grade_com_segmentacoes`). `FEATHERSHEET_CAPTURAR_FICAR=1` deixa a janela aberta depois
da foto para um `PELICANO_ROTEIRO` clicar (assim o #122 foi reproduzido).

## v0.2.0 — 27/10/2026: macros do Excel

- **Motor das macros: Falcão** (`feather-works/falcao`, tag v0.1.1; repositório próprio desde
  27/10, extraído de `FeatherOffice/falcao`), pelo feather-io v0.2.0 (opção `falcao`, padrão no
  `feathersheet-macros`; sem ela volta o Boa). Ponte direta Valor↔JSON, prazo e Parar
  (`Motor::interromper`), pilha com `api:L:C`/`macro:L:C` (a coluna do Falcão é a do começo da
  expressão; os testes de `posicoes.rs` marcam a diferença por `cfg!(feature = "falcao")`).
  Não tirar as checagens de limite da gaiola nem da pilha: são a segurança do FeatherWeb.
- **Conversor VBA**: matrizes com limite de baixo (`lb`), `Range.Value` em blocos, datas como
  série, `RemoveDuplicates`, `FormatConditions`, `On Error Resume Next`, várias áreas,
  `Worksheets.Add`; prelúdio `vba` com caminhos rápidos (cache de máscara, memo).
- **Macro do botão noutra thread** (`app/macro_dialogo/em_fundo.rs`): "Executando… N s" e Parar,
  prazo de 10 min; a tela usa um espelho vazio da pasta enquanto ela roda.
- **Escrita em lote** (macro de 200 mil linhas, `docs/planejamento/video-v0.2/Fechamento.bas`:
  30 s no Boa → 7,9 s no app; Excel 25 s): `Sheet::gravar_bloco_vazio`/`gravar_bloco_literais`
  (`Passo::Divisao`/`Passo::Regravacao`, só o que havia vai para o desfazer, análise do texto em
  paralelo por coluna), `formatar_faixa` (`Passo::Estilos`, só índices de estilo),
  `RemoveDuplicates` por permutação. Medir: `medir_fechamento` no `feathersheet-vba`
  (`FEATHERSHEET_XLSX` aceita `.xlsx` e `.fshz`; com LTO, como o app).

## v0.1.5 — 26/10/2026

Primeira versão só no pelicano depois do Iced apagado: formulários das macros
(e o UserForm do `.xlsm`, #67), `Módulo.função`, formatação condicional em
janela própria, e as correções da auditoria (#45–#64). Dependências: feather-io
v0.1.4 (prazo de 30 s nas macros), feather-charts v0.3.2, feather-ui v0.3.2,
pelicano v0.1.11.

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

24. **Toda funcionalidade nova ou mudada atualiza a Ajuda F1** (30/10/2026, pedido do
    usuário). Na mesma entrega: o tópico de "Como usar" (`ajuda.comoUsar` em
    `src/idiomas/pt-BR.json`, `en-US.json` e `es-ES.json`; lista em `TOPICOS` de
    `app/ajuda.rs`), com o caminho de menu conferido na faixa (`app/faixa_nativa_grupos.rs`).
    Função de fórmula vai no catálogo de funções; API de macro, em `ajuda_macros.rs`;
    atalho, em `ajuda_atalhos.rs`. Recurso que sai ou muda de lugar também sai/muda na ajuda.

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

### Lote de 10/11/2026 (depois da v0.3.0, entra na 0.4.0) — commitado em 11/11

Detalhe item a item em `docs/planejamento/ATIVIDADES-0.3.0.md` (linhas `Situação (10/11)`), `PARIDADE-EXCEL.md` e `MCP-PENDENTE.md`.

- **Francês** (`Lingua::Fr`, `src/idiomas/*.fr-FR.json`): quarta língua da interface. Texto novo sai em pt-BR, en-US, es-ES **e fr-FR**; o teste das novidades cobra as três traduções.
- **`.xlsx` com senha** (`feathersheet-io/src/xlsx_senha.rs`): abre Standard e Agile, grava Agile; Revisão › Proteger › Criptografar com senha. Falta conferir com arquivo cifrado pelo Excel de verdade.
- **Memória com teto nos testes** (`teste_memoria.rs`, `tetos_de_memoria.rs`): alocador de teste que recusa pedido acima do teto. Achou `Bloco::Constante` (fórmula até a linha 100 milhões: 403 MB → 43 KB por coluna) e a máscara de presença que cresce sob demanda.
- **Dinâmica**: itens agrupados à mão, itens calculados, subtotais em cima/embaixo, recolher/expandir nível, páginas do filtro, adiar atualização, origem em outra aba ou tabela, tema gravado no `.xlsx`.
- **Consolidar** (`operacoes/consolidar.rs`, `app/consolidar_ui.rs`), **referências 3D** (`Jan:Dez!B5`; renomear uma ponta reescreve a fórmula, `pontas_3d`; nome de aba não aceita `:`), **datas 1904** só na leitura do `.xlsx`.
- **Faixa**: fora da Página Inicial os grupos que não cabem viram um botão que abre o grupo (`plano_recolhido`).
- **Windows**: `.github/workflows/windows.yml` compila a cada push no `main`.
- **Senha e `.fshz` (11/11)**: com `senha_documento`, `formato_sugerido` dá `.xlsx` e o `Msg::Salvar` num caminho que não é `.xlsx/.xlsm` abre o Salvar como em vez de gravar em claro. Novidade: "Pasta com senha não é mais gravada em .fshz sem aviso: o Salvar sugere .xlsx."
- **Atualizar no Windows sem console (#159, 11/11)**: o `.ps1` roda com `CREATE_NO_WINDOW`; erro vira `MessageBox`, sem `Read-Host`. Só conferido por teste do texto do roteiro e pelo job de Windows no CI; falta ver numa máquina Windows. Novidade: "Atualizar no Windows não abre mais a janela do PowerShell."
- **Listas próprias da alça (11/11)**: Configurações ▸ Planilha ▸ "Listas personalizadas da alça" (nome trocado em 13/11; `config.listas_alca`, itens com `;`, listas com `|`; `Msg::ListasAlca`). São globais no motor (`ordenacao_ext::definir_listas_do_usuario`, como o idioma) e a `SerieDeLista` procura nelas depois das prontas; teste em `tests/lista_da_alca.rs` (arquivo à parte por ser global). Novidade: "Crie suas listas para a alça de preenchimento (P, M, G, GG) em Configurações ▸ Planilha."
- **Feather Sans, a letra da interface (11/11, PUB-4 a 8)**: `feather_ui::fonte::usar_na_interface()` no `main.rs`, antes de abrir a janela (feather-ui **v0.3.16**; pelicano **0.1.33** do crates.io, com `set_default_family` e `Font::SYSTEM_SANS`). A interface inteira usa a Feather Sans; **as células continuam com a letra do sistema** (`Some("sans-serif")` em `grade.rs` e `estilos_comuns.rs`, que o pelicano resolve para a sem serifa que o sistema tinha antes da troca), porque o texto da célula segue o arquivo, não a interface. A fonte é gerada por `feather-ui/assets/fontes/feather_sans.py` (precisa de `fonttools` e `shapely`); o D foi redesenhado a pedido do usuário (trecho reto e semicírculo, versão 1.001), só esse glifo mudou. Conferido por foto no Linux. **Sem conferir:** o Windows, e o Tab entre campos na grade (o pelicano 0.1.31 mudou isso; só os testes rodaram). Novidade: "Letra nova na interface: Feather Sans."
- **Cálculo iterativo (11/11, GRI-2)**: Configurações ▸ Planilha ▸ "Cálculo iterativo" (`config.calculo_iterativo`, `iteracoes_maximo` 100, `iteracoes_diferenca` 0,001; `Msg::CalculoIterativo`/`IteracoesMaximo`/`IteracoesDiferenca`, que chamam `calcular_agora`). Global no motor (`planilha::iterativo::definir`, como o idioma): desligado, o ciclo dá `#CIRCULAR!`; ligado, a célula a que a conta voltou devolve o valor da rodada anterior (`Sheet::valor_da_volta`, zero na primeira) e `fechar_ciclo` repete o ciclo até o máximo ou até mudar menos que a diferença. `CalcCache` guarda as cabeças do ciclo, quem calculou com o ciclo aberto (`no_ciclo`, sujado a cada iteração), as células de ciclo já vistas (`em_ciclo`, que o F9 e a troca da opção sujam por `sujar_ciclos`) e o último valor (`anterior`, porque sujar apaga o cache e o F9 continua de onde parou, como no Excel). Teste em `tests/iterativo.rs` (arquivo à parte e uma função só, por ser global). **Limites**: ciclo que passa por outra aba anda uma iteração por recálculo; ciclo mais comprido que a corrente adiada continua `#CIRCULAR!`; a opção não é lida nem gravada no `.xlsx` (`calcPr iterate`). Novidade: "Cálculo iterativo: fórmula que depende dela mesma repete a conta em vez de dar #CIRCULAR! (Configurações ▸ Planilha)."
- **Zoom na seleção (11/11)**: Exibir ▸ Zoom ▸ "Zoom na seleção" (`Msg::ZoomSelecao`, `Grade::zoom_para_area`: usa o `tamanho_canvas` do último desenho e o `Eixo::inicio`, sem percorrer linhas; respeita os limites de zoom das Configurações e leva a seleção ao canto). Novidade: "Zoom na seleção: a área selecionada passa a encher a tela."
- **Mesmo campo em Filtros e em Linhas/Colunas (#160, 11/11)**: arrastar da **lista** um campo que já está numa área o deixa nas duas (`por_em_posicao(.., manter)`, `pagina_tambem`); arrastar de uma área para outra e o "Mover para ›" continuam movendo. Novidade: "Tabela dinâmica: o mesmo campo pode ficar em Filtros e em Linhas ou Colunas, arrastando da lista."
- **Lista do filtro da dinâmica fecha ao clicar fora (#161, 11/11)**: área de clique atrás da lista em `dinamica_ui/fluxo.rs` manda `FiltroOk` (fica com o que foi marcado, porque a tabela já mostrou cada marcação). Sem teste automático: é só desenho; conferir na tela.
- **Faixa, acabamento (11/11, conferido por foto)**: o botão do grupo recolhido usa o ícone do primeiro botão grande do grupo (`notar_icone`/`guardar_icone_do_grupo`, anotado enquanto o grupo é montado; sem botão, `layout-grid`); com aba contextual em janela abaixo de 1300 px a busca encolhe para 92 px e o texto vira "Pesquisar", para o nome da aba (Tabela Dinâmica) não ficar cortado.
- **O nome é FeatherQuery, não OpenQuery (11/11, pedido enfático do usuário)**: todo texto visível (faixa, janela, mensagens, Ajuda, novidades, README, descrições do MCP, chaves de tradução) foi trocado. Ficaram com o nome antigo só os identificadores do código (`openquery_ui`, `Auxiliar::OpenQuery`, `Msg::OpenQuery`, `EventoOpenQuery`) e o id de ação `openOpenQuery`, que pode estar gravado nos atalhos de quem usa. As seções antigas deste arquivo e os planos em `docs/` ainda dizem OpenQuery: são registro histórico. Texto novo: sempre FeatherQuery.
- **Valor lógico na língua do app (depois da v0.3.1, 11/11; ideia do PR #158 de ash-171)**: a célula, a barra fx, o Mostrar fórmulas e a impressão mostram TRUE/FALSE, VERDADERO/FALSO, VRAI/FAUX. As palavras **vêm do dicionário de funções** (`nomes["TRUE"]`/`["FALSE"]` de `funcoes.<idioma>.json`, `idioma::logicos_de`), entregues ao motor por `locale::definir_logicos` em `idioma::definir`: língua nova não pede código novo. A entrada (`parse_literal` → `locale::logico_digitado`) aceita a palavra da língua em uso além de VERDADEIRO/TRUE, para F2+Enter não virar texto. **Dentro de fórmula também** (pedido do usuário no mesmo dia): `=A1&""`, CONCAT, ESQUERDA e as demais funções de texto usam `values::to_text_formula` (o `Evaluator::text`, o `&`, `fn_helpers::texto_de` e todo o `fn_text.rs`), e `to_boolean` aceita de volta a palavra da língua (`=SE("VRAI";…)`). O `to_text` continua em português **de propósito**: a coluna guarda o lógico por esse texto e os filtros/dinâmicas salvos usam essa chave; por isso a lista do AutoFiltro e os rótulos da dinâmica ainda mostram VERDADEIRO em outras línguas. Um arquivo salvo em português com `=A1&""` recalcula para TRUE quando aberto em inglês, como no Excel. Conferido por foto em francês. Novidade: "VERDADEIRO e FALSO aparecem na língua da interface (TRUE, VRAI…)."
- **Textos prontos do motor no dicionário (11/11, pedido do usuário: "todos os textos prontos devem ser extraídos")**: o motor não carrega dicionário (regra 11), então o app entrega o seu em `feathersheet_core::idioma::definir_tradutor` (chamado em `idioma::definir`), e todo texto pronto do core, io, macros e vba passa por `feathersheet_core::tr!("…")` / `trf!("… {x}")` (no core, `crate::tr!`). A chave é o texto em português, no mesmo `interface.<idioma>.json` da interface. Sem tradutor (testes, MCP avulso) sai o português. Foram ~690 mensagens: erros de fórmula e do leitor (`ParseError`), a tabela de explicações (`explicar::significado`), abrir/salvar arquivo, macros, e os motivos do conversor de VBA (`Motivo::novo`); o `feathersheet-vba` passou a depender do core fora dos testes. A explicação de erro reconhece o motivo por `idioma::veio_de(texto, modelo_pt)` (trechos fixos do modelo traduzido), não mais por palavra em português. **Regra nova: mensagem nova no motor nasce com `tr!`/`trf!`** — o teste `todo_texto_pronto_do_motor_esta_nos_dicionarios` (app) varre as fontes dos quatro crates e falha se a chave faltar em en, es ou fr. As 686 traduções vieram de agentes Haiku e foram conferidas por script (chaves e marcadores iguais), e depois **lidas uma a uma** (as 875 de todas as levas, 11/11): 34 corrigidas, quase todas nome de função que não batia com `funcoes.<idioma>.json` (ID.REGISTRO, IMPORTAR.DATOS, FONCTION.APPELANTE…), "pasta" traduzida como "dossier" em francês e concordância. Mensagem que cita função usa o nome de `nomes` da língua. **Segunda leva (mesmo dia)**: nomes de meses e dias pelo dicionário (`formato_personalizado` com `MESES_CURTOS`/`DIAS_CURTOS`, agrupar por mês na dinâmica, `consulta/query`, e os itens de `ordenacao_ext::listas_prontas` que a alça continua: em inglês a alça passa a continuar Jan → Feb); mensagens de expressão regular (`regex::erro`), do Preenchimento relâmpago (`ErroRelampago::mensagem` agora devolve `String`), funções sem serviço (cubo OLAP), fonte Web, ZIP, "há N minutos" dos recentes, "Página N de T" do PDF; e, traduzidos na tela pelo app com `tr!`, os rótulos das permissões de proteção e os nomes e descrições dos modelos. **Chave passada por variável (`tr!(x)`) não é vista pelo teste de guarda**, só `tr!("literal")`: nesses casos confira o dicionário à mão. **Terceira leva (mesmo dia)**: rótulos que o motor escreve na planilha — dinâmica ("Total Geral", "Total Geral ({nome})", "Rótulos de Linha", "{nome} Total", nome da aba "Dinâmica{n}") e Dados › Subtotal (uma frase inteira por função: `subtotais::rotulo_do_grupo`/`rotulo_geral`, porque a chave solta "Total" já existia com outro sentido). Quem compara pelo rótulo usa `dinamica::e_total_geral(texto)`, que aceita a língua do app **e** o português (dinâmica montada antes de trocar a língua fica com o rótulo antigo até atualizar): gráfico dinâmico, `graficos.rs` e a INFODADOSTABELADINÂMICA. Conferido por foto em inglês. **Cuidado com chave curta**: antes de criar uma chave de uma palavra, veja se ela já existe no dicionário com outro sentido. **Códigos de erro (mesmo dia, o usuário mandou fazer tudo)**: a célula, a barra fx, a impressão, a explicação de erro e o lógico-para-texto em fórmula mostram o código na língua (`#VALOR!`, `#NOME?`, `#N/D` em português; `#¡VALOR!` em espanhol; `#VALEUR!` em francês), por `ErrorCode::na_lingua()`. As palavras vêm do bloco `erros` de `funcoes.<idioma>.json` (inclusive o pt-BR), entregues por `locale::definir_codigos_de_erro`; `ErrorCode::from_code` aceita o código da língua em uso. `as_str()` e `to_text` continuam com o canônico em inglês (arquivo, chaves, comparações). **Última leva (11/11)**: "Série {n}", "Gráfico {n}" e "(vazio)" dos gráficos e consultas; rótulos das listas de filtro (dinâmica em `avancado::opcoes_de_filtro`, AutoFiltro de tabela em `rotulo_valor_lista`) — **a chave do filtro não muda**, só o que aparece; e as frases do roteiro do atualizador (`FRASES_DO_ROTEIRO` + `roteiro_na_lingua`, que tira aspas da tradução para não quebrar o bash nem o PowerShell; teste `roteiro_sai_na_lingua_do_app_sem_aspas_novas`). **Foto da última leva (11/11, francês)** achou títulos em maiúsculas escritos direto no código: FILTROS/COLUNAS/LINHAS/VALORES do painel da dinâmica (`dinamica_ui/fluxo.rs`, agora `tr!("Filtros").to_uppercase()`), COLAR (Colar especial), ESPECIAL (Ir para) e PERSONALIZADOS (estilos de tabela). O teste de guarda da interface não vê literal passado a um ajudante (`grupo("…")`, `area("…")`, `secao("…")`): título novo em maiúsculas leva `tr!` na chamada. **Varredura do mesmo dia**: mais 13 títulos (VALORES dos cenários; HORA/RESULTADO/MACRO/GATILHO do histórico de macros; FECHAR/CONSULTA/COLUNAS/LINHAS/COMBINAR/ORDENAR/CONSULTAS da faixa do FeatherQuery; "(Geral)" do editor de macros) e a mensagem "A aba não tem a forma" de `objetos.rs`. Para repetir: procurar literal com acento ou em maiúsculas em linha sem `tr!` cujo texto não é chave do dicionário. **Fica em português de propósito**: o conteúdo do modelo de queda de tensão (as fórmulas dele comparam esses textos), os comentários que o conversor de VBA escreve no JavaScript gerado (a primeira linha "// Convertida do VBA de…" é lida de volta para reconhecer o módulo), `bloco::texto_formatado` (alimenta a gravação do `.xlsx` e o Preenchimento relâmpago), e as respostas do MCP para a IA.
- **Vírgula de milhar ao digitar em inglês (12/11; ideia e casos de teste do PR #162 de ash-171)**: em inglês `1,234` virava 1,234 e `$1,234.50` virava 1,2345. A correção fica **só na digitação e na colagem de texto de fora** (`input::sem_milhar_digitado`, chamada em `formula_para_gravar` e no `colar_modo`): o separador de milhar da língua sai antes de o texto chegar ao `parse_literal`, nas quatro línguas: vírgula em inglês, ponto em português e espanhol, espaço em francês (comum ou o que não quebra, que vem na colagem) (`1.234` digitado em português passou a ser 1234, como no Excel; pedido do usuário em 12/11, "tem que ser igual no Excel"). Cada separador pede ao menos três dígitos depois dele (`1,2345` é 12345), e o agrupamento indiano continua aceito. **O `parse_literal` não pode mudar com a língua**: o `.fshz` guarda o texto digitado (avulsas e `textos`) e é relido por ele, e macros, MCP e CSV mandam número com ponto; o PR, que trocava o analisador, fazia `0.169` de arquivo antigo virar 169 e `1.5` virar texto em português. No analisador mudou uma regra, igual em toda língua: ponto sozinho que não é milhar é decimal na porcentagem (`15.5%` era 155%). **Auditoria do mesmo dia (cinco achados, todos corrigidos)**: (1) a barra de edição mostrava o número como guardado (`1234.5` em português, `2,575` em inglês) e editar um dígito caía na regra de milhar (1234.567 gravava 1234567): a barra mostra agora o decimal da língua (`input::numero_na_lingua`, no `recarregar_barra_fx`, só para célula de valor numérico; ordenar, aparar espaços, AutoSoma e Esc passaram a chamar o `recarregar_barra_fx` em vez de ler o `get_raw`), e o `formula_para_gravar` devolve o original também quando o texto é esse; (2) **colagem de fora lê cada coluna pelo decimal que ela mostra** (`input::decimal_da_coluna`: `3.14`, `0.5`, `12345.678`, `1,234.50` e `1,234,567` são ponto decimal; o espelho é vírgula): `3.142` ao lado de `3.14` não vira mais 3142; coluna sem sinal vale a língua do app; (3) **o CSV faz o mesmo** na abertura (`csv_colunas::linguas_da_amostra`, pelos primeiros 64 KiB; `"12,345"` numa coluna de ponto decimal é 12345, `1.234` numa de vírgula é 1234), nos dois caminhos (disco por colunas e bytes); coluna sem sinal é lida como sempre foi, e o caminho de separador fora do ASCII (só biblioteca) não tem a regra; (4) o `parse_number_str` consultava a língua do app para aceitar espaço como milhar, e o arquivo com o código `01 310` virava 1310 aberto em francês: o espaço comum não é mais milhar no analisador em língua nenhuma (o que não quebra continua valendo; em francês a digitação e a colagem tiram o espaço antes), então **número com espaço comum gravado como texto digitado em arquivo francês antigo reabre como texto**; (5) o percentual lê o número como o número puro (`1.234%` no analisador é 1,234%, antes 1234%; digitado em português continua 1234% pela regra de milhar). **A moeda ficou como era** (`R$ 1.234` e `$2.500` leem o ponto como milhar): mudar o analisador mudaria o valor de arquivo antigo com `R$ 1.234` guardado como texto digitado. **Limites** (a regra do Excel aqui é de memória, não foi conferida num Excel de verdade): em inglês `12,34` e `1,5` continuam número com a vírgula decimal (o Excel daria texto; não há como forçar texto na camada de digitação); em português `1.5` e `12.34` continuam decimais e `0.169` também (primeiro grupo "0" nunca é milhar); na digitação célula a célula `12345.678` em português é 12345678 (sem coluna para decidir). Localizar e substituir, preenchimento e formulário de dados não passam pela regra de milhar. Novidades: "O separador de milhar digitado ou colado é entendido como no Excel: 1,234 em inglês e 1.234 em português entram como mil duzentos e trinta e quatro." e "Dados colados ou abertos de CSV em formato americano (1,234.50) ou brasileiro (1.234,50) são lidos certo, coluna por coluna, em qualquer língua do app."
- **Auditoria de 11/11** (ponytail-audit no código novo): espaço comum só é milhar ao digitar na língua que separa assim (`01 310` fica texto em português; o espaço que não quebra vale sempre); rótulos do Consolidar por índice (`Rotulos`, antes busca linear).

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

- **Nas macros, o objeto de aba guarda o id, não o índice** (27/10/2026): `__sheet(i)` troca o
  índice pelo id (`idAba`) e o hospedeiro traduz de volta em cada chamada, porque
  `wb.addSheet`/`deleteSheet` mudam os índices. Quem guarda índice no `Estado` (`alteradas`,
  `aba_ativa`, `ativou`) anda junto no `deslocar_indices`; se a macro dá erro, `desfazer_estrutura`
  volta as abas (a apagada volta inteira, `Workbook::devolver_aba`) antes de reverter as transações.
- **Inserir/excluir linhas ou colunas esquece o histórico** das abas mexidas
  (`Sheet::esquecer_historico`, auditoria de 26/10/2026, #46): o deslocamento
  não é um passo do desfazer, e um Ctrl+Z velho apagava a célula errada. Quem
  reajusta tabelas/mesclas depois (`refluir`) esquece de novo. **O Ctrl+Z da
  operação (#65, 08/11/2026)** é do app (`app/desfazer_estrutura.rs`): antes, a
  pasta vira foto `.fshz` na memória e os históricos de todas as abas são
  tirados (`Sheet::tirar_historico`); o Ctrl+Z, quando a aba não tem mais nada
  no próprio histórico, devolve a foto (`Workbook::restaurar_foto`, mantém o
  `id` das abas) e os históricos. Recusa se outra aba mudou depois; o Ctrl+Y
  exige que nenhuma edição nova tenha entrado (`Sheet::edicoes_novas`). Novo
  lugar que insere/exclui: chamar `foto_antes_da_estrutura` antes e
  `guardar_foto_da_estrutura` depois. Pasta acima de 5 milhões de células na área
  usada não ganha foto (avisa). Medido: 200 mil linhas × 3, inserir 68 ms,
  desfazer 34 ms, foto 237 KB (`medir_foto_da_estrutura`, `--ignored`).
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
(endereço, `=`, conteúdo; itens separados por `|`). `FEATHERSHEET_CAPTURAR_MENU_CLASSICO=2` recolhe a faixa e abre esse menu clássico; `FEATHERSHEET_CAPTURAR_JANELA=condicional` cria uma regra de exemplo e fotografa a janela de formatação condicional; `=dividir` abre o Dividir texto na etapa do formato com 30 colunas e 3 a 8 selecionadas. `FEATHERSHEET_CAPTURAR_ABA=2` mostra essa aba (índice a partir de 0; só no app, útil com um arquivo na linha de comando). `FEATHERSHEET_CAPTURAR_CAIXAS=1` põe caixas de seleção (Inserir > Checkbox) na seleção de
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
`FEATHERSHEET_CAPTURAR_GRAFICO=painel:<tipo>` (ou `painel-fim:`/`personalizar:`; tipos `pizza`, `colunas`, `dispersao`,
`velas`, `gantt`, `linha3d`, `calor`, `histograma`, `bolhas`, `radar`, `cascata`…) monta um gráfico de exemplo e abre o
painel daquele tipo. `FEATHERSHEET_CAPTURAR_DINAMICA=filtro|menu|resumir` monta uma dinâmica de exemplo (STATUS em
Filtros e Linhas) e abre a lista do filtro de B1, o submenu "Mover para ›" ou o "Resumir por ›".

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
