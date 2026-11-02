# Publicar uma versão nova

Ordem: versão no código → instaladores → release no GitHub → Snap Store.
O snap baixa o `.deb` da release, então a release vem antes dele.
Exemplo abaixo: `0.2.2` → `0.2.3`.

## 1. Versão no código

- `crates/feathersheet/Cargo.toml` → `version = "0.2.3"`.
- `CLAUDE.md`: seção `## v0.2.3 — <data>` com o que mudou.
- Commit e tag:
  ```bash
  git commit -am "claude: v0.2.3 — <resumo>"
  git tag v0.2.3 && git push          # a tag fica só local por enquanto
  ```
  **Não enviar a tag antes dos instaladores** (02/11/2026): o launcher sem cota da API lê o
  `releases.atom`, que lista tag sem release; a tag nova sem `SHA256SUMS` fazia o FeatherSheet
  aparecer como "Ainda sem versão publicada". Enviar a tag (`git push origin v0.2.3`) logo antes
  do `gh release create` do passo 3.
  A tag dispara o CI do macOS, que põe o `.dmg` na release sozinho (se a release já existir;
  senão o `.dmg` fica nos artefatos do CI e vai à mão).

## 2. Instaladores (Linux e Windows)

Num `git worktree` limpo da tag (o crate usa o `feathersheet-core` por caminho e pegaria
mudança não commitada):

```bash
git worktree add /tmp/fs-v0.2.3 v0.2.3
systemd-run --user --scope -q -p MemoryMax=6G -p MemorySwapMax=0 nice -n 19 \
  /tmp/fs-v0.2.3/pacote/gerar.sh
```

Saída em `/tmp/fs-v0.2.3/pacote/saida/0.2.3/`.

## 2b. Antivírus antes de publicar

Enviar o `.exe` do Windows ao https://www.virustotal.com e só publicar com **0 ou 1** alerta de
heurística de baixa certeza (ex.: `Suspicious.low.ml.score`). Alerta de antivírus grande (Microsoft,
Kaspersky, ESET, Avast, Bitdefender) ou nome de família (`Trojan:…`) para tudo: achar o gatilho
(shell escondido, ver `FeatherOffice/docs/PADRAO-SISTEMA-OPERACIONAL.md`) ou mandar o falso positivo
ao fornecedor antes de lançar. O Defender na máquina ainda pode acusar por reputação (`!ml`, arquivo
novo sem assinatura) mesmo com o VirusTotal limpo: falso positivo em
https://www.microsoft.com/en-us/wdsi/filesubmission.

## 3. Release no GitHub

Os nomes na release são **sem versão** (o link "baixar a última" do README e o snap dependem disso):

```bash
s=/tmp/fs-v0.2.3/pacote/saida/0.2.3
cp $s/feathersheet_0.2.3_amd64.deb             /tmp/feathersheet-linux-amd64.deb
cp $s/feathersheet-0.2.3-linux-x86_64.tar.gz   /tmp/feathersheet-linux-x86_64.tar.gz
cp $s/FeatherSheet-0.2.3-instalador.exe        /tmp/FeatherSheet-windows-x64.exe
(cd /tmp && sha256sum feathersheet-linux-amd64.deb feathersheet-linux-x86_64.tar.gz FeatherSheet-windows-x64.exe > SHA256SUMS)
gh release create v0.2.3 --title "FeatherSheet v0.2.3 — <resumo em inglês>" --notes-file <notas.md> \
  /tmp/feathersheet-linux-amd64.deb /tmp/feathersheet-linux-x86_64.tar.gz \
  /tmp/FeatherSheet-windows-x64.exe /tmp/SHA256SUMS
git worktree remove /tmp/fs-v0.2.3
```

Notas da release em inglês, sem rodapé "Generated with Claude Code".

## 4. Snap Store

```bash
sed -i 's/0\.2\.2/0.2.3/g' snap/snapcraft.yaml   # version: e a URL do .deb
snapcraft pack
sudo snap install --dangerous feathersheet_0.2.3_amd64.snap && snap run feathersheet
snapcraft upload --release=stable feathersheet_0.2.3_amd64.snap
git commit -am "claude: snap 0.2.3"
```

- Testar antes com `--release=beta`, instalar com `sudo snap refresh feathersheet --beta`
  e promover sem recompilar: `snapcraft release feathersheet <revisão> stable`.
- Versão ruim: `snapcraft release feathersheet <revisão-anterior> stable`.
- Quem tem o snap atualiza sozinho em algumas horas.
- Login expirado: `snapcraft login`.

## Conferir no fim

- [ ] `gh release view v0.2.3` lista `.deb`, `.tar.gz`, `.exe`, `.dmg` e `SHA256SUMS`.
- [ ] `snap info feathersheet` mostra `latest/stable: 0.2.3`.
- [ ] A Central de Aplicativos acha "feathersheet" e mostra a versão nova.
