# 배터리 하단 표시·창 테두리·탭 수명 주기 후속

2026-10-18. 사용자가 설치본에서 보고한 다섯 문제와 추가 설정창 글자 깨짐을 함께 처리한다. 기준 HEAD는 `0e542e9950631adafb1e0399596e5c15fdf0f024`다. 사용자 설치 파일은 SHA256 `63400890d27a4c28a5750cb43363d0f6018286a782385871ecb01efde3db0c57`인 기존 alpha.7이다. 설치본·계정·전역 설정·WSL 배포판은 교체하거나 종료하지 않았다.

## 변경

| 사용자 요청 | 반영한 동작 |
| --- | --- |
| 1. Windows Codex/Claude의 AI Battery가 입력 위에 뜨거나 이후 사라짐 | 프로그램이 실제 출력한 배터리 행의 문자·색상을 보존해 본창 하단의 별도 행에 표시한다. 입력 영역과 분리하고 원래 행의 중복 표시를 줄인다. 입력/응답이나 화면 지우기로 배터리 행이 없어져도 마지막 실제 출력은 유지한다. |
| 2. WSL 축소 상태·리사이즈 뒤 배터리 누락 | 입력·요약·확장 모드에서 같은 하단 행을 사용한다. 커서 아래의 배터리도 수집하고 크기 변경 뒤 새 입력 없이 표시한다. |
| 3. 가장자리 크기 조절 커서가 간헐적 | DPI에 맞춘 경계 폭을 4→7 DIP로 넓히고 `WM_SETCURSOR`에서 hit-test 방향에 맞는 시스템 커서를 명시한다. |
| 4. 마지막 탭에서 exit 후 추가 입력 필요 | 종료 통지·출력·리사이즈가 본창 ROOT viewport를 다시 그리도록 지정한다. 마지막 셸이 종료되면 탭을 제거하고 창을 숨기며 트레이는 유지한다. |
| 5. 다시 실행했을 때 빈 창 | 살아 있는 앱을 다시 열 때 세션과 시작 중인 세션이 모두 없으면 새 탭을 하나 만든다. 트레이·단축키에도 같은 조건을 적용한다. |
| 추가. 설정창이 열려 있을 때 본창 글자 깨짐 | 기존 alpha.7에 빠진 다중 viewport의 글꼴 atlas 수정이 현재 소스에 포함돼 있다. 본창만 텍스처 제한을 바꾸지 않고 renderer의 동일한 제한을 공유한다. 새 최종 빌드의 픽셀 회귀는 아래 보안 차단으로 미완료다. |

배터리는 새 사용량 API를 호출하거나 추정한 수치가 아니다. `src/battery.rs`와 PTY 파서가 최대 512칸의 실제 행을 수집한다. 같은 pipe read 안에서 cursor restore/erase가 이어져도 출력 직후 수집하고, 동기화된 프레임은 commit/timeout 뒤 수집한다. Bash의 실제 다음 프롬프트에 `OSC 133;A`를 표시해 프로그램이 끝나면 배터리도 지운다. 기존 종료 코드·프롬프트 지침·콜백·셸 초기화 파일은 유지한다. Windows의 inline 실행 경로와 출력 기록도 유지한다.

