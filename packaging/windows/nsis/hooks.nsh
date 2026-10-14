; Tauri NSIS installer hooks.
; Installs the Visual C++ runtime required by the bundled PHP/Apache builds.

!macro NSIS_HOOK_POSTINSTALL
  IfFileExists "$INSTDIR\resources\vcredist\vc_redist.x64.exe" 0 +2
    ExecWait '"$INSTDIR\resources\vcredist\vc_redist.x64.exe" /install /quiet /norestart'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Stop running services so no file is locked during uninstall.
  ExecWait '"$INSTDIR\lumina.exe" --stop-all'
!macroend
