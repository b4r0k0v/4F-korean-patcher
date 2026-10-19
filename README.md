# 진여신전생 4F 한글 패처

Windows용 Rust + Tauri 2 앱입니다. 외부 도구와 게임·패치 파일은 포함하지 않습니다.

개발 준비: Rust MSVC 도구 체인, Visual Studio C++ Build Tools, WebView2, Node.js 22 이상.

```powershell
npm.cmd install
npm.cmd run tauri dev
npm.cmd run tauri build
```

도구 호출·입출력 규약은 [도구 연동 문서](docs/tool-integration.md)를 따릅니다.

[사용 안내](docs/user-guide.md) · [검증 기록](docs/verification.md)

```powershell
npm.cmd run build
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --locked --manifest-path src-tauri/Cargo.toml --lib --bins
```

준비한 실파일로 네 조합을 검증하려면 `dev-temp/games/base-decrypted.3ds`, `dev-temp/games/dlc.cia`와 계획 문서의 도구·ZIP을 `dev-temp/`에 준비한 뒤 아래 명령을 실행합니다. 출력과 실패 자료는 `dev-temp/integration/`에 남으며 Git에 포함되지 않습니다.

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/verify-integration.ps1 -All
```
