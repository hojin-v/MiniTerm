# 기능·디자인 구현 결과

> 최신 로컬 빌드와 스크롤 검증: [Codex 스크롤 수정](SCROLL.md). 단축키 표는 [브랜드·단축키 후속 변경](KEYBOARD_BRAND.md)에 있다. 아래 초기 기능 및 8~11번 해시는 해당 시점의 기록이다.

> 8~11번 구현 당시 기록: [후속 구현 결과](FOLLOWUP.md). 완료·미완료 분류는 [feature.md](feature.md), 진행률 표시 방법은 [PROGRESS.md](PROGRESS.md)를 따른다.

> 후속 설정창 개선: [설정창과 즉시 적용](SETTINGS.md). 아래 실행 파일·해시는 첫 기능 구현 당시 기록이며, 후속 빌드와 구분한다.

2026-10-12 요청으로 구현을 재개했다. `feature.md`의 원문과 기존 작업 트리를 유지했다. 아래 변경은 로컬 개발 빌드에 적용하며 공개 alpha.6의 검증·배포 상태를 변경하지 않는다.

| 요구 | 변경 |
| --- | --- |
| 입력/요약 전환 | 글자 없이 입력 화면 아이콘과 체크리스트 아이콘을 전환한다. 요약 모드는 강조색으로 표시하고 현재 모드·클릭 결과는 호버로 설명한다. |
| 빠른 투명도 | 숨기기 클릭을 유지한다. 450ms 호버 시 창 위에 별도 네이티브 세로 슬라이더를 띄운다. `Ctrl+Alt+휠`은 5%씩 조절하며 범위는 0~80%다. 변경을 즉시 적용하고 저장하며 숨기기·종료 직전에도 저장한다. |
| exit·탭 종료 | 실제 프로세스 종료를 감지해 탭과 브로커 등록을 제거한다. 다른 탭의 세션 ID를 유지한다. 마지막 탭이 닫히면 HUD를 숨기고 트레이를 유지한다. 비정상 종료는 종료 코드를 안내한다. PTY 종료 메시지로 전송 스레드의 참조 순환도 해제한다. |
| 새 탭 단축키 | `Ctrl+Shift+T`로 새 탭을 연다. 이벤트를 먼저 소비해 Bash에 Ctrl+T가 전달되지 않도록 한다. 최대 4개와 시작 중인 탭의 기존 제한을 유지한다. |
| 브랜드 | 상단 MiniTerm 글자를 제거했다. 민트색 터미널과 위에 떠 있는 보라색 미니 창을 결합한 SVG를 사용한다. 상단·트레이·창·EXE와 NSIS 설치/제거 아이콘을 통일하고 설치 앱 목록의 DisplayIcon도 지정한다. 16~256px ICO를 빌드 시 재생성한다. |
| Codex 휠 | 대체 화면에서 방향키를 무조건 보내던 기본 동작을 제거했다. 마우스 보고를 요청한 프로그램에는 원래의 휠 이벤트를 전달한다. 별도 설정으로 방향키 전달을 켠 경우 DEC 1007과 애플리케이션 커서 모드를 따른다. |
| Git Bash의 WSL | `wsl`·`wsl.exe` 호출에만 MSYS 인수 경로 변환을 끈다. `/bin/bash`, `/tmp` 등이 Windows Git 경로로 바뀌던 실패를 재현하고 수정했다. 기존 사용자 함수/별칭과 다른 명령의 경로 변환은 보존한다. |

## 검증 범위

- 일반 자동검사 60개(단위 59개·fixture 1개) 통과. 환경/실기가 필요한 무시 검사는 전체 통과로 세지 않는다. 별도 Git/WSL 실기 7개는 위 범위에서 실행했다.
- 일반 자동검사 및 실제 egui 입력 회귀: 새 탭 조합이 PTY로 새지 않음, 투명도 조절과 본문 휠이 동시에 실행되지 않음, 일반 휠 보존, Codex 기본 대체 화면에서 방향키 미전송, DEC 모드·탭 선택 유지.
- Git Bash 실기: 로그인 파일·프롬프트 갱신·리터럴·포그라운드 입력, Windows Node/Codex 브리지의 TTY·색·CLI 버전, Unicode·파이프·Ctrl+C·Ctrl+Z·bg/fg·resize·같은 PID, `exit` 종료 감지.
- WSL2 Ubuntu-24.04 실기 3개: 홈·환경·TTY·Unicode·Windows interop·resize·잡 제어, 관리 프롬프트, 실제 Git Bash 안의 기본 `wsl`, Linux 절대 경로 인수를 포함한 `wsl` 및 `wsl.exe`, 기존 사용자 함수 보존.
- GUI: `scripts/verify-features.ps1`은 별도 데이터/IPC/프로세스에서 아이콘 전환, 투명도 팝업·네이티브 alpha·저장, 숨김 시 세션 유지, 타이핑한 exit, 마지막 탭의 트레이 유지, EXE 아이콘을 검사한다. 기존 사용자 MiniTerm 프로세스를 유지한다. 결과의 `newTabShortcut`·`modifierWheel`을 각각 읽어야 한다.
- 자동 키 입력에서 Windows가 Ctrl 상태를 유지하지 않아 물리 `Ctrl+Shift+T`·`Ctrl+Alt+휠`은 미검증이다. 이 경우 GUI 도구는 통과로 꾸미지 않고 개별 false 및 이유를 기록한다. egui 입력 소비 회귀는 별도로 통과한다.

