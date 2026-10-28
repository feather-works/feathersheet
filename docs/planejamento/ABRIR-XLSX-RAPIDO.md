# Abrir `.xlsx` grande rápido — plano e meta (28/10/2026)

Responsável: Claude (sessão de 28/10). Pedido do usuário: chegar perto do
`.fshz` (abre em menos de 1 s) também no `.xlsx`, **em máquina comum**, não só
nesta de 16 threads.

## Régua

Arquivo: `~/Documentos/FeatherSheet/FeatherSheet_teste_pesado_1M.xlsx`
(94 MB compactados, 668 MB de XML na aba `Dados`, 13 milhões de células,
1 milhão de `PROCX` numa tabela de 200 produtos).

Medir sempre com a bancada que já existe, que abre e calcula como o app e
confere valores de células conhecidas:

```bash
cd crates/feathersheet-io
cargo build --release --example medir_abrir_calcular
# 16 threads, 4 núcleos (notebook comum) e 2 núcleos (máquina simples):
for cpus in 0-15 0-3 0-1; do
  systemd-run --user --scope -q -p MemoryMax=8G /usr/bin/time -v \
    taskset -c $cpus ./target/release/examples/medir_abrir_calcular ARQUIVO.xlsx
done
```

Sem outro build rodando ao mesmo tempo (o número sai errado). Perfil:
`perf record -g` num build com `RUSTFLAGS="-C force-frame-pointers=yes"` e
`CARGO_TARGET_DIR` à parte (o perf daqui não desenrola pilha por DWARF).

## Onde estávamos e meta

| | Manhã de 28/10 | Depois do leitor rápido | Meta |
|---|---|---|---|
| 16 threads | 7,6 s (app) | 4,9 s (abrir 3,3 + calcular 1,6) | ~1 s |
| 4 núcleos | — | 9,1 s → **6,2 s** (núcleos por tamanho da aba) | 2–3 s |
| 2 núcleos | — | 11,2 s (abrir 7,6 + calcular 3,5) | 4–5 s |
| Pico de memória | 0,69 GB (app) | 0,72–0,82 GB | não passar de hoje |

O piso é descompactar o XML da aba grande (~0,6 s aqui): um fluxo DEFLATE só
não se divide entre núcleos.

## Passos (cada um medido e com commit próprio)

Primeiro o que corta trabalho (vale em qualquer máquina), depois paralelismo.

1. **Índice no `PROCX`/`PROCV`/`CORRESP` exatos** — hoje cada busca percorre a
   tabela (1 M buscas × 200 linhas). Índice montado uma vez por intervalo
   buscado e versão da aba, como o `agregado` do `SOMASES`
   (`planilha/agregado.rs`). Meta: calcular 2,0 s → < 0,3 s.
2. **Menos trabalho por célula na leitura** — o leitor rápido
   (`xlsx_read/xml/rapido.rs`) já sabe o tipo da célula; entregar número e
   texto prontos em vez de montar `String` e analisar de novo no preparo.
3. **Descompactar numa thread, analisar noutra** — feito (28/10): só quando a
   aba tem 4+ threads de preparo; 16 threads 3,35 → 3,05 s; com 4 núcleos
   ficaria pior, então lá não liga.
4. **XML em blocos paralelos** — a thread que descompacta corta em blocos de
   linhas inteiras; cada bloco é analisado numa thread. Fórmula compartilhada
   (`t="shared"`) resolvida em ordem depois.
5. **Gravar no motor por coluna em paralelo** — hoje uma thread só (2,8 s).

6. **Todas as fórmulas** (pedido do usuário, 28/10) — levantar as funções que
   fazem trabalho repetido por célula (varrem o intervalo a cada chamada, não
   param na área usada com coluna inteira, não usam o `agregado`) e medir por
   família numa planilha gerada de 1 M linhas. Otimizar pela ordem do tempo
   medido. Já feitos: `CONT.SES` de coluna inteira (para na área usada);
   busca do nome da função (`lookup`, mapa).

   Levantamento (28/10, pela leitura do código, falta medir por família):
   - **6a. Parar na área usada** (feito em 28/10/2026: `fn_helpers::linhas_com_conteudo`;
     100 mil linhas × 36 fórmulas de coluna inteira, 84,7 s → 51 ms) — o laço de SOMASE/SOMASES/CONT.SE/
     MÉDIASE(S)/MÁXIMOSES/MÍNIMOSES fora do `agregado` (critério `">0"`,
     `"<>x"`, curinga) vai até a linha 1.048.576; MÉDIASE ainda lê por
     `get_value` sem `LeitorFaixa`; CONTAR.VAZIO e SOMARPRODUTO percorrem o
     retângulo declarado. E o `Contexto` do cálculo paralelo
     (`calculo_paralelo.rs`) não tem `ultima_linha_usada` (o conserto do
     CONT.SES não vale lá).
   - **6b. Memorizar por (retângulo, geração)** — SOMA/MÉDIA/MÁXIMO/MÍNIMO/
     CONT.NÚM sobre coluna fixa (`=B2/SOMA(B:B)`) refazem 1 M por linha;
     ORDEM/MAIOR/MENOR/PERCENTIL/MED ordenam tudo por chamada (vetor ordenado
     guardado + busca binária).
   - **6c. Intervalo que cresce** (`SOMA($B$2:B2)`, `CONT.SE($A$2:A2;A2)`):
     O(n²) — soma de prefixos servida da faixa da coluna.
   - **6d. Volátil desliga os atalhos da aba inteira** (`HOJE()`, SUBTOTAL,
     DESLOC… em `sheet.rs:1346/1449`): invalidar por geração em vez de desligar.
   - **6e. PROCV/PROCH/CORRESP aproximados**: busca binária (o exato é o passo 1).
   - **6f. Custos fixos por célula**: `to_uppercase` do nome em todo `Call`
     (`evaluator.rs:301`), clone da árvore inteira por célula
     (`modelo_padrao.rs:96`), `padrao.to_string()` por célula no paralelo.
   - **Resultado errado** (antes da velocidade): #114 (`MATRIX_LIMIT` de 50 mil
     células vira só a 1ª célula, sem aviso) e #115 (CORREL e afins pareiam
     errado com vazios).

## Regras para mexer

- **Resultado idêntico.** O leitor rápido tem teste de comparação com o
  quick_xml (`leitor_rapido_das_linhas_igual_ao_quick_xml` e, nos arquivos,
  `leitor_rapido_igual_nos_arquivos` com `FEATHERSHEET_XLSX=a.xlsx,b.xlsx`);
  manter os dois passando. Função de busca nova compara com a varredura em
  teste (aproximada, curinga, duplicados, texto × número, maiúsculas).
- **Memória:** não passar do pico de hoje. Paralelismo com fila limitada.
- **Nada na thread da janela** (memória do projeto): o que for pesado roda
  fora dela.
- `feathersheet-core` sem dependência nova.
