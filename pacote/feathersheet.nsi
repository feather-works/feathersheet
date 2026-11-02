; Instalador do FeatherSheet para Windows (NSIS). Gerado por pacote/gerar.sh:
;   makensis -DVERSAO=0.0.1 -DEXE=caminho/feathersheet.exe -DSAIDA=caminho/instalador.exe pacote/feathersheet.nsi
; Instala em Arquivos de Programas\FeatherWorks\FeatherSheet (pede administrador: a pasta é
; protegida, nenhum programa comum altera o .exe), para todos os usuários; cria atalhos, associa o
; .fshz e registra o desinstalador em "Programas e Recursos". Remove a instalação antiga por
; usuário (%LOCALAPPDATA%\Programs\FeatherSheet, até a v0.2.6).

Unicode true
!include "MUI2.nsh"

!ifndef VERSAO
  !define VERSAO "0.0.0"
!endif

Name "FeatherSheet ${VERSAO}"
OutFile "${SAIDA}"
InstallDir "$PROGRAMFILES64\FeatherWorks\FeatherSheet"
InstallDirRegKey HKLM "Software\FeatherWorks\FeatherSheet" "Pasta"
RequestExecutionLevel admin
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

!include "FileFunc.nsh"
!define DESINSTALAR "Software\Microsoft\Windows\CurrentVersion\Uninstall\FeatherSheet"

Function .onInit
  SetShellVarContext all
  SetRegView 64
FunctionEnd

Function un.onInit
  SetShellVarContext all
  SetRegView 64
FunctionEnd

Section "FeatherSheet"
  ; A instalação por usuário de antes (v0.2.6 e anteriores): sai, senão ficam duas.
  SetShellVarContext current
  IfFileExists "$LOCALAPPDATA\Programs\FeatherSheet\desinstalar.exe" 0 +2
    ExecWait '"$LOCALAPPDATA\Programs\FeatherSheet\desinstalar.exe" /S _?=$LOCALAPPDATA\Programs\FeatherSheet'
  Delete "$LOCALAPPDATA\Programs\FeatherSheet\desinstalar.exe"
  RMDir "$LOCALAPPDATA\Programs\FeatherSheet"
  SetShellVarContext all

  SetOutPath "$INSTDIR"
  File "/oname=feathersheet.exe" "${EXE}"
  File "icones\feathersheet.ico"
  File "/oname=LICENSE.txt" "..\LICENSE"
  WriteUninstaller "$INSTDIR\desinstalar.exe"

  CreateShortCut "$SMPROGRAMS\FeatherSheet.lnk" "$INSTDIR\feathersheet.exe" "" "$INSTDIR\feathersheet.ico"
  CreateShortCut "$DESKTOP\FeatherSheet.lnk" "$INSTDIR\feathersheet.exe" "" "$INSTDIR\feathersheet.ico"

  ; Planilha do FeatherSheet (.fshz): ícone e abrir com dois cliques.
  WriteRegStr HKLM "Software\Classes\.fshz" "" "FeatherSheet.Planilha"
  WriteRegStr HKLM "Software\Classes\FeatherSheet.Planilha" "" "Planilha do FeatherSheet"
  WriteRegStr HKLM "Software\Classes\FeatherSheet.Planilha\DefaultIcon" "" "$INSTDIR\feathersheet.ico"
  WriteRegStr HKLM "Software\Classes\FeatherSheet.Planilha\shell\open\command" "" '"$INSTDIR\feathersheet.exe" "%1"'
  ; "Abrir com" para .xlsx, .xlsm e .csv, sem tomar o lugar do programa padrão.
  WriteRegStr HKLM "Software\Classes\.xlsx\OpenWithProgids" "FeatherSheet.Planilha" ""
  WriteRegStr HKLM "Software\Classes\.xlsm\OpenWithProgids" "FeatherSheet.Planilha" ""
  WriteRegStr HKLM "Software\Classes\.csv\OpenWithProgids" "FeatherSheet.Planilha" ""

  WriteRegStr HKLM "Software\FeatherWorks\FeatherSheet" "Pasta" "$INSTDIR"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayName" "FeatherSheet"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayVersion" "${VERSAO}"
  WriteRegStr HKLM "${DESINSTALAR}" "Publisher" "FeatherWorks"
  ; Tamanho em "Programas e Recursos" (KB, do que foi instalado).
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${DESINSTALAR}" "EstimatedSize" "$0"
  WriteRegStr HKLM "${DESINSTALAR}" "DisplayIcon" "$INSTDIR\feathersheet.ico"
  WriteRegStr HKLM "${DESINSTALAR}" "URLInfoAbout" "https://github.com/feather-works/feathersheet"
  WriteRegStr HKLM "${DESINSTALAR}" "UninstallString" '"$INSTDIR\desinstalar.exe"'
  WriteRegDWORD HKLM "${DESINSTALAR}" "NoModify" 1
  WriteRegDWORD HKLM "${DESINSTALAR}" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$SMPROGRAMS\FeatherSheet.lnk"
  Delete "$DESKTOP\FeatherSheet.lnk"
  Delete "$INSTDIR\feathersheet.exe"
  Delete "$INSTDIR\feathersheet.ico"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\desinstalar.exe"
  RMDir "$INSTDIR"
  RMDir "$PROGRAMFILES64\FeatherWorks"
  DeleteRegKey HKLM "Software\Classes\FeatherSheet.Planilha"
  DeleteRegValue HKLM "Software\Classes\.xlsx\OpenWithProgids" "FeatherSheet.Planilha"
  DeleteRegValue HKLM "Software\Classes\.xlsm\OpenWithProgids" "FeatherSheet.Planilha"
  DeleteRegValue HKLM "Software\Classes\.csv\OpenWithProgids" "FeatherSheet.Planilha"
  ReadRegStr $0 HKLM "Software\Classes\.fshz" ""
  StrCmp $0 "FeatherSheet.Planilha" 0 +2
    DeleteRegKey HKLM "Software\Classes\.fshz"
  DeleteRegKey HKLM "Software\FeatherWorks\FeatherSheet"
  DeleteRegKey HKLM "${DESINSTALAR}"
SectionEnd
