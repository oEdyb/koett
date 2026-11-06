Unicode true
!include FileFunc.nsh
!include StrFunc.nsh
${StrStr}
Name "Koett"
OutFile "${OUTPUT}"
RequestExecutionLevel user
SilentInstall silent
Icon "${__FILEDIR__}\Koett.ico"
SetCompressor /SOLID zlib
!ifdef SIGN_LAUNCHER
!finalize '${SIGN_LAUNCHER} "%1"' = 0
!endif
Section
 ReadINIStr $0 "$EXEDIR\launch.ini" "Koett" "Current"
 StrLen $1 $0
 StrCmp $1 25 0 invalid
 StrCpy $1 $0 9
 StrCmp $1 "versions\" 0 invalid
 StrCpy $4 9
 validate:
 StrCpy $2 $0 1 $4
 ${StrStr} $3 "0123456789abcdef" "$2"
 StrCmp $3 "" invalid
 IntOp $4 $4 + 1
 IntCmp $4 25 launch validate invalid
 launch:
 IfFileExists "$EXEDIR\$0\Koett.exe" 0 invalid
 ${GetParameters} $1
 SetOutPath "$EXEDIR\$0"
 ClearErrors
 Exec '"$EXEDIR\$0\Koett.exe" $1'
 IfErrors invalid
 Quit
 invalid:
 MessageBox MB_OK|MB_ICONSTOP "Koett could not start. Run the setup again to repair the installation; your data is kept." /SD IDOK
 SetErrorLevel 1
SectionEnd
