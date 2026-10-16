# Codex 입력·재개·Bash 연결 작업 인계

## 2026-10-16 현재 — 공유 알림·재개·Bash·R 복구 통과

사용자가 성능 진단·해결은 성능 세션에서 진행하고 이 세션은 기능 구현에 집중하라고 지시했다. 성능 세션도 같은 작업 폴더를 수정 중임을 확인했다. 이후 성능 조치나 측정은 하지 않았으며, 그 세션의 진행 중인 소스와 빌드 결과를 덮어쓰지 않도록 기능 검증 사본과 실행 파일을 분리했다.

공유 Codex의 정확한 탭별 알림을 추가했다. 기존 서버의 공식 proxy에 탭별 인증 연결을 만들고 성공한 대화 시작/재개/분기의 응답만 연결한다. 제목 생성 등의 임시 대화가 이 연결을 덮어쓰던 문제와 완료 뒤 thread 활동이 배지를 지우는 문제를 수정했다. 원격 TUI에는 현재 Bash 폴더를 전달하고 명시적인 `-C`/`--cd`를 보존한다. 응답 중·완료·승인/입력 요청·오류·중단을 공식 이벤트로 구분하며 원래 콜백·옵션·공유 daemon을 유지한다. [구현·검증과 제한](SHARED_NOTIFICATIONS.md)을 따른다.

- 고정 사본과 실행 파일: `artifacts/feature-notify-final-20261015/miniterm.exe`.
- SHA256: `8ec850cf1efeb2478cc8d174c4f1e3b183ec648d5bc206212a4967a86b5d8b54`.
- 사본 소스 지문: `183fa6e7aa4573f8bad066369169add8dec56b9e0247ab17d7c1c9ef4130c338`. UTC 빌드 `2026-10-16T03:44:24.8701681Z`. 같은 폴더에 `src/`, `BINARY_BUILD_INFO.json`, `VERIFICATION.json`을 보존한다.
- 일반 검사 **88개(단위 87개·SSE 1개)** 통과. 무시한 환경 의존 검사는 합계에 포함하지 않는다. 이전 사본의 별도 notify 회귀 1개 통과는 당시 결과이며, 최종 release는 아래 실제 콜백 보존 검사로 확인했다. 10월 15일 캡처 당시의 성능 변경은 포함하지만 이후 성능 변경을 통합한 파일은 아니다.
- 중간 전체 검사는 공유 파일이 추가/수정되는 도중 컴파일 오류로 중단됐다. 기능 검사 84개를 통과한 이전 결과와, 이후 고정 사본의 전체 통과를 구분한다. 성능 모듈의 누락된 trait import 한 줄만 통합 빌드 차원에서 보완했고 성능 동작은 수정하지 않았다. 사본은 성능 세션이 보완한 테스트 초기화/타입 지정까지 포함한다.

최종 release `artifacts/shared-bash-a0eec413-5e77-448a-a652-7eca0ec7d9ea/verification.json`은 **통과**했다. 숨긴 실제 GUI의 IPC와 설치된 native Codex 0.162.1을 사용했다. 첫 탭만 완료·다른 탭에서 같은 대화/이력 재개·두 탭의 배지 유지·호출 폴더·Bash의 `printf`/`ls`·실제 writer 충돌 중 R·시험 서버의 소유권 해제 후 R/새 입력·최종 공유 재개·원래 대화 콜백 4회·GUI 종료 후 daemon 유지/중계 종료를 확인했다. 자체 daemon/updater 정리도 통과했다. 데스크톱 키보드·마우스를 합성하지 않았고 사용자 설정·계정·서버에는 연결하지 않았다.

이전 debug GUI의 첫 입력·휠·초안·탭별 완료 통과와 최종 release의 백그라운드 native 통과를 구분한다. release GUI 시도는 다른 앱으로 포커스가 이동해 첫 입력 전에 취소했으므로 최종 파일의 GUI 휠·설정창 재검증으로 세지 않는다. 사용자 실제 대화/IME/붙여넣기·팝업 클릭, 4MiB를 넘는 재개 응답의 알림 연결과 물리 Ctrl 단축키 확인은 남는다. 일반 Git Bash 입력 누락의 이전 4건도 남는다. `feature.md`의 2·4·7·13·14번을 완료로 옮기지 않는다. 공개 배포·설치본 교체·사용자 CODEX_HOME 변경은 하지 않았다.

