# 공유 Codex의 탭별 알림

2026-10-16. 미완료 기능 14번의 공유 알림과 대화 재개 후속이다. 배터리·팬 등 성능 진단은 사용자 지시에 따라 별도 성능 세션이 맡는다.

## 동작

설정의 **알림 → Codex → 알림 등록** 후 새 Codex 실행부터 적용한다. 공유 서버는 유지하고 각 프런트엔드 연결에서 성공한 시작·재개·분기 응답을 MiniTerm 탭에 연결한다. 응답 완료·승인/입력 요청은 원래 탭에 표시하고, 보고 있는 탭에서는 팝업을 생략한다. 알림 등록을 해제하거나 사용자가 notify를 변경하면 다음 실행부터 중계를 사용하지 않는다.

`src/codex_link.rs`는 기존 daemon의 공식 `app-server proxy`에 연결한다. 로컬 listener는 `127.0.0.1`의 임의 포트만 사용하고 실행별 bearer token을 확인한다. TUI의 `--remote`와 `--remote-auth-token-env`를 사용하며 `-c notify`나 `--no-daemon`을 추가하지 않는다. CLI가 경로를 포함한 URL을 거부하므로 root URL과 토큰 환경 변수로 연결한다. 원본 프로토콜은 변경하지 않으며 알림의 상태·대화/turn ID·작업 폴더만 broker로 전달한다. 토큰·입력·응답을 로그에 남기지 않는다.

실패한 재개는 탭을 연결하지 않는다. 제목 생성 등 임시 대화(`ephemeral`)와 하위 에이전트가 같은 연결을 사용해도 사용자 대화의 연결을 덮어쓰지 않는다. 다른 대화와 닫힌 연결의 늦은 이벤트도 제외한다. 응답 완료는 작업 성공이나 계획 완료로 간주하지 않는다. 승인/입력 대기 뒤 active flags가 해제되면 응답 중으로 돌아가지만, 완료·오류·중단 뒤의 thread 활동은 완료 배지를 지우지 않는다. 새 응답은 `turn/started`로 구분한다. 기존 콜백·명시적 실행 옵션·별칭/함수는 유지한다.

원격 TUI가 공유 daemon의 내부 폴더에서 시작하지 않도록 현재 Bash 폴더를 `--cd "$PWD"`로 전달한다. 사용자가 지정한 `-C`/`--cd`는 우선한다. 대화 UUID와 이전 이력을 유지하며 다른 탭에서 재개하면 새 응답의 알림은 그 탭에 표시한다.

프런트엔드가 끝나면 Bash 함수는 원래 종료 코드를 반환하고 인증 환경 변수를 복원한다. 탭/GUI 종료 시 listener와 그 실행이 소유한 proxy의 Job Object를 정리한다. 기존 daemon·대화·사용자 프로세스는 종료하지 않는다. 연결을 열지 않은 listener는 30초 후 종료한다.

## 검증

기능 확인용 release는 `artifacts/feature-notify-final-20261015/miniterm.exe`, SHA256 `8ec850cf1efeb2478cc8d174c4f1e3b183ec648d5bc206212a4967a86b5d8b54`다. 소스 지문 `183fa6e7aa4573f8bad066369169add8dec56b9e0247ab17d7c1c9ef4130c338`, 빌드 UTC `2026-10-16T03:44:24.8701681Z`. 같은 폴더에 소스 사본과 `BINARY_BUILD_INFO.json`을 보존한다. 일반 **88개(단위 87·SSE 1)**를 통과했고 환경 의존 ignored 검사는 합계에서 제외했다.

성능 세션이 같은 작업 폴더를 수정하므로 10월 15일 고정한 소스에 기능 후속만 반영해 빌드했다. 캡처 당시의 성능 변경은 포함하지만 이후 성능 변경을 통합한 파일은 아니다. 공용 `artifacts/miniterm.exe`는 교체하지 않았다.

