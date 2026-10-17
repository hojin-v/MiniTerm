# 2026-10-17 실사용·설치 보완

## 확인한 원인과 변경

- 창 크기 변경 직후 입력 누락: MSYS가 resize 레코드를 확인한 뒤 SIGWINCH를 전달하며 입력 잠금을 먼저 놓았다. 다른 콘솔 reader가 레코드를 소비하고 키를 되돌려 놓으면 첫 reader의 뒤늦은 discard가 입력의 첫 글자를 버렸다. resize 레코드를 잠금 안에서 먼저 소비한 뒤 신호를 보낸다. 화면 geometry 캐시와 마지막 신호 크기를 구분한다.
- PortableGit 2.56.0(2), Cygwin 3.6.10 소스 `b11613e4…`, 공식 MSYS 패키지 recipe `8f92182c…`의 61개 패치를 고정해 전용 런타임을 준비했다. 사용자 Git·전역 PATH는 교체하지 않는다. 최신 UCRT64 폴더 구조에 맞게 셸 환경을 선택한다.
- Codex 대기 CPU: Windows accept 소켓이 비차단 listener 설정을 상속했다. read timeout만 지정한 중계가 `WouldBlock`을 쉬지 않고 재시도했다. 실제 GUI에서 대기 30초 동안 전체 CPU 약 12.3%(8개 논리 CPU 중 한 코어 약 98.7%)를 관측했다. 회귀 검사에서 400ms 대기 중 390ms CPU 사용을 재현했다. 연결별 소켓을 blocking 모드로 복구하고 기존 read/write 제한 시간과 shutdown 취소를 유지한다.
- 설치 과정에 시작 메뉴·바탕화면 바로가기·로그인 시 실행 선택을 추가했다. 기본은 시작 메뉴만 켜짐이다. 업데이트 때 선택을 기억하며 앱 설정과 시작 레지스트리를 함께 반영한다. 다른 설정 키를 보존하고 손상되거나 1 MiB가 넘는 설정은 덮어쓰지 않는다. 완료 화면에서 위치·선택 결과·바로 실행을 제공한다.
- 설치·제거는 실행 중인 앱을 종료하지 않는다. 설치 화면에서 정상 종료 후 재시도할 수 있다. 제거는 배포 파일 목록만 지우고 사용자 추가 파일과 설정은 보존한다.

## 검증 기록

- blocking 소켓 수정 전 앱 `ee212803…`, 전용 DLL `ea33feb8…`: 기본 선택 1개, Session/ConPTY 9개, Windows bridge 1개, Codex 알림 1개, 에이전트 Bash 2개가 모두 통과했다. 기존 resize 입력 실패 4건을 허용 없이 통과했다.
- 같은 DLL의 raw·canonical 입력과 SIGWINCH/siglongjmp 각각 1,000회가 통과했다. 스크립트·서버·실행 파일은 시험 소유 경로만 사용했다.
- 실제 기본 AI Battery/Codex 0.163.0 GUI에서 요청·응답, 응답 중 draft 입력·휠·축소/확장, 취소 후 재요청이 통과했다. 마지막 대기 CPU 검사에서 위 busy loop를 발견했다. 앱 최대 working set은 155,226,112bytes인 실행과 211,415,040bytes인 실행이 각각 있었으며, 실행 조건·페이지 상주량 차이의 관측을 임의로 누수로 단정하지 않는다.
- 재개 검사 중 요청 JSON을 쓰는 순간 읽어 발생한 시험 스크립트 파싱 오류는, 원본 기록과 분리한 작은 prompt ledger를 원자적으로 교체하도록 수정했다. 제품 오류로 표시하지 않는다.
- 최종 blocking 수정본과 설치 UI 검증은 진행 중이다. 최종 EXE·CI·게시 기록은 이 문서에 추가한다.

원시 근거: `artifacts/bundled-application/verification-3c1e98c035b24434bd5ac37ce060cd50`, `artifacts/scroll-e6993f9e-a9e8-4360-9f07-d154b4a677fe`, `artifacts/scroll-802fde8f-f51c-447c-9429-84fdf4438296`, 안정성 guard의 `latest-msys-{raw,canonical,sigjump}-1000` 기록. 원시는 Git 제외 대상이다.

설계·소스·배포 게이트는 [전용 런타임](../docs/BUNDLED_RUNTIME.md)을 따른다. Windows accept 속성 상속은 [Microsoft 문서](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-accept)에 설명되어 있다. 사용자에게 나타난 정확한 read-only 상태 진입 순서와 10월 13일 OpenConsole 메모리 고갈의 최초 트리거는 재현하지 못했다. 계정/서비스 가용성과 장기간 외부 모델 대화는 격리된 localhost 시험과 구분한다.
