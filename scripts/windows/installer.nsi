Unicode true
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"
!include "WinVer.nsh"

!ifndef VERSION
  !error "Use scripts/package-windows-installer.py to build this installer."
!endif

Name "cppm ${VERSION}"
OutFile "${OUTPUT_PATH}"
InstallDir "$LOCALAPPDATA\Programs\cppm"
InstallDirRegKey HKCU "Software\cppm" "InstallDir"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "cppm"
VIAddVersionKey /LANG=1033 "FileDescription" "cppm setup with C++ build tools"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" "MIT - cppm contributors"
ShowInstDetails show
ShowUninstDetails show

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "Welcome to cppm ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "This wizard installs cppm and prepares Python, Conan, CMake, Ninja, Emscripten, Node.js, MSVC and the Windows SDK.$\r$\n$\r$\nAn Internet connection is required. The native and WebAssembly toolchains download several GB. If MSVC is missing, Microsoft Build Tools will request administrator approval.$\r$\n$\r$\nSetup configures your user PATH. Open a new terminal to start using cppm, including its WebAssembly template."
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${PROJECT_ROOT}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "cppm is installed"
!define MUI_FINISHPAGE_TEXT "Open a new terminal and run:$\r$\n$\r$\n  cppm init my-app$\r$\n  cd my-app$\r$\n  cppm run dev$\r$\n$\r$\nUse cppm doctor to check your tools. If Windows requests a restart, restart before building."
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "cppm setup requires 64-bit Windows 10 or later." /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "cppm setup requires Windows 10 or later." /SD IDOK
    SetErrorLevel 1
    Quit
  ${EndIf}
FunctionEnd

Section "cppm and C++ build tools" Main
  SectionIn RO
  SetShellVarContext current
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=cppm.exe "${BINARY_PATH}"
  SetOutPath "$INSTDIR\installer"
  File "${PROJECT_ROOT}\scripts\install.ps1"
  File "${PROJECT_ROOT}\scripts\uninstall.ps1"
  File "${PROJECT_ROOT}\scripts\windows-tools.ps1"
  File "${PROJECT_ROOT}\scripts\windows-tools.json"
  File "${PROJECT_ROOT}\scripts\setup-windows.ps1"
  File "${PROJECT_ROOT}\scripts\setup-wasm.py"
  SetOutPath "$INSTDIR\docs"
  File "${PROJECT_ROOT}\LICENSE"
  File "${PROJECT_ROOT}\THIRD_PARTY_NOTICES.txt"
  File "${PROJECT_ROOT}\docs\installation.md"
  File "${PROJECT_ROOT}\docs\windows.md"
  DetailPrint "Installing cppm and preparing its dependencies. This can take a while..."
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\installer\setup-windows.ps1" -BinaryPath "$PLUGINSDIR\cppm.exe" -InstallRoot "$INSTDIR"'
  Pop $0
  ${If} $0 == 3010
    SetRebootFlag true
    SetErrorLevel 3010
  ${ElseIf} $0 != 0
    MessageBox MB_ICONSTOP "Installation did not finish.$\r$\nSee $INSTDIR\install.log for details, then rerun setup.$\r$\nNo success was reported." /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  SetOutPath "$INSTDIR"
  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKCU "Software\cppm" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "DisplayName" "cppm"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "Publisher" "cppm contributors"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "UninstallString" '$\"$INSTDIR\uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "QuietUninstallString" '$\"$INSTDIR\uninstall.exe$\" /S'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "URLInfoAbout" "https://github.com/GaussNoob/cppm"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm" "NoRepair" 1
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  nsExec::ExecToLog '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$INSTDIR\installer\uninstall.ps1" -InstallDir "$INSTDIR\bin" -RemoveTools'
  Pop $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Uninstallation did not finish. Close cppm and its build tools, then try again." /SD IDOK
    SetErrorLevel 1
    Abort
  ${EndIf}
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\cppm"
  DeleteRegKey HKCU "Software\cppm"
  Delete "$INSTDIR\installer\install.ps1"
  Delete "$INSTDIR\installer\uninstall.ps1"
  Delete "$INSTDIR\installer\windows-tools.ps1"
  Delete "$INSTDIR\installer\windows-tools.json"
  Delete "$INSTDIR\installer\setup-windows.ps1"
  Delete "$INSTDIR\installer\setup-wasm.py"
  Delete "$INSTDIR\docs\LICENSE"
  Delete "$INSTDIR\docs\THIRD_PARTY_NOTICES.txt"
  Delete "$INSTDIR\docs\installation.md"
  Delete "$INSTDIR\docs\windows.md"
  Delete "$INSTDIR\install.log"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR\installer"
  RMDir "$INSTDIR\docs"
  RMDir "$INSTDIR\bin"
  RMDir "$INSTDIR"
SectionEnd
