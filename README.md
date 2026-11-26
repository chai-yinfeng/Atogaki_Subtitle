# Atogaki

Atogaki 是一个面向中文使用者的、本地优先的外语音视频理解与学习工作台。它通过识别、翻译、字幕校对、词典与可回看的媒体时间轴，把用户本机已有的节目整理成可以理解、复习和导出的双语资料。项目最初用于整理ヨルシカ「後書き」电台；字幕编辑、样式和烧录是建立可信资料的基础能力，但 Atogaki 目前不以替代完整视频字幕制作软件为目标。

macOS Apple Silicon 是日常开发和完整体验基线。正式版采用 ad-hoc 签名，尚未公证，最低要求 macOS 12.0。Windows 开发、自动 CI 与后续打包已从 2026-11-13 暂停；既有 Windows 11 x86_64 未签名历史基线为 [`v0.1.0-alpha.10`](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/tag/v0.1.0-alpha.10)，悬浮字幕仍不稳定，不建议使用。Atogaki 是独立的个人开发项目，与ヨルシカ及其官方运营方没有隶属、授权或赞助关系。

## 功能划分与架构

[![Atogaki 桌面架构。Tauri 主进程连接离线任务、字幕与学习用例、在线字幕会话和本机媒体服务。](docs/diagrams/atogaki-architecture.svg)](docs/diagrams/atogaki-architecture.html)

[交互架构图（下载 HTML 后用浏览器打开）](docs/diagrams/atogaki-architecture.html) · [Archify 图源](docs/diagrams/atogaki-architecture.json) · [架构详解](docs/architecture.md)

| 功能区 | 职责与边界 |
| --- | --- |
| **工作台** | 导入本机音视频，安排识别、对齐和翻译任务。支持从已提交检查点恢复。完整原文进入统一字幕编辑器，可校对、调整时间、局部重识别和重译。候选经确认后才更新当前字幕。 |
| **收听** | 播放已有任务，跟随双语字幕、定位原句并收藏。视频全屏支持双语和仅译文字幕轨。 |
| **学习** | 管理词句收藏、词典释义和可回听的例句，支持导出 Anki TSV。可从日语字幕查词、看活用、分析句型，也可浏览 N5 至 N1 的中文语法目录和基础概念。 |
| **在线字幕** | 独立会话采集系统声音，用本地模型识别和翻译，并在主窗口或浮窗显示可修订字幕。v0.1.4 有 One Pass／Two Pass 入口，实时语义和延迟仍待验收。Gemini／百炼实时翻译、保存录音并转离线任务都还在研究或规划中。 |

