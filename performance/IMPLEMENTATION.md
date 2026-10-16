# CLI 에이전트의 Bash 실행 연결

> 2026-10-13 17:14 KST 후속 중단: [Codex 입력·재개·Bash 연결 인계](../feature/CODEX_RECOVERY_HANDOFF.md)를 먼저 읽는다. 실행별 Bash 지침 주입이 공유 서버 재개와 충돌하여 현재 소스의 기본 주입은 꺼지고 명시적 선택으로 남았다. 기본 Bash 실행 환경 등록 방식의 격리 CLI 검사는 통과했지만 공유 서버 GUI는 미완료이며 사용자 설정에는 아직 적용하지 않았다. 아래 사용법·검증은 이전 구현 당시의 기록이다.

2026-10-12. 사용자가 `performance/performance.md` 구현을 요청해 이 요구사항의 개발을 재개했다. 기존 기능·디자인 변경은 유지하며 공유 UI 파일은 다른 세션에서 작업한다.

## 구현

MiniTerm의 Git Bash에서 `codex`를 시작하면 Bash 문법을 사용하는 세션 지침과 `exec_command`의 `shell="bash"`, `login=false` 사용 지침을 전달한다. Windows 기본 셸을 나타내는 `SHELL` 변수만으로 Codex 실행 도구가 바뀌지 않으므로 `features.unified_exec=true`도 실행 인자로 전달한다. 기존 공유 daemon이 다른 PATH를 가지고 있을 수 있어 해당 실행에는 `--no-daemon`을 사용한다. 기존 sandbox·승인·모델·인증 설정은 변경하지 않는다.

설치된 Codex의 stdio `config/read`로 적용되는 개발자 지침을 읽어 Bash 지침을 덧붙인다. 이 조회는 모델 요청 없이 실행하며 3초 제한과 소유 프로세스 트리 종료를 적용한다. 현재 Codex app-server는 `--profile`을 지원하지 않아 이름을 검증한 사용자 `*.config.toml`의 개발자 지침을 별도로 반영한다. 프로젝트·명령행·상위 관리 설정의 개발자 지침이 있으면 그 값을 보존한다. 프로필이 프로젝트 신뢰/루트 또는 legacy profile 구조까지 변경하면 자동 연결을 생략하고 이유를 표시한다. 이 경우 기존 Codex 실행은 계속 가능하다.

일반 npm 설치, 직접 `codex.exe`, 기존 AI Battery Node 브리지를 지원한다. 원본 실행 파일이나 설치 스크립트는 수정하지 않는다. 사용자 함수·별칭은 보존하며 직접 절대 경로로 실행하는 Codex에는 이 함수가 적용되지 않는다. 사용자 명령행 인자는 주입된 기본값 뒤에 전달하므로 명시한 개발자 지침·기능 설정을 우선한다.

실제 프로필의 `bash.exe` 경로를 `MINITERM_BASH_EXE`로 전달하고 그 폴더를 세션 PATH 앞에 둔다. Windows의 System32 WSL Bash 실행기로 잘못 연결되는 것을 방지한다. Claude Code에는 설정되지 않은 `CLAUDE_CODE_GIT_BASH_PATH`를 해당 경로로, `CLAUDE_CODE_USE_POWERSHELL_TOOL`을 `0`으로 지정한다. 사용자가 이미 설정한 값은 유지한다. 사용자 초기화 파일과 전역 PATH·에이전트 설정 파일은 수정하지 않는다.

WSL 프로필은 기존 Linux 실행 경로를 유지한다. Linux용 CLI 에이전트를 설치해 실행해야 Linux의 Bash 실행 도구를 사용한다. Git Bash에서 실행하는 모든 Windows 프로그램의 내부 셸을 일괄 변경하는 기능은 아니다. Gemini CLI의 Windows 기본 도구는 PowerShell이므로 이 요구에는 WSL의 Linux용 Gemini를 사용한다.

## 사용과 검증

새 MiniTerm Git Bash 세션에서 평소처럼 `codex` 또는 `claude`를 실행한다. Codex의 Bash 지침을 한 번 건너뛰려면 `MINITERM_AGENT_BASH=0 codex`를 사용한다. Claude의 명시적 환경 설정도 유지한다. 이미 실행 중인 에이전트에는 소급 적용되지 않는다.

```powershell
./scripts/test.ps1
./scripts/test.ps1 -AgentBash
./scripts/test.ps1 -WindowsBridge
./scripts/test.ps1 -CodexNotify
```

`-AgentBash`는 일반 검사와 별도다. 설치된 Git Bash·Node·Codex가 필요하며 외부 모델 호출 없이 로컬 SSE fixture와 모의 npm 실행기를 사용한다. 기존 `-Pty`의 호환성 검사 집합에는 새 환경 의존 검사를 추가하지 않았다.

확인한 범위:

