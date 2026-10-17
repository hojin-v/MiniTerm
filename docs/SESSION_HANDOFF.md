# 세션 인계 기준

> 2026-10-16 alpha.7 사용자 후속: [Codex 연결·설정창 글자·확장 후 스크롤](../feature/CODEX_SETTINGS_FIX_2026-10-16.md). 추가 연결 직렬 대기, 공유 글꼴 atlas 재생성, 축소/확장 뒤 정지한 커서의 휠 무시를 재현해 수정했다. 일반 97개와 실제 GUI의 연속 응답 중 입력·스크롤·취소 후 재요청·재개가 통과했다. 최신 로컬 EXE는 `4f58251e…`; 사용자 설치본과 공개 릴리스는 자동 교체하지 않는다. 정확한 read-only 문구의 사용자 실행 순서, Git resize 입력 누락 4건, 기본 브리지 활성 시 약 210MB 관측은 남는다. 실행 파일·설치 파일·검증 범위는 해당 기록을 따른다.

> 2026-10-16 성능 후속: [메모리·종료·응답 경로 보완](../performance/STABILITY_2026-10-16.md). Ghostty의 bounded queue·짧은 잠금·byte/grapheme 상한을 참고해 콘솔 보호·파이프 종료·자동 응답·history/결합 문자·글꼴·Codex 중계를 보완했다. 일반 96개·별도 ConPTY 1개·release·18만 행 출력·1/4세션 GUI·공유 Codex 0.163.0 회귀가 통과했다. 1/4세션 유휴 최대 working set은 각각 150.1/152.9MB다. 검증한 소스와 현재 소스의 일치를 확인하고 `artifacts/miniterm.exe`를 `d0e6d9f5…`로 갱신했다. 기능 세션 사본은 유지하며 기존 Git resize 입력 누락 4건과 사고의 정확한 트리거는 남는다. 최종 파일·해시·검증은 안정성 기록을 따른다.

> 2026-10-16 기능 후속: 공유 Codex 알림의 임시 대화 간섭·완료 배지 변경·호출 폴더를 수정했다. 일반 88개와 별도 release의 native 통합 검사(탭 이동 재개·이력·Bash·R·콜백·서버 유지)를 통과했다. 실행 파일·해시·실기 미완료 범위는 [현재 인계](../feature/CODEX_RECOVERY_HANDOFF.md)와 [공유 알림 기록](../feature/SHARED_NOTIFICATIONS.md)을 따른다. 성능 세션의 이후 변경은 이 기능 사본에 통합하지 않았으며 공용 실행 파일도 교체하지 않았다.

> 2026-10-15 기능 재개: 공유 Codex의 탭별 알림을 추가했다. 최신 기능 사본·검증 범위는 [현재 인계](../feature/CODEX_RECOVERY_HANDOFF.md)와 [공유 알림 기록](../feature/SHARED_NOTIFICATIONS.md)을 따른다. 같은 폴더에서 성능 세션도 소스를 수정 중이므로 기능 실행 파일은 별도 사본으로 보존한다. 사용자 지시에 따라 배터리·팬 등 성능 진단/해결은 성능 세션이 맡으며 기능 검사 통과를 배터리 문제 해결로 표시하지 않는다.

> 2026-10-14 현재: 공유 Codex의 첫 입력·스크롤·같은 대화 재개·R 복구를 release GUI에서 확인했다. 설정창 모서리·UI 글꼴 수정도 반영했다. 기본 공유 Codex의 탭별 알림과 사용자 실제 입력 확인은 남는다. [현재 작업 인계](../feature/CODEX_RECOVERY_HANDOFF.md), [실행 구조](../feature/CODEX_STARTUP.md), [배터리 재검증](../performance/BATTERY_DIAGNOSIS_2026-10-13.md)이 아래 중단 당시 설명보다 우선한다.

> 2026-10-14 배터리 진단: [배터리 급감 재검증](../performance/BATTERY_DIAGNOSIS_2026-10-13.md). 관리자 SRUM·WER 발생 시각으로 과거 GPU 보고의 재기록을 확인했고 초반 Explorer·후반 WSL의 전력 사용을 구분했다. 최신 기능 실행 파일과 통과/미완료 범위는 [현재 인계](../feature/CODEX_RECOVERY_HANDOFF.md)의 맨 위 갱신을 따른다.

