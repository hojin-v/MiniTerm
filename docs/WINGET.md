# WinGet 공식 카탈로그 배포

패키지 ID는 **`hojin-v.MiniTerm`**이다. 현재 `0.1.0-alpha.1`의 제출용 매니페스트를 준비했다. Microsoft `winget-pkgs`의 등록 PR이 병합되고 카탈로그에 반영되기 전에는 일반 검색·설치를 제공한다고 표시하지 않는다. 매니페스트를 이 저장소에 올리는 것만으로 공식 등록이 완료되지는 않는다.

## 등록 후 설치·업데이트·제거

```powershell
winget show --id hojin-v.MiniTerm --exact --source winget
winget install --id hojin-v.MiniTerm --exact --source winget
winget upgrade --id hojin-v.MiniTerm --exact --source winget
winget uninstall --id hojin-v.MiniTerm --exact
```

Windows 11 x64가 필요하다. MiniTerm은 사용자별 설치이고, Git·Visual C++ v14 x64 런타임은 패키지 의존성으로 선언했다. 의존성 설치에서 관리자 권한이 필요할 수 있다. 업데이트·제거 전에는 작업을 마치고 MiniTerm을 종료한다. 실행 중인 세션을 강제로 종료하는 설치 방식은 사용하지 않는다. 알파의 알려진 문제는 [현재 상태](STATUS.md)에 유지한다.

## 제출 파일과 검증

[`winget/manifests/h/hojin-v/MiniTerm/0.1.0-alpha.1`](../winget/manifests/h/hojin-v/MiniTerm/0.1.0-alpha.1)에 버전·설치·영어·한국어 매니페스트 네 개를 보관한다. 커뮤니티 저장소에는 이 경로에서 `winget/` 접두사를 뺀 `manifests/h/hojin-v/MiniTerm/0.1.0-alpha.1`로 제출한다.

- 스키마 1.12.0, `nullsoft`, x64, 사용자 범위, Windows 11 최소 빌드
- 공개된 버전별 NSIS 설치 URL과 실제 설치 파일 SHA256
- 실제 설치 프로그램의 이름·게시자·버전·제거 레지스트리 키
- `/S` 무인 설치, 업데이트 중 실행 상태와 런타임 오류의 종료 코드
- `Git.Git`, `Microsoft.VCRedist.2015+.x64` 의존성

```powershell
./scripts/verify-winget.ps1 -VerifyInstaller
./scripts/prepare-winget.ps1 -ReleaseTag v0.1.0-alpha.1
./scripts/submit-winget.ps1 -ValidateOnly
```

첫 명령은 매니페스트와 다운로드 해시를 확인한다. 두 번째는 이미 공개된 릴리스에서 제출용 파일을 `artifacts/winget/`에 만든다. 검사 목적으로 개발 장치에 앱을 재설치하거나 WinGet 설정을 변경하지 않는다. 별도 `WinGet package validation` CI는 임시 Windows 장치에서 실제 `winget install --manifest`와 제거·설정 보존을 확인한다. 이 CI의 로컬 매니페스트 기능 활성화는 일반 사용자 설치에 필요하지 않다.

## 공식 등록

1. 검증과 CI 결과를 확인하고 같은 패키지·버전의 열린 PR이 없는지 확인한다.
2. GitHub에 로그인한 계정으로 `microsoft/winget-pkgs`를 fork하고 네 파일만 새 브랜치에 추가한다.
3. 제목 `New package: hojin-v.MiniTerm version 0.1.0-alpha.1`로 PR을 제출한다. [PR 본문](WINGET_PR.md)의 검사 항목은 실제 결과에 맞게 표시한다.
4. Microsoft 봇과 검토자의 결과를 확인한다. CLA 동의가 요청되면 계정 소유자가 직접 확인하고 서명한다. 인증 토큰을 소스·로그·PR에 넣지 않는다.
5. 병합과 카탈로그 반영 후 `winget show --id hojin-v.MiniTerm --exact --source winget`으로 등록을 확인하고 README의 상태를 갱신한다.

GitHub CLI 로그인과 WinGet CI 성공 후 `./scripts/submit-winget.ps1`로 제출할 수 있다. 도구는 열린 중복 PR을 확인하고, 계정의 fork에 새 브랜치를 만들어 네 파일만 제출한다. 기존 fork의 브랜치를 덮어쓰지 않는다. `-ValidateOnly`는 제출 내용을 확인하며 GitHub 인증·fork·PR 생성을 수행하지 않는다.

후속 버전은 새 GitHub Release를 먼저 공개하고 `prepare-winget.ps1`로 새 버전의 해시를 확인한 뒤 별도 업데이트 PR로 제출한다. 이미 게시한 설치 파일·태그·해시를 바꾸지 않는다. 공개 릴리스와 공식 카탈로그는 별도의 배포 단계다.

[Microsoft 등록 절차](https://learn.microsoft.com/en-us/windows/package-manager/package/repository) · [커뮤니티 정책](https://github.com/microsoft/winget-pkgs/blob/master/doc/Policies.md)
