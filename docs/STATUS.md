# 현재 상태

## v0.1.0-alpha.8 배포 후속 — 2026-10-17

Codex 추가 연결 대기, 설정창 글자 깨짐, 확장 후 정지한 커서의 휠 누락을 수정했다. 설치 과정에 시작 메뉴·바탕화면 바로가기·로그인 시 실행 선택과 명확한 완료 화면을 추가했다. 업데이트 시 선택을 유지하고 앱의 자동 실행 설정과 동기화하며 다른 설정 항목을 보존한다.

전용 PortableGit 2.56.0(2)와 MSYS 입력 경쟁 상태 수정을 배포하도록 구성했다. 사용자 Git과 전역 PATH를 교체하지 않는다. 배포 검사는 수정 런타임의 소스 빌드와 기본 호환성 전체 통과, 실제 설치 옵션·업데이트·제거를 요구한다. 이전의 알려진 실패 허용 결과로 게시하지 않는다. 최종 공개 여부와 정확한 바이너리는 [alpha.8 릴리스](https://github.com/hojin-v/MiniTerm/releases/tag/v0.1.0-alpha.8)의 자산·빌드 기록·체크섬을 따른다.

일반 98개와 Clippy가 통과했다. Codex/설정창 수정의 실제 GUI 검증은 [기록](../feature/CODEX_SETTINGS_FIX_2026-10-16.md)을 따른다. 기본 브리지 활성 중 약 210MB 관측은 1세션 유휴 200MB 목표와 구분한다. 사용자의 정확한 read-only 오류 진입 순서와 10월 13일 메모리 고갈의 최초 트리거는 확인하지 못했으며, 외부 계정·서비스 가용성을 터미널 검증으로 보장하지 않는다. 실제 IME·혼합 DPI·다른 GPU·장기간 외부 계정 대화와 코드 서명은 남는다.

## v0.1.0-alpha.7 — 2026-10-16

Codex 공유 대화·Bash 실행·탭별 알림과 터미널 안정성 변경을 포함하는 테스터 알파다. 실제 공개 파일은 [alpha.7 릴리스](https://github.com/hojin-v/MiniTerm/releases/tag/v0.1.0-alpha.7)의 `BUILD_INFO.json`, `COMPATIBILITY.json`, `SHA256SUMS`로 확인한다. 태그 CI는 빌드·검사·패키징과 일회용 러너의 실제 설치/제거를 확인한 뒤 게시한다.

- 로컬 일반 96개·별도 ConPTY 검사·18만 행 출력·1/4세션 GUI·공유 Codex 0.163.0의 Bash/이력/탭별 알림/재개/R 복구를 통과했다. 실제 계정이나 외부 모델은 호출하지 않았다.
- 로컬 30초 유휴 최대 앱 working set은 1세션 150,097,920bytes, 4세션 152,948,736bytes다. 공개 CI 파일·다른 장치·전력 소모의 성능 판정과 구분한다.
- 바깥 콘솔 커밋 상한·취소 가능한 종료·VT 응답 대기 분리·history/결합 문자 상한·글꼴 파일 매핑·큰 executor 응답 스트리밍을 적용했다. [구현과 근거](https://github.com/hojin-v/MiniTerm/blob/v0.1.0-alpha.7/performance/STABILITY_2026-10-16.md)를 따른다.
- 설치 Git/MSYS의 resize 직후 입력 누락은 **로컬 9개 검사 중 5개 통과·기존 4개 실패**다. 실패를 유지하고 수정 MSYS/PortableGit은 번들하지 않는다. 10월 13일 OOM의 정확한 트리거도 미확정이다.
- 실제 IME·팝업 클릭·장시간 계정 대화·혼합 DPI·다른 GPU/Windows·입력→픽셀·코드 서명 검증은 남는다. 안정판으로 표시하지 않는다.
- WinGet [PR #450836](https://github.com/microsoft/winget-pkgs/pull/450836)은 10월 16일 조회 기준 OPEN·미병합, alpha.6 제출 상태다. 이번 GitHub 릴리스와 카탈로그 등록은 별도다.

새 세션은 [세션 인계 기준](SESSION_HANDOFF.md)을 먼저 읽는다. 아래는 이전 공개 버전의 이력이다.

## 이전 alpha.6 배포 기록

> 2026-10-13 로컬 기능 후속: [Codex 스크롤 수정](../feature/SCROLL.md)의 실행 파일과 검증을 완료했다. 아래 공개 alpha.6·WinGet 상태는 기존 배포 시점의 기록이며 이 로컬 빌드로 바뀌지 않는다.

2026-10-12. **v0.1.0-alpha.6 GitHub 배포 완료, WinGet 공식 등록 검증 중**이다. 제한된 테스터 알파이며 전체 제품의 안정판 검증을 완료한 것은 아니다.

새 작업 세션은 [세션 인계 기준](SESSION_HANDOFF.md)에서 영역별 문서·코드·검증 근거·미해결 항목을 먼저 확인한다. 구현 목표는 일시중지 상태다.

## 배포와 확인 결과

- [공개 alpha.6](https://github.com/hojin-v/MiniTerm/releases/tag/v0.1.0-alpha.6): 설치본·ZIP·실행 파일·문서 등 10개 자산. 다운로드 파일 전체를 GitHub SHA256 digest와 대조하고 실행 파일 로딩을 확인했다.
- 배포 소스: `09d7c01780be6b091597030e07f0474f3b5b144e`. [태그 CI](https://github.com/hojin-v/MiniTerm/actions/runs/38207963396)의 빌드·패키징·설치·제거·설정 보존·게시가 통과했다.
- [WinGet CI](https://github.com/hojin-v/MiniTerm/actions/runs/38208455393): alpha.6 매니페스트·공개 설치본 해시·실제 WinGet 설치·앱 로딩·무인 제거·설정 보존 모두 통과. 매니페스트 소스는 `17cbfc701f5cdd533857e79a54de541a7dc4d4eb`이다.
- Microsoft [PR #450836](https://github.com/microsoft/winget-pkgs/pull/450836)을 같은 alpha.6 파일로 갱신했다. 승인·병합·카탈로그 반영은 별도이며 일반 `winget install miniterm` 가능 여부는 공식 카탈로그를 기준으로 한다.

2026-10-12 20:30 KST 조회에서 Microsoft PR은 OPEN·미병합이며 `Azure-Pipeline-Passed`, `Validation-Completed` 라벨과 CLA 통과를 확인했다. 자동 검증의 통과와 공식 등록 완료는 구분한다.

## 반복 실패 원인과 수정

| 실패 | 확인과 수정 |
| --- | --- |
| Microsoft `Policy-Test-2.7` | 검증 산출물에서 영문 Description의 성인 콘텐츠 오탐 확인. `explicit plan progress`를 `structured plan progress`로 수정한 alpha.3 재검증은 정책 포함 전체 통과. alpha.6는 새 설치본으로 재검증한다. |
| Headless 설치 대기 | SmartScreen과 별도의 Attachment Manager 서명 경고를 각각 확인. 일회용 GitHub 러너에서만 설치 검사 동안 조정하고 원래 레지스트리 값을 복원한다. 해시 검증·Defender는 유지한다. |
| alpha.4 게시 중단 | 앞선 검사에서 실제 기본 데이터 폴더가 생성됨. 테스트별 데이터·IPC 경로를 격리했다. 기존 폴더를 삭제해 통과시키지 않는다. |
| WinGet 제거 미완료 | 설치 프로그램의 `QuietUninstallString` 누락. alpha.6가 `/S` 무인 제거 명령을 등록한다. NSIS 임시 자식의 실제 파일·레지스트리 삭제 완료도 확인한다. |

반복 GUI/런타임 실험은 종료했고 기존 코드·후보·증거는 보존했다. 중복 일반 빌드는 취소하고 배포 태그와 WinGet 검사 결과를 각각 확인했다. alpha.4 실패 태그와 alpha.3/alpha.5 공개 자산을 덮어쓰지 않았다.

## 알려진 제품 제한

- 기본 Git Bash의 리사이즈 직후 입력 누락은 남는다. 공개 alpha.6의 `COMPATIBILITY.json`은 8개 연동 검사 중 **4개 통과·알려진 리사이즈 4개 실패**를 기록한다. 다운로드 파일의 SHA256을 공개 digest와 대조했으며 실제 실패를 유지한다.
- 전용 Bash 선택·SHA256 검증 코드는 포함하지만 **수정 MSYS DLL과 PortableGit은 번들하지 않는다**. 별도 Git for Windows가 필요하다. 개발 후보의 상태는 [BASH_RUNTIME.md](BASH_RUNTIME.md)를 참고한다.
- alpha.3 공개 빌드의 4세션 유휴 메모리는 약 210MB로 200MB 목표를 초과했다. alpha.6의 성능 목표 통과를 주장하지 않는다. [PERFORMANCE.md](PERFORMANCE.md)에 측정 범위를 기록했다.
- 실제 IME·혼합 DPI·다른 장치, 실제 계정/모델과 알림 클릭, WSL·출력 부하·입력 지연·코드 서명 검증이 남는다.
- Headless 설치 기능 검사의 성공은 일반 사용자 장치의 SmartScreen 평판이나 공식 카탈로그 설치 성공을 의미하지 않는다.

[테스터 안내](TESTER_GUIDE.md) · [WinGet 상태](WINGET.md) · [전체 체크리스트](WORK_CHECKLIST.md)
