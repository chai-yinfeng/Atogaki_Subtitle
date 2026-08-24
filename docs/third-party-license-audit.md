# 第三方依赖许可证审计

_审计日期：2026-08-24_

本记录用于 Atogaki `0.1.0` macOS Apple Silicon 预发布基线和 Windows x86_64 构建基线的工程审计，不构成法律意见。每次升级锁文件、sidecar 或目标平台后都必须重新生成并复核。

## 范围与结论

审计覆盖以下实际分发边界：

- `src-tauri/Cargo.lock` 中 `aarch64-apple-darwin` 的 Rust/Tauri 正常与构建依赖；
- `ui/package-lock.json` 中进入 WebView 的运行依赖，以及为完整性记录的前端构建依赖；
- App Bundle 内独立执行的 `whisper-cli`、`ffmpeg`、`ffprobe` 和 FFmpeg 静态字幕栈；
- Atogaki 自有源码与安装包材料。

当前锁定版本中没有发现迫使 Atogaki 自有源码采用 GPL、AGPL 或 SSPL 的必选依赖。Atogaki 继续使用 Apache-2.0；第三方组件保留各自许可。最重要的分发义务来自 LGPL FFmpeg/FriBidi 字幕栈：Release 必须同时提供许可证、精确对应源码、校验值和重建信息。

该结论适用于当前 macOS arm64 已发布配置，以及下文单独记录的 Windows x86_64 构建配置。x86_64 macOS、Windows ARM64、新 provider、新模型格式或任何依赖升级都需要单独审计。

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
- 60 个构建依赖，使用 MIT、Apache-2.0、BSD-3-Clause、ISC 或 MPL-2.0；其中 lightningcss 的平台/构建包为 MPL-2.0，但不会作为 Node package 复制进 App。报告为它们记录包名、版本、许可证表达式和来源，不附随 `npm ci` 当前平台变化的可选原生构建包全文；
- 运行依赖缺少已安装许可证全文，或 lockfile 出现未审查表达式时，生成立即失败。

## Windows x86_64 差异审计

Windows 使用同一 `src-tauri/Cargo.lock`，但按 `x86_64-pc-windows-msvc` 重新解析 target-specific 依赖并生成 [`src-tauri/third-party/rust-licenses-windows.html`](../src-tauri/third-party/rust-licenses-windows.html)。报告包含 Windows、WebView2、注册表与 MSVC target crate；接受的许可证类别仍为 Apache-2.0、MIT、Unicode-3.0、BSD-3-Clause、MPL-2.0、ISC、CDLA-Permissive-2.0、Zlib 与 0BSD，没有新增 GPL、AGPL、SSPL 或未知表达式。Windows CI 使用固定 `cargo-about 0.9.1` 离线重生成，并在报告与仓库版本不一致时停止打包。

Windows sidecar 与 macOS 使用同一组固定上游版本和源码 SHA-256，但构建清单、二进制哈希和对应源码包按 target 独立生成：

- `whisper-cli.exe` 使用 MSVC 静态 CPU 基线；FFmpeg／libass 字幕栈使用 MSYS2 UCRT64／MinGW-w64，并通过独立进程边界与 Atogaki 通信。
- FFmpeg 构建检查 `--disable-gpl`、`--disable-nonfree`、无 libx264、LGPL 自述、`ass` filter 与 MPEG-4 encoder。
- PE 依赖检查拒绝 `msys-2.0.dll`、MinGW GCC／C++／pthread DLL 和动态字幕栈 DLL，避免用户机器隐式依赖开发环境。
- 对应源码归档包含七份精确上游源码、Windows 构建脚本、许可证、清单和逐文件 SHA-256；开发候选将它与 NSIS 作为 Actions Artifact 保存，公开预发布则将二者及各自 SHA-256 放入同一 Release。

2026-08-22 的 [Windows sidecars 32575359901](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/32575359901) 在 Windows Server 2022 runner 上重新生成 Rust target 与前端报告并确认仓库无差异，完成 LGPL 配置与 PE 依赖检查、对应源码归档、NSIS 安装后能力检查和卸载；这使上述 Windows 构建结论成为已执行的 CI 基线，而不是只在 macOS 上准备的交叉平台材料。

