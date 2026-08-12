; Startup + instagram-windows:// protocol

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Instagram" '"$INSTDIR\Instagram.exe"'
  WriteRegStr HKCU "Software\Classes\instagram-windows" "" "URL:Instagram Windows"
  WriteRegStr HKCU "Software\Classes\instagram-windows" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\instagram-windows\shell\open\command" "" '"$INSTDIR\Instagram.exe" "%1"'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Instagram"
  DeleteRegKey HKCU "Software\Classes\instagram-windows"
!macroend
