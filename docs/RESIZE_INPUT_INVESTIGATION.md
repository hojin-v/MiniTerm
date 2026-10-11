# 크기 변경 후 첫 글자 누락 조사

2026-10-11. 격리된 수정 DLL에서 입력·신호 검사가 통과했지만 기본 제품 프로필에는 아직 적용하지 않았다. 입력에 지연·보충 문자·명령 재전송을 넣어 숨기지 않는다. 실행 중인 사용자 콘솔에는 연결하거나 입력을 보내지 않고 별도 ConPTY만 사용했다.

| 비교 경로 | 결과 | 원시 기록 |
| --- | --- | --- |
| Windows 콘솔 API만 사용하는 C# 프로그램 | 크기 변경 직후 1,000개 줄이 정확하게 도착. `PeekConsoleInputW`와 `ReadConsoleInputW`의 비교에서 불일치 0개 | `artifacts/experiments/console-input-dba885d7-5442-4735-8d01-ab098aaa5363/verification.json`, `input.log` |
| Bash 편집기를 사용하지 않는 MSYS Perl `sysread` | POSIX termios에서 echo/canonical 변환을 끄고 EINTR을 재시도해도 19번째 줄의 `printf`가 `rintf`로 도착 | `artifacts/experiments/msys-read-f6f8f604-71b4-4519-a8fa-22cf9f63cb67/verification.json` |
| 정상 가상키/스캔 코드를 쓰는 실제 Bash | 7번째 명령의 첫 글자 누락 | `artifacts/experiments/resize-input-physical-vk.json` |
| 기존 production Session 회귀 | 앞선 시험에서 가로+세로, 세로만 resize와 managed prompt 경로 실패. 이번 색상 수정에서 입력 코드는 바꾸지 않았고 해당 시험을 재실행하지 않음 | `artifacts/CURRENT_BUILD_STATUS.json` |

콘솔 API 비교는 520(`ENABLE_WINDOW_INPUT | ENABLE_VIRTUAL_TERMINAL_INPUT`) 모드에서 수행한다. 재현 프로그램은 `artifacts/experiments/console-input-probe.cs`, `.cjs`와 `msys-read-probe.pl`, `.cjs`에 있다. 이는 실제 MSYS 스레드·시그널 처리를 모두 구현한 비교가 아니며, Windows 모든 콘솔 입력이 정상이라는 증거로 확대하지 않는다. 콘솔 API 프로그램은 1,000개 줄 및 종료 입력까지 기록했지만 harness의 종료 대기 안에 exit 이벤트가 오지 않아 테스트 소유 PTY를 종료했다.