## 2026-10-14 재개 갱신 — 아래 중단 당시 기록보다 우선

사용자가 미완료 작업 재개를 요청했다. 배터리 과다 사용을 먼저 진단한 뒤 기능 검사를 재개했다. **공유 Codex GUI의 첫 입력·대화 재개·R 복구는 release 회귀를 통과했다.** 기본 공유 경로의 정확한 탭별 알림과 사용자 실제 입력은 미완료다. 원래 중단 요청은 재개 요청으로 해제됐고 아래 17:14 기록은 당시 이력이다.

### 최종 실행 파일과 같은 파일의 GUI 회귀

- 실행 파일: `artifacts/miniterm.exe`, SHA256 `9b868ff20c42f816e45a6aca83fd1bb83c491115eee508bbdeca8b1205ad26ad`.
- 소스 지문: `40e27f1f95e7eabd225aca1e6706dfbdf881c2d9bb405f58cd65570c645ec305`, 현재 소스와 일치. 빌드 UTC `2026-10-14T02:28:12.1281510Z`.

| 검사 | 최종 근거 |
| --- | --- |
| 첫 입력·Bash·휠·재개·R·서버 유지 | `artifacts/scroll-9e3adaa1-8c4a-4381-a483-cb52b6140d09/verification.json`, `passed=true`. GUI 시작 뒤 셸에서만 지정한 `MINITERM_SCROLL_CALLER`가 실제 Bash의 `printf`·`ls` 실행에 전달됐다. 첫 정확한 문장·확장/축소 위아래 기록·초안·같은 대화 이력·충돌 중 R·소유권 해제 후 R/새 입력·최종 공유 재개·GUI 종료 뒤 서버 연결을 확인했다. 외부 선행 daemon start는 하지 않았다. |
| 설정창 | `artifacts/settings-65d01f2d-40ef-43db-9a80-f8b08eab7f68/verification.json`, `passed=true`. 560×720 DIP·즉시 저장/X/재시작·숨긴 HUD·원래 셸 UUID·Codex/Claude 등록 보존·Bash 적용/해제를 확인했다. 최종 모서리·혼합 글꼴·Codex 알림 범위의 캡처도 확인했다. |
| 실제 목표 재개 메뉴 | `artifacts/goal-hud-f26ea436-de8d-4ef5-bfc7-bc4e4641bfb7/verification.json`. 설치된 Codex/AI Battery inline overlay에서 메뉴 하단 위치·다크/라이트·선택 후 제거·시험 Bash 종료 확인. 계정/모델 요청은 없다. |
| 단축키 GUI | `artifacts/shortcuts-ae147064-b0c3-448f-88d2-91ff087f6aef/verification.json`, `passed=true`, `inputMode=egui_fixture`. 탭 생성/선택/닫기/취소·글꼴·설정·확장/축소·요약·출력 기록·EXE 아이콘·UUID 보존. 사람의 실제 Ctrl 키 조합 확인과 구분한다. |

모든 GUI 행은 위 동일한 release 해시다. 실제 Codex는 설치된 0.162.1이며 고정 localhost 제공자와 자체 CODEX_HOME만 사용했다. Bash 명령을 실행한 최종 fixture는 `danger-full-access`를 자체 설정에만 사용한다. 앞선 `scroll-1382bbb0…`의 `read-only` 정책이 명령을 거부한 결과를 권한 우회로 바꾸지 않았다. 마지막 서버/updater 정리도 통과했고 조회한 사용자 전역 서버·updater·CLI는 유지된다.

일반 Git Bash 호환성은 별도 미완료다. 같은 소스 지문에서 `scripts/test.ps1 -Pty`의 **5개 통과·4개 실패**를 확인했다. managed/raw resize, Session 가로/세로 resize 뒤 `printf`의 첫 `p`가 빠져 `rintf`가 실행됐다. `artifacts/git-compatibility-20261014-final.{json,out.txt,err.txt}`에 실제 Rust 검사 결과와 cargo 101을 기록했다. 첫 로그 수집은 PowerShell 5.1의 stderr 리다이렉트 때문에 컴파일 안내에서 중단됐으며 실제 검사로 세지 않는다. 이후 숨긴 자식 PowerShell의 stdout/stderr를 OS 파일로 각각 받아 검사했다. launcher의 비어 있는 종료 코드 대신 Rust 실패 결과를 기준으로 기록했다. GUI/Codex 회귀 통과나 ConPTY DLL 검색 경로 보완이 이 4건을 해결했다는 주장은 하지 않는다.

