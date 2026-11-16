; Installateur Windows de Dawsend (NSIS 3).
;
; Construit par installer/build.sh (compilation croisée depuis Linux) ou
; installer/build.ps1 (sous Windows). Les fichiers à installer sont attendus dans
; installer/staging/ (voir ces scripts).

Target amd64-unicode
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!define STAGING "staging"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Dawsend"

Name "Dawsend ${VERSION}"
OutFile "..\target\installer\Dawsend-Setup-${VERSION}.exe"
InstallDir "$PROGRAMFILES64\Dawsend"
RequestExecutionLevel admin
SetCompressor /SOLID lzma
BrandingText "Dawsend ${VERSION}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1036 "ProductName" "Dawsend"
VIAddVersionKey /LANG=1036 "CompanyName" "Metaloul"
VIAddVersionKey /LANG=1036 "LegalCopyright" "Copyright Romain Rousseau (Metaloul)"
VIAddVersionKey /LANG=1036 "FileDescription" "Installateur de Dawsend"
VIAddVersionKey /LANG=1036 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "ProductVersion" "${VERSION}"

; ------------------------------------------------------------------ pages
!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Installation de Dawsend ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "Dawsend envoie le son de ton DAW (ASIO) vers une sortie Windows normale, pour que le partage d'écran (Discord…) l'entende.$\r$\n$\r$\nCet installateur va :$\r$\n  • installer le plugin Dawsend (VST3 et CLAP) dans les dossiers standard ;$\r$\n  • installer l'appli Dawsend et ses raccourcis.$\r$\n$\r$\nFerme Studio One (et tout autre DAW) avant de continuer."
!insertmacro MUI_PAGE_WELCOME
!define MUI_LICENSEPAGE_TEXT_TOP "Code source sous licence MIT. Le binaire VST3 est distribué sous GPLv3 (texte complet installé avec Dawsend)."
!insertmacro MUI_PAGE_LICENSE "${STAGING}\LICENSE.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "Dawsend est installé.$\r$\n$\r$\n1. Ouvre Studio One et ajoute « Dawsend » en insert sur ta piste guitare (dernier de la chaîne).$\r$\n2. Lance « Dawsend » depuis le menu Démarrer.$\r$\n3. Dans Discord, partage l'écran entier avec le son.$\r$\n$\r$\nSi le plugin n'apparaît pas dans Studio One, relance une recherche des plug-ins."
!define MUI_FINISHPAGE_RUN "$INSTDIR\dawsend-console.exe"
!define MUI_FINISHPAGE_RUN_PARAMETERS "play"
!define MUI_FINISHPAGE_RUN_TEXT "Lancer Dawsend maintenant"
!define MUI_FINISHPAGE_RUN_NOTCHECKED
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\LISEZMOI.txt"
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Ouvrir le mode d'emploi"
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "French"

; ------------------------------------------------------------------ init
Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "Dawsend nécessite une version 64 bits de Windows."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

; ------------------------------------------------------------------ sections
Section "Appli Dawsend" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  ; En cas de mise à jour, l'appli ne doit pas tourner.
  nsExec::Exec 'taskkill /F /IM dawsend-console.exe'
  File "${STAGING}\dawsend-console.exe"
  File "${STAGING}\LICENSE.txt"
  File "${STAGING}\GPL-3.0.txt"
  File "${STAGING}\LICENCES.txt"
  File "${STAGING}\LISEZMOI.txt"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\Dawsend"
  CreateShortCut "$SMPROGRAMS\Dawsend\Dawsend.lnk" "$INSTDIR\dawsend-console.exe" "play" "$INSTDIR\dawsend-console.exe" 0 SW_SHOWNORMAL "" "Joue la première source Dawsend sur la sortie Windows"
  CreateShortCut "$SMPROGRAMS\Dawsend\Dawsend - liste des sources.lnk" "$SYSDIR\cmd.exe" '/k "$\"$INSTDIR\dawsend-console.exe$\" list"' "$INSTDIR\dawsend-console.exe" 0
  CreateShortCut "$SMPROGRAMS\Dawsend\Dawsend - bip de test.lnk" "$INSTDIR\dawsend-console.exe" "sine" "$INSTDIR\dawsend-console.exe" 0 SW_SHOWNORMAL "" "Publie un bip de test, comme le ferait le plugin"
  CreateShortCut "$SMPROGRAMS\Dawsend\Mode d'emploi.lnk" "$INSTDIR\LISEZMOI.txt"
  CreateShortCut "$SMPROGRAMS\Dawsend\Désinstaller Dawsend.lnk" "$INSTDIR\uninstall.exe"

  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "Dawsend"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Metaloul"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\dawsend-console.exe"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKLM "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Plugin VST3" SecVst3
  SetOutPath "$COMMONFILES64\VST3\Dawsend.vst3\Contents\x86_64-win"
  File "${STAGING}\Dawsend.vst3\Contents\x86_64-win\Dawsend.vst3"
  WriteRegDWORD HKLM "${UNINST_KEY}" "Vst3" 1
SectionEnd

Section "Plugin CLAP" SecClap
  SetOutPath "$COMMONFILES64\CLAP"
  File "${STAGING}\Dawsend.clap"
  WriteRegDWORD HKLM "${UNINST_KEY}" "Clap" 1
SectionEnd

Section "Raccourci sur le bureau" SecDesktop
  CreateShortCut "$DESKTOP\Dawsend.lnk" "$INSTDIR\dawsend-console.exe" "play" "$INSTDIR\dawsend-console.exe" 0
SectionEnd

Section /o "Lancer Dawsend au démarrage de Windows" SecAutostart
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Dawsend" '"$INSTDIR\dawsend-console.exe" play'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "L'appli qui joue le son des plugins sur la sortie Windows (console pour l'instant)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecVst3} "Plugin VST3 dans C:\Program Files\Common Files\VST3 (Studio One, Cubase, Reaper…)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecClap} "Plugin CLAP dans C:\Program Files\Common Files\CLAP (Bitwig, Reaper, Studio One récent…)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Raccourci « Dawsend » sur le bureau."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutostart} "Ouvre Dawsend automatiquement à l'ouverture de session (une fenêtre de console reste ouverte)."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; ------------------------------------------------------------------ désinstallation
Section "Uninstall"
  nsExec::Exec 'taskkill /F /IM dawsend-console.exe'
  Delete "$COMMONFILES64\VST3\Dawsend.vst3\Contents\x86_64-win\Dawsend.vst3"
  RMDir "$COMMONFILES64\VST3\Dawsend.vst3\Contents\x86_64-win"
  RMDir "$COMMONFILES64\VST3\Dawsend.vst3\Contents"
  RMDir "$COMMONFILES64\VST3\Dawsend.vst3"
  Delete "$COMMONFILES64\CLAP\Dawsend.clap"

  Delete "$INSTDIR\dawsend-console.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\GPL-3.0.txt"
  Delete "$INSTDIR\LICENCES.txt"
  Delete "$INSTDIR\LISEZMOI.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\Dawsend\*.lnk"
  RMDir "$SMPROGRAMS\Dawsend"
  Delete "$DESKTOP\Dawsend.lnk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Dawsend"
  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
