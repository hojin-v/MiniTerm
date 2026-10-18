# 기능·디자인 통합 세션 인계

> 2026-10-18: 새 다섯 증상과 설정창 동시 표시의 글자 깨짐은 [배터리·창·탭 후속](../../feature/FOOTER_LIFECYCLE_2026-10-18.md)을 먼저 따른다. 중간 Windows/WSL GUI와 일반 101개가 통과했지만 최종 release는 Defender 격리로 실행 검증이 막혀 있다. 보호 설정은 유지하고, 현재 파일의 보안 검토 결과 없이 다른 이름/빌드로 우회하지 않는다. 성능 측정은 별도 세션 범위다.

> 2026-10-16: 공유 알림의 임시 대화 간섭·완료 배지·호출 폴더를 수정하고 일반 88개 및 별도 release의 native Codex 통합 검사를 통과했다. 다른 탭에서 같은 대화/이력 재개·Bash·R 복구·원래 콜백·daemon 유지도 확인했다. [현재 인계](../../feature/CODEX_RECOVERY_HANDOFF.md)가 최신 실행 파일과 남은 실기 확인의 기준이다. 배터리·성능은 계속 별도 세션 범위다.

> 2026-10-15: 기본 공유 Codex의 탭별 알림 중계를 구현했다. [현재 인계](../../feature/CODEX_RECOVERY_HANDOFF.md)와 [공유 알림 구현·검증](../../feature/SHARED_NOTIFICATIONS.md)이 아래 이전 미완료 설명보다 우선한다. 성능 세션과 같은 폴더를 사용하므로 기능 검증은 고정 사본과 별도 실행 파일로 수행한다. 배터리·팬 등 성능 측정과 해결은 사용자 지시에 따라 이 세션에서 진행하지 않는다.

> 2026-10-14 현재: 기본 공유 Codex의 첫 입력·확장/축소 휠·초안·기존 대화 이력·R 복구·창 종료 후 서버 유지의 release GUI 회귀를 통과했다. [현재 인계](../../feature/CODEX_RECOVERY_HANDOFF.md)와 [실행 구조·알림 한계](../../feature/CODEX_STARTUP.md)를 먼저 읽는다. 아래 첫 Enter 미완료 및 중단 설명은 수정 전 기록이다.

> 2026-10-14 재개 당시: 배터리 진단 후 설정창 모서리·한글/영문 글꼴을 수정했다. 당시 남았던 native GUI 첫 Enter 요청 실패는 이후 공유 서버 준비 순서 변경으로 해결했고 맨 위 현재 인계에 최종 근거를 기록했다. [배터리 진단](../../performance/BATTERY_DIAGNOSIS_2026-10-13.md)도 함께 따른다.

> 2026-10-13 17:14 KST 사용자 요청으로 중단. 재개 시 [Codex 입력·재개·Bash 연결 인계](../../feature/CODEX_RECOVERY_HANDOFF.md)를 먼저 읽는다. RGB 입력 오염·공유 대화 충돌 후속과 Bash 실행 환경을 구현 중이며 공유 서버 GUI는 미완료다. 아래 이전 스크롤 통과 결과를 최신 기본 실행 경로의 완료로 해석하지 않는다.

> 2026-10-13 최신 기능 후속: [Codex 스크롤 수정](../../feature/SCROLL.md). 최근 실행 파일의 스크롤 실패를 재현하고, 출력 기록 보존과 확장/축소 입력 화면을 수정했다. 최종 로컬 실행 파일·검증·적용 범위는 이 기록을 따른다.

2026-10-12 인계 정리 당시 제품 구현 목표는 **일시중지**였고 문서·소스만 읽었다. 이후 사용자가 `feature/feature.md` 구현을 명시적으로 요청해 기능·디자인 작업을 재개했다. 최신 로컬 변경·검증·남은 확인은 [기능 구현 결과](../../feature/IMPLEMENTATION.md)를 따른다. 아래 공개 제품/MSYS 및 실기 미검증 기록은 당시 범위로 유지한다. 기능과 디자인을 한 세션에서 맡고 성능은 별도 세션으로 둔다.

## 현재 기준과 읽기 순서

