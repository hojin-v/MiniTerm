# 메모리·종료·응답 경로 안정성 보완

2026-10-16. 사용자의 안정성 조사·수정 재개 요청에 따라 구현했다. 같은 작업 폴더에서 진행한 기능 세션의 알림·재개·UI 변경을 보존하고 검증 소스와 실행 파일을 별도 경로에 고정한다. 공개 릴리스나 설치본 교체는 하지 않는다.

## 확인한 위험과 변경

| 경로 | 확인한 위험 | 적용한 처리 |
| --- | --- | --- |
| ConPTY 호스트 | 셸의 Job Object에 속하지 않는 바깥 콘솔은 기존 셸 종료·자원 보호 범위 밖이다. 실제 독립 ConPTY 관찰에서 MiniTerm을 대신한 생성 프로세스의 직접 자식 `conhost.exe`임을 확인했다. | 생성 전후의 자체 자식 목록을 비교하고 단일 새 호스트만 전용 Job에 배정한다. 호스트 커밋 상한 256MiB, 닫힐 때 종료를 적용한다. Bash/에이전트의 Job에는 이 메모리 상한을 적용하지 않는다. 식별·보호 실패 시 해당 세션 시작을 오류로 처리한다. |
| 입력·종료 | 가득 찬 입력 FIFO에 종료 명령을 넣거나 동기 파이프 IO를 기다리면 탭을 닫아도 작업 스레드가 남을 수 있다. | FIFO 밖의 정지 플래그·소유 Job 종료·소유 IO 스레드 취소를 사용한다. GUI에서 join하지 않는다. 백그라운드 reaper만 최대 2초간 취소 경쟁을 처리한다. 정상 셸 종료는 마지막 출력 배출을 기다리고 정체된 연결만 중단한다. 스레드 생성 실패도 세션 오류로 반환한다. |
| VT 자동 응답 | VT 잠금을 잡은 콜백에서 writer 대기를 하면 writer의 resize와 교착할 수 있다. 응답을 단순히 버리면 TUI가 응답을 기다리며 멈춘다. | 콜백은 최대 64KiB를 모으고 VT 잠금을 푼 후 전송한다. 사용자 입력 1MiB에 응답용 64KiB 여유를 둔다. 응답 token bucket은 초기 256KiB·초당 128KiB이며 폭주 시 해당 세션만 명시적 오류로 중단한다. 사용자 입력에는 이 속도 제한을 적용하지 않는다. |
| 스크롤백 | 최대 줄 수만 제한하면 가로 확대 시 history reflow의 고정 셀 저장 공간이 크게 늘어난다. | 고정 셀 저장 공간을 32MiB 기준으로 계산하고 줄 수 설정과 함께 제한한다. 가로 확대 **전**에 history를 줄인다. 가장 최근 출력은 보존한다. |
| 결합 문자·렌더링 | 기존 VT 엔진은 한 셀의 zero-width 문자 Vec를 무한히 키운다. 한 셀만으로도 힙과 shaping 비용이 증가한다. | Alacritty 0.25.1을 소스/라이선스와 함께 고정하고 `Cell::push_zerowidth`에서 suffix codepoint 64개까지 보존한다. 한도를 넘으면 copy-on-write 전에 무시한다. vendored Rust/manifest를 빌드 지문에도 포함한다. |
| 시스템 글꼴 | 기존 7개 OS face를 전부 읽어 private 힙에 프로세스 수명 동안 보관한다. | 고정 OS 글꼴만 읽기 전용 파일 매핑으로 공유하고 필요한 페이지를 읽는다. 매핑과 deny-write 파일 핸들 7개를 프로세스 수명 동안 유지한다. 사용자 지정 글꼴은 기존 교체 가능한 owned buffer를 유지한다. |
| Codex exec-server | 최대 64MiB JSON 프레임 전체를 버퍼·DOM·재직렬화하면 동시에 여러 복사본이 생긴다. stdout EOF 뒤 살아 있는 executor를 무제한 wait할 수 있다. | 256KiB 이내 메타데이터만 분석한다. 큰 프레임은 원본을 스트리밍하며 다음 프레임 분석을 재개한다. 프로토콜 상한 64MiB는 유지한다. EOF 뒤 자체 executor가 200ms 내 종료하지 않으면 해당 Job을 정리한다. |
| Codex 상태 관찰 | WebSocket 원본 전달 스레드에서 GUI IPC의 연결·응답을 기다려 Codex 출력도 지연시킬 수 있다. | 상태 IPC는 별도 worker로 전달한다. 느린 IPC 동안 대기 상태를 최신 1건으로 병합해 메모리를 제한한다. 원본 WebSocket bytes는 그대로 전달한다. 연결 종료 시 대기 상태를 버리며 공유 daemon은 유지한다. |
| 서버 준비 오류 | native daemon 시작 실패 이유를 버려 연결 실패의 원인을 알기 어렵다. | stderr를 별도 cancellable reader로 배출하고 마지막 4KiB만 보관한다. 시작 실패 시 종료 코드와 제한된 오류 내용을 반환한다. |