재검증 명령은 `$env:MINITERM_SCROLL_RETRY='1'; node scripts/verify-scroll.cjs default artifacts/miniterm.exe shared full`, `./scripts/verify-settings.ps1`, `node scripts/verify-goal-hud.cjs artifacts/codex-ui-f12d6288-4698-46ef-a76a-725a78216224`, `./scripts/verify-shortcuts.ps1 -FixtureKeys`다. GUI는 순차 실행한다. 자체 seed와 기록은 Git 제외 경로이므로 새 clone에서는 다시 준비해야 한다.

### 구현 및 이전 검사

- 실행 순서: `codex-prepare`가 현재 셸 환경·폴더·CODEX_HOME을 GUI IPC로 전달하고 공유 서버를 프런트엔드 전에 준비한다. 서버는 터미널 Job Object 밖에서 시작한다. 기존 서버를 자동 종료/재시작하거나 명시적 private/config/remote 실행을 덮어쓰지 않는다. [구조·Bash 등록·알림 범위](CODEX_STARTUP.md)를 따른다.
- 공유 GUI/R: `artifacts/scroll-18018204-5709-41e8-9ac3-a14c28475a80/verification.json`, release `6498a809…`. 외부 선행 서버 시작 없이 첫 입력·확장/축소 위아래 휠·초안·같은 대화 이력·기본 재개·실제 writer 충돌과 소유권 해제 후 R/새 입력·창 종료 후 공유 서버 유지를 통과했다. 서버를 종료한 것은 해당 시험의 명시적 R 복구 단계뿐이며 사용자 서버는 유지한다. 최신 환경 전달 변경과 UI 회귀 근거는 위 최종 갱신을 따른다.

