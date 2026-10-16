# Codex 스크롤 수정

> 2026-10-14 현재: 공유 서버를 먼저 준비하는 release GUI에서 첫 입력의 RGB 오염 없음·확장/축소 휠·초안·세션 보존과 대화 재개/R 복구를 확인했다. 기본 레이아웃은 `inline`이다. [실행 구조](CODEX_STARTUP.md)와 [최신 실행 파일·검증 근거](CODEX_RECOVERY_HANDOFF.md)가 아래 역사 기록보다 우선한다.

> 2026-10-13 17:14 KST 후속 중단: 최신 상태는 [Codex 입력·재개·Bash 연결 인계](CODEX_RECOVERY_HANDOFF.md)를 따른다. 아래 첫 검사에서 RGB가 첫 요청에 섞였음을 뒤늦게 확인했다. 기본 배터리는 현재 소스에서 `inline`으로 변경했고 별도 서버 GUI 회귀는 통과했으나 공유 서버 GUI는 미완료다. 아래 `tui`·최종 실행 파일 설명은 당시 기록이다.

2026-10-13. 사용자가 최근 실행 파일에서 Codex 휠이 동작하지 않는다고 확인한 후 재현·수정했다. 로컬 실행 파일만 갱신하며 공개 릴리스와 설치된 사용자 앱은 변경하지 않았다.

## 변경

- `src/shell.rs`: AI Battery 관리 Codex 브리지의 기본 레이아웃을 `tui`로 지정한다. 설치된 rowpty가 출력 기록을 보존하고 배터리의 마지막 행을 예약한다. Codex의 `--no-alt-screen`, Bash 도구 연결, 알림 인수는 유지한다. 레이아웃은 해당 호출에만 전달하며 사용자 환경의 명시적 레이아웃과 기존 alias/함수를 보존한다. AI Battery 없는 실행은 변경하지 않는다.
- `src/app.rs`: 휠/키 입력 후 최종 오프셋으로 같은 프레임의 표시 행을 다시 계산한다. 휠로 오프셋이 바뀌면 다시 그리기를 요청해 CLI의 다음 출력에 의존하지 않는다.
- `src/mini.rs`: 축소 입력 화면도 스크롤 중에는 과거 출력 행을 표시한다. 아래로 돌아오면 현재 입력 위치를 따른다.
- `src/ipc.rs`: 격리된 진단 인스턴스에서 버퍼 모드·기록 행 수·오프셋·휠 수치만 조회할 수 있다. 터미널 문장은 노출하지 않는다.

기존 배터리 `composite`는 Codex에 `--no-alt-screen`을 전달한 상태에서도 바깥 화면을 대체 화면으로 바꿨다. 진단만 추가한 수정 전 실행 경로에서 실제 Codex 0.162.1의 `alternateScreen=true`, `mouseReporting=false`, `historyLines=0`을 확인했다. 방향키 전달 제거만으로는 이 화면을 스크롤할 수 없다. 당시 기록은 `artifacts/scroll-6c84a19e-3ebb-43be-b362-42c07315b9c1/`에 보존했다.

## 최종 실행 파일

- 경로: `artifacts/miniterm.exe`
- SHA256: `d9b8bbc0e63e63b8c3ea0178c2751a63e5aa68b1c2c112d203dfd1d54c79b40a`
- sourceFingerprint: `f0e0b9426d9159cfb15696a302eef3a942cd895840d71da4c050bb7a244ec272`
- builtUtc: `2026-10-13T05:27:04.1049119Z`

## 통과한 검사

| 검사 | 근거와 범위 |
| --- | --- |
| 자동 검사 70개 | 69개 단위 검사와 로컬 SSE 전송 검사 1개. 새 Bash 브리지 검사는 실제 Git Bash에서 기본 `tui`, 명시적 레이아웃, 자동 인라인 해제, 배터리 없는 실행, 인수·부모 환경·기존 함수 보존을 확인한다. 새 축소 화면 검사는 과거 행 표시와 현재 커서 보존/복귀를 확인한다. 환경 의존 ignored 검사는 통과 수에 포함하지 않는다. |
| 실제 Codex + AI Battery | `artifacts/scroll-e9533cca-f69a-42e2-ad71-eee52400bbc6/verification.json`. 실제 설치된 Codex 0.162.1, MiniTerm GUI, ConPTY와 rowpty를 사용했다. 고정 SSE 응답 180행 이후 기록 154행, 오프셋 0 → 20 → 0을 확인했다. 확장/축소 입력 화면의 위/아래 스크롤, 같은 세션 유지, `SCROLL-DRAFT-KEEP-TAIL` 초안의 동일한 서버 수신을 통과했다. |
| 실제 Codex, 배터리 없는 EXE 경로 | `artifacts/scroll-4d920b96-a181-4210-927e-444df4e485fe/verification.json`. 설치된 EXE를 PATH 앞에 놓은 시험 소유 자식 환경에서 같은 스크롤·축소·초안·세션 검사를 통과했다. 사용자 PATH는 변경하지 않았다. |
| 목표 재개 메뉴 회귀 | `artifacts/goal-hud-8e72ea4a-d676-4bf7-8054-c336f30f933a/verification.json`. 시험 소유 중단 목표 기록으로 실제 Codex 재개 메뉴를 열었다. 새 rowpty 경로에서 입력 영역에 붙은 위치, 다크/라이트 표시, 선택 후 메뉴 제거, 기존 앱 보존과 시험 Bash 종료를 통과했다. |

스크롤은 시험 소유 창에 Win32 `WM_MOUSEWHEEL`을 보내 실제 winit/egui 경로로 검사했다. 물리 휠 장치 검증과 구분한다. 전역 `mouse_event` 합성은 이 데스크톱에서 요청과 반대 부호로 관측돼 실패 기록을 남겼으며, 원인은 확정하지 않았다. 제품의 스크롤 방향을 뒤집어 시험을 통과시키지 않았다.

응답·배터리·설정·IPC·재개 기록은 모두 격리했다. 실제 계정/모델 요청, 사용자 인증/설정/세션 편집은 하지 않았다. 첫 재개 회귀 시도는 시험 설정에 상위 프로젝트 신뢰 정보가 빠져 메뉴까지 진입하지 못했다. 시험 소유 CODEX_HOME의 신뢰 정보를 보완한 후 최종 검사에 통과했고 실패 폴더도 보존했다. 기존 설치 Git의 빠른 resize/입력 실패 4개는 이번 수정의 완료 범위가 아니다.

## 다시 검사하기

GUI 검사는 같은 데스크톱에서 순서대로 실행한다.

```powershell
node scripts/verify-scroll.cjs
node scripts/verify-scroll.cjs direct
```

관리 브리지의 `composite` 상태를 의도적으로 비교하려면 `node scripts/verify-scroll.cjs composite`를 사용한다. 이 경로의 출력 기록 이동 실패를 통과로 바꾸지 않는다. 목표 재개 회귀는 `verify-codex-ui.cjs`의 시험 소유 `goal-seed`를 만든 뒤 `node scripts/verify-goal-hud.cjs artifacts/codex-ui-<seed-id>`로 실행한다. 사용자 세션 폴더는 사용하지 않는다.

## 적용

새 실행 파일로 MiniTerm을 실행하고 새 셸에서 Codex를 다시 시작해야 새 브리지가 적용된다. 이미 대체 화면에서 버린 기록을 복원하는 변경은 아니다. 사용자 지정 레이아웃·alias·함수나 WSL Linux CLI 등 자동 적용 범위 밖의 경우는 [스크롤 동작 안내](../docs/SCROLL_BEHAVIOR.md)에 기록했다.
