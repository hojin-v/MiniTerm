# MiniTerm 구현 현황

**최신 상태 (2026-10-11):** 선택 줄의 한글 가림, AI Battery 기호 폴백, 라이트 모드 ANSI 검정/흰색 선택 영역, 미완료 동기화 프레임과 새 Codex 세션의 알림 helper 선택을 보완했다. 로직 38개와 실제 Codex 메뉴·정지된 목표 재개 메뉴·완료 이벤트의 별도 시험이 통과한다. 실제 Git Bash 크기 변경 뒤 첫 글자 누락 시험은 앞선 실행에서 3개가 실패했으며 아직 해결되지 않았다. 작업 결과 사이의 메뉴 잔상도 재현·해결을 확정하지 않았다. 4세션 앱 유휴 측정은 200MB 이내였지만, 1세션의 확장된 기호 샘플에서 200MB를 초과해 메모리 목표도 추가 조사 대상이다. P0 완료나 새 설치본 검증 완료로 표시하지 않는다. 아래 이전 수치는 당시 빌드의 기록이며 최신 검증은 문서 마지막에 추가한다.

2026-10-10. PLAN.md와 WINDOWS_HANDOFF.md 전체를 읽고 Windows NTFS 작업 폴더에서 구현을 시작했다. 이후 사용자 요구에 따라 Tauri 제안을 네이티브 GPU UI로 변경했다.

## 구현

현재 코드 경로는 `src/`와 루트 `Cargo.toml`이다. Rust 1.90.0 MSVC와 Cargo.lock으로 도구·의존성을 고정한다. Node/WebView2/WSL은 기본 의존성이 아니다. Starboard 참고 저장소와 기존 수정 사항은 변경하지 않았다.

| 항목 | 구현 상태 |
| --- | --- |
| 네이티브 UI | egui/eframe + wgpu DX12, LowPower GPU 선택 |
| 터미널 상태 | Alacritty VT 코어, ANSI/UTF-8/alternate screen/scrollback/reflow/selection |
| 기본 셸 | Git for Windows 자동 탐색 및 직접 지정, login interactive, CHERE_INVOKING |
| 선택 WSL | 설치된 WSL2 배포판·사용자·Linux 폴더 지정, 선택 세션에서만 실행 |
| 셸 수명 | 세션별 지속 ConPTY, 확장·숨기기·전환 시 재실행 없음 |
| HUD | 기본 560×112 DIP, 마우스 이동·크기 조절, 같은 폭의 세로 확장, 미니 입력/요약, 자동 축소 선택, 동작 줄이기 |
| 세션 | 최대 4개, 번호 선택·종료 확인, 새 세션 시작은 작업 스레드 |
| 트레이/단축키 | 열기·숨기기·설정·종료, Ctrl+Alt+Space 충돌 안내 |
| 창 환경 | 작업 영역·DPI 배치, Windows display/settings 메시지로 재배치 |
| 입력 | VT 키 매핑, IME preedit/commit 및 후보창 위치, bracketed paste, 선택·복사 |
| 렌더링 비용 | visible 행 캐시, 변경된 행만 shape 재구성, 비선택/숨긴 세션 repaint 억제 |
| 입출력 | 전용 읽기/쓰기/resize 스레드, bounded input 큐, 16KiB VT 처리 단위 |
| 종료 | Windows Job Object로 Git Bash의 Windows 자식 프로세스까지 종료. WSL proxy와 Linux Bash 종료도 fixture 확인 |
| 설정 | schemaVersion 1, 사용자별 JSON 저장, 다크/라이트·글꼴·scrollback |

위 표는 코드에 구현된 경로를 뜻한다. Windows 사용성/호환성의 모든 시나리오가 통과했다는 뜻은 아니다.

## 격리 MSYS 런타임 검증

2026-10-11. 자체 SDK에서 설치 Git과 같은 소스의 기준 DLL과 수정 후보를 컴파일했다. v3는 크기 이벤트 소비를 SIGWINCH보다 먼저 수행하고, 마지막 보고 크기를 조회/출력 캐시와 분리한다. raw/canonical 각각 1,000회, 입력 없는 SIGWINCH/siglongjmp 1,000회, 기존 연동 10개와 같은 후보 DLL의 native GUI가 통과했다. [SHA·원시 기록·검증 범위](patches/MSYS_RUNTIME_VALIDATION.md)에 기록했다.

사용자 Git/기본 제품 프로필에는 적용하지 않았다. 기본 Git의 5개 통과·3개 실패, 메모리·실기·실제 계정/모델의 미검증 항목은 유지한다. 제품 런타임 구성·배포/복구·다른 버전/부하 검증이 남는다. UI staging 앱 SHA는 `ef017394…`, 현재 사용자 PID 5852의 설치본은 `e625872a…`로 유지한다.

## 환경

- 작업 경로: `C:\path\to\MiniTerm`, Windows 로컬 NTFS
- Windows: 25H2, build 26200.9457 (ProductName 레지스트리의 Windows 10 표기는 실제 빌드와 구분)
- MSVC: 14.43.34808, Windows SDK 10.0.22621.0
- Git: 2.50.1.windows.1, Bash: 5.2.37
- Rust: 1.90.0 x86_64-pc-windows-msvc, 프로젝트 `.tools` 설치

## 검증

단위 테스트는 음수 좌표/혼합 DPI 크기 계산, 하단·오른쪽 확장 기준 유지, 작은 작업 영역, 셸 프로필 오류, 설정 범위/버전, VT 키 매핑, bracketed paste escape 차단에 집중한다.

`scripts/test.ps1 -Pty`는 실제 Bash/ConPTY에서 공백·한글 시작 경로, TTY, 환경 변수, 파이프, 터미널 크기 변경 후 PID 유지, Ctrl+C, alternate-screen 시퀀스와 100회 명령 round trip을 확인한다. GUI 지연과 한글 IME는 별도 실기 검증 대상이다.

측정 결과는 완료 후 이 문서에 기록하며, PTY 수치를 GUI pixel 지연 또는 Windows Terminal 대비 우위로 해석하지 않는다.

현재 장치의 실측 결과:

| 항목 | 결과와 범위 |
| --- | --- |
| 초기 단위 테스트 | 당시 7개 통과. 최신 추가 검증은 아래 기록 참조 |
| 실제 Bash/ConPTY 테스트 | 2개 통과. 한글·공백 경로, TTY, 변수·파이프, resize/PID 유지, Ctrl+C, Ctrl+Z/jobs/bg/fg, alternate-screen 시퀀스 |
| 명령 round trip | 최종 100회, 중앙값 9.530ms, p95 15.745ms, 범위 6.636–58.882ms. 앞선 실행의 p95는 8.583ms. 테스트용 단순 PS1 사용, GUI 지연 아님 |
| release 1세션 유휴 | 30.51초, CPU 시간 증가 0% (측정 해상도 내), working set 평균 169.03MiB, 최대 169.04MiB, private bytes 평균 160.57MiB |
| 유휴 렌더링 | 150초 smoke 실행에서 UI 구성 9회. UI 구성 CPU 중앙값 0.0961ms, p95 0.5011ms. GPU present 시간 제외 |
| 초기 메모리 결함 | DX12 기본 heap 설정에서 working set 약 494MiB. MemoryUsage 설정 후 약 169MiB로 감소 |
| 빌드/정적 검사 | Windows x64 release 빌드, Clippy warnings-as-errors 통과 |

추가 GUI 검증에서 한 번 클릭 후 입력, 영문 첫 글자와 `한글` 표시, 헤더 더블 클릭, 하단·오른쪽 기준 유지, 숨기기와 Bash PID 유지, `WM_HOTKEY` 메시지 처리에 의한 표시/포커스 복귀가 통과했다. 시작 전후 foreground HWND가 동일함도 진단 기록에서 확인했다. 입력 이벤트를 프레임별로 배치하고 레이아웃 재실행에서 입력/창 동작이 중복 실행되지 않게 했다. 헤더 더블 클릭과 전역 단축키 수신은 Win32 메시지 경로로 처리한다.

