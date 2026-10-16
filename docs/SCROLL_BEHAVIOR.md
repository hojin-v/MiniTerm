# 터미널 화면 모드와 마우스 스크롤

> 2026-10-14: 기본 배터리 레이아웃은 `inline`이며 공유 서버 GUI의 첫 입력·확장/축소 스크롤·초안·재개/R 회귀를 통과했다. [최신 실행 파일과 검증](../feature/CODEX_RECOVERY_HANDOFF.md), [실행 구조](../feature/CODEX_STARTUP.md)를 따른다.

2026-10-14 갱신. 이전 `composite`·`tui` 조사와 수정 이력은 [Codex 스크롤 수정](../feature/SCROLL.md)에 보존한다.

## 원인과 수정

Windows Git Bash의 Codex 브리지는 `--no-alt-screen`을 넣었지만, 설치된 AI Battery의 기본 `composite` 경로는 바깥 터미널을 다시 대체 화면으로 바꿨다. 실제 Codex 0.162.1과 로컬 응답 서버로 실행했을 때 `alternateScreen=true`, `mouseReporting=false`, `historyLines=0`이었다. 기본 휠의 방향키 전송만 제거하면 입력 이력 전환은 막히지만 출력 기록을 탐색할 수 없다.

MiniTerm의 AI Battery 관리 브리지는 기본 레이아웃을 `inline`으로 정했다. Codex를 원래 콘솔에서 실행하고 배터리를 overlay로 표시한다. `tui`/rowpty 경로에서 첫 입력에 RGB 응답이 섞이는 것을 확인했으므로 기본 경로에서 사용하지 않는다. 설정은 해당 Codex 호출에만 전달한다. 사용자가 지정한 `AI_BATTERY_WIN_LAYOUT`, `CLAUDEX_BATTERY_WIN_LAYOUT`, `AI_BATTERY_LAYOUT`은 우선하며, `MINITERM_CODEX_INLINE=0`은 이전 기본 선택으로 되돌린다. AI Battery 없는 EXE/npm 실행에는 배터리 레이아웃을 주입하지 않는다.

휠 처리 뒤 변경된 오프셋으로 같은 프레임의 행을 다시 계산하고 다시 그리기를 요청한다. 축소 입력 화면도 스크롤 중에는 과거 출력 행을 보여주고, 맨 아래로 돌아오면 현재 입력 위치를 다시 따른다. 키 입력은 기존처럼 현재 입력으로 돌아오며, 휠 자체는 프롬프트 초안을 바꾸지 않는다.

## 입력 경로

| 조건 | 동작 |
| --- | --- |
| 입력 화면 본문, 일반 화면, 마우스 보고 없음 | 터미널 출력 기록을 위아래로 이동 |
| CLI가 마우스 보고를 요청함 | 기존 마우스 휠 보고를 CLI에 전달 |
| Shift+휠 | CLI 마우스 보고/방향키 전달을 우회하여 터미널 기록 이동 |
| 대체 화면, 마우스 보고 없음 | 기본 방향키 전달 없음. 대체 화면에 기록이 없으면 이동할 기록도 없음 |
| 대체 화면의 방향키 전달을 사용자가 켬 | DEC 1007 요청과 애플리케이션 커서 모드를 따름 |
| Ctrl+Alt+휠 | 창 투명도 5% 조절, 터미널에는 전달하지 않음 |
| 축소 요약 화면 | 현재 작업 요약 표시를 유지. 터미널 기록은 입력 화면에서 탐색 |

사용자 alias/함수, `command codex`, EXE 직접 호출, WSL의 Linux CLI는 Windows 브리지 자동 설정을 거치지 않을 수 있다. 기본 터미널 기록을 원하는 직접 실행에는 `codex --no-alt-screen`을 사용한다. AI Battery의 `composite`를 명시한 경우에는 다음처럼 해당 호출의 레이아웃을 선택할 수 있다.

```bash
AI_BATTERY_WIN_LAYOUT=inline codex --no-alt-screen
```

이전에 대체 화면에서 사라진 기록은 새 실행 파일로 복원할 수 없다. 새 MiniTerm 셸에서 Codex를 다시 실행해야 새로운 기본 실행 경로가 적용된다. 설치된 AI Battery의 inline overlay를 실제 GUI에서 검증했다. WSL Linux CLI의 실제 화면은 이 검증에 포함하지 않았다.

## 검증 범위

설치된 Codex CLI, 실제 MiniTerm GUI와 ConPTY, 고정 로컬 SSE 응답, 고정 배터리 읽기를 사용했다. 시험 소유 창에 Win32 `WM_MOUSEWHEEL`을 보내 winit과 egui의 실제 입력 경로를 통과시켰다. 확장/축소 입력 화면의 위/아래 이동, 전후 세션 보존, 작성 중인 입력 초안의 동일한 서버 수신을 검사한다. 실제 계정/모델 호출이나 물리 휠 장치 검증을 뜻하지 않는다.

스크롤 진단은 `MINITERM_DIAGNOSTICS=1`과 별도 `MINITERM_INSTANCE_ID`가 있는 시험 인스턴스에서만 버퍼 모드·기록 행 수·오프셋·휠 수치를 반환한다. 사용자 터미널 문장은 반환하지 않는다. 시험 방법과 산출물 경로는 [수정 기록](../feature/SCROLL.md)에 있다.
