# CLI 응답 알림과 Windows 팝업

MiniTerm은 터미널 출력을 추정해서 알림 완료를 판단하지 않는다. 앱 세션에 전달된 공식 이벤트만 알림 상태로 표시한다. **응답 완료는 작업 성공·모든 백그라운드 작업 종료를 뜻하지 않는다.** 상단의 알림 아이콘은 제거했고 알림/소리는 트레이 설정에서 변경한다. [작업 요약](MINI_SUMMARY.md)의 skill 계획 보고는 별도 통로이며 알림 상태를 바꾸지 않는다.

## 연결

설치 후 MiniTerm 설정의 **알림 → CLI 응답 알림 등록**에서 Codex 또는 Claude Code의 `알림 등록`을 누른다. 등록 결과와 적용 시점을 화면에 표시한다. 등록 후 새 MiniTerm 탭에서 CLI를 시작하면 해당 세션의 이벤트를 받는다. `등록 해제`는 MiniTerm의 알림 전달 등록만 제거한다. 로그인 정보나 인증 파일은 수정하지 않는다. 현재 Windows 사용자 설정만 지원하며 WSL 안의 CLI 설정 등록은 지원하지 않는다.

`Windows 팝업 표시`와 `팝업과 함께 소리 재생`은 변경 즉시 적용·저장한다. 팝업을 끄면 소리 항목도 비활성화하지만 선택한 소리 값은 유지한다. X로 설정창을 닫아도 적용한 값은 유지된다. CLI의 알림 등록·해제와 Windows 앱 알림 등록은 각각 버튼으로 실행하는 별도 동작이다.

CLI에서도 설치된 실행 파일의 `integrate codex enable`, `integrate claude enable`을 사용할 수 있다. `disable`은 해당 연결만 해제한다. 설정 위치는 Codex의 `CODEX_HOME` 또는 사용자 `.codex/config.toml`, Claude Code의 `CLAUDE_CONFIG_DIR` 또는 사용자 `.claude/settings.json`이다.

- Codex: 기존 `notify` 인자 배열을 보관하고 원래 JSON으로 함께 호출한다. MiniTerm 밖에서 실행한 Codex도 기존 콜백을 유지한다. TOML 주석·다른 설정은 보존한다. 연결 후 사용자가 notify를 바꾸면 해제 시 덮어쓰지 않는다.
- Claude Code: 기존 훅 배열에 실행 파일의 명령을 추가한다. 해제는 정확히 같은 명령만 제거한다. 순서를 보존하기 위해 동기 훅을 사용하며, helper는 결정 JSON을 출력하지 않고 오류 시에도 에이전트 동작을 차단하지 않는다.
- 변경 전 원본 사본을 남긴다. 연결 기록은 사용자별 `MiniTerm/integrations.json`에 저장하여 제거 프로그램이 해당 위치를 찾을 수 있게 한다.

설정의 `Windows 알림 등록`은 개발 실행 파일을 Windows 앱 알림에 등록한다. 설치본은 이를 자동 등록한다. 개발 파일을 이동하면 등록과 에이전트 연결 경로도 다시 설정해야 한다.

Windows Codex의 세션별 notify 재바인딩은 `MINITERM_CODEX_SESSION_NOTIFY=1` 또는 이전 Bash 지침의 `MINITERM_AGENT_BASH=1`을 명시한 실행에서 사용한다. 이미 활성화된 MiniTerm notify 기록이 현재 설정과 정확히 일치할 때 그 실행에만 현재 앱의 도우미를 지정한다. 기존 연결을 켜거나 전역 파일을 다시 쓰지 않는다. 원래 콜백과 chain 파일을 유지하고 현재 앱의 탭인지 확인한다. 사용자 별칭·함수는 그대로 둔다.

2026-10-16 기본 Windows Codex의 공유 대화는 등록된 경우 탭별 중계로 알림을 받는다. 기존 공유 daemon의 공식 proxy를 이용하며 실행별 notify 설정이나 특정 탭의 환경 변수를 서버에 주입하지 않는다. 성공한 시작·재개 응답을 해당 탭에 연결하므로 다른 탭·임시 대화·하위 에이전트의 완료는 제외한다. 닫힌 연결의 늦은 이벤트도 거부한다. [공유 실행 구조](../feature/CODEX_STARTUP.md)와 [구현·검증 범위](../feature/SHARED_NOTIFICATIONS.md)를 따른다.