최종 GUI 검증 후 30.53초 유휴 측정은 working set 평균/최대 177.62MiB, private bytes 평균 164.16MiB, CPU 시간 증가 0% (측정 해상도 내)였다. 확장으로 생성된 GPU/font 리소스가 유지된 상태의 값이며 시작 직후 미니 HUD 측정과 구분한다. 원시 결과는 `artifacts/idle-final.json`, `artifacts/native-ui.json`이다.

**미확인 사항:** `scripts/verify-ui.ps1 -PhysicalHotkey`에서 SendInput으로 보낸 Ctrl+Alt+Space가 등록한 창에 도착하지 않았다. 기본 probe는 `WM_HOTKEY` 메시지를 직접 보내 수신·복귀 경로만 확인한다. 실제 키보드 조합은 아직 통과로 표시하지 않는다. Unicode SendInput의 한글 표시는 IME preedit/commit·후보창의 실기 검증을 대체하지 않는다.

측정 장치는 Intel Core Ultra 5 226V (8 logical CPUs), 약 16GB RAM, Intel Arc 130V (드라이버 32.0.101.8626)이다. GPU dedicated/shared memory와 화면 입력→present는 별도로 검증해야 한다. 기본 사용자 PS1보다 단순한 테스트 PS1로 명령 round trip을 측정했으므로 기본 프롬프트 성능으로 일반화하지 않는다.

원시 개발 측정은 `artifacts/idle-memory.json`, `artifacts/native-final.json`에 남겼다. 앱의 입력·출력·프롬프트 본문은 metrics에 기록하지 않는다. UI 검증용 스크린샷에는 테스트 명령만 표시한다.

## 다음 출시 조건

### 에이전트·설치 영역 추가 검증

Windows 사용자 전용 Named Pipe/helper, 공식 Codex notify·Claude Code 훅 정규화, 512개 식별자 중복 제거, 배지·100개 알림 목록, WinRT toast·COM 활성화를 구현했다. 에이전트 연결은 설정에서 명시적으로 적용하며 기존 notify·훅·인증을 보존한다. 단축키 변경/비활성화, 로그인 시작, 알림·소리, 저장 실패 시 등록 복원과 설정 취소도 추가했다. 상세 동작은 [NOTIFICATIONS.md](NOTIFICATIONS.md)에 있다.

사용자별 NSIS 설치본과 잠긴 의존성 라이선스·기본 글꼴 라이선스·빌드 정보·체크섬을 생성한다. 설치/제거 전에는 앱 인스턴스와 IPC를 확인하며 세션을 강제 종료하지 않는다. 제거는 기록된 연결의 정확히 일치하는 명령과 해당 실행 파일의 등록만 정리한다. 실행 중인 프로세스 종료는 테스트용으로 추적한 프로세스에만 수행했다.

추가 결과:

- 로직 테스트 13개 통과. 별도 실제 Named Pipe 왕복·중복 인스턴스 차단 1개 통과. 실제 Bash/ConPTY 2개 통과. 최종 PTY 100회 p95는 25.149ms이며 GUI present 수치가 아니다.
- **설치본**에서 첫 클릭 입력·한글 문자열 표시·확장 기준점·숨긴 Bash PID 유지·WM_HOTKEY 복귀 통과. 다크/라이트와 저장한 14px 글꼴 설정을 화면으로 확인했다. 실제 IME 조합은 미검증이다.
- **설치본** helper/IPC/WinRT/COM으로 완료 중복, 숨긴 HUD 포커스 유지, 하위 에이전트·추가 Stop 제외, 승인 뒤 재개·오류, 보고 있는 세션 팝업 억제, 세션 복귀를 확인했다. OS 알림 기록에 3개 저장됐다. COM 직접 호출과 foreground 권한 부여로 검증했으므로 실제 팝업 클릭과 구분한다.
- 알림·COM 복귀 뒤 release 1세션 30.51초 유휴: working set 평균 184.29MiB, 최대 184.30MiB, private 평균 163.46MiB, CPU 시간 증가 0 (측정 해상도 이하). 원시 파일의 기존 `MB` 키는 실제로 MiB였으며 측정 스크립트의 후속 출력은 명확히 MiB/bytes로 기록한다.
- 사용자 권한 설치, 최신 실행 파일과 설치본 payload SHA 일치, 실제 설정 파일이 있는 동일 버전 재설치에서 바이트 보존, 실행 중 설치/제거 차단, 제거 후 앱 파일·toast/제거 등록 정리, 설정 보존과 시험 설정 원복 통과. 기존 Windows 11 장치에서 진행했으며 깨끗한 장치·다른 버전으로의 마이그레이션은 미검증이다.

원시 결과는 `artifacts/notifications.json`, `artifacts/installer.json`, `artifacts/idle-notifications-release.json`에 있다. 현재 설치본은 **서명되지 않은 로컬 알파**다. 남은 전체 범위는 [MATURITY.md](MATURITY.md)에서 추적한다. 실제 키보드, IME, 팝업 클릭/재시작/DND, 실제 에이전트와 WSL, 다중 세션·TUI·부하·입력→pixel, Windows Terminal/WSL 비교, 다중 모니터, 서명·새 환경 검증을 완료로 표시하지 않는다.

### 창·프로필·아이콘 추가 구현 및 검증

라이트 테마 기본 ANSI 16색·본문·커서의 대비를 높이고, 기본 색상과 OSC 조회 응답이 같은 팔레트를 사용하게 했다. 색상 설정·조회·초기화가 같은 PTY 출력 청크에 섞이거나 OSC 종료 문자가 읽기 경계를 넘어도 당시 색상으로 응답한다. 셸 실행 전에 세션을 등록하고 UI 등록은 받은 상태·알림을 보존하며, 시작 중 알림 복귀를 해당 세션 준비까지 기다린다.

창 크기·간격·모니터·확장 비율, 본문 더블 클릭 옵션과 동작 줄이기를 schemaVersion 1의 기본값이 있는 필드로 추가했다. 기존 설정은 기본 창 배치를 유지한다. 확장/축소는 180ms 동안 기준점을 유지하고 PTY 크기는 종료 시 확정한다. WSL 목록은 레지스트리에서 읽으며 기본 프로필로 VM을 시작하지 않는다.

사용자 요청으로 HUD 및 설정 동작 버튼을 Rust/egui 선 아이콘으로 교체했다. 고정/설정/탭 선택 상태, 읽지 않은 알림 배지, 비활성화, 호버와 키보드 포커스를 표시하며 설명은 툴팁으로 제공한다. 좁은 HUD는 브랜드·상태 표시를 줄여 네 개 세션과 아이콘을 유지한다. 숨기면 미저장 설정과 열린 대화 상자를 닫고 세션은 유지한다.

