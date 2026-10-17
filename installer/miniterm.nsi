# -*- coding: utf-8 -*-
Unicode true
!include "MUI2.nsh"
!include "x64.nsh"
!include "LogicLib.nsh"
!include "nsDialogs.nsh"
!include "FileFunc.nsh"
Var StartMenuChoice
Var DesktopChoice
Var AutostartChoice
Var StartMenuBox
Var DesktopBox
Var AutostartBox
Var OptionsDialog
Var FinishSummary
Var CommandParameters
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
Page custom OptionsPage OptionsLeave
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "MiniTerm 설치 완료"
!define MUI_FINISHPAGE_TEXT "$FinishSummary"
!define MUI_FINISHPAGE_RUN "$INSTDIR\miniterm.exe"
!define MUI_FINISHPAGE_RUN_TEXT "MiniTerm 지금 실행"
!define MUI_FINISHPAGE_LINK "설치 폴더 열기"
!define MUI_FINISHPAGE_LINK_LOCATION "$INSTDIR"
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Korean"
!insertmacro MUI_LANGUAGE "English"

!macro ReadChoice Option Variable
  ClearErrors
  ${GetOptions} $CommandParameters "${Option}" $0
  ${IfNot} ${Errors}
    ${If} $0 != "0"
    ${AndIf} $0 != "1"
      SetErrorLevel 2
      Abort
    ${EndIf}
    StrCpy ${Variable} $0
  ${EndIf}
!macroend

Function OptionsPage
  !insertmacro MUI_HEADER_TEXT "시작 및 바로가기 설정" "MiniTerm을 어디에서 열고 언제 시작할지 선택하세요."
  nsDialogs::Create 1018
  Pop $OptionsDialog
  ${If} $OptionsDialog == error
    Abort
  ${EndIf}
  ${NSD_CreateLabel} 0 0 100% 24u "설치가 끝나면 MiniTerm을 바로 실행할 수 있습니다. 이 옵션은 업데이트할 때도 기억됩니다."
  Pop $0
  ${NSD_CreateCheckbox} 0 36u 100% 14u "시작 메뉴에 MiniTerm 표시 (권장)"
  Pop $StartMenuBox
  ${NSD_SetState} $StartMenuBox $StartMenuChoice
  ${NSD_CreateCheckbox} 0 64u 100% 14u "바탕화면 바로가기 만들기"
  Pop $DesktopBox
  ${NSD_SetState} $DesktopBox $DesktopChoice
  ${NSD_CreateCheckbox} 0 92u 100% 14u "Windows에 로그인할 때 MiniTerm 실행"
  Pop $AutostartBox
  ${NSD_SetState} $AutostartBox $AutostartChoice
  ${NSD_CreateLabel} 0 118u 100% 24u "로그인 시 실행 설정은 MiniTerm 설정에서도 변경할 수 있습니다."
  Pop $0
  nsDialogs::Show
FunctionEnd

Function OptionsLeave
  ${NSD_GetState} $StartMenuBox $StartMenuChoice
  ${NSD_GetState} $DesktopBox $DesktopChoice
  ${NSD_GetState} $AutostartBox $AutostartChoice
FunctionEnd

Function .onInit
  SetShellVarContext current
  SetRegView 64
  ${GetParameters} $CommandParameters
  StrCpy $StartMenuChoice 1
  StrCpy $DesktopChoice 0
  StrCpy $AutostartChoice 0
  ClearErrors
  ReadRegDWORD $0 HKCU "Software\MiniTerm" "StartMenuShortcut"
  ${IfNot} ${Errors}
    StrCpy $StartMenuChoice $0
  ${EndIf}
  ClearErrors
  ReadRegDWORD $0 HKCU "Software\MiniTerm" "DesktopShortcut"
  ${IfNot} ${Errors}
    StrCpy $DesktopChoice $0
  ${Else}
    IfFileExists "$DESKTOP\MiniTerm.lnk" 0 +2
    StrCpy $DesktopChoice 1
  ${EndIf}
  ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "MiniTerm"
  ${If} $0 == '"$INSTDIR\miniterm.exe" --autostart'
    StrCpy $AutostartChoice 1
  ${EndIf}
  !insertmacro ReadChoice "/STARTMENU=" $StartMenuChoice
  !insertmacro ReadChoice "/DESKTOP=" $DesktopChoice
  !insertmacro ReadChoice "/AUTOSTART=" $AutostartChoice
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
check_running:
  ExecWait '"$PLUGINSDIR\miniterm-guard.exe" --check-update-safe' $0
  ${If} $0 == 3
    IfSilent running_abort
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "MiniTerm이 실행 중입니다.$\r$\n작업을 마치고 MiniTerm을 정상 종료한 뒤 '다시 시도'를 누르세요." IDRETRY check_running
running_abort:
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
  ; A prepared runtime is required by release packaging, with an explicit
  ; per-file removal list so user-created files are never recursively erased.