명시적인 `-c notify=...`는 사용자의 정책을 따르고, 선택한 `<profile>.config.toml`의 notify를 함께 확인한다. 알림을 따로 지정하지 않은 프로필은 기본 연결을 상속한다. 프로필의 다른 콜백이나 `notify=[]`는 변경하지 않는다. 이 경로는 [OpenAI Docs의 실행별 설정과 프로필](https://learn.chatgpt.com/docs/config-file/config-advanced#profiles)을 사용한다. 이미 실행 중인 셸·Codex에는 새 연결 코드가 들어가지 않는다.

현재 Windows COM/앱 알림 등록은 기존 설치 경로에 연결되어 있다. 별도 진단 창의 helper/IPC 전달이 맞더라도 그 창으로 실제 toast 클릭이 돌아온다고 보장하지 않는다. 이 검증은 설치본 갱신 후 진행할 항목이다.

## 이벤트와 상태

| 원본 | HUD 상태 |
| --- | --- |
| Codex 공유 `turn/started` | 응답 중 |
| Codex 공유 `turn/completed`의 `completed` / `failed` / `interrupted` | 응답 완료 / 응답 오류 / 중단됨 |
| Codex 공유 승인·사용자 입력 요청 | 입력 필요 |
| Codex 공유 `thread/status/changed`의 active flags | 진행 중인 응답의 입력 대기 / 응답 중; 완료 뒤 활동은 배지 유지 |
| Codex `agent-turn-complete` | 응답 완료 |
| Claude `SessionStart`, `SessionEnd` | 연결됨·상태 미확인 |
| Claude `UserPromptSubmit`, `PostToolUse` | 응답 중 |
| Claude `PermissionRequest`, 지원하는 `Notification` | 입력 필요 |
| Claude `Stop` | 응답 완료 |
| Claude `StopFailure` | 응답 오류 |
| Claude `Stop`의 `stop_hook_active=true` | 상태 미확인; 완료 팝업 생략 |
| 하위 에이전트 이벤트, 지원하지 않는 이벤트 | 부모 상태를 변경하지 않음 |

단독 실행의 Codex notify는 응답 완료만 지원한다. 공유 중계는 app-server의 공식 수명 주기 이벤트도 사용하며, 화면 출력으로 실행·승인·성공 상태를 만들어 내지 않는다. Claude Notification은 `permission_prompt`, `idle_prompt`, `elicitation_dialog`만 입력 필요로 매핑한다. [Codex 알림 설정](https://learn.chatgpt.com/docs/config-file/config-advanced#notifications), [Claude Code 훅](https://code.claude.com/docs/en/hooks)

turn/event ID가 있으면 최대 512개의 최근 식별자로 중복 제거한다. 식별자가 없으면 같은 세션·에이전트·상태의 2초 이내 반복을 억제하며, 다른 상태를 거친 재개는 수신한다. 최근 알림 목록은 100개로 제한한다.

## 전달과 복귀

셸은 고유 `MINITERM_SESSION_ID`를 상속한다. helper는 프롬프트·응답 본문·transcript 경로를 제거한 메타데이터만 현재 사용자 Named Pipe에 전달한다. 사용자 SID와 SYSTEM만 허용하는 DACL, 원격 접속 거부, 프레임 64KiB 제한, 읽기·쓰기 시간 제한을 사용한다. 출력 본문은 저장하지 않는다.

다른 앱을 사용하거나 다른 세션을 보고 있으면 세션 배지와 Windows toast를 만든다. 같은 세션을 보고 있으면 팝업을 생략한다. 알림 설정을 끄거나 Windows가 팝업을 차단해도 앱 내 목록은 유지된다. 소리는 기본 꺼짐이다.

toast의 활성화 인자는 세션 UUID만 포함한다. WinRT로 알림을 전달하고 현재 사용자 COM 서버와 시작 메뉴 AppUserModelID로 활성화한다. 클릭 복귀는 세션 선택·확장·포커스만 수행하며 명령 또는 Enter를 전달하지 않는다. 종료된 세션 UUID는 새 세션에 재사용하지 않는다. 재시작 후 오래된 알림은 세션 종료 안내로 연결한다. [Microsoft 데스크톱 알림 식별자](https://learn.microsoft.com/en-us/windows/win32/shell/enable-desktop-toast-with-appusermodelid)

## 검증 상태

`scripts/verify-shared-notifications.cjs`는 숨긴 실제 GUI의 IPC와 백그라운드 native ConPTY를 사용한다. 같은 대화를 다른 탭에서 재개한 뒤 두 탭의 완료/읽지 않음 표시, 이력·폴더·Bash 실행·R 복구·기존 콜백·서버 유지·중계 정리를 검사한다. 데스크톱 마우스/키보드나 실제 계정은 사용하지 않는다. 최신 실행 파일과 결과 경로는 [공유 알림 기록](../feature/SHARED_NOTIFICATIONS.md)을 따른다.

`MINITERM_SCROLL_NOTIFY=1`을 설정한 `scripts/verify-scroll.cjs`는 실제 GUI의 두 탭, 설치된 Codex 0.162.1, 자체 CODEX_HOME·localhost 제공자로 공유 응답 완료가 원래의 배경 탭에만 도착하는지 검사한다. 첫 입력·확장/축소 휠·초안·Bash 실행도 함께 확인한다. `MINITERM_SCROLL_RETRY=1`을 함께 지정하면 공유 재개·실제 writer 충돌·소유권 해제 뒤 R 복구도 검사한다. 사용자 계정·물리 입력·실제 Windows 팝업 클릭의 통과와 구분한다. 메시지별 알림 분석의 4MiB 제한과 최신 근거는 [공유 알림 기록](../feature/SHARED_NOTIFICATIONS.md)에 있다.

`scripts/test.ps1 -CodexNotify`는 설치된 실제 Windows Codex·AI Battery bridge, 현재 helper와 별도 Named Pipe/Broker를 사용한다. 새 CODEX_HOME의 로컬 SSE 제공자가 고정 메시지로 완료하므로 외부 모델·사용자 인증·기존 콜백을 호출하지 않는다. 기본 연결과 알림을 상속하는 프로필은 현재 broker에 도착하고, 알림 비활성화·프로필의 별도 콜백·사용자 별칭·함수는 보존되는지 검사한다. 원래 콜백의 한글·공백·셸 특수문자 인자와 원본 JSON도 확인한다. GUI 배지·실제 Windows 팝업·실제 계정 응답 시험과 구분한다.

`scripts/verify-notifications.ps1`은 실제 helper·Named Pipe·WinRT·COM 경로를 사용한다. 공식 형식의 fixture 이벤트, 중복, 숨긴 HUD의 포커스 유지, 하위 에이전트/추가 Stop 제외, 승인 후 재개·오류 구분, 보고 있는 세션의 팝업 억제, Bash PID 유지가 설치본에서 통과했다.

COM 복귀 검증은 foreground 권한을 부여한 뒤 COM 메서드를 직접 호출했다. **실제 Windows 팝업 클릭, 앱 종료 후 COM 재기동, 알림 차단/DND, 실제 에이전트 이벤트, Linux 에이전트 훅 설치는 아직 통과로 표시하지 않는다.** Windows 전달 성공과 알림 기록 저장은 확인했지만 자동화에서 팝업 UI를 찾지 못했다. 결과는 `artifacts/notifications.json`과 `toast-info`로 확인한다.

WSL2 `Ubuntu-24.04`의 추가 fixture에서 `WSLENV`로 전달한 세션 ID와 `/p` 변환된 실행 파일을 사용해 Linux Bash가 Windows helper를 호출했다. 숨긴 HUD에 완료 이벤트가 도착하고, 화면·포커스 유지와 동일 UUID로 복귀, 앱 종료 후 Windows wsl.exe 및 Linux Bash 종료를 확인했다. `scripts/verify-session.ps1 -WslDistribution Ubuntu-24.04`, `artifacts/sessions.json`에 기록한다. **기존 Linux Codex notify/Claude 훅을 보존하는 설치·해제와 실제 Linux 에이전트 연결은 남아 있다.** 시험은 직접 helper를 호출하며 사용자 에이전트 설정을 변경하지 않는다.

세션은 셸을 실행하기 전에 broker에 등록하며 UI가 나중에 등록해도 이미 받은 상태·배지·중복 기록을 보존한다. 시작 중 알림 복귀는 해당 세션이 준비될 때까지 기다린다. 다른 세션에 입력을 보내거나 이전 명령을 재실행하지 않는다.
