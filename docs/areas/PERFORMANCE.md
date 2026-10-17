# 성능 세션 인계

> 2026-10-16 alpha.7 사용자 후속: [설정창 글자·Codex 연결·휠 수정](../../feature/CODEX_SETTINGS_FIX_2026-10-16.md). 본창의 atlas 제한 불일치와 Codex 추가 연결 직렬화를 수정했다. 실제 스트리밍 중 축소/확장 뒤 정지한 커서의 휠 누락도 재현·수정했다. 최종 EXE `4f58251e…`, 일반 97개 및 실제 GUI의 요청·응답·입력·스크롤·취소/재요청·재개 통과. 설정창 30초 유휴 최대 149,983,232bytes이며, 기본 AI Battery 브리지 활성 중 최대 209,707,008bytes도 관측했다. 모든 활성 상태가 200MB 이내라고 판정하지 않는다. 이전 안정성 실행 파일과 최신 수정본을 구분한다.

> 2026-10-16 안정성 보완: [메모리·종료·응답 경로 보완](../../performance/STABILITY_2026-10-16.md). 콘솔 호스트 자원 보호, 파이프 취소, VT 응답 대기 분리, history·결합 문자 상한, 글꼴 매핑과 Codex 중계를 수정했다. 일반 검사 96개·별도 ConPTY 검사 1개·release·18만 행 출력·공유 Codex 0.163.0 회귀가 통과했다. 1세션 30초 유휴 최대 working set은 150,097,920bytes다. 실행 파일·추가 검증과 기존 Git resize 4건은 해당 기록을 따른다. 아래의 문서 전용·일시중지 안내는 이전 인계 시점의 상태다.

> 2026-10-13 사고 후속: [콘솔 메모리 고갈 조사](../../performance/INCIDENT_2026-10-13.md). Windows 로그에서 OpenConsole 45.3GB 커밋 사용을 확인했고 외부 DLL 자동 선택을 차단하는 소스를 추가했다. 정확한 트리거와 새 EXE 빌드는 미완료이며 기존 EXE용 임시 우회 스크립트가 있다.

> 후속 안내: 사용자가 `performance/performance.md`의 CLI 에이전트 Bash 실행 요구사항 구현을 요청해 해당 개발을 재개했다. 최신 변경·검증·제한은 [Bash 실행 연결 결과](../../performance/IMPLEMENTATION.md)를 따른다. 아래 P1/P2/P3 계측 기록은 별도 범위다.

2026-10-12 기준. 제품 구현 목표는 **일시중지**이며 이번 세션은 읽기와 이 문서 작성만 수행했다. 빌드·시험·계측·GUI·배포를 실행하지 않았다.
새 세션은 [공통 인계](../SESSION_HANDOFF.md)를 먼저 읽고 이 문서를 읽는다. 작업 위치는 `C:\Users\ghwls\00_Projects\miniterm`; 구성은 **기능·디자인 통합 / 성능** 두 세션이다.

## 현재 기준과 증거

| 대상 | 기록과 적용 범위 |
| --- | --- |
| 작업 소스 | 인계 기준 `main`, `17cbfc701f5cdd533857e79a54de541a7dc4d4eb`. 이번 세션에서 Git 상태를 다시 조회하거나 변경하지 않았다. |
| 공개 alpha.6 | 소스 `09d7c01780be6b091597030e07f0474f3b5b144e`, exe SHA256 `f94f402b89177766381e991fa288676c9a1fb6f0eb4e89cb2f59d7ccab765cb4`. 배포 CI 성공과 성능 목표 달성은 별개; 성능 미검증. |
| 공개 alpha.3 | exe `2b2f6b32c18190e5e5b547dea78ee9424cda206f8d9fff580bb43fbb5b0da01d`; Intel Arc 130V/DX12, 4세션 샘플 후 30.52초 유휴 최대 working set **210,030,592bytes: 실패**. `artifacts/ux-e17b06e5-43d9-4fe4-90e4-99c300511124`. |
| 로컬 수정판 | exe `17f9476b1918106e69d59065ffdbf9f964fbb9c1358e0246b83ea5fb98b78480`; 1세션 184,573,952 / 4세션 185,303,040bytes: 해당 유휴 시나리오 통과. `artifacts/ux-8336747d-45a5-4ba6-8280-33f1150ff457`, `artifacts/ux-ba381792-7fe9-4730-9415-9dc078cab78b`. 공개 빌드에 대입하지 않는다. |
| 후속 개발판 | `1dabcc9b…` 185,757,696bytes 및 MSYS 후보 `fa322901…` 185,511,936bytes는 각각 별도 exe/런타임 결과. 전자는 `artifacts/ux-2ca13445-060f-477e-a8b6-2a7d18eeca52`, 후자는 `artifacts/ux-78519338-e367-4fa5-9222-8da7053b5e48`. |

근거 문서는 [성능 기록](../PERFORMANCE.md), [현재 제품 제한](../STATUS.md), [계획의 인수 목표](../PLAN.md)다. 표의 원시 파일은 Git 제외 대상이며 새 clone에 없을 수 있다; 문서 기록과 실제 원시 증거 확보를 구분한다.

