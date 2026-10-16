# WinGet 공식 카탈로그 배포

> 2026-10-16: GitHub의 다음 버전은 alpha.7이다. Microsoft PR #450836은 조회 시 OPEN·미병합이며 아래 alpha.6 매니페스트를 유지한다. 이번 GitHub 배포에서는 WinGet PR이나 기존 버전 자산을 변경하지 않는다.

2026-10-12. 패키지 ID는 `Hnode.MiniTerm`, 게시자는 `Hnode`, 제출 버전은 **0.1.0-alpha.6**이다. [기존 PR #450836](https://github.com/microsoft/winget-pkgs/pull/450836)을 갱신했으며 새 PR을 중복 생성하지 않았다. Microsoft 승인·병합·카탈로그 반영이 끝나기 전에는 일반 검색·설치가 완료됐다고 표시하지 않는다.

현재 직접 설치는 [GitHub alpha.6](https://github.com/hojin-v/MiniTerm/releases/tag/v0.1.0-alpha.6)의 설치본을 사용한다. 코드 서명과 평판은 별도이며 사용자 장치의 보안 설정을 변경하지 않는다.

## 완료한 검증

- [태그 CI](https://github.com/hojin-v/MiniTerm/actions/runs/38207963396): NSIS 설치·앱 로딩·무인 제거 명령 등록·제거·설정 보존 및 GitHub 게시 성공.
- [WinGet CI](https://github.com/hojin-v/MiniTerm/actions/runs/38208455393): 같은 alpha.6 매니페스트의 공개 설치본 SHA256, 실제 `winget install --manifest`, 앱 로딩, `winget uninstall --manifest`, 파일·레지스트리 삭제와 설정 보존 성공.
- 설치본 SHA256: `5e1a80beff9a6af201876a0ee554977d5df19dd7b3694391602a93eb52fad4fb`.
- CI에서 SmartScreen과 Attachment Manager의 비대화형 대기를 구분했다. 일회용 GitHub 러너에서 검사 동안만 관련 프롬프트 설정을 조정하고 `finally`에서 복원한다. 해시 검증과 Defender는 유지한다. 이 결과는 보안 평판 검증이 아니다.
- alpha.6는 `QuietUninstallString`에 `/S`를 등록한다. 이전 설치본의 무인 제거 결함을 수정했으며 실제 제거 완료를 확인한다.

이전 `Policy-Test-2.7`은 영문 설명의 오탐이었다. 의미를 보존해 `structured plan progress`로 수정하자 alpha.3의 정책 검사와 전체 Microsoft 자동 검증이 통과했다. alpha.6의 설치 파일·해시로 PR을 갱신했으므로 Microsoft 검증은 새 버전에서 다시 진행한다. 앞선 버전의 통과를 새 버전의 통과로 표시하지 않는다. [공식 정책 검증 안내](https://github.com/microsoft/winget-pkgs/blob/master/doc/ValidationFailureGuide.md#content-policy-labels)를 참고한다.

## 등록 후 사용

```powershell
winget install miniterm
winget upgrade miniterm
winget uninstall miniterm
```

`winget install hnode.miniterm`도 같은 ID를 선택한다. Windows 11 x64, Git for Windows와 Microsoft Visual C++ v14 x64 런타임이 필요하며 두 패키지는 매니페스트 의존성으로 선언했다. 업데이트·제거 전 실행 중인 MiniTerm 작업을 마치고 종료한다.

## 관리할 파일과 절차

매니페스트는 [`winget/manifests/h/Hnode/MiniTerm/0.1.0-alpha.6`](../winget/manifests/h/Hnode/MiniTerm/0.1.0-alpha.6)의 네 YAML이다. Microsoft 저장소에는 `winget/` 접두사를 제외한 동일 경로와 내용으로 제출한다. 버전별 설치 파일과 SHA256을 고정하며 이전 Release 자산을 교체하지 않는다.

```powershell
./scripts/verify-winget.ps1 -VerifyInstaller
./scripts/prepare-winget.ps1 -ReleaseTag v0.1.0-alpha.6
./scripts/submit-winget.ps1 -ValidateOnly
```

설치 시험은 일회용 GitHub 러너에서만 실행한다. 일반 사용자나 개발 장치에서 `verify-winget-headless.ps1`은 실행을 거부한다. 정식 PR 제출은 동일 매니페스트의 WinGet CI 성공 증거를 확인해야 한다. 기존 PR이 있으면 그 PR을 갱신하며 검증이 끝난 다음 검토 상태로 전환한다.

현재 사용자가 별도로 서명하거나 동의 댓글을 남길 필요는 없다. CLA는 통과했다. Microsoft가 추가 자료를 요청하거나 승인·병합할 단계는 저장소 관리자의 권한이며 자동으로 대신 완료할 수 없다.