- 단위 20개, 별도 실제 IPC 1개, Git Bash/ConPTY 2개, 선택한 `Ubuntu-24.04` WSL/ConPTY 1개 통과. Clippy warnings-as-errors 및 release 빌드 통과.
- Git Bash 단순 프롬프트 100회 PTY round trip: 중앙값 3.457ms, p95 6.110ms, 범위 2.793–6.893ms. 실행별 차이가 크며 GUI 입력→pixel 또는 Windows Terminal 비교 수치가 아니다.
- release GUI: 다크/라이트·280 DIP 미니·4세션·크기/간격·없는 모니터의 주 모니터 복귀·확장 기준점·동작 줄이기·설정 화면/WSL 선택 확인. 애니메이션 외부 샘플에서 최종 크기를 처음 관찰한 시간은 191.667ms, 동작 줄이기는 17.076ms였다. IPC 전달과 화면 조회 시간이 포함되므로 내부 GPU present 시간으로 해석하지 않는다.
- 실제 마우스로 아이콘의 확장·고정 후 blur 유지·고정 해제 후 축소·설정 취소/저장·새 세션 동작을 확인했다. 첫 클릭 입력·숨김·Bash PID 유지·WM_HOTKEY 복귀도 새 UI에서 통과했다. 고속 합성 마우스 이벤트의 위치 갱신과 누름 사이에 대기를 넣어 시험 도구를 보완했다.
- release 1세션 30.45초: working set 평균 178.34MiB, 최대 178.37MiB (187,035,648bytes), CPU 0.058% (8 logical CPUs 정규화). 4세션 30.41초: 평균 180.60MiB, 최대 180.61MiB (189,382,656bytes), CPU 증가 0 (측정 해상도 이하). 앱 프로세스만의 수치이며 shell/GPU/WSL 비용과 구분한다.
- WSL fixture에서 Linux 홈/ext4 계열 파일시스템·TTY·Unicode·resize/PID·Ctrl+Z/jobs/bg/fg/Ctrl+C와 Windows 실행 파일 interop 확인. Linux Bash→Windows helper→IPC 완료, 숨긴 창/포커스 유지, 알림 배지, 같은 세션 복귀 및 Windows wsl.exe/Linux Bash 종료 확인. 실제 Linux 에이전트 훅 설치는 아직 미검증이며 사용자 설정을 변경하지 않았다.

시험은 `scripts/verify-window.ps1`, `verify-icons.ps1`, `verify-session.ps1`과 `test.ps1 -WslDistribution Ubuntu-24.04`로 재현한다. `MINITERM_DATA_DIR`로 시험 설정을 분리하며 `MINITERM_DIAGNOSTICS=1`인 인스턴스에서만 진단 제어 명령을 허용한다. 원시 기록은 `artifacts/window-icons-*.json`, `icons.json`, `sessions.json`, `idle-window-one.json`, `idle-window-four.json`이다. 실제 모니터 분리·혼합 DPI와 실제 IME·TUI·OS 팝업 클릭·최종 화면 지연은 남아 있다.

### 한 줄 프롬프트·창 모양 구현 및 검증

Git Bash의 호스트 정보와 입력 사이 강제 줄바꿈을 한 줄 PS1으로 바꿨다. 설정의 `›_` 탭에서 사용자·호스트·경로 각각의 표시 여부/별칭, 전체 경로/폴더 이름, 입력 기호 및 미리보기를 제공한다. 변경은 열린 세션의 다음 프롬프트에서 적용하며 현재 입력과 실행 중 프로그램을 건드리지 않는다. 기존 프롬프트로 돌아가는 스위치도 제공한다.

실제 로그인 Bash에 `--noprofile`을 추가하고 첫 프롬프트 직전 세션 전용 bootstrap에서 정상 초기화 파일을 한 번 읽는다. 로그인/대화형 플래그, alias/function과 초기화 파일의 PROMPT_COMMAND를 유지한다. 별칭은 변수 값으로만 확장하므로 `$()`·백틱·`\\n`이 명령이나 줄바꿈으로 평가되지 않는다. 프롬프트를 끄면 원래 PS1과 promptvars 설정을 복원한다. 프롬프트 상태 파일은 세션별로 소유하고 정상 종료 시 정리한다.

창 설정에 모서리와 전체 창 투명도를 추가했다. 사용자 참고 화면 `docs/img.png`와 추가 요청에 따라 기본 반경은 **4 DIP**, 설정 범위는 **0–12 DIP**로 줄였다. 작은 HUD는 높이에 비례한 반경 상한을 적용해 112-DIP 높이에서는 약 4 DIP까지만 둥글어진다. 앞선 13–24 DIP 저장값은 나머지 설정을 보존하며 새 상한으로 변환한다. 투명도는 **0–80%**, 기본 0%이며 저장 전 미리보기와 취소 복원을 지원한다. 실제 창 스타일 갱신 중 alpha가 초기화되는 문제를 수정했다.

- 단위 테스트 22개, 실제 IPC 1개, Git Bash/ConPTY 3개, `Ubuntu-24.04` WSL/ConPTY 2개 통과. fmt·Clippy warnings-as-errors·release 빌드 통과.
- 실제 ConPTY에서 로그인·TTY·한 줄 PS1·alias/function·첫 프롬프트부터의 기존 훅 실행·종료 상태, 리터럴 별칭, foreground read 중 설정 변경, 기본 프롬프트/옵션 복원과 재활성화를 확인했다. Git Bash managed prompt 100회 round trip 중앙값 7.711ms, p95 9.088ms; 같은 실행의 단순 프롬프트 중앙값 6.342ms, p95 7.232ms. WSL managed prompt 중앙값 4.385ms, p95 5.796ms. GUI 입력→pixel 수치가 아니다.
- 실제 GUI 클릭으로 별칭·호스트 숨김·모서리·투명도 저장, 다음 프롬프트 반영, Bash PID 유지, 반경 0/최대값, 작은 HUD의 반경 제한, 취소 시 파일/region/alpha 복원과 숨김·확장·축소 뒤 투명도 유지를 확인했다.
- 다크/라이트·4세션·애니메이션 기준점·없는 모니터 복귀·저장/취소·고정/blur·새 세션 및 첫 클릭 입력 회귀 검증 통과. WSL helper→IPC·숨긴 포커스·같은 세션 복귀·Windows/Linux 자식 종료도 통과했다.
- 최종 release 1세션 유휴 약 30초씩: 불투명 working set 평균 181.03MiB, 최대 181.04MiB (189,833,216bytes); 투명 모드 평균 181.78MiB, 최대 181.85MiB (190,685,184bytes). 두 경우 CPU 시간 증가는 측정 해상도 이하였다. 앱 프로세스만의 수치이며 GPU/Bash/WSL 자원과 구분한다.

재현: `scripts/verify-appearance.ps1`, `test.ps1 -Pty`, `test.ps1 -WslDistribution Ubuntu-24.04`. 원시 결과: `artifacts/appearance.json`, `idle-appearance.json`, `idle-transparent.json`, `window-rounding-dark.json`, `window-rounding-light.json`, `icons.json`, `sessions.json`. 실행 파일 SHA256은 `a1e57eca74b3b123e12c2f4b708af0b21bc00142dfe1bd059ad45885917ac0c4`이다. 설치본도 갱신했고 네 배포 파일의 체크섬을 확인했다. 설치 프로그램 자체의 재설치·제거 실기 검증은 앞선 기록과 구분한다.

P0는 아직 완료 상태가 아니다. 다음 작업을 끝내야 한다.

1. 실제 키보드 Ctrl+Alt+Space, 한글 IME 조합 취소·후보창, fullscreen TUI, 출력 폭주 중 입력 실기 검증 및 결함 수정. Ctrl+Z/jobs/bg/fg는 현재 Git Bash/ConPTY 통합 테스트에서 통과했다.
2. 클릭→입력→present p95, release 유휴 자원, 4세션 자원, GPU 전용/공유 메모리 기록.
3. Windows Terminal 동일 환경 기준선과 Windows/WSL A/B/C 파일시스템 비교.
4. 실제 에이전트와 WSL 연동, 동시 세션, 중단 신호, 팝업 클릭·재시작·DND 검증 및 결함 수정. IPC·어댑터·배지·목록·WinRT/COM 기본 경로는 구현했다.
5. NSIS·로그인 시작·설정의 추가 개선, 교차 버전·새 환경 검증과 공개 배포 서명. 로컬 설치/재설치/제거는 검증했다.

네이티브 UI의 오류를 해결하는 과정에서 웹뷰·다른 셸로 자동 대체하거나 사용자 프로젝트를 다른 파일시스템으로 옮기지 않는다.