## 목표와 검증 범위

- 이미 실행 중인 세션: 클릭→입력 가능 p95 ≤100ms, 입력→화면 표시 p95 ≤50ms. 입력 표본 ≥100회, 작업 비교 ≥10회; 최초 셸/WSL 기동과 초기화 비용은 별도다.
- 유휴 앱 CPU 평균 ≤0.5%, 앱 메모리 ≤**200,000,000bytes**. CPU는 전체 논리 CPU 수로 나눈 비율과 단일 코어 비율을 함께 기록하며, 메모리는 측정 구간 최대 working set으로 판정한다. MB/MiB 환산으로 통과 기준을 바꾸지 않는다.
- `PLAN.md`는 한 세션 목표를 명시한다. 4세션은 기존 측정과 최대 세션 동작을 위한 별도 확장 시나리오다; 세션 수·표본 준비·선택/숨김 상태를 결과마다 표시한다.
- 앱 CPU/working set/private bytes, DX12 전용·공유 GPU 메모리/사용률, Bash·에이전트·WSL 자원을 분리한다. private bytes나 GPU shared memory를 앱 working set의 대체값으로 사용하거나 단순 합산해 판정하지 않는다.
- PTY 명령 round trip은 셸 처리·프롬프트 비용을 포함하고 GPU 표시를 제외한다. UI 구성 CPU 시간, GPU frame/present, 실제 입력→pixel은 각각 다른 지표다; FPS와 애니메이션 geometry 시간으로 입력 지연을 주장하지 않는다.

## 구현·검증 완료와 한계

- 소스 확인: Rust/egui/wgpu DX12, LowPower와 `MemoryHints::MemoryUsage`; atlas 한도 최대 4096, 고정 OS 글꼴 7개 공유, borrowed font data, 행 내용·스타일 캐시와 atlas/layout 변경 시 무효화.
- `src/app.rs`는 보이는 셀 snapshot을 복사한 뒤 VT 잠금을 풀고 layout/shape를 구성한다. 숨김·비선택 세션은 화면을 렌더링하지 않으며, 확장 애니메이션은 180ms 동안만 repaint한다.
- `src/terminal.rs`는 16KiB 읽기, 16슬롯 출력 큐(256KiB), 별도 파서와 FairMutex, 입력/resize 공통 FIFO(16슬롯·입력 1MiB 한도), 출력 repaint 병합을 구현한다. 유휴 파서는 대기하고 동기화 프레임 중에만 제한 시간으로 깨어난다.
- 과거 로컬 GUI/유휴 회귀 통과와 공개 alpha.3 메모리 실패는 위 문서의 기록이다. 이번 인계에서 재검증하지 않았고, 출력 부하·GPU 전체 비용·다른 장치·실제 IME·입력→pixel 통과를 뜻하지 않는다.
- `scripts/measure-idle.ps1`: 앱 PID의 CPU 증가와 1초 메모리 표본을 측정하나 현재 JSON은 집계만 보존한다. exe 해시·셸 버전·장치 조건·GPU/자식 자원·원시 시계열은 추가 산출물이 필요하다. 0 CPU 표시는 측정 해상도 한계다.
- `scripts/verify-ux.ps1`: 격리된 `artifacts/ux-<GUID>`에 `verification.json`, `idle.json`(선택), `metrics.json`, 캡처를 보존한다. `metrics.json`은 종료 때 최초 최대 10,000프레임의 UI 구성 CPU 통계를 저장하며 GPU present/입력→pixel을 제외한다. 공용 `artifacts/ux.json`만 증거로 삼지 않는다.

## 미해결과 우선순위가 있는 다음 세 작업

공개/로컬 메모리 차이 원인은 미확정이다. 공개 alpha.3도 atlas 4096×32였으므로 atlas 크기만으로 차이를 단정하지 않는다. alpha.6 기본 Git의 알려진 입력/resize 실패는 기능·디자인 세션이 담당하며 성능 수치로 호환성 실패를 덮지 않는다.
아래 작업은 사용자가 구현·검증 재개를 명시한 뒤 순서대로 수행한다.

1. **P1 — 공개 alpha.6 자원 기준선과 메모리 차이 근거 확보.** 기존 원시 증거가 있으면 먼저 재사용한다. 부족할 때 같은 장치·셸·글꼴·테마·행/열·샘플 준비에서 공개 exe와 확인된 로컬 exe의 1/4세션 유휴 30초를 각 조건 한 번 측정한다.
   완료 조건: 각 조건의 최대 working set/평균 CPU 판정과 앱·GPU·셸 자원이 분리된 결과가 남고, 공개/로컬 차이를 특정 할당·빌드/환경 차이로 설명하거나 미확정 가설 하나와 다음 관측을 명시한다. alpha.6 결과는 해당 SHA에만 적용한다.
