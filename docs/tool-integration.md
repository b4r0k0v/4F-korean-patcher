# 외부 도구 조사 및 연동 규약

## 조사 범위

2026-10-19 기준 `dev-temp`에 준비된 도구의 배치 파일과 패치 ZIP 내부 항목·설명서를 읽었다. 실행 파일이나 게임 파일로 실제 패치를 수행하지는 않았다. 아래에서 배치 동작은 정적 분석 결과이며, 앱 연동 방식은 그 결과에 따른 구현 계획이다.

주요 확인 파일:

- `../Batch CIA 3DS Decryptor/Batch CIA 3DS Decryptor.bat`, `readme.txt`
- `../HackingToolkit3DSv9/HackingToolkit3DS.bat`
- 같은 폴더의 `Unpack3DSYes.bat`, `Unpack3DSNo.bat`, `UnpackCIAYes.bat`, `UnpackCIANo.bat`, `Repack3DS.bat`, `RepackCIA.bat`
- `../HackingToolkit3DSv9/ScriptsReady/`의 실행 파일 경로 지정 방식
- 본편 1·2차 및 DLC ZIP, 2차 본편·DLC README, DLC의 `build_dlc_cia.ps1`

## 1. Batch CIA 3DS Decryptor

| 항목 | 확인한 동작 |
| --- | --- |
| 진입점 | `Batch CIA 3DS Decryptor.bat` |
| 파일 인자 | 없음. 현재 작업 디렉터리의 `*.cia`, `*.3ds`를 열거 |
| 의존 도구 | 같은 실행 환경에서 찾을 수 있는 `decrypt.exe`, `ctrtool.exe`, `makerom.exe` |
| 입력 이름 조건 | 이름에 `decrypted`가 들어간 파일은 처리에서 제외 |
| DLC 분기 | `ctrtool -tmd` 출력의 타이틀 ID 접두사 `0004008C`로 판별 |
| DLC 결과 | `<입력 stem> (DLC)-decrypted.cia` |
| 진단 파일 | `log.txt`, 중간 `content.txt`, `<입력 stem>.*.ncch` |
| 종료 | 끝에서 `pause >nul` 후 `exit`. 하위 명령 실패 시 즉시 중단하는 구조가 아님 |

DLC 분기는 `decrypt`로 NCCH를 생성하고, TMD에서 콘텐츠 정보를 읽어 `makerom -f cia -dlc -ignoresign -target p -o ... -i ...`로 다시 묶는다. 콘텐츠 인덱스와 콘텐츠 ID를 구분해서 전달한다.

배치는 시작과 끝에 현재 폴더의 `*.ncch`를 삭제한다. 따라서 사용자 도구 폴더나 입력 폴더에서 직접 실행하지 않는다. 실행별 전용 디렉터리에 필요한 도구와 `dlc.cia` 복사본만 준비한다.

앱 구현 시 선택할 경로:

- 우선안: Rust에서 배치의 DLC 처리 순서를 구현하고 세 실행 파일을 직접 호출한다. 각 단계의 종료 코드와 오류 출력을 수집하고, `pause` 없이 종료한다.
- 배치를 그대로 이용해야 하는 경우: 전용 디렉터리를 CWD로 지정해 `cmd.exe /d /c`로 실행하고, 마지막 키 입력 대기를 해소할 표준입력 처리를 검증한다. 종료 코드만으로 성공을 판정하지 않는다.
- 원본 도구·입력은 수정하지 않으며, 상세 trace에 `log.txt` 내용도 연결한다.

## 2. HackingToolkit3DS

### 진입점과 인자형 배치

`HackingToolkit3DS.bat`는 `set /p`로 메뉴를 받고 작업 후 `pause`를 거쳐 메뉴로 돌아간다. 메뉴는 본편 추출 `D`, 3DS 재생성 `R`, CIA 추출 `CE`, CIA 재생성 `CR`이다. `CR`은 출력 stem과 minor/micro 버전을 추가로 입력받는다.

자동화에 참고할 별도 배치:

