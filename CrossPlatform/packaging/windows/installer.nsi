Unicode true
!include MUI2.nsh
!include WordFunc.nsh
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "Koett"
VIAddVersionKey /LANG=1033 "FileDescription" "Koett Setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "Dyberg & Co AB"
Name "Koett"
OutFile "${OUTPUT}"
RequestExecutionLevel user
InstallDir "$LOCALAPPDATA\Programs\Koett"
SetCompressor /SOLID zlib
ManifestDPIAware true
!define MUI_ABORTWARNING
!define MUI_ICON "${__FILEDIR__}/Koett.ico"
!define MUI_UNICON "${__FILEDIR__}/Koett.ico"
!define MUI_WELCOMEFINISHPAGE_BITMAP "${__FILEDIR__}/setup-sidebar.bmp"
!define MUI_WELCOMEPAGE_TITLE "Install Koett"
!define MUI_WELCOMEPAGE_TEXT "Close Koett before installing. Your settings and transcripts are kept."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\versions\${VERSIONKEY}\Koett.exe"
!insertmacro MUI_PAGE_FINISH
!define MUI_UNCONFIRMPAGE_TEXT_TOP "Koett will be removed. Your settings, transcripts and models are kept."
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!ifdef SIGN_UNINSTALLER
!uninstfinalize '${SIGN_UNINSTALLER} "%1"'
!endif
Var SetupMutex
!macro SetupGuard
 System::Call 'kernel32::CreateMutexW(p 0, i 0, w "Local\KoettSetup") p .r0 ?e'
 Pop $1
 StrCpy $SetupMutex $0
 StrCmp $0 0 setup_busy
 StrCmp $1 183 setup_busy setup_available
 setup_busy:
 MessageBox MB_OK|MB_ICONSTOP "Another Koett setup is running. Close it and retry." /SD IDOK
 SetErrorLevel 1
 Abort
 setup_available:
!macroend
Function .onInit
 !insertmacro SetupGuard
 SetShellVarContext current
 StrCpy $INSTDIR "$LOCALAPPDATA\Programs\Koett"
 ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "DisplayVersion"
 StrCmp $0 "" version_ok
 ${VersionCompare} "$0" "${VERSION}" $1
 StrCmp $1 "1" 0 version_ok
 MessageBox MB_OK|MB_ICONSTOP "A newer Koett version is installed. Use its installer or the explicit rollback command." /SD IDOK
 SetErrorLevel 1
 Abort
 version_ok:
FunctionEnd
Section
 InitPluginsDir
 SetOutPath "$PLUGINSDIR\payload"
 File /r "${PAYLOAD}/*"
 nsExec::ExecToStack /TIMEOUT=120000 '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\payload\manage-preview.ps1" -Action Install -Root "$INSTDIR" -Payload "$PLUGINSDIR\payload" -ShortcutName Koett'
 Pop $0
 Pop $1
 StrCmp $0 "0" install_ok
 MessageBox MB_OK|MB_ICONSTOP "Installation could not finish. Close Koett and retry.$\r$\n$1" /SD IDOK
 SetErrorLevel 1
 Abort
 install_ok:
 SetOutPath "$INSTDIR"
 File /oname=manage-install.ps1 "${PAYLOAD}/manage-preview.ps1"
 File /oname=Koett.ico "${__FILEDIR__}/Koett.ico"
 WriteUninstaller "$INSTDIR\Uninstall.exe"
 CreateShortcut "$SMPROGRAMS\Koett.lnk" "$INSTDIR\versions\${VERSIONKEY}\Koett.exe" "" "$INSTDIR\Koett.ico"
 WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "DisplayName" "Koett"
 WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "DisplayVersion" "${VERSION}"
 WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "Publisher" "Dyberg & Co AB"
 WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "InstallLocation" "$INSTDIR"
 WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "DisplayIcon" "$INSTDIR\Koett.ico"
 WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
 WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
 WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "NoModify" 1
 WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett" "NoRepair" 1
SectionEnd
Function un.onInit
 !insertmacro SetupGuard
 SetShellVarContext current
 StrCmp $INSTDIR "$LOCALAPPDATA\Programs\Koett" valid
 MessageBox MB_OK|MB_ICONSTOP "Run the uninstaller from the installed Koett directory."
 Abort
 valid:
FunctionEnd
Section Uninstall
 SetOutPath "$TEMP"
 nsExec::ExecToStack /TIMEOUT=120000 '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\manage-install.ps1" -Action Uninstall -Root "$INSTDIR" -ShortcutName Koett'
 Pop $0
 Pop $1
 StrCmp $0 "0" uninstall_ok
 MessageBox MB_OK|MB_ICONSTOP "Uninstall could not finish. Close Koett and retry.$\r$\n$1" /SD IDOK
 SetErrorLevel 1
 Abort
 uninstall_ok:
 DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Koett"
 Delete "$INSTDIR\Koett.ico"
 Delete "$INSTDIR\manage-install.ps1"
 Delete "$INSTDIR\Uninstall.exe"
 RMDir "$INSTDIR"
SectionEnd
