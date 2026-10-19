# Política de segurança

## Versões com correção

Correções de segurança saem na versão mais recente, publicada na [página de versões](https://github.com/feather-works/feathersheet/releases).

## Como relatar uma falha

**Não abra uma issue pública.** Use o relato privado do GitHub: aba **Security → Report a vulnerability** ([link direto](https://github.com/feather-works/feathersheet/security/advisories/new)).

Conte o que encontrou, como reproduzir e, se possível, um arquivo de exemplo **sem dados reais**. Exemplos do que interessa:

- arquivo `.fshz`, `.xlsx`, `.xlsm`, `.csv` ou `.parquet` que faz o app travar, consumir memória sem limite ou executar algo ao abrir;
- macro que escapa do interpretador e acessa o sistema;
- o canal MCP (`--mcp`) aceitando pedido de fora da máquina ou escrevendo com o modo somente leitura ligado.

Respondemos assim que possível e avisamos quando a correção for publicada.
