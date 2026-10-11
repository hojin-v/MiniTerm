# MSYS 후보 v3 검증 기록

2026-10-11. 자체 빌드·복사본에서만 검사했다. DLL SHA256은 `bd6674212e79904dc20827603f25253b46bd7321a5ce4126889ee3030d27281b`다. MiniTerm 기본 제품 프로필과 설치본은 변경하지 않았다.

| 검사 | 결과 | 원시 기록 |
| --- | --- | --- |
| raw 즉시 resize→입력 | 1,000/1,000, 한글·이모지 정확, 크기 불일치 0, SIGWINCH 999회 | `artifacts/experiments/msys-resize-runtime-24c62b0b-318a-4e0e-b3cb-0357f4ecd7bf` |
| canonical 즉시 resize→입력 | 1,000/1,000, 크기 불일치 0, SIGWINCH 999회 | `artifacts/experiments/msys-resize-runtime-f097689d-7cb6-4d32-83e6-3bfc2eb004e0` |
| 입력 없이 신호 대기·핸들러 siglongjmp 후 입력 | 1,000/1,000, 신호·점프 각각 999회, 강제 종료 없음 | `artifacts/experiments/msys-sigjump-5af7bfa8-9921-4288-8994-a8e86e12ad77` |
| 기존 production Session·prompt·ConPTY | 8개 통과, 가로/세로·세로 전용 resize, ANSI 스타일, TTY·Unicode·Ctrl+C/Z | `artifacts/experiments/msys-production-72e1925d-228d-46b9-98a1-9e300c75207b/Pty.log` |
| Windows Node·Codex bridge | 1개 통과, 네이티브 TTY·인자·종료 코드·Codex 버전 | 같은 fixture의 `WindowsBridge.log` |
| Codex 알림 연결 | 1개 통과, 자체 CLI/로컬 SSE/helper/IPC, 기존 정책·콜백 보존 | 같은 fixture의 `CodexNotify.log` |
| native GUI와 실제 Bash 경로 | 요약·활동명·다크/라이트·작은 창·툴팁 1개·진행 보고·더블 클릭 통과, 자체 Bash 종료 | `artifacts/ux-5f655c3b-8f81-4bd9-9f8d-eeb4c6d8248e` |

raw/canonical은 resize와 입력 사이에 지연을 넣지 않는다. production Session의 두 resize 검사도 즉시 입력하며 각 30회다. 기존 다른 Bash/prompt 검사의 대기 시간까지 모두 제거한 검사는 아니다. SIGWINCH 검사는 입력을 보내기 전에 콜백을 기다려 입력 경쟁과 신호 대기 회복을 구분한다. 최초 입력에는 resize가 없어 신호·점프는 각각 999회다. 모든 최종 후보 검사 프로세스는 정상 종료했다.

잡 제어 시험은 Ctrl+Z의 정지를 확인하고 jobs/bg/fg를 실행하지만 전체 상태 전이를 각각 검증하지 않는다. Node/Codex·알림 시험은 실제 계정/모델, Windows toast 클릭이나 GUI 픽셀 지연을 검증하지 않는다. GUI는 고정 VT/보고 데이터이며 실제 모델의 skill 선택이나 사람의 IME/호버 스크롤 검사가 아니다. 현재 SDK/실험 경로는 개발용이며 배포 경로가 아니다.

## 빌드

fixture: `artifacts/msys-runtime-build-fa913284-a51e-4702-9f39-0d82eb94a89f`.

- SDK 커밋: `a889f5bfa362c451584f3fe7388fdd0f45826a1b`
- runtime 소스: `7674c51e18b9323c0072c8299c9356a7d9511a4e`
- 무수정 기준 DLL SHA256: `2b13081b3860029ffeb2c33dbd8dca192be6b31eb7a8ff23cdb0a9d8924ee352`
- 컴파일: GNU GCC 15.3.0, `-O2 -pipe -g0`, 기존 `-Werror`, `make -j2`
- 출력: `build/x86_64-pc-msys/winsup/cygwin/new-msys-2.0.dll`

설치된 Git과 같은 소스 커밋을 기준으로 컴파일한 무수정 DLL에서도 첫 글자 누락이 재현됐다. 설치 DLL과 기준 DLL은 컴파일러·빌드 시점이 달라 SHA가 같지 않다. 버전 생성의 shallow Git describe 경고 뒤에도 정확한 고정 커밋의 버전으로 빌드가 정상 종료했다. [공식 빌드 안내](https://gitforwindows.org/building-msys2-runtime.html), [공식 패키지 빌드 정의](https://github.com/git-for-windows/MSYS2-packages/blob/master/msys2-runtime/PKGBUILD)를 참고했고 예전 문서의 출력 이름을 실제 결과로 사용하지 않았다.

`artifacts/experiments/build-msys-runtime.py`는 자체 SDK·소스·build·TEMP를 확인하고 로그와 활성 PID를 기록한다. 빌드 의존성은 이 SDK에만 설치했다. 사용자 Git DLL SHA는 `b6c7e56b63e6447b204f2feb5295565ad767fbb690b03930f11e37d55c002e8a`로 유지됐다. 실행 직전 DLL SHA를 각 기록에 넣고 검증 중 교체하지 않는다.

## 실행 도구

`tests/bash_conpty.rs`의 모든 Git 검사 프로필이 `MINITERM_GIT_TEST_BASH`를 사용하도록 통일했다. 지정하지 않으면 기존 기본 프로필을 사용한다. `verify-msys-production.py <자체 runtime 경로>`는 이 값을 자식 프로세스에만 전달하고 검사 로그·SHA·범위를 기록한다.

```powershell
python artifacts/experiments/verify-msys-production.py artifacts/msys-runtime-build-fa913284-a51e-4702-9f39-0d82eb94a89f/runtime-patched
```

`verify-msys-resize.cjs <bash.exe> raw|canonical 1000`과 `verify-msys-sigjump.cjs <자체 runtime 경로> 1000`은 각각 별도 자체 ConPTY만 사용한다. 실패 후 정리 전/후 출력과 종료 카운터도 보관해 신호 대기 실패와 입력 잠금·강제 종료를 구분한다.

`scripts/verify-ux.ps1 -BashPath <usr/bin/bash.exe> -SummarySample -ProgressSample`은 자체 설정에 명시적 Bash 경로를 넣고 실제 자식 실행 경로와 DLL SHA를 확인한다. 사용자 창·설정·포커스를 유지하고 테스트 앱/Bash만 닫는다.

이 도구들로 설치된 Git의 원래 실패와 격리 후보의 통과를 구분한다. 기본 제품 적용·별도 런타임 구성/배포/복구·다른 Git/Windows·출력 부하·실제 IME/pager/mouse-report/전체 잡 제어 검증은 남는다.