### 2026-10-11: 실사용 UI 결함 수정과 입력 호환성 검증

설정 화면 안에서 테마를 바꾸면 egui의 글꼴 atlas가 교체되는데 이전 행 캐시가 옛 좌표를 사용해 글자가 깨졌다. 기준 글리프의 캐시 객체 교체를 감지해 행 캐시를 무효화했다. 실제 별도 창에서 설정을 닫기 전에도 다크/라이트 문자와 배경이 올바르게 바뀌는 것을 확인했다. 한글·영문·숫자의 셀 기준선을 맞추고 기본 14px Consolas, 설치된 Cascadia Mono/Code 등의 GUI 선택, 기본 글꼴의 실제 볼드/이탤릭과 사용자 글꼴의 합성 강조를 추가했다. CLI에 xterm-256color/truecolor 환경을 전달하고 자동화에서 상속된 NO_COLOR를 제거한다. 프로그램이 출력하는 ANSI 색상·강조를 표시한다.

미니 입력 영역 더블 클릭으로 확장하고 헤더를 드래그해 이동하며 native 창 가장자리로 크기를 조절한다. 크기·위치를 저장하고 숫자 크기 조절과 사용자 곡률 설정을 제거했다. 픽셀 region을 고정 DWM 작은 모서리로 교체했다. 항상 위에 있는 HUD의 핀 버튼은 미니 입력/요약 전환 아이콘으로 바꾸고 확장 유지 여부는 자동 축소 설정으로 정리했다. 미니 화면은 VT 크기를 유지하며 커서 줄과 최근 출력을 보여 주고, 요약에서 상태·프로그램 제목·출력의 표시 항목을 선택한다.

펼칠 때 폭과 하단 기준점을 유지하고 약 180ms 동안 위로 연다. 최종 PTY 크기를 먼저 요청해 중간 화면에서도 커서 주변 내용을 보여 준다. 입력과 resize는 같은 비동기 FIFO 작업자에서 처리한다. 높이를 너무 작게 저장해도 펼치기가 미니보다 작아지지 않도록 제한했다.

Git Bash가 npm의 codex 셸 shim을 사용해 Windows AI Battery launcher를 건너뛰는 경로를 보완했다. Windows PATH의 첫 관리 대상 codex.cmd와 companion Node bridge를 연결하며 기존 사용자 alias/function을 보존한다. 이 장치의 잘못된 PATH 따옴표도 자식 프로세스의 PATH에서만 복구한다. 실제 설치된 bridge의 Codex 버전 조회, native Node TTY, 한글·메타 문자 인자와 종료 코드 전달을 확인했다. 실제 AI Battery HUD·모델 대화는 아직 자동 통과로 표시하지 않는다.

| 검증 | 현재 결과 |
| --- | --- |
| 로직·정적 검사 | 28개 통과, fmt 및 Clippy warnings-as-errors 통과, Windows release 빌드 통과 |
| 실제 IPC | 별도 진단 UUID/설정 경로에서 왕복·중복 인스턴스 거부 1개 통과. 사용자 앱 유지 |
| GUI | 불투명/30% 투명 및 Cascadia Mono에서 본문 확장, 같은 폭·하단 기준, 중간 크기, resize hit test, 이동/크기 저장, DWM 모서리/region 제거 확인. 불투명 창에서 설정 안의 테마 전환·문자 표시 확인. 별도 fixture의 실제 VT에서 색상·볼드·이탤릭·DIM과 확대 후 보존을 boolean으로 검사하며 터미널 본문은 IPC에 노출하지 않음 |
| GUI 시험 범위 | Win32 client 마우스 메시지와 이동/resize 종료 메시지 및 실제 창·화면 조회. 실제 손으로 하는 드래그, IME와 입력→pixel 측정은 별도 검증 필요 |
| Git Bash 통합 | 6개 통과·2개 실패. 일반 Session 입력과 ANSI flags, TTY/잡 제어, 기존 prompt 훅 보존은 통과 |
| **실패: resize 후 입력** | 실제 Session에서 가로+세로 또는 세로만 변경 후 `printf`가 `rintf`로 도착한다. 새 30회 반복 회귀 시험 2개가 실패. renderer 외의 순수 ConPTY에서도 간헐 재현 |
| 비교 시험 | 별도 MinGit 2.56.0.windows.2/Bash 5.3.15/MSYS 3.6.10, system ConPTY와 flag 변경, node-pty에서도 입력 누락 재현. 이것만으로 특정 upstream 원인을 확정하지 않음. 설치된 Git/AI Battery를 변경하지 않음 |
| Windows AI Battery bridge | 명시적인 `test.ps1 -WindowsBridge` 1개 통과. 실제 Codex 0.162.0 버전 조회이며 모델/인증 호출 없음 |
| WSL | 2개 시작 실패. 앱 밖의 `/bin/true` 직접 실행도 `Wsl/Service/E_UNEXPECTED`, exit -1. 사용자 VM/서비스를 종료하거나 재설정하지 않음 |

최신 Git Bash PTY 100회 round trip 중앙값 5.900ms, p95 7.325ms; managed prompt 중앙값 4.299ms, p95 5.119ms다. GUI 픽셀 지연이나 Windows Terminal 대비 성능 수치가 아니다. 새 빌드의 GPU/유휴 자원·부하 인수 시험도 별도로 필요하다.

현재 `artifacts/miniterm.exe`는 UI 검토용 개발 빌드다. 입력 회귀 시험이 실패해 새 설치본으로 승격하지 않았으며 사용 중인 설치본 PID 10756과 Codex 세션을 유지했다. 원시 GUI 결과는 `artifacts/appearance.json`, `artifacts/ux.json`이며 빌드별 fixture 경로와 SHA256을 기록한다. 재현은 `scripts/test.ps1 -Pty`, `-WindowsBridge`, `scripts/verify-appearance.ps1`, `verify-ux.ps1`을 사용한다. 기존 scripts의 모든 실기 통과 기록을 최신 빌드 통과로 간주하지 않는다.

현재 UI 검토용 실행 파일 SHA256: `e7e2d0a464e0c710a5135797055a6956740be4e392220e4fa0d14b7e8505ee32`. 사용 중인 설치본은 기존 `a1e57eca...` 빌드로 유지한다. `artifacts/CURRENT_BUILD_STATUS.json`은 이 구분과 실패 항목을 기록한다.

### 2026-10-11: 사용자 스크린샷의 한글·배터리 표시 회귀 수정

사용자가 `screenshot/스크린샷 2026-10-11 124759.png`, `124814.png`를 제공했고 왼쪽에서 실행한 수정본의 문제임을 확인했다. `/resume` 선택 줄에서 한글이 조각처럼 보이는 원인은 인코딩이나 한글 글꼴 부재가 아니라, 두 칸 글리프를 그린 뒤 다음 spacer 셀의 배경을 덧칠한 순서였다. 행의 배경을 먼저 모두 그린 뒤 글리프·밑줄을 그리도록 수정했다.

설치된 AI Battery는 막대로 `❚` (U+275A)를 출력한다. 이 장치의 Consolas·Malgun·Cascadia Mono에는 해당 글리프가 없고 Segoe UI Symbol에는 있다. OS 심볼 글꼴을 공통 폴백에 추가하고 regular/bold/italic/bolditalic에서 한글·막대·점자·블록·선 글리프의 존재를 검사했다. 사용자 글꼴과 한글 폴백을 교체하거나 AI Battery를 변경하지 않았다. 불투명 다크·라이트 실제 창의 고정 샘플에서 온전한 한글 선택 줄과 녹색/회색 막대를 확인했다.