2026-08-23 的 [`v0.1.0-alpha.6` 发布流水线](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/32622817441)在固定 tag 上再次从源码冷构建并完成同一审计与安装门禁；其[公开 prerelease](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/tag/v0.1.0-alpha.6)同时提供安装器、安装器 SHA-256、45.5 MB 对应源码包及其 SHA-256，验证 Windows LGPL 交付材料已实际随二进制发布。

这证明依赖闭包和构建材料达到 Windows 打包基线；未签名 NSIS 的 SmartScreen 行为、真实用户数据目录、Credential Manager、媒体闭环与卸载边界仍必须在 Windows 11 实机验收，不能由许可证审计替代。

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

归档包含七份精确上游源码、逐文件 `SHA256SUMS`、当前二进制构建清单、固定版本文件、完整 sidecar 构建脚本和许可证文本。脚本会先验证打包前 sidecar 与源码基线完全一致；任一版本或哈希漂移都会停止。Tauri 对最终 Bundle 做 ad-hoc 签名后 Mach-O 文件哈希会变化，因此 App 内 sidecar 改用 `codesign` 校验，整个交付物再由 DMG SHA-256 固定。

## 模型与云端服务

Whisper/VAD 模型不打包进 App 或 DMG，由用户在设备上选择或下载。内建目录只保存下载地址、固定 SHA-256 和上游许可/说明链接；正式发布前仍应逐项复核模型页面的许可证与使用条件。模型镜像只改变传输来源，不改变已接受的模型字节。

DeepL、DeepSeek 与自定义 OpenAI-compatible 服务都通过现有 HTTPS 客户端直接调用，App 不分发这些服务的 SDK。用户仍需遵守所选服务的条款、配额和内容处理要求；自定义端点由用户自行确认运营方。后续增加带新 SDK 的翻译服务时，仍需审计新增依赖、服务条款、数据发送说明和输出归属。

学习词典使用的 `tar`、`zstd`、`csv` 与 `sha1` Rust 依赖已进入同一 `cargo-about` 报告，其许可仍落在现有接受集合内；FreeDict 退役后已移除专用 `xz2`／liblzma 依赖。JMdict、Tomoshi 与 ECDICT 的数据不随 App 分发，由用户明确下载；UI 对实际读取范围显示对应版本、署名和许可标签。ECDICT 仓库当前声明 MIT，但 README 记载的混合历史数据来源仍需持续审计；本轮只提供固定基础 CSV 的下载入口，不把数据打进 App，也不使用缺少发布方 checksum 的增强版 ZIP。

Merriam-Webster 是用户自带 Key 的非商业网络服务，不随 App 分发 SDK 或词典数据。API 结果仅由用户点击查询，缓存 24 小时；界面使用官方品牌规范提供且未修改的浅色背景 PNG Logo，以批准的 50px 尺寸显示。仓库文件 `ui/public/merriam-webster-logo.png` 来自 `https://dictionaryapi.com/images/info/branding-guidelines/MWLogo_LightBG_120x120_2x.png`，原始尺寸 240×240，SHA-256 为 `6ddee7e22cbe0686e9ae6de180eea8342ae89d4f0e923b39d409e6a0c76f49bd`。公开发行前必须再次确认非商业、每日每 reference 1,000 次、最多两个 reference、品牌展示和禁止比较／基准测试等当时条款；当前按来源切换的独立展示不得演变为自动排名。

## 每次 Release 的合规检查

1. 锁定提交、版本号、target triple 和 sidecar `build-manifest.txt`。
2. 重新生成 Rust 与前端 HTML，检查新增/未知许可证并提交生成结果。
3. 重新构建 sidecar，确认 FFmpeg 配置没有 `--enable-gpl`、`--enable-nonfree` 或 `libx264`。
4. 生成对应源码包，校验外层 SHA-256，并抽查归档内的 `SHA256SUMS`。
5. 确认 DMG/App 含 Apache-2.0 项目许可证、`third-party/` 声明和 sidecar 许可证。
6. 将 DMG、DMG 校验文件、对应源码包及其校验文件上传到同一个 GitHub Release。
7. 在 Release notes 写明架构、最低 macOS、ad-hoc 签名/未公证状态、模型不内置和第三方材料入口。

## 尚未覆盖

- macOS x86_64、Windows ARM64，以及 Windows 11 实机上的 SmartScreen、系统集成和用户数据边界；
- Apple 签名、公证和未来应用商店条款；
- 模型本身的逐版本法律审查；
- 用户导入媒体、字幕及云端翻译内容的权利。
