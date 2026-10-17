# Codex 연결·설정창 글자·확장 후 스크롤 수정

기준 소스는 `24f01ce67381f5b795cc2e0cd3f50d26bcddf4d8`이다. 사용자가 설치한 `%LOCALAPPDATA%/Programs/MiniTerm/miniterm.exe`의 SHA256은 공개 alpha.7 자산과 같은 `63400890d27a4c28a5750cb43363d0f6018286a782385871ecb01efde3db0c57`이었다. 사용자 앱·Codex 대화·인증·설정은 변경하거나 종료하지 않았다.

## 확인한 원인과 수정

### Codex의 추가 연결이 기존 연결 뒤에서 대기

`codex_link::start`가 `accept`한 연결의 `relay`를 같은 스레드에서 끝까지 실행했다. 기존 TUI 연결을 유지한 채 세션 선택기가 추가 WebSocket 연결을 만들면 TCP 연결만 성립하고 HTTP Upgrade 응답이 전달되지 않았다.

공개 alpha.7에서 다음을 재현했다.

- 첫 대화와 Bash 실행은 완료됐지만, 기존 TUI가 열린 동안 두 번째 인증 연결은 5초 이내 handshake에 응답하지 않았다.
- 별도 재현에서 `/new` 뒤 일반 문장 입력은 성공했으나 `/resume`은 사용자와 같은 `Failed to start TUI session picker: failed to connect to remote app server`를 출력했다.

리스너와 각 연결의 중계를 분리했다. 연결별 소켓·proxy Job을 소유하고 링크당 동시 연결은 최대 4개로 제한한다. 한도를 넘는 연결만 닫으며, 탭 종료·detach·앱 종료 시 해당 링크의 모든 소켓과 proxy Job을 정리한다. 연결이 살아 있는 동안 리스너를 유지하고 마지막 연결이 닫힌 뒤 30초 동안 재접속을 허용한다. 스레드 생성 실패도 연결 슬롯과 자식 프로세스를 정리한다. 공유 daemon과 다른 대화의 writer lease는 그대로 보존한다.