동기화 프레임 `CSI ?2026h`를 시작하고 종료 신호가 끊겼을 때 기존 blocking reader는 더 이상 출력이 오지 않으면 VT 코어의 제한 시간을 처리하지 못했다. 출력 운반/파싱 사이에 256KiB bounded 큐를 두고 파서는 활성 프레임의 제한 시간에만 깨어나 확정한다. 계속 출력이 밀려 있어도 만료를 먼저 확인한다. 부분 프레임은 제한 시간 전에 표시하지 않으며, 읽기 청크를 가로지르는 종료 신호와 EOF에서도 프레임을 보존한다. 회복 시험 3개를 추가했고 평상시 유휴 폴링은 추가하지 않았다.

최신 결과:

- 로직 32개, 별도 진단 이름의 실제 IPC 1개, Windows AI Battery bridge 1개 통과. fmt·Clippy warnings-as-errors·Windows release 빌드 통과.
- Git Bash 5개 통과·3개 실패. managed prompt resize와 실제 Session의 가로/세로·세로만 resize에서 `printf`의 첫 글자가 빠졌다. 원시 ConPTY에도 있는 간헐 결함이며 동기화 프레임 회복 수정으로 해결되었다고 표시하지 않는다.
- 100회 Bash PTY round trip 중앙값 6.744ms, p95 12.667ms. managed prompt 중앙값 8.281ms, p95 10.270ms. 다른 실행과 시스템 부하 차이가 있고 GUI 입력→pixel 지연이 아니다.
- 실제 실행 중인 수정본의 콘솔 크기를 읽기만 했다. Bash 80×52, AI Battery 안의 Codex 80×51로 배터리 한 줄을 제외한 크기가 맞았다. 이를 메뉴 위치의 정상 동작 증거로 사용하지 않는다.
- `scripts/verify-ux.ps1 -GlyphSample`은 고정 Unicode/ANSI 선택 줄과 하단 위치 샘플을 실제 창으로 캡처한다. `-IdleSeconds 30`으로 해당 시험 앱의 유휴 자원도 측정한다. 실제 Codex 메뉴, 실제 IME와 입력→pixel 검증은 구분한다.

화면 갱신 결함의 수정만으로 사용자 스크린샷의 메뉴 잔상이 해결되었다고 확정하지 않는다. 라이트 테마의 명시적인 ANSI 검정/흰색 배경 조합은 대비 조정이 추가로 필요하다. 입력 결함 때문에 설치본 승격은 계속 보류한다. 기존 설치본 PID 10756과 사용자 수정본 PID 25000, 그 안의 Codex 세션은 유지했다. 최종 빌드 SHA와 캡처·자원 결과는 `artifacts/CURRENT_BUILD_STATUS.json` 및 해당 `artifacts/ux-<UUID>`에서 구분한다.

큰 한글 TTF가 egui의 owned 경로에서 다시 복사되는 비용도 확인했다. 고정 OS 글꼴 7개를 프로세스 수명의 borrowed 데이터로 공유해 중복을 제거했다. 임의 사용자 글꼴은 기존 owned 방식으로 교체 시 해제한다. 동일한 다크/라이트·Unicode 샘플 뒤 1세션 release 유휴 30초 측정에서 최대 working set이 **214,700,032bytes → 184,528,896bytes** (204.75 → 175.98MiB)로 줄어 200,000,000bytes 목표를 통과했다. CPU 시간 증가는 측정 해상도 이하였고 private 평균은 164.62MiB다. 앱 프로세스만의 값이며 GPU·Bash·Codex·4세션 부하·입력→pixel과 구분한다.

해당 최종 빌드 SHA256은 `081963280ce1c82edfad0ccd30ca50f336817368d7a9bcf814c60e5d4203e275`다. 기본 글꼴 캡처·자원은 `artifacts/ux-f65fe331-4e14-4474-8c48-281e26ebc092`, Cascadia Mono 캡처는 `artifacts/ux-f7cb0e38-ce18-4700-a921-443ea92ce0e1`에 있다. 모든 시험용 창은 닫았으며 사용자 두 창의 실행 파일은 그대로다.

추가로 설치된 예전 helper에 사용하지 않는 진단 UUID로 `sessions`를 읽기만 했는데 기존 앱의 세션을 반환했다. 예전 helper가 새 진단 namespace를 지원하지 않아, 기존 Codex notify 설정을 그대로 사용하는 별도 수정본의 알림 라우팅은 아직 정상으로 검증할 수 없다. 사용자 notify 설정·기존 callback·Windows 등록을 변경하거나 알림을 시험 발송하지 않았다. 같은 테스트 앱 내부로 helper/이벤트가 도착하는 경로와 기존 설치본 경로를 구분해 보완해야 한다.

### 2026-10-11: 실제 Codex 선택 메뉴의 기준 터미널 비교

`scripts/verify-codex-ui.ps1`은 새 UUID의 CODEX_HOME·작업 폴더에서 실제 Windows Codex CLI를 실행한다. 로컬 HTTP 제공자는 모든 요청에 고정 HTTP 400을 반환하므로 외부 모델이나 사용자 인증을 사용하지 않는다. 한글 요청으로 테스트 세션을 만들고 `/model`, `/resume`을 열고 닫으며 80×52 → 80×36 → 60×24 → 96×48 크기로 바꾼다. `/resume`에서 실제 한글 세션 제목을 확인한다. 단순 시간 대기 대신 화면 내용으로 단계 완료를 검사한다.

`-Battery`는 설치된 AI Battery의 실제 Windows compositor를 함께 실행한다. 배터리 데이터 생성기만 고정된 로컬 출력으로 대체하므로 계정 사용량을 읽거나 조회하지 않는다. 배터리가 마지막 행을 유지하는지, 메뉴를 닫았을 때 선택 화면이 남지 않는지도 확인한다. 이 장치에는 해당 Node 런타임과 node-pty/@xterm/headless 의존성이 이미 설치되어 있으며 시험은 패키지나 런타임을 자동 설치하지 않는다.

기준 @xterm/headless의 화면·스타일·커서와 원시 VT 및 resize 경계를 기록하고, 별도 opt-in Rust 시험으로 MiniTerm의 `parse_output`·Alacritty 상태에 재생한다. 모든 화면 행, 한글 폭, ANSI 색/RGB·bold/dim/italic/inverse, 커서 위치가 일치했다. 관측한 CLI 버전은 0.162.1이다. 앱 GPU 픽셀, 실제 IME, 모델 응답 도중 goal 선택 메뉴, 사용자 스크린샷의 잔상 재현까지 통과했다는 뜻은 아니다.

- Codex 직접 실행: `artifacts/codex-ui-1133da34-22e9-408d-b6bb-d8a7d25b2ed8/verification.json` 통과.
- AI Battery 합성: `artifacts/codex-ui-91bc828b-181e-4a7c-a83c-341f39e44576/verification.json` 통과.
- 일반 시험 32개 통과·2개 opt-in 제외, fmt 및 Clippy warnings-as-errors 통과. 기존 Bash 실패 시험의 결과는 바뀌지 않았다.

표시 수정 실행 파일 `08196328...`을 새 사용자 검토 창 PID 29644로 실행했다. 제목은 `MiniTerm - Glyph Fix Preview`, 경로는 `artifacts/user-preview-ef8bb6b3-a88d-4743-b418-7663067c1f67/miniterm.exe`다. 기존 PID 10756·25000과 그 세션은 유지했고 설치본·글로벌 연동은 변경하지 않았다. 새 창은 단축키·자동 시작·알림을 끄고 별도 설정/IPC 이름을 사용한다. 실행 기록은 `artifacts/manual-test/glyph-fixed-preview.json`에 있다.

### 2026-10-11: 실행별 Codex 알림 전달과 프로필 보존