- 배터리: 사용자가 수집한 관리자 SRUM·WER 메타데이터로 확인했다. 18:06의 28개 GPU 보고는 2025-12~2026-09 발생 기록의 재보고다. 실제 새 `193` 보고는 21:57 1개다. 3시간 방전 약 51.4Wh, 앱 추정은 WSL 8.568Wh·Brave 5.214Wh·Explorer 4.391Wh·MiniTerm GUI 약 0.100Wh이며 Unknown 22.601Wh는 개별 앱에 배분하지 않는다. Explorer가 초반, WSL이 후반에 높다. [진단 원문과 한계](../performance/BATTERY_DIAGNOSIS_2026-10-13.md)를 따른다. 드라이버/WSL/사용자 서버 설정을 변경하지 않았다.
- 설정창 후속 15번: 프레임 내부의 별도 둥근 배경을 제거하고 native DWM 작은 모서리를 적용했다. 맑은 고딕을 UI의 비고정폭 글꼴 맨 앞에 두어 영문·한글을 같은 face로 표시한다. 터미널/명령 고정폭 글꼴은 유지한다. `artifacts/settings-028165cf-caff-4c3a-b024-996864e2bf2e/verification.json`에서 release `8c3a6e08…`의 고정 크기·즉시 저장/X·재시작·사용자 세션 보존·Bash 연결 적용/해제를 통과했다. 캡처를 확인했고 모서리 픽셀에서 기존 내부 검은 배경이 사라진 것도 확인했다.
- 공유 서버 유지와 기존 PowerShell 대화의 Bash 전환: release helper `9e326348…`, `artifacts/shared-bash-6d58a7a1-ef14-40ab-99df-fa8ab138896b/verification.json` 통과. 공식 WebSocket RPC `thread/loaded/list`로 첫 CLI 종료 후 서버가 대화를 유지한 것을 확인한다. 기존 UUID·실제 이전 요청 내용·Bash 기본 도구·지침·핵심 config 보존을 확인했다.
- Win32 키 프로토콜: `artifacts/shared-bash-aba44f95-9daa-4207-b1fc-676f52043180/verification.json`에서 실제 Codex 콘솔의 `9001` 요청을 확인하고 고정 문자와 Enter를 Win32 VK/scan/Unicode/down/up 패킷으로 전송했다. Bash 도구 실행·같은 대화 재개·지침/이력/설정 보존을 통과했다. 네이티브 GUI 이벤트 전달과 사람의 입력 검증을 대신하지 않는다.
- 알림: `./scripts/test.ps1 -CodexNotify`의 명시적 MiniTerm 콜백 재바인딩·기존 콜백/프로필/해제 보존을 통과했다. fixture `artifacts/codex-notify-a641bd40-90f2-41a6-8f50-d62304bfd5ea/`. 긴 응답 마커가 ConPTY 줄바꿈/커서 이동으로 나뉘어 검사가 거짓 실패하던 것을 완료 turn 이벤트로 고쳤다. 실제 기본 공유 경로의 알림 또는 사용자 계정 시험으로 확대하지 않는다.
- Bash 연결 트랜잭션: 고유 임시 파일·동시 MiniTerm 변경 잠금·config 실패 시 기존 등록 기록 복구·해제 실패 시 소유 config 복구를 추가했다. 읽기 전용 config의 실패/재시도도 검사했다. 일반 검사 **78개 통과**(단위 77개·SSE fixture 1개), 환경 의존 AgentBash 2개·WindowsBridge 1개도 통과했다. ignored 검사는 합계에 넣지 않는다.
- 시험 프로세스 누수: native `daemon stop`만으로 updater가 종료되지 않음을 확인했다. `scripts/stop-fixture-updater.ps1`은 시험 marker·home·프로세스 시작 FILETIME·명령줄·이미지 경로를 검증해 해당 updater만 종료한다. 이전 시험 소유 updater 19개를 정리하고 공유 Bash/scroll의 finally에 연결했다. 원래 정리에서 실패했던 오래된 PID는 00:52 재감사에서 실행 중이지 않았고 안전하게 통과했다. 사용자 전역 서버는 유지한다.

### 수정 전 GUI 입력 실패와 시험 도구 수정 이력

- `artifacts/scroll-7d3a57cb-b1b5-4b1e-9a0c-1636fbea5184/verification.json`: debug GUI + release executor helper의 **1회 통과**. 같은 결과가 최신 release에서도 안정적이라고 표시하지 않는다.
- `artifacts/scroll-6d03cf05-ef2e-447b-90db-7b4176b06c53/`: release `8c3a6e08…`의 첫 요청 전송 실패.
- `artifacts/scroll-dd818568-6872-445f-8985-a0e1a3cee60a/first-enter-probe.json`: 진단 포함 release `a7c88557…`. native Enter 후 요청이 없었고 후속 egui Enter는 요청을 전송했다. 보조 Enter로 검사를 통과시키지 않았으며 전체 실패다. 보조 입력 전 `focused=true`, `composing=false`, `enterEvents=2`, `enterEncoded=2`, `lastEnterWin32=true`; 이후 `enterEvents=3`, `enterEncoded=3`, 출력 기록 156행이다. 마지막 Enter의 수정키 상태 추가 관측을 준비했다.
- 다른 진단 `scroll-f5ab3d24-65d0-4798-9cc3-e3d7c0901f10`에서는 보조 Enter도 실패했다. 포커스 취소 `scroll-e2c05338…`, `scroll-867e7cdd…`, `scroll-09c391c9…`는 첫 입력 전에 중단됐으므로 동일 요청 실패의 증거로 세지 않는다.
- GUI와 동시에 새 PowerShell 도구 프로세스를 실행하면 포커스에 영향을 줄 수 있다. GUI 중에는 별도 `exec_command`를 시작하지 않고 실행 세션 출력만 poll한다. 포커스를 잃으면 입력을 취소하고 원인을 기록한다. `verify-scroll.ps1`에 foreground PID/class 진단도 추가했다.
- 진단 인스턴스의 IPC에는 포커스·IME 상태·Enter 관측/변환 횟수와 프로토콜 모드를 추가했다. 사용자 입력 내용은 기록하지 않는다. `first-enter-probe.json`은 native Enter 실패 뒤 고정 fixture에 egui Enter 한 번을 보내 차이를 기록한다. 제품 입력 지연/문자 재전송 우회는 없다.