콘솔 보호는 커밋 할당의 상한이다. 메모리 제한 오류를 콘솔이 어떻게 처리하는지에 따라 세션 종료나 출력 오류가 생길 수 있으며, 모든 프로그램의 메모리를 제한하는 기능은 아니다. Windows Job의 `ProcessMemoryLimit` 의미는 [Microsoft 문서](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information), 동기 IO 취소의 비동기 완료·경쟁 조건은 [CancelSynchronousIo 문서](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelsynchronousio)를 따른다.

글꼴의 파일 매핑은 [MapViewOfFile](https://learn.microsoft.com/en-us/windows/win32/api/memoryapi/nf-memoryapi-mapviewoffile)의 읽기 전용 view를 사용한다. 파일 전체를 private 메모리에 복사하지 않으며, 실제 접근한 파일 페이지는 working set에 포함될 수 있다. 앱 종료 시 OS가 고정 view/핸들을 회수한다.

32MiB는 history의 **고정 Cell 저장 공간** 기준이다. 현재 화면·reflow 임시 공간·hyperlink·CellExtra·GPU·글꼴·다른 세션까지 포함하는 앱 전체 메모리 상한은 아니다. 64자 결합 문자 상한도 입력 bytes/sec의 제한은 아니다. 긴 단일 grapheme의 초과 문자는 표시에서 생략된다.

## Ghostty 참고 범위

로컬 `../ghostty/`, commit `22fa134a07f5749edd9e92c6bc17d8aaf61843be`를 읽었다. 코드를 복사하거나 터미널 엔진을 교체하지 않았다.

- `src/termio/mailbox.zig`: bounded queue, writer를 깨운 뒤 renderer mutex를 풀고 대기하는 순서.
- `src/termio/Thread.zig`: 명시적 stop/wake·종료 배출·동기 출력 timeout.
- `src/renderer/State.zig`, `src/renderer/Thread.zig`: 짧은 상태 잠금·이벤트 기반 렌더·숨긴 창의 작업량 감소. MiniTerm의 기존 visible snapshot/FairMutex/repaint 병합을 유지했다.
- `src/config/Config.zig`: scrollback byte budget. 해당 revision 기본값은 50,000,000bytes이며 MiniTerm 수치와 혼동하지 않는다.
- `src/terminal/page.zig`: `grapheme_max_len = 64`, 입력이 한도를 넘으면 추가 저장을 중단한다. MiniTerm도 셀 내부에서 같은 상한을 적용했다.

## 고정 소스와 검증

근거 폴더: `artifacts/stability-20261015T121043Z-e6735e27-3674-4474-b224-20a6f9ef3c02/`. 경로의 날짜는 최초 조사 시작 시각이며 이번 재개·검증은 10월 16일이다. 최종 `source-final/`에 소스·tests·assets·vendor·scripts를 보존했다. 소스 지문은 `1b2bb21bbaf077158077d8bd1e2372c334978adce4f1da1e2fc9d1396384c610`다. `source/`와 `miniterm-before-font.exe`는 글꼴 변경 전 비교 사본이다.

- 일반 검사 **96개 통과**: 단위 95개·SSE fixture 1개. 환경 의존 ignored 검사는 포함하지 않는다. `tests-final.{json,out.txt,err.txt,resources.csv}`.
- 별도 실제 ConPTY 검사 **1개 통과**: 콘솔 Job의 256MiB 한도·Bash Job의 무제한·Bash가 콘솔 Job 밖에 있음·1,000행 후 마지막 마커·출력 중 닫기·콘솔 종료·작업 스레드 상태 해제·닫힌 입력 거부를 확인했다. `native-final.{json,out.txt,err.txt,resources.csv}`. 이 검사는 외부 모델·GUI 입력 없이 격리된 `MINITERM_DATA_DIR`와 설치 Git을 사용한다.
- 검사 도중 프로세스는 별도 Windows Job·시간 상한으로 보호하고 자체 PID 트리만 표본 수집했다. 낮은 시스템 가용 커밋/물리 메모리를 감지하면 자체 시험만 중단하도록 했다. 측정용 Python wrapper는 앱의 런타임 의존성이 아니다.

최종 release SHA256은 `d0e6d9f532a90ab1ca34cbf7b0928e78b32e6cc09e6ea4f10266b75f71a48c67`이다. 같은 폴더의 `miniterm.exe`, `BINARY_BUILD_INFO.json`, `release-final.{json,out.txt,err.txt,resources.csv}`를 함께 보존한다. release 빌드는 단일 작업으로 통과했다.

검증 후 현재 작업 소스와 사본의 지문 일치를 확인하고 **`artifacts/miniterm.exe`와 빌드 정보도 이 파일로 갱신했다.** 기존 `9b868ff2…` 실행 파일과 빌드 정보는 근거 폴더의 `miniterm-previous-artifact.exe`, `BINARY_BUILD_INFO-previous-artifact.json`으로 보존했다. 기능 세션의 `artifacts/feature-notify-final-20261015/` 사본은 유지한다. `VERIFICATION.json`에 교체 전후 해시·소스·검증 경로를 기록했다.

| 최종 release 검사 | 결과와 범위 | 원시 근거 |
| --- | --- | --- |
| 화면·1세션 30초 유휴 | 테마·한글/ANSI·확장/축소·드래그/크기 저장·자체 Bash 종료 통과. 30.46초 최대 working set **150,097,920bytes**, 평균 private 약 129.69MiB. CPU 증가량은 측정 해상도상 0. 200,000,000bytes 목표 통과. | `artifacts/ux-2afd96de-43fe-4492-95a4-e2ad4cb1c947/{verification.json,idle.json,metrics.json}`, `ux-final.resources.csv` |
| 4세션 30초 유휴 | 네 개의 실제 Bash 세션·GUI 동작·자체 셸 종료 통과. 30.47초 최대 working set **152,948,736bytes**, 평균 private 약 139.28MiB. CPU 증가량은 측정 해상도상 0. 다크/라이트의 한글·선택·bars·braille·박스 문자 캡처도 직접 확인했다. | `artifacts/ux-d277760e-18c1-4091-ba91-45b4140c84b8/{verification.json,idle.json,metrics.json}`, `ux-four-final.resources.csv` |
| 18만 행 출력 | 자체 HOME의 Bash startup에서 60,000행을 3회 출력했다. 매회 history **17,476행**으로 유지되고 정상 종료했다. 전체 구간 앱 최대 working set **176,799,744bytes**, private **165,769,216bytes**. Bash·콘솔·GPU는 별도다. | 근거 폴더의 `output-fixture-57f8c220-d399-4b0d-bd2e-65458e3f28bf/verification.json`, `output-final.{json,resources.csv}` |
| 공유 Codex native 회귀 | 설치된 **0.163.0**에서 첫 탭만 완료·다른 탭으로 같은 대화 재개·이력·호출 폴더·Bash 도구·writer 충돌 중 R·해제 후 R·원래 콜백 4회·GUI 종료 뒤 daemon 유지와 listener 정리를 통과했다. 실제 계정·모델·데스크톱 입력은 사용하지 않았다. | `shared-codex-runtime.{json,out.txt,resources.csv}`, `artifacts/shared-bash-5e57b0b6-cdda-4b90-bc86-bad3adf1cebc/verification.json` |
| 설치 Git 호환성 | **5개 통과 / 기존 resize 4개 실패**. raw/managed prompt 및 production Session 가로·세로 resize 뒤 입력이 누락됐다. | `pty-compatibility.{json,out.txt,err.txt,resources.csv}` |

글꼴 변경 전 같은 `-GlyphSample -SessionCount 1 -IdleSeconds 30` 조건의 비교 사본은 최대 working set **206,475,264bytes**로 목표를 초과했다. 최종은 약 **56.4MB(27.3%)** 감소했다. 각 한 번의 국소 비교이며 모든 장치·작업 부하에 같은 감소율을 보장하지 않는다. 변경 전 근거는 `artifacts/ux-c20192dc-b748-4b5a-abe3-4b67e675c95a/idle.json`과 `ux.resources.csv`다. 원래 dark glyph 캡처는 비동기 샘플 도착 전 화면이어서 해당 샘플의 시각 검증으로 세지 않았다. light 캡처는 한글·선택 배경·bars·braille·박스 문자를 확인했다. GUI fixture의 캡처 전 대기도 보완했다.

초기 공유 Codex 검사는 시험용 Job의 kill-on-close 제약 때문에 0.163.0 launcher가 백그라운드 시작을 거부했다. 첫 스크립트는 0.162.1 daemon package를 고정 복사하던 문제도 있어 설치 native 버전을 읽어 같은 package를 준비하도록 수정했다. 시험 전용 launcher에 breakaway를 허용한 뒤에는 정상 통과했다. 앱/프런트엔드는 Job으로 제한하고, escaped daemon/updater는 **이번 실행의 새 ownership marker·자체 home의 이미지 경로·프로세스 생성 시각**을 검증해 500ms 간격으로 메모리를 감시했다. 이 별도 daemon 감시는 OS의 hard allocation limit과 구분한다. 각 시험의 자체 daemon/updater 정리와 원래 사용자 프로세스 보존을 확인한다. 실패·계측기 job ancestry 오류 기록도 삭제하지 않는다.

## 적용 범위와 남은 확인

2026-10-13 OpenConsole 45.3GB 사고의 정확한 할당 스택·입출력 트리거는 확보하지 못했다. 이번 변경은 확인한 무제한 저장·동기 대기와 콘솔 보호 공백을 보완한 것이며 그 사고를 정확히 재현하거나 원인을 모두 제거했다는 뜻은 아니다. 이전 DLL 검색 경로 차단도 유지한다.

설치된 Git/MSYS의 resize 직후 첫 글자 누락 4건은 별도 호환성 결함이다. 과거 개발용 수정 DLL의 통과와 현재 설치 Git의 결과를 구분하며, 수정 DLL을 이 실행 파일 옆에 자동 배포하지 않는다. [기존 검증](../docs/patches/MSYS_RUNTIME_VALIDATION.md)을 따른다. 실제 계정의 장시간 대화·IME·다른 GPU/Windows·전력 소모와 입력→픽셀 p95는 이번 단위/ConPTY 검사의 통과 범위가 아니다. 다음 호환성 작업은 지원할 최신 Git/MSYS 소스에 기존 신호/입력 패치를 이식하고 공식 런타임과 같은 즉시 resize 검사를 비교하는 것이다.