글로벌 notify는 예전 helper의 경로를 사용하므로, 별도 최신 앱의 세션 ID가 예전 helper에서 잘못된 IPC 이름으로 전달되는 문제가 있었다. 새 Git Bash의 Codex 함수가 현재 helper에 읽기 전용 조회를 요청한다. helper는 현재 앱에 등록된 세션인지, CODEX_HOME의 설정·프로필 notify가 보관한 MiniTerm 기록과 정확히 같은지 확인한다. 맞으면 해당 실행에만 `-c notify=...`를 추가해 현재 helper로 전달한다. chain 파일·기존 콜백·설정 파일을 바꾸지 않는다. 사용자 함수/별칭, 명시적인 notify 설정, 프로필의 다른 콜백·비활성화도 보존한다. 모델만 바꾸는 프로필은 기본 연결을 계속 상속한다.

Windows 경로 구분자가 CODEX_HOME에서 `/`로 전달되는 경우도 시험에서 확인했다. chain 파일의 경로를 문자열 대신 Path로 비교하여 같은 Windows 경로가 잘못 제외되지 않게 했다. 조회 실패·미등록 세션·소유하지 않은 설정에서는 원래 CLI 호출을 유지한다. 이 코드가 기존 실행 중인 셸에 삽입되지는 않는다.

`scripts/test.ps1 -CodexNotify`는 새 CODEX_HOME와 로컬 Responses SSE 제공자에서 실제 Codex를 네 번 완료한다. 기본·상속 프로필의 이벤트 두 건은 현재 helper/별도 IPC/Broker에 도착하고, 비활성화 실행은 전달되지 않는다. 프로필의 별도 콜백과 보관한 기존 콜백의 총 세 번 호출, 한글·공백·`&`·`$()` 인자의 리터럴 유지, 원본 payload, 설정·backup 바이트 보존을 확인했다. 같은 Bash에서 사용자 함수·별칭을 정의한 뒤 bridge를 다시 읽어도 동작을 유지했다. 로컬 제공자는 입력을 폐기하고 고정 메시지만 보내며 외부 모델·사용자 인증을 사용하지 않는다. 원시 결과: `artifacts/codex-notify-75785672-75cc-4b0f-b5ec-28266eabf5cc/verification.json`.

현재 사용자 수정본 PID 29644에 대해 새 release helper의 `sessions`·`codex-notify-override` 조회만 실행해 실제 보관된 연결에서 현재 helper와 기존 chain 파일을 선택함을 확인했다. 알림·원래 콜백·GUI 동작은 실행하지 않았다. 기록: `artifacts/manual-test/notify-lookup.json`. 사용자 config/backup SHA256도 시험 전후 동일했다.

- 최신 로직 35개 통과·2개 opt-in 제외, fmt·Clippy warnings-as-errors·release 빌드 통과. 실제 Windows bridge의 TTY·인자·종료 코드·버전 시험도 통과.
- 최신 실행 파일 SHA256: `4d2b6cdb90a9542ca4023dd71ca67a5dcf72c74ed2bb669c85cf6856cf6b49dd`.
- 최신 GUI 회귀 `artifacts/ux-519f74fe-3917-4fe6-b179-9fc22c045731`: 본문 확장·같은 폭/하단·중간 크기, 이동/크기 저장, native hit test, 고정 DWM 모서리, 설정 안의 테마 변경, ANSI flags·기호 샘플 통과. Win32 합성 메시지와 고정 샘플이며 실물 마우스·IME·원래 Codex goal 잔상을 대신하지 않는다. 라이트 샘플의 명시적 검정/흰색 ANSI 배경 대비 문제는 남았다.
- 위 release 앱의 10.18초 유휴 최대 working set 183,828,480bytes (175.31MiB), private 평균 162.38MiB, 측정 CPU 증가 0. 앱만의 값이고 GPU·Bash·Codex·4세션 부하·입력→pixel과 구분한다.
- 별도 node-pty/ConPTY에서 키의 정상 가상키·스캔 코드를 전송했어도 7번째 명령의 `printf`가 `rintf`가 됐다. 원시 기록 `artifacts/experiments/resize-input-physical-vk.json`. 가상키가 0이라 누락된다는 가설로 수정하지 않는다. 기존 입력 실패 시험은 해결되지 않았다.

## 2026-10-11 중립색 대비·정지된 목표 메뉴·4세션 추가 검증

라이트 ANSI 팔레트의 white/bright white를 실제 밝은 색으로 제공해 `30;47` 같은 검정 글자/흰 배경이 어두운 배경으로 바뀌지 않게 했다. 기본 글자 또는 기본 배경 위의 테마 소유 흰색 글자는 실제 색상 쌍의 대비를 확보한 뒤 inverse를 적용한다. 명시적인 RGB/OSC 색상과 명시적인 fg/bg 조합은 유지하며, 행 캐시도 최종 색상 쌍으로 무효화한다. sRGB 변환은 256개 공유 lookup으로 처리한다. 일반/밝은/인덱스 검정·흰색 조합, 기본 반전, 색 배경의 기본 글자, RGB·OSC 보존 시험이 통과했다. 로직 총 38개·Clippy·fmt·release 빌드 통과.

최신 staging 실행 파일 SHA256은 `c09a9ead41bcd1f3edda171398fbd31ac92f69328bb9270e990ba0d1e513492b`다. `artifacts/ux-bc25b0ef-b0d4-49e2-95ad-d7b61191dce7`의 다크·라이트 캡처에서 한글 선택 줄·막대·검정/흰색·기본 반전·파랑 배경의 글자를 확인했다. 같은 폭의 세로 확장과 중간 크기, 이동/크기 저장, 설정 안에서의 테마 전환도 통과했다. 이 기록은 고정 샘플과 Win32 합성 메시지이며 실제 사용자 Codex의 도구 실행 중 선택창을 대신하지 않는다.

