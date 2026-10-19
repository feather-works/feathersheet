# Como contribuir

Obrigado pelo interesse! O FeatherSheet é escrito em Rust, com interface nativa em Iced.

## Defeitos e sugestões

Abra uma [issue](https://github.com/FeatherOffice/feathersheet/issues/new/choose) pelos modelos de **Defeito** ou **Sugestão**. Para defeito, conte os passos, a versão e o sistema, e cole o console (**F12**) se houver erro. Não anexe planilhas com dados pessoais.

## Compilar e testar

```sh
git clone https://github.com/FeatherOffice/feathersheet
cd feathersheet/crates/feathersheet-core
cargo test
cargo clippy --all-targets
```

O app: `cd crates/feathersheet-gui && cargo run --release`. Para medir velocidade sem esperar o build de lançamento, use o perfil `medir` (`--profile medir`).

## Regras do projeto

- **Português do Brasil** na interface, nos comentários e nas mensagens de erro. Texto novo da interface entra também nos arquivos de `src/idiomas/`.
- **O `feathersheet-core` não ganha dependência.** Ele compila em segundos e é isso que deixa rodar os testes a cada mudança. O que precisar de crate vai para `feathersheet-io` ou para a interface.
- **Nada de item de menu sem função.** Se não está implementado, não entra no menu.
- **Mudança de desempenho vem medida** — antes e depois, de preferência com um arquivo grande de verdade (`scripts/gerar_xlsx_pesado.py` gera um).
- Tela nova segue o padrão visual do que já existe e mira numa referência conhecida (Excel, Sheets).
- Commits pequenos, com mensagem em português explicando o porquê; a correção de uma issue fecha com `Fixes #N`.

## Licença

Ao contribuir, você concorda que sua contribuição seja distribuída sob a [GPL-3.0](LICENSE).
