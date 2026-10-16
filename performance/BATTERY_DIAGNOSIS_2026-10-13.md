# 2026-10-13 배터리 급감 재검증

2026-10-14 00시대 KST에 사용자가 제공한 외부 보고서와 실제 Windows 기록을 대조했다. **18:06의 GPU 충돌 폭증은 과거 보고의 재기록이며, 배터리 급감 구간에서는 초반 Explorer, 후반 WSL의 전력 사용이 확인된다.** 사용자가 관리자 권한으로 수집한 SRUM·WER 메타데이터도 분석했다. WSL이 앱에 귀속된 전력 추정치 중 가장 크지만, 전체 전력의 약 44%는 앱 미귀속이어서 단일 원인으로 확정하지 않는다.

## 배터리와 장치

- Samsung `960XHA`, Windows 11 Home `10.0.26200`, 재부팅 시각 2026-10-13 15:05:56 KST.
- 실제 GPU는 **Intel Arc 130V GPU (8GB)**다. 보고서의 Iris Xe 표기는 실기 조회와 다르다.
- 설치 드라이버 `32.0.101.8626`, 드라이버 날짜 2026-03-11. 날짜만으로 최신 여부·불량 여부를 판정하지 않았다.
- 배터리 설계 용량 73,906mWh, 현재 완충 용량 66,736mWh, 사이클 180. 완충 용량은 설계의 약 90.3%다.
- Windows 배터리 보고서의 18:07:28~21:07:06 구간: 58,728 → 7,341mWh, 약 88% → 11%. **51.387Wh / 2.994시간 = 평균 약 17.2W**. 배터리 급감 자체는 외부 보고서와 일치한다.

## 오류 기록의 발생 시각과 재보고 시각

Application의 Windows Error Reporting 1001은 오류 보고 기록이며, 그 행 수를 새로운 GPU 충돌 횟수로 세지 않는다. 보고 ID·덤프 이름과 별도 `Microsoft-Windows-WerKernel/Operational`의 생성 기록을 함께 확인했다.

| 보고 시각(KST) | LiveKernelEvent 행 수 | 확인한 내용 |
| --- | ---: | --- |
| 17:17~17:18 | 28 | 과거 보고 묶음 |
| 17:56 | 28 | 같은 보고 ID 묶음 |
| 18:06 | 28 | 같은 보고 ID 묶음, 덤프 날짜 2025-12-14~2026-09-27 |
| 21:36~21:37 | 28 | 같은 과거 보고 묶음 |
| 21:39 | 28 | 같은 과거 보고 묶음 |
| 21:46 | 28 | 같은 과거 보고 묶음 |
| 21:57 | 30 | 과거 28행 + 새 보고 ID 1개가 2행으로 기록됨 |

17:56·18:06·21:39의 **28개 ReportId가 정확히 동일**하다. 18:06의 코드 구성은 `193` 22행, `141` 4행, `1a8`·`1b8` 각 1행이다. 이 범위의 WerKernel 로그에는 18:06 새 덤프 생성 기록이 없고, **21:57:17의 생성/제출 성공 두 이벤트(record 179/180)**만 있다. 새 파일명은 `WATCHDOG-20261013-2157.dmp`, WER ReportId는 `2eff310e-f7c2-4708-85ef-15e853996def`, 코드 `193`이다.

관리자 수집의 `Report.wer EventTime`으로도 확인했다. **18:06의 28개 보고 원래 발생 시각은 2025-12-14~2026-09-27**이다. 하루 전체 LiveKernelEvent 814행은 고유 ReportId 29개이며, 원래 발생 시각이 2026-10-13인 고유 보고는 `193` 1개뿐이다. 새 보고의 EventTime은 **21:57:24.9605226 KST**다. 그에 앞선 WerKernel 생성 성공 기록(21:57:17)과 별개 필드이므로 둘을 같은 시각으로 바꿔 쓰지 않는다.