- 첫 입력 실패는 modifier가 없는 Enter도 관측됐고, 본문이 Enter 이후 한꺼번에 나타났다. 자체 native daemon을 먼저 준비한 비교는 통과했다. 그 순서를 제품의 GUI IPC에 적용한 뒤 외부 선행 실행 없이 release 통합 회귀가 통과했다. 단순한 Enter 재전송이나 고정 제품 지연으로 해결하지 않았다.
- 검증 도구의 `daemon status`는 설치 CLI에 없는 명령이었다. 실행 중인 제어 소켓을 조회하는 `daemon version`으로 고쳤으며 종료한 시험 서버에서는 이 명령이 연결 오류를 내는 것도 확인했다. `/quit` 완료는 ConPTY의 DEC 9001 해제가 아닌 소유 native frontend 종료로 검사한다. 자동 제목 생성 요청을 turn 수로 세지 않고 마지막 실제 사용자 문장으로 재개/복구를 확인한다.
- HTTP fixture의 변수 이름 오류로 중단된 `scroll-e67bb6b2…`의 잔존 GUI는 psutil로 PID·시험 data dir·CODEX_HOME·instance ID를 확인한 후 해당 프로세스만 정리했다. 앞으로 GUI PID/시작 FILETIME/instance를 `owned-gui-records.json`에 기록한다. 이전 도구 실패나 포커스 취소를 제품 통과로 바꾸지 않는다.

**남은 결과:** 기본 공유 Codex의 정확한 탭별 알림 연결, 사용자 계정의 원래 대화·IME/붙여넣기·물리 단축키/휠 확인. 6번은 구현·합성 Win32 GUI 검증 완료로 이동했고 2·4·7·13·14번은 미완료다. 설치·공개 릴리스·커밋은 하지 않는다. 최신 실행 파일 해시는 `artifacts/BINARY_BUILD_INFO.json`과 소스 지문을 함께 확인한다.

---

2026-10-13 17:14 KST 기준. 사용자가 “이후에 다시 작업할 수 있도록 작업을 정리하고 중단해”라고 요청하여 **작업을 중단했다. 전체 수정은 미완료**다. 재개 전 이 문서를 먼저 읽는다. 기존 기능·디자인 기록은 각 작업 당시의 결과로 유지한다.

## 요구와 현재 결론