- 일반 검사: 성공/실패한 재개·임시 대화/하위 에이전트 제외·완료 뒤 늦은 thread 활동·완료/오류/중단 구분·양방향 request ID 충돌·승인 후 응답 재개·분할/마스킹/확장 길이 WebSocket 프레임·잘못된 토큰·닫힌 연결·Bash 폴더/리터럴/종료 코드/부모 환경 보존을 검사한다.
- **최종 release 통합 통과:** `artifacts/shared-bash-a0eec413-5e77-448a-a652-7eca0ec7d9ea/verification.json`, 위 SHA256의 실행 파일로 `passed=true`. 첫 탭만 완료, 두 번째 탭에서 같은 대화 재개와 두 탭의 배지 유지, 이력·원래 폴더·Bash 실행, 실제 writer 충돌 중/해제 후 소문자 R, 최종 공유 재개, 원래 대화의 콜백 4회, GUI 종료 후 daemon 유지와 listener 종료를 모두 확인했다. 별도 임시 대화의 원래 콜백 4회도 보존됐지만 탭 상태에는 반영하지 않았다. fixture의 자체 daemon/updater 정리도 통과했다.
- 10월 16일 debug의 백그라운드 native 검사 `artifacts/shared-bash-61968f9e-c924-45c4-b7b0-b7b033edf10b/verification.json`은 통과했다. 실제 GUI IPC와 설치 Codex 0.162.1을 사용한다. 첫 탭만 완료·두 번째 탭에서 같은 대화/이력 재개·원래 폴더·Bash의 `printf`/`ls`·실제 writer 충돌과 소문자 R·소유권 해제 후 복구·최종 공유 재개·원래 대화의 콜백 4회·GUI 종료 후 daemon 유지/중계 종료를 검사한다. 데스크톱 입력을 합성하지 않는다.
- debug GUI `artifacts/scroll-480eb917-503d-4012-9bbc-2ff06c659b4b/verification.json`: 설치 Codex 0.162.1, 두 MiniTerm 탭에서 첫 완료가 원래 배경 탭에만 도착하고 다른 탭은 unknown을 유지했다. 읽지 않음 ID 하나·알림 목록 하나를 확인했다. 첫 정확한 입력·Bash 환경/명령·확장/축소 스크롤·초안·GUI 종료 후 공유 서버 유지도 통과했다. 최종 release 검사와 구분한다.
- release GUI 시도 `feature-notify-212303cf-be03-412a-ab08-dc03d58cc93c/artifacts/scroll-468bf645-758b-4814-a447-4f3c7f592253`는 다른 앱으로 포커스가 이동해 첫 입력 전에 취소됐다. 최종 release의 GUI 휠·설정창 통과로 표시하지 않는다.
- 수정 전 실패도 보존했다. `scroll-0ef9e45f…`는 경로가 붙은 URL 거부, `shared-bash-109e94d7…`는 caller 폴더 불일치, `shared-bash-11fcdbda…`는 검사기의 Windows `\\?\` 경로 비교 오류, `shared-bash-8989c212…`와 `shared-bash-693d2219…`는 임시 대화에 의한 배지 변경이다. `shared-bash-b5848bb8…`는 private TUI가 업데이트 메뉴에서 멈춘 검사 실패다. [공식 설정](https://developers.openai.com/codex/config-reference)의 `check_for_update_on_startup=false`를 시험 CODEX_HOME에만 적용해 버전을 고정했다. 사용자 업데이트 설정은 바꾸지 않았다.

프로젝트 루트에서 다음 명령으로 백그라운드 native 검사를 재현한다. 라이브러리는 프로젝트의 `.tools`에만 설치하며 AI Battery 설치 여부와 독립적이다. fixture의 서버·업데이터·프런트엔드는 종료 시 정리한다.

```powershell
npm install --prefix .tools/feature-test-node --ignore-scripts --no-audit --no-fund --save-exact node-pty@1.1.0 @xterm/headless@6.0.0
node scripts/verify-shared-notifications.cjs artifacts/feature-notify-final-20261015/miniterm.exe
```

별도 데스크톱을 사용할 수 있을 때만 GUI 검사를 순서대로 실행한다. 아래 명령은 검증 방법이며 최종 release에서 전부 통과했다는 기록이 아니다.

```powershell
$env:MINITERM_SCROLL_NOTIFY='1'
$env:MINITERM_SCROLL_RETRY='1'
node scripts/verify-scroll.cjs default artifacts/feature-notify-final-20261015/miniterm.exe shared full
./scripts/verify-settings.ps1 -Executable artifacts/feature-notify-final-20261015/miniterm.exe
```

## 남은 확인

알림 분석은 WebSocket JSON 한 메시지당 4MiB로 제한한다. 초과 메시지의 원본 전달은 유지하지만 분석은 생략하므로, 큰 재개 응답에서 대화 연결을 놓칠 수 있다. 사용자 실제 긴 대화와 IME/붙여넣기, 실제 승인 UI·Windows 팝업 클릭·DND/재기동은 별도 확인이 필요하다. WSL 안의 CLI 알림 등록은 구현하지 않았다. 일반 Git Bash의 resize 직후 첫 글자 누락 4건과 물리 Ctrl 단축키 인수도 기존 미완료 상태를 유지한다.

사용자 CODEX_HOME·Windows 알림 등록·WSL·Quick Share 설정은 이 검사에서 변경하지 않는다. 시험은 자체 CODEX_HOME과 고정 localhost 응답만 사용한다. 공개 릴리스·설치본 교체는 수행하지 않는다.