공식 [App Server 문서](https://learn.chatgpt.com/docs/app-server)의 원격 TUI/WebSocket 연결을 사용한다. 연결을 동시에 여는 동작과 이번 직렬화 결함은 설치된 Codex 0.163.0으로 직접 확인한 결과다.

### 설정창이 본창의 글꼴 atlas를 프레임 중간에 재생성

본창의 `raw_input_hook`만 텍스처 한 변을 4096으로 제한하고 있었다. eframe 0.33.3의 immediate 설정창은 이 hook을 거치지 않아 renderer의 8192 제한을 사용했다. egui는 두 창이 공유하는 글꼴 atlas를 다시 만들었고, 먼저 생성한 본창의 글자 mesh는 이전 UV 좌표를 계속 사용했다. 공개 alpha.7의 한글·영문이 글자 조각으로 깨지는 화면을 확인했다.

본창에만 적용하던 제한을 제거해 모든 viewport가 renderer의 같은 값을 사용하도록 했다. GPU 장치 제한을 낮추지 않아 큰 화면의 surface 지원도 유지한다. 기존 atlas 변경 감지와 터미널 행 캐시는 유지한다. 고정 OS 글꼴의 읽기 전용 매핑, history/결합 문자/파이프 상한도 유지한다.

## 회귀 검증

### 실사용 검사에서 추가 발견한 정지한 커서의 휠 누락

실제 MiniTerm의 Git Bash에서 Codex 0.163.0을 실행하고 localhost 응답을 15초에 걸쳐 180행으로 나눠 보냈다. 응답 중 초안을 쓰고 위로 스크롤한 뒤 축소/확장하면, 같은 위치에 둔 마우스의 다음 휠은 도착해도 `lastWheelHovered=false`가 되어 무시됐다. 기록 오프셋 100에 머물러 새 출력이 멈춘 것처럼 보일 수 있었다. `artifacts/scroll-ab6c7139-e604-4611-a2a9-f2acbbb44393/scroll-snapshots.json`에 휠 이벤트 2→4 증가와 고정 오프셋을 보존했다.

winit의 Windows 경로는 `WM_MOUSELEAVE` 후 같은 좌표의 `WM_MOUSEMOVE`를 중복으로 생략하고, egui-winit은 `CursorLeft`에서 `PointerGone`을 유지한다. 휠 이벤트에는 좌표가 없다. 본창 `raw_input_hook`에서 새 포인터 위치가 없는 휠 프레임에 한해 실제 커서의 client 좌표를 확인하고 DPI를 적용한다. 최상단 창이 본창인 경우만 갱신하므로 설정창·다른 창 아래에 있는 본창으로 휠이 새지 않는다. 유휴 polling이나 합성 키·입력 재전송은 추가하지 않는다. 이 hook은 텍스처 제한을 변경하지 않는다.

검사도 현행 Codex 버전과 Windows의 정규화된 신뢰 경로에 맞췄다. 스트리밍 중에는 화면이 정지할 때까지 기다리지 않고 실제 휠 이벤트 수신을 확인한다. 준비 화면/시험 대기 조건 때문에 실패한 기록은 제품 회귀로 합산하지 않으며 원본을 보존한다.

- 최종 소스의 일반 검사 97개(단위 96개 + SSE 1개), all-target Clippy `-D warnings`, format·release 빌드 통과. GUI/설치 환경이 필요한 21개 ignored 검사는 이 숫자에 포함하지 않는다.
- 소켓 단위 검사: 부분 handshake 하나가 다른 연결을 막지 않음, 잘못된 인증 거부, 동시 연결 4개 상한, 초과 연결 거부, 탭 종료 시 모든 대기 소켓 종료.
- `scripts/verify-shared-notifications.cjs`: 실제 Codex 0.163.0 + 별도 CODEX_HOME + localhost SSE fixture. 동시 인증 연결, `/new` 이후 입력·Bash, `/resume` 선택기 표시/닫기, 다른 탭으로 대화 재개·이력·cwd·원래 콜백, writer 충돌 중 R/소유자 종료 후 복구, GUI 종료 뒤 daemon 유지와 중계 리스너 종료를 확인한다. 실제 계정/모델을 호출하지 않는다.
- `scripts/verify-settings-glyphs.ps1`: 별도 앱에서 한글·영문·스타일 표본을 그린 뒤 두 창을 나란히 배치한다. 설정창을 열기 전후의 터미널 표본을 다크·라이트 각각 3회 PNG SHA256으로 비교한다. 공개 alpha.7은 6회 모두 불일치, 첫 수정본은 6회 모두 일치했다. 캡처 이미지도 직접 확인했다. 키보드·마우스 입력 합성은 하지 않는다. `-IdleSeconds 30`은 설정창을 연 상태의 자원 사용도 기록한다.

초기 재현/검증 경로:

| 검사 | 증거 |
| --- | --- |
| alpha.7 추가 연결 timeout | `artifacts/shared-bash-4c4455c8-c389-4b1f-9e1a-b72aad389106/verification.json` |
| alpha.7 실제 `/resume` 오류 | `artifacts/shared-bash-604bd3db-a369-4eb9-b7f1-d46e21b1f816/linked-screen.txt` |
| alpha.7 글자 깨짐 | `artifacts/settings-glyphs-44322c85-c937-49a8-b5d0-2a9f72c874dd/` |
| 첫 수정본 Codex 통과 | `artifacts/shared-bash-0ab29f98-7cbc-41e8-b17a-1e3857990326/verification.json` |
| 첫 수정본 글자 6회 일치 | `artifacts/settings-glyphs-9281c45b-a8aa-4e33-81bc-31def5ce3c60/verification.json` |

첫 수정본의 SHA256은 `123d85e2fbb73051a00f1a23af5519d4ec976ba12b91f645efb5330371fd8a1e`다. 이후 출력 스레드 생성 실패 처리도 반영한 최종 실행 파일과 검증 경로는 `artifacts/current-codex-settings-fix.txt`가 가리키는 폴더의 `BINARY_BUILD_INFO.json`과 `VERIFICATION.json`을 따른다. 이전 성능 기록의 실행 파일과 구분한다.

### 최종 실행 파일과 실제 GUI 검증

- 실행 파일 SHA256: `4f58251eabc41f0f48136b8c3f895c63024afbb89f506e8a3e8f0b2c2ae5ac58`
- sourceFingerprint: `995e54964309360bfc90160d8262f9fec1daca82f9e8e56e4fa180f197214681`
- 로컬 수정본이며 공개 alpha.7 및 사용자 설치본과 다르다. 바이너리와 소스 fingerprint를 대조한 뒤 공용 `artifacts/miniterm.exe`도 이 파일로 갱신했다. 이전 공용 EXE와 빌드 정보는 수정본 폴더에 보존했다.

| 검사 | 결과와 증거 |
| --- | --- |
| 실제 MiniTerm + 직접 Codex 0.163.0 | `artifacts/scroll-bdbfca2c-9d5f-41d9-a1d3-a55567c1db04/verification.json`: 첫 입력의 OSC 오염 없음, Bash 실행·호출 환경, 15초 180행 응답 중 초안 작성·휠·축소/확장, 확장/축소 화면 스크롤, 초안 원문 전송, 끝나지 않는 응답의 Esc 취소·새 요청/응답, 앱 종료 후 재개/이력 보존, writer 보호·소유자 해제 후 R 복구와 최종 공유 재개 통과. 최대 앱 working set 154,853,376bytes. |
| 기본 AI Battery 브리지 | `artifacts/scroll-fba56c36-1881-4f02-9ed2-e98e0208dff1/verification.json`: 동일 스트리밍·초안·휠·취소 후 재요청 통과. 배터리 데이터는 고정 시험 출력이며 실제 계정 조회가 아니다. 이 실행의 최대 앱 working set은 **209,707,008bytes**였다. |
| MiniTerm 알림/관찰 연결 없이 실행 | `artifacts/scroll-0fb51faa-d2b8-4028-9b80-4b3c6f39acc1/verification.json`: native 기본 공유 서버 연결, 첫/다음 요청·응답·확장/축소 휠·초안 통과. 최대 앱 working set 152,993,792bytes. |
| 최종 설정창 글자/유휴 | `artifacts/settings-glyphs-476e7144-c4ec-4040-8642-917320d0b454/verification.json`: 다크/라이트 각각 3회 픽셀 일치. 설정창을 연 30초 유휴 최대 working set 149,983,232bytes, 측정 CPU 증가 0(계측 해상도 한계). |
| 최종 4탭 GUI/유휴 | `artifacts/ux-a0ebe5a1-886a-47d1-8d02-f7647a51286c/verification.json`: 본문 더블 클릭, 수직 확장/축소, ANSI 보존, 이동/크기 저장, 즉시 다크/라이트와 4개 셸 종료 통과. 30초 유휴 최대 working set 153,276,416bytes, CPU 증가 0(계측 해상도 한계). |
| 전체 설정 UI | `artifacts/settings-cf081eb9-d9e0-435c-b17d-a039152b3dd9/verification.json`: 즉시 반영·저장/재시작·알림/콜백·Bash 등록/해제·투명도·단축키 유효성·prompt와 셸 UUID 유지. 이전 `02c549…` 수정본의 결과다. 이후 제품 변경은 휠 포인터 갱신뿐이다. |
| 공유 연결·선택기·알림 수명 | `artifacts/shared-bash-5301e63e-26de-4d09-bc05-656d76823f20/verification.json`: 동시 연결, `/new`, 실제 `/resume` 선택기, 다른 탭 재개, 콜백 중복 없음, daemon 유지/리스너 종료 통과. 이전 `02c549…` 수정본이며 이후 `codex_link.rs`는 변경하지 않았다. |

모든 Codex 검사는 설치된 native CLI와 별도 CODEX_HOME/대화/localhost 제공자를 사용했다. 실제 모델/계정 요청은 하지 않았다. 실제 GUI 입력은 자기 창에만 Win32 키 입력을 보내고 포커스를 잃으면 즉시 취소한다. 휠은 자기 창의 `WM_MOUSEWHEEL`을 winit/egui에 통과시킨 결과이며 물리 휠/IME/혼합 DPI 전체 검증은 아니다. 캡처에서 초안 원문, 취소 표시와 다음 응답도 직접 확인했다.

기본 브리지의 약 210MB 관측은 숨기지 않는다. 추가 30초 자원 검사는 첫 요청 전에 다른 창으로 포커스가 이동해 중단됐으므로 유휴 결과로 사용할 수 없다(`artifacts/scroll-e741f4d6-79fa-4271-a027-28137b030019/`). 설정창 유휴 150MB 통과를 모든 Codex 활성 시나리오가 200MB 이내라는 주장으로 확대하지 않는다. 자원 보호로 중단된 검사나 프로세스 메모리 폭증은 이번 유효 검사에서 없었다.

## 확인 범위와 사용

`This conversation is read-only or unavailable; no operation was sent.`의 정확한 사용자 실행 순서는 아직 확인 중이다. Codex 0.163.0의 [요청 제출 코드](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/tui/src/app/thread_routing.rs)와 [연결 상태 검사](https://github.com/openai/codex/blob/rust-v0.163.0/codex-rs/tui/src/app/reconnect.rs)에서 이 문구는 원격 대화의 attachment가 Live가 아닐 때 요청을 보내지 않는 경로에 있다. 사용량 25% 안내나 파일 sandbox의 read-only 설정과 같은 의미로 해석하지 않는다. 이번 fixture의 새 대화와 공유 대화 재개는 정상 동작하지만, 해당 문구 자체가 같은 원인이라고 단정하지 않는다. 다른 앱이 소유한 대화의 writer 보호를 우회하지 않는다.

사용자 환경에서 `codex doctor --json`을 읽기 진단했다. 서버 0.163.0 실행·설정 읽기·인증 구성은 정상이며, 대화 기록 파일/DB가 각각 157개로 일치했다(누락/중복/손상된 이름 0). 이것은 실제 계정의 모델 응답 성공이나 문제 당시 대화의 attachment 복구 증거는 아니다. 사용자 대화·DB·서버는 수정하거나 재시작하지 않았다.

수정본은 로컬 검증용이다. 공개 alpha.7 자산을 교체하지 않는다. 설치본이 실행 중이므로 사용자 프로세스를 강제 종료하거나 설치 폴더의 EXE를 교체하지 않는다. 새 설치 파일을 사용할 때는 기존 MiniTerm과 그 안의 작업을 정상적으로 종료한 뒤 설치한다. 앱 설정은 유지된다.

로컬 설치 파일은 `artifacts/current-codex-settings-fix.txt`가 가리키는 폴더의 `MiniTerm-0.1.0-alpha.7-fix1-windows-x64-setup.exe`다. 기존 alpha.7과 같은 NSIS 템플릿으로 컴파일했고 구성 파일의 SHA256을 대조했다. 이번 설치 파일을 사용자 설치본 위에 실제 설치/제거하는 검사는 하지 않았다. 최종 설치 파일 해시는 같은 폴더의 `PACKAGE_MANIFEST.json`·`SHA256SUMS`·`VERIFICATION.json`을 따른다.

실사용 추가 검사에서 설치된 Git 2.55.0.windows.5의 production Session/ConPTY 9개 중 5개 통과·4개 실패를 재확인했다(`artifacts/COMPATIBILITY.json`, 2026-10-16T09:05:33Z). 실패는 [기존 MSYS resize 입력 경쟁](../docs/RESIZE_INPUT_INVESTIGATION.md) 4건으로, 창 크기 변경 직후 입력 손실이 남는다. 허용 실패 옵션으로 harness가 종료 코드 0을 반환한 것을 전체 통과로 표시하지 않는다. 이 문제 때문에 무조건적인 실사용 무결점 판정은 하지 않는다. 수정 후보 MSYS DLL은 전체 호환성·배포/복구 검증이 끝나지 않아 사용자 Git이나 설치 패키지에 넣지 않았다.
