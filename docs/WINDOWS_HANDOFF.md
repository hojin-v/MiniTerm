# MiniTerm Windows 개발 인수인계 및 성능 계획

작성/수정: 2026-10-10 · P0 구현 진행 · 상위 계획: [PLAN.md](PLAN.md)

**현재 구현 기준:** 초기 Tauri/WebView2/xterm.js 제안은 사용자 성능 요구에 따라 **Rust 네이티브 egui/eframe + wgpu(DX12) + Alacritty VT 코어 + ConPTY**로 대체했다. 아래 Tauri/Node/WebView2 준비 항목은 초기 환경 조사 기록이다. 현재 Windows 빌드에는 Rust/MSVC/Windows SDK가 필요하며, Node와 WebView2는 필요하지 않다. 작업 기준 사본은 Windows NTFS의 `C:\path\to\MiniTerm`이다. 실제 진행 상태는 [IMPLEMENTATION.md](IMPLEMENTATION.md), 최신 성능 설계는 [PERFORMANCE.md](PERFORMANCE.md)를 확인한다.

## 1. 다음 작업에서 유지할 요구 사항

Windows가 주 플랫폼이다. 작업표시줄 위 작은 상주 HUD에서 한 번 클릭하면 입력하고, 헤더 더블 클릭으로 같은 터미널을 위쪽으로 확장한다. 셸과 실행 중인 에이전트는 확장·축소·숨기기 중 유지한다. CLI 에이전트 응답 완료 알림과 알림 클릭 시 해당 세션으로 복귀하는 기능을 제공한다. macOS Starboard를 상호작용 참고 자료로 사용한다.

추가 요구는 **MiniTerm의 입력 셸을 Bash 또는 Zsh로 제공하고, Windows 프로젝트의 성능을 고려하는 것**이다. 첫 버전은 Bash로 선택한다. PowerShell/cmd를 제품의 기본 셸이나 자동 대체 셸로 사용하지 않는다.

최신 계획은 **Windows 네이티브 앱 + Git for Windows Bash 기본 프로필 + 선택형 WSL2 Bash**다. Windows C:/D: 프로젝트를 주 대상으로 한 기본 가정이며, 실제 주 프로젝트가 WSL의 `/home`에 있다면 해당 프로젝트에는 WSL 프로필을 선택한다. WSL2 필수 의존성으로 단일화하지 않는다.

## 2. 셸 및 실행 환경 선택

| 선택 | 처리 |
| --- | --- |
| Bash와 Zsh 중 하나 | Bash. 기존 Bash 실행 파일을 호스팅하며 문법 해석기를 새로 만들지 않음 |
| Windows 파일의 프로젝트 | Git for Windows Bash + Windows용 개발 도구·CLI 에이전트 |
| Linux 파일의 프로젝트 | WSL2 Bash + Linux용 개발 도구·CLI 에이전트 |
| Zsh | 첫 버전의 검증 범위에서 제외, 후속 프로필 후보 |
| Git for Windows 미설치 | 설치 안내 또는 사용자가 Bash 경로 지정. PowerShell로 조용히 대체하지 않음 |
| WSL2 미설치 | 기본 프로필은 정상 실행. WSL 프로필에만 준비 안내 표시 |

