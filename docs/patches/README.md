# 격리 빌드에서 검증한 MSYS 수정 후보

2026-10-11. `msys-resize-signal-after-discard.patch` v3은 기록한 Git for Windows runtime `7674c51e18b9323c0072c8299c9356a7d9511a4e`의 `console.cc`만 변경한다. 자체 SDK에서 기준 DLL과 후보를 컴파일하고 별도 Git 복사본에서 검증했다. **사용자 Git·MiniTerm 배포·실행 중인 사용자 세션에는 적용하지 않았다.** 기본 제품 프로필의 입력 누락이 해결됐다고 표시하지 않는다.

수정은 크기 이벤트를 입력 잠금 안에서 소비한 다음 SIGWINCH를 보내는 순서 변경과, 조회·출력이 갱신하는 `con.dwWinSize`에서 마지막 보고 크기를 분리하는 것이다. 기존 공유 `tty_min::winsize`를 입력 잠금 아래에서 비교·갱신하고 첫 콘솔 열기에서 초기화한다. 공유 구조의 크기와 DLL export는 바꾸지 않는다. 입력 지연·재전송·문자 보정이나 master 비활성화는 추가하지 않았다.

최종 DLL SHA256은 `bd6674212e79904dc20827603f25253b46bd7321a5ce4126889ee3030d27281b`다. 검증 범위·원시 기록·빌드와 재현 절차는 [런타임 검증](MSYS_RUNTIME_VALIDATION.md), 입력 경쟁과 실패한 대조 실험은 [입력 조사](../RESIZE_INPUT_INVESTIGATION.md)에 있다.

- [x] raw/canonical 즉시 resize→입력 각각 1,000회, 한글·이모지와 크기 조회
- [x] 입력 없는 SIGWINCH·siglongjmp 대기 1,000회, 이후 입력과 정상 종료
- [x] 기존 production Session·prompt·ConPTY 8개, Windows bridge와 Codex 알림 연결 각 1개
- [x] 동일 런타임을 쓰는 별도 GUI의 요약·툴팁·진행 보고·더블 클릭 검증
- [ ] 실제 키보드·IME·붙여넣기·mouse-report 소비자·pager, 전체 잡 제어 상태 전이
- [ ] 다른 버전·출력 부하·여러 세션, 제품 구성·배포·업데이트·복구

위 결과는 격리된 후보의 검사다. 기존 기본 Git의 5개 통과·3개 실패 기록과 메모리·실기·실제 계정/모델 검증은 유지한다. 사용자 런타임을 덮어써 시험하지 않는다.