21:57의 새 진단 덤프는 사실이지만 **18~21시 방전보다 뒤의 사건**이며, GPU 리셋·드라이버 원인·MiniTerm 원인을 이 기록만으로 확정하지 않는다. 관리자 수집에서도 해당 날짜의 덤프 파일 목록은 비어 있었다. 파일 삭제·보존 정책·다른 시각 속성 중 어느 이유인지 확정하지 않는다. 덤프 본문이나 할당 스택은 분석하지 않았다.

코드 해석도 외부 보고서와 다르다.

- `0x141`은 디스플레이 엔진 응답 시간 초과의 live dump다. 이번 18:06의 4행은 모두 과거 날짜를 가리킨다. [Microsoft 0x141 문서](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/bug-check-0x141---video-engine-timeout-detected).
- `0x193`의 정확한 이름은 **VIDEO_DXGKRNL_LIVEDUMP**다. Microsoft는 실제 bug check용 코드가 아니라고 명시한다. 외부 보고서의 `VIDEO_DXGKRNL_FATAL_ERROR` 및 커널 치명적 충돌이라는 단정은 수정해야 한다. [Microsoft 0x193 문서](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/bug-check-0x193--video-dxgkrnl-livedump).
- `0x1a8`·`0x1b8`은 검은 화면 진단용 live dump이며, 코드만으로 GPU 타임아웃을 뜻하지 않는다. [Microsoft 0x1a8 문서](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/bug-check-0x1a8--video-dxgkrnl-black-screen-livedump), [0x1b8 문서](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/bug-check-0x1b8--video-miniport-black-screen-livedump).
- Intel 전용 로그의 Error 10은 18:06:19, 19:05:34, 20:18:39, 21:36:44에 있다. 메시지 정의가 로컬에 없고 필드만 확보했으므로 4건을 타임아웃/리셋으로 해석하지 않았다. 일부 시각은 전원·대기 상태 전환과 겹친다.

## WSL·Explorer·개발 작업

사용자가 00:25에 관리자 수집을 완료했다. SRUM CSV의 UTC 시각에 +09:00을 적용하고 `OnBattery=TRUE`, 저녁 방전 구간을 덮는 종료 버킷 **19:04·20:07·20:17·20:18·21:08 KST**를 집계했다. 정확한 분 단위 요청 구간과 버킷 경계에는 차이가 있다. `TotalEnergyConsumption`은 mJ이며 3,600,000으로 나눠 Wh로 환산했다. 원시 `EMI_RAPL_*` 하드웨어 카운터는 앱 에너지/다른 rail과 겹치므로 합계에서 제외했다. CSV 62,227행 중 앱 경로에 쉼표가 들어 있는 5행은 뒤의 25개 고정 필드를 기준으로 복원했다. 제외·중복 행을 숨겨 합계를 맞추지 않았다.

