# 진행률과 현재 작업 표시 방법

요약의 진행률은 **에이전트가 보고한 전체 계획 중 완료한 항목의 비율**이다. 작업 시간이나 실제 명령의 성공 여부를 추정하지 않는다. 3개 중 1개 완료는 33%, 2개는 66%, 전부 완료는 100%다. 계획에 항목을 추가하면 비율이 낮아질 수 있다.

## 사용 방법

1. MiniTerm 안에서 이 프로젝트의 에이전트를 실행한다. 설정 → **창과 동작 → 미니 화면 → 작업 요약**을 선택하고 **실행 상태 · 계획 진행률**, **계획 작업 목록**을 켠다. 상단 아이콘으로도 입력/요약을 전환할 수 있다.
2. 에이전트에 `$miniterm-progress`를 사용해 계획과 현재 작업을 보고하도록 요청한다. 이 프로젝트의 [스킬](../.agents/skills/miniterm-progress/SKILL.md)이 새 작업은 `begin`, 항목 완료·현재 작업 변경·중단은 `update`로 보고하도록 안내한다. 스킬과 Python이 있어야 한다. 다른 프로젝트에는 이 스킬이 자동 설치되지 않는다.
3. 에이전트는 명시적인 전체 계획을 보내고, 매 갱신마다 계획 전체와 상태를 다시 보낸다. 앱이 비율을 계산하므로 에이전트가 임의의 퍼센트를 보내지 않는다.

MiniTerm이 셸에 넘긴 `MINITERM_SESSION_ID`와 `MINITERM_NOTIFY_EXE`를 [Python 어댑터](../.agents/skills/miniterm-progress/scripts/report.py)가 사용한다. 사용자가 세션 ID를 찾거나 전역 CLI 설정을 바꿀 필요는 없다. 환경 변수를 제거한 별도 실행 환경이나 이전 helper에서는 지원되지 않을 수 있다. 어댑터는 앱과 helper의 `progress-v1` 지원 여부를 먼저 확인한다.

Git Bash에서 직접 보고를 시험하는 예시다. 프로젝트 루트에서 실행하며, 실제 작업을 보고할 때는 항목과 상태를 실제 계획에 맞춘다.

```bash
python .agents/skills/miniterm-progress/scripts/report.py begin <<'JSON'
{
  "title": "설정창 개선",
  "state": "running",
  "tasks": [
    {"text": "화면 분석", "state": "completed"},
    {"text": "설정창 수정", "state": "running"},
    {"text": "동작 검증", "state": "pending"}
  ]
}
JSON
```

위 예시는 **33% · 1/3 완료 · 진행 중**, **진행: 설정창 수정**을 표시한다. 출력된 영수증의 `workId`와 `revision`을 보관한다. 갱신은 `python …/report.py update --work-id <workId> --revision <이전 revision + 1>`에 UTF-8 JSON 전체 계획을 전달한다. `begin`의 revision은 1이므로 첫 update는 2다. `read`는 이 탭의 보고만 조회하고, `clear --work-id … --revision …`는 보고를 지운다.

| 필드 | 허용 값과 규칙 |
| --- | --- |
| `title` | 짧은 작업명, 최대 128자 |
| 작업 `state` | `running`, `completed`, `paused`, `failed` |
| `tasks` | 전체 계획 1~64개, 각 항목의 `text`는 최대 256자 |
| 항목 `state` | `pending`, `running`, `completed`, `stopped` |
| 크기 | JSON 최대 48KiB, UTF-8 stdin |

현재 작업은 첫 `running` 항목이며 없으면 첫 `pending` 항목이다. 진행 중인 항목은 하나로 보고하면 화면이 명확하다. `paused`·`failed`에서는 멈춘 작업을 표시하고 완료 비율은 유지한다. 모든 항목이 `completed`여야 작업 상태 `completed`를 허용한다. 응답 종료 알림만으로 계획을 완료시키지 않는다.

Enter 또는 Ctrl+C를 실제 PTY에 보내면 이전 보고와 영수증이 만료된다. 승인 화면의 Enter도 해당한다. 따라서 갱신은 에이전트의 도구/subprocess에서 호출한다. 사용자가 다음 update 명령을 입력하고 Enter를 누르면 기존 영수증부터 만료되므로, 수동 시험은 하나의 스크립트 안에서 begin과 update를 연속 호출하거나 현재 계획으로 새 begin을 사용한다. 에이전트가 승인 뒤 이어서 작업할 때도 새 `begin`이 필요하다. 늦은 update나 다른 작업의 영수증은 거부한다. 보고는 탭별 메모리에 유지하며 앱 재시작 시 복구하지 않는다.

## 구현 경로와 표시 범위

`report.py` → [helper CLI](../src/main.rs)의 `progress` → [로컬 IPC](../src/ipc.rs) → [세션 브로커](../src/agent.rs)의 `progress` → [보고 슬롯](../src/progress.rs) → [요약 모델](../src/mini.rs)의 `with_report` → [요약 그리기](../src/app.rs)의 `paint_summary` 순서다. 작업 ID와 revision을 검증하고 선택된 탭의 보고만 표시한다. 제목은 프로그램의 OSC 제목이 있으면 이를 우선한다.

직접 보고가 없을 때는 VT 화면에 명시된 계획을 분석한다. 최근 기록 128행과 현재 화면, 행당 512칸·항목 64개로 범위를 제한하며, 일부가 접히거나 잘린 계획은 퍼센트를 표시하지 않는다. 명시적인 계획도 없으면 실행 상태·최근 활동 또는 **작업 계획 없음**을 표시한다. 화면 분석은 revision이 바뀔 때만 계산한다. 직접 보고는 화면 스크롤과 관계없이 유지하며 실제 AI Battery 행도 보존한다.

내용이 폭을 넘는 요약 행에만 툴팁 하나를 띄워 전체 내용과 보고 출처·시각을 제공한다. 다 보이는 문장을 호버로 반복하지 않는다.

스킬이 실제 에이전트에게 선택되고 계속 갱신되는지는 에이전트의 이행에 달려 있다. 모든 CLI 작업을 자동으로 수집하는 기능은 아니다. transcript·작업 파일·계정 사용량 API를 읽거나 모델을 호출하지 않는다. 알림 등록 버튼과 계획 보고는 별개의 경로다.

## 검증

```powershell
./scripts/test.ps1
./scripts/build.ps1
./scripts/verify-summary.ps1
```

`verify-summary.ps1`은 별도 앱·데이터·IPC에서 실제 프로젝트 스킬 어댑터로 한글 계획을 보내고, 33%·66%·100% 막대 폭과 현재 작업·일시정지·거짓 완료 거부·탭 간 분리·clear를 검사한다. 잘린 행과 다 보이는 행의 실제 호버도 확인한다. 실제 계정/모델의 스킬 자동 선택을 통과로 간주하지 않는다. 이번 빌드의 결과와 캡처는 [후속 구현 결과](FOLLOWUP.md)에 기록한다.
