# Documentação do FeatherSheet

Documentação técnica do FeatherSheet, organizada por assunto. **A interface ativa é Rust sobre pelicano + feather-ui** ([INTERFACE-PELICANO.md](INTERFACE-PELICANO.md)); o Iced e o frontend TypeScript foram aposentados, e os documentos de migração abaixo são históricos.

## Por onde começar

- [Histórico](planejamento/HISTORICO.md) — cada entrega, versão, medição e decisão; procure o recurso pelo nome.
- [Rumo à 0.4.0](planejamento/RUMO-0.4.0.md) — o que falta para a próxima versão.
- [Paridade com o Excel](planejamento/PARIDADE-EXCEL.md) — botão por botão da faixa (250 / 290 em 24/11/2026).
- [Arquitetura](arquitetura/ARQUITETURA.md) — responsabilidades dos componentes.
- [Documentação do funcionamento](arquitetura/DOCUMENTACAO.md) — fluxo de dados e comandos.
- [Roadmap até a 1.0](../ROADMAP.md) — o que falta para ficar 100% funcional.

## Índice por assunto

### Arquitetura

- [Arquitetura](arquitetura/ARQUITETURA.md)
- [Documentacao](arquitetura/DOCUMENTACAO.md)
- [Plano Backend](arquitetura/PLANO-BACKEND.md)

### Funcionalidades e motor

- [Criar Tabela Rust](funcionalidades/CRIAR-TABELA-RUST.md)
- [Csv E Operacoes Grandes](funcionalidades/CSV-E-OPERACOES-GRANDES.md)
- [Funcoes](funcionalidades/FUNCOES.md)
- [Graficos Nativos](funcionalidades/GRAFICOS-NATIVOS.md)
- [Openquery Rust Paridade](funcionalidades/OPENQUERY-RUST-PARIDADE.md)
- [Tabelas Ts Para Rust](funcionalidades/TABELAS-TS-PARA-RUST.md)
- [Xlsx Memoria Importacao](funcionalidades/XLSX-MEMORIA-IMPORTACAO.md)

### Interface (histórico da fase Iced)

- [Classificacao Pagina Rust](interface/CLASSIFICACAO-PAGINA-RUST.md)
- [Estatisticas Selecao Rust](interface/ESTATISTICAS-SELECAO-RUST.md)
- [Gui Divisorias Cabecalhos](interface/GUI-DIVISORIAS-CABECALHOS.md)
- [Pintar Linha Coluna](interface/PINTAR-LINHA-COLUNA.md)
- [Renomear Aba](interface/RENOMEAR-ABA.md)
- [Ribbon Fonte Posicionamento](interface/RIBBON-FONTE-POSICIONAMENTO.md)
- [Ribbon Legada Comandos Nativos](interface/RIBBON-LEGADA-COMANDOS-NATIVOS.md)
- [Tabelas Ribbon Contextual E Filtro](interface/TABELAS-RIBBON-CONTEXTUAL-E-FILTRO.md)

### Planejamento e paridade

- [Funcoes Faltantes](planejamento/FUNCOES-FALTANTES.md)
- [Paridade Avaliacao](planejamento/PARIDADE-AVALIACAO.md)
- [Paridade Excel](planejamento/PARIDADE-EXCEL.md)
- [Paridade das funções VBA](planejamento/PARIDADE-VBA-FUNCOES.md)
- [Auditoria de 13/11](planejamento/AUDITORIA-2026-11-13.md)
- [Pendências do MCP](planejamento/MCP-PENDENTE.md)
- [Metas por versão](planejamento/METAS-VERSOES.md)
- [Histórico](planejamento/HISTORICO.md)
- [Rumo à 0.4.0](planejamento/RUMO-0.4.0.md)
- [Roadmap](planejamento/ROADMAP.md)
- [Tarefas Gemini](planejamento/TAREFAS-GEMINI.md)

### Migração TypeScript → Rust

- [Auditoria Frontend Ts Rust 3apps](migracao/AUDITORIA-FRONTEND-TS-RUST-3APPS.md)
- [Menu Typescript Rust](migracao/MENU-TYPESCRIPT-RUST.md)
- [Plano Migracao Iced](migracao/PLANO-MIGRACAO-ICED.md)

### Migração — casos da interface e motor

- [Migracao Arraste Referencias Iced](migracao/rust-iced/MIGRACAO-ARRASTE-REFERENCIAS-ICED.md)
- [Migracao Buscas Worksheetfunction Rust](migracao/rust-iced/MIGRACAO-BUSCAS-WORKSHEETFUNCTION-RUST.md)
- [Migracao Politica Backup Rust](migracao/rust-iced/MIGRACAO-POLITICA-BACKUP-RUST.md)
- [Migracao Regiao Atual Iced](migracao/rust-iced/MIGRACAO-REGIAO-ATUAL-ICED.md)
- [Migracao Worksheetfunction Numericas](migracao/rust-iced/MIGRACAO-WORKSHEETFUNCTION-NUMERICAS.md)

### Migração — macros

- [Migracao Macros Api Formato Rust](migracao/macros/MIGRACAO-MACROS-API-FORMATO-RUST.md)
- [Migracao Macros Api Valores Rust](migracao/macros/MIGRACAO-MACROS-API-VALORES-RUST.md)
- [Migracao Macros Busca Rust](migracao/macros/MIGRACAO-MACROS-BUSCA-RUST.md)
- [Migracao Macros Criterios Rust](migracao/macros/MIGRACAO-MACROS-CRITERIOS-RUST.md)
- [Migracao Macros Enderecos Rust](migracao/macros/MIGRACAO-MACROS-ENDERECOS-RUST.md)
- [Migracao Macros Estatisticas Rust](migracao/macros/MIGRACAO-MACROS-ESTATISTICAS-RUST.md)
- [Migracao Macros Estrutura Rust](migracao/macros/MIGRACAO-MACROS-ESTRUTURA-RUST.md)
- [Migracao Macros Lote Aplicacao Mesclas Formulas Rust](migracao/macros/MIGRACAO-MACROS-LOTE-APLICACAO-MESCLAS-FORMULAS-RUST.md)
- [Migracao Macros Navegacao Rust](migracao/macros/MIGRACAO-MACROS-NAVEGACAO-RUST.md)
- [Migracao Macros Validacao Rust](migracao/macros/MIGRACAO-MACROS-VALIDACAO-RUST.md)
- [Migracao Recalculo Macro Rust](migracao/macros/MIGRACAO-RECALCULO-MACRO-RUST.md)
- [Migracao Status Erros Macros Rust](migracao/macros/MIGRACAO-STATUS-ERROS-MACROS-RUST.md)

## Convenções

- Consulte o código Rust antes de tratar uma checklist antiga como funcionalidade concluída.
- Documentos de migração registram decisões e verificações, não substituem os testes de compilação e paridade.
- [Licença](../LICENSE).
