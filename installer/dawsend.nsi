; Installateur Windows de Dawsend (NSIS 3).
;
; Construit par installer/build.sh (compilation croisée depuis Linux) ou
; installer/build.ps1 (sous Windows). Les fichiers à installer sont attendus dans
; installer/staging/ (voir ces scripts).

; Stub 64 bits quand NSIS le fournit (paquets Linux), sinon installateur 32 bits
; Unicode (NSIS officiel pour Windows) : il installe quand même dans les dossiers
; 64 bits ($PROGRAMFILES64, $COMMONFILES64, registre en vue 64 bits).
!if /FileExists "${NSISDIR}\Stubs\lzma_solid-amd64-unicode"
  Target amd64-unicode
!else
  Unicode true
!endif
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "Sections.nsh"

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
; Clé du runtime WebView2 (Edge), nécessaire à la fenêtre de l'appli.
!define WEBVIEW2_KEY "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1036 "ProductName" "Dawsend"
VIAddVersionKey /LANG=1036 "CompanyName" "Metaloul"
VIAddVersionKey /LANG=1036 "LegalCopyright" "Copyright Romain Rousseau (Metaloul)"
VIAddVersionKey /LANG=1036 "FileDescription" "Installateur de Dawsend"
VIAddVersionKey /LANG=1036 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "ProductVersion" "${VERSION}"

; ------------------------------------------------------------------ pages
!define MUI_ABORTWARNING
!define MUI_ICON "${STAGING}\dawsend.ico"
!define MUI_UNICON "${STAGING}\dawsend.ico"
!define MUI_WELCOMEPAGE_TITLE "Installation de Dawsend ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "Dawsend envoie le son de ton DAW (ASIO) vers une sortie Windows normale, pour que le partage d'écran (Discord…) l'entende.$\r$\n$\r$\nCet installateur va :$\r$\n  • installer le plugin Dawsend (VST3 et CLAP) dans les dossiers standard ;$\r$\n  • installer l'appli Dawsend et ses raccourcis.$\r$\n$\r$\nFerme Studio One (et tout autre DAW) avant de continuer."
!insertmacro MUI_PAGE_WELCOME
!define MUI_LICENSEPAGE_TEXT_TOP "Code source sous licence MIT. Le binaire VST3 est distribué sous GPLv3 (texte complet installé avec Dawsend)."
!insertmacro MUI_PAGE_LICENSE "${STAGING}\LICENSE.txt"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TEXT "Dawsend est installé.$\r$\n$\r$\n1. Ouvre Studio One et ajoute « Dawsend » en insert sur ta piste guitare (dernier de la chaîne).$\r$\n2. Dawsend tourne près de l'horloge (zone de notification) : clic sur son icône pour ouvrir la fenêtre.$\r$\n3. Dans Discord, partage l'écran entier avec le son.$\r$\n$\r$\nSi le plugin n'apparaît pas dans Studio One, relance une recherche des plug-ins."
!define MUI_FINISHPAGE_RUN "$INSTDIR\dawsend.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Lancer Dawsend maintenant"
!define MUI_FINISHPAGE_SHOWREADME "$INSTDIR\LISEZMOI.txt"
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Ouvrir le mode d'emploi"
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "French"

; ------------------------------------------------------------------ init
Var IsUpdate

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "Dawsend nécessite une version 64 bits de Windows."
    Abort
  ${EndIf}
  SetRegView 64
  ; Mise à jour lancée par l'appli (/S /UPDATE) : garder les choix de l'installation.
  ${GetParameters} $R0
  ClearErrors
  ${GetOptions} $R0 "/UPDATE" $R1
  ${IfNot} ${Errors}
    StrCpy $IsUpdate 1
    Call KeepPreviousChoices
  ${EndIf}
  ; Windows 11 et Windows 10 à jour ont WebView2 ; sinon, prévenir.
  ReadRegStr $0 HKLM "${WEBVIEW2_KEY}" "pv"
  ${If} $0 == ""
    ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${EndIf}
  ${If} $0 == ""
    ReadRegStr $0 HKCU "${WEBVIEW2_KEY}" "pv"
  ${EndIf}
  ${If} $0 == ""
    MessageBox MB_YESNO|MB_ICONEXCLAMATION "La fenêtre de Dawsend a besoin de Microsoft Edge WebView2, qui ne semble pas installé.$\r$\n$\r$\nInstalle-le depuis https://developer.microsoft.com/microsoft-edge/webview2/ (programme « Evergreen Bootstrapper »).$\r$\n$\r$\nContinuer quand même ?" IDYES +2
    Abort
  ${EndIf}
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

