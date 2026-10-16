# 브랜드·단축키 후속 변경

> 이후 스크롤 수정과 최신 로컬 실행 파일은 [Codex 스크롤 수정](SCROLL.md)을 따른다. 아래 해시·검증은 브랜드·단축키 변경 당시 기록이다.

2026-10-13 사용자 후속 요청 기준. 브랜드를 하나의 이어진 민트색 M 심볼로 변경하고, 없는 터미널 조작 단축키를 추가했다. 설정창의 저장 설명도 제거했다.

## 브랜드와 설정창

- `>_`와 보라색 창을 겹친 기존 아이콘을 대체했다. M의 양쪽 윤곽과 터미널의 끝선을 같은 굵기·색상의 연속 경로로 묶는다. 작은 상단/트레이 크기에서도 읽히도록 단일 실루엣을 사용한다.
- [SVG](../assets/miniterm.svg)와 [픽셀 생성기](../assets/brand_pixels.rs)의 경로·색상을 일치시킨다. 16/24/32/48/64/128/256px ICO를 다시 생성하고 상단·설정창·트레이·창·EXE·설치/제거 아이콘에 사용한다.
- 설정창에서 “변경한 설정은 바로 적용하고 저장합니다”, “X로 닫아도 적용한 설정은 유지됩니다”, X 툴팁의 저장 설명을 제거했다. 오류와 CLI 등록 결과가 있을 때만 하단 안내를 표시한다. 즉시 적용·저장 동작과 고정 창 크기는 유지한다.

## 단축키 목록

MiniTerm 본창에 포커스가 있을 때 적용한다. 셸/TUI로 보내기 전에 앱의 키 이벤트를 소비한다. 설정 편집·종료 확인·IME 조합 중에는 본창 단축키를 처리하지 않는다. 탭은 화면 번호 순서로 이동하며 마지막/처음에서 순환한다. 없는 번호는 탭을 새로 만들지 않는다.

| 동작 | 키 | 상태 |
| --- | --- | --- |
| 새 탭 | `Ctrl+Shift+T` | 기존, 최대 4개 |
| 현재 탭 닫기 | `Ctrl+Shift+W` | 추가, 종료 확인 |
| 다음 / 이전 탭 | `Ctrl+Tab` / `Ctrl+Shift+Tab` | 추가 |
| 1~4번 탭 선택 | `Ctrl+Alt+1` ~ `Ctrl+Alt+4` | 추가 |
| 설정 열기 | `Ctrl+,` | 추가 |
| 선택 복사 | `Ctrl+Shift+C` | 기존. `Ctrl+C`도 선택이 있으면 복사, 없으면 인터럽트 |
| 붙여넣기 | `Ctrl+V` / `Ctrl+Shift+V` / `Shift+Insert` | 기존 egui-winit 경로, bracketed paste 처리 |
| 글꼴 확대 / 축소 | `Ctrl++` 또는 `Ctrl+=` / `Ctrl+-` | 추가, 크기 10~24, 저장 |
| 글꼴 기본 크기 | `Ctrl+0` | 추가, 기본 크기 14, 저장 |
| 출력 기록 위 / 아래 | `Ctrl+Shift+↑` / `Ctrl+Shift+↓` | 추가, 3행 |
| 출력 기록 한 페이지 이동 | `Ctrl+Shift+PageUp` / `Ctrl+Shift+PageDown` | 추가 |
| 출력 기록 처음 / 마지막 | `Ctrl+Shift+Home` / `Ctrl+Shift+End` | 추가 |
| 확장 / 축소 | `Ctrl+Alt+Enter` | 추가, MiniTerm 전용 |
| 입력 / 요약 전환 | `Ctrl+Alt+M` | 추가, MiniTerm 전용 |
| 종료 확인 취소 / 설정 닫기 | `Esc` | 확인 취소를 추가, 셸에는 전달하지 않음 |
| 앱 표시 / 숨기기 | `Ctrl+Alt+Space` 기본 | 기존 전역 단축키, 설정에서 변경/비우기 가능 |
| 투명도 | `Ctrl+Alt+휠` | 기존, 5%씩 |

새 탭·탭 이동·번호 선택·설정·글꼴·기록 이동은 [Windows Terminal 공식 기본 키 설정](https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalSettingsModel/defaults.json)에 맞춘다. Windows Terminal의 `Ctrl+Shift+W`는 현재 pane 닫기이며, pane 분할이 없는 MiniTerm에서는 현재 탭의 셸을 닫는 확인으로 연결한다. MiniTerm 전용 두 조합은 위 표에서 구분했다. `Alt+F4`는 기존 상주 앱 동작에 따라 HUD를 숨기며, 앱 전체 종료는 트레이의 종료 확인을 사용한다.