2. **P2 — 출력 부하에서 비용 위치와 입력 보존 확인.** 유한한 동일 출력 fixture의 바이트 수·속도·최종 VT 기대값을 정하고 선택/비선택/숨김과 1/4세션을 구분한다. reader/parse/VT 잠금 대기·UI 구성·present 및 앱/GPU/셸 자원의 시간 관계를 관측한다.
   완료 조건: raw trace·자원 시계열·출력/입력 보존 판정·비용 상위 경로가 남는다. 측정 근거가 있는 경우에만 snapshot 이중 버퍼/행 damage 등의 변경 하나를 제안하고, 변경 후 같은 조건 비교 한 번으로 효과와 기능 회귀를 판정한다. 부하 목표 수치는 근거 없이 만들지 않는다.
3. **P3 — 실제 GUI 지연과 동일 환경 기준 터미널 비교.** 입력 사건과 VT revision/프레임/present를 동일 QPC 시간축으로 연결하고 실제 표시를 확인할 수단을 검증한다. ≥100 입력·클릭 표본의 p95와 누락 수를 구한다. Windows Terminal도 같은 Bash/초기화/폴더/도구/데이터/행·열에서 작업 ≥10회 비교한다.
   완료 조건: UI CPU/present/입력→pixel 각각의 원시 표본과 p95·목표 판정, 기준 터미널 차이 표가 남는다. present만 확보하면 pixel은 미검증으로 표시한다. NTFS/WSL `/home`/WSL NTFS는 별도 조건이며 cold start·캐시·데이터 준비를 작업 시간에서 분리한다.

## 한 번의 계측에 남겨야 할 산출물과 반복 중단 조건

- 고유 폴더 `artifacts/perf-<UTC>-<GUID>/`에 `manifest.json`(exe SHA·소스·셸/DLL·OS/GPU/driver·논리 CPU·전원·DPI·폰트/테마·그리드·세션·데이터/IPC 경로·PID 트리·명령/fixture 해시·목표·표본 수)을 작성한다.
- 같은 폴더에 `resources.csv`(QPC/시각·PID·누적 CPU·working set/private bytes·GPU/셸/WSL 별도 표본), 해당 `idle.json`/`metrics.json`/`verification.json`, ETL/지연 표본, 필요한 캡처를 보존한다. `result.md`에는 조건별 판정·측정 한계·실패·다음 한 작업과 증거 SHA256을 남긴다. 미측정 필드는 0 대신 미측정으로 표시한다.
- 기존 도구의 집계만으로 원시 시계열이나 end-to-end 측정을 확보했다고 표시하지 않는다. 새 계측 수단은 시험 계획/산출물 형식을 먼저 정하고, 같은 실행에서 메타데이터·실패 기록까지 저장한다.
- 같은 SHA·환경·조건의 유효한 통과 결과는 재사용한다. 통계 계획의 ≥100/≥10회 표본은 한 측정 세트이며 사후 재실행과 구분한다. 재시험은 변경된 가설/코드/환경/무효 표본의 이유를 기록한 경우만 수행한다.
- 새 근거가 없거나 수단이 준비되지 않아 판정 불가이면 제한과 다음 관측 하나를 기록하고 종료한다. 실패를 덮어쓰거나 전체 GUI/일반 빌드를 반복하지 않는다.

## 담당과 공유 파일 경계

성능 담당: `scripts/measure-idle.ps1` 및 자원·지연 계측 계획/산출물. 기능·디자인 통합 담당: 셸/입력/resize/스크롤 호환성, IME·DPI·테마·시각 회귀의 인수 판정.
`src/app.rs`, `src/terminal.rs`, `src/pty.rs`, `src/native.rs`, `src/settings.rs`, `scripts/verify-ux.ps1`는 공유 파일이다. 재개 후 변경은 한 세션씩 조정하고 통합 담당자가 diff와 영향 범위를 확인한다. GUI는 같은 데스크톱에서 동시에 실행하지 않는다.
기존 사용자 앱·설정·초기화/인증·전역 Git/WSL은 유지하고 시험 소유 PID만 관리한다. 릴리스·WinGet·PR은 영역 세션이 독립 수행하지 않는다. 이번 세션의 쓰기 범위는 이 파일뿐이다.

## 새 세션 시작 프롬프트

> MiniTerm 성능 세션을 이어받아라. `docs/SESSION_HANDOFF.md`와 `docs/areas/PERFORMANCE.md`를 먼저 읽고 명시된 영역 자료만 확인하라. 구현 목표는 일시중지이므로 재개 지시 전에는 코드·설정·Git·실행 앱을 바꾸거나 빌드/시험/GUI/배포를 실행하지 마라. 공개 alpha.3 실패·로컬 185MB 통과·alpha.6 미검증과 PTY/UI CPU/present/pixel을 구분하라. 기능·디자인 통합 세션과 공유 파일을 조정하고, 재개하면 P1부터 기존 증거를 재사용해 고유 폴더에 한 번의 완결된 계측 결과를 남겨라. 같은 조건을 이유 없이 반복하지 말고 판정·한계·다음 한 작업을 기록하라.
