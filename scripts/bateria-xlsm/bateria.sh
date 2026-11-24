#!/bin/bash
# Bateria de .xlsm reais (#143): baixa os arquivos de fontes.txt (repositórios públicos do GitHub, "repo ramo caminho"),
# converte as macros de cada um com o exemplo converter_vba e resume: instruções convertidas, o que não converteu
# agrupado pelo motivo, e os arquivos que nem converteram. Só converte, não roda macro de terceiros.
# Uso: scripts/bateria-xlsm/bateria.sh [pasta, padrão /tmp/bateria-xlsm]
# shortcut: as fontes apontam para o ramo, não para um commit; se um repositório mudar, o placar muda junto.
set -u
AQUI=$(cd "$(dirname "$0")" && pwd)
RAIZ=$(cd "$AQUI/../.." && pwd)
D=${1:-/tmp/bateria-xlsm}
mkdir -p "$D"
n=0
while read -r repo ramo caminho; do
    n=$((n + 1))
    f="$D/$(printf %02d $n).xlsm"
    [ -s "$f" ] && continue
    url="https://raw.githubusercontent.com/$repo/$ramo/$(python3 -c 'import sys,urllib.parse;print(urllib.parse.quote(sys.argv[1]))' "$caminho")"
    curl -sfL --max-time 60 -o "$f" "$url" || echo "não baixou: $repo/$caminho"
done < "$AQUI/fontes.txt"
(cd "$RAIZ/crates/feathersheet-vba" && nice -n 19 cargo build -q --release --example converter_vba) || exit 1
X="$RAIZ/crates/feathersheet-vba/target/release/examples/converter_vba"
: > "$D/conversao.txt"
for f in "$D"/*.xlsm; do
    echo "== $(basename "$f")" >> "$D/conversao.txt"
    timeout 60 "$X" "$f" >> "$D/conversao.txt" 2>&1 || echo "FALHOU" >> "$D/conversao.txt"
done
python3 - "$D/conversao.txt" <<'PY'
import sys, re, collections
blocos = re.split(r'^== ', open(sys.argv[1]).read(), flags=re.M)[1:]
motivos = collections.Counter(); conv = pend = 0; falhas = []
for b in blocos:
    nome = b.split('\n')[0]
    erro = re.search(r'panicked at [^\n]*\n(.*)', b)
    if erro or 'FALHOU' in b:
        falhas.append(f"{nome}: {erro.group(1) if erro else 'FALHOU'}"); continue
    i = re.search(r'(\d+) instruções convertidas, (\d+) trechos', b)
    conv += int(i.group(1)); pend += int(i.group(2))
    for n, m in re.findall(r'^\s+(\d+) × (.*)$', b, flags=re.M): motivos[m] += int(n)
print(f"{len(blocos)} arquivos, {len(falhas)} não converteram; {conv} instruções convertidas, {pend} não "
      f"({100 * conv / max(conv + pend, 1):.1f}% convertido)")
for f in falhas: print("  NÃO CONVERTEU", f)
for m, n in motivos.most_common(30): print(f"{n:6}  {m}")
PY
