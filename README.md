# 진여신전생 4F 한글 패처

Windows용 Rust + Tauri 2 앱입니다. 외부 도구와 게임·패치 파일은 포함하지 않습니다.

개발 준비: Rust MSVC 도구 체인, Visual Studio C++ Build Tools, WebView2, Node.js 22 이상.

```powershell
npm.cmd install
npm.cmd run tauri dev
npm.cmd run tauri build
```

도구 호출·입출력 규약은 [도구 연동 문서](docs/tool-integration.md)를 따릅니다.