| 배치 | 입력 | 추출·출력 규칙 |
| --- | --- | --- |
| `Unpack3DSYes.bat` | 첫 번째 인자: 3DS 파일명 | `<파일명.3ds>_Unpacked/`, ExeFS에 `-xutf` 사용 |
| `Unpack3DSNo.bat` | 첫 번째 인자: 3DS 파일명 | 동일 경로, ExeFS에 `-xtf` 사용 |
| `UnpackCIAYes.bat` | 첫 번째 인자: CIA 파일명 | `<파일명.cia>_Unpacked/`, ExeFS에 `-xutf` 사용 |
| `UnpackCIANo.bat` | 첫 번째 인자: CIA 파일명 | 동일 경로, ExeFS에 `-xtf` 사용 |
| `Repack3DS.bat` | 첫 번째 인자의 stem | `<stem>.3ds_Unpacked/`를 읽고 `<stem>_Edited.3ds` 생성 |
| `RepackCIA.bat` | 첫 번째 인자의 stem | `<stem>.cia_Unpacked/`를 읽고 `<stem>_Edited.cia` 생성 |

이 인자형 배치에는 메뉴 루프나 마지막 `pause`가 없다. 경로는 CWD 기준이며, `%1` 뒤에 `_Unpacked`를 직접 붙이는 등 인용 처리가 불완전하므로 사용자 경로를 그대로 넣어도 안전하게 동작한다고 가정하지 않는다.

### 추출·재생성 과정

- 3DS: `3dstool -xtf 3ds`로 NCSD 헤더와 파티션을 분리하고, 파티션 0은 `cxi`, 1·2·6·7은 `cfa`로 추출한다.
- CIA: `ctrtool --content=...`로 `DecryptedApp.*`를 추출한다. 배치는 콘텐츠 0·1·2를 파티션 파일로 바꾼 뒤 각각 추출한다. 그 외 `DecryptedApp.*`는 별도로 보존해야 한다.
- 본편 작업 루트에는 `HeaderNCCH*.bin`, `HeaderExeFS.bin`, `DecryptedExHeader.bin`, `ExtractedExeFS/`, `ExtractedRomFS/` 등이 생성된다. 옵션 파티션의 존재는 실제 입력에 따라 확인한다.
- 재생성은 RomFS/ExeFS 생성 → CXI/CFA 파티션 생성 → 3DS 또는 CIA 컨테이너 생성 순서다. 배치는 `Custom*.bin` 중 20,000바이트 이하를 삭제하는 처리도 포함하므로 이를 일반적인 성공 판정 기준으로 복제하지 않는다.

### 준비된 도구에서 확인한 연동 차이

1. 루트의 CIA 배치는 `ctrtool.exe`, `MakeRom.exe`를 호출하지만 실제 폴더에는 `CtrTool32.exe`, `CtrTool64.exe`, `MakeRom32.exe`, `MakeRom64.exe`가 있다. 앱은 실제 존재하며 실행 가능한 아키텍처의 파일을 명시적으로 선택한다.
2. `ScriptsReady/` 배치는 `%PROGRAMFILES%/HackingToolkit3DS/`를 전제로 한다. 사용자가 지정한 압축 해제 경로를 지원하려면 이 고정 설치 경로에 의존하지 않아야 한다.
3. `RepackCIA.bat`는 `.cia_Unpacked`를 전제로 한다. 본편 `.3ds_Unpacked` 결과를 CIA로 변환할 때는 동일 작업 루트를 명시하는 어댑터가 필요하다.
4. 대부분의 명령은 출력을 `NUL`로 버리고 실패 여부를 검사하지 않는다. Rust에서 개별 실행 파일을 호출해 각 단계의 진단 정보를 확보한다.
5. 일반 CIA 재생성 배치는 콘텐츠 0·1·2만 포함한다. DLC 전체를 다시 묶는 작업에 그대로 사용하면 안 된다.

따라서 메뉴에 키 입력을 흉내 내는 방식 대신, Rust 어댑터가 준비된 도구 폴더의 실행 파일을 절대 경로와 개별 인자로 호출한다. CWD는 앱의 전용 작업 디렉터리로 지정한다. 실행 파일은 계속 사용자가 준비하며 앱에 번들하지 않는다.

## 3. ZIP 패치 구조와 적용 경로