- Codex 시작 시 `]1;rgb:1/1515/1d1d\`가 입력되는 현상, 다른 앱에서 대화가 열렸다는 경고와 R 재시도 실패/멈춤을 해결해야 한다.
- Bash 연결을 제거해 Codex의 실행 도구가 PowerShell로 돌아가는 임시 처리는 사용자 요구를 충족하지 않는다. Bash 명령 실행과 기존 대화 재개를 함께 유지해야 한다.
- 사용자는 MiniTerm을 종료했다고 확인했다. 과거 중단된 1번 탭을 현재 살아 있는 프로세스처럼 다루지 않는다.
- 기본 Bash 실행 환경을 제공하면서 공유 서버로 대화를 재개하는 **격리된 CLI 검사는 통과**했다. 그러나 같은 경로의 **MiniTerm GUI는 첫 입력이 전송되지 않아 실패**했다. 원인은 아직 확정하지 않았다.

## 확인한 원인과 구현

### RGB 입력 오염과 배터리

이전 스크롤 성공 기록 `artifacts/scroll-e9533cca-f69a-42e2-ad71-eee52400bbc6/requests.json`의 첫 요청은 `]1;rgb:1/1515/1d1d\SCROLL-FIRST-TURN`이었다. 이전 검사가 두 번째 초안만 확인해 첫 입력 오염을 놓쳤다. 설치된 AI Battery의 `tui`/rowpty 입력 변환 경로에서 OSC 응답이 키로 재해석되는 경로를 확인했다.

`src/shell.rs`의 관리 배터리 기본 레이아웃을 `inline`으로 바꿨다. 명시적 사용자 레이아웃과 기존 함수/별칭은 유지한다. 저장소 밖의 설치된 rowpty는 수정하지 않았다. `scripts/verify-scroll.cjs`는 첫 요청의 정확한 문자열도 검사한다. 유효한 OSC 응답을 전역 차단하거나 입력 문자열을 임의로 지우지 않는다.

### 공유 서버와 Bash 실행 환경

Codex 0.162.1에서 `--no-daemon`뿐 아니라 실행별 개발자 지침/기능/알림 설정 재정의도 공유 서버 사용을 막을 수 있다. 공유 서버가 보유한 대화를 별도 내장 서버에서 열면 쓰기 소유권 충돌이 발생했다. 기존 프런트엔드를 닫아도 해당 서버의 대화 소유권이 유지될 수 있으므로 R만 눌러서는 해결되지 않는다. 사용자 서버 종료나 대화 잠금 삭제로 우회하지 않았다.

Windows Codex의 기본 실행 셸은 PowerShell이다. `SHELL`이나 절대 Bash 경로만으로 해결되지 않으며, 선택한 셸 종류를 서버 PATH에서 다시 찾는 경로가 System32의 WSL Bash를 고를 수 있었다.

새 `src/codex_bash.rs`는 공식 `codex exec-server --listen stdio`를 실행하고 해당 자식의 PATH 앞에 선택한 Git Bash 폴더를 둔다. 환경 정보 응답의 기본 셸과 제공자 식별자를 Bash 환경에 맞추고 나머지 JSON 메시지는 전달한다. Codex의 실행/권한 처리 엔진을 사용한다.

등록은 `CODEX_HOME/environments.toml`의 기본 환경 `miniterm-git-bash`와 소유 기록 `miniterm-bash.json`을 사용한다. 기존 사용자 환경·주석을 보존하고, 다른 기본 환경/소유권 불일치에는 덮어쓰기를 거부한다. 해제는 소유한 항목만 제거하고 원래 기본값을 복원한다. 두 파일 갱신 중 장애 복구는 추가 검토가 필요하다.

`src/main.rs`에 `codex-bash-server`와 `integrate codex-bash enable|disable`을 추가했다. `src/app.rs` 설정의 Codex 탭은 명시적으로 적용/해제하며 Windows의 다른 Codex에도 적용되는 범위를 안내한다. 실행 중인 Codex를 닫고 공유 서버를 다시 시작해야 한다는 안내를 제공하며 서버를 자동 종료하지 않는다.

**현재 사용자 CODEX_HOME에는 등록하지 않았다.** 격리 시험의 소유 환경에서만 적용했다. 새 기본 실행 경로에서 기존 실행별 Bash 지침/notify 주입은 꺼져 있다. `MINITERM_AGENT_BASH=1`과 `MINITERM_CODEX_SESSION_NOTIFY=1`의 명시적 선택 경로는 남아 있다. 등록을 적용하지 않으면 기본 Windows Codex 도구는 PowerShell을 사용하므로 실제 사용자 적용까지 완료했다고 말하면 안 된다. 기본 경로의 알림 연동 영향도 확인해야 한다.

### Windows 키 입력: 구현 중

`src/win32_input.rs`에 DEC 9001 모드 추적과 Win32 입력 패킷(`CSI Vk;Sc;Uc;Kd;Cs;Rc_`)을 추가했다. `terminal.rs`가 모드를 추적하고 `app.rs`가 키/문자/IME 입력을 해당 형식으로 보낸다. `ipc.rs` 진단에는 모드 여부만 추가했다. OSC 응답과 붙여넣기는 기존 원시 경로를 유지한다.

단위 검사는 통과했고 마지막 별도 서버 GUI 회귀도 통과했다. 그러나 공유 서버 GUI는 `win32Input=true`여도 첫 요청이 전송되지 않았다. **9001 모드가 원인이라는 가설은 아직 입증하지 못했다.** 키 전달과 서버의 대화 초기화 정체를 구분해야 한다. IME·클립보드·Codex 종료 후 셸 복귀 등 실제 입력 회귀도 남아 있다.

## 검증과 증거

모두 시험 소유 CODEX_HOME/대화/작업 폴더와 localhost 고정 응답을 사용했다. 실제 계정·모델 호출, 사용자 인증·대화 수정은 하지 않았다. Bash 도구 fixture는 시험 환경에서 `danger-full-access`로 실행했으므로 제한 정책에서도 같은 결과라고 주장하지 않는다.

| 검사 | 결과와 근거 |
| --- | --- |
| 최신 일반 검사 | 단위 75개와 SSE 1개, 총 76개 통과. ignored 검사는 제외. 이후 새 제품 수정 없이 마지막 GUI 검사를 실행했다. |
| 기본 Bash + 공유 대화 재개 | `artifacts/shared-bash-d0858744-b146-4454-8203-905ab8d001c8/verification.json`: Rust debug helper 등록, 첫 실행/같은 대화 재개에서 셸 인수 없는 도구의 Bash 명령 실행, 지침·핵심 설정 보존, 쓰기 충돌 없음. 통과. |
| 기존 PowerShell 대화의 Bash 전환 | `artifacts/shared-bash-10bbd93d-296a-4b43-8265-520bd381833a/verification.json`: 기존 대화 UUID 유지, 재개 환경 Bash, 실행 성공, 지침 보존, 쓰기 충돌 없음. 통과. |
| 최신 별도 서버 GUI | `artifacts/scroll-65b1fc97-0129-4b97-80ee-1c4cc2fdffe0/verification.json`: debug 실행 파일, inline 배터리, `--no-daemon --strict-config` 시험 경로. 첫 입력 정확성·확장/축소 위아래 휠·초안·세션 유지 모두 통과. **기본 공유 서버 경로의 성공 증거는 아니다.** |
| 공유 서버 GUI | `artifacts/scroll-f1cd87d4-231f-4e6c-a7ad-267e5126573d/`: 최신 Win32 입력 포함 debug에서 실패. 첫 입력은 화면에 보였지만 로컬 요청 없음. `failure-snapshot.json`의 9001 모드는 켜짐. |
| 배터리 없는 공유 서버 GUI | `artifacts/scroll-93b91230-aff8-4eea-a3a8-f773ff0c1f67/`: 직접 EXE 경로도 같은 첫 요청 실패. 배터리만 원인이라고 단정할 수 없음. |
| 충돌 재현 비교 | `artifacts/reconnect-1a00a697-718b-442c-b29f-c9e7b5e99fa5/`: 별도 서버 차단과 공유 서버 재연결은 확인. 원격 실행의 Bash 지침 전달은 실패하여 전체 판정은 실패. |

공유 GUI 시험 소유 서버의 기록에는 등록된 기본 환경이 있음에도 `thread/start`의 `environment_id=local`과 PowerShell 관련 경고가 나타났다. CLI 성공 경로와 이 차이를 우선 확인한다. CLI 시험은 위치 인수로 첫 요청을 제출하므로 키보드 Enter 검증을 대신하지 않는다.

`scripts/verify-goal-hud.ps1`의 배터리 기대값과 `tests/bash_conpty.rs`의 명시적 옵션을 수정했지만 관련 환경 의존 검사는 아직 다시 실행하지 않았다. 새 Codex 설정 탭의 GUI 적용/해제, 기본 알림 경로, 기존 단축키·IME 검증도 남아 있다.

## 소스·실행 파일·프로세스

- 주요 새 파일: `src/codex_bash.rs`, `src/win32_input.rs`, `scripts/verify-codex-reconnect.cjs`, `scripts/verify-codex-shared-bash.cjs`.
- 주요 수정: `src/{app,terminal,ipc,shell,main,lib}.rs`, `scripts/verify-scroll.{cjs,ps1}`, `scripts/verify-goal-hud.ps1`, `tests/bash_conpty.rs`. 기존 기능 작업과 병행 성능 작업도 작업 트리에 있다. `src/console_host.rs` 및 관련 PTY 변경을 이 세션의 변경으로 간주하거나 되돌리지 않는다.
- 중단 시 Rust 소스 지문: `7d53732e02fda507b0c9b7e8ee9b98fcfc658d05ecc1d6a6576e434231fb4d93`.
- `artifacts/miniterm.exe`는 **Win32 입력 추가 전** release다. SHA256 `12ed114d9c81df6de4a741be3685e0f0118f96ba5459fb6c53402afd73023f0d`, 소스 지문 `1f63d8c8d666ede7e5579651acdba6b3f9658af604c6f5d4cab1b5ca1b335c83`, builtUtc `2026-10-13T07:43:15.6940015Z`. 현재 소스의 최종 검증 실행 파일로 안내하지 않는다.
- 마지막 GUI가 사용한 `.tools/target/debug/miniterm.exe`의 SHA256: `4a6bd94fe3325325dd765a6aa1e65456a2f5822f593db98a087bc74a51155f23`.
- 마지막 시험은 문서 정리 시점에 정상 종료돼 결과를 저장했다. 조회상 실행 중인 MiniTerm과 검증용 Node 프로세스가 없었다. 공유 GUI 시험의 소유 서버 종료 기록은 exit code 0이다. 사용자 Windows Terminal의 Codex/AI Battery 프로세스는 유지했다.
- 설치·공개 릴리스·커밋은 하지 않았다. 변경 파일과 성공/실패 증거 폴더를 보존했다.

## 재개 순서

1. 실제 작업 트리/프로세스/소스 지문을 다시 확인하고 위 증거부터 읽는다. 실패 공유 GUI의 환경 선택 `local`과 정상 CLI의 Bash 환경 선택 차이를 비교한다.
2. 소유 GUI에서 네이티브 콘솔 입력 probe로 문자/Enter의 실제 VK·Unicode 전달을 확인하여 키 문제와 서버 초기화 문제를 분리한다. `artifacts/experiments/console-input-probe.exe`와 대응 probe 자료가 있다. 공유 서버가 MiniTerm의 셸 Job에 포함되어 창 종료 시 함께 죽는지도 확인한다. 광범위한 Job 탈출 허용으로 우회하지 않는다.
3. 같은 소유 CODEX_HOME의 공유 서버로 GUI 첫 요청·Bash 도구·기존 대화 재개·확장/축소 스크롤·초안을 함께 통과시킨다. 사용자 서버 종료나 잠금 삭제, 다른 사용자 대화 사용은 하지 않는다.
4. 실제 기본 경로의 알림·설정 등록/해제와 소유권 보호, IME/붙여넣기/단축키, Codex 종료 후 Bash 복귀, 환경 의존 Bash/notify 및 목표 재개 GUI 회귀를 확인한다.
5. 변경을 확정한 소스로 release를 다시 빌드하고 해시/지문을 기록한 뒤 동일 실행 파일을 검증한다. `feature.md`의 완료 상태와 배터리/Bash 문서를 그 결과에 맞춰 갱신한다. 현재 실패 상태를 완료로 바꾸지 않는다.

## 로컬 빌드 환경

기존 MSVC/linker가 없어 공식 Microsoft 배포 자료로 `.tools/msvc-local`에 로컬 도구만 준비했다. 전역 설치나 사용자 PATH 변경은 하지 않았다. MSVC 14.43.34808, SDK 10.0.22621.0이며 자료는 `artifacts/build-tools-manifest.json`에 남아 있다. 재개 후 필요한 프로세스에만 다음 환경을 설정한다.

```powershell
$taskToolchain = (Resolve-Path '.tools/msvc-local').Path
$env:PATH = "$taskToolchain/VC/Tools/MSVC/14.43.34808/bin/Hostx64/x64;$taskToolchain/c/bin/10.0.22621.0/x64;$env:PATH"
$env:LIB = "$taskToolchain/VC/Tools/MSVC/14.43.34808/lib/x64;$taskToolchain/c/um/x64;$taskToolchain/c/ucrt/x64"
./scripts/test.ps1
./scripts/build.ps1
```

GUI 검사는 같은 데스크톱에서 순차 실행한다. 공유 경로의 실패를 재현하는 명령은 `node scripts/verify-scroll.cjs default .tools/target/debug/miniterm.exe shared`다. `default` 자리는 PowerShell 5의 빈 인수 누락을 피하기 위한 것이다. 기본 Bash CLI 검사는 `node scripts/verify-codex-shared-bash.cjs .tools/target/debug/miniterm.exe`, 기존 대화 전환은 뒤에 `migration`을 붙인다. 이 명령들은 **재개 후** 사용하며 중단 시 새 검사를 시작하지 않았다.
