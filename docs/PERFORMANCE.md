# MiniTerm 네이티브 UI와 성능 기준

2026-10-10. 사용자의 “Warp를 참고해 성능을 고려한 완성도 있는 터미널 구현” 요구를 반영한다. 이 문서의 기술 선택은 PLAN.md와 WINDOWS_HANDOFF.md의 초기 Tauri 제안을 대체한다.

## 선택과 근거

현재 앱은 Rust 네이티브 실행 파일이다. egui/eframe의 Windows 이벤트 루프와 wgpu의 DX12 렌더러를 사용하며, WebView2·DOM·JavaScript·터미널 출력용 JSON IPC를 포함하지 않는다. GPU 선택은 LowPower로 설정한다. 이는 구조상 비용을 줄이는 선택이며 성능 목표 달성을 증명하는 측정 결과는 아니다.

DX12 디바이스에는 `MemoryHints::MemoryUsage`를 지정한다. wgpu 27의 DX12 기본 throughput 설정은 GPU/host suballocation pool을 256MiB/64MiB로 잡고, MemoryUsage는 8MiB/4MiB를 사용한다. 작은 상주 HUD에서는 실제로 이 차이가 앱 메모리에 크게 영향을 줬다. 수치는 사용 중인 버전의 소스에서 확인했다: `.tools/cargo/registry/src/.../wgpu-hal-27.0.4/src/dx12/suballocation.rs`.

로컬 `warp/` 참고 소스의 확인 커밋은 `2055f0f48`이다. `crates/warpui/src/rendering/glyph_cache.rs`에서 glyph·scale factor·subpixel alignment별 atlas 캐시, `rendering/wgpu/renderer.rs`에서 rect/glyph/image GPU pipeline과 present 경로를 확인했다. MiniTerm에서도 DPI가 바뀌면 행 shape 캐시를 무효화하고 폰트 layout과 GPU 준비 동안 VT 잠금을 유지하지 않는다. Warp 코드를 가져오거나 빌드 의존성으로 추가하지 않았다.

Warp 공식 설명은 Rust UI, GPU 렌더링, glyph atlas 재사용, draw call 최소화 및 보이는 콘텐츠만 렌더링하는 구조를 강조한다. MiniTerm은 이 원칙을 적용하며 Warp의 블록 UI·명령 편집기·AI 기능을 복제하지 않는다. 기존 Bash/TUI의 정확한 동작과 작은 HUD의 유휴 자원을 우선한다.