; ------------------------------------------------------------------ sections
Section "Appli Dawsend" SecApp
  SectionIn RO
  SetOutPath "$INSTDIR"
  ; En cas de mise à jour, l'appli ne doit pas tourner.
  nsExec::Exec 'taskkill /F /IM dawsend.exe'
  nsExec::Exec 'taskkill /F /IM dawsend-console.exe'
  File "${STAGING}\dawsend.exe"
  File "${STAGING}\dawsend-console.exe"
  File "${STAGING}\LICENSE.txt"
  File "${STAGING}\GPL-3.0.txt"
  File "${STAGING}\OFL-1.1.txt"
  File "${STAGING}\LICENCES.txt"
  File "${STAGING}\LISEZMOI.txt"
  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateDirectory "$SMPROGRAMS\Dawsend"
  CreateShortCut "$SMPROGRAMS\Dawsend\Dawsend.lnk" "$INSTDIR\dawsend.exe" "" "$INSTDIR\dawsend.exe" 0 SW_SHOWNORMAL "" "Joue le son des plugins Dawsend sur la sortie Windows"
  CreateShortCut "$SMPROGRAMS\Dawsend\Dawsend - liste des sources.lnk" "$WINDIR\System32\cmd.exe" '/k "$\"$INSTDIR\dawsend-console.exe$\" list"' "$INSTDIR\dawsend-console.exe" 0
  CreateShortCut "$SMPROGRAMS\Dawsend\Dawsend - bip de test.lnk" "$INSTDIR\dawsend-console.exe" "sine" "$INSTDIR\dawsend-console.exe" 0 SW_SHOWNORMAL "" "Publie un bip de test, comme le ferait le plugin"
  CreateShortCut "$SMPROGRAMS\Dawsend\Mode d'emploi.lnk" "$INSTDIR\LISEZMOI.txt"
  CreateShortCut "$SMPROGRAMS\Dawsend\Désinstaller Dawsend.lnk" "$INSTDIR\uninstall.exe"

  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "Dawsend"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "Metaloul"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\dawsend.exe"
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
  CreateShortCut "$DESKTOP\Dawsend.lnk" "$INSTDIR\dawsend.exe" "" "$INSTDIR\dawsend.exe" 0
SectionEnd

Section "Lancer Dawsend au démarrage de Windows" SecAutostart
  ; Même valeur que la case « Lancer avec Windows » de l'appli.
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Dawsend" '"$INSTDIR\dawsend.exe" --minimized'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecApp} "L'appli qui joue le son des plugins sur la sortie Windows. Elle se range dans la zone de notification."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecVst3} "Plugin VST3 dans C:\Program Files\Common Files\VST3 (Studio One, Cubase, Reaper…)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecClap} "Plugin CLAP dans C:\Program Files\Common Files\CLAP (Bitwig, Reaper, Studio One récent…)."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Raccourci « Dawsend » sur le bureau."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutostart} "Lance Dawsend à l'ouverture de session, réduit dans la zone de notification. Modifiable ensuite dans l'appli."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; Mise à jour : ne réinstalle que ce qui était installé, sans toucher au raccourci du
; bureau ni au démarrage avec Windows (réglé depuis l'appli).
Function KeepPreviousChoices
  !insertmacro UnselectSection ${SecAutostart}
  ${IfNot} ${FileExists} "$DESKTOP\Dawsend.lnk"
    !insertmacro UnselectSection ${SecDesktop}
  ${EndIf}
  ReadRegDWORD $0 HKLM "${UNINST_KEY}" "Clap"
  ${If} $0 != 1
    !insertmacro UnselectSection ${SecClap}
  ${EndIf}
  ReadRegDWORD $0 HKLM "${UNINST_KEY}" "Vst3"
  ${If} $0 != 1
    !insertmacro UnselectSection ${SecVst3}
  ${EndIf}
FunctionEnd

; Après une mise à jour, relancer l'appli avec les droits de l'utilisateur (pas ceux de
; l'installateur) : l'explorateur la démarre pour nous.
Function .onInstSuccess
  ${If} $IsUpdate == 1
    Exec '"$WINDIR\explorer.exe" "$INSTDIR\dawsend.exe"'
  ${EndIf}
FunctionEnd

; ------------------------------------------------------------------ désinstallation
Section "Uninstall"
  nsExec::Exec 'taskkill /F /IM dawsend.exe'
  nsExec::Exec 'taskkill /F /IM dawsend-console.exe'
  Delete "$COMMONFILES64\VST3\Dawsend.vst3\Contents\x86_64-win\Dawsend.vst3"
  RMDir "$COMMONFILES64\VST3\Dawsend.vst3\Contents\x86_64-win"
  RMDir "$COMMONFILES64\VST3\Dawsend.vst3\Contents"
  RMDir "$COMMONFILES64\VST3\Dawsend.vst3"
  Delete "$COMMONFILES64\CLAP\Dawsend.clap"

  Delete "$INSTDIR\dawsend.exe"
  Delete "$INSTDIR\dawsend-console.exe"
  Delete "$INSTDIR\LICENSE.txt"
  Delete "$INSTDIR\GPL-3.0.txt"
  Delete "$INSTDIR\OFL-1.1.txt"
  Delete "$INSTDIR\LICENCES.txt"
  Delete "$INSTDIR\LISEZMOI.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\Dawsend\*.lnk"
  RMDir "$SMPROGRAMS\Dawsend"
  Delete "$DESKTOP\Dawsend.lnk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Dawsend"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Dawsend"
  ; Les réglages (%APPDATA%\com.metaloul.dawsend) et les journaux
  ; (%LOCALAPPDATA%\com.metaloul.dawsend\logs) sont conservés.
  DeleteRegKey HKLM "${UNINST_KEY}"
SectionEnd
