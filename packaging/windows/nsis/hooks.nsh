; Tauri NSIS installer hooks (bundle.windows.nsis.installerHooks).
;
; Install: the Visual C++ runtime that the PHP and Apache builds need
;   (VS18 builds: 14.50 or newer), only when it is missing or older.
; Uninstall: stop Lumina and its services first, and ask whether to delete the
;   data folder (projects, databases, certificates). The answer defaults to
;   keeping it, and nothing is asked during updates or silent/passive runs.

!macro NSIS_HOOK_POSTINSTALL
  ; HKLM\SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64: Installed=1, Major.Minor >= 14.50
  SetRegView 64
  ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Installed"
  ReadRegDWORD $1 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Major"
  ReadRegDWORD $2 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Minor"
  SetRegView lastused
  StrCpy $3 "missing"
  ${If} $0 == 1
    ${If} $1 > 14
      StrCpy $3 "ok"
    ${ElseIf} $1 == 14
    ${AndIf} $2 >= 50
      StrCpy $3 "ok"
    ${EndIf}
  ${EndIf}
  ${If} $3 != "ok"
  ${AndIf} ${FileExists} "$INSTDIR\vcredist\vc_redist.x64.exe"
    DetailPrint "Installing the Microsoft Visual C++ runtime (found $1.$2)..."
    ; Asks for administrator rights itself; Lumina works per user.
    ExecWait '"$INSTDIR\vcredist\vc_redist.x64.exe" /install /quiet /norestart' $4
    DetailPrint "Visual C++ runtime installer finished with code $4"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; Stop the services and close Lumina, so no file is in use. Without a running
  ; Lumina this starts and exits at once.
  ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --quit'
  Sleep 1000

  ${If} $UpdateMode <> 1
  ${AndIf} $PassiveMode <> 1
    IfSilent lumina_keep_data
    MessageBox MB_YESNO|MB_ICONQUESTION|MB_DEFBUTTON2 \
      "Also delete your Lumina data folder?$\r$\n$\r$\nIt holds your projects (www), databases, certificates and settings. This cannot be undone.$\r$\n$\r$\nChoose No to keep it; a new installation will use it again." \
      /SD IDNO IDNO lumina_keep_data
    ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --remove-data'
  lumina_keep_data:
  ${EndIf}
!macroend
