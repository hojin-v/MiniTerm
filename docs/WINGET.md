# WinGet 공식 카탈로그 배포

사용자에게 안내하는 설치 명령은 **`winget install miniterm`**이다. 매니페스트의 이름은 `MiniTerm`, 별칭은 `miniterm`으로 설정했다. 등록용 패키지 ID는 `Hnode.MiniTerm`이며 일반 설치 명령에는 입력할 필요가 없다. 현재 `0.1.0-alpha.3`의 제출용 매니페스트를 준비했다. Microsoft `winget-pkgs`의 등록 PR이 병합되고 카탈로그에 반영되기 전에는 일반 검색·설치를 제공한다고 표시하지 않는다. 매니페스트를 이 저장소에 올리는 것만으로 공식 등록이 완료되지는 않는다.

등록 ID는 `Hnode.MiniTerm`, 매니페스트와 설치 프로그램의 게시자는 `Hnode`다. 기본 검색은 대소문자를 구분하지 않으므로 `winget install hnode.miniterm`도 같은 패키지를 선택한다.

2026-10-12에 GitHub CLI 인증과 [등록 PR #450836](https://github.com/microsoft/winget-pkgs/pull/450836) 제출을 확인했다. 이후 게시자 Hnode와 입력 하단 정렬·메모리 수정본 alpha.2로 갱신을 준비한다. 계정 소유자의 CLA 동의는 Microsoft가 확인했다. 초기 제출의 PR 형식·매니페스트·URL·도메인·카탈로그 내용·설치본 스캔은 통과했으나, 새 버전은 새 설치 파일 해시로 다시 검증한다. 정책의 수동 검토, Microsoft 승인·병합·카탈로그 반영이 남아 있다. 최신 결과는 PR의 검사·댓글에서 확인한다.

## 등록 후 설치·업데이트·제거

```powershell
winget show miniterm
winget install miniterm
winget upgrade miniterm
winget uninstall miniterm
```

일반 설치 안내에는 이름·별칭 `miniterm`을 사용한다. 기본 검색은 대소문자를 구분하지 않으므로 `winget install MiniTerm`도 사용할 수 있다. `--id`는 검색 필드를 ID로 제한하고, `--exact`는 정확히 일치하는 결과만 선택하며, `--source winget`은 검색할 소스를 제한하는 선택 옵션이다. 검색 결과가 모호할 때 또는 배포 검증을 자동화할 때 추가한다. [Microsoft 설치 명령 문서](https://learn.microsoft.com/en-us/windows/package-manager/winget/install)를 참고한다.

Windows 11 x64가 필요하다. MiniTerm은 사용자별 설치이고, Git·Visual C++ v14 x64 런타임은 패키지 의존성으로 선언했다. 의존성 설치에서 관리자 권한이 필요할 수 있다. 업데이트·제거 전에는 작업을 마치고 MiniTerm을 종료한다. 실행 중인 세션을 강제로 종료하는 설치 방식은 사용하지 않는다. 알파의 알려진 문제는 [현재 상태](STATUS.md)에 유지한다.

## 제출 파일과 검증

[`winget/manifests/h/Hnode/MiniTerm/0.1.0-alpha.3`](../winget/manifests/h/Hnode/MiniTerm/0.1.0-alpha.3)에 버전·설치·영어·한국어 매니페스트 네 개를 보관한다. 커뮤니티 저장소에는 이 경로에서 `winget/` 접두사를 뺀 `manifests/h/Hnode/MiniTerm/0.1.0-alpha.3`로 제출한다.

- 스키마 1.12.0, `nullsoft`, x64, 사용자 범위, Windows 11 최소 빌드
- 공개된 버전별 NSIS 설치 URL과 실제 설치 파일 SHA256
- 실제 설치 프로그램의 이름·게시자·버전·제거 레지스트리 키
- `/S` 무인 설치, 업데이트 중 실행 상태와 런타임 오류의 종료 코드
- `Git.Git`, `Microsoft.VCRedist.2015+.x64` 의존성

```powershell
./scripts/verify-winget.ps1 -VerifyInstaller
./scripts/prepare-winget.ps1 -ReleaseTag v0.1.0-alpha.3
./scripts/submit-winget.ps1 -ValidateOnly
```

첫 명령은 매니페스트와 다운로드 해시를 확인한다. 두 번째는 이미 공개된 릴리스에서 제출용 파일을 `artifacts/winget/`에 만든다. 검사 목적으로 개발 장치에 앱을 재설치하거나 WinGet 설정을 변경하지 않는다. 별도 `WinGet package validation` CI는 임시 Windows 장치에서 실제 `winget install --manifest`와 제거·설정 보존을 확인한다. 이 CI의 로컬 매니페스트 기능 활성화는 일반 사용자 설치에 필요하지 않다.

미등록 로컬 매니페스트와 공식 카탈로그는 WinGet의 신뢰 처리 경로가 다르다. Microsoft의 [다운로드 처리 구현](https://github.com/microsoft/winget-cli/blob/master/src/AppInstallerCLICore/Workflows/DownloadFlow.cpp)은 신뢰하는 소스의 해시 일치 파일과 로컬 파일을 구분한다. CI의 보안 확인 대기를 카탈로그 설치 성공으로 표시하지 않는다. Microsoft의 검증과 실제 카탈로그 설치 확인이 남아 있다.

## 공식 등록

현재 완료한 작업과 남은 작업은 다음과 같다.

- [x] GitHub Release에 버전이 고정된 설치본과 해시 공개
- [x] 매니페스트 네 개 작성 및 `winget validate` 통과
- [x] 공개 설치본의 독립 무인 설치·앱 로딩·제거·설정 보존 검사
- [x] 등록 PR 본문과 제출 도구 준비
- [x] 계정 소유자의 GitHub CLI 인증
- [x] 미완료 검증을 명시한 Draft PR 제출: [#450836](https://github.com/microsoft/winget-pkgs/pull/450836)
- [ ] 갱신한 제출 파일 네 개가 현재 매니페스트와 일치하는지 확인
- [ ] 갱신한 매니페스트와 새 공개 설치 파일의 해시·설치 검증
- [x] 계정 소유자의 CLA 동의 댓글과 Microsoft `license/cla` 검사 통과 확인
- [ ] Windows Sandbox 등 대화형 환경 또는 Microsoft의 PR 설치 검증으로 WinGet 설치를 확인하고 지적된 오류 수정
- [ ] 설치 검증과 저자가 수정할 오류를 해결한 뒤 일반 PR 전환
- [ ] 정책 검사의 Microsoft 수동 검토 완료
- [ ] Microsoft 승인·병합 및 카탈로그 반영
- [ ] 이름·별칭으로 공식 패키지가 검색되는지 확인하고 설치·업데이트 검증 및 README의 등록 상태 갱신

GitHub CLI 인증과 Draft 제출은 이 작업 공간에서 완료했다. 다른 장치에서 같은 절차를 재현할 때는 아래 인증·제출 명령을 참고한다. 이 작업 공간에는 GitHub CLI를 `.tools/github-cli/bin/gh.exe`에 준비했다. 시스템에 GitHub CLI를 설치한 경우 같은 옵션으로 `gh auth login`을 사용할 수 있다.

```powershell
./.tools/github-cli/bin/gh.exe auth login --hostname github.com --git-protocol ssh --web --skip-ssh-key
```

SSH로 이 저장소를 push할 수 있는 것과 GitHub API로 fork·PR을 만드는 인증은 별개다. 이 명령은 새 SSH 키 등록 단계를 건너뛰고 GitHub CLI의 브라우저 인증을 진행한다. 토큰을 대화나 저장소 파일에 붙여 넣지 않는다. [GitHub CLI 인증 문서](https://cli.github.com/manual/gh_auth_login)를 참고한다.

독립 설치 검사는 통과했지만 로컬 WinGet 검증은 미완료인 상태에서 제출 도구의 Draft 경로를 사용했다. 같은 패키지·버전의 PR을 중복 생성하지 않고 기존 PR의 검증 결과를 확인한다.

```powershell
./scripts/submit-winget.ps1 -Draft
```

[Microsoft 봇의 CLA 요청 댓글](https://github.com/microsoft/winget-pkgs/pull/450836#issuecomment-6115485956)에 원문과 동의 방법이 표시되어 있다. 계정 소유자의 동의 댓글을 Microsoft가 처리해 `license/cla` 검사가 통과했고 `Needs-CLA`가 해제됐다. PR 본문의 CLA 항목도 실제 검사 결과에 맞춰 완료로 표시했다. 다른 제출에서도 계정 소유자가 약관과 제출물의 권리 관계를 검토해 직접 동의한다.

등록 도구를 사용하지 않고 직접 제출하는 절차는 다음과 같다.

1. 검증과 CI 결과를 확인하고 같은 패키지·버전의 열린 PR이 없는지 확인한다.
2. GitHub에 로그인한 계정으로 `microsoft/winget-pkgs`를 fork하고 네 파일만 새 브랜치에 추가한다.
3. 제목 `New package: Hnode.MiniTerm version 0.1.0-alpha.3`로 PR을 제출한다. [PR 본문](WINGET_PR.md)의 검사 항목은 실제 결과에 맞게 표시한다.
4. Microsoft 봇과 검토자의 결과를 확인한다. CLA 동의가 요청되면 계정 소유자가 직접 확인하고 서명한다. 인증 토큰을 소스·로그·PR에 넣지 않는다.
5. 병합과 카탈로그 반영 후 `winget show miniterm`과 `winget show MiniTerm`이 공식 패키지로 연결되는지 확인한다. `winget install miniterm`으로 설치하고 업데이트도 확인한 뒤 README의 상태를 갱신한다. 자동 검사에서 등록 ID와 소스를 엄격하게 확인할 때는 `winget show --id Hnode.MiniTerm --exact --source winget`을 사용한다.

GitHub CLI 로그인과 WinGet CI 성공 후 `./scripts/submit-winget.ps1`로 제출할 수 있다. 도구는 열린 중복 PR을 확인하고, 계정의 fork에 새 브랜치를 만들어 네 파일만 제출한다. 기존 fork의 브랜치를 덮어쓰지 않는다. `-ValidateOnly`는 제출 내용을 확인하며 GitHub 인증·fork·PR 생성을 수행하지 않는다.

독립 설치 검사가 통과했지만 로컬 WinGet 검증이 미완료이면 `./scripts/submit-winget.ps1 -Draft`로 검토용 PR을 준비할 수 있다. 이 경우 검사한 매니페스트의 해시를 대조하고, WinGet 설치 항목은 미완료로 표시하며 SmartScreen 대기와 검증 범위를 본문에 적는다. Draft 생성은 공식 카탈로그 등록 완료가 아니다. WinGet 설치 검증과 저자가 수정할 오류를 해결한 뒤 일반 PR로 전환한다. 정책의 수동 검토, Microsoft 승인·병합·카탈로그 반영은 별도 단계다.

후속 버전은 새 GitHub Release를 먼저 공개하고 `prepare-winget.ps1`로 새 버전의 해시를 확인한 뒤 별도 업데이트 PR로 제출한다. 이미 게시한 설치 파일·태그·해시를 바꾸지 않는다. 공개 릴리스와 공식 카탈로그는 별도의 배포 단계다.

[Microsoft 등록 절차](https://learn.microsoft.com/en-us/windows/package-manager/package/repository) · [커뮤니티 정책](https://github.com/microsoft/winget-pkgs/blob/master/doc/Policies.md)
