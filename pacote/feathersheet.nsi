; Instalador do FeatherSheet para Windows (NSIS). Gerado por pacote/gerar.sh:
;   makensis -DVERSAO=0.0.1 -DEXE=caminho/feathersheet.exe -DSAIDA=caminho/instalador.exe pacote/feathersheet.nsi
; Instala por usuário (sem pedir administrador), cria atalhos, associa o .fshz
; e registra o desinstalador em "Aplicativos instalados".

Unicode true
!include "MUI2.nsh"

!ifndef VERSAO
  !define VERSAO "0.0.0"
!endif

Name "FeatherSheet ${VERSAO}"
OutFile "${SAIDA}"
InstallDir "$LOCALAPPDATA\Programs\FeatherSheet"
InstallDirRegKey HKCU "Software\FeatherWorks\FeatherSheet" "Pasta"
RequestExecutionLevel user
SetCompressor /SOLID lzma

!define MUI_ICON "icones\feathersheet.ico"
!define MUI_UNICON "icones\feathersheet.ico"
!define MUI_FINISHPAGE_RUN "$INSTDIR\feathersheet.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Abrir o FeatherSheet"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "PortugueseBR"

!define DESINSTALAR "Software\Microsoft\Windows\CurrentVersion\Uninstall\FeatherSheet"

Section "FeatherSheet"
  SetOutPath "$INSTDIR"
  File "/oname=feathersheet.exe" "${EXE}"
  File "icones\feathersheet.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortCut "$SMPROGRAMS\FeatherSheet.lnk" "$INSTDIR\feathersheet.exe" "" "$INSTDIR\feathersheet.ico"
  CreateShortCut "$DESKTOP\FeatherSheet.lnk" "$INSTDIR\feathersheet.exe" "" "$INSTDIR\feathersheet.ico"

  ; Planilha do FeatherSheet (.fshz): ícone e abrir com dois cliques.
  WriteRegStr HKCU "Software\Classes\.fshz" "" "FeatherSheet.Planilha"
  WriteRegStr HKCU "Software\Classes\FeatherSheet.Planilha" "" "Planilha do FeatherSheet"
  WriteRegStr HKCU "Software\Classes\FeatherSheet.Planilha\DefaultIcon" "" "$INSTDIR\feathersheet.ico"
  WriteRegStr HKCU "Software\Classes\FeatherSheet.Planilha\shell\open\command" "" '"$INSTDIR\feathersheet.exe" "%1"'
  ; "Abrir com" para .xlsx e .csv, sem tomar o lugar do programa padrão.
  WriteRegStr HKCU "Software\Classes\.xlsx\OpenWithProgids" "FeatherSheet.Planilha" ""
  WriteRegStr HKCU "Software\Classes\.csv\OpenWithProgids" "FeatherSheet.Planilha" ""

  WriteRegStr HKCU "Software\FeatherWorks\FeatherSheet" "Pasta" "$INSTDIR"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayName" "FeatherSheet"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKCU "${DESINSTALAR}" "Publisher" "FeatherWorks"
  WriteRegStr HKCU "${DESINSTALAR}" "DisplayIcon" "$INSTDIR\feathersheet.ico"
  WriteRegStr HKCU "${DESINSTALAR}" "URLInfoAbout" "https://github.com/feather-works/feathersheet"
  WriteRegStr HKCU "${DESINSTALAR}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegDWORD HKCU "${DESINSTALAR}" "NoModify" 1
  WriteRegDWORD HKCU "${DESINSTALAR}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\FeatherSheet.lnk"
  Delete "$DESKTOP\FeatherSheet.lnk"
  Delete "$INSTDIR\feathersheet.exe"
  Delete "$INSTDIR\feathersheet.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Classes\FeatherSheet.Planilha"
  DeleteRegValue HKCU "Software\Classes\.xlsx\OpenWithProgids" "FeatherSheet.Planilha"
  DeleteRegValue HKCU "Software\Classes\.csv\OpenWithProgids" "FeatherSheet.Planilha"
  ReadRegStr $0 HKCU "Software\Classes\.fshz" ""
  StrCmp $0 "FeatherSheet.Planilha" 0 +2
    DeleteRegKey HKCU "Software\Classes\.fshz"
  DeleteRegKey HKCU "Software\FeatherWorks\FeatherSheet"
  DeleteRegKey HKCU "${DESINSTALAR}"
SectionEnd
