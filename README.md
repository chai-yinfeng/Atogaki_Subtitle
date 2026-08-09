# Atogaki

Atogaki 是一个本地优先的外语音视频理解、字幕校对与导出工具。它最初是为了把
ヨルシカ「後書き」电台整理成便于中文使用者理解和学习的日中双语资料；长期方向是扩展为泛用的本地字幕识别、翻译、编辑和烧录工作台。

项目目前处于 macOS Apple Silicon 预发布阶段。核心闭环已经可用，但尚未提供 Developer ID 签名、公证的正式安装包。Atogaki 是独立的个人开发项目，与ヨルシカ及其官方运营方没有隶属、授权或赞助关系。

## 能做什么

- 导入本机音频或视频，用内置 `whisper-cli` 和用户下载的 Whisper 模型在本地识别日语或英语。
- 使用 Silero VAD 过滤静音、音乐和环境声，并通过词表提高人名与作品名的一致性。
- 在播放器中按时间轴查看、跳转和编辑字幕；原文修改后会标记对应译文为待重译。
- 可选使用 DeepL 将日语或英语翻译为简体中文。翻译接口已与工作区解耦，后续可以增加 Google Translate 或 LLM provider。
- 从当前 SQLite 工作区导出原文、简体中文和双语 SRT/ASS，或把字幕烧录到 MP4。
- 记录任务、失败阶段、重试来源和烧录历史；重启后会识别被中断的任务，并允许从冻结参数创建新任务重试。

媒体、模型、任务和编辑结果默认保留在设备上。只有启用云端翻译时，待翻译的原文字幕及有限的相邻上下文会发送给所选 provider。

## 安装与首次配置

当前首先支持 Apple Silicon Mac。正式 GitHub Release 发布前，开发者可以按下文从源码构建；测试版 DMG 会作为 GitHub Release asset 提供，而不会提交到 Git 历史。

首次打开 App 时，启动配置会引导完成三部分：

1. **本地模型。** 选择已有 `ggml-*.bin`，或下载 Whisper 与 Silero VAD。App 管理的默认目录通常是 `~/Library/Application Support/com.chai-yinfeng.atogaki/models/`，界面会显示本机的实际路径。
2. **网络。** 选择跟随启动环境、强制直连或自定义 HTTP/HTTPS 代理；也可以填写 HTTPS 模型镜像。连接测试使用当前输入；点击下载会自动保存当前网络草稿。
3. **云端翻译。** DeepL 是可选项。不配置仍可完成本地识别、编辑和原文字幕导出；启动时不访问 Keychain，首次翻译才读取已有 Key。

Whisper 模型建议：

| 模型 | 大约大小 | 适用场景 |
| --- | ---: | --- |
| small | 466 MiB | 8 GB 内存或更快的初步识别 |
| medium | 1.5 GiB | 当前日语节目质量基线，16 GB 及以上内存推荐 |
| large-v3 q5_0 | 1.1 GiB | 质量实验档，适合与 medium 做真实节目对比 |
| large-v3-turbo q5_0 | 547 MiB | 速度/空间实验档 |
| large-v3-turbo q8_0 | 834 MiB | turbo 的较高精度量化档，适合与 q5 对比 |
| Silero VAD v6.2.0 | 865 KiB | 推荐与任一 Whisper 模型配套使用 |

`large-v3-turbo q5_0` 比 medium 文件更小，是因为它属于更快的 turbo 架构并经过 5-bit 量化；`large-v3 q5_0` 则是原始 large-v3 的 q5 量化版。文件大小不能直接代表识别质量。所有内建下载项都有固定 SHA-256，镜像内容与预期不一致时不会安装。

Finder 启动的 App 不会执行 `.zshrc`，因此终端中的 `proxy_on` 不一定传给它。透明/TUN 代理通常可以直接生效；使用本地 HTTP 代理端口时，请在 App 设置中填写，例如 `http://127.0.0.1:7897`。模型镜像失败后会回退 Hugging Face 官方源。

DeepL API Key 在 macOS 写入 Keychain；SQLite 只保存 provider、其他非敏感设置及“曾成功保存 Key”的状态标记，任务目录也不会复制 Key。启动和设置页不读取 Keychain；旧版已保存的 Key 如需显示状态，可重新填写并保存一次。Windows 版本将使用 Credential Manager，Linux 版本将使用 Secret Service。`DEEPL_AUTH_KEY` 仅作为开发兼容回退。

## 基本使用

1. 在“设置”中选好或下载 Whisper 模型，建议同时启用 VAD；需要中文时再配置 DeepL。
2. 在首页选择节目语言、本地媒体、识别模型，以及同语言的可选词表，创建转写任务。
3. 等待任务完成后进入工作区，点击时间码定位原音，检查并修正原文。
4. 单段重译或批量翻译；也可以直接人工编辑简体中文译文。
5. 导出四份字幕文件，或选择原文、译文、双语样式烧录视频。

删除一个已结束的任务会移除 Atogaki 管理的 SQLite 记录与任务派生产物，不会删除最初导入的媒体，也不会删除共用模型。模型下载中的 `.part` 临时文件会在 App 下次启动时清理；系统临时目录仍由 macOS 自行管理。

硬字幕必须重新编码视频。内置 FFmpeg 依次尝试：

1. Apple VideoToolbox H.264；
2. FFmpeg 原生 LGPL MPEG-4 软件编码；
3. 两者都失败时保留错误与 ASS 快照，并明确将烧录标记为失败。

分发版不包含 GPL `libx264`。因此软件回退生成的文件通常会比 x264 更大，但 VideoToolbox 故障不会直接让导出失去回退路径。

