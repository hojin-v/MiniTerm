# MiniTerm에서 Codex 직접 사용하기

현재 MVP에서는 Git Bash를 통한 Windows용 Codex 실행, 입력/화면 표시, 세션 유지와 응답 완료 알림을 직접 시험할 수 있다. 실제 IME·Codex TUI·Windows 팝업 클릭은 이 사용 시험에서 확인할 항목이다.

## 시작

**현재 실행본:** 사용자 요청으로 이전 창을 종료하고 최신 검증 빌드를 기존 설치 경로에 교체해 실행했다. 현재 기록은 `artifacts/manual-test/latest-running.json`이며 새 Bash 세션이다. 상단은 새 탭·입력/요약 전환·숨기기만 제공하고, 설정은 트레이 메뉴에서 연다. 향후 명시적으로 최신본으로 다시 전환할 때 `scripts/restart-latest.ps1`을 사용할 수 있다. 이 스크립트는 실행 중인 MiniTerm과 그 셸/Codex 작업을 종료하고 실행 파일·설정 백업 후 검증 빌드로 재실행한다.

Windows 시작 메뉴의 **MiniTerm**을 실행한다. 이 장치에는 검증한 빌드를 `%LOCALAPPDATA%\Programs\MiniTerm\miniterm.exe`에 설치했고, 기본 시작 폴더를 이 프로젝트로 지정했다. 설정의 종 모양 탭에서 Windows 알림과 Codex 연결을 관리한다. 연결 전 Codex 설정의 사본을 보존하고, 기존 notify 콜백도 함께 호출한다.

MiniTerm의 Git Bash에서 다음을 실행한다.

```bash
codex --no-daemon
```

설치된 Codex CLI 0.162.0의 `--help`에서 `--no-daemon` 지원을 확인했다. 이 시험에서는 별도 서버로 실행해 MiniTerm 세션 환경을 그대로 상속하도록 한다. 모델·인증·승인 설정은 기존 Codex 설정을 사용한다. 다른 폴더에 있다면 먼저 `cd /c/projects/MiniTerm`으로 이동한다.

창을 확장한 상태로 시작한다. 이전 설치본에서는 핀 아이콘으로 확장을 유지하며 새 UI에서는 창 설정의 자동 축소를 끈다. 최신 MiniTerm 생성 Windows 연결은 inline 모드가 기본이다. 전체 화면과 비교할 때는 Bash에서 `MINITERM_CODEX_INLINE=0 codex --no-daemon`을 사용한다. 한글 IME는 별도로 확인한다.

## 입력과 표시

1. 한글을 조합해 입력하고 Backspace·방향키·Enter·조합 취소를 확인한다.
2. 여러 줄 붙여넣기, 긴 답변, 코드 블록, 스크롤과 선택 복사를 확인한다.
3. Codex 실행 중 확장·축소·숨김·복귀와 세션 전환을 해 본다. 같은 작업이 이어지는지 확인한다.
4. 명령 실행 중 Ctrl+C가 정상적으로 중단되는지 확인한다. 셸 작업 제어는 Codex를 종료한 Bash에서 `sleep 30`, Ctrl+Z, `jobs`, `bg`, `fg`, Ctrl+C 순으로 확인할 수 있다.

## 응답 완료 알림

Codex에 다음과 같이 입력한 뒤 바로 다른 앱으로 전환한다.

```text
명령으로 10초 기다린 다음 '알림 테스트 완료'라고 답해. 파일은 변경하지 마.
```

기대 동작은 다음과 같다.

- 다른 앱이나 다른 세션을 보고 있을 때 완료되면 세션 번호에 읽지 않은 배지가 생기고 Windows 알림이 전달된다.
- Windows 알림을 클릭하면 해당 세션을 선택하고 창을 확장해 돌아온다. 클릭은 명령을 다시 실행하거나 Enter를 보내지 않는다.
- 같은 세션을 계속 보고 있을 때는 Windows 팝업을 생략한다.
- 알림은 Codex의 응답 종료를 뜻한다. 작업 성공·승인 요청·모든 백그라운드 작업 종료를 뜻하지 않는다.