!ifdef BUNDLED_RUNTIME
  File "${ARTIFACTS}\BASH_RUNTIME_FILES.json"
  File "${ARTIFACTS}\BUNDLED_RUNTIME.md"
  SetOutPath "$INSTDIR\runtime\git"
  File /r "${ARTIFACTS}\runtime\git\*.*"
  CreateDirectory "$INSTDIR\runtime\git\dev\shm"
  CreateDirectory "$INSTDIR\runtime\git\dev\mqueue"
  SetOutPath "$INSTDIR"
!endif
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\MiniTerm" "InstallDir" "$INSTDIR"
  WriteRegDWORD HKCU "Software\MiniTerm" "StartMenuShortcut" $StartMenuChoice
  WriteRegDWORD HKCU "Software\MiniTerm" "DesktopShortcut" $DesktopChoice
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "DisplayName" "MiniTerm"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "DisplayIcon" '"$INSTDIR\miniterm.exe",0'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "DisplayVersion" "${RELEASE_LABEL}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "Publisher" "Hnode"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm" "NoRepair" 1
  CreateDirectory "$SMPROGRAMS"
  ExecWait '"$INSTDIR\miniterm.exe" --register' $0
  ${If} $0 != 0
    IfSilent +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "Windows 알림을 등록하지 못했습니다. MiniTerm 설정에서 다시 등록할 수 있습니다."
  ${EndIf}
  ${If} $StartMenuChoice == 0
    Delete "$SMPROGRAMS\MiniTerm.lnk"
  ${EndIf}
  ${If} $DesktopChoice == 1
    CreateDirectory "$DESKTOP"
    CreateShortCut "$DESKTOP\MiniTerm.lnk" "$INSTDIR\miniterm.exe" "" "$INSTDIR\miniterm.exe" 0 SW_SHOWNORMAL "" "MiniTerm · Bash HUD"
  ${Else}
    Delete "$DESKTOP\MiniTerm.lnk"
  ${EndIf}
save_startup:
  ExecWait '"$INSTDIR\miniterm.exe" --set-autostart $AutostartChoice' $0
  ${If} $0 != 0
    IfSilent autostart_error
    MessageBox MB_RETRYCANCEL|MB_ICONSTOP "시작 설정을 저장하지 못했습니다.$\r$\n사용자 설정과 기존 시작 항목을 확인한 뒤 다시 시도하세요. 취소하면 설치가 오류로 끝납니다. Windows 앱 설정에서 제거할 수 있습니다." IDRETRY save_startup
autostart_error:
    SetErrorLevel 6
    Abort
  ${EndIf}
  StrCpy $FinishSummary "MiniTerm ${RELEASE_LABEL} 설치가 완료되었습니다.$\r$\n$\r$\n설치 위치: $INSTDIR$\r$\n"
  ${If} $StartMenuChoice == 1
    StrCpy $FinishSummary "$FinishSummary$\r$\n• 시작 메뉴에서 MiniTerm을 열 수 있습니다."
  ${EndIf}
  ${If} $DesktopChoice == 1
    StrCpy $FinishSummary "$FinishSummary$\r$\n• 바탕화면 바로가기를 만들었습니다."
  ${EndIf}
  ${If} $AutostartChoice == 1
    StrCpy $FinishSummary "$FinishSummary$\r$\n• Windows 로그인 시 자동으로 실행합니다."
  ${EndIf}
  StrCpy $FinishSummary "$FinishSummary$\r$\n$\r$\n아래 'MiniTerm 지금 실행'을 선택하고 마침을 누르세요."
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
  Delete "$DESKTOP\MiniTerm.lnk"
  Delete "$SMPROGRAMS\MiniTerm.lnk"
!ifdef BUNDLED_RUNTIME
  !include "${ARTIFACTS}\runtime-delete.nsh"
  Delete "$INSTDIR\BASH_RUNTIME_FILES.json"
  Delete "$INSTDIR\BUNDLED_RUNTIME.md"
!endif
  Delete "$INSTDIR\Uninstall.exe"
  ; Non-recursive: never remove user-created files or the user's saved settings.
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MiniTerm"
  DeleteRegKey HKCU "Software\MiniTerm"
SectionEnd