설치된 Git의 MSYS 소스(`7674c51`)에서 `process_input_message()`는 콘솔 이벤트를 먼저 peek하고, `WINDOW_BUFFER_SIZE_EVENT` 처리에서 `send_winch_maybe()`를 호출한 뒤 원래 수만큼 이벤트를 버린다. `send_winch_maybe()`는 시그널 핸들러의 longjmp를 고려해 입력 mutex를 잠시 해제하고 재획득한다. 같은 입력 mutex를 사용하는 console master thread도 resize 이벤트를 제거하고 나머지 키를 writeback한다. [Git for Windows의 해당 소스](https://github.com/git-for-windows/msys2-runtime/blob/7674c51/winsup/cygwin/fhandler/console.cc)

이 잠금 해제 구간에서 master thread가 resize 이벤트를 먼저 제거하면, 앞선 peek 결과에 따른 discard가 첫 키를 버릴 수 있다는 가설을 세웠다. 현재 최신 main 소스에도 해당 잠금 해제 구간이 있다. 아래 추가 추적에서 **실제 입력 큐의 경쟁 순서를 관찰했다.** DLL 내부의 모든 mutex/시그널 호출을 추적한 것은 아니며 내부 설명은 기록된 소스와 API 호출 순서를 함께 해석한 것이다. 앱의 입력 FIFO나 렌더러 변경으로 해결되었다고 표시하지 않는다.

## 테스트 소유 MSYS의 실제 API 추적

`artifacts/experiments/console-trace.cpp`는 실행 중인 테스트 소유 DLL의 import table에서 Peek/Read/Write/FlushConsoleInput 호출을 기록한다. `console-trace-loader.cpp`는 Git Bash/Perl 경로만 허용하고, DLL은 자신의 고유 fixture 폴더에 복사된 `probe.pl`의 정확한 경로가 프로세스 command line에 있는지 추가 검증한다. 기존 사용자 프로세스에 연결하지 않으며 설치된 DLL을 수정하지 않는다. 콘솔 입력은 고정 시험 문자열뿐이고 모든 helper는 별도 fixture에서 종료한다. 이 계측은 타이밍에 영향을 주므로 빈도나 성능을 측정하는 도구가 아니다.

Perl만 추적한 첫 유효 기록 `console-trace-a5532407-ca00-40c0-b966-8311e3abc506`은 2개 줄 이후 첫 글자 누락을 재현했다. 크기 이벤트를 peek했지만 다음 one-record read는 키 `p`를 받았다. 별도 프로세스의 제거가 의심되어 두 번째 기록에서는 부모 Bash도 추적했다.

`artifacts/experiments/console-trace-107177b0-2589-42f1-8dba-d00367d701a7`은 21개 줄을 정상 수신한 다음, **0부터 세는 요청 21**의 `printf`를 `rintf`로 수신했다. 공유 QPC 시간으로 두 프로세스의 기록을 합친 `analysis.json`에서 다음 순서를 확인했다.

| 순서 | 프로세스 | API 관찰 |
| --- | --- | --- |
| 1 | Perl 30864, thread 37200 | Peek seq 166: `80×25` 크기 이벤트 한 개 |
| 2 | 부모 Bash 21624, thread 33384 | Read seq 2: 크기 이벤트와 `printf ...21...` 키 총 59개 제거 |
| 3 | 부모 Bash | Write seq 3/4: 크기 이벤트를 제외한 키 58개 되돌리기 |
| 4 | Perl | Read seq 168: 앞선 조회의 개수 1로 읽지만 반환된 것은 `p` key-down |
| 5 | Perl | 다음 Peek seq 170: `p` key-up과 `rintf ...21...`가 남음. 실제 ACK도 `rintf` |

두 API 호출은 숫자가 다른 콘솔 핸들을 쓰지만 같은 입력 큐에 대한 제거·되돌리기와 남은 키의 변화를 보여준다. `ReadConsoleInput`은 실제로 읽은 이벤트를 제거하고 Peek는 제거하지 않는다. [Microsoft API 의미](https://learn.microsoft.com/en-us/windows/console/readconsoleinput)

`node artifacts/experiments/console-trace-analyze.cjs <위 fixture 경로>`는 이 순서를 다시 생성한다. 추적 대상 두 프로세스의 종료도 확인했다. 동작을 바꾸지 않는 계측에서 손실이 재현되었으며 Windows API 단독·MSYS raw read·production Session 결과와 일치하는 범위다.

## 수정 후보와 남은 검증

[수정 후보](patches/msys-resize-signal-after-discard.patch)는 `process_input_message()`가 크기 이벤트를 처리할 때 먼저 기존 입력 잠금 안에서 확인한 이벤트를 소비하고, 이후 SIGWINCH를 보내도록 순서를 바꾼다. 이벤트를 소비하기 전에 잠금을 놓아 다른 프로세스가 큐를 바꾸는 구간을 피한다. `create-msys-resize-candidate.cjs`가 기록된 소스로부터 생성하며 자체 SDK에서 기준 DLL과 후보 DLL을 빌드했다. **사용자 Git/설치본에는 적용하지 않았다.**

master thread를 끄는 대조 실험도 했다. `console-trace-f4473fea-7629-4859-9276-a61c681a0f1e`에서 private Win32 input mode 출력 뒤 첫 입력부터 응답하지 않았고 일반 종료 입력도 처리하지 못했다. 그 fixture의 소유 프로세스만 확인해 정리했다. **이 방식은 실패했으며 제품의 우회로로 사용하지 않는다.** 단순 비활성화가 신호·입력 모드·잡 제어와 호환된다고 가정하지 않는다.

격리 후보 v3는 즉시 raw/canonical 입력 각각 1,000회, 입력 없는 SIGWINCH/siglongjmp 대기 1,000회, 기존 production Session·prompt·ConPTY·Windows bridge·Codex 알림 검사 10개를 통과했다. 같은 DLL을 쓰는 별도 native GUI도 통과했다. 초기 ANSI 도착 실패와 prompt 간헐 실패가 이번에는 재현되지 않았지만 모두 같은 원인이었다고 단정하지 않는다. SHA·실행 기록·검증의 한계는 [런타임 검증](patches/MSYS_RUNTIME_VALIDATION.md)에 있다. 기본 제품 적용·배포/복구·실제 IME/pager/mouse-report·전체 잡 제어·다른 버전/부하는 남는다.

무수정 기준 DLL에서도 raw 두 번째 줄의 `printf`→`rintf`가 재현됐다. v1 DLL만 같은 기준 runtime 디렉터리에서 바꾸면 1,000회 통과하고 기준 DLL을 복원하면 두 번째 줄에서 다시 누락됐다. `artifacts/msys-runtime-build-fa913284-a51e-4702-9f39-0d82eb94a89f/same-directory-control.json`에 조건을 기록했다.

신호 검사에서 v1은 109/200, v2는 141/200 후 새 SIGWINCH 대기 응답을 받지 못했다. 무수정 기준 DLL도 200회 샘플은 통과했지만 1,000회 검사에서는 266회 또는 147회에서 실패했다. 실패 후 종료 입력은 정상 처리됐고 추가 신호도 없었다(`msys-sigjump-cbb05b56-f682-4dc7-b218-cbf81a2de107`, 정리 전/후 출력·카운터). 이를 패치가 만든 회귀나 영구 입력 잠금으로 단정하지 않았다.

소스의 `set_cursor_maybe()`·출력·TIOCGWINSZ도 `con.fillin()`으로 `dwWinSize`를 갱신한다. 기존 신호 경로는 그 캐시의 갱신 전/후 차이만 비교한다. v3는 기존 공유 `tty_min::winsize`에 마지막 보고 크기를 분리하고 입력 잠금 아래에서 비교·갱신한다. v2와 같은 디렉터리·C 소비자에서 이 후보의 신호 대기 1,000회가 통과했다. 모든 누락의 내부 호출 순서를 새로 계측해 확정한 결과로 확대하지 않는다. 후보별 DLL·패치·소스와 실패 기록은 보존했다.
