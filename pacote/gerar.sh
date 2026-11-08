#!/usr/bin/env bash
# Gera os instaladores do FeatherSheet em pacote/saida/<versão>/:
#   feathersheet_<v>_amd64.deb         Linux (Debian, Ubuntu, Mint…)
#   feathersheet-<v>-linux-x86_64.tar.gz   Linux portátil (qualquer distribuição)
#   FeatherSheet-<v>-instalador.exe     Windows 10/11 (por usuário, sem administrador)
#   SHA256SUMS
#
# Uso: pacote/gerar.sh [linux|windows ...]   (sem argumentos = todos)
# Pacote de teste rápido: PERFIL=medir VERSAO=0.1.3~teste1 pacote/gerar.sh linux
# (sem LTO: ~1 min em vez de 6–10; VERSAO não sobrescreve a pasta de uma versão lançada).
# Precisa de: cargo, dpkg-deb, objdump; podman ou docker para o Linux (sem eles compila aqui, e o
# .deb só instala em distros com a glibc desta máquina); para Windows, o alvo x86_64-pc-windows-gnu
# (rustup target add x86_64-pc-windows-gnu), mingw-w64 e makensis.
set -euo pipefail

pacote="$(cd "$(dirname "$0")" && pwd)"
raiz="$(dirname "$pacote")"
# O aplicativo é o crate `crates/feathersheet` (interface no pelicano).
gui="$raiz/crates/feathersheet"
bin_nome="feathersheet"
alvo_dir="${CARGO_TARGET_DIR:-$gui/target}"
versao="${VERSAO:-$(sed -n 's/^version = "\(.*\)"/\1/p' "$gui/Cargo.toml" | head -1)}"
perfil="${PERFIL:-release}"
saida="$pacote/saida/$versao"
alvos=("$@")
[[ ${#alvos[@]} -eq 0 ]] && alvos=(linux windows)
mkdir -p "$saida"
echo "FeatherSheet $versao → $saida"

# O binário pede a glibc de onde foi compilado: num Debian 12 (2.36) ele roda no Ubuntu 22.04+,
# Debian 12+ e Mint 21+ (aqui seria a 2.43, só Ubuntu 26.04+). A imagem é a do FeatherNotes/FeatherDocs.
compilar_linux() {
  local motor; motor="$(command -v podman || command -v docker || true)"
  if [[ -z $motor || -n ${SEM_CONTEINER:-} ]]; then
    echo "Sem podman nem docker: compilando aqui (o .deb vai pedir a glibc desta máquina)." >&2
    cargo build --profile "$perfil" --manifest-path "$gui/Cargo.toml"
    bin="$alvo_dir/$perfil/$bin_nome"
    return
  fi
  if ! "$motor" image inspect feathernotes-debian12 >/dev/null 2>&1; then
    "$motor" build -t feathernotes-debian12 -f - "$pacote" <<'IMG'
FROM docker.io/library/debian:12
RUN apt-get update && apt-get install -y --no-install-recommends gcc libc6-dev curl ca-certificates git && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
IMG
  fi
  # Metade do processador (o resto da máquina continua usável) e teto de memória.
  "$motor" run --rm --memory 8g --cpus 8 -v "$raiz":/src:z -v feathernotes-registro:/root/.cargo/registry \
    -e CARGO_TARGET_DIR=/src/crates/feathersheet/target/debian12 -e CARGO_BUILD_JOBS=8 -w /src \
    feathernotes-debian12 cargo build --profile "$perfil" --manifest-path crates/feathersheet/Cargo.toml
  bin="$gui/target/debian12/$perfil/$bin_nome"
}

linux() {
  local bin; compilar_linux
  strip "$bin" 2>/dev/null || true
  # A glibc mínima é a maior versão que o binário usa.
  local glibc; glibc="$(objdump -T "$bin" | grep -o 'GLIBC_2\.[0-9]*' | sort -V | tail -1 | cut -d_ -f2)"
  echo "glibc mínima: $glibc"

  # .deb
  local d; d="$(mktemp -d)"
  install -Dm755 "$bin" "$d/usr/bin/feathersheet"
  install -Dm644 "$pacote/feathersheet.desktop" "$d/usr/share/applications/feathersheet.desktop"
  install -Dm644 "$pacote/feathersheet-mime.xml" "$d/usr/share/mime/packages/feathersheet.xml"
  for t in 16 24 32 48 64 128 256 512; do
    install -Dm644 "$pacote/icones/feathersheet-$t.png" "$d/usr/share/icons/hicolor/${t}x${t}/apps/feathersheet.png"
    install -Dm644 "$pacote/icones/feathersheet-$t.png" "$d/usr/share/icons/hicolor/${t}x${t}/mimetypes/application-x-feathersheet.png"
  done
  install -Dm644 "$pacote/icones/feathersheet.svg" "$d/usr/share/icons/hicolor/scalable/apps/feathersheet.svg"
  install -Dm644 "$raiz/LICENSE" "$d/usr/share/doc/feathersheet/copyright"
  local tamanho; tamanho="$(du -sk "$d/usr" | cut -f1)"
  mkdir -p "$d/DEBIAN"
  cat > "$d/DEBIAN/control" <<EOF
Package: feathersheet
Version: $versao
Section: office
Priority: optional
Architecture: amd64
Installed-Size: $tamanho
Depends: libc6 (>= $glibc), libxkbcommon0, libwayland-client0 | libx11-6, libfontconfig1
Recommends: libvulkan1 | libgl1
Maintainer: FeatherWorks <https://github.com/FeatherWorks>
Homepage: https://github.com/feather-works/feathersheet
Description: Planilha da suíte FeatherWorks
 Planilha em Rust com interface nativa: fórmulas em português, arquivos
 enormes, .xlsx, .csv e .parquet, macros em JavaScript. Roda inteira na
 máquina: sem conta, sem login, sem telemetria.
EOF
  cat > "$d/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
command -v update-mime-database >/dev/null && update-mime-database /usr/share/mime || true
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
EOF
  cp "$d/DEBIAN/postinst" "$d/DEBIAN/postrm"
  chmod 755 "$d/DEBIAN/postinst" "$d/DEBIAN/postrm"
  dpkg-deb --root-owner-group --build "$d" "$saida/feathersheet_${versao}_amd64.deb" >/dev/null
  rm -rf "$d"

  # .tar.gz portátil: o executável, o atalho, os ícones e um instalar.sh para a pasta do usuário.
  local t; t="$(mktemp -d)/feathersheet-$versao"
  mkdir -p "$t"
  cp "$bin" "$t/feathersheet"
  cp "$pacote/feathersheet.desktop" "$pacote/feathersheet-mime.xml" "$raiz/LICENSE" "$t/"
  cp -r "$pacote/icones" "$t/"
  cat > "$t/instalar.sh" <<'EOF'
#!/bin/sh
# Instala o FeatherSheet só para este usuário (~/.local), sem administrador.
set -e
cd "$(dirname "$0")"
mkdir -p ~/.local/bin ~/.local/share/applications ~/.local/share/mime/packages
install -m755 feathersheet ~/.local/bin/feathersheet
sed "s#^Exec=feathersheet#Exec=$HOME/.local/bin/feathersheet#" feathersheet.desktop > ~/.local/share/applications/feathersheet.desktop
cp feathersheet-mime.xml ~/.local/share/mime/packages/feathersheet.xml
for t in 16 24 32 48 64 128 256 512; do
  mkdir -p ~/.local/share/icons/hicolor/${t}x${t}/apps
  cp icones/feathersheet-$t.png ~/.local/share/icons/hicolor/${t}x${t}/apps/feathersheet.png
done
update-mime-database ~/.local/share/mime 2>/dev/null || true
update-desktop-database ~/.local/share/applications 2>/dev/null || true
echo "FeatherSheet instalado. Procure por \"FeatherSheet\" no menu de aplicativos."
EOF
  chmod 755 "$t/instalar.sh"
  tar -C "$(dirname "$t")" -czf "$saida/feathersheet-$versao-linux-x86_64.tar.gz" "feathersheet-$versao"
  rm -rf "$(dirname "$t")"
}

windows() {
  cargo build --release --target x86_64-pc-windows-gnu --manifest-path "$gui/Cargo.toml"
  local exe="$alvo_dir/x86_64-pc-windows-gnu/release/$bin_nome.exe"
  x86_64-w64-mingw32-strip "$exe" 2>/dev/null || true
  (cd "$pacote" && makensis -V2 -DVERSAO="$versao" -DEXE="$exe" -DSAIDA="$saida/FeatherSheet-$versao-instalador.exe" feathersheet.nsi)
}

for a in "${alvos[@]}"; do
  echo "== $a"
  "$a"
done
(cd "$saida" && sha256sum -- * | grep -v SHA256SUMS > SHA256SUMS)
ls -lh "$saida"
