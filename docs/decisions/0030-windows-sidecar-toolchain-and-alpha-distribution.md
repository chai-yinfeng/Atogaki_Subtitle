# 0030：Windows sidecar 使用分离工具链并以 NSIS 交付首个测试版

日期：2026-08-22
状态：已采用

## 背景

Atogaki 的 Windows 主程序由 Tauri／Rust 构建，同时需要分发 `whisper-cli`、`ffmpeg` 和 `ffprobe` 三个独立 sidecar。Tauri 官方 Windows 路径以 MSVC、Windows SDK 和 WebView2 为基线；FFmpeg、libass 及其字体栈则以上游 `configure`／Meson／pkg-config 构建为主。强制所有组件使用同一编译器会增加补丁和维护成本，直接采用第三方预编译 FFmpeg 又难以固定功能、许可证配置、对应源码和可复现性。

首个 Windows 测试版还需要确定安装包、系统范围和未签名分发边界，避免 CI 偶然产出的文件被误认为正式支持。

## 决定

- Tauri／Rust 主程序使用 `x86_64-pc-windows-msvc`；`whisper-cli` 使用 CMake 与 MSVC，首版关闭针对构建机的原生 CPU 指令集优化，并以 CPU 识别为稳定基线。
- FFmpeg、ffprobe、libass、libunibreak、FriBidi、FreeType 与 HarfBuzz 使用 Windows 原生 MSYS2 UCRT64／MinGW-w64，从 `scripts/sidecar-versions.zsh` 固定的上游源码和 SHA-256 构建。
- FFmpeg 字幕栈优先静态链接进 sidecar，只保留 Windows 系统 DLL 依赖；不得要求用户安装 MSYS2、FFmpeg、Whisper 或把开发工具加入 `PATH`。
- Atogaki 只通过参数、文件、标准流和退出状态调用 sidecar，不把 MinGW 库链接进 MSVC 主进程，因此不跨工具链共享 C/C++ ABI、对象或内存所有权。
- FFmpeg 保持 LGPL-only：关闭 GPL、nonfree、network 和 libx264，必须提供 libass `ass` filter 与 MPEG-4 软件编码器；构建后检查配置、能力、动态依赖、许可证、二进制哈希和对应源码材料。
- 首个安装包只生成 NSIS，Windows 11 x86_64 是正式验收范围；Windows 10 在取得实测结论前只做尽力兼容，不作为首版门禁。
- 允许向知情测试者提供明确标注的未签名 alpha。GitHub Actions 产物先作为临时 Artifact；通过真实 Windows 设备冒烟后才能进入 GitHub Release。公开推广前重新评估受信任代码签名证书。

## 后果

- 每类上游使用其成熟工具链，减少维护私有构建补丁；代价是 CI 同时维护 MSVC 与 MSYS2 环境。
- 进程边界隔离两套 ABI，但仍必须检查 sidecar 没有意外依赖 UCRT64 工具链 DLL，并在干净 Windows 设备验证启动。
- 静态字幕栈降低缺失 DLL、加载路径和版本漂移风险；每次 Release 必须同时提供精确对应源码、许可证、构建脚本、清单和校验值。
- NSIS、Windows 11 和 unsigned alpha 只定义首个测试范围，不排除后续增加 MSI、Windows 10、ARM64、GPU 后端或正式代码签名；这些扩展必须各自建立测试和发布门禁。