Git for Windows는 MSYS2 기반이며 Bash 및 관련 도구를 제공한다. 첫 버전은 기존 설치를 재사용하고 Bash 런타임을 자체 배포물에 묶지 않는다. 일반 MSYS2까지 자동 탐색하는 기능은 기본 프로필 호환성 검증 이후로 미룬다. [Git for Windows](https://gitforwindows.org/), [기술 개요](https://gitforwindows.org/technical-overview.html)

“완전한 Bash 구현”은 Bash 구문·파이프·리다이렉션·스크립트·변수·완성·히스토리·작업 제어를 정확하게 호스팅하는 출시 목표다. Windows에서 모든 Linux 프로그램과 Linux 커널 기능이 동일하게 실행된다는 보장은 아니다. Linux 환경이 필요한 프로그램은 WSL 프로필에서 실행한다.

### Windows 기본 프로필

- 실행 파일: 설치된 Git for Windows의 `bin\bash.exe` 또는 검증한 동등 경로.
- 인자: `--login`, `--noprofile`, `-i`를 구조화된 인자 배열로 전달. 첫 프롬프트 직전 세션 전용 bootstrap에서 `/etc/profile`과 첫 사용자 로그인 파일을 정상 순서대로 한 번 읽은 뒤 프롬프트를 설정한다. 실제 로그인·대화형 플래그를 유지하며 초기화 파일을 수정하지 않는다.
- 호스트: MiniTerm의 ConPTY. `git-bash.exe`/Mintty를 실행해서 별도 창을 띄우지 않음.
- 시작 디렉터리: 선택한 Windows 프로젝트의 실제 경로. 로그인 초기화가 홈으로 이동시키지 않도록 `CHERE_INVOKING=1` 등을 해당 Git for Windows 버전에서 검증.
- 환경: Git for Windows 실행 환경을 따르되 Windows용 Node/Rust/에이전트가 올바르게 탐색되는지 확인. 사용자별 셸 초기화 파일·전역 PATH를 임의로 바꾸지 않음.
- 프로세스: 셸은 세션마다 한 번 실행. 명령마다 `bash -c`나 `wsl.exe`를 새로 실행하지 않음.

MSYS2는 Bash를 다른 터미널에 연결하는 실행 형태를 공식 문서에 제시한다. MiniTerm에서도 동일한 목적의 환경 설정과 TTY 동작을 검증한다. MSYS2의 ConPTY 지원 상태를 Git for Windows의 특정 버전에 그대로 적용했다고 가정하지 않는다. [MSYS2 터미널 연동](https://www.msys2.org/docs/terminals/)

### WSL 선택 프로필

WSL2 배포판과 Linux 사용자를 선택하고 그 안의 `/bin/bash`를 직접 실행한다. 호스트 쪽 프로세스는 `wsl.exe`이며, 앱 시작만으로 WSL을 기동하지 않는다. WSL 프로필을 처음 선택할 때 세션을 만든다.

현재 확인한 배포판을 사용하는 수동 실행 예시는 아래와 같다. 다른 PC에서는 `--distribution` 값이 설치된 배포판 이름이어야 한다.

```text
wsl.exe --distribution Ubuntu-24.04 --cd ~ --exec /bin/bash --login -i
```

구현에서는 실행 파일·인자를 배열로 전달한다. 설정에 저장한 Linux 작업 디렉터리를 `--cd`에 전달하며, `/home/<user>/projects` 등 Linux 파일시스템을 기본으로 사용한다. 사용자의 시스템 기본 배포판을 변경하지 않는다. [WSL 명령 문서](https://learn.microsoft.com/en-us/windows/wsl/basic-commands)

## 3. `/mnt` 문제를 피하는 저장 위치

WSL2 사용 자체가 `/mnt/c` 사용을 강제하지 않는다. 비용은 파일을 어느 실행 환경에서 읽는지에 따라 달라진다. Microsoft는 Linux 도구를 쓸 때 WSL 파일시스템에, Windows 도구를 쓸 때 Windows 파일시스템에 프로젝트를 두도록 권고한다. [Microsoft 파일시스템 권고](https://learn.microsoft.com/en-us/windows/wsl/filesystems)

| 프로젝트/도구 | 권장 위치 | Bash에서 보이는 경로 | 처리 |
| --- | --- | --- | --- |
| Windows 도구로 MiniTerm 개발 | `C:\Projects\miniterm` | Git Bash의 `/c/Projects/miniterm` | 기본 개발 경로 |
| Windows 에이전트로 일반 프로젝트 작업 | `C:\Projects\app` 또는 D: 드라이브 | Git Bash의 `/c/Projects/app` 또는 `/d/...` | 기본 제품 프로필 |
| Linux 에이전트로 Linux 프로젝트 작업 | WSL의 `/home/<user>/projects/app` | 같은 Linux 경로 | WSL 제품 프로필 |
| Linux 도구가 NTFS 프로젝트를 처리 | `C:\Projects\app` | WSL의 `/mnt/c/Projects/app` | 비교 측정 대상, 기본값으로 사용하지 않음 |
| Windows 도구가 WSL 프로젝트를 처리 | `\\wsl.localhost\<distro>\home\...` | Windows 네트워크 형태 경로 | 기본 빌드·검색·감시 경로로 사용하지 않음 |

Git Bash의 `/c/...`는 MSYS 경로 표기이며 WSL의 `/mnt/c/...`와 다르다. Windows용 Node/Git/Rust는 Windows 파일을 직접 다루게 한다. Linux용 도구와 Windows용 도구를 같은 프로필에서 무분별하게 섞지 않는다. `node_modules`, Rust `target`, Python 가상환경과 캐시도 실행 환경별로 구분한다. [MSYS2 경로 변환](https://www.msys2.org/docs/filesystem-paths/)

MiniTerm은 프로젝트를 자동 복제하거나 Windows/WSL 사이에서 실시간 동기화하지 않는다. 처음 지정한 위치를 유지하고 사용자가 실행 프로필을 선택한다. 위치와 프로필이 맞지 않을 때 설정 화면에서 적합한 프로필을 안내한다. 이미 사용자가 선택한 WSL/NTFS 조합을 자동으로 바꾸지 않는다.

현재 Linux 프로젝트와 테스트 데이터를 WSL `/home`에 유지하는 것은 성능 계획과 일치한다. Windows 앱 빌드용 작업 사본만 Windows NTFS에 준비한다.

## 4. 다음 개발은 Windows 네이티브 환경에서 진행

앱 구현과 Windows 검증은 **Windows 네이티브 Rust/MSVC + 로컬 NTFS 작업 폴더**에서 진행한다. HUD 포커스, 한글 IME, 모니터별 DPI, ConPTY, 트레이, 설치본 알림을 실제 제품과 같은 환경에서 확인하기 위해서다.

WSL에는 Linux 프로필 검증용 도구와 데이터를 둔다. WSL Linux용 Rust/Node를 설치했다고 Windows 빌드 도구가 준비된 것으로 취급하지 않는다. 현재 네이티브 앱의 개발에는 Microsoft C++ Build Tools, MSVC Rust 툴체인, Windows SDK가 필요하다. Node·WebView2는 앱 빌드/실행 조건이 아니다.

### 현재 환경에서 확인한 내용

2026-10-10의 읽기 전용 확인 결과다. 새 환경에서는 다시 확인한다.

| 항목 | 확인 결과 |
| --- | --- |
| 현재 작업 위치 | WSL의 `/home/dev/projects/miniterm` |
| WSL 버전 | 2.6.3.0 |
| Windows 빌드 | 10.0.26200.9457 |
| 현재 Linux 배포판 | Ubuntu-24.04, WSL2 |
| Linux Bash | 5.2.21 |
| Windows Bash 실행 파일 | `C:\Program Files\Git\bin\bash.exe` |
| Windows Bash | 5.2.37, Windows 실행 파일의 버전 출력 성공 |
| Windows Git | 2.50.1.windows.1, 버전 출력 성공 |
| Windows Rust/MSVC/Node | 준비 상태 미검증 |
| 앱 구현 | 아직 없음. 계획 문서와 Starboard 참고 소스만 있음 |
| Git 상태 | MiniTerm 루트는 아직 Git 저장소가 아님. `starboard/`는 별도 Git 저장소 |

이 확인은 실행 파일 존재와 버전 조회까지다. ConPTY 연결, Bash 작업 제어, HUD, 에이전트 TUI, 성능은 아직 검증하지 않았다.

### 작업 폴더 옮기기

Windows 탐색기에서 현재 프로젝트를 열 수 있는 경로는 다음과 같다.

```text
\\wsl.localhost\Ubuntu-24.04\home\dev\projects\miniterm
```

`docs/`와 `starboard/`를 포함한 프로젝트를 `C:\Projects\miniterm` 등 로컬 NTFS 폴더로 복사한다. 이 복사는 개발 환경 전환을 위한 한 번의 작업이며 제품의 자동 동기화 기능이 아니다. 이후 앱 작업의 기준 사본은 Windows 폴더로 정하고 두 사본을 동시에 수정하지 않는다.

Windows에서 프로젝트를 다시 열고 먼저 이 문서와 `docs/PLAN.md`를 읽는다. 참고용 Starboard는 수정 대상이 아니다.

### 준비 및 확인

1. Git for Windows 설치를 확인하고 Bash 경로를 기록한다.
2. Microsoft C++ Build Tools에서 **Desktop development with C++**와 Windows SDK를 준비한다.
3. Windows용 Rust의 `x86_64-pc-windows-msvc` 툴체인을 준비한다.
4. DX12 지원 그래픽 드라이버를 확인한다.
5. Node/npm은 CLI 에이전트 등 Windows 개발 도구에 필요할 때만 준비한다.
6. Codex/Claude Code를 테스트할 실행 환경에 설치하고 로그인한다. Windows 설치와 WSL 설치는 별개다.

초기 준비 확인용 명령은 Windows 터미널에서 실행한다. 아래의 Windows 관리 도구 사용과 MiniTerm 제품의 Bash 셸 요구는 서로 다른 범위다.

```text
git --version
rustc --version
cargo --version
rustup show
node --version
npm --version
```

선택형 WSL 프로필을 검증할 때만 다음 명령으로 배포판과 버전을 확인한다. 현재 PC에는 WSL2 배포판이 있으므로 WSL을 다시 설치할 필요가 없다.

```text
wsl.exe --version
wsl.exe --list --verbose
```

이 명령들은 새 작업 환경에서 수행할 절차이며 현재 세션에서 개발 도구 설치나 시스템 설정 변경을 진행하지 않았다.

## 5. 에이전트와 알림 연결

Windows Bash에는 Windows용 CLI 에이전트를, WSL Bash에는 Linux용 CLI 에이전트를 연결한다. Windows/WSL의 인증·설정 경로를 하나라고 가정하지 않는다. 동일한 세션 안에서 프롬프트 입력, TUI 표시, Ctrl+C와 이벤트 발생을 검증한다.

Claude Code는 Windows에서 Git Bash 경로 지정과 Bash 도구 사용을 지원한다. `CLAUDE_CODE_GIT_BASH_PATH` 등 공식 설정을 지원 버전에서 확인한다. Codex도 Windows 네이티브 CLI 사용이 가능하지만, 에이전트 내부 명령 실행 셸은 MiniTerm의 입력 셸과 별도다. MiniTerm이 Bash라는 이유로 모든 에이전트 내부 명령까지 자동으로 Bash가 된다고 표시하지 않는다. [Claude Code Windows 설정](https://code.claude.com/docs/en/setup#install-on-native-windows), [OpenAI Docs: Windows](https://learn.chatgpt.com/docs/windows/windows-sandbox)

기본 알림 경로는 Windows 에이전트 → Windows 이벤트 실행 파일 → 사용자별 로컬 IPC → HUD/Windows 알림이다. 파일이나 프로세스를 짧은 주기로 계속 스캔하는 방식 대신 기존 계획의 공식 이벤트를 사용한다.

WSL 프로필에서는 세션 식별자를 Linux에 전달하고 Linux 어댑터가 Windows 이벤트 실행 파일을 호출한다. `WSLENV`와 interop가 제한된 경우를 구별하며, 이때 Bash 실행과 알림 연결 상태를 따로 표시한다. 기본 Windows 프로필은 이 경로에 의존하지 않는다. [WSL 환경 변수 전달](https://learn.microsoft.com/en-us/windows/wsl/filesystems#share-environment-variables-between-windows-and-wsl-with-wslenv), [WSL interop 설정](https://learn.microsoft.com/en-us/windows/wsl/wsl-config#interop-settings)

## 6. 성능 검증 계획

### 비교 조합

| ID | 셸/프로그램 | 저장 위치 | 목적 |
| --- | --- | --- | --- |
| A | Git for Windows Bash + Windows 도구 | Windows 로컬 NTFS | 기본 프로필 후보 |
| B | WSL2 Bash + Linux 도구 | WSL Linux 파일시스템 | Linux 환경과 대량 작업 후보 |
| C | WSL2 Bash + Linux 도구 | `/mnt/c`의 NTFS | 사용자가 우려한 파일시스템 경계 비용 확인 |

A가 B보다 모든 작업에서 빠르다고 가정하지 않는다. A/C 비교는 실행 환경까지 다르므로 결과를 순수 파일시스템 차이로 해석하지 않는다. **B/C는 같은 WSL 배포판과 Linux 도구 버전으로 측정**하여 파일 위치에 따른 비용을 비교한다. A는 Windows 네이티브 작업에 적합한지 평가한다.

각 조합에서 MiniTerm과 Windows Terminal이 **같은 셸·초기화 파일·도구·데이터**를 쓰도록 맞춘다. 이 비교로 MiniTerm의 터미널/UI 추가 비용을 분리한다.

### 측정 항목

- HUD: 이미 실행 중인 세션의 클릭 → 포커스 → 첫 입력 반영, 확장/축소 중 입력 손실 여부.
- 셸: 최초 시작과 실행 중인 세션 재활성화, WSL 기동과 배포판 시작 비용.
- 프로세스: 짧은 외부 명령 반복 및 프로세스를 많이 만드는 Bash 스크립트. Bash builtin만 사용한 결과와 분리.
- 저장소: 대표 프로젝트의 `git status`, 전체 파일 검색, 소규모 변경 뒤 재검색.
- 개발 도구: 의존성 설치, 최초 빌드와 변경 후 빌드, 파일 감시 이벤트 지연.
- 터미널: 대량 출력 중 입력 반응·출력 속도·메모리 증가.
- 자원: 네이티브 앱, GPU 메모리, Bash 런타임, 에이전트, 선택형 WSL VM·Linux 프로세스를 구분한 CPU/메모리.

파일 수가 많은 대표 저장소와 합성 데이터셋을 준비하고 **복사/준비 시간은 작업 측정에서 제외**한다. 같은 파일 내용·저장소 상태를 사용한다. CPU/RAM/SSD, OS·WSL·Bash·도구 버전, 배포판, 파일 수, 초기화 파일, 백신 상태를 기록한다. 백신을 임의로 비활성화하거나 제외 경로를 설정하지 않는다.

입력 지연은 100회 이상의 표본으로 p95를 기록한다. 저장소/빌드 작업은 최소 10회 반복하여 중앙값과 범위를 기록한다. cold/warm 결과를 구분하고 네트워크 다운로드, 도구 캐시, WSL 기동 비용을 작업 시간과 섞지 않는다. 의존성 설치는 동일한 캐시/네트워크 조건끼리 비교한다.

### 초기 통과 기준

| 항목 | 목표 |
| --- | --- |
| 기존 세션 클릭 후 입력 가능 | p95 100ms 이내 |
| 로컬 입력 반영 | p95 50ms 이내, 한글 첫 글자 누락 없음 |
| 기본 프로필 시작 | Windows Terminal의 같은 Bash 구성 대비 MiniTerm 추가 준비 지연 300ms 이내 목표 |
| 일반 Git/검색/빌드 | 같은 프로필의 Windows Terminal 대비 중앙값 추가 비용 10% 이내 목표. 아주 짧은 작업은 절대 시간도 함께 평가 |
| 앱 유휴 자원 | 네이티브 앱 CPU 평균 0.5% 이하, 메모리 200MB 이하 목표. GPU 메모리는 별도 기록 |
| 세션 수명 | 창 크기 변경·숨기기에서 프로세스 재시작 없음 |
| 기본 프로필 독립성 | WSL 비활성/미설치 환경에서도 동작, 앱 시작 시 WSL 기동 없음 |

수치는 아직 실측 결과가 아닌 초기 목표다. 측정 장치와 결과를 기록한 뒤 현실적인 지원 기준을 확정한다. 사용자 셸 플러그인과 CLI 에이전트의 비용을 MiniTerm 자체 비용으로 오해하지 않도록 항목을 분리한다.

### 구현 시 성능 원칙

세션을 유지하고 클릭마다 셸을 만들지 않는다. 추가 세션과 WSL은 사용 시 생성한다. 애니메이션 중 화면만 이동하고 PTY 행/열 갱신은 끝에서 확정하여 TUI의 반복 재그리기를 줄인다. 실제 사용자 크기 조절은 적절히 묶어 처리한다.

PTY 출력은 Rust VT 코어에서 bounded chunk로 처리하고 UI에는 visible snapshot을 제공한다. HUD에 보이지 않는 출력도 계속 소비하되 숨겨진 세션을 다시 그리지 않는다. 대량 출력 중 UI와 입력이 멈추지 않게 한다.

알림은 이벤트로 처리하고, 모니터/작업 영역 변경은 Windows 이벤트에 맞춰 처리한다. 유휴 상태의 무의미한 파일 전체 검색과 촘촘한 프로세스 폴링을 피한다. 프로젝트 파일 접근은 선택한 실행 환경의 도구가 담당한다.

## 7. Windows에서 재개할 첫 작업

1. 이 문서와 `PLAN.md`를 읽고 Windows NTFS의 기준 작업 폴더 및 Windows 개발 도구를 확인한다.
2. Rust 네이티브 GPU HUD + Alacritty VT 코어 + ConPTY에 Git for Windows Bash 한 세션을 연결한다.
3. Bash 문법/입출력, 시작 디렉터리, 한글 IME, TTY, Ctrl+C, Ctrl+Z/jobs/bg/fg 및 에이전트 TUI를 검증한다.
4. 같은 조건의 Windows Terminal과 비교하여 UI·셸 실행 추가 비용을 측정한다.
5. 선택형 WSL Bash에서 B/C 파일 위치 비교와 알림 연결을 검증한다.
6. Windows 설치본에서 완료 알림과 해당 세션 복귀를 확인한다.
7. 통과한 Bash/에이전트 버전과 성능 결과를 문서에 기록한 뒤 다중 세션·설정 UI를 확장한다.

현재 남은 작업은 구현과 실제 성능 검증이다. Windows Bash의 핵심 호환성이 미달이면 문제를 고친 뒤 MSYS2의 다른 구성과 비교한다. WSL을 필수로 바꾸거나 사용자 프로젝트를 옮기는 결정은 측정 근거 없이 하지 않는다.

다음 작업에 사용할 요청 문장:

> docs/PLAN.md와 docs/WINDOWS_HANDOFF.md를 읽고 MiniTerm의 P0 구현을 진행해 주세요. Windows 네이티브 환경에서 Git for Windows Bash를 ConPTY에 연결하고, WSL 없이 기본 동작하게 해 주세요. Windows 프로젝트는 NTFS에서, 선택형 Linux 프로젝트는 WSL /home에서 처리하며, Windows Terminal 기준선과 성능·Bash 호환성·한글 입력·알림 복귀를 먼저 검증해 주세요.
