# Codex의 공유 실행과 Bash 환경

2026-10-14. 시작 직후 입력이 늦게 나타나고 첫 Enter가 요청을 보내지 않는 문제를 공유 서버 초기화와 프런트엔드 입력 처리의 순서로 분리했다. 설치된 Codex 0.162.1을 고정한 격리 시험에서 서버를 먼저 준비하면 문제가 사라졌다. Codex 내부의 구체적인 대기 지점까지 추적한 결과는 아니다. 최신 실행 파일·최종 검증 근거는 [현재 인계](CODEX_RECOVERY_HANDOFF.md)를 따른다.

## 실행 순서

MiniTerm이 생성한 Git Bash의 `codex` 함수는 일반 대화 실행 전에 `codex-prepare`를 호출한다. helper는 현재 탭 UUID·CODEX_HOME·작업 폴더·호출한 셸의 환경을 사용자 Named Pipe로 전달한다. GUI의 IPC 작업자가 `codex app-server daemon start`를 실행하고 완료 상태를 반환한 뒤 원래 프런트엔드를 실행한다.

서버 시작 프로세스는 터미널의 Job Object 밖에서 생성한다. 창을 닫을 때 프런트엔드와 셸은 정리하지만 Codex가 관리하는 공유 서버의 대화는 유지한다. 서버가 이미 실행 중이면 기존 서버를 사용하며 자동 종료·재시작하지 않는다. 실제 쓰기 소유자가 있는 별도 서버의 대화를 R로 빼앗는 동작도 추가하지 않았다.

환경은 GUI 시작 당시 값 대신 호출한 셸의 값을 사용한다. 새 서버는 `CODEX_HOME`, 현재 폴더와 API/도구 환경을 받되 이전 Codex 대화 ID·서버 종료 소켓·MiniTerm의 탭/인스턴스 식별자는 상속하지 않는다. 특정 탭의 식별자를 장기간 살아 있는 공유 서버에 고정하지 않기 위해서다. 환경 값과 사용자 입력을 진단 로그로 출력하지 않는다. 이미 실행 중인 서버의 환경을 덮어쓰는 기능은 아니다.

서버 시작은 최대 20초, helper의 전체 대기는 최대 25초로 제한한다. 작업 중에는 GUI IPC에 시작 완료 여부를 조회하며 GUI 스레드를 대기시키지 않는다. 오류는 해당 호출에 표시한다. 문자나 Enter를 자동 재전송하거나 제품 입력에 고정 지연을 넣지 않는다.

`--no-daemon`, 원격 서버, 프로필/실행별 config와 비대화형 명령은 원래 실행 정책을 따른다. 기존 alias·함수와 명시적 AI Battery 레이아웃도 보존한다. 기본 관리 배터리는 `inline`, 일반 Codex 화면에는 `--no-alt-screen`을 사용한다.

알림 중계의 원격 TUI에는 호출한 Bash의 현재 폴더를 `--cd "$PWD"`로 전달한다. 이를 생략하면 daemon의 내부 작업 폴더를 선택할 수 있다. 사용자가 지정한 `-C`/`--cd`는 우선하며 Bash의 Windows 경로 변환도 유지한다.

## 실제 Bash 실행

Windows Codex에서 `SHELL`이나 프롬프트 지침만 바꾸면 기본 실행 도구가 Bash로 바뀌지 않는다. 설정의 **Codex → Bash 실행 환경 적용**은 `CODEX_HOME/environments.toml`에 `miniterm-git-bash` 환경을 등록한다. MiniTerm의 어댑터가 공식 `codex exec-server --listen stdio`를 실행하고 선택된 Git Bash의 경로와 셸 정보를 제공한다. 나머지 실행·권한 프로토콜은 원래 executor에 전달한다.

등록 후에는 Bash의 `ls`, `printf`, 셸 변수와 파이프를 기본 실행 도구로 사용할 수 있다. Codex가 명시적으로 PowerShell을 선택하는 것까지 막지는 않는다. Windows의 다른 Codex 실행에도 영향을 주므로 설정에서 적용 범위를 안내하고 명시적으로 적용/해제한다. 기존 환경·기본값·주석·notify·인증·개발자 지침을 보존하며 파일 변경 실패 때 소유 기록을 복구한다.

