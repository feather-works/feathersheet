# Auditoria de 13/11/2026

Três leituras em paralelo do código (só leitura, sem compilar): motor de fórmulas, arquivos
(.xlsx/.csv) e o app (desfazer, salvar, fechar, colar). Cada item tem um caso concreto.
✅ = corrigido e com teste; ⏳ = falta. A ordem dentro de cada bloco é a de gravidade.

## Motor de fórmulas (`feathersheet-core`)

Testes: `planilha/workbook/tests_auditoria.rs` (um por item, com o resultado do Excel em pt-BR).

- ✅ M1. Número vira texto com ponto dentro da fórmula (`="Peso: "&A1` dava "Peso: 1.5") e
  `VALOR` em inglês tirava o ponto ("1.5" virava 15). Agora 15 algarismos e o separador da língua.
- ✅ M2. Fórmula que aponta célula vazia valia "vazio": `MÉDIA` e `CONT.NÚM` de um resumo com
  `=A2` erravam. Agora vale 0, como no Excel (nos dois caminhos de cálculo e no paralelo).
- ✅ M3. Célula avulsa dentro de `SOMA`/`MÉDIA` (`SOMA(A1;B1)`) contava texto e lógico como
  número digitado. Agora conta como intervalo (a referência 3D segue pelo caminho antigo).
- ✅ M4. Curinga no critério: `"<>*abc*"` não usava o curinga; `"*"` contava número e célula
  vazia (`CONT.SE(A:A;"*")` contava a coluna inteira). Agora só texto casa com curinga.
- ✅ M5. Critério 0 casava com célula vazia (`CONT.SE(A1:A10;0)` dava 10). Também no agregado.
- ✅ M6. `SOMASE`, `SOMASES`, `MÉDIASE(S)`, `MÁXIMOSES` e `MÍNIMOSES` pulavam erro na faixa somada;
  agora o erro é o resultado (o agregado marca NaN e cai na varredura, que devolve o erro).
- ✅ M8. `SEERRO` sobre matriz (`SOMA(SEERRO(A1:A3/B1:B3;0))`) devolvia o erro inteiro.
- ✅ M10. `ÍNDICE(…;0;n)` dava #REF!; agora é a coluna (ou linha) inteira, até a última linha usada.
- ✅ M11. Datas: `FIMMÊS("2024-01-15";0)` e as outras davam #VALOR! com data em texto e com vazia;
  `NÚMSEMANA` ignorava o tipo (2 = semana começa na segunda; 21 = ISO).
- ✅ M12. `TEXTO` sem seções (`"0,00;(0,00)"` dava "-000.005)") e sem os 15 algarismos
  (`TEXTO(2,675;"0,00")` dava "2,67").
- ✅ M7. Copiar ou arrastar fórmula desloca coluna/linha inteira (`B:B` → `C:C`, `5:7` → `6:8`, com `$`):
  `formula_text::deslocar_inteiras`, no mesmo varredor do inserir/excluir (`mapear_inteiras`).
- ✅ M9. `PROCX` devolve a linha inteira do retorno de várias colunas (derrama; só essa forma de PROCX
  deixa de ir para a coluna empacotada), procura do fim (-1/-2) e com curinga (modo 2), pelo
  `posicao_xmatch`; modo inválido dá #VALOR!.
- Fica (anotado): `ANO(vazia)` dá 1899 e os seriais 1 a 60 diferem do Excel, porque o motor não
  tem o 29/02/1900 que não existiu. Mudar a numeração mexe na compatibilidade dos arquivos.
