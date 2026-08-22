# Windows x86_64 兼容计划

_状态：下一主线；最后更新：2026-08-22_

## 目标与边界

Windows 首版让普通用户在不安装 Rust、Node、FFmpeg、Whisper 或开发工具的情况下，从 Tauri 安装包完成本地音视频理解闭环。稳定基线是 `x86_64-pc-windows-msvc`、CPU Whisper、Windows Credential Manager、WebView2 和带 libass 的 LGPL FFmpeg／MPEG-4 软件烧录。

首版不把以下项目设为门禁：CUDA／DirectML、Windows 专属视觉重做、韩语专项调参、实时系统音频采集、商业代码签名方案，以及完整字幕样式系统。平台 GPU 和韩语优化要等真实 Windows 用户回归后按瓶颈决定。

## 仓库现状

已有的跨平台基础：

- Tauri `externalBin` 已按 target triple 查找 `ffmpeg`、`ffprobe` 和 `whisper-cli`，核心路径解析也会在 Windows 使用 `.exe`。
- `keyring` 已启用 `windows-native`，凭据服务通过统一 `CredentialStore` 暴露 Windows Credential Manager。
- 悬浮字幕已有非 macOS 的普通 Tauri 窗口路径，可作为 Windows 置顶行为的起点。
- 硬字幕运行时先探测编码器；没有 VideoToolbox 时可使用 LGPL MPEG-4，因此 Windows 首版不需要先选择新的硬件 H.264 后端。
- SQLite、任务目录、语言代码、字幕编辑、provider 和导出逻辑位于共享 Rust／前端层，不应为 Windows 复制第二套业务实现。

已经确认的缺口：

- 仓库没有 Windows 构建环境或 CI；现有 sidecar 构建、版本清单和对应源码归档脚本均为 macOS/zsh 专用。
- `src-tauri/binaries/` 目前只有 `aarch64-apple-darwin` 产物；第三方声明和构建清单也只覆盖当前 target。
- “在文件管理器中定位”命令在非 macOS 平台直接报错，需要实现 Explorer 的文件选中或安全降级。
- 悬浮字幕的非 macOS 路径尚未在 Windows 普通桌面、多显示器、任务栏和最小化／关闭流程中验证。
- 部分错误和能力文案直接提到 VideoToolbox 或 Finder，需要按实际平台显示；实时录音代码仍固定使用 AVFoundation，但实时能力不属于本阶段。
- README、发布说明和测试记录目前只有 macOS 构建命令与候选产物，Windows 安装包类型、命名和校验流程尚未固定。

## 实施顺序

### W1. 建立原生编译基线

- 准备可重复使用的 Windows x86_64 构建机或 CI runner；Tauri 最终安装包和真实窗口回归必须在 Windows 原生环境完成。
- 在不引入 sidecar 的前提下先通过根 Rust 测试、`src-tauri` 编译和前端生产构建，清理错误的 `cfg`、路径分隔符、文件名和平台 API 假设。
- 固定 Rust、Node/npm、Tauri、MSVC Build Tools 和 WebView2 的构建前置条件；依赖下载失败与代理行为按现有开发工作流记录。
- 建立最小自动化基线。仓库当前没有 `.github/workflows`，应先验证普通提交的 Windows compile/test，再决定是否自动生成安装包。

首个基线由 `.github/workflows/windows-compile.yml` 承载：使用 Windows Server 2022 x86_64 runner、Rust 1.95.0 和 Node.js 22，运行前端锁定依赖构建、共享 Rust 测试与 Tauri 桌面编译。Tauri 编译步骤通过仅对该步骤生效的 `TAURI_CONFIG` merge patch 把 `externalBin` 覆盖为空，避免 build script 在 `cargo check` 阶段要求尚未产生的 Windows sidecar；正式配置和后续打包仍要求三个真实二进制。它刻意不调用 `tauri build`，也不生成或上传安装包；三个 Windows sidecar 和安装包属于 W2/W4，在没有合规二进制前不使用占位文件伪造打包成功。

**出口：** 共享核心测试和 Tauri 桌面代码能在 Windows x86_64 编译；失败项已区分为代码问题、sidecar 缺失或环境门禁。

### W2. 固定 Windows sidecar 与合规产物

- 为固定版本 whisper.cpp 构建 CPU `whisper-cli.exe`。
- 为固定版本 FFmpeg 构建 `ffmpeg.exe`／`ffprobe.exe`，必须包含 libass 和 MPEG-4 encoder，不启用 GPL、nonfree 或 libx264。
- 检查 DLL 依赖，确保安装机不需要 MSYS2、开发环境或构建机绝对路径；决定静态或随包 DLL 的方式前记录可复现性、许可证和安装体积取舍。
- 生成 `x86_64-pc-windows-msvc` 对应的 build manifest、第三方声明、许可证目录和对应源码归档。macOS 的生成文件不能原样复用。
- 用目标后缀命名三个 sidecar，并验证 Tauri 安装包能在无环境变量和无系统 FFmpeg/Whisper 时启动它们。

