# Atogaki

Atogaki 是一个面向中文使用者的、本地优先的外语音视频理解与学习工作台。它通过识别、翻译、字幕校对、词典与可回看的媒体时间轴，把用户本机已有的节目整理成可以理解、复习和导出的双语资料。项目最初用于整理ヨルシカ「後書き」电台；字幕编辑、样式和烧录是建立可信资料的基础能力，但 Atogaki 目前不以替代完整视频字幕制作软件为目标。

项目目前处于预发布阶段。macOS Apple Silicon 是日常开发和完整体验基线，最新测试版为 [`v0.1.0-alpha.8`](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/tag/v0.1.0-alpha.8)，采用 ad-hoc 签名且尚未公证。Windows 11 x86_64 已发布较早的未签名 [`v0.1.0-alpha.6`](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/tag/v0.1.0-alpha.6)，但不包含此后加入的学习区、词典和字幕样式等能力；Windows 会在选定候选完成专项实机测试后再更新。Atogaki 是独立的个人开发项目，与ヨルシカ及其官方运营方没有隶属、授权或赞助关系。

## 能做什么

- 导入本机音频或视频，用内置 `whisper-cli` 和用户下载的 Whisper 模型在本地识别日语、英语或韩语。
- 使用 Silero VAD 过滤静音、音乐和环境声，并通过词表提高人名与作品名的一致性。
- 在播放器中按时间轴查看、跳转和编辑字幕；原文修改后会标记对应译文为待重译。
- 可选使用 DeepL 或 DeepSeek 将日语、英语或韩语翻译为简体中文；也可填写自定义 OpenAI-compatible Base URL 与模型。
- 在任务内字幕编辑器中用波形调整时间、切分／合并字幕，并保留会话撤销。
- 为原文和译文设置字体、字号、颜色、描边、背景、位置与边距；FFmpeg/libass 真实预览、双语 ASS 和视频烧录共享同一套样式。
- 在“收听”中连续阅读并按当前句跟随播放；选择词、短语、语法或整句保存到按语言分册的“学习”区域，并返回原节目时间。
- 查询用户自行下载的 JMdict、Tomoshi、ECDICT 离线词典，或使用用户配置的 Merriam-Webster API；可选择词典释义作为学习条目的简明译义。
- 按需导出原文、简体中文或双语 SRT/ASS，或把原文、译文、双语字幕烧录到 MP4。
- 记录任务、失败阶段、重试来源和烧录历史；重启后会识别被中断的任务，并允许从冻结参数创建新任务重试。
- 持久化工作台任务顺序；模型下载中断后可在下次启动通过 HTTP Range 安全续传。

媒体、模型、词典包、任务、学习资料和编辑结果默认保留在设备上。只有启用云端字幕翻译时，待翻译的原文字幕及有限的相邻上下文会发送给所选 provider；只有用户主动查询在线词典时，所选文本才会发送给对应词典 API。

## 安装与首次配置

当前首先支持 Apple Silicon Mac。普通测试者应从 [GitHub Releases](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases) 下载带版本名的 DMG 和相邻 SHA-256；开发者也可以按下文从源码构建。DMG 不提交到 Git 历史。

macOS 测试版没有 Developer ID 签名或公证。核对下载来源与 SHA-256 后，首次打开若被 Gatekeeper 阻止，可在 Finder 中右键 App 选择“打开”。Windows `alpha.6` 安装包同样没有商业代码签名，SmartScreen 可能显示“未知发布者”；不要为运行测试版而全局关闭系统安全功能。

首次打开 App 时，启动配置会引导完成三部分；学习词典可在进入 App 后按需配置：

1. **本地模型。** 选择已有 `ggml-*.bin`，或下载 Whisper 与 Silero VAD。App 管理的默认目录通常是 `~/Library/Application Support/com.chai-yinfeng.atogaki/models/`，界面会显示本机的实际路径。
2. **网络。** 选择跟随启动环境、强制直连或自定义 HTTP/HTTPS 代理；也可以填写 HTTPS 模型镜像。连接测试使用当前输入；点击下载会自动保存当前网络草稿。
3. **云端翻译。** 可选择 DeepL、DeepSeek 或高级 OpenAI-compatible 入口。不配置仍可完成本地识别、编辑和原文字幕导出；启动时不访问 Keychain，首次翻译才读取当前 provider 的已有 Key。LLM 入口还可配置模型与口语风格。

学习词典与字幕翻译相互独立。JMdict、Tomoshi 和 ECDICT 由用户在设置页明确下载到 `dictionaries/`；Merriam-Webster Key 单独保存到系统凭据库。Collins 目前只有配置边界而没有可用查询 adapter，Cambridge 因官方确认 API 不可用已从产品路径移除。

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