커서 메시지의 hit-test 값과 처리는 [Microsoft WM_SETCURSOR 문서](https://learn.microsoft.com/en-us/windows/win32/menurc/wm-setcursor)를 따른다. 상태 표시를 다시 그리기 위해 키를 보내거나 주기적으로 입력을 재전송하지 않는다.

## 통과한 검사

- 중간 소스 일반 검사 **101개(단위 100·SSE 1)** 통과. 동기화 프레임의 배터리 수집 누락을 새 검사로 재현한 뒤 보완했다. 1바이트 분할·동일 read의 지우기·색상·resize 보존·실제 프롬프트의 제거도 검사했다. 환경 의존 ignored 검사는 이 수에 포함하지 않는다.
- 일반 검사 이후 UI helper의 인자 묶음·배경/글자 그리기 순서를 정리했고 최종 소스의 all-target Clippy `-D warnings`와 release 빌드를 통과했다. 일반 검사 실행의 시점과 구분한다.
- Windows GUI: `artifacts/footer-lifecycle-0b456c04-9a55-4b14-8ccd-a0d8496edae5/verification.json`, `passed=true`.
- WSL Ubuntu-24.04 GUI: `artifacts/footer-lifecycle-7c361ca3-3b30-42d6-9fe7-221a83b33106/verification.json`, `passed=true`.
- 두 GUI 검사는 중간 debug SHA256 `ced698e8fd86dbada60d56e2d7f22c908c3bd3101311c4864e95578796a2a5b7`에서 실제 PTY·Bash와 고정 VT 배터리를 사용했다. 위/아래 배치, 입력/요약/확장, 크기 변경 뒤 입력 없는 유지, 실제 프롬프트 복귀 후 제거, 8방향 경계 메시지 반복, 두 탭/마지막 탭의 exit, 다시 실행한 첫 탭 생성을 확인했다. 캡처도 확인했다. 실제 계정·모델·물리 커서/키보드 검증으로 확대하지 않는다.

검증 도구 `scripts/verify-footer-lifecycle.ps1`은 자신의 EXE 사본·HOME·설정·IPC 인스턴스만 사용한다. WSL은 해당 셸 자식만 닫고 배포판은 유지한다. 앞선 허용 목록 누락과 PowerShell의 UTF-8 BOM 때문에 실패한 fixture도 보존했다. 둘은 최종 통과로 세지 않는다.

## 최종 실행 파일의 보안 차단 — 미완료

최종 release의 기록은 `artifacts/feature-footer-20261018/BINARY_BUILD_INFO.json`이다.

- 소스 지문: `a4ebb5a2cd24a70653a1907c1e32694dc0c1ca59510372c8ee5374caf52c940d`.
- 실행 파일 SHA256: `6002be03d60e7ef932abfabfd1be523ca5e428d12d40c5605889d413e1501ac3`.
- 빌드 UTC: `2026-10-18T05:01:28.5948828Z`.
- Windows Defender가 14:02:55 KST 이벤트 1116으로 `Trojan:Win32/Bearfoos.A!ml` 탐지를 기록했고, 14:03:15 이벤트 1117로 `artifacts/feature-footer-20261018/miniterm.exe`를 격리했다. 탐지 ID는 `{02897F15-95FC-4F10-AC72-7B01A49CDF70}`다.
- `artifacts/settings-glyphs-571affa6-3430-4b6a-8336-4f36dffeb08b/verification.json`은 Windows의 실행 거부로 실패했다. 새 release의 설정창 픽셀 검사·Windows Codex/Claude 통합을 통과로 표시하지 않는다.

현재 근거만으로 오탐을 확정하지 않는다. 보호 기능 변경·예외 등록·다른 이름/빌드로 실행 차단 우회·사용자 설치본 교체는 하지 않았다. 소스 사본과 검사 기록은 남기고, 사용자의 보안 검토 결과를 기다린다. 격리된 실행 파일을 정상 실행 가능한 산출물로 안내하지 않는다.

`artifacts/feature-footer-20261018/source/`에는 빌드 입력인 `src`·`vendor`·리소스·Cargo 파일과 검사 스크립트를 보존했다. 원본과 사본의 소스 지문이 위 값으로 일치하는 것을 확인했다. 통과 범위·최종 실행 거부·사본 확인 결과는 `artifacts/feature-footer-20261018/VERIFICATION.json`, 해당 경로의 Defender 이벤트는 `defender-events.json`에 남겼다. 설치 파일의 SHA256은 여전히 위 기존 alpha.7 값이다.

기존 글꼴 수정과 alpha.7의 재현/6회 픽셀 비교 기록은 [10월 16일 글자 수정](CODEX_SETTINGS_FIX_2026-10-16.md)을 따른다. 과거 통과를 위 새 파일의 통과로 대체하지 않는다.

## 보안 검토 뒤 다음 검사

복원된 파일이 위 SHA256과 일치하는지 확인한 뒤 같은 파일로 실행한다. 재빌드하거나 다른 파일로 바꾸어 결과를 섞지 않는다.

```powershell
./scripts/verify-footer-lifecycle.ps1 -Executable artifacts/feature-footer-20261018/miniterm.exe
./scripts/verify-footer-lifecycle.ps1 -Executable artifacts/feature-footer-20261018/miniterm.exe -WslDistribution Ubuntu-24.04
./scripts/verify-settings-glyphs.ps1 -Executable artifacts/feature-footer-20261018/miniterm.exe
$env:MINITERM_SCROLL_FOOTER='1'
node scripts/verify-scroll.cjs default artifacts/feature-footer-20261018/miniterm.exe shared full
```

마지막 검사는 실제 Codex·AI Battery와 자체 localhost 제공자로 초기/입력 뒤/축소/확장 뒤의 하단 배터리, 초안과 스크롤을 확인하도록 보완했다. 아직 실행하지 못했다. Claude Code의 실제 계정 응답 및 물리 장치 확인도 남아 있다. 성능 측정·공개 릴리스·설치본 갱신은 수행하지 않았다.