상단 탭·브랜드·전환 아이콘 툴팁에 관련 키를 표시한다. 설정 → 창과 동작의 **키보드 단축키**를 펼쳐 전체 목록을 볼 수 있다. 앱 단축키와 일치하지 않는 `Ctrl+W`, `Alt+T`, 추가 수정키 조합은 기존 셸 입력 경로를 유지한다. 글꼴 조절은 egui 전체 화면 배율과 분리하며, 기록 이동은 원래 VT 기록을 사용한다. 대체 화면에 없는 스크롤백을 만들어내지 않는다.

## 검증 방법과 적용 범위

```powershell
./scripts/test.ps1
./scripts/build.ps1
./scripts/verify-shortcuts.ps1 -FixtureKeys
./scripts/verify-settings.ps1
```

`verify-shortcuts.ps1`의 기본 모드는 Windows `SendInput`을 사용한다. 이 환경에서 합성 Ctrl 상태가 유지되지 않아 첫 새 탭 조합을 확인하지 못했다. 포커스 확보와 수정키 유지 시간을 달리한 실패도 보존한다. 사람의 실제 키보드 조합 검사는 남아 있다.

`-FixtureKeys`는 별도 데이터·IPC를 쓰는 진단 앱에 제한된 egui 키 이벤트를 넣는다. 실제 단축키 선택기를 통과시켜 GUI의 탭·설정·글꼴·기록·종료 확인 동작을 검사하며 앱 액션을 직접 호출하지 않는다. `MINITERM_DIAGNOSTICS=1`과 별도 `MINITERM_INSTANCE_ID`가 모두 있는 앱에서만 이 시험 입력을 허용한다. 출력 기록은 고정 VT 표본을 사용한다. 일반 앱·사용자 셸·CLI 설정·클립보드를 변경하지 않는다. 두 입력 모드는 결과 JSON의 `inputMode`로 구분한다.

일반 자동검사 68개(단위 67개·fixture 1개)가 통과했다. egui 키 이벤트의 정확한 수정키 매칭·PTY 전달 차단과 일반 셸 키 보존·Escape 취소를 검사한다. ignored 단위 3개·환경 통합 16개는 통과로 세지 않는다.

## 최종 기록

- 단축키 GUI: `artifacts/shortcuts-80376ca4-c1a2-4722-af90-412ab2aebc27/verification.json`, `passed=true`, `inputMode=egui_fixture`. 새 탭·다음/이전/번호 선택과 없는 번호·일반 Ctrl+W 보존·글꼴 확대/축소/기본과 전체 UI 배율 유지·입력/요약·설정/닫기·확장/축소·기록 이동·종료 확인/취소·올바른 탭 종료·셸 ID 유지·EXE 아이콘 모두 통과했다.
- 설정 GUI: `artifacts/settings-b69b236a-5038-4026-8b9e-c78f69408cbe/verification.json`, `passed=true`. 설명을 제거한 새 배치에서도 즉시 저장·다크/라이트·X/재열기/재시작·고정 크기·숨긴 HUD·투명도·오류 입력과 별도 설정·프롬프트/셸 ID·격리된 CLI 콜백 보존이 통과했다.
- 캡처: 위 단축키 기록의 `brand-summary.png`, `exe-icon.png`, `settings-clean.png`, `shortcut-close-confirmation.png`. 상단/EXE 브랜드와 설정창의 안내 제거, 종료 확인 배치를 직접 확인했다.
- 실행 파일: `artifacts/miniterm.exe`, SHA256 `ce1b777615c82d17cdfd6ba85725daa4b4f40ef0f804ce9e2240c8875fbcd330`.
- 소스 지문: `4fea6e8231e6101639dc8ed0f4e95d9fee9b6c2ac7452bd4c5dea4d69946857e`, `builtUtc=2026-10-13T02:04:19.2485880Z`. 두 최종 GUI 검사와 같은 실행 파일이며 현재 소스 지문과 일치한다.

최초 포커스 실패 `shortcuts-ceff70a0-b4e2-4d7a-a517-77f3ec1cc6b7`, 합성 키 실패 `shortcuts-6335ee85-c010-4d10-80f9-e7b31dd1c060`·`shortcuts-bfe4d738-6f33-4c25-bd55-0cfc5918ccf3`는 보존한다. 키 이벤트 모드의 최초 실행 `shortcuts-90e2f518-706d-4f0a-a5af-a61adc0baf6e`는 기록을 만들기 위한 타이핑에서 `printf`의 첫 `p`가 누락돼 중단했다. 그 입력 실패를 보충하거나 재전송하지 않았다. 원인은 이 검사로 확정하지 않으며 Git 입력/resize 호환성 미해결 범위를 유지한다. 최종 기록 이동 검사는 고정 VT 표본으로 분리했으며 ConPTY 입력 호환성 통과를 뜻하지 않는다.

공개 릴리스·설치본을 교체하지 않았다. 기존 작업을 마친 뒤 새 `artifacts/miniterm.exe`로 다시 실행해야 적용된다. 로컬 원시 기록은 Git 제외 경로다.