- 공통 기준은 [SESSION_HANDOFF.md](../SESSION_HANDOFF.md)의 2026-10-12 20:30 KST 관찰이다. `main` HEAD `17cbfc701f5cdd533857e79a54de541a7dc4d4eb`; 공개 alpha.6 소스는 `09d7c01780be6b091597030e07f0474f3b5b144e`다. 재개 시 실제 작업 트리와 실행 상태를 새로 확인한다.
- 공개 alpha.6은 Windows 11 x64 제한 알파다. Rust·egui/eframe·wgpu/DX12·Alacritty VT·ConPTY를 사용한다. 기본은 설치 Git Bash, WSL2 Bash는 선택형이다. 수정 MSYS DLL·PortableGit을 공개 제품에 포함하지 않았다.
- 공개 실행 파일 SHA256은 `f94f402b89177766381e991fa288676c9a1fb6f0eb4e89cb2f59d7ccab765cb4`다. 개발 후보 GUI의 `fa322901…` 통과를 공개 제품 검증으로 대입하지 않는다.
- 디자인 자료: [MINI_SUMMARY.md](../MINI_SUMMARY.md), [TESTER_GUIDE.md](../TESTER_GUIDE.md), [IMPLEMENTATION.md](../IMPLEMENTATION.md)의 후반 UI 기록.
- 기능 자료: [BASH_RUNTIME.md](../BASH_RUNTIME.md), [RESIZE_INPUT_INVESTIGATION.md](../RESIZE_INPUT_INVESTIGATION.md), [MSYS_RUNTIME_VALIDATION.md](../patches/MSYS_RUNTIME_VALIDATION.md), [NOTIFICATIONS.md](../NOTIFICATIONS.md), [SCROLL_BEHAVIOR.md](../SCROLL_BEHAVIOR.md).

## 확정 요구

- 확장·축소·숨김·세션 전환에서도 같은 셸과 실행 중 에이전트를 유지한다. 입력 누락을 지연·보충 문자·명령 재전송으로 숨기지 않는다.
- 상단은 새 탭·입력/요약 전환·숨기기 세 아이콘과 세션 제목이다. 설명은 호버, 설정·알림은 트레이 경로를 사용한다.
- 본문 어느 위치든 더블 클릭으로 확장/축소한다. 요약 단일 클릭은 포커스만 이동하며 Shift+더블 클릭은 단어 선택이다. 같은 폭·하단 기준을 유지하고 위로 부드럽게 확장한다.
- 마우스 이동·가장자리 크기 조절, 고정된 작은 DWM 모서리, 투명도, 설정을 닫기 전 다크/라이트 반영을 유지한다. 사용자 곡률 수치 설정을 되살리지 않는다.
- 한글·영문 기준선과 선택 배경, ANSI 색·볼드·이탤릭·기호 폴백, GUI 글꼴 선택을 제공한다. 프롬프트는 한 줄 구성이고 사용자·호스트·경로·별칭을 GUI로 조절한다.
- 요약은 현재 태스크·명시된 계획의 완료 항목 비율·실행 상태를 우선하고 실제 AI Battery를 하단에 둔다. Bash/내부 명령·원시 인수·Codex `/status` 안내를 태스크로 표시하지 않는다.
- 응답 완료와 계획 완료·작업 성공을 구분한다. 기존 에이전트 콜백·별칭·함수·인증·초기화 파일을 보존하며 실제 입력·재개 메뉴·알림으로 확인한다.

## 디자인: 구현 및 검증된 범위