- 실제 Codex 0.162.1의 실행 도구에서 여러 줄 Bash, `$()` 변수 대입, 파이프, 정렬, heredoc, 한글·공백·작은따옴표 경로 실행.
- 원래 개발자 지침 및 명시된 사용자 프로필 지침 보존, 격리된 설정 파일 불변.
- 일반 npm 실행 경로에서 TTY, 리터럴 argv, 종료 코드, 명시된 CLI 옵션 우선 및 Bash 지침 opt-out.
- Claude Bash 환경 변수 전달. 실제 Claude 모델의 도구 선택은 별도 확인이 필요하다.

검사 근거와 최종 빌드 정보는 아래 최종 결과에 기록한다. 최초 fixture는 Codex 자체의 첫 사용 프로젝트 신뢰 기록 때문에 파일 불변 검사가 실패했다. 격리된 신뢰 기록을 미리 준비해 연결 코드의 변경과 분리했다. 다음 실행에서는 app-server의 프로필 미지원이 확인돼 위 프로필 처리를 추가했다. 실패 폴더와 transcript도 삭제하지 않는다.

## 한계와 다음 확인

실제 모델이 매번 Bash를 선택한다는 보장은 아니다. Codex 도구에서 `shell`을 생략하면 여전히 Windows 기본 셸이 선택되므로 세션 지침으로 Bash를 지정하도록 한다. 로컬 fixture는 이 지침 전달과 지정된 Bash 실행 경로를 검증한다. 실제 계정에서 모델이 생성하는 명령의 자동 선택은 다음 확인 항목이다. 새 CLI 버전/다른 설치 형태는 별도 검증하며 구버전에서는 `MINITERM_AGENT_BASH=0`으로 원래 실행을 사용한다.

메모리 200MB, 유휴 CPU, 입력→pixel, 작업 속도 비교는 이번 Bash 연결 검사의 판정 범위에 포함하지 않는다. Bash 사용으로 모델 성능이나 작업 시간이 개선됐다는 수치를 주장하지 않는다.

설정 및 실행 동작의 근거: [OpenAI Codex 설정](https://learn.chatgpt.com/docs/config-file/config-reference), [Codex app-server](https://learn.chatgpt.com/docs/app-server), [Codex 0.162.1 실행 도구 소스](https://github.com/openai/codex/blob/rust-v0.162.1/codex-rs/core/src/tools/handlers/unified_exec.rs), [Claude Code 환경 변수](https://code.claude.com/docs/en/env-vars), [Gemini CLI 셸 도구](https://geminicli.com/docs/tools/shell/).

## 최종 결과

- 일반 검사 66개(단위 65개·SSE fixture 1개) 통과. 환경 의존 ignored 검사는 이 수에 포함하지 않는다.
- 별도 실기 4개 통과: Bash 실행/프로필, npm 실행/opt-out, Windows 브리지/TTY/버전, Codex 알림/콜백 보존.
- 동시 UI 작업과 소스 근거를 구분하기 위해 `artifacts/perf-20261012T133106Z-d0a60cc1-6564-463d-9659-85a33d7fb5c6/source`에 소스를 보존하고 이 사본으로 release 빌드를 확인했다. 작업 폴더의 이후 UI 변경은 이 빌드에 포함됐다고 가정하지 않는다. GUI 시험은 실행하지 않았다.
- 실행 파일: 같은 폴더의 `miniterm.exe`, SHA256 `d6b3e1bfe082e65f6c98d3638d3791b7461170dbc144ab907ae07b8357162fca`. 소스 지문 `ff9d216bbb8f51ccbe6f9e6a075f48c5e5da146b99a207c26683ddba02cdcddd`. `manifest.json`에 버전·소스·해시를 기록했다.
- release 실행 파일에서도 격리된 Codex 설정을 읽고 기존 지침에 Bash 지침을 덧붙이는 helper 경로를 확인했다. `release-helper.json`의 `passed=true`. PowerShell 5의 Python stdin 전달에서 한글 경로가 손상됐던 추가 검사 실패는 `release-helper-invalid-fixture.json`에 보존하며, 실제 경로를 파일시스템에서 찾아 전달한 검사로 구분했다.
- 사본 기준 Bash 실행 근거: `source/artifacts/agent-bash 한글 ' 423602b9-0252-43b7-8357-4fba6bddf4ff/{verification.json,tool-output.json,transcript.txt,profile-transcript.txt}`. npm 근거: `source/artifacts/agent-bash-npm-22389703-c114-4b31-b69e-def6d7200a96/verification.json`.
- 알림 회귀 근거: `artifacts/codex-notify-2eefc04c-53f6-4348-bbad-0e914e928036`.
- 사본을 검증할 때도 실제 상위 Git 저장소의 첫 사용 신뢰 기록이 추가되는 fixture 한계를 확인했다. 시작 폴더와 상위 저장소를 격리 설정에 함께 준비한 뒤 재검증했다. 제품의 Bash 연결 코드는 바뀌지 않았으며 실패 폴더 `source/artifacts/agent-bash 한글 ' 768ea221-eae2-4fce-8419-89847d95e653`를 보존한다.

`artifacts/`는 Git 제외다. 새 clone에는 이 실행 파일과 원시 근거가 없으므로 위 검사 명령으로 해당 환경을 재검증한다. 실제 계정에서 Bash 도구 자동 선택을 관찰하는 것이 다음 작업이다.
