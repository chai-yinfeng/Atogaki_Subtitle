# 第三方依赖许可证审计

_审计日期：2026-08-06_

本记录用于 Atogaki `0.1.0` macOS Apple Silicon 预发布基线的工程审计，不构成法律意见。每次升级锁文件、sidecar 或目标平台后都必须重新生成并复核。

## 范围与结论

审计覆盖以下实际分发边界：

- `src-tauri/Cargo.lock` 中 `aarch64-apple-darwin` 的 Rust/Tauri 正常与构建依赖；
- `ui/package-lock.json` 中进入 WebView 的运行依赖，以及为完整性记录的前端构建依赖；
- App Bundle 内独立执行的 `whisper-cli`、`ffmpeg`、`ffprobe` 和 FFmpeg 静态字幕栈；
- Atogaki 自有源码与安装包材料。

当前锁定版本中没有发现迫使 Atogaki 自有源码采用 GPL、AGPL 或 SSPL 的必选依赖。Atogaki 继续使用 Apache-2.0；第三方组件保留各自许可。最重要的分发义务来自 LGPL FFmpeg/FriBidi 字幕栈：Release 必须同时提供许可证、精确对应源码、校验值和重建信息。

该结论只适用于当前 macOS arm64 锁文件与当前 sidecar 配置。x86_64 macOS、Windows、新 provider、新模型格式或任何依赖升级都需要单独审计。

## Rust 与 Tauri

使用 `cargo-about 0.9.1` 对 `src-tauri/Cargo.toml` 的锁定 macOS arm64 图执行生成，产出 [`src-tauri/third-party/rust-licenses.html`](../src-tauri/third-party/rust-licenses.html)。当前报告的主要许可证为 Apache-2.0、MIT、Unicode-3.0、BSD-3-Clause、MPL-2.0、ISC、CDLA-Permissive-2.0、Zlib 与 0BSD。

需要注意的表达式：

- MPL-2.0 出现在 `cssparser`、`cssparser-macros`、`dtoa-short`、`option-ext` 和 `selectors` 等依赖中。当前使用的是未修改的上游 crate；MPL 义务由随 App 提供许可证声明来承接，不会把独立的 Atogaki 源码整体改为 MPL。
- `r-efi` 声明为 `MIT OR Apache-2.0 OR LGPL-2.1-or-later`，分发可采用其许可表达式中的宽松选项；报告生成器仍允许完整 SPDX 表达式中的 LGPL 标识。
- 生成配置遇到未知或未接受的许可证会失败，防止新增依赖静默进入分发物。

复现命令：

```bash
cargo install --locked --features cli --version 0.9.1 cargo-about
./scripts/generate-rust-licenses.sh
```

脚本固定 target、锁文件、离线解析和工具版本。依赖首次取回可在联网环境先运行一次 Cargo metadata/build；最终声明生成不允许临时更改 lockfile。

## 前端

[`scripts/generate-frontend-licenses.mjs`](../scripts/generate-frontend-licenses.mjs) 直接读取 `ui/package-lock.json`，区分 WebView 运行依赖与仅构建依赖，并生成 [`src-tauri/third-party/frontend-licenses.html`](../src-tauri/third-party/frontend-licenses.html)。当前结果为：

- 1 个运行依赖：`@tauri-apps/api`，Apache-2.0 OR MIT；
- 60 个构建依赖，使用 MIT、Apache-2.0、BSD-3-Clause、ISC 或 MPL-2.0；其中 lightningcss 的平台/构建包为 MPL-2.0，但不会作为 Node package 复制进 App；
- 运行依赖缺少已安装许可证全文，或 lockfile 出现未审查表达式时，生成立即失败。

复现命令：

```bash
npm --prefix ui ci
node ./scripts/generate-frontend-licenses.mjs
```

## Sidecar 与静态字幕栈

App 将 sidecar 当作独立命令行程序，通过参数、文件、标准流和退出状态通信：

| 组件 | 固定版本 | 许可/配置 |
| --- | --- | --- |
| whisper.cpp / `whisper-cli` | v1.8.6，commit `23ee035…` | MIT |
| FFmpeg / `ffmpeg` / `ffprobe` | 8.1.2 | LGPL-2.1-or-later；关闭 GPL/nonfree/network |
| libass | 0.17.5 | ISC |
| libunibreak | 7.0 | 宽松许可，见归档全文 |
| FriBidi | 1.0.16 | LGPL-2.1-or-later |
| FreeType | 2.14.3 | FreeType License/GPL 双许可材料；当前按 FreeType License 分发 |
| HarfBuzz | 14.3.0 | 宽松许可，见归档全文 |

`scripts/build-sidecars-macos.sh` 明确拒绝 GPL/nonfree 配置并检查 `libx264` 不存在。使用 macOS 系统 zlib、bzip2、iconv、VideoToolbox 和 AudioToolbox；这些系统组件不复制到源码归档。

对应源码通过以下命令生成：

```bash
./scripts/package-sidecar-sources-macos.sh
```

默认输出：

```text
src-tauri/target/release/bundle/sources/
├── Atogaki-0.1.0-third-party-sources.tar.xz
└── Atogaki-0.1.0-third-party-sources.tar.xz.sha256
```

归档包含七份精确上游源码、逐文件 `SHA256SUMS`、当前二进制构建清单、固定版本文件、完整 sidecar 构建脚本和许可证文本。脚本会先验证已构建二进制清单与源码基线完全一致；任一版本或哈希漂移都会停止。

## 模型与云端服务

Whisper/VAD 模型不打包进 App 或 DMG，由用户在设备上选择或下载。内建目录只保存下载地址、固定 SHA-256 和上游许可/说明链接；正式发布前仍应逐项复核模型页面的许可证与使用条件。模型镜像只改变传输来源，不改变已接受的模型字节。

DeepL API 不向 App 分发 SDK；当前通过 HTTPS API 调用。用户仍需遵守自己的服务条款、配额和内容处理要求。未来增加 Google Translate 或 LLM provider 时，应同时审计 SDK/协议依赖、服务条款、数据发送说明和输出归属。

## 每次 Release 的合规检查

1. 锁定提交、版本号、target triple 和 sidecar `build-manifest.txt`。
2. 重新生成 Rust 与前端 HTML，检查新增/未知许可证并提交生成结果。
3. 重新构建 sidecar，确认 FFmpeg 配置没有 `--enable-gpl`、`--enable-nonfree` 或 `libx264`。
4. 生成对应源码包，校验外层 SHA-256，并抽查归档内的 `SHA256SUMS`。
5. 确认 DMG/App 含 Apache-2.0 项目许可证、`third-party/` 声明和 sidecar 许可证。
6. 将 DMG、DMG 校验文件、对应源码包及其校验文件上传到同一个 GitHub Release。
7. 在 Release notes 写明架构、最低 macOS、未签名/公证状态、模型不内置和第三方材料入口。

## 尚未覆盖

- macOS x86_64 和 Windows 的完整依赖闭包、系统库、sidecar 构建与安装器；
- Apple 签名、公证和未来应用商店条款；
- 模型本身的逐版本法律审查；
- 用户导入媒体、字幕及云端翻译内容的权利。