- Menores, não feitos: `DATA(24;1;1)` (o Excel lê 1924), `=""+1` (o Excel dá #VALOR!), critério
  `">=01/01/2024"` como data, `~~` no critério.

## Arquivos (`feathersheet-io`)

O Ctrl+S de um `.xlsx` grava por cima do próprio `.xlsx`: cada perda abaixo cai no arquivo original.

- ✅ A1. Formato de coluna inteira não ia para o `.xlsx` (moeda e negrito na coluna sumiam). Agora
  cada célula leva o estilo combinado e o `<col style>` vale nas vazias. Falta: estilo de linha
  inteira (`<row s>`) e da aba inteira nas células vazias.
- ✅ A2 (mínimo). Imagens, formas e caixas de texto lidas do `.xlsx` não são gravadas: agora contam
  no aviso, e o Ctrl+S pede "Salvar como" dizendo o que se perderia. Falta: gravar de verdade
  (`xl/media` e o desenho), e imagem colada no app também some ao gravar `.xlsx`.
- ✅ A3. Painéis congelados e linhas de grade ocultas agora vão para o `.xlsx` (`<sheetViews>`), no
  Ctrl+S e no Exportar. No `.fshz` eles continuam não sendo guardados (só na sessão).
- ✅ A4. Intervalo de coluna inteira saía com a linha 100.000.000 (formatação condicional, validação,
  nomes): agora para em 1.048.576 × XFD, o limite do Excel.
- ✅ A5. Nome de aba que o Excel recusa (`[ ] * ? / \`, apóstrofo na ponta, mais de 31 letras)
  agora é recusado ao renomear. Falta: arquivos antigos com esses nomes ao gravar.
- ✅ A6. Erros que o Excel não conhece (`#CIRCULAR!`, `#ERROR!`, `#CALCULANDO!`): vai só a fórmula,
  sem valor guardado (o Excel recalcula). Falta: fórmula que o leitor não entende ainda vai como texto.
- ✅ A7. CSV exportado com o valor cru (data virava 46327): número com formato escolhido sai como a
  célula mostra; sem formato, o número cru (reabre igual).
- ✅ A8. `_xHHHH_` do Excel (o `_x000D_` do Windows) agora é lido. Falta escrever (`_x005F_` antes de
  um `_x…_` literal e caractere de controle), raro.
- ✅ A9. Linha ou célula sem o atributo `r` (permitido pela norma) era pulada; agora segue a anterior
  (nos dois leitores).
- ✅ A10. Link para dentro da pasta (`location=`) é lido como `#Aba!A1` (nos dois leitores), gravado de
  volta no `location` e, no app, o clique vai até a célula (`link_ui::link_do_estilo`, `Msg::IrPara`).

## App (`feathersheet`)

- ✅ P1. Ctrl+Z logo depois de abrir outro arquivo (ou Novo) trazia o conteúdo do anterior, e o
  Ctrl+S gravava isso no arquivo novo. As pilhas de inserir/excluir linha são limpas ao trocar de pasta.
- ✅ P2. Salvar e rodar macro (ou cálculo em fundo) juntos: a resposta da gravação era barrada e o
  "salvando" ficava preso (não salvava mais e fechar esperava para sempre). Macro e cálculo agora
  esperam a gravação.
- ✅ P3. A cópia de recuperação durante macro ou cálculo saía com as abas vazias (o espelho).
- ✅ P4. Ctrl+C em coluna inteira, linha inteira ou Ctrl+A montava 100 milhões de células.
  Agora copia até a área usada.
- ✅ P5. "Excluir aba" apagava sem perguntar e sem Ctrl+Z. Aba com dados agora pede confirmação.
- ✅ P6. Ctrl+Z de inserir linha levava embora macro salva depois (e formulários, estilos, propriedades).
- ✅ P7. Renomear aba reescreve as fórmulas e a aba reescrita esquece o histórico (como inserir/excluir):
  o Ctrl+Z não volta mais o nome velho para dentro da fórmula.
- ✅ P8. Excluir aba com fórmula em edição fazia o Enter gravar na aba errada: agora pede para terminar antes.
- ✅ P9. Delete numa tabela ou AutoFiltro com filtro ativo pula as linhas escondidas
  (`tabelas::puladas_pelo_filtro`). Sem filtro ativo, linha oculta à mão é apagada, como no Excel.

Não lidos pelas auditorias: gráficos, dinâmica (além do fundo), editor de macros, FeatherQuery,
vínculos entre janelas, impressão, o formato `.fshz` em detalhe, estilos do leitor de `.xlsx`.
