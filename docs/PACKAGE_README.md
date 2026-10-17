# MiniTerm tester alpha

Windows 11 x64와 [Microsoft Visual C++ v14 x64 런타임](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)이 필요합니다. Bash는 포함되어 있습니다. 설치 파일을 실행하거나 ZIP 전체를 풀고 `miniterm.exe`를 실행하세요. Codex·Claude Code·AI Battery는 별도 설치한 도구를 사용합니다.

설치 중 시작 메뉴·바탕화면 바로가기·Windows 로그인 시 실행을 선택할 수 있습니다. 완료 화면에서 바로 실행할 수 있습니다. 시작과 시험 순서는 [TESTER_GUIDE.md](TESTER_GUIDE.md), 검증 범위는 [RELEASE_NOTES.md](RELEASE_NOTES.md)와 [STATUS.md](STATUS.md), 포함된 Bash의 소스와 빌드는 [BUNDLED_RUNTIME.md](BUNDLED_RUNTIME.md)를 확인하세요.

설정은 `%LOCALAPPDATA%\MiniTerm\settings.json`에 저장됩니다. 압축형과 설치본은 같은 설정과 단일 앱 인스턴스를 사용합니다. 업데이트·제거 전 작업을 마치고 트레이에서 종료하세요.

실행 파일의 소스 커밋·서명 여부는 `BUILD_INFO.json`, 해당 장치의 기본 Bash 검사 결과는 `COMPATIBILITY.json`에 기록합니다. 폴더의 `SHA256SUMS`는 설치 구성 파일의 해시입니다. Release 페이지의 `SHA256SUMS`는 다운로드 자산의 해시입니다.

[Repository](https://github.com/hojin-v/MiniTerm) · [Releases](https://github.com/hojin-v/MiniTerm/releases) · [Issues](https://github.com/hojin-v/MiniTerm/issues/new/choose)