등록을 적용하지 않으면 Windows Codex의 기본 도구는 PowerShell이다. 이번 검증은 격리된 시험 환경에만 등록했고 사용자 CODEX_HOME에는 적용하지 않았다. 실행 중인 서버의 환경 변경을 반영하려면 사용자가 기존 작업을 마친 뒤 서버를 재시작해야 한다. MiniTerm이 사용자 대화를 종료하지 않는다.

## 알림과 남은 범위

실행별 `-c notify=...`는 공유 서버와 다른 실행 경로를 선택할 수 있어 기본 함수에서 주입하지 않는다. 기존 단독 실행의 세션별 알림 재바인딩은 `MINITERM_CODEX_SESSION_NOTIFY=1`의 명시적 선택 경로로 남겼다. `MINITERM_AGENT_BASH=1`의 이전 Bash 지침 방식도 명시적 선택이며 기본 Bash 환경 등록과 구분한다.

2026-10-15부터 알림 등록이 활성화된 기본 공유 실행은 탭별 중계 연결을 사용한다. `codex-prepare`가 기존 daemon을 준비한 뒤 루프백 주소와 실행별 인증 토큰을 반환한다. TUI의 공식 `--remote`·`--remote-auth-token-env`와 `app-server proxy`로 **같은 공유 서버**에 연결한다. 단독 서버나 새 대화를 대신 만들지 않는다. 토큰은 해당 Bash 함수의 지역 환경 변수이며 실행 후 원래 값으로 복원된다.

중계는 성공한 `thread/start`·`thread/resume`·`thread/fork` 응답을 그 탭에 연결하고 `turn/started`·`turn/completed`·승인/입력 요청을 기존 알림 broker에 전달한다. 실패한 재개, 제목 생성 등의 임시(`ephemeral`) 대화, 다른 대화와 하위 에이전트의 이벤트는 제외한다. 완료 뒤 thread 활동만으로 응답 중 상태로 돌아가지 않는다. 응답 본문을 보고 상태를 추정하지 않는다. 상태·대화/turn ID·작업 폴더만 IPC에 전달하며 입력·응답·인증 토큰은 로그에 남기지 않는다. 공식 이벤트의 의미는 [OpenAI Docs](https://learn.chatgpt.com/docs/app-server#events)를 따른다.

기존 notify 설정과 기록이 일치할 때만 중계를 켠다. 등록 해제·사용자가 변경한 콜백·`notify=[]`와 명시적 원격/프로필/실행별 설정은 보존한다. 기존 notify 콜백의 원본 JSON 호출도 유지한다. 프런트엔드 종료·탭 닫기·GUI 종료 시 해당 중계와 소유 proxy만 정리하고 공유 daemon은 유지한다. 닫힌 연결의 늦은 이벤트는 거부한다.

알림 분석은 WebSocket JSON 한 메시지당 4MiB로 제한한다. 초과 메시지는 원래 연결로 계속 전달하지만 알림 분석에서 생략하므로, 큰 재개 응답에서는 해당 실행의 알림 연결을 놓칠 수 있다. 설정·실기·남은 범위는 [공유 알림 구현](SHARED_NOTIFICATIONS.md)과 [알림 안내](../docs/NOTIFICATIONS.md)를 따른다.

GUI 검증은 설치된 Codex·AI Battery·MiniTerm과 고정 localhost SSE 제공자를 사용한다. 첫 입력 문자열, 확장/축소 휠, 초안, 같은 대화 이력, 실제 쓰기 충돌과 소유권 해제 후 소문자 R, 창 종료 뒤 서버 연결을 확인한다. GUI 휠은 실제 Win32 → winit → egui 경로의 합성 메시지다. 물리 장치·한글 IME·사용자 계정/모델·실제 팝업 클릭 검증과 구분한다. Bash 도구 실행을 허용한 fixture에서도 사용자 설정의 권한 정책을 바꾸지 않는다.