Windows FFmpeg／libass 的具体工具链和 DLL 分发方式具有持久影响；在选定实现前应新增决策记录，比较原生 MSVC、MSYS2/MinGW 或其他可复现方案，而不是把一次本机构建隐式变成发布标准。

**出口：** 干净 Windows 设备能直接运行三个内置 sidecar；能力检查显示 Whisper 可用、ASS filter 可用、MPEG-4 可用，并且许可证与对应源码材料可追溯。

### W3. 平台系统集成

- 固定 Tauri Windows 安装包类型、产品名、版本升级标识、图标和应用数据目录；安装与卸载不得触碰用户原媒体。
- 回归 Windows Credential Manager 的保存、检查、更新和删除，确认错误链不回显 Key。
- 回归原生媒体／模型／目录选择器、长路径、空格、中文文件名、盘符路径和媒体重新定位。
- 实现 Explorer 定位导出字幕和烧录视频；失败时提供可理解的路径与错误，不显示 Finder 文案。
- 回归环境／直连／自定义代理、HTTPS 镜像、官方回退、SHA-256 校验和模型目录权限。
- 用 WebView2 验证视频播放、`audio.wav` 回退、任务切换销毁和本地 asset protocol 范围。
- 验证悬浮字幕置顶、拖动、缩放、关闭、任务栏行为和主窗口退出；不复制 macOS NSPanel 或 activation policy。

**出口：** 设置、凭据、下载、文件选择、播放和悬浮字幕在普通 Windows 桌面形成稳定平台行为。

### W4. 离线闭环与安装包候选

- 使用至少一段英语或韩语真实节目运行 Whisper/VAD CPU 识别，记录设备、模型、媒体时长、处理耗时和内存体感，不显示未经验证的 ETA。
- 完成 DeepL 或 DeepSeek 翻译、原文／译文编辑、待重译、词表应用和重启恢复。
- 导出原文、译文、双语 SRT 与双语 ASS；验证中文、日文和韩文字体 fallback。
- 使用内置 FFmpeg 完成原文、译文或双语视频烧录；首版以 MPEG-4 软件编码为验收基线，并记录输出大小与质量。
- 验证任务重命名、删除、失败恢复、从冻结快照重试、原媒体移动后重新定位和应用退出时子进程终止。
- 在最终安装包而非 `cargo run` 中复用 `docs/desktop-testing.md` 的非 macOS 专有场景，并记录所有平台差异。

**出口：** 真实 Windows 用户从安装包独立完成“安装 → 配置 → 导入 → 识别 → 翻译 → 编辑 → 导出／烧录”，没有依赖开发环境的隐藏步骤。

## 最小测试矩阵

| 维度 | 首版要求 |
| --- | --- |
| 系统 | 至少一个仍受支持的 Windows 11 x86_64 实机；增加 Windows 10 前先确认 Tauri/WebView2 支持边界 |
| 安装状态 | 开发机；无 Rust/Node/FFmpeg/Whisper 的干净用户机 |
| 媒体 | 音频一份、常见 MP4 视频一份、WebView2 不支持而回退 `audio.wav` 的样本一份 |
| 路径 | 空格、中文、非系统盘和较长路径 |
| 语言 | 英语或韩语真实闭环；日语共享逻辑做最小回归 |
| 翻译 | 一个已配置云 provider；未配置 provider 时仍能完成原文导出 |
| 网络 | 直连、自定义代理、镜像失败后官方回退、校验失败 |
| 输出 | 四种字幕文件；至少一种带字幕 MP4；Explorer 定位 |
| 窗口 | 主窗口、悬浮字幕、切换任务、最小化、关闭和多显示器基础行为 |

如果 Windows 10、ARM64、CUDA、DirectML 或特定企业代理成为真实需求，分别进入后续矩阵，不在首版验收中用形式化勾选替代实际设备测试。

## 完成与文档同步

每完成 W1–W4 的一个出口，更新 `docs/roadmap.md` 的对应条目和剩余风险。Windows sidecar 工具链确定后新增架构决策；生成的许可证与源码材料按 `docs/third-party-license-audit.md` 审阅。首个候选安装包完成真实设备回归后，再把确定的安装包格式、资产命名、校验和发布步骤写入 `docs/releasing.md`，不要提前把未跑通的命令记录为发布事实。