- [Warp: How Warp Works](https://www.warp.dev/blog/how-warp-works) (2021-07-12)
- [Warp: The Block Model Behind Warp's Agentic Development Environment](https://www.warp.dev/blog/block-model-behind-warps-agentic-development-environment) (2026-04-29)
- [egui의 네이티브 GPU 백엔드](https://github.com/emilk/egui)
- [Alacritty 터미널 코어](https://github.com/alacritty/alacritty/tree/master/alacritty_terminal)

ANSI/VT 파서·UTF-8 상태·alternate screen·스크롤백·선택·reflow는 Alacritty 코어를 재사용한다. GPU에 표시하는 셀과 HUD만 MiniTerm에서 구현한다. 검증된 코어의 사용만으로 IME·키 입력·TUI 호환성이 검증되었다고 간주하지 않는다.

## 현재 구현의 비용 관리

| 경로 | 구현 원칙 |
| --- | --- |
| PTY 읽기 | 전용 스레드가 16KiB 단위로 계속 소비하고 VT를 처리. UI로 전체 출력을 복사하지 않음 |
| PTY 쓰기 | 전용 스레드, 프레임 내 입력 이벤트를 FIFO로 배치, 슬롯 수 및 총 1MiB 바이트 한도. UI는 blocking write를 수행하지 않음 |
| PTY 크기 변경 | 입력과 같은 비동기 FIFO 작업자에서 순서대로 처리, 크기가 실제로 달라진 경우만 요청 |
| 화면 업데이트 | 출력 변경 시 이벤트로 wakeup, 연속 출력 repaint 요청은 합침. 매 프레임 전체 프로세스 폴링 없음 |
| 숨긴/비선택 세션 | VT와 scrollback은 갱신, 터미널 화면은 렌더링하지 않음 |
| 터미널 그리기 | viewport에 보이는 행만 조회, 행별 내용/스타일 해시가 달라진 경우만 glyph shape 재구성. DPI 변경 시 캐시 무효화 |
| 글꼴 | glyph atlas와 layout cache를 GPU 렌더러가 재사용, Windows 설치 글꼴 사용 |
| 유휴 상태 | 커서 blink 루프 없음. 확장/축소 애니메이션은 180ms 동안만 repaint하고 종료. 입력·출력·창 변경 시 갱신 |
| 셸 수명 | 창 크기 변경·숨기기·세션 선택으로 셸을 다시 실행하지 않음 |
| 종료 | Windows Job Object로 Bash의 자식 프로세스도 함께 종료 |
| 기본 프로필 | Git Bash/NTFS, 앱 시작 시 WSL 검색·기동 없음 |

창·동작 설정은 마우스로 조절한 크기를 DIP로, 이동한 위치를 모니터 작업 영역의 상대 기준점으로 저장한다. 모니터와 작업 영역은 배치 변경 시 조회하며 상시 폴링하지 않는다. `GetDpiForWindow`로 최종 배치의 DPI를 적용하고, 지정 화면이 없으면 주 모니터로 복귀한다. Windows의 애니메이션 설정과 앱의 동작 줄이기를 존중한다. 펼치기 시작 시 최종 PTY 크기를 한 번 요청하고 같은 폭으로 위로 연다. 미니로 축소할 때는 VT 크기를 유지해 TUI가 작은 그리드로 재배치되지 않게 한다. 마우스 크기 조절 중에는 PTY 요청을 반복하지 않고 조절 종료 시 확정한다. [Windows DPI](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getdpiforwindow), [Windows 애니메이션 설정](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-systemparametersinfow)

아이콘은 Rust/egui 선·도형으로 그리며 별도 아이콘 폰트·이미지 디코더·런타임 의존성을 추가하지 않는다. 호버·선택·키보드 포커스·알림 배지를 같은 위젯에서 처리하고 유휴 애니메이션을 사용하지 않는다.

한 줄 프롬프트는 Bash PS1에서 구성하며 VT 출력의 줄바꿈을 제거하지 않는다. `--login --noprofile -i`의 첫 프롬프트에서 정상 로그인 초기화 파일을 한 번 읽고 기존 PROMPT_COMMAND를 유지한다. 설정 저장은 세션별 작은 상태 파일을 원자적으로 교체한다. Bash의 buffered `mapfile` builtin이 프롬프트 표시 시에만 여섯 줄을 읽으며 키 입력·실행 중 프로그램·유휴 시간에는 조회하지 않는다. 명령 실행·외부 프로세스 생성·PTY 명령 주입으로 설정을 갱신하지 않는다. [Bash 시작 파일](https://www.gnu.org/software/bash/manual/html_node/Bash-Startup-Files.html), [PROMPT_COMMAND](https://www.gnu.org/software/bash/manual/html_node/Bash-Variables.html)

모서리는 `DwmSetWindowAttribute`의 고정 `DWMWCP_ROUNDSMALL`을 사용한다. 픽셀 단위 region을 제거해 Windows 합성기의 부드러운 경계를 사용하고 사용자 반경 설정은 제공하지 않는다. 투명도는 `SetLayeredWindowAttributes`의 창 전체 alpha로 적용하고 0%에서는 layered 스타일을 제거한다. winit의 창 상태 갱신이 alpha 스타일을 지우지 않도록 WM_STYLECHANGING에서 유지한다. [Windows 11 모서리 합성](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners), [창 alpha](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setlayeredwindowattributes), [WM_STYLECHANGING](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-stylechanging)

테마 전환 시 egui가 글꼴 atlas를 재생성한다. 매 프레임 사용하는 기준 글리프의 캐시 객체가 교체되면 행 캐시를 비워 이전 atlas 좌표로 문자가 깨지는 문제를 방지한다. DPI·글꼴·화면 위치 변경도 캐시 키에 반영한다. 한글·영문·숫자는 같은 기준선으로 정렬한다. 기본 Consolas의 실제 볼드·이탤릭 얼굴을 사용하고 별도 얼굴이 없는 사용자 글꼴은 합성 강조를 적용한다.

행의 모든 배경을 먼저 그린 뒤 글자를 그린다. 선택 배경·반전 색상이 있는 두 칸 한글의 오른쪽을 다음 spacer 셀의 배경이 덮지 않도록 한다. Windows의 Segoe UI Symbol을 추가 폴백으로 읽어 기본 글꼴에 없는 AI Battery의 `❚`와 점자 등 TUI 기호를 제공한다. 사용자 글꼴과 한글 폴백은 유지하며 OS 글꼴을 배포 파일에 복사하지 않는다.

고정 OS 글꼴 7개는 프로세스 수명 동안 한 번 읽어 공유한다. egui의 borrowed `FontData`/`FontRef` 경로를 사용해 큰 한글 TTF를 다시 `FontVec`로 복사하지 않는다. 캐시 대상은 고정된 파일 목록이며, 임의의 사용자 글꼴을 영구 캐시하지 않는다. 사용자 글꼴은 기존 owned 경로로 교체 시 해제한다. OS 글꼴을 바꾸면 앱을 다시 시작해야 한다.

글꼴 선택·다국어 폴백·볼드/이탤릭·색상과 약한 강조의 보존은 [Ghostty의 글꼴·스타일 설정](https://ghostty.org/docs/config/reference#font-family)과 [iTerm2의 텍스트 프로필](https://iterm2.com/documentation-preferences-profiles-text.html)을 참고한다. 기본 14px Consolas와 한글 폴백을 제공하고 설치된 Cascadia Mono/Code 등의 글꼴을 GUI에서 선택한다. 프로그램이 출력한 ANSI 스타일을 유지하며 터미널이 임의로 평문을 요청/응답/코드로 분류하지 않는다.

공유 VT 잠금은 bounded chunk 처리 동안만 잡는다. 대량 출력에서 잠금 경쟁이 나타나면 측정 결과를 근거로 snapshot 이중 버퍼 또는 행 damage 기반 갱신을 적용한다. 임의의 출력 손실·history 무제한 증가로 수치를 개선하지 않는다.

VT 상태에는 Alacritty의 FairMutex를 사용한다. UI는 visible snapshot을 복사한 뒤 잠금을 풀고 glyph layout을 수행한다. reader가 연속 출력 중 잠금을 다시 얻으며 UI를 계속 기다리게 하는 상황을 줄인다.

PTY 출력 운반과 파싱을 나누고 16 × 16KiB의 bounded 큐를 둔다. 파서는 평소에는 출력 수신을 무기한 기다리고, `CSI ?2026h`로 시작한 동기화 프레임이 있을 때만 VT 코어의 150ms 제한 시간으로 기다린다. 종료 신호 `CSI ?2026l`이 출력 읽기 경계에 걸려도 처리하며, 더 이상 출력이 오지 않거나 EOF가 되어도 남은 프레임을 확정한다. 동기화 중간 내용 때문에 화면 갱신을 반복하지 않고 유휴 타이머 폴링도 하지 않는다. 이 회복 경로의 통과만으로 실제 Codex 메뉴 위치가 해결되었다고 간주하지 않는다.

## 통과 기준과 측정의 구분

기존 목표를 유지한다: 이미 실행 중인 세션의 클릭→입력 가능 p95 ≤100ms, 입력→화면 표시 p95 ≤50ms, 유휴 앱 CPU 평균 ≤0.5%, 앱 메모리 ≤200MB. 입력 측정은 100회 이상, 작업 비교는 10회 이상 수행한다. DX12 전용/공유 GPU 메모리와 Bash·CLI 에이전트·WSL 자원도 별도로 기록한다.

`scripts/test.ps1 -Pty`은 실제 ConPTY/Bash의 호환성과 100회 명령 round trip을 측정한다. **이 round trip 수치는 GUI 클릭·키 입력→pixel p95가 아니다.** shell prompt 재표시와 초기화 비용이 포함되며 GPU 표시 비용은 포함되지 않는다.

`scripts/measure-idle.ps1 -ProcessId <PID> -Seconds 30`은 앱의 CPU 시간 증가, working set, private bytes를 기록한다. 전체 논리 CPU 수로 나눈 CPU 백분율과 단일 코어 기준 비율을 모두 기록한다. release 실행 파일을 사용하고 shell/agent는 별도로 측정한다. 결과는 장치 및 상태에 종속되며 통합 GPU shared memory나 파일시스템별 비교를 대체하지 않는다.

최종 GUI 지연은 Windows ETW/WPR 또는 input→present를 연계하는 instrumentation으로 확인한다. UI 구성 시간, GPU frame/present, end-to-end 지연을 구분한다. 평균 FPS만으로 유휴 자원 또는 키 입력 지연을 주장하지 않는다.

Windows Terminal과는 같은 Bash 버전·초기화 파일·폴더·도구·데이터·행/열 크기로 비교한다. Windows Bash/NTFS, WSL Bash/Linux /home, WSL Bash/NTFS의 A/B/C 표는 WINDOWS_HANDOFF.md의 절차를 유지한다. WSL cold start, 네트워크, 캐시 및 데이터 준비 시간은 작업 시간과 구분한다.

## 출시 전에 남은 실기 검증

한글 첫 입력/조합/취소/후보창, mixed DPI·음수 좌표·모니터 분리, 작업표시줄 자동 숨김, 선택·붙여넣기·mouse-report TUI, Ctrl+C·Ctrl+Z/jobs/bg/fg, 출력 폭주 중 입력, 4세션, 알림 클릭 복귀, 깨끗한 설치 환경을 검증한다. 이 항목들이 통과하기 전에는 P0 완료 또는 Warp 수준의 호환성·성능으로 표시하지 않는다.

## 2026-10-12 글꼴 캐시 메모리 검증

기호·한글·ANSI 샘플과 설정 화면의 테마 전환을 거친 release 앱에서 기존 메모리 초과를 재현했다. epaint는 장치의 최대 텍스처 크기를 글꼴 atlas의 폭으로 사용한다. 입력의 텍스처 한도를 최대 4096으로 제한해 작은 HUD에 필요한 글리프를 더 좁게 배치한다. 장치 한도가 더 작으면 그 값을 유지하며, 한도 변경이 없는 프레임의 `None`도 유지해 atlas를 매 프레임 바꾸지 않는다. 글꼴·한글·기호 폴백은 제거하지 않았다.

| 시나리오 | 관찰 시간 | 앱 최대 working set | 유휴 CPU | 메모리 목표 |
| --- | --- | --- | --- | --- |
| 수정 전: 1세션, 기호/테마 회귀 후 | 30.53초 | 206,626,816bytes | 0.038% | 초과 |
| 수정 후: 같은 1세션 시나리오 | 30.54초 | 184,573,952bytes | 측정 해상도 이하 | 통과 |
| 수정 후: 4세션, 기호·요약·계획 보고 회귀 후 | 30.52초 | 185,303,040bytes | 측정 해상도 이하 | 통과 |

장치는 Intel Arc 130V / DX12이며 목표는 200,000,000bytes다. 수정판 SHA256은 `17f9476b1918106e69d59065ffdbf9f964fbb9c1358e0246b83ea5fb98b78480`, 기록된 마지막 atlas 크기는 4096×32다. 원시 기록은 `artifacts/ux-a4ebb0f0-21fa-49d1-9453-4ad4637e2869`(기준), `artifacts/ux-8336747d-45a5-4ba6-8280-33f1150ff457`(1세션), `artifacts/ux-ba381792-7fe9-4730-9415-9dc078cab78b`(4세션)에 있다.

두 테마의 고정 글자 영역 각각 110,160픽셀에서 기준판과 달라진 픽셀은 24개였고, 화면 검토에서 한글·배터리 막대·선택/반전·ANSI 대비를 확인했다. 로직 53개·fmt·Clippy·release 빌드와 GUI의 요약·보고·더블 클릭·자체 Bash 종료가 통과했다. 포커스 검사는 다른 앱으로의 전환과 테스트 앱의 전면 활성화를 구분하도록 수정했으며, 시작과 종료 시점의 관찰만 검증한다.

이 값은 고정 샘플 뒤 앱 유휴 상태의 측정이다. GPU 전체 메모리·Bash/에이전트·출력 부하·입력→pixel·다른 장치를 검증한 결과가 아니며, 현재 실행 중인 사용자 앱이나 공개 alpha.1에 소급 적용하지 않는다.