- `app.rs`·`icons.rs`에 세 아이콘·세션 제목·명시적 툴팁·본문 더블 클릭·요약 단일 클릭을 구현했다. `native.rs`·`geometry.rs`에 이동/크기·하단 기준·180ms 확장과 동작 줄이기·DWM 작은 모서리·투명도 경로가 있다.
- 글꼴 기준선·한글 선택 배경·기호 폴백·ANSI 강조·라이트 중립색 대비와 테마 변경 시 글꼴/행 캐시 무효화가 구현돼 있다. 계산·글리프 존재·VT 스타일 검사는 실제 IME나 모든 GPU 픽셀의 보장이 아니다.
- 고정 GUI 회귀는 다크/라이트·작은 창·한글 요약/직접 보고·하단 배터리·같은 폭/하단·위/가운데/아래 더블 클릭을 검사했다. `artifacts/ux-f98545e3-e514-405b-bd1c-e851d591c153` 및 `ux-a61782d0-00ac-4620-9721-263707eb5745`가 근거다.
- 최신 개발 후보의 4세션 기본 Bash GUI는 `artifacts/ux-78519338-e367-4fa5-9222-8da7053b5e48`에서 통과했다. 자동화가 실제 Windows 포인터를 놓아 툴팁 하나를 관찰한 기록은 사람의 호버/스크롤 검사와 구분한다. 첫 툴팁 실패 원인은 미확정이다.
- 실제 Codex 0.162.1·설치 AI Battery의 로컬 제공자/고정 배터리로 목표 재개창 하단 위치·방향키/Enter 닫기·VT 문자/ANSI/커서를 확인했다. `artifacts/goal-hud-f896c2c6-9a6d-470f-9154-b852b34e5eca`가 근거이며 실제 계정/모델·도구 작업 중 잔상 전체 검증은 아니다.

## 기능: 구현 및 검증된 범위

- PTY 입력과 resize는 `terminal.rs`의 동일 FIFO에서 순서대로 처리하고 셸을 Job에 묶는다. 프롬프트는 같은 Bash 안에서 일반 로그인 파일을 읽고 별칭·함수를 유지하며 세션별 상태 파일로 갱신한다.
- 기본 Bash 선택은 명시적 사용자 경로 → 앱 옆 검증된 `runtime/git` → 설치 Git 순이다. 전용 폴더가 손상되면 오류로 표시한다. `runtime.rs`의 CNG 해시는 파일 일치 검사이며 출처 인증을 대신하지 않는다.
- **공개 alpha.6의 기본 Git `2.55.0.windows.5`: 4개 통과·4개 실패**, `allPassed=false`, `acceptedForAlpha=true`. 실패는 raw/managed prompt typeahead, Session 가로+세로/세로 전용 resize다. 공개 `COMPATIBILITY.json` 사본은 `artifacts/context-handoff-dd208229d75b4984977dedb55825c565/COMPATIBILITY.json`에 있다.
- **개발 MSYS v3 후보:** Git `2.50.1.windows.1`, 소스 `7674c51e18b9323c0072c8299c9356a7d9511a4e`, DLL SHA256 `bd6674212e79904dc20827603f25253b46bd7321a5ce4126889ee3030d27281b`. 해시·패치는 [BASH_RUNTIME.json](../../runtime/BASH_RUNTIME.json)과 MSYS 검증 문서에 고정돼 있다.
- 후보 raw/canonical 즉시 resize→입력 각각 1,000회와 SIGWINCH/siglongjmp 1,000회, 전용 기본 경로 선택 포함 연동 11개·4세션 GUI 통과 기록이 있다. 설치 Git DLL은 덮어쓰지 않았다. 전체 잡 제어·최신 Git·다른 환경·부하·제품 배포는 미완료다.
- 최신 Unicode/공백 경로 연동 11개는 `artifacts/runtime integration 한글-f4cccf1f2a274a308257e62ccbd35283/verification-25dbf7f4633940e1bee0d31c60414b79`에 있다. 이전 Codex SSE 실패는 fixture idle 연결 차단 회귀를 고친 뒤 통과했으며 실제 모델 결과가 아니다.
- 직접 진행 보고는 `progress-v1`의 작업 ID/다음 revision, bounded JSON, 잘못된 완료/오래된 보고 거부를 구현했다. 완료 항목 수/전체 항목 수이며 Enter·Ctrl+C 전송 시 기존 보고가 만료된다. 프로젝트 skill의 고정 어댑터·GUI 갱신은 검증했고 실제 모델의 선택·지속 갱신은 남는다.
- Windows Codex/Claude 연동은 기존 notify/훅을 보존하고 공식 이벤트를 helper→사용자 Named Pipe→broker로 전달한다. 세션별 상태·중복 억제·숨긴 창 포커스 유지와 COM 직접 호출 복귀는 fixture에서 통과했다. 실제 toast 클릭 통과를 뜻하지 않는다.
- WSL2 Ubuntu-24.04 fixture는 Bash 환경/TTY/resize/PID·interop, Linux Bash의 Windows helper 호출·동일 UUID 복귀·소유 Windows/Linux 자식 종료를 확인했다. Linux 에이전트 설정의 설치·해제/기존 훅 보존과 실제 에이전트 연결은 남는다.

