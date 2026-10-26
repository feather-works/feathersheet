# FeatherSheet

**Planilha da suíte FeatherWorks, escrita em Rust com interface nativa no pelicano.**
Abre arquivos enormes, calcula em paralelo, fala português nas fórmulas e roda
inteira na sua máquina: sem conta, sem login, sem telemetria.

> **Estado:** em desenvolvimento ativo. Versão alpha prevista para **30/10/2026**.

![FeatherSheet com o modelo Fluxo de caixa e um gráfico de entradas e saídas, com a dica do mouse aberta em junho](docs/imagens/planilha-e-grafico.png)

## Baixar

[![Baixar para Windows](https://img.shields.io/badge/Windows-baixar-0078D6?style=for-the-badge&logo=windows11&logoColor=white)](https://github.com/feather-works/feathersheet/releases/latest/download/FeatherSheet-windows-x64.exe)
[![Baixar para Linux (.deb)](https://img.shields.io/badge/Linux%20.deb-baixar-E95420?style=for-the-badge&logo=ubuntu&logoColor=white)](https://github.com/feather-works/feathersheet/releases/latest/download/feathersheet-linux-amd64.deb)
[![Baixar para Linux (.tar.gz)](https://img.shields.io/badge/Linux%20.tar.gz-baixar-333333?style=for-the-badge&logo=linux&logoColor=white)](https://github.com/feather-works/feathersheet/releases/latest/download/feathersheet-linux-x86_64.tar.gz)
![macOS em breve](https://img.shields.io/badge/macOS-em%20breve-999999?style=for-the-badge&logo=apple&logoColor=white)

| Sistema | Arquivo | Como instalar |
|---|---|---|
| **Windows 10/11** | [FeatherSheet-windows-x64.exe](https://github.com/feather-works/feathersheet/releases/latest/download/FeatherSheet-windows-x64.exe) | abra o instalador (não pede administrador) |
| **Linux** (Ubuntu, Debian, Mint…) | [feathersheet-linux-amd64.deb](https://github.com/feather-works/feathersheet/releases/latest/download/feathersheet-linux-amd64.deb) | `sudo apt install ./feathersheet-linux-amd64.deb` |
| **Linux** (qualquer distribuição) | [feathersheet-linux-x86_64.tar.gz](https://github.com/feather-works/feathersheet/releases/latest/download/feathersheet-linux-x86_64.tar.gz) | extraia e rode `./instalar.sh` |
| **macOS** | — | ainda não disponível |

Todas as versões e as somas de verificação (`SHA256SUMS`) ficam na
[página de versões](https://github.com/feather-works/feathersheet/releases).
Para atualizar, baixe o arquivo novo e instale por cima: o `apt` e o
instalador do Windows substituem a versão anterior.

> A v0.0.1 para Linux exige Ubuntu 26.04 ou mais novo (glibc 2.43).

---

## Novidades da v0.1.3

![Janela "O que há de novo" da versão 0.1.3: vínculo com outro arquivo no .xlsx, fórmulas novas no Excel sem #NOME?, colar data até o fim da coluna, clique com a caneta da mesa, imagem entrando e saindo da célula, fórmulas corrigidas, macros com laço grande, borda do Ctrl+C animada e Formatar células mais compacto](docs/imagens/novidades-0.1.3.png)

A mesma janela abre sozinha na primeira vez depois de atualizar (e fica no
menu Ajuda, em *O que há de novo*).

## O que já faz

| Área | Destaques |
|---|---|
| **Fórmulas** | as 520 funções do Excel, com nomes em português ou inglês conforme o idioma da interface; avaliar fórmula passo a passo; matrizes dinâmicas, `LAMBDA`, voláteis |
| **Arquivos grandes** | até 100 milhões de linhas; abre `.parquet` e `.csv` grande; cálculo paralelo com invalidação seletiva; rodapé com soma e contagem instantâneas; colar um valor nas 100 milhões de linhas de uma coluna em 0,3 s; fórmula na coluna inteira de 13,7 milhões de linhas em milissegundos; tabela dinâmica em segundo plano, sem travar a janela |
| **Excel** | abre e grava `.xlsx` com cores do tema, formas, imagens, degradês, gráficos na posição certa, tabela dinâmica e vínculo com outro arquivo; lê `.xlsm` e converte as macros VBA para JavaScript |
| **Gráficos** | 25 tipos (combinado, cascata, radar, Gantt, mapa de calor…), dica do mouse e legenda clicável; gráficos editáveis com o painel *Formatar gráfico* (temas, cor por ponto, eixos, rótulos), pelo motor comum [`feather-charts`](https://github.com/feather-works/feather-charts) |
| **Dados** | tabelas, tabela dinâmica (mostrar valores como, gráfico dinâmico, segmentação, detalhes no duplo clique), formatação condicional, validação, agrupar/subtotal, proteger planilha, Texto para colunas |
| **Vínculos** | referência a outro arquivo (`=[Vendas.fshz]Dados!A1`) que fica viva com os dois abertos; aponte clicando na outra janela |
| **Macros** | editor Alt+F11 com macros em JavaScript (interpretador Boa, local) guardadas dentro do próprio arquivo |
| **IA local** | servidor MCP (`feathersheet --mcp`) para uma IA ler, escrever, ordenar, filtrar e conferir planilhas por stdio; liga e desliga em Configurações |
| **Desenho** | caneta com pressão (mesa digitalizadora) sobre a planilha |

### Gráficos

![Quatro gráficos do FeatherSheet: combinado (colunas de vendas com linhas de meta e média do setor), radar comparando dois produtos, cascata de receita e custos até o total, e mapa de calor por dia e hora](docs/imagens/galeria-graficos.png)

| | |
|---|---|
| ![Painel Formatar gráfico com os elementos analíticos: tendência linear com equação e R², linha de média](docs/imagens/formatar-grafico.png) | ![Gráfico com a série Saídas desligada pela legenda, o item hachurado e a dica mostrando a porcentagem](docs/imagens/legenda-clicavel.png) |
| **Formatar gráfico:** tendência (linear, exponencial, logarítmica, polinomial, potência, média móvel) com equação e R², barras de erro e linhas de máximo, mínimo e média | **Legenda clicável e dica do mouse:** clique num item para ocultar a série (fica hachurado); a dica mostra categoria, série, valor e porcentagem, e é personalizável |

## Desempenho: a Planilha Pesada

Para medir velocidade usamos um `.xlsx` gerado por script, com a cara de
uma planilha de vendas grande:

| Aba | Conteúdo | Fórmulas |
|---|---|---|
| **Dados** | 1.000.000 de linhas × 13 colunas: data, região, produto, quantidade, preço e 7 colunas calculadas (multiplicação, `SE` aninhado, `MÊS`, `PROCX` numa tabela de produtos e uma corrente em que cada linha soma a anterior) | 7.000.000 |
| **Produtos** | 200 produtos com `SOMASES`, `CONT.SES` e ticket médio sobre o milhão de linhas | 600 |
| **Resumo** | líquido e pedidos por região e mês (`SOMASES`/`CONT.SES` sobre 1 milhão de linhas), `SOMARPRODUTO`, `MÉDIASES` | 207 |
| **Total** | **13 milhões de células** | **7.000.807** |

O arquivo não traz os resultados guardados (`fullCalcOnLoad`): tudo é
calculado ao abrir.

| AMD Ryzen 7 5700G, 16 threads, 32 GB | v0.0.4 | v0.0.5 | v0.1.0 |
|---|---|---|---|
| Ler o `.xlsx` | 27,8 s | 7,4 s | **5,8 s** |
| Calcular as 7 milhões de fórmulas | 6,3 s | 2,4 s | **2,4 s** |
| Até a planilha pronta | ~34 s | 9,7 s | **8,2 s** |

A coluna da v0.0.4 vem do console do app (F12); as outras, da bancada
`medir_abrir_calcular` (o mesmo código de abrir e calcular, sem a janela;
mediana de 3 rodadas). Na v0.1.0 a fórmula repetida linha a linha
(`E2*F2`, `E3*F3`…) é traduzida do Excel uma vez só; num computador de
2 núcleos, ler esse `.xlsx` caiu de 33,7 s para 8,4 s.

**O mesmo arquivo salvo como `.fshz`**, o formato do FeatherSheet:

| v0.1.0, mesma máquina | `.xlsx` | `.fshz` |
|---|---|---|
| Tamanho do arquivo | 93,6 MB | **26,4 MB** |
| Ler o arquivo | 5,8 s | **0,5 s** |
| Calcular ao abrir | 2,4 s (7.000.799 fórmulas) | **0,6 s** (799 fórmulas) |
| Até a planilha pronta | 8,2 s | **1,1 s** |

O `.fshz` guarda as colunas já separadas por tipo e os resultados das
fórmulas: ao abrir, só os 799 resumos que somam o milhão de linhas
(`SOMASES`, `CONT.SES`) são refeitos. O `.xlsx` deste teste não traz
resultado nenhum, então tudo é calculado de novo.

**Comparando com outros programas** (mesmo arquivo, v0.1.1):

| | FeatherSheet (`.fshz`) | FeatherSheet (`.xlsx`) | Microsoft Excel ² | LibreOffice Calc 26.2 | OnlyOffice 9.4 | WPS Office 12.1 |
|---|---|---|---|---|---|---|
| Máquina | Ryzen 7 5700G, 32 GB | Ryzen 7 5700G, 32 GB | Core Ultra 7 155H, 64 GB (notebook) | Ryzen 7 5700G | Ryzen 7 5700G | Ryzen 7 5700G |
| Pronto para usar | **0,9 s** | **7,7 s** | 1 min 36,8 s | ~3 min 40 s | não abriu em 10 min | travou: mostra a planilha em ~50 s, mas não dá para usar e não calcula |
| Pico de RAM | 270 MB ⁴ | ~0,69 GB | 4,5 GB | 3,5 GB | 5,1 GB ¹ | 2,5 GB |
| RAM depois de pronto | **270 MB** | **362 MB** | 4,5 GB ³ | 3,5 GB | | |

¹ Feito sobre o Chromium, com vários processos; somar a RAM de cada um
conta duas vezes a memória que eles dividem, então o pico real é um pouco
menor.
² Cronometrado à mão em outra máquina: carregou em 1 min 24,9 s e
terminou de calcular as fórmulas em 1 min 36,8 s. Referência, não medida
exata.
³ Considerado igual ao pico (não medido depois de pronto).
⁴ Considerado igual à RAM depois de pronto (pico não medido).

Os tempos do FeatherSheet são os da barra de status do app; o do
LibreOffice foi visto na tela; a RAM dos outros é a soma de todos os
processos de cada programa. Uma medição por programa, sem repetir.

**Tabela dinâmica e colagem na aba Dados** (1 milhão de linhas, v0.1.0,
tempos do console F12):

| Operação | Tempo |
|---|---|
| Criar a dinâmica (nasce vazia, como no Excel) | instantâneo |
| Trocar um campo (1 campo em Linhas + 1 em Valores) | 130–205 ms |
| Trocar um campo (2 campos em Linhas + 1 em Valores)¹ | ~350 ms |
| Duplo clique num valor com 124 mil linhas de origem (Detalhes) | 353 ms |
| Colar um valor em A2:A100000000 (desfazer: 0,07 s) | 0,33 s |

¹ Da bancada `medir_etapas_da_troca` (o mesmo código, sem a janela); as outras linhas, do F12.

Para comparar, o **Microsoft Excel** abrindo o mesmo arquivo num notebook
com Intel Core Ultra 7 155H e 64 GB levou **1 min 24,9 s** para carregar e
terminou de calcular as fórmulas em **1 min 36,8 s** (cronometrado à mão).
As máquinas são diferentes, então a comparação é uma referência, não uma
medida exata.

Para gerar o arquivo e testar na sua máquina (Python 3, sem bibliotecas
externas; o segundo argumento, opcional, é o número de linhas):

```sh
python3 scripts/gerar_xlsx_pesado.py Planilha_Pesada.xlsx 1000000
```

## Paridade com o Excel

| O que se compara | Placar | Detalhe |
|---|---|---|
| **Funcionalidades** (comandos da faixa, botão por botão) | **211 / 290** (72,8%) | [`PARIDADE-EXCEL.md`](docs/planejamento/PARIDADE-EXCEL.md) · [avaliação geral](docs/planejamento/PARIDADE-AVALIACAO.md) |
| **Funções de planilha** | **520 / 520** | [catálogo das funções](docs/funcionalidades/FUNCOES.md) · [limites conhecidos](docs/planejamento/FUNCOES-FALTANTES.md) |
| **Funções do VBA** (conversor VBA → JavaScript) | **52** cobertas e **12** parciais, de **141** | [`PARIDADE-VBA-FUNCOES.md`](docs/planejamento/PARIDADE-VBA-FUNCOES.md) |

Os placares medem amplitude (o comando ou a função existe e funciona), não
fidelidade visual; cada documento explica o critério.

## Formato `.fshz`

O arquivo nativo é um contêiner compactado com **histórico Git embutido**
(feito com [`gix`](https://github.com/Byron/gitoxide), Rust puro): cada
gravação pode virar uma versão consultável, sem servidor nenhum.

## Estrutura

| Crate | Papel |
|---|---|
| [`feathersheet-core`](crates/feathersheet-core) | o motor: referências, valores, fórmulas e recálculo |
| [`feathersheet-io`](crates/feathersheet-io) | `.fshz`, `.xlsx`, `.xlsm`, `.csv` e `.parquet` |
| [`feathersheet-pelicano`](crates/feathersheet-pelicano) | a interface nativa (pelicano) |
| [`feathersheet-macros`](crates/feathersheet-macros) | macros JavaScript no interpretador Boa |
| [`feathersheet-vba`](crates/feathersheet-vba) | conversor de macros VBA para JavaScript |
| [`feathersheet-mcp`](crates/feathersheet-mcp) | servidor MCP para IA local |
| [`feathersheet-ponte`](crates/feathersheet-ponte) | camada de comandos da interface antiga (referência de paridade) |

Bibliotecas comuns da suíte usadas aqui:
[`feather-io`](https://github.com/feather-works/feather-io) (arquivos, tabelas, imagens e macros),
[`feather-ui`](https://github.com/feather-works/feather-ui) (tema, ícones e componentes),
[`feather-charts`](https://github.com/feather-works/feather-charts) (gráficos),
[`feather-draw`](https://github.com/feather-works/feather-draw) (desenho vetorial, SVG e PDF) e
[`feather-shapes`](https://github.com/feather-works/feather-shapes) (formas e ícones).

## Compilar

As bibliotecas da suíte vêm do GitHub por tag, pelo próprio Cargo: basta
clonar este repositório.

```sh
git clone https://github.com/feather-works/feathersheet
cd feathersheet/crates/feathersheet-pelicano
cargo run --release
```

No Linux, a janela precisa das bibliotecas do sistema (`libxkbcommon`,
`libwayland` ou `libx11`, `libfontconfig`). Comandos, testes e pendências da
interface estão em [Interface Pelicano](docs/INTERFACE-PELICANO.md).

## Privacidade

Tudo roda localmente. Nenhum conteúdo de planilha sai da máquina; não há conta,
login, telemetria nem servidor. Conectores que buscam dados de fora só agem por
pedido explícito e em mão única.

## Documentação

- [Roadmap até a 1.0](ROADMAP.md) — o que falta para ficar 100% funcional
- [Índice da documentação](docs/README.md)
- [Registro de tarefas](docs/REGISTRO-DE-TAREFAS.md) — histórico das entregas até a separação do repositório

## Licença

[GPL-3.0](LICENSE).
