Var BeaconKeepSetup
Var BeaconUnhookResult

!macro BeaconDefaultInstallDir
  ${If} $INSTDIR == "$LOCALAPPDATA\Beacon"
    StrCpy $INSTDIR "$LOCALAPPDATA\Beacon\app"
  ${EndIf}
!macroend

!define MUI_CUSTOMFUNCTION_GUIINIT BeaconGuiInit
Function BeaconGuiInit
  !insertmacro BeaconDefaultInstallDir
FunctionEnd

!macro BeaconStop
  ${If} ${FileExists} "$INSTDIR\beacon.exe"
    nsExec::Exec '"$INSTDIR\beacon.exe" stop'
    Pop $0
  ${EndIf}
  nsExec::Exec 'taskkill /F /IM beacon-app.exe'
  Pop $0
  Sleep 1000
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro BeaconDefaultInstallDir
  SetOutPath $INSTDIR
  !insertmacro BeaconStop
!macroend

!macro NSIS_HOOK_POSTINSTALL
  StrCpy $1 6
  ${IfNot} ${Silent}
  ${AndIf} $PassiveMode <> 1
    MessageBox MB_YESNO|MB_ICONQUESTION "Connect Beacon to Claude Code now? This adds Beacon's hooks to your Claude Code settings (a backup is kept)." IDYES +2
    StrCpy $1 7
  ${EndIf}
  ${If} $1 = 6
    nsExec::Exec '"$INSTDIR\beacon.exe" install --yes'
    Pop $0
    ${If} $0 <> 0
      MessageBox MB_OK|MB_ICONEXCLAMATION "Beacon could not be connected to Claude Code. Run 'beacon install' from a terminal, or use Install in Beacon's settings, to try again." /SD IDOK
    ${EndIf}
  ${EndIf}
  Exec '"$INSTDIR\beacon-app.exe" --background'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $BeaconKeepSetup 0
  ${If} $UpdateMode = 1
  ${OrIf} $EXEDIR == $INSTDIR
    StrCpy $BeaconKeepSetup 1
  ${EndIf}
  ${If} $BeaconKeepSetup = 0
    StrCpy $BeaconUnhookResult missing
    ${If} ${FileExists} "$INSTDIR\beacon.exe"
      nsExec::Exec '"$INSTDIR\beacon.exe" uninstall --yes'
      Pop $BeaconUnhookResult
    ${EndIf}
    ${If} $BeaconUnhookResult != 0
      MessageBox MB_OK|MB_ICONEXCLAMATION "Beacon's hooks could not be removed from your Claude Code settings file. Beacon's data in $LOCALAPPDATA\Beacon is kept, so beacon-hook.exe and the settings backups stay. Remove the beacon-hook.exe entries from the settings file's hooks, or reinstall Beacon and run 'beacon uninstall'." /SD IDOK
    ${EndIf}
  ${EndIf}
  !insertmacro BeaconStop
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $BeaconKeepSetup = 0
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Beacon"
    Delete "$LOCALAPPDATA\Beacon\autostart"
    ${If} $BeaconUnhookResult == 0
      MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 "Also delete Beacon's data (settings, state, logs and hook backups) in $LOCALAPPDATA\Beacon?" /SD IDNO IDNO +2
      RMDir /r "$LOCALAPPDATA\Beacon"
    ${EndIf}
  ${EndIf}
!macroend
