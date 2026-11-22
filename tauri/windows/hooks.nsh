; NSIS installer hooks for the Krake setup (bundle.windows.nsis.installerHooks
; in tauri.conf.json). docs/specs/desktop-app.md, AC-32.
;
; A silent setup (/S, as winget runs it) is a scripted or managed install
; whose users should not meet the first-start update question, so it stores
; "install automatically" for the installing user, unless that user already
; has a desktop.json (an earlier choice, or an install being updated). An
; interactive or passive setup (/P, how the updater runs it) writes nothing:
; whoever ran it gets the question at the first start.
;
; The installer is per user (installMode currentUser), so $APPDATA is the
; user's roaming folder, where Tauri's app_data_dir() for the identifier
; com.amigo-labs.krake lies. DesktopState (crates/server/src/desktop/state.rs)
; fills every missing field with its default.

!macro NSIS_HOOK_POSTINSTALL
  IfSilent 0 krake_update_mode_done
  IfFileExists "$APPDATA\com.amigo-labs.krake\desktop.json" krake_update_mode_done
  Push $0
  CreateDirectory "$APPDATA\com.amigo-labs.krake"
  ClearErrors
  FileOpen $0 "$APPDATA\com.amigo-labs.krake\desktop.json" w
  IfErrors krake_update_mode_pop
  FileWrite $0 '{"update_mode":"auto"}'
  FileClose $0
  krake_update_mode_pop:
  Pop $0
  krake_update_mode_done:
!macroend
