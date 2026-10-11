# 테스터 배포 방식

배포 채널은 GitHub Releases의 **prerelease**다. `main`은 소스, Git 태그는 배포할 소스의 고정 지점이다. 설치 파일은 Git에 커밋하지 않고 Release 자산으로 제공한다. 현재는 공개 저장소이므로 게시된 prerelease도 공개된다. 테스터에게 메시지를 자동 발송하지 않는다.

공식 WinGet 카탈로그도 설치 채널로 준비한다. `hojin-v.MiniTerm`의 등록 매니페스트는 공개된 같은 설치본과 해시를 사용한다. Microsoft 저장소의 PR 검증·병합·카탈로그 반영이 완료되어야 일반 `winget install`이 가능하다. [WinGet 등록 절차](WINGET.md)를 따른다.

## 제공 파일

| 파일 | 목적 |
| --- | --- |
| `MiniTerm-<release>-windows-x64-setup.exe` | 사용자별 NSIS 설치본 |
| `MiniTerm-<release>-windows-x64.zip` | 설치 없이 폴더에서 실행하는 압축형 |
| `SHA256SUMS` | 설치본·ZIP·실행 파일·배포 문서의 SHA256 |
| `BUILD_INFO.json` | 앱/배포 버전, 소스 커밋, Rust·대상·서명·런타임 포함 여부 |
| `THIRD_PARTY_NOTICES.html` | Rust 의존성 고지 |
| `TESTER_GUIDE.md`, `RELEASE_NOTES.md`, `STATUS.md` | 사용 순서, 알려진 문제, 검증 범위 |
| `COMPATIBILITY.json` | 해당 CI 장치의 실제 기본 Git Bash 검사 결과 |

압축형에도 문서·라이선스·빌드 정보와 파일별 해시를 넣는다. 앱은 Git Bash·에이전트·AI Battery를 번들하지 않는다. MSYS v3 실험 DLL도 이번 배포에 포함하지 않는다. 이들의 통합·복구가 검증되기 전 사용자 Git을 덮어쓰는 방식으로 배포하지 않는다.

## 로컬 준비

```powershell
./scripts/test.ps1
./scripts/cargo.ps1 -CargoArguments @('fmt','--all','--','--check')
./scripts/cargo.ps1 -CargoArguments @('clippy','--locked','--all-targets','--','-D','warnings')
./scripts/verify-git-compatibility.ps1 -AllowKnownFailures
./scripts/build.ps1
./scripts/package.ps1 -SkipBuild -ReleaseTag v0.1.0-alpha.1
```

마지막 명령은 문서·체크섬·ZIP·설치본을 생성한다. `-SkipBuild`는 먼저 빌드한 실행 파일이 있을 때만 사용한다. 버전 태그의 숫자 부분은 Cargo 버전과 일치해야 한다. `BUILD_INFO.json`에는 소스 커밋과 작업 트리 변경 여부를 기록한다. 게시할 때는 깨끗한 커밋의 결과를 사용한다.

기본 Git Bash의 알려진 네 리사이즈 회귀만 알파 배포에서 허용한다. 관리 프롬프트, Bash raw typeahead, production Session 가로/세로·세로 전용 검사다. Git 2.55 CI에서도 raw resize의 `printf`→`rintf`를 확인해 같은 입력 누락으로 분류했다. 일반 입력·명령 왕복을 포함한 다른 실패·테스트 미실행·빌드 실패는 게시를 막는다. 호환성 기록에는 실제 전체 통과 여부와 허용된 실패를 각각 기록한다. MSYS 후보의 통과를 기본 프로필 통과로 대체하지 않는다.

## GitHub 게시

1. 소스·README·릴리스 노트와 `.gitignore`를 점검하고 `main`에 푸시한다.
2. 로직·fmt·Clippy·IPC·Git Bash 호환성·패키지 생성 검사와 업로드 대상 검사를 확인한다.
3. 다음 태그를 같은 커밋에 만들고 푸시한다.

   ```bash
   git tag -a v0.1.0-alpha.1 -m "MiniTerm tester alpha 1"
   git push origin v0.1.0-alpha.1
   ```

4. Windows Actions가 검사·패키징 후 해당 태그의 prerelease와 자산을 생성한다. 작업이 실패하면 배포를 완료했다고 표시하지 않는다. 자산이 모두 올라간 후에만 draft를 공개 prerelease로 바꾼다.
5. 테스터에게 `https://github.com/hojin-v/MiniTerm/releases/tag/v0.1.0-alpha.1`과 테스터 안내 링크를 공유한다. 다운로드·실행 여부와 피드백은 직접 확인한다.

현재 workflow는 `v*-alpha.*` 태그만 게시한다. 안정판을 자동 게시하지 않는다. Actions의 저장소 한정 `GITHUB_TOKEN`과 게시 단계의 `contents: write`를 사용하며 별도 PAT를 저장소에 넣지 않는다. 외부 Actions는 커밋 SHA로 고정한다. [GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases), [Actions 인증](https://docs.github.com/en/actions/tutorials/authenticate-with-github_token)을 참고한다.

단위·정적 검사 및 IPC는 필수다. 기본 Git의 알려진 입력 실패·메모리 초과·실기 검증·서명 부재는 알파 릴리스 노트에 유지한다. 깨끗한 설치·업데이트/복구·다른 장치·실제 계정/IME/알림·출력 부하와 성능 기준을 통과하기 전 안정판 또는 완성된 MVP로 승격하지 않는다.