## 미해결과 문서 해석의 한계

| 영역 | 남은 문제와 근거 |
| --- | --- |
| 기본 입력 | MSYS의 resize 이벤트 peek/discard 사이 큐 경쟁을 두 프로세스 API 추적으로 관찰했다. 후보는 이벤트 소비 후 SIGWINCH·별도 마지막 보고 크기로 통과했으나 모든 간헐 ANSI/prompt 실패가 같은 원인이었다고 단정하지 않는다. |
| 런타임 배포 | 후보는 `publicPackagingApproved=false`. 최신 지원 Git 재빌드, 전체 잡 제어/부하, 대응 소스·빌드 절차·고지, NSIS/ZIP 설치·업데이트·제거·복구·깨끗한 환경을 완료하지 않았다. |
| 스크롤 | `app.rs:1375`의 Shift는 마우스 전달을 피하고 `:1419`의 방향키 3회 대신 `scroll_display`를 호출한다. 대체 화면의 기록이 복원된다는 실기 근거는 없다. 코드가 `ALT_SCREEN`만 확인하므로 DEC mode 1007 호환을 구현 보장으로 읽지 않는다. |
| Codex 실행 조건 | Windows 함수는 기본 `--no-alt-screen`을 추가하지만 기존 별칭/함수·직접 실행 경로·WSL·`MINITERM_CODEX_INLINE=0`에서는 적용되지 않을 수 있다. agy/Codex/TUI 전체를 같은 화면 모드로 가정하지 않는다. |
| 실제 UI | 사람의 호버/툴팁 안 스크롤·물리 키보드 단축키·IME 조합/취소/후보창·혼합 DPI/실제 모니터 이동·선택/붙여넣기·실제 TUI 잔상·체감 애니메이션은 미완료다. Unicode 합성 입력과 고정 캡처로 대체하지 않는다. |
| 실제 에이전트/알림 | 계정/모델/실제 도구 출력·skill 선택/갱신/승인 Enter 후 재보고·실제 사용량·toast 클릭·앱 종료 후 COM 재기동·DND/차단은 미완료다. 응답 이벤트 도착과 팝업 UI를 구분한다. |
| 오래된 문서 | IMPLEMENTATION의 5통과/3실패·이전 클릭/곡률 설정·PID는 당시 기록이다. RESIZE/MSYS 검증의 “기본 프로필 미적용”도 10/11 격리 시험의 범위다. 현재는 선택 코드가 있고 수정 DLL은 공개 배포하지 않는 상태다. |
| 스크롤 안내 | 이번 인계에서 SCROLL_BEHAVIOR와 IMPLEMENTATION 10/12의 Shift 설명을 호출 경로와 실기 미검증 범위로 제한했다. 조사 문서와 TESTER_GUIDE의 우회를 수정 완료·모든 대체 화면 기록 복원으로 표시하지 않는다. |
| 근거 전달 | `artifacts/`·DLL·SDK·캡처는 Git에서 제외돼 새 clone에 없다. 필요한 증거를 별도 인계한다. 같은 바이너리/조건의 통과를 이유 없이 반복하지 않으며 실패 기록도 유지한다. |

## 재개 후 다음 작업과 완료 조건

