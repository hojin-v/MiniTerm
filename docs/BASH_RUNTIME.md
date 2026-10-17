# 전용 Bash 런타임 적용 진행

> 2026-10-17 배포 후속: alpha.8은 PortableGit 2.56.0(2)와 최신 공식 MSYS 소스에 입력 수정을 적용한 전용 런타임을 포함한다. 소스 고정·빌드·배포 검사 기준은 [BUNDLED_RUNTIME.md](BUNDLED_RUNTIME.md)를 따른다. 아래 Git 2.50.1 후보와 비번들 설명은 이전 실험의 이력이다.

> 2026-10-14 Windows Codex 후속: 공유 서버를 프런트엔드보다 먼저 준비하고 공식 exec-server 기반 Git Bash 기본 환경을 제공한다. 전용 MSYS 런타임의 배포와 별개인 변경이다. [실행 순서·등록 범위·알림 한계](../feature/CODEX_STARTUP.md), [최신 검증](../feature/CODEX_RECOVERY_HANDOFF.md)을 따른다.

2026-10-12. 제품의 기본 프로필에 전용 Bash 자동 선택 경로를 구현하고 개발용 구성에서 검증했다. 공개 alpha.6에는 선택·검증 코드가 포함됐지만 수정 MSYS DLL·PortableGit은 번들하지 않는다. 현재 후보는 입력 누락 조사에 사용한 Git 2.50.1이며 공개 배포 승인 상태가 아니다. 현재 기준과 후속 Unicode 경로 검증은 [SESSION_HANDOFF.md](SESSION_HANDOFF.md)를 참고한다.

## 실행 경로

설정에서 Bash 실행 파일을 지정하면 그 경로를 사용한다. 경로를 비우면 앱 실행 파일 옆의 `runtime/git`을 먼저 확인하고, 해당 폴더가 없으면 설치된 Git을 탐색한다. 전용 폴더가 있으면 `miniterm-runtime.json`의 스키마·x64 구성과 Bash/MSYS DLL의 SHA256, 프로필·Git 실행 파일을 확인한다. 손상되거나 일부 파일이 누락된 구성은 오류로 표시한다.

Windows CNG로 파일을 64KiB 단위로 읽어 해시를 계산한다. DLL 전체를 앱 메모리에 적재하거나 검증을 위해 외부 프로세스를 실행하지 않는다. 이 검사는 배포 구성과 파일의 일치 확인이며, 코드 서명 또는 배포 출처 인증을 대신하지 않는다.

## 개발용 준비와 검증

[`runtime/BASH_RUNTIME.json`](../runtime/BASH_RUNTIME.json)은 공식 PortableGit·7-Zip 다운로드와 검증한 MSYS v3 DLL·패치의 SHA256을 고정한다. 준비 스크립트는 작업 공간의 `artifacts` 아래 새 폴더만 사용하며, 공식 압축본을 검사하고 추출한다. 설치된 Git이나 개인 설정을 복사하지 않는다. 원본 PortableGit 고지와 수정한 MSYS 소스의 라이선스·출처 기록을 함께 둔다.

```powershell
$candidate = './artifacts/msys-runtime-build-fa913284-a51e-4702-9f39-0d82eb94a89f/patched-msys-2.0.dll'
$application = ./scripts/prepare-bash-runtime.ps1 -PatchedDll $candidate
./scripts/verify-bash-runtime.ps1 -ApplicationDirectory $application
```

준비 도구는 SDK에서 DLL을 자동 빌드하거나 공개하지 않는다. 후보 빌드·입력/신호 검사 기록은 [MSYS 검증](patches/MSYS_RUNTIME_VALIDATION.md)에 있다. 배포 전 최신 지원 Git에 대한 재빌드·호환성 검사와 대응 소스 제공을 완료해야 한다.

검증 도구는 테스트 실행 파일을 실제 앱 폴더 구조에 복사해 `ShellProfile::default()`를 사용한다. Bash 경로를 환경 변수나 설정으로 지정하지 않는다. 기본 선택·명시적 사용자 경로 보존 1개, 기존 PTY/프롬프트/리사이즈 8개, Windows 도구 1개, Codex 알림 연결 1개가 통과했다. 기록은 `artifacts/managed-bash-009ad9b80aa640228fe283771583cba3/verification-6514718e13d04806b29bd295c7466c86`이다. 실제 계정/모델이나 알림 클릭의 통과를 의미하지 않는다.

같은 개발 실행 파일 SHA256 `fa32290185b3d2e71bdc9730cf1184ec38b2c5411e927044a1c1586152187678`을 전용 폴더에 두고 GUI 4세션을 검증했다. 설정의 Bash 경로를 비운 채 실제 자식 프로세스가 전용 Bash인지 확인했다. 가용 입력 3행·하단 정렬, 요약/보고, 툴팁 하나, 테마, 확장/축소와 소유 Bash 종료가 통과했다. `verify-ux.ps1 -ExpectedBashPath`는 관찰할 경로만 지정하며 설정의 Bash 경로를 변경하지 않는다.

GUI 기록은 `artifacts/ux-78519338-e367-4fa5-9222-8da7053b5e48`다. 30.50초 유휴 최대 working set 185,511,936bytes·CPU 0.006%를 기록했다. 첫 GUI 시도 `ux-4de60ae8…`는 툴팁 관찰 실패로 중단됐고 성공으로 세지 않는다. 실패 시 포인터·위젯 좌표를 남기도록 진단을 추가한 후 같은 바이너리의 재검사는 통과했지만 최초 실패 원인은 미확정이다. 사람의 입력/호버·IME, 출력 부하, GPU와 입력→픽셀은 별도 검증이 남는다. 공개 alpha.3의 210MB 초과 기록은 [성능 기록](PERFORMANCE.md)에 유지한다.

## 공개 배포 전 남은 작업

- 최신 지원 Git 환경에서 패치 재빌드와 기본 경로·여러 세션·부하·잡 제어 검증
- 수정 MSYS와 포함한 각 구성요소의 대응 소스·빌드 절차·원본 고지 제공
- NSIS/ZIP 포함, 설치·업데이트·제거·복구 및 깨끗한 환경 검증
- 실제 키보드·IME·pager·mouse-report TUI와 사용자 계정의 에이전트 검증

현재 준비 기록은 `publicPackagingApproved: false`이며 일반 배포 스크립트는 이 후보를 포함하지 않는다. DLL과 SDK·개발 GUI 캡처는 Git 업로드에서 제외한다.
