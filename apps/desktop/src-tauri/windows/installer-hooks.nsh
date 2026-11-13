; Claude Buddy additions to Tauri's NSIS installer (#31). Tauri inserts these
; macros into its template; `$UpdateMode` and `$DeleteAppDataCheckboxState`
; come from that template.

!macro NSIS_HOOK_PREUNINSTALL
  ; An update uninstalls the old version first: keep the hooks then.
  ${If} $UpdateMode <> 1
    ; Before the app is closed: `--uninstall-hooks` runs ahead of its
    ; single-instance check, so an open Claude Buddy does not get in the way.
    ${If} ${Cmd} `MessageBox MB_YESNO|MB_ICONQUESTION "Also remove Claude Buddy's hooks from your Claude Code settings?$\r$\n$\r$\nIf you keep them, Claude Code reports a hook error in every session until you remove them. A backup of the settings file is kept." /SD IDYES IDYES`
      ExecWait '"$INSTDIR\${MAINBINARYNAME}.exe" --uninstall-hooks' $0
      ${If} $0 <> 0
        MessageBox MB_OK|MB_ICONEXCLAMATION "Claude Buddy could not remove its hooks. Remove the entries that run buddy-hook.exe from your Claude Code settings.json by hand." /SD IDOK
      ${EndIf}
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; "Start with Windows" (src-tauri/src/win/autostart.rs): the `Run` value and
  ; the enabled/disabled flag Task Manager keeps under `StartupApproved\Run`.
  ; Tauri's template removes the first too; both are kept on updates.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Claude Buddy"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "Claude Buddy"
  ${EndIf}

  ; "Delete the application data" also removes our own folder
  ; (bridge.json, config.json), which is not under the bundle identifier.
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    RMDir /r "$APPDATA\claude-buddy"
  ${EndIf}
!macroend