1. **P0 — 기본 Git 입력 누락의 제품 해결 경로 확정.** 최신 지원 Git에 후보를 재빌드하고 격리된 기본 프로필·즉시 입력·신호/잡 제어/부하로 비교한다. 완료: 실행 파일/셸/DLL SHA·조건을 고정한 8개 기본 검사와 raw/canonical/신호 회귀 통과, 실패 보존, 대응 소스/고지 및 설치·업데이트·제거·복구 근거가 갖춰진 배포 후보. 공개 승인·릴리스는 통합 담당자와 별도 조정한다.
2. **P1 — 실제 TUI 스크롤 재현과 정책 결정.** 일반 화면·ALT_SCREEN·마우스 보고·Shift·Windows 래퍼 우회/비우회·WSL에서 휠이 보내는 입력과 보이는 기록을 기록한다. 완료: 화면 모드/CLI 버전/실행 경로별 기대 동작, 재현 기록, 필요한 최소 수정과 검사, Shift 우회의 적용 범위를 반영한 문서가 일치한다.
3. **P1 — 디자인·입력 실기 인수.** 한 앱에서 세 아이콘·툴팁 하나/안내 스크롤·본문 위치별 더블 클릭·폭/하단·테마/선택/글꼴·한글 IME·혼합 DPI·애니메이션을 확인한다. 완료: 실제 장치/배율/글꼴과 재현 영상/캡처, 동일 셸 PID 유지 및 입력 중복/누락 없음, 사용자 요구별 통과/남은 결함이 기록된다. 성능 세션의 입력→pixel 측정과 조건을 맞춘다.
4. **P1 — 실제 에이전트·요약·알림 끝까지 검증.** 실제 작업의 skill begin/update, 중단·승인 Enter 후 재보고, AI Battery·재개 메뉴·공식 이벤트·toast 클릭/DND/재기동을 확인한다. 완료: 직접 보고/화면 계획/응답 이벤트를 구분한 증거, 콜백/설정 보존, 정확한 세션 선택·확장·포커스 복귀와 명령 재실행 없음이 확인된다.
5. **P2 — WSL 에이전트 연동 완성.** 격리된 Linux Codex notify/Claude 훅에 설치·해제/기존 설정 보존 경로를 구현·검증한다. 완료: 실제 Linux 에이전트 이벤트가 동일 세션에 전달되고 원래 훅·사용자 설정·자식 종료 동작을 보존한다. 미지원 구성은 UI/문서에 정확히 표시한다.

## 담당 파일과 공유 경계

- 통합 기능·디자인: `src/app.rs`의 표시·입력·스크롤·설정 UI를 주로 맡는다. `icons.rs`, `palette.rs`, `geometry.rs`, `shell.rs`, `runtime.rs`, `prompt.rs`/`prompt.bash`, `agent.rs`, `mini.rs`, `progress.rs`, `integration.rs`, `ipc.rs`, `notification.rs`, 관련 기능/GUI 검증 도구와 문서가 담당 범위다.
- 공유: `app.rs`의 글리프/행 캐시·그리기·repaint, `terminal.rs`의 parser/큐/요약 캐시, `pty.rs`의 전송 비용은 성능 세션과 겹친다. `native.rs`·`settings.rs`는 창/프로세스/계측·설정에 따라 공유한다. 공유 파일은 한 세션씩 수정하고 통합 담당자가 diff·동작 영향을 확인한다.
- 성능 세션이 working set·CPU/GPU·셸 자원·출력 부하·입력→pixel·기준 터미널 비교를 맡는다. 개발 GUI의 185MB 수치를 공개 alpha.6의 200MB 달성으로 사용하지 않는다.
- 기존 작업 트리·사용자 앱/셸·Codex 세션·전역 Git/WSL·인증·초기화 파일을 유지한다. GUI는 데스크톱을 공유하므로 동시 실행하지 않는다. 배포·WinGet·릴리스·PR을 영역별로 독립 수행하지 않는다.

## 새 세션 시작 프롬프트

```text
MiniTerm 기능·디자인 통합 세션이다. C:\Users\ghwls\00_Projects\miniterm에서
docs/SESSION_HANDOFF.md와 docs/areas/DESIGN_FUNCTIONALITY.md부터 읽어라.
구현 목표는 paused이므로 이번에는 현재 상태·근거·다음 한 작업만 확인한다.
공개 alpha.6 기본 Git 4통과/4실패와 개발 MSYS 후보 통과를 구분하고
Shift+휠을 검증된 스크롤 해결로 단정하지 마라. 기존 변경·사용자 설정/앱을
유지하며 app.rs/terminal.rs/pty.rs 등 공유 수정은 성능 세션과 조정한다.
명시적인 구현 재개 후 우선순위와 완료 조건에 따라 작업하고 실패도 기록하라.
```
