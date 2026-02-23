# Alpha PNG to 32-bit BMP Converter (Koei FaceTool 호환)

이 프로그램은 배경이 투명한 PNG 파일을 **코에이(Koei) 삼국지 FaceTool** 및 구형 게임 엔진에서 완벽하게 인식하는 **알파 채널 포함 32비트 BMP**로 변환해주는 도구입니다.


## ✨ 주요 기능

- **배경 투명도 유지**: PNG의 알파 채널을 32비트 BMP(ARGB)로 완벽하게 변환하여 게임 내에서 투명 배경이 정상 출력됩니다.
- **Koei FaceTool 최적화**: 
  - 표준 40바이트 `BITMAPINFOHEADER` 사용.
  - 고전적인 **Bottom-Up**(하단부터 기록) 픽셀 정렬 방식 적용.
  - 구형 툴에서 발생하는 "지원하지 않는 형식" 오류 해결.
- **드래그 앤 드롭 (Drag & Drop)**: 탐색기에서 파일을 창으로 끌어다 놓기만 하면 즉시 변환됩니다. (Windows Hook을 통한 권한 필터 우회 적용)
- **현대적인 UI**: Slint 프레임워크를 사용한 세련되고 직관적인 인터페이스.

## 🚀 시작하기

### 사전 요구 사항

- [Rust](https://www.rust-lang.org/tools/install) (버전 1.70 이상 권장)
- Windows OS (드래그 앤 드롭 기능은 Windows 전용으로 최적화되어 있습니다)

### 설치 및 실행

1. 저장소를 클론합니다.
   ```bash
   git clone https://github.com/your-username/rust_bmp.git
   cd rust_bmp
   ```

2. 프로그램을 빌드하고 실행합니다.
   ```bash
   cargo run --release
   ```

## 🛠 사용 방법

1. 프로그램을 실행합니다.
2. 변환하고 싶은 **투명 배경 PNG** 파일을 준비합니다.
3. 파일을 프로그램 창 위로 **드래그 앤 드롭**하거나, **Load PNG File** 버튼을 클릭하여 선택합니다.
4. 원본 파일과 같은 위치에 알파 채널이 포함된 `.bmp` 파일이 생성됩니다.
5. 생성된 파일을 삼국지 FaceTool 등에서 불러와 사용합니다.

## 📦 기술 스택

- **Language**: Rust
- **UI Framework**: [Slint](https://slint.dev/)
- **Image library**: `image` crate
- **Windows API**: `windows-sys` (Win32 API Hooking for Drag & Drop)

## 📝 라이선스

이 프로젝트는 MIT 라이선스 하에 배포됩니다. 자유롭게 수정 및 배포가 가능합니다.

---
**Note**: 포토샵에서 수동으로 채널 작업을 할 필요 없이 바로 게임에 적용 가능한 BMP를 만들어줍니다.