`scripts/verify-codex-ui.ps1 -Goal`은 별도 한글 기록에 목표를 만들고 정지한 뒤, 종료된 자체 fixture만 복사해 실제 `Resume paused goal?` 선택창을 연다. 작은/큰 크기, Leave paused 선택·닫기와 이후 한글 입력을 검증한다. Node `node:sqlite`로 복사본의 rollout 경로만 갱신하며 사용자 목표 DB와 인증은 복사하지 않는다. 긴 입력 시험의 동기화도 고유 요청 표식으로 바꿔 앞선 오류를 다음 응답으로 오인하지 않게 했다. [공식 Goal 명령 안내](https://developers.openai.com/cookbook/examples/codex/using_goals_in_codex)

- 직접 실행: `artifacts/codex-ui-839c5efc-36af-4881-b28c-238f23130fa0`.
- 실제 설치된 AI Battery 합성기: `artifacts/codex-ui-e723bd68-5f66-454d-95f8-93f258eaa610`.
- 기존 모델/재개 메뉴 시험의 변경 후 확인: `artifacts/codex-ui-09d09f37-02c2-4eac-8eac-e074306d040a`.

모든 행의 텍스트·스타일·한글 폭·커서가 MiniTerm 파서와 기준 @xterm/headless에서 일치했고 닫힌 목표 메뉴는 다음 로컬 오류 응답에 남지 않았다. 기록이 긴 정지된 목표 메뉴는 추가 검증됐지만, **사용자 스크린샷의 도구 출력 사이 잔상이 해결됐다는 증거는 아니다.** 로컬 제공자는 HTTP 오류만 반환하며 실제 모델·도구·GUI 픽셀은 이 시험 범위에 포함하지 않는다.

유휴 자원은 샘플별로 결과가 달라 평균해서 통과로 표시하지 않는다.

| 현재 빌드·범위 | 측정 | 결과 |
| --- | --- | --- |
| 1세션, 확장된 한글/기호/색 샘플 후 | 10.21초, 최대 working set 206,774,272bytes (197.20MiB), private 평균 163.35MiB | **200,000,000bytes 목표 초과**, CPU 증가 측정 해상도 이하 |
| 4세션, 기본 ANSI/창 시험 후 | 30.47초, 최대 working set 186,052,608bytes (177.43MiB), private 평균 172.16MiB | 해당 샘플은 목표 이내, CPU 증가 측정 해상도 이하 |

4세션 원시 기록은 `artifacts/ux-34cd973d-b391-4486-9fcd-93322884d2f4`다. 실제 앱의 session 수 4·pendingStarts 0과 앱 종료 후 테스트 Bash 네 개의 종료를 확인했다. 두 자원 측정 모두 앱 프로세스만 대상으로 하며 GPU·Bash·에이전트 자체 자원, 출력 부하와 입력→pixel 지연을 포함하지 않는다. 기호 샘플의 working set 초과 원인은 추가 조사 대상이다.

입력 경로 비교에서는 Windows 콘솔 API 단독 시험 1,000회가 정확하게 도착했고, MSYS의 일반 Perl `sysread`에서도 첫 글자 누락이 재현됐다. [입력 조사 기록](RESIZE_INPUT_INVESTIGATION.md)에 범위와 SIGWINCH 처리의 잠금 해제 구간에 대한 가설을 정리했다. 실제 DLL 호출 순서로 원인을 확정하거나 입력 누락을 고친 상태는 아니다. 실행 중인 사용자 앱 10756·25000·29644는 앞선 바이너리와 세션을 유지하며 이번 변경이 자동으로 적용되지 않았다. 설치본·Windows 등록·전역 에이전트 설정도 변경하지 않았다.

사용자 세 창 PID 10756·25000·29644와 그 세션을 유지했다. 설치본과 등록을 바꾸거나 네 번째 사용자 창을 추가하지 않았다. 새 실행별 전달 코드와 실제 GUI/Windows toast 활성화는 별도 범위이며, 현재 Windows COM 등록은 여전히 기존 설치 경로에 연결되어 있다.

## 2026-10-11 작업 요약 구성 변경

요약의 Bash 줄을 제거하고 OSC 프로그램 제목을 상단 표시줄로 옮겼다. 현재 화면의 Working/Thinking/중단 상태, 명시된 계획의 현재·최근 완료/중단 항목, 실제 AI Battery를 선택한다. 커서 아래 배터리도 읽어 하단에 원래 ANSI 스타일로 표시하고 `/status`, `5h limit`, shortcuts 안내를 작업 내용에서 제외한다. 계획이 없으면 최근 실행/확인 작업이나 의미 있는 출력을 표시한다. 표시 항목 GUI 설정과 기존 설정 파일의 기본값 호환을 유지한다.

진행률은 관찰된 계획의 완료 항목 비율이다. 계획이 없거나 생략/탐색 범위 끝으로 목록이 잘리면 표시하지 않는다. 중단·응답 종료로 100%가 되지 않으며 새 요청에 이전 계획·상태·최근 출력을 붙이지 않는다. 한글의 두 칸 폭·자동 줄바꿈을 합쳐 작업명을 읽고 코드 블록/제목 없는 일반 목록은 계획에서 제외한다. 세션별 VT revision 캐시로 유휴 재탐색을 피하며 추가 API 호출이나 사용자 transcript 읽기·훅 설치는 하지 않는다. 자세한 동작과 제한은 [미니 요약 문서](MINI_SUMMARY.md)에 있다.

- 로직 **45개 통과·2개 opt-in 제외**, fmt·Clippy warnings-as-errors·release 빌드 통과.
- 기존 실제 CLI/AI Battery 캡처 `artifacts/codex-ui-09d09f37-02c2-4eac-8eac-e074306d040a`의 출력 재생에서도 커서 아래 실제 배터리 선택과 계획 없는 상태를 확인했다.
- 최신 release SHA256: `e814edd9535fa8ca55ad1f16d71f0383aeb088d96921e169c4eda26cbb1d0513`.
- 최신 GUI: `artifacts/ux-f96ccc39-34ab-4cc7-a077-dd90efb1d1b2`. 다크/라이트, 제목·상태·현재 작업·완료 작업·하단 배터리, 작은 창의 줄 수와 계획 없는 경우를 확인했다. 요약 한 번 클릭의 native 입력 경로를 추가해 펼치기를 확인했으며 여는 클릭/해제를 새 TUI로 전달하지 않는다. 기존 입력 화면 더블클릭·세로 확장·이동/크기·테마 검사도 통과했다. 실제 마우스/IME·호버 스크롤은 미검증이다.
- 위 앱의 요약/GUI 시험 후 **10.17초 유휴**, 최대 working set **183,525,376bytes (175.02MiB)**, private 평균 **162.27MiB**, CPU 증가 측정 해상도 이하. 이 샘플은 200,000,000bytes 이내이며 이전 기호 샘플의 목표 초과를 해소했다고 표시하지 않는다.

실패도 남긴다. `artifacts/ux-45c8661f-5729-4a2d-a94b-9ad1bd8e1e9b`에서는 한 번 클릭 검사가 실패해 native 입력 연결을 보완했다. `artifacts/ux-7aaa5489-9015-4f69-b3e4-84dea28f0adb`의 debug 시험은 초기 ANSI 샘플 도착 검사에서 실패했다. 최종 release 통과로 이 간헐 입력/초기 동기화 실패의 원인이 해결되었다고 표시하지 않는다. 로컬 Codex 0.162.1의 계획 도구 시험도 `update_plan` 지원 부재로 계획 화면을 만들지 못했으므로 고정 계획 샘플과 구분한다.

실행 중인 사용자 세 창은 기존 바이너리와 세션을 유지했다. 새 staging 요약이 기존 화면에 자동 적용된 상태는 아니며 설치본·Windows 등록·전역 에이전트 설정을 변경하지 않았다.

## 2026-10-11 상단 간소화와 직접 태스크 보고

상단에는 새 탭·입력/요약 전환·숨기기만 남겼다. 확장·알림·설정 아이콘을 제거해 OSC 제목 공간을 넓혔으며 기존 더블 클릭과 트레이 설정 경로를 유지했다. 요약 첫 줄은 비율을 먼저 표시하고 현재 태스크를 다음에 배치한다. 임의의 최근 출력 줄을 현재 태스크로 선택하지 않는다.

프로젝트 skill `.agents/skills/miniterm-progress`와 `progress begin/update/read/clear` CLI를 구현했다. 짧은 작업명·항목·명시적인 상태만 로컬 IPC로 받아 세션 메모리에 보관한다. 전체 현재 계획과 작업 ID/다음 revision으로 갱신하고, 오래된 보고·종료된 셸·미완료 항목이 남은 완료 보고를 거부한다. Enter/Ctrl+C 입력을 큐에 보내면 이전 보고를 만료시키며 승인 Enter도 포함한다. 보고는 에이전트의 계획 상태이고 명령 성공의 독립 판정이나 시간 예측이 아니다. 공식 응답 완료 이벤트와 계획 완료를 분리했다. 일반 sessions 조회에는 작업 텍스트가 없으며 추가 요약 모델/API·transcript·파일/프로세스 polling을 사용하지 않는다.

fmt, 로직 50개(별도 opt-in 2개 및 PTY 12개 제외), Clippy all-targets `-D warnings`, release가 통과했다. skill 공식 validator는 fixture에만 설치한 PyYAML과 UTF-8 모드에서 통과했으며 전역 Python/에이전트 설정은 바꾸지 않았다. `verify-ux.ps1 -ProgressSample -IdleSeconds 10`은 실제 helper/IPC/skill 어댑터의 한글 전달, 33→66%·중단·완료·초기화, 오래된 갱신/잘못된 완료 거부, 배터리 유지, 세 아이콘과 제목 공간, 다크/라이트·작은 창·native 한 번 클릭을 검사했다. `artifacts/ux-d3f76684-664e-4936-9620-c5843ddc223b`의 화면을 직접 확인했다. 구버전 helper·서버와 미등록 세션에서 알 수 없는 progress 명령을 보내지 않는 stub 시험은 `artifacts/progress-adapter-4e351f0e-7e87-46b5-a29f-e90b7400eacc`다.

실제 모델의 skill 자동 선택/지속 갱신·물리적 입력 후 보고 만료·호버/IME 검증은 남는다. skill은 이 프로젝트 범위에만 제공하며 사용자 전역에 설치하지 않았다. 새 보고 통로는 현재 실행 중인 이전 앱에 자동 적용되지 않는다. 최신 SHA256은 `e625872a5cafb30b1a6412517c5a5389be7840e1e69fb7b412286a15d68dc2cf`다. GUI 종료 검증 후에도 초기 사용자 세 프로세스가 살아 있었고, 나중 최종 조회에서는 PID 25000이 없어졌다. 종료 원인은 관찰하지 못했으며 이번 작업이 그 프로세스를 종료하지 않았다. 설치본/두 이전 preview 파일의 SHA256은 그대로다.

이번 앱 10초 유휴 최대 working set은 **205,062,144bytes (195.56MiB)**로 200,000,000bytes 목표를 초과했다. CPU 증가는 측정 분해능 안에서 0이었다. 앱만 측정했고 GPU·셸·에이전트·입력→pixel은 제외했으며 기존 메모리/입력 실패를 해소한 결과로 표시하지 않는다.

## 2026-10-11: 목표 재개창과 클릭 동작 수정

기본 Codex 0.162.1 + 설치된 AI Battery compositor에서 중앙 목표 재개창을 재현했다. 기존 native UI 시험이 강제한 `--no-alt-screen`은 실제 기본 실행과 달랐다. 하단 위치 회귀 검사 `artifacts/codex-ui-5de1a97d-9fcb-405a-bf58-2e21df668e0d`는 52행 중 제목 22행·힌트 29행에서 실패했다. VT 파서가 xterm과 일치한다는 이전 결과는 UI 배치 통과가 아니었다.

MiniTerm Windows Codex 함수가 실행별 `--no-alt-screen`을 추가한다. 사용자 인수와 알림 연결을 함께 전달하고 중복 flag를 넣지 않는다. 전역 Codex/AI Battery 설정과 이미 실행 중인 세션은 변경하지 않는다. 기존 별칭·함수는 보존하고 `MINITERM_CODEX_INLINE=0` 실행은 원래 전체 화면 방식을 허용한다. 실제 로컬 SSE 응답의 notify/profile/disabled-notify 보존도 통과했다: `artifacts/codex-notify-5633bf67-b01b-4d6c-bfab-c0209fd411e3`.

요약의 native/egui 단일 클릭 확장을 제거하고 본문 확장·축소를 더블 클릭으로 고정했다. 기존 `bodyDoubleClickExpands=false` 값은 새 동작을 제한하지 않는다. Shift + 더블 클릭은 단어 선택을 유지한다. 이전 설정 checkbox는 고정 동작 안내로 교체했다. `artifacts/ux-f98545e3-e514-405b-bd1c-e851d591c153`에서 단일 클릭 크기 유지, 요약 더블 클릭 확장, 확장 본문 위·가운데·아래 축소, 세로 확장·이동/크기/테마/작은 요약을 검사했다.

새 `scripts/verify-goal-hud.cjs`/`.ps1`은 완료된 자체 seed만 복사하고 인증 없이 로컬 제공자를 설정한다. 실제 Bash의 생성된 연결로 Codex를 실행하며 AI Battery composite 시작과 inline 인수를 관찰한다. 진단 IPC에는 알려진 선택창의 좌표만 노출하고 터미널 텍스트를 반환하지 않는다. `artifacts/goal-hud-f896c2c6-9a6d-470f-9154-b852b34e5eca`에서 본문 y=32..896, 선택창 y=712..872, 마지막 배터리 줄, 다크/라이트, 방향키·Enter 닫기와 소유 Bash 종료를 검증했다. 캡처를 직접 검토했다. 초기 harness의 tray 핸들 오인과 runtime fallback은 수정했고, 배터리가 관찰되지 않은 초기 GUI 기록은 실제 compositor 검증으로 인정하지 않는다.

`artifacts/codex-ui-523c331f-394f-46a7-883b-ddbc12fb6759`는 no-profile Bash/AI Battery에서 24→52행 확대와 크기 변경, 닫기, 다음 로컬 오류 응답, 배터리 위치를 검사한다. 해당 VT의 MiniTerm 문자/ANSI 스타일/커서 비교도 통과했다. 계정·모델·진행 중 실제 도구 응답·한글 IME 시험으로 확대하지 않는다.

최종 staging SHA256은 `56d92bf287d3131ca06e768869b119ced3b285389edf1881121b7ca138b7e8b5`다. 로직 50개, fmt, all-target Clippy, release가 통과했다. 사용자 PID 5852와 설치 파일 e625 빌드는 유지했고 테스트 창은 종료했다. 입력 누락/메모리 목표는 남으며 이번 수정에서 메모리를 재검증하지 않았다.

## 2026-10-11: 읽기 쉬운 요약과 단일 호버 안내

사용자 스크린샷의 요약에 PowerShell 변수·명령문이 그대로 표시됐고, 생략된 Label의 기본 툴팁과 명시적 안내가 동시에 열리는 원인을 확인했다. `Ran`은 알아볼 수 있는 첫 명령과 하위 명령을 짧은 활동명으로 변환하고 인수·경로를 요약/호버에 넣지 않는다. 용도를 알 수 없는 복합 스크립트는 `명령 실행`으로 표시한다. `Explored`는 파일·코드 확인, `Edited`는 파일 수정이다. 이 분류로 태스크 목적·성공·완료율을 만들지 않는다. 현재 태스크와 완료율은 계속 명시적 계획/직접 보고가 있어야 제공한다.

`Working (58s)`는 `작업 중 · 58초`, `Thinking (1m 24s)`는 `검토 중 · 1분 24초`로 표시한다. 진행률은 `33% · 1/3 완료`로 바꾸고 보고 출처는 호버에 유지한다. 제목과 모든 요약 행의 기본 생략 툴팁을 끄고 기존 전체 작업명·계획 안내 하나만 사용한다. 설정의 표시 항목 이름도 `최근 활동`으로 바꿨다.

새 로직 시험은 실제 VT의 활동 분류에서 원시 인수를 노출하지 않고 계획을 만들지 않는지, 상태/시간 변환에서 임의 문자열을 복사하지 않는지, 실제 요약 Label이 생략될 때 egui 툴팁 영역이 하나인지 검사한다. 로직 53개와 fmt·all-target Clippy `-D warnings`·release가 통과했다. `verify-ux.ps1 -SummarySample -ProgressSample`의 최신 결과는 `artifacts/ux-a61782d0-00ac-4620-9721-263707eb5745`다. 실제 창의 긴 한글 작업명에 Windows 포인터를 잠시 올려 툴팁 하나를 관찰했고 전체 작업명이 표시된 캡처를 검토했다. 포인터는 원래 위치로 복원하며 사람이 중간에 옮긴 경우 덮어쓰지 않는다. 초기 WM_MOUSEMOVE만 사용한 fixture는 툴팁이 뜨지 않아 호버 증거에서 제외했다.

다크/라이트·작은 창·명령 활동·계획 없는 요약·한글 직접 보고·진행률 갱신·더블 클릭 회귀도 통과했다. native 진단 모드에 툴팁 좌표만 기록하며 텍스트를 일반 IPC로 노출하지 않는다. 사람의 호버/스크롤·실제 모델의 계획 보고 검증은 남는다. 새 staging SHA256은 `ef01739494a2b609fce6ee46309d5234b84642d1e7fbbd8800f26e9b523cdc2d`이며 사용자 PID 5852/설치 파일 e625는 유지했다. 테스트 앱/Bash는 종료됐고 입력 누락·메모리 실패를 이번 결과로 해소했다고 표시하지 않는다.