> 2026-10-13 17:14 KST 중단: 사용자 요청으로 작업을 중단했다. 최신 상태는 [Codex 입력·재개·Bash 연결 인계](../feature/CODEX_RECOVERY_HANDOFF.md)를 먼저 따른다. 기본 Bash·공유 대화 재개는 격리 CLI에서 통과했지만 공유 서버 GUI의 첫 입력 전송은 미완료다. 아래 스크롤 완료 및 실행 파일 안내는 당시 기록이며 현재 소스의 최종 검증을 뜻하지 않는다.

> 2026-10-13 최신 기능 후속: [Codex 스크롤 수정](../feature/SCROLL.md). 최근 실행 파일의 스크롤 실패를 재현하고, 출력 기록 보존과 확장/축소 입력 화면을 수정했다. 최종 로컬 실행 파일·검증·적용 범위는 이 기록을 따른다.

> 후속 안내: 2026-10-12 사용자가 `feature/feature.md`의 기능·디자인 구현을 요청해 해당 작업을 재개했다. 최신 로컬 산출물·검증·남은 확인은 [기능 구현 결과](../feature/IMPLEMENTATION.md)를 따른다. 아래 문서는 20:30 KST 당시 공개 제품과 조사 근거를 유지한 기록이다.

> 성능 후속: 사용자가 `performance/performance.md` 구현을 요청해 CLI 에이전트의 Bash 실행 연결을 재개했다. 해당 변경·검증·빌드·남은 확인은 [성능 요구사항 구현 결과](../performance/IMPLEMENTATION.md)를 따른다.

2026-10-12 20:30 KST 기준. 새 세션은 이 문서를 먼저 읽고 담당 영역의 문서로 이동한다. 긴 대화나 과거 실험 전체를 다시 읽을 필요는 없다. 제품 구현 목표는 **일시중지** 상태이며 이번 인계에서는 문서 확인과 영역 분리만 수행한다.

## 지금의 기준