各 provider 的 API Key 在 macOS 按独立条目写入 Keychain，在 Windows 写入 Credential Manager；SQLite 只保存 provider、模型、端点、风格等非敏感设置及“曾成功保存 Key”的状态标记，任务目录也不会复制 Key。启动和普通设置加载不读取系统凭据；忘记是否保存过时，可切换到对应 provider 后点击“检查所选 Key”，它只确认系统凭据条目是否存在，不会回显 Key 或调用翻译 API。未来 Linux 版本使用 Secret Service。`DEEPL_AUTH_KEY` 和 `DEEPSEEK_API_KEY` 仅作为开发兼容回退。

## 基本使用

1. 在“设置”中选好或下载 Whisper 模型，建议同时启用 VAD；需要中文时再配置一个翻译 provider。
2. 在首页选择节目语言、本地媒体、识别模型，以及同语言的可选词表，创建转写任务。
   随 App 提供的内置词表会按仓库 TXT 内容自动升级；第一次编辑内置词表时，App 会创建独立的自定义副本，后续升级不会覆盖该副本或旧任务的词表快照。
3. 等待任务完成后进入任务详情，在“翻译与词表／字幕校对／导出成品”之间工作；需要精确时间和结构编辑时进入任务内字幕编辑器。
4. 单段重译或批量翻译，也可以直接人工编辑简体中文译文；在导出区调整字幕样式并查看真实渲染预览。
5. 进入“收听”连续阅读和播放；选择内容保存到“学习”，按词典来源查看释义并可返回原句。
6. 按需导出原文、译文、双语 SRT/ASS，或选择原文、译文、双语字幕烧录视频。

删除一个已结束的任务会移除 Atogaki 管理的 SQLite 记录与任务派生产物，不会删除最初导入的媒体、共用模型或已经保存的学习条目快照。模型下载中的合法 `.part` 会跨重启保留，并在服务器支持且响应边界正确时断点续传；摘要或 Range 响应不可信时不会把临时文件安装为模型。系统临时目录仍由 macOS 自行管理。

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
├── models/
└── dictionaries/
```

可在开发或隔离测试时使用绝对路径覆盖：

```bash
ATOGAKI_DATA_DIR=/private/tmp/atogaki-isolated-test \
  cargo run --manifest-path src-tauri/Cargo.toml
```

产品遵循以下边界：

- 只处理用户在本机拥有或有权使用的媒体，不提供受限制内容下载或绕过机制。
- 本地识别不上传媒体；云端翻译只接收字幕文本与为连贯性所需的局部上下文。
- 离线词典查询不上传选词；在线词典只在用户主动查询时接收当前选词，不接收整段媒体。
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

构建 ad-hoc 签名、未公证的 App 或 DMG：

```bash
tauri build --bundles app
# 仅用于验证 DMG 结构；会跳过 Finder 图标布局
CI=true tauri build --bundles dmg
# 最终发布 DMG；使用 tauri.conf.json 中的 Finder 布局
tauri build --bundles dmg
```

产物位于 `src-tauri/target/release/bundle/`。详细窗口回归见 [`docs/desktop-testing.md`](docs/desktop-testing.md)，发布资产和 LGPL 源码归档见 [`docs/releasing.md`](docs/releasing.md)，下一阶段的 Windows 工作分解见 [`docs/windows-porting.md`](docs/windows-porting.md)。

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

- 正确性、可编辑性和可回看性优先于实时性；实时辅助属于后续阶段。
- 时间轴和人工修正是一等数据，重新识别与重试应派生新任务而非覆盖旧结果。
- 首批语言组合是日语、英语或韩语识别、简体中文翻译；任务会持久化语言对，provider 与领域接口不写死某一种组合。
- macOS Apple Silicon 是当前主要质量基线；Windows 11 x86_64 已有独立构建和预发布基线，但只在选定候选完成许可证审计与真实设备回归后更新。x86_64 macOS 尚未列入当前主线。
- 当前没有账号、云端文件托管、跨设备同步或自动媒体下载。

项目方向见 [`docs/product-direction.md`](docs/product-direction.md)，完成度和技术债见 [`docs/roadmap.md`](docs/roadmap.md)。适合后续补充的公开材料包括 App 截图/短演示、已知问题、测试设备矩阵、贡献指南和稳定 Release 下载入口；这些应在首轮功能回归与外部测试反馈稳定后加入。

## 许可证

Atogaki 自有源码、文档和构建配置使用 [Apache License 2.0](LICENSE)。第三方 crate、前端包、sidecar、模型和其他材料遵循各自许可证；详见 [`src-tauri/third-party/README.md`](src-tauri/third-party/README.md) 和 [`docs/third-party-license-audit.md`](docs/third-party-license-audit.md)。