실행 방법:

```powershell
./scripts/test.ps1
./scripts/test.ps1 -WslDistribution Ubuntu-24.04
./scripts/test.ps1 -WindowsBridge
./scripts/cargo.ps1 -CargoArguments @('test','--locked','--test','bash_conpty','git_session_exit','--','--ignored','--nocapture')
./scripts/build.ps1
./scripts/verify-features.ps1
./scripts/verify-ux.ps1 -SummarySample
```

## 호환성 한계와 다음 확인

- 지원 셸 프로필은 Git Bash와 WSL2 Bash다. WSL1, 별도의 PowerShell/CMD 프로필은 제공하지 않는다. 배포판 설치·전환·WSL 서비스 문제를 앱이 자동 복구하지 않는다. 기존 `wsl` 함수·별칭이나 절대 경로의 직접 EXE 호출은 사용자가 정한 동작을 유지한다.
- 기본 `wsl`은 수정 전에도 이 환경의 실기에서 통과했다. 확인한 실패는 Linux 인수의 MSYS 경로 변환이다. 오류 문구가 없는 사용자의 기본 호출 실패가 같은 원인이었다고 단정하지 않는다.
- 대체 화면에 스크롤백이 없으면 Shift 또는 기본 휠로 과거 출력이 생기지 않는다. Codex 기록을 보려면 `codex --no-alt-screen`을 사용한다. 기존 Windows Codex 브리지는 이 옵션을 기본 주입하지만 사용자 함수/별칭·직접 실행·WSL의 Codex에는 보장하지 않는다.
- 설치 Git의 빠른 resize 직후 입력 누락 4건은 기존 미해결이다. 이번 변경에서 해결됐다고 표시하지 않는다. 수정 MSYS DLL은 번들하지 않는다. 세부 근거는 `docs/SESSION_HANDOFF.md`에 유지한다.
- 다음 실기 확인은 실제 키보드의 두 조합과 사용자가 실패했던 정확한 기본 WSL 호출이다. 실제 모델·IME·혼합 DPI·WinGet·공개 릴리스 전체 검증은 이 작업의 통과 범위에 포함하지 않는다.

브랜드 원본은 `assets/miniterm.svg`, 픽셀 형상은 `assets/brand_pixels.rs`다. `scripts/generate-icon.ps1`로 ICO를 재생성한다. 두 형상을 함께 수정한다.

## 최종 로컬 산출물과 근거

- 실행 파일: `artifacts/miniterm.exe`; SHA256 `16ef8efc4b37b04b4cec8689096c0bd438e76c35f495c167740ccd7ff7601a54`.
- 소스 지문: `de049f31b0015d9bbb77b6f826ebecce5a407ca61c5ec17c1c11c08d64b110e3`; `artifacts/BINARY_BUILD_INFO.json`에 기록. Git `2.55.0.windows.5`, WSL2 `Ubuntu-24.04`로 실기 검사했다.
- 최종 GUI 기능: `artifacts/features-ec40b8cf-4656-4547-b93c-ed0c58941983/verification.json`. 아이콘 전환·팝업 조절/alpha/저장·exit·마지막 탭·사용자 앱 유지·EXE 아이콘 통과. 물리 Ctrl 조합 두 항목은 false다.
- 기존 UX 회귀: `artifacts/ux-0fd5ebed-3c64-424c-8775-3b2b7e78988d/verification.json`. 한 앱에서 3개 헤더 동작·요약/하단 배터리·툴팁 1개·다크/라이트·작은 창·폭/하단 유지·본문 위치별 더블 클릭·소유 셸 종료 통과.
- 설치 아이콘: `artifacts/branding-installer-ece2ca93-d2ea-4f09-b26d-9bc57ca9d5d7/verification.json`. NSIS 컴파일 및 설치 EXE에서 민트·보라색 브랜드 아이콘 추출을 확인했다. 컴파일 전용 fixture이며 설치 실행·게시·배포 인수 검사가 아니다.
- WSL 사전 재현 오류: `execvpe(C:/Program Files/Git/usr/bin/bash) failed: No such file or directory`. 수정 후 `/bin/bash`·`/bin/pwd`·`/tmp`와 기본 wsl 호출, 사용자 함수 보존 통과.
- 실패 GUI 시도도 각 `artifacts/features-*/verification.json`에 보존했다. 초기 창 준비·키 스캔 코드/포커스·Ctrl 상태·텍스트 입력 API·세션 목록의 UUID 정렬을 구분해 검증 도구를 보정했다. 실패를 제품 통과로 세지 않는다.

`artifacts/`는 Git 제외이므로 다른 clone에는 위 로컬 근거가 없다. 재검증 명령과 체크인된 SVG/ICO/검사 도구를 함께 전달한다.