| 항목 | 확인한 상태 |
| --- | --- |
| 작업 위치 | Windows PowerShell, `C:\Users\ghwls\00_Projects\miniterm` |
| 소스 | `main`, HEAD `17cbfc701f5cdd533857e79a54de541a7dc4d4eb`. 로컬 `origin/main`과 일치 |
| 공개 제품 | [v0.1.0-alpha.6](https://github.com/hojin-v/MiniTerm/releases/tag/v0.1.0-alpha.6), prerelease, 자산 10개 |
| 공개 소스 | `09d7c01780be6b091597030e07f0474f3b5b144e` |
| 기술 구성 | Rust, egui/eframe, wgpu/DX12, Alacritty VT, Windows ConPTY. 초기 Tauri 계획을 대체함 |
| 기본 셸 | 설치된 Git for Windows Bash. 선택형 WSL Bash. 전용 런타임 선택 코드는 포함하지만 수정 DLL은 배포하지 않음 |
| WinGet | ID `Hnode.MiniTerm`, Publisher `Hnode`, Moniker `miniterm`. [PR #450836](https://github.com/microsoft/winget-pkgs/pull/450836) OPEN, 일반 PR, 미병합 |
| Microsoft 검사 | 조회 시 `Azure-Pipeline-Passed`, `Validation-Completed` 라벨과 CLA 통과. 승인·병합·카탈로그 반영은 별도 |
| 사용자/시험 프로세스 | 조회 시 `miniterm.exe`, `verify-bash-conpty.exe`, `verify-bash-selection.exe` 없음. 재개할 때 새로 확인할 것 |

구현은 완료로 표시하지 않는다. 알파 배포 성공과 터미널 호환성·성능 목표 달성은 각각 확인한다. GitHub/프로세스 상태는 위 시점의 관찰이다.

## 반드시 유지할 사용자 요구

- 가벼운 네이티브 상주 터미널. 확장·축소·숨김에서도 같은 셸과 실행 중인 에이전트가 유지돼야 한다.
- 상단은 새 탭·입력/요약 전환·숨기기 세 아이콘과 세션 제목. 확장·고정·알림·설정 버튼을 다시 늘리지 않는다. 설명은 호버로 제공한다.
- 본문 더블 클릭은 위치에 관계없이 확장/축소. 요약 단일 클릭은 포커스만 이동. 같은 폭을 유지하며 위쪽으로 부드럽게 확장한다.
- 마우스 이동·크기 조절, 프로그램이 정한 작은 DWM 모서리, 투명도, 다크/라이트 즉시 표시. 곡률의 사용자 수치 설정을 다시 넣지 않는다.
- 한글·영문 기준선과 선택 배경, ANSI 색·볼드·이탤릭·기호 폴백, GUI 글꼴 선택. 프롬프트는 한 줄이고 사용자/호스트/경로 표시와 별칭을 GUI로 조절한다.
- 요약은 현재 태스크·명시된 계획의 완료 항목 비율·실행 상태가 중심이며 AI Battery를 하단에 표시한다. 내부 명령문, Bash 줄, Codex `/status` 안내를 작업 요약으로 표시하지 않는다.
- Codex/AI Battery의 입력·재개 메뉴·알림을 실제 동작으로 검증한다. 게시자는 Hnode이며 GitHub 저장소는 `hojin-v/MiniTerm`을 유지한다.

## 기능·디자인 / 성능 두 영역으로 인계

영역별 시작 문서는 [기능·디자인 통합 인계](areas/DESIGN_FUNCTIONALITY.md)와 [성능 인계](areas/PERFORMANCE.md)다. 각 문서에 우선순위·완료 조건·공유 파일 경계와 새 Codex 세션에 전달할 시작 프롬프트를 담았다.

| 세션 | 먼저 읽을 문서 | 담당 코드 | 우선 해결할 결과 |
| --- | --- | --- | --- |
| 기능·디자인 통합 | [MINI_SUMMARY.md](MINI_SUMMARY.md), [IMPLEMENTATION.md](IMPLEMENTATION.md), [BASH_RUNTIME.md](BASH_RUNTIME.md), [RESIZE_INPUT_INVESTIGATION.md](RESIZE_INPUT_INVESTIGATION.md), [MSYS_RUNTIME_VALIDATION.md](patches/MSYS_RUNTIME_VALIDATION.md), [NOTIFICATIONS.md](NOTIFICATIONS.md), [SCROLL_BEHAVIOR.md](SCROLL_BEHAVIOR.md) | `app.rs`의 표시·입력 UI, `icons.rs`, `palette.rs`, `geometry.rs`, `native.rs`, `pty.rs`, `terminal.rs`, `shell.rs`, `runtime.rs`, `prompt.rs`/`prompt.bash`, `agent.rs`, `mini.rs`, `progress.rs`, `integration.rs`, `ipc.rs`, `notification.rs`, `tests/bash_conpty.rs` | UI 동작과 터미널 호환성을 함께 재현. 입력 누락·스크롤, 실제 호버/IME/혼합 DPI, 진행 보고·알림/WSL의 완료 조건 |
| 성능 | [PERFORMANCE.md](PERFORMANCE.md), [STATUS.md](STATUS.md), [PLAN.md](PLAN.md)의 인수 목표 | `app.rs`의 캐시/그리기, `terminal.rs`/`pty.rs`의 처리 비용, `scripts/measure-idle.ps1` | 공개 빌드 200MB 초과 원인의 근거, 출력 부하·GPU/셸 자원, 입력→pixel 계측과 동일 환경 기준선 |

각 세션의 첫 결과는 담당 영역의 **완료/미해결/근거/다음 작업/완료 조건**을 담은 짧은 인계 문서다. 전체 대화는 전달하지 않고 이 문서와 해당 영역 자료로 시작한다. 공통 배포 상태는 [STATUS.md](STATUS.md), [WINGET.md](WINGET.md), [DISTRIBUTION.md](DISTRIBUTION.md)를 따른다.

`app.rs`, `native.rs`, `settings.rs`, `pty.rs`, `terminal.rs` 등은 영역이 겹친다. 세션 분리는 코드 소유권의 완전 분리를 뜻하지 않는다. 기능·디자인의 동작은 통합 세션이, 계측·자원 목표는 성능 세션이 맡는다. 이번 두 세션은 문서 진단만 수행하고 제품 코드는 동시에 수정하지 않는다. 구현 재개 시 공유 파일 변경은 한 세션씩 조정하고 통합 담당자가 diff와 영향을 확인한다. 배포·릴리스·PR 수정은 영역별 세션이 독립적으로 수행하지 않는다.

## 검증 결과와 적용 범위

### 공개 alpha.6

- [Release CI 38207963396](https://github.com/hojin-v/MiniTerm/actions/runs/38207963396): `09d7c017…`, completed/success.
- [WinGet CI 38208455393](https://github.com/hojin-v/MiniTerm/actions/runs/38208455393): `17cbfc701…`, completed/success. 무인 설치·로딩·제거·설정 보존의 검사이며 Microsoft 카탈로그 공개를 뜻하지 않는다.
- 설치 파일 SHA256: `5e1a80beff9a6af201876a0ee554977d5df19dd7b3694391602a93eb52fad4fb`.
- 실행 파일 SHA256: `f94f402b89177766381e991fa288676c9a1fb6f0eb4e89cb2f59d7ccab765cb4`.
- 공개 `COMPATIBILITY.json`을 다운로드해 GitHub digest `f3d3b9aca1752dfdf57554f1c6bccb2106c7e927723c517b63b09f430761e4db`와 일치 확인. Git `2.55.0.windows.5`, **4개 통과·4개 실패**, `allPassed=false`, `acceptedForAlpha=true`. 실패는 managed prompt typeahead, raw typeahead, Session 가로/세로 및 세로 전용 resize 검사다. 허용 실패가 수정 완료를 뜻하지 않는다.
- 다운로드 기록: `artifacts/context-handoff-dd208229d75b4984977dedb55825c565/COMPATIBILITY.json`. 기존 CI 증거: `artifacts/alpha6-ci-evidence/`.
- alpha.6의 200MB 성능 목표 달성은 확인하지 않았다. 공개 alpha.3의 4세션 최대 working set은 210,030,592bytes로 실패했다. 다른 로컬 실행 파일의 185MB 통과 결과를 공개 제품에 대입하지 않는다.
- [PLAN.md](PLAN.md)의 기본 자원 목표는 1세션 기준이며 4세션은 확장 시나리오로 구분한다. `measure-idle.ps1` 결과는 자원 집계이고 `metrics.json`의 UI CPU p95는 입력→pixel이나 GPU present 지연이 아니다.

### 개발용 MSYS 후보

- 패치 DLL `bd6674212e79904dc20827603f25253b46bd7321a5ce4126889ee3030d27281b`, Git `2.50.1.windows.1`, 소스 `7674c51e18b9323c0072c8299c9356a7d9511a4e`. 해시·다운로드·패치 기준은 [`runtime/BASH_RUNTIME.json`](../runtime/BASH_RUNTIME.json).
- raw/canonical 각각 1,000회와 SIGWINCH/siglongjmp, 기본 경로 연동 11개 및 GUI 4세션의 통과 기록이 있다. 설치된 Git DLL은 덮어쓰지 않았다.
- Unicode/공백 폴더의 최신 11개 결과: `artifacts/runtime integration 한글-f4cccf1f2a274a308257e62ccbd35283/verification-25dbf7f4633940e1bee0d31c60414b79`. 앞선 Codex SSE 실패와 개별 재검사도 해당 폴더에 보존돼 있다. fixture의 idle 연결 차단 회귀를 수정한 결과이며 실제 계정/모델 통과가 아니다.
- GUI 결과: `artifacts/ux-78519338-e367-4fa5-9222-8da7053b5e48`, 개발 exe `fa322901…`, 최대 working set 185,511,936bytes. 최초 툴팁 관찰 실패의 원인은 미확정이다.
- 후보는 `publicPackagingApproved=false`. 최신 지원 Git 재빌드·전체 잡 제어/부하·대응 소스와 빌드 절차·NSIS/ZIP 설치/업데이트/제거/복구가 남는다. 과거 후보를 최신 지원 런타임으로 간주하지 않는다.

### 요약과 스크롤

- 직접 계획 보고의 비율은 완료 항목 수/전체 항목 수다. 자동 시간 예측이나 모든 CLI의 태스크 수집이 아니다. [miniterm-progress skill](../.agents/skills/miniterm-progress/SKILL.md)은 프로젝트에만 있으며 실제 모델의 선택·갱신 이행은 미검증이다.
- 로컬 제공자/고정 배터리 시험과 실제 계정·도구 출력·팝업 클릭은 구분한다. 실제 IME·혼합 DPI·WSL 서비스/훅·알림 클릭·출력 부하·입력→pixel·기준 터미널 비교는 미완료다.
- 현재 휠 입력 분기는 `src/app.rs`에 있다. `ALT_SCREEN`에서 방향키로 전달하는 동작과 Windows Codex 래퍼의 `--no-alt-screen`은 코드에서 확인한다. [SCROLL_BEHAVIOR.md](SCROLL_BEHAVIOR.md)의 Shift 우회가 모든 대체 화면에서 기록을 복원한다는 보장은 실기로 확인해야 한다. 조사 문서를 수정 완료 기록으로 읽지 않는다.

## 이어받을 때의 절차와 작업 경계

1. `git status --short --branch`와 이 문서의 기준 커밋을 비교한다. 인계 점검 시작 전에 이미 `README.md`, `docs/IMPLEMENTATION.md`, `docs/STATUS.md`, `docs/TESTER_GUIDE.md`, `docs/WINGET.md` 변경과 미추적 `docs/SCROLL_BEHAVIOR.md`가 있었다. 기존 변경을 지우거나 임의로 되돌리지 않는다. 이번 인계 문서도 커밋 전에는 같은 작업 폴더에서만 공유된다.
2. 담당 영역 문서만 읽고 첫 재현/산출물과 완료 조건을 정한다. 같은 바이너리·같은 조건의 통과 검사를 이유 없이 반복하지 않는다. 실패 재시도는 원인 가설이나 변경점이 있을 때 수행하고 실패 기록도 남긴다.
3. 실행 파일 SHA, 소스/셸 버전, 데이터·IPC 경로를 기록하고 시험 소유 프로세스만 관리한다. 기존 사용자 앱·Codex 세션·초기화 파일·인증·전역 Git/WSL 설정을 바꾸지 않는다.
4. 실제 변경에 맞는 검사만 수행한다. PowerShell 5.1에서는 Cargo 인자를 [`scripts/cargo.ps1`](../scripts/cargo.ps1)의 `-CargoArguments` 배열로 전달한다. 새 빌드가 필요한 경우 `scripts/build.ps1`; 검사는 `scripts/test.ps1`과 영역별 도구를 사용한다. 이 인계에서는 빌드나 제품 검사를 다시 실행하지 않았다.
5. GUI 시험은 같은 데스크톱의 마우스·포커스·창을 사용하므로 여러 세션에서 동시에 돌리지 않는다. 오래된 PID/캡처 파일을 현재 실행 상태로 가정하지 않는다.
6. 산출물에는 바뀐 내용, 근거 파일/해시, 통과 범위, 남은 위험, 다음 한 작업을 남긴다. 유의미한 진전 없이 반복되는 경우 기록을 정리하고 종료하라는 사용자 요청을 유지한다.

## 문서의 역할과 오래된 기록

- 최신 진입점은 이 문서, 제품·배포 요약은 [STATUS.md](STATUS.md), 전체 추적 목록은 [WORK_CHECKLIST.md](WORK_CHECKLIST.md).
- [IMPLEMENTATION.md](IMPLEMENTATION.md), [MATURITY.md](MATURITY.md), [WINDOWS_HANDOFF.md](WINDOWS_HANDOFF.md)에는 과거 단계·환경·PID·검사 개수가 남아 있다. 역사 기록이며 현재 제품 상태를 덮어쓰지 않는다.
- `artifacts/`, `.tools/`, `screenshot/`, DLL/EXE 등은 Git에서 제외된다. 다른 PC/새 clone에는 원시 기록과 빌드 SDK가 없다. 위 경로의 핵심 증거를 별도로 전달하거나 해당 조건으로 새 검증해야 한다. 공개 CI/Release와 체크인된 해시·패치는 저장소 밖에서도 조회 가능하다.
- `artifacts/WINGET_SUBMISSION.json`은 alpha.3의 이전 기록이다. 새 제출이나 PR 상태 판단에 그대로 사용하지 않는다. 실제 Microsoft PR과 현재 `winget/manifests/h/Hnode/MiniTerm/0.1.0-alpha.6` 파일을 확인한다.