지원 형식은 `.zip`으로 고정한다. 사용자 지정 규칙과 준비된 ZIP 엔트리를 함께 확인했다.

| 패치 | 준비된 파일 | 적용 기준 |
| --- | --- | --- |
| 본편 1차 | `SMT4Final 2.0.zip` | ZIP 루트 전체. 실제 최상위 폴더는 `ExeFS/`, `RomFS/` |
| 본편 2차 | `SMT4F_KO_FOR_REPACK_v1.1.2.zip` | ZIP 루트의 `patch/` 내부 |
| DLC | `SMT4F_KO_DLC_Patch_v1.1.2.zip` | ZIP 루트의 `patch/` 내부 |

- 1차의 `ExeFS/*` → 본편 작업 루트의 `ExtractedExeFS/*`, `RomFS/*` → `ExtractedRomFS/*`로 매핑한다. 폴더명 차이 때문에 사용되지 않는 `ExeFS/`, `RomFS/`가 별도로 생기는 것을 막는다.
- 2차에는 `patch/DecryptedExHeader.bin`, `patch/ExtractedExeFS/code.bin`, `patch/ExtractedRomFS/...`가 있다. `patch/` 접두사만 제거해서 작업 루트에 덮어쓴다. ExHeader와 code.bin을 함께 적용한다.
- DLC에는 `patch/DecryptedApp.*`, `patch/DecryptedManual.bin`, `patch/DecryptedDownloadPlay.bin`, 추출 하위 폴더 변경분이 있다. 동일하게 `patch/` 접두사만 제거한다.
- ZIP의 설명서·검증 파일·동봉 빌더를 패치 적용 대상으로 실행하지 않는다. 패치 및 도구 해시 검증도 추가하지 않는다.

## 4. DLC 전용 재생성 계획

DLC ZIP의 설명서와 빌드 스크립트는 일반 CIA 배치와 다른 구성을 명시한다.

- 콘텐츠는 0~32의 총 33개다. 콘텐츠 3~32는 `DecryptedApp.<index>.<id>`를 사용한다.
- 콘텐츠 1·2는 `HeaderNCCH1.bin`·`HeaderNCCH2.bin`과 패치된 `DecryptedManual.bin`·`DecryptedDownloadPlay.bin`으로 CFA를 만든다.
- 동봉 빌더는 콘텐츠 0에 `rebuild/content.0000.cfa`를 사용하고, `rebuild/dlc.rsf`와 `makerom -dlc -rsf ... -DSaveSize=0`을 이용한다.
- 동봉 빌더를 그대로 실행하면 패치 파일 해시 검증, 별도 임시 경로와 정리 정책까지 실행된다. 요구사항과 맞추기 위해 빌드 규약을 참고해 Rust 어댑터로 구현한다.
- 기본 계획은 HackingToolkit3DS 추출 결과의 콘텐츠 0을 포함해 원본 콘텐츠와 인덱스·ID를 보존하고, `patch/` 변경분을 적용해 전부 다시 묶는 방식이다. 동봉 `rebuild/`의 특수 데이터가 필수인지는 실제 추출 결과와 비교해 검증한다. 필요하면 해당 데이터를 명시적인 보조 입력으로 사용하며 `patch/` 적용 규칙과 구분한다.
- 원본 콘텐츠 0 보존 방식과 준비된 도구의 DLC 재생성 옵션 호환성을 실파일로 확인하기 전에는 DLC 연동 완료로 판정하지 않는다.

## 5. 남은 검증

- 본편 `code.bin`의 압축 해제 여부와 1차·2차 적용 후 재생성 호환성.
- 선택적 파티션의 존재를 구분한 성공 판정, 실제 DLC 전체 콘텐츠 추출과 재생성.
- DLC ZIP의 보조 빌드 데이터 필요 여부, 선택한 외부 실행 파일과 명령 옵션의 호환성.
- 공백·한글 경로, 취소 시 자식 프로세스 종료, 디스크 사용량과 처리 시간.

도구 진입점, 배치의 인자와 CWD 규칙, ZIP 형식·적용 루트는 확인을 마친 항목이다. 위 목록은 실제 실행으로 검증해야 하는 항목이다.