OpenAI Docs의 [notify 설정](https://learn.chatgpt.com/docs/config-file/config-advanced#notifications)은 현재 `agent-turn-complete` 이벤트를 제공한다. Windows 방해 금지와 앱별 알림 설정은 팝업 표시를 좌우하므로, 팝업이 없으면 트레이의 알림 설정과 Windows 알림 센터를 확인한다.

## 이 터미널에서 개발을 이어갈 때

새 Codex 세션에는 다음과 같이 작업을 넘길 수 있다.

```text
docs/의 문서를 읽고 docs/MATURITY.md의 남은 구현을 이어서 진행해.
나는 현재 설치된 MiniTerm에서 이 Codex를 실행해 입력·TUI·알림을 실사용 테스트하고 있다.
현재 실행 중인 MiniTerm과 그 세션은 유지해. 우선 발견한 결함과 기존 동작 보존을 중심으로 수정하고 검증해.
자동 GUI/실제 IPC 시험은 이 앱을 종료해야 하므로, 그 조건을 확인하고 실행해.
```

설치된 앱과 작업 폴더의 빌드 산출물은 별도 경로이므로 앱을 사용하면서 새 빌드를 만들 수 있다. 변경한 UI를 사용하려면 작업을 마친 뒤 MiniTerm을 종료하고 검증한 새 설치본을 설치해 다시 실행한다. `verify-ux.ps1`과 현재 `verify-appearance.ps1`은 별도 설정·IPC 이름으로 사용 중인 앱과 함께 실행할 수 있다. `verify-window.ps1`, `verify-session.ps1`과 이름을 분리하지 않은 실제 IPC 시험은 앱을 종료한 상태에서 실행한다.

**2026-10-11 검증 제한:** 새 개발 빌드에서 크기 변경 직후 Bash 명령의 첫 글자가 빠지는 회귀 시험이 실패했다. 새 UI는 별도 창에서 검증했으며, 이 입력 문제는 아직 해결되지 않았다. 현재 설치본을 교체하지 않았다. AI Battery의 Windows launcher 연결은 실제 Codex 버전 조회로 확인했지만 대화형 HUD와 실제 모델 대화는 사용 시험에서 확인해야 한다. 이 장치의 WSL은 `/bin/true` 직접 실행도 `Wsl/Service/E_UNEXPECTED`로 실패하므로 현재 WSL 시험은 통과 상태가 아니다.

문제를 기록할 때에는 실행 명령, 입력 내용의 종류, 확장/축소 상태, 재현 순서와 기대/실제 동작을 적는다. 민감한 프롬프트나 인증 정보는 기록할 필요가 없다. 실제 에이전트와 팝업 클릭은 직접 통과를 확인한 뒤 검증 기록을 갱신한다.

## 계정과 분리한 Codex 메뉴 검증

현재 장치에 설치된 Windows Codex와 AI Battery의 Node/PTY 의존성을 사용한다. 패키지 자동 설치는 하지 않는다.

```powershell
.\scripts\verify-codex-ui.ps1
.\scripts\verify-codex-ui.ps1 -Battery
.\scripts\verify-codex-ui.ps1 -Goal
.\scripts\verify-codex-ui.ps1 -Goal -Battery
```

새 테스트 홈의 한글 세션으로 모델/재개 메뉴를 열고 닫으며 크기를 바꾸고, 그 VT 출력을 MiniTerm 엔진에 재생해 기준 화면과 비교한다. 로컬 제공자는 고정 오류만 반환하며 실제 모델 서버를 호출하거나 사용자 설정·인증을 복사하지 않는다. 배터리 수치도 고정 로컬 값이다. 실제 대화 중 메뉴 잔상·IME·GUI 픽셀·알림 검증과 구분한다. 원시 결과는 `artifacts/codex-ui-<UUID>`에 저장된다.

`-Goal`은 자체 한글 기록과 정지된 목표를 만든 뒤, 종료된 테스트 세션만 복사해 실제 `Resume paused goal?` 선택창의 크기 변경·닫기·다음 입력을 비교한다. 이 선택지는 Node의 `node:sqlite`를 제공하는 버전과 현재 검증한 Codex 0.162.1의 저장 형식을 사용한다. 사용자 세션이나 인증을 읽어 seed로 쓰지 않는다. 로컬 HTTP 오류로 만든 기록이며 실제 도구 실행이 진행되는 동안의 잔상 재현과는 구분한다.

`scripts/verify-ux.ps1 -IdleSeconds 30 -SessionCount 4`로 최대 네 개의 테스트 Bash 세션을 열고 앱 유휴 자원을 측정할 수 있다. 테스트 앱 종료 후 해당 Bash 종료도 확인하며 사용자 창은 유지한다. 앱 CPU/working set 측정이고 GPU·Bash 자체 자원·출력 부하·입력→pixel 지연은 포함하지 않는다.

알림 전달은 `scripts/test.ps1 -CodexNotify`로 별도 검사한다. 실제 CLI가 로컬 고정 SSE 응답을 완료하면 현재 helper·별도 IPC/Broker에 이벤트가 도착하는지, 기존 콜백·프로필·비활성화·사용자 별칭과 함수를 보존하는지 확인한다. 실제 계정/모델이나 Windows 팝업은 사용하지 않는다. 새 실행별 연결 코드는 새 빌드의 새 셸 세션에 적용되고, 현재 열어 둔 사용자 창에는 자동으로 적용되지 않는다.

## 격리 MSYS 수정 후보

자체 SDK에서 컴파일한 후보 v3는 즉시 raw/canonical 입력 각각 1,000회, SIGWINCH/siglongjmp 1,000회와 기존 연동 10개를 통과했다. 제품 기본 프로필/설치 Git에는 아직 적용하지 않았다. [원시 기록과 재현 절차](patches/MSYS_RUNTIME_VALIDATION.md)를 확인한다.

`MINITERM_GIT_TEST_BASH`를 지정하면 `test.ps1 -Pty`, `-WindowsBridge`, `-CodexNotify`의 모든 Git 검사가 해당 Bash를 사용한다. 기본 Git의 기존 실패와 격리 후보의 통과를 구분한다. `verify-ux.ps1 -BashPath <usr/bin/bash.exe> -SummarySample -ProgressSample`은 자체 설정과 실제 자식 경로·DLL SHA를 확인하며 사용자 창을 유지한다. 후보의 최신 GUI 기록은 `artifacts/ux-5f655c3b-8f81-4bd9-9f8d-eeb4c6d8248e`다. 실험 경로를 제품 설치 경로로 사용하지 않는다.

## 작업 요약 확인

화면 구성 아이콘에서 작업 요약을 선택한다. 프로그램 제목은 상단 표시줄에, `작업 중 · 58초` 같은 상태·계획 진행률·현재/최근 완료 작업은 본문에, 실제 AI Battery는 하단에 표시한다. 계획이 없는 경우 내부 명령문 대신 `최근 활동: 파일 내용 확인` 같은 활동명을 표시한다. 설정의 창 탭에서 각 표시 항목을 고를 수 있다. 창을 작게 줄이면 일부 작업 줄을 생략하고 호버 안내 하나로 긴 작업명과 계획을 확인한다. 한 번 클릭은 창 크기를 유지하고, 본문·제목의 더블 클릭은 확장·축소한다.

진행률은 명시된 계획의 완료 항목 비율이다. 계획이 없는 작업이나 목록 일부가 잘린 경우에는 표시하지 않으며 응답 종료를 전체 작업 성공으로 계산하지 않는다. 에이전트가 계획을 출력하는지와 실제 호버/마우스 동작은 사용 시험에서 확인한다. [동작·데이터·검증 범위](MINI_SUMMARY.md)를 확인한다.

`scripts/verify-ux.ps1 -SummarySample`은 별도 진단 창의 다크/라이트·작은 창·계획 없는 요약, 단일 클릭 크기 유지, 더블 클릭 확장과 본문 위·가운데·아래 축소를 검사한다. 긴 한글 작업명에 Windows 포인터를 잠시 올려 툴팁이 하나인지 확인하고 원래 위치로 복원한다. 내부 명령문을 활동명으로 표시하는 샘플도 제공한다. 고정 VT 샘플이며 실제 모델의 계획 이벤트를 생성하거나 사람의 호버 스크롤을 확인하는 시험은 아니다. 최신 fixture는 `artifacts/ux-a61782d0-00ac-4620-9721-263707eb5745`다. 사용 중인 세션은 유지되며 staging 변경이 실행 중인 창에 자동으로 반영되지는 않는다.

목표 재개 선택창은 MiniTerm이 만든 Windows Codex 연결에서 `--no-alt-screen`을 추가해 입력 영역에 표시한다. AI Battery의 compositor와 알림 연결은 유지하며 전역 Codex 설정은 변경하지 않는다. 이미 실행 중인 Codex와 사용자가 직접 만든 별칭·함수에는 소급 적용하지 않는다. 전체 화면 방식이 필요한 실행에는 `MINITERM_CODEX_INLINE=0 codex`를 사용할 수 있다.

`node scripts/verify-goal-hud.cjs <완료된 자체 goal-seed fixture>`는 복사한 시험용 목표와 로컬 HTTP-error 제공자로 실제 Bash→Codex→AI Battery→MiniTerm GUI를 검사한다. 자체 테스트 창에만 입력하며 다크/라이트 캡처, 하단 위치와 닫힌 뒤 잔상 제거를 확인한다. 최신 결과는 `artifacts/goal-hud-f896c2c6-9a6d-470f-9154-b852b34e5eca`다. 실제 계정·모델 응답·IME·진행 중인 사용자 도구 출력은 포함하지 않는다.

상단 버튼은 새 탭·입력/요약 전환·숨기기만 제공한다. 확장은 더블 클릭, 설정은 트레이 메뉴에서 연다. 최신 보고 통로는 `scripts/verify-ux.ps1 -ProgressSample -IdleSeconds 10`으로 검사한다. 한글 데이터를 실제 helper/IPC로 보내 현재 태스크·33→66% 갱신·중단·완료·초기화와 잘못된 완료/이전 보고의 거부를 확인한다. fixture는 `artifacts/ux-d3f76684-664e-4936-9620-c5843ddc223b`다. 앱 유휴 메모리 목표는 이 샘플에서도 초과했으므로 기능 시험 통과를 전체 성능 통과로 간주하지 않는다.

`scripts/verify-progress-adapter.ps1`은 자체 helper 응답 stub에서 이전 helper·이전 서버·미등록 세션을 거부하는지 확인한다. 실제 사용자 앱을 활성화하거나 에이전트 설정을 수정하지 않는다. skill 어댑터의 형식/전달 검사이며 모델 선택 검증과 구분한다.

최신 MiniTerm의 새 셸에서 이 프로젝트를 작업 폴더로 열고 Codex를 새로 시작한 뒤 `$miniterm-progress`를 포함한 작업 요청으로 시험한다. 프로젝트 `.agents/skills`에만 제공했으므로 다른 폴더에서 자동으로 발견된다고 가정하지 않는다. 실제 작업 전환·완료·중단 보고, 새 요청의 기존 비율 제거, Enter 승인 후 새 `begin`, 긴 작업 목록의 호버, 하단 배터리 유지 여부를 확인한다. 화면 분석만 쓸 때는 계획이 없으면 비율이 표시되지 않는다.