**文件任务**将媒体转成带时间轴的原文，可再按任务配置使用本地或云端文本翻译。当前编辑稿、学习资料与来源关系保存在 SQLite，任务目录另存派生音频、识别产物与恢复检查点，凭据保存在系统钥匙串。Gemini 局部重识别另行确认具体音频范围。首批文件工作流支持日语、英语或韩语识别并译为简体中文，日语节目是首要质量基线。详见[数据与隐私](#本地数据与隐私)。

**导出**位于工作台的任务内。独立导出支持 SRT／ASS，以及带可切换字幕轨的 MKV。烧录导出把字幕写入视频画面，可直接输出 MP4，也可输出 ProRes 中间文件供后期压制。发行进度见[路线图](docs/roadmap.md)。

## 安装与首次配置

macOS 首次安装请下载带版本号的 arm64 DMG 和同名 `.sha256` 文件。已安装 v0.1.4 或更新的正式版，可到「设置 → 更新」检查并安装。Windows 继续使用 alpha.10 的 NSIS 安装器。开发者也可以按下文从源码构建。[下载最新 macOS 正式版](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/latest) · [发行记录](docs/release-notes)

macOS 版本没有 Developer ID 签名或公证。核对下载来源与 SHA-256 后，首次打开若被 Gatekeeper 阻止，可在 Finder 中右键 App 选择“打开”。Windows `alpha.10` 安装包同样没有商业代码签名，SmartScreen 可能显示“未知发布者”；不要为运行测试版而全局关闭系统安全功能。

首次打开 App 时，启动配置会引导完成三部分；学习词典可在进入 App 后按需配置：

1. **本地模型。** 选择已有 `ggml-*.bin`，或下载 Whisper 与 Silero VAD。App 管理的默认目录通常是 `~/Library/Application Support/com.chai-yinfeng.atogaki/models/`，界面会显示本机的实际路径。
2. **网络。** 选择跟随启动环境、强制直连或自定义 HTTP/HTTPS 代理；也可以填写 HTTPS 模型镜像。连接测试使用当前输入；点击下载会自动保存当前网络草稿。
3. **翻译方式。** 可选择仅转录、混元本地翻译、DeepL、DeepSeek、Gemini 或高级 OpenAI-compatible 入口。「设置 → 翻译」可下载 Hy-MT3 1.8B，供文件翻译和实时字幕共用。不配置翻译仍可完成本地识别、编辑和原文字幕导出；首次需要云端翻译时才读取对应 Key。

学习词典与字幕翻译相互独立。JMdict、Tomoshi 和 ECDICT 由用户在设置页明确下载到 `dictionaries/`；Merriam-Webster Key 单独保存到系统凭据库。Collins 未取得可测试的 API 账户，Cambridge API 当前不可用，两者均不出现在产品路径中。

v0.1.4 提供 Qwen 离线转录与强制对齐入口，使用时需准备识别包、ForcedAligner 和对应运行环境。准备与边界见[0084](docs/decisions/0084-offline-qwen-and-forced-alignment.md)。

Whisper 模型建议：

| 模型 | 大约大小 | 适用场景 |
| --- | ---: | --- |
| small | 466 MiB | 8 GB 内存或更快的初步识别 |
| medium | 1.5 GiB | 当前日语节目质量基线，16 GB 及以上内存推荐 |
| large-v3 q5_0 | 1.1 GiB | 质量实验档，适合与 medium 做真实节目对比 |
| large-v3-turbo q5_0 | 547 MiB | 速度/空间实验档 |
| large-v3-turbo q8_0 | 834 MiB | turbo 的较高精度量化档，适合与 q5 对比 |
| Silero VAD v6.2.0（GGML） | 865 KiB | 与 Whisper 文件转录配套 |
| Silero VAD v6.3（ONNX） | 2.20 MB | 实时字幕判断说话和停顿，在「设置 → 识别」下载后下次启动会话生效 |

`large-v3-turbo q5_0` 比 medium 文件更小，是因为它属于更快的 turbo 架构并经过 5-bit 量化；`large-v3 q5_0` 则是原始 large-v3 的 q5 量化版。文件大小不能直接代表识别质量。所有内建下载项都有固定 SHA-256，镜像内容与预期不一致时不会安装。

Finder 启动的 App 不会执行 `.zshrc`，因此终端中的 `proxy_on` 不一定传给它。透明/TUN 代理通常可以直接生效；使用本地 HTTP 代理端口时，请在 App 设置中填写，例如 `http://127.0.0.1:7897`。模型镜像失败后会回退 Hugging Face 官方源。

自定义代理是显式的网络边界，Atogaki 不会在代理失效时静默绕过它。如果翻译错误包含 `tunnel error` 或 `Connection refused`，请启动对应代理、修正监听端口，或在设置中选择“直连”；错误信息会同时标出当前网络模式和可执行的处理方式。

各 provider 的 API Key 在 macOS 按独立条目写入 Keychain，在 Windows 写入 Credential Manager；SQLite 只保存 provider、模型、端点、风格等非敏感设置及“曾成功保存 Key”的状态标记，任务目录也不会复制 Key。启动和普通设置加载不读取系统凭据；忘记是否保存过时，可切换到对应 provider 后点击“检查所选 Key”，它只确认系统凭据条目是否存在，不会回显 Key 或调用翻译 API。未来 Linux 版本使用 Secret Service。`DEEPL_AUTH_KEY` 和 `DEEPSEEK_API_KEY` 仅作为开发兼容回退。

## 用日语字幕学习

以下入口适用于包含日语学习功能的版本。从「设置 → 词典」下载 **Sudachi 日语分词**。需要中文释义时安装 **Tomoshi**，需要英文释义时安装 **JMdict**。第一次查词可能需要等待词库准备完成。

1. **查词。** 在「收听」打开日语节目，点击或框选日语原文。点候选词查看词典形、读音和释义。有明确的活用时会显示「本句词形」。找到要保存的释义后点「收藏」。
2. **分析句型。** 点字幕旁的「分析句型」，也可以框选日语原文后再分析。结果会高亮匹配到的部分并显示「本句用法」。点「收藏语法例句」可保存原句。暂未识别的表达可到「学习 → 日语语法」按句型或 JLPT 等级查阅。
3. **回听与复习。** 在「学习」点「播放例句」，即可回到原节目。收藏可以导出为 Anki TSV。想记下识别有误或遗漏的位置，可在分析结果中点「标记误报」或「标记漏报」，之后在学习区搜索「反馈」回看。反馈保存在本机，Anki 文件只包含收藏。

## 基本使用

1. 在“设置”中选好或下载 Whisper 模型，建议同时启用 VAD；需要中文时再配置一个翻译 provider。
2. 在首页选择节目语言、本地媒体、识别模型，以及同语言的可选词表，创建转写任务。
   随 App 提供的内置词表会按仓库 TXT 内容自动升级；第一次编辑内置词表时，App 会创建独立的自定义副本，后续升级不会覆盖该副本或旧任务的词表快照。
3. 完整识别且已有字幕的任务直接进入统一字幕编辑器；翻译、词表、候选复核和导出按需打开，共用播放器和当前编辑位置。处理中任务保留只读进度与部分原文。
4. 单段重译或批量翻译，也可以直接人工编辑简体中文译文；在导出区调整字幕样式并查看真实渲染预览。
5. 进入“收听”连续阅读和播放；选择内容保存到“学习”，按词典来源查看释义并可返回原句。
6. 按需导出原文、译文、双语 SRT/ASS，或选择原文、译文、双语字幕烧录视频。

删除一个已结束的任务会移除 Atogaki 管理的 SQLite 记录与任务派生产物，不会删除最初导入的媒体、共用模型或已经保存的学习条目快照。模型下载中的合法 `.part` 会跨重启保留，并在服务器支持且响应边界正确时断点续传；摘要或 Range 响应不可信时不会把临时文件安装为模型。系统临时目录仍由 macOS 自行管理。

硬字幕必须重新编码视频。按码率烧录时，内置 FFmpeg 依次尝试：

1. Apple VideoToolbox H.264；
2. FFmpeg 原生 LGPL MPEG-4 软件编码；
3. 两者都失败时保留错误与 ASS 快照，并明确将烧录标记为失败。

画质优先 MP4 使用 VideoToolbox，ProRes 中间文件使用独立编码器。两种方式需要对应的编码能力。分发版 FFmpeg 使用 LGPL 配置。

## 本地数据与隐私

macOS 正式数据目录通常为：

```text
~/Library/Application Support/com.chai-yinfeng.atogaki/
├── atogaki.sqlite
├── jobs/
├── models/
├── dictionaries/
└── themes/imported/
```

可在开发或隔离测试时使用绝对路径覆盖：

```bash
ATOGAKI_DATA_DIR=/private/tmp/atogaki-isolated-test \
  cargo run --manifest-path src-tauri/Cargo.toml
```

产品遵循以下边界：

- 只处理用户在本机拥有或有权使用的媒体，不提供受限制内容下载或绕过机制。
- 本地识别不上传媒体；云端翻译只接收字幕文本与为连贯性所需的局部上下文。
- 离线查词、句型分析和学习反馈均在本机处理。在线词典只在用户主动查询时接收当前选词，不接收整段媒体。
- 原始媒体不由任务删除操作管理；密钥不进入 SQLite、日志或任务快照。
- 任务参数、识别词表快照、学习来源和导出来源可追溯，重试不会覆盖原任务和人工修改。

## 从源码构建桌面 App

需要 Rust、Node.js/npm、Tauri CLI，以及用于构建 sidecar 的 Xcode Command Line Tools、CMake、Meson、Ninja、pkg-config、Autoconf/Automake/Libtool。sidecar 构建脚本会下载并校验固定版本源码。

```bash
npm --prefix ui ci
npm --prefix ui run build
./scripts/build-sidecars-macos.sh
cargo check --manifest-path src-tauri/Cargo.toml
cargo run --manifest-path src-tauri/Cargo.toml
```

完整打包还要求 `src-tauri/binaries/llama-server-aarch64-apple-darwin`。固定 runtime 版本与构建参数见 [`hy-mt2-versions.zsh`](scripts/hy-mt2-versions.zsh) 和 [`validate-hy-mt2.zsh`](scripts/validate-hy-mt2.zsh)；后者是会下载模型并执行本地推理的开发验证脚本，普通安装用户无需运行。

开发主线的 complete 配置另包含系统音频采集 helper；手工打包前在仓库根目录运行以下命令。`build-sidecars-macos.sh` 只准备 FFmpeg／FFprobe／whisper-cli，不会自动生成 llama-server 或采集 helper。打包 helper 也不等于已分发 Python／MLX 在线运行环境。

```bash
./scripts/prepare-online-capture.sh src-tauri/binaries/atogaki-audio-tap-aarch64-apple-darwin
```

构建 ad-hoc 签名、未公证的 App 或 DMG：

```bash
cd src-tauri
cargo tauri build --config tauri.macos-complete.conf.json --bundles app,dmg
```

产物位于 `src-tauri/target/release/bundle/`。详细窗口回归见 [`docs/desktop-testing.md`](docs/desktop-testing.md)，发布资产和 LGPL 源码归档见 [`docs/releasing.md`](docs/releasing.md)。[`Windows 记录`](docs/windows-porting.md)仅保留为将来恢复参考。

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

桌面 App 复用 `src/` 的 application/domain/infrastructure 层。`serve` 与 Postgres schema 是早期探索，不是当前桌面产品的主架构。

## 开发准则与当前边界

- 正确性、可编辑性和可回看性优先于实时性；实时辅助已在开发主线接入，质量及稳定发行仍未验收。
- 时间轴和人工修正是一等数据；识别候选明确比较后采用，任务恢复复用已提交检查点。
- 首批语言组合是日语、英语或韩语识别、简体中文翻译；任务会持久化语言对，provider 与领域接口不写死某一种组合。
- macOS Apple Silicon 是当前主要质量基线；Windows `alpha.10` 及其源码／验证材料保留为历史，开发、自动 CI 和后续打包已暂停。Windows 悬浮字幕不稳定且不建议使用；x86_64 macOS 尚未列入当前主线。
- 当前没有账号、云端文件托管、跨设备同步或自动媒体下载。

文档入口见[文档索引](docs/README.md)，项目方向见 [`docs/product-direction.md`](docs/product-direction.md)，当前进度和待验收项见 [`docs/roadmap.md`](docs/roadmap.md)。逐次提交与历史技术债保留在路线图历史快照，研究提案和验收证据分别索引。

## 许可证

Atogaki 自有源码、文档和构建配置使用 [Apache License 2.0](LICENSE)。第三方 crate、前端包、sidecar、模型和其他材料遵循各自许可证；详见 [`src-tauri/third-party/README.md`](src-tauri/third-party/README.md) 和 [`docs/third-party-license-audit.md`](docs/third-party-license-audit.md)。
