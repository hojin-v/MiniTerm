# -*- coding: utf-8 -*-
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"
!ifndef ARTIFACTS
  !define ARTIFACTS "..\artifacts"
!endif
!ifndef VERSION
  !define VERSION "0.1.0"
!endif
!ifndef RELEASE_LABEL
  !define RELEASE_LABEL "${VERSION}"
!endif
!ifndef RUNTIME_DLL_NAME
  !define RUNTIME_DLL_NAME "VCRUNTIME140.dll"
!endif
Name "MiniTerm ${RELEASE_LABEL}"
!define MUI_ICON "..\assets\miniterm.ico"
!define MUI_UNICON "..\assets\miniterm.ico"
OutFile "${ARTIFACTS}\MiniTerm-${RELEASE_LABEL}-windows-x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\MiniTerm"
InstallDirRegKey HKCU "Software\MiniTerm" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "MiniTerm"
VIAddVersionKey "CompanyName" "Hnode"
VIAddVersionKey "FileDescription" "MiniTerm per-user installer"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MiniTerm contributors"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Korean"
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  SetShellVarContext current
  SetRegView 64
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "MiniTerm은 Windows 11 x64가 필요합니다."
    SetErrorLevel 2
    Abort
  ${EndIf}
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  ${If} $0 < 22000
    MessageBox MB_OK|MB_ICONSTOP "MiniTerm은 Windows 11이 필요합니다."
    SetErrorLevel 2
    Abort
  ${EndIf}
  ${DisableX64FSRedirection}
  IfFileExists "$SYSDIR\${RUNTIME_DLL_NAME}" runtime_present runtime_missing
runtime_missing:
  ${EnableX64FSRedirection}
  IfSilent +2
  MessageBox MB_OK|MB_ICONSTOP "Microsoft Visual C++ v14 x64 런타임을 먼저 설치해 주세요. 공식 다운로드와 절차는 TESTER_GUIDE.md에 있습니다."
  SetErrorLevel 4
  Abort
runtime_present:
  ${EnableX64FSRedirection}
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=miniterm-guard.exe "${ARTIFACTS}\miniterm.exe"
  ExecWait '"$PLUGINSDIR\miniterm-guard.exe" --check-update-safe' $0
  ${If} $0 == 3
    IfSilent +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "세션을 마치고 MiniTerm을 종료한 뒤 설치해 주세요."
    SetErrorLevel 3
    Abort
  ${ElseIf} $0 != 0
    IfSilent +2
    MessageBox MB_OK|MB_ICONSTOP "MiniTerm 실행 검증에 실패했습니다. Microsoft Visual C++ v14 x64 런타임과 TESTER_GUIDE.md를 확인해 주세요."
    SetErrorLevel 5
    Abort
  ${EndIf}
FunctionEnd

Section "MiniTerm" Main
  SetOutPath "$INSTDIR"
  File "${ARTIFACTS}\miniterm.exe"
  File "..\LICENSE"
  File "${ARTIFACTS}\README.md"
  File "${ARTIFACTS}\TESTER_GUIDE.md"
  File "${ARTIFACTS}\RELEASE_NOTES.md"
  File "${ARTIFACTS}\STATUS.md"
  File "${ARTIFACTS}\COMPATIBILITY.json"
  File "${ARTIFACTS}\THIRD_PARTY_NOTICES.html"
  File "${ARTIFACTS}\BUILD_INFO.json"
  File /oname=SHA256SUMS "${ARTIFACTS}\COMPONENT_SHA256SUMS"
  File /oname=NSIS-LICENSE.txt "${NSISDIR}\COPYING"
  ExecWait '"$INSTDIR\miniterm.exe" --register' $0
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONEXCLAMATION "Windows 알림을 등록하지 못했습니다. MiniTerm 설정에서 다시 등록할 수 있습니다."
  ${EndIf}
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\MiniTerm" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "DisplayName" "MiniTerm"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "DisplayIcon" '"$INSTDIR\miniterm.exe",0'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "DisplayVersion" "${RELEASE_LABEL}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "Publisher" "Hnode"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "NoRepair" 1
SectionEnd

Function un.onInit
  SetShellVarContext current
  SetRegView 64
  ExecWait '"$INSTDIR\miniterm.exe" --check-update-safe' $0
  ${If} $0 != 0
    IfSilent +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "세션을 마치고 MiniTerm을 종료한 뒤 제거해 주세요."
    SetErrorLevel 3
    Abort
  ${EndIf}
FunctionEnd
Section "Uninstall"
  ExecWait '"$INSTDIR\miniterm.exe" --cleanup' $0
  ${If} $0 != 0
    IfSilent +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "변경된 에이전트 설정은 보존했습니다. 사용자 설정을 확인해 주세요."
  ${EndIf}
  Delete "$INSTDIR\miniterm.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\TESTER_GUIDE.md"
  Delete "$INSTDIR\RELEASE_NOTES.md"
  Delete "$INSTDIR\STATUS.md"
  Delete "$INSTDIR\COMPATIBILITY.json"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.html"
  Delete "$INSTDIR\NSIS-LICENSE.txt"
  Delete "$INSTDIR\BUILD_INFO.json"
  Delete "$INSTDIR\SHA256SUMS"
  Delete "$INSTDIR\Uninstall.exe"
  ; Non-recursive: never remove user-created files or the user's saved settings.
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm"
  DeleteRegKey HKCU "Software\MiniTerm"
SectionEnd