## 本地数据与隐私

macOS 正式数据目录通常为：

```text
~/Library/Application Support/com.chai-yinfeng.atogaki/
├── atogaki.sqlite
├── jobs/
└── models/
```

可在开发或隔离测试时使用绝对路径覆盖：

```bash
ATOGAKI_DATA_DIR=/private/tmp/atogaki-isolated-test \
  cargo run --manifest-path src-tauri/Cargo.toml
```

产品遵循以下边界：

- 只处理用户在本机拥有或有权使用的媒体，不提供受限制内容下载或绕过机制。
- 本地识别不上传媒体；云端翻译只接收字幕文本与为连贯性所需的局部上下文。
- 原始媒体不由任务删除操作管理；密钥不进入 SQLite、日志或任务快照。
- 任务参数、词表快照和导出来源可追溯，重试不会覆盖原任务和人工修改。

## 从源码构建桌面 App

需要 Rust、Node.js/npm、Tauri CLI，以及用于构建 sidecar 的 Xcode Command Line Tools、CMake、Meson、Ninja、pkg-config、Autoconf/Automake/Libtool。sidecar 构建脚本会下载并校验固定版本源码。

```bash
npm --prefix ui ci
npm --prefix ui run build
./scripts/build-sidecars-macos.sh
cargo check --manifest-path src-tauri/Cargo.toml
cargo run --manifest-path src-tauri/Cargo.toml
```

构建 ad-hoc 签名、未公证的 App 或 DMG：

```bash
tauri build --bundles app
# 仅用于验证 DMG 结构；会跳过 Finder 图标布局
CI=true tauri build --bundles dmg
# 最终发布 DMG；使用 tauri.conf.json 中的 Finder 布局
tauri build --bundles dmg
```

产物位于 `src-tauri/target/release/bundle/`。详细窗口回归见 [`docs/desktop-testing.md`](docs/desktop-testing.md)，发布资产和 LGPL 源码归档见 [`docs/releasing.md`](docs/releasing.md)。

根目录和 Tauri 目录各有一个 Cargo package，因此会出现两个构建目录：

- `target/`：根 package 的 Rust 核心、CLI、测试和旧 Web API 构建缓存。
- `src-tauri/target/`：桌面 package、Tauri App/DMG 和相关构建缓存。

两者都不是源码，也不会提交到 Git；可以在不需要增量编译时分别用对应 manifest 的 `cargo clean` 重建。

## CLI（开发与兼容入口）

CLI 仍用于核心开发、自动化和故障定位，但不再是面向普通用户的主要产品形态：

```bash
cargo run -- --help
cargo run -- process input.mp4 --model /path/to/ggml-medium.bin
cargo run -- render input.mp4 atogaki_jobs/job-... --output output.mp4
```

CLI 可通过参数或 `ATOGAKI_FFMPEG`、`ATOGAKI_WHISPER_CLI`、`ATOGAKI_WHISPER_MODEL`、`ATOGAKI_VAD_MODEL`、`ATOGAKI_GLOSSARY` 和 `DEEPL_AUTH_KEY` 覆盖开发环境。打包 App 默认使用 Bundle 内的 sidecar 和 App 自己的设置，不要求用户配置这些环境变量。

## 代码库构成

```text
Atogaki_Sub/
├── src/                    Rust 处理核心、CLI 与早期 Web API
│   ├── application/        任务编排和本地服务
│   ├── domain/             字幕、分段、词表和导出规则
│   ├── infrastructure/     FFmpeg、Whisper、provider、SQLite 与文件系统
│   └── interface/          CLI / HTTP 输入边界
├── src-tauri/              Tauri 桌面主进程、系统集成和打包配置
│   ├── binaries/           按 target triple 命名的本地 sidecar
│   └── third-party/        构建清单、许可证与生成的第三方声明
├── ui/                     TypeScript/CSS 桌面界面
├── assets/glossaries/      可复用的内建识别词表
├── migrations/             Postgres 与 SQLite schema 迁移
├── scripts/                sidecar 构建、许可证审计和发布源码归档
└── docs/                   产品方向、路线图、测试、发布与架构决策
```

桌面 App 复用 `src/` 的 application/domain/infrastructure 层。`serve` 与 Postgres schema 是早期探索，不是当前桌面 MVP 的主架构。

## 开发准则与当前边界

- 正确性、可编辑性和可回看性优先于实时性；实时辅助属于后续阶段。
- 时间轴和人工修正是一等数据，重新识别与重试应派生新任务而非覆盖旧结果。
- 首批语言组合是日语或英语识别、简体中文翻译；任务会持久化语言对，provider 与领域接口不写死某一种组合。
- 首个平台是 macOS Apple Silicon；x86_64 macOS 和 Windows 需要独立构建、许可证审计与真实设备回归。
- 当前没有账号、云端文件托管、跨设备同步或自动媒体下载。

项目方向见 [`docs/product-direction.md`](docs/product-direction.md)，完成度和技术债见 [`docs/roadmap.md`](docs/roadmap.md)。适合后续补充的公开材料包括 App 截图/短演示、已知问题、测试设备矩阵、贡献指南和稳定 Release 下载入口；这些应在首轮功能回归与外部测试反馈稳定后加入。

## 许可证

Atogaki 自有源码、文档和构建配置使用 [Apache License 2.0](LICENSE)。第三方 crate、前端包、sidecar、模型和其他材料遵循各自许可证；详见 [`src-tauri/third-party/README.md`](src-tauri/third-party/README.md) 和 [`docs/third-party-license-audit.md`](docs/third-party-license-audit.md)。
