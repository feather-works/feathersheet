#!/bin/bash
# Confere um .xlsm gravado pelo FeatherSheet sem Excel: o LibreOffice (snap) abre o arquivo, lista os módulos VBA
# que importou, roda a macro pedida e mostra as células A1:B6, D1, Z1:Z2. Perfil isolado em ~/snap/libreoffice/common.
# Uso: rodar.sh conferir.bas <arquivo.xlsm em ~/snap/libreoffice/common/…> <macro> [LANG, padrão pt_BR.UTF-8]
# Para gerar os .xlsm de prova: FEATHERSHEET_XLSM_SAIDA=<pasta> cargo test --bin feathersheet -- gerar_xlsm_para_conferir_fora --ignored
# Cuidado: no Basic do LibreOffice, "Lib" é palavra reservada; erro de compilação trava o modo sem tela.
C=$HOME/snap/libreoffice/common; P=$C/perfil-fs; D=$C/fs-conferir
mkdir -p $D
[ -d $P/user/basic/Standard ] || timeout 120 libreoffice -env:UserInstallation=file://$P --headless --terminate_after_init
parar() { for p in $(pidof soffice.bin); do grep -q perfil-fs /proc/$p/cmdline && kill $p; done; sleep 1; }
parar
python3 - "$P/user/basic/Standard/Module1.xba" "$1" <<'PY'
import sys,html
codigo=open(sys.argv[2]).read()
open(sys.argv[1],'w').write('<?xml version="1.0" encoding="UTF-8"?>\n<!DOCTYPE script:module PUBLIC "-//OpenOffice.org//DTD OfficeDocument 1.0//EN" "module.dtd">\n<script:module xmlns:script="http://openoffice.org/2000/script" script:name="Module1" script:language="StarBasic">\n'+html.escape(codigo,quote=True)+'</script:module>\n')
PY
rm -f $D/saida.txt
L=${4:-pt_BR.UTF-8}
LANG=$L LC_ALL=$L timeout 60 libreoffice -env:UserInstallation=file://$P --headless --norestore "macro:///Standard.Module1.Conferir(\"$2\",\"$D/saida.txt\",\"$3\")" >/dev/null 2>&1
echo "rc=$?"
parar
cat $D/saida.txt 2>/dev/null