앱 귀속 합계 **28.725Wh** + `Unknown` **22.601Wh** = **51.326Wh**다. 배터리의 실제 감소 **51.387Wh**와 약 0.12% 차이다. 이는 시간 구간·단위·집계 방식의 교차 확인이며 E3의 앱별 추정 정확성을 입증하지 않는다. [Microsoft E3 단위와 추정 범위](https://devblogs.microsoft.com/sustainable-software/measuring-your-application-power-and-carbon-impact-part-1/).

| 앱/프로세스 | 전력 사용 추정(Wh) | 그중 CPU / 디스플레이(Wh) |
| --- | ---: | --- |
| vmmemWSL | 8.568 | CPU 7.450 |
| Brave | 5.214 | CPU 1.284 / 디스플레이 3.176 |
| Explorer | 4.391 | CPU 3.724 / 디스플레이 0.094 |
| Obsidian | 1.907 | CPU 0.026 / 디스플레이 1.631 |
| Windows Terminal | 0.976 | CPU 0.098 / 디스플레이 0.750 |
| Windows PowerShell | 0.767 | CPU 0.664 |
| rustc | 0.262 | CPU 0.223 |
| Windows Node 전체(사용자·검사 합계) | 0.237 | 프로세스 경로로 묶음 |
| 시험 소유 Codex 바이너리 합계 | 0.160 | 고유 격리 경로로 묶음 |
| MiniTerm GUI(debug + release) | 0.100 | 직접 GUI 프로세스만 |
| 앱 미귀속 Unknown | 22.601 | 개별 앱/GPU로 재배분하지 않음 |

위 개발 도구 일부는 사용자 작업과 검사를 함께 포함한다. Windows PowerShell·WSL·Defender·디스크·미귀속 전력까지 MiniTerm 개발 전체의 소모에서 제외하는 표가 아니다. **MiniTerm GUI 자체가 소모의 대부분이었다는 근거는 없지만, 개발 세션의 간접 비용을 무시하지 않는다.**

구간별로 Explorer는 **19:04 종료 버킷에서 4.235Wh**, 이후 모든 버킷 합계는 약 0.156Wh다. 반대로 WSL은 첫 버킷 0.042Wh, 이후 약 8.527Wh다. 따라서 Explorer 사용량을 GPU 충돌의 희생양이라고 지우거나, VM이 처음부터 3시간 동안 주범이었다고 설명하면 실제 시계열과 어긋난다. Explorer의 높은 초반 소비를 만든 작업 내용은 현재 SRUM에서 알 수 없다.

- System의 Hyper-V-VmSwitch 기록에는 스위치 이름 **`WSL (Hyper-V firewall)`**이 명시돼 있다. 18:35 연결, 18:54 해제, 19:11 다른 VM ID 연결을 확인했다. 외부 보고서의 20:07~20:09 연결 해제는 조회한 System 기록에서 확인되지 않았다.
- 현재 `vmwp`/`vmmemWSL`의 생성 시각은 19:11:20이며 `wsl.exe`는 19:11:56에 생성됐다. 네트워크 연결 기록만으로는 CPU/RAM/전력량을 판정하지 않지만, **추가 SRUM에서 WSL의 높은 후반 기여를 확인했다.** 현재 실행 중인 Ubuntu-24.04에서 읽기 전용 프로세스 조회를 했고 Codex·Python·Tailscale·Apache 등의 실행을 확인했다. 현재 평균 CPU 표본은 저녁에 종료된 작업을 복원하지 못하므로 게스트의 특정 작업에 8.568Wh를 귀속하지 않는다. 사용자 WSL은 종료하지 않았다.
- Explorer 배터리 50% 표시를 **누적 CPU 시간 비율**이라고 바꿔 해석할 근거를 확보하지 못했다. Windows는 SRUM의 에너지 추정 자료를 제공한다. CPU 시간만으로 에너지 기여도를 환산하지 않는다. 썸네일 재생성·아이콘 캐시 복구라는 인과관계도 현재 기록에 없다. [Microsoft SRUM/E3 설명](https://devblogs.microsoft.com/sustainable-software/measuring-your-application-power-and-carbon-impact-part-1/).
- 작업 중 Rust 빌드, 도구 준비, 여러 격리 Codex/GUI 검사가 수행됐다. GUI fixture 생성 기록은 19:28~19:54, 최신 release 빌드 완료는 19:51:27이다. 이 작업은 방전 구간과 겹치므로 개발 작업의 기여를 배제하지 않는다. 과거 CPU/GPU/디스크 전력 계측이 없어 기여도는 미확정이다.
- **검증 도구가 공유 서버의 PID 갱신 프로세스를 남긴 결함도 발견했다.** 19:35의 정리 기록에서 시험 소유 updater 19개를 종료했다. 사용자 전역 서버는 유지했고, 이후 검증은 소유 marker·경로·PID 생성 시각·명령줄을 확인해 updater까지 정리하도록 수정했다. 소모 전력은 당시 측정하지 않았으므로 배터리 급감의 주원인이라고 단정하지 않는다.
- 00:24 전후 8초 CPU 관측에서 Explorer 약 1.68%, Brave 개별 프로세스 약 3.11% 등이 관측됐다(논리 프로세서 8개 전체 기준). MiniTerm은 실행 중이 아니었다. 이는 **현재 표본**이며 저녁 3시간의 전력 사용자를 식별하지 못한다. 일부 시스템 프로세스 CPU는 접근 제한으로 포함되지 않는다.
- 오후 15시의 OpenConsole 커밋 고갈·재부팅은 [별도 사고 기록](INCIDENT_2026-10-13.md)에 있다. 오후 18~21시 배터리 원인으로 합쳐 단정하지 않는다.

## 남은 원인 확인과 개발 진행

현재 도구 프로세스는 비관리자다. 처음 `powercfg /srumutil`은 `SruUpdateStats failed`, status 5로 실패했고, `/sleepstudy`와 `/requests`도 관리자 권한을 요구했다. 이후 **사용자가 아래 스크립트를 관리자 PowerShell에서 실행해 SRUM·battery·sleepstudy 모두 exitCode 0으로 수집했다.** 다시 실행할 필요는 없다.

관리자 PowerShell에서 다음 명령을 실행하면 프로젝트 `artifacts/battery-evidence-*`에 SRUM CSV·전원 보고서·오류 보고의 원래 발생 시각·덤프 파일 메타데이터만 저장한다. 설정 변경·드라이버 설치·서비스/VM 종료·덤프 본문 복사는 수행하지 않는다.

```powershell
& 'C:\Users\ghwls\00_Projects\miniterm\scripts\collect-battery-evidence.ps1'
```

GPU 충돌 폭증이라는 핵심 전제는 반증했고, 앱별 전력 추정과 실제 방전도 대조했다. **진단 후 기능 개발을 재개할 수 있다.** 검사 소유 보조 프로세스까지 종료하는 절차를 유지하고 같은 실행 파일의 필요한 검사만 순서대로 실행한다. 현재 AC 연결을 확인했다. 원시 하드웨어 rail/Unknown 22.6Wh를 모두 GPU에 귀속하거나, 드라이버 업데이트로 이 배터리 문제가 해결된다고 단정하지 않는다. 게스트의 당시 실행 작업과 Explorer의 초반 소비 작업은 후속 계측이 필요한 한계다.

드라이버 조치가 필요해도 Samsung 해당 모델의 호환 드라이버를 먼저 확인한다. `winget upgrade Intel.IntelGraphicsCommandCenter`는 디스플레이 설정 앱 업데이트이며 그래픽 드라이버 업데이트를 대신하지 않는다. [Intel 인터페이스 구분](https://www.intel.com/content/www/us/en/support/articles/000088086/graphics/intel-arc-dedicated-graphics-family.html), [Intel OEM 드라이버 안내](https://www.intel.com/content/www/us/en/support/articles/000038757/graphics.html). 드라이버·전원 설정·Hyper-V/WSL 변경은 수행하지 않았다.

## 보존한 근거

`artifacts/battery-diagnosis-20261014/`에 비관리자 수집 자료를 저장했다. 그 안의 `wer-metadata.json`은 접근 거부 때문에 발생 시각이 비어 있다. **성공한 관리자 원본은 `artifacts/battery-evidence-20261013-d336f45d5462419e9a9122dcc4a89412/`**의 `srum.csv`, `kernel-report-metadata.json`, `battery-report.xml`, `sleepstudy.xml`, `collection-status.json`이다. 집계는 같은 폴더의 `srum-window-analysis.json`에 보존했다. 검증 updater 정리 근거는 `artifacts/fixture-updater-cleanup.json`이다. `artifacts`는 Git 제외 대상이므로 다른 환경에는 문서와 수집 스크립트만 전달된다.
