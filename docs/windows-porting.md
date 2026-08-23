# Windows x86_64 兼容计划

_状态：W1/W2 与 NSIS 自动发布基线完成，首个 Windows prerelease 已发布，W3/W4 多设备实测进行中；最后更新：2026-08-23_

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

已经确认的实机缺口：

- Explorer 文件选中路径已经实现并通过 Windows 编译；中文、空格、长路径、非系统盘和现有 Explorer 进程下的窗口行为仍需实机验证。
- 悬浮字幕的非 macOS 路径尚未在 Windows 普通桌面、多显示器、任务栏和最小化／关闭流程中验证。
- 部分错误和能力文案直接提到 VideoToolbox 或 Finder，需要按实际平台显示；实时录音代码仍固定使用 AVFoundation，但实时能力不属于本阶段。
- Windows NSIS 类型、Release 命名和校验流程已经固定；README 的平台入口与面向非开发者的安装说明仍需随扩测反馈继续完善。

## 实施顺序

### W1. 建立原生编译基线

- [x] 准备可重复使用的 Windows x86_64 CI runner；Tauri 最终安装包和真实窗口回归仍必须在 Windows 原生环境完成。
- [x] 在不引入 sidecar 的前提下通过根 Rust 测试、`src-tauri` 编译和前端生产构建，清理首批平台文件锁和构建资源问题。
- [x] 固定首轮 Rust、Node/npm、Tauri、MSVC Build Tools 和 WebView2 的构建前置条件；依赖下载失败与代理行为继续按现有开发工作流记录。
- [x] 建立最小自动化基线；当前只做 compile/test，不自动生成安装包。

首个基线由 `.github/workflows/windows-compile.yml` 承载：使用 Windows Server 2022 x86_64 runner、Rust 1.95.0 和 Node.js 22，运行前端锁定依赖构建、共享 Rust 测试与 Tauri 桌面编译。Tauri 编译步骤通过仅对该步骤生效的 `TAURI_CONFIG` merge patch 把 `externalBin` 覆盖为空，避免 build script 在 `cargo check` 阶段要求尚未产生的 Windows sidecar；正式配置和后续打包仍要求三个真实二进制。它刻意不调用 `tauri build`，也不生成或上传安装包；三个 Windows sidecar 和安装包属于 W2/W4，在没有合规二进制前不使用占位文件伪造打包成功。

基线采用路径过滤的合并门禁：普通功能分支 push 不自动运行；修改共享 Rust、Tauri、前端或构建依赖的 PR 与进入 `main` 的提交会运行，纯文档和无关文件跳过，也可通过 `workflow_dispatch` 手动检查任意候选。这样保留跨平台编译反馈，但不要求以 Windows 为日常开发环境。

**出口：** 共享核心测试和 Tauri 桌面代码能在 Windows x86_64 编译；失败项已区分为代码问题、sidecar 缺失或环境门禁。

2026-08-22 W1 出口已通过：Windows Server 2022 x86_64 runner 完成前端生产构建、71 个共享核心测试和 Tauri 桌面壳编译。首次运行发现 Windows 不允许删除仍被 SQLite pool 打开的文件，现有磁盘数据库测试会显式等待连接池关闭；Tauri 编译还要求 Windows ICO，并会在 build script 阶段校验 externalBin。仓库已增加由现有 App 图标生成的多尺寸 ICO，W1 则用步骤级配置 merge patch 延后尚未构建的 sidecar。该结果只证明源码可原生编译，不代表安装包、sidecar 或窗口体验已验收。

### W2. 固定 Windows sidecar 与合规产物

- [x] 为固定版本 whisper.cpp 构建 CPU `whisper-cli.exe`。
- [x] 为固定版本 FFmpeg 构建 `ffmpeg.exe`／`ffprobe.exe`，包含 libass 和 MPEG-4 encoder，不启用 GPL、nonfree 或 libx264。
- [x] 检查 DLL 依赖，拒绝 MSYS2、MinGW runtime 和动态字幕栈 DLL；三个 sidecar 独立打包，不要求安装机配置开发工具 `PATH`。
- [x] 生成 `x86_64-pc-windows-msvc` 对应的 build manifest、第三方声明、许可证目录和对应源码归档，不复用 macOS target 报告。
- [x] 用目标后缀命名三个 sidecar，并在 current-user NSIS 静默安装后启动它们，检查 Whisper、ASS filter 与 MPEG-4 encoder，再完成静默卸载。

Windows sidecar 工具链已由决策记录 0030 固定：Tauri／Rust 与 whisper.cpp 使用 MSVC，FFmpeg／libass 字幕栈使用 MSYS2 UCRT64／MinGW-w64，并优先静态纳入 sidecar。构建必须检查没有意外的工具链 DLL 依赖；进程间只通过参数、文件、标准流和退出状态通信，不跨 ABI 共享对象或内存。

**出口：** 干净 Windows 设备能直接运行三个内置 sidecar；能力检查显示 Whisper 可用、ASS filter 可用、MPEG-4 可用，并且许可证与对应源码材料可追溯。

2026-08-22 当前自动化出口由 [Windows sidecars 32581475065](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/32581475065) 通过。流水线固定 Node.js 22、Rust 1.95.0、Tauri CLI 2.11.0、MSVC 和 MSYS2 UCRT64，离线锁定构建 NSIS，并在安装目录检查主程序 PE GUI 子系统、运行三个 sidecar、检查 Rust／前端／sidecar 合规资源和卸载结果。上传的 `Atogaki-windows-x86_64-unsigned-nsis` Artifact 约 27.6 MB，`Atogaki-windows-x86_64-sidecars` 约 75.9 MB，后者包含二进制、许可证和对应源码；两者当前保留到 2026-09-05。CI runner 上的安装冒烟证明包内闭合性，但 W2 的“干净用户设备”最终确认与 W3/W4 仍合并在 Windows 11 实机执行。

2026-08-23 的[发布流水线 32622817441](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/32622817441)从 `v0.1.0-alpha.6` 固定 tag 冷构建并通过相同门禁，随后由最小写权限的独立 job 核验 tag、产物哈希与发布说明，创建[首个 Windows x86_64 prerelease](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/tag/v0.1.0-alpha.6)。Release 同时提供带版本名的 NSIS、SHA-256、对应源码包及其 SHA-256；Actions Artifact 继续只用于开发候选。

完整流水线不跟随普通产品代码 push：Windows 构建脚本、sidecar 版本、合规生成器、安装冒烟或 Windows 专属打包配置变化时，PR／`main` 自动验证；选定的 Windows 稳定候选则从目标 commit 手动触发 `workflow_dispatch`。日常 macOS 开发因此只在合并边界承担编译门禁，不持续生成约 27.6 MB 的临时安装包。

### W3. 平台系统集成

- 固定 Tauri Windows 安装包类型、产品名、版本升级标识、图标和应用数据目录；安装与卸载不得触碰用户原媒体。
- 回归 Windows Credential Manager 的保存、检查、更新和删除，确认错误链不回显 Key。
- 回归原生媒体／模型／目录选择器、长路径、空格、中文文件名、盘符路径和媒体重新定位。
- 实现 Explorer 定位导出字幕和烧录视频；失败时提供可理解的路径与错误，不显示 Finder 文案。
- 回归环境／直连／自定义代理、HTTPS 镜像、官方回退、SHA-256 校验和模型目录权限。
- 用 WebView2 验证视频播放、`audio.wav` 回退、任务切换销毁和本地 asset protocol 范围。
- 验证悬浮字幕置顶、拖动、缩放、关闭、任务栏行为和主窗口退出；不复制 macOS NSPanel 或 activation policy。

**出口：** 设置、凭据、下载、文件选择、播放和悬浮字幕在普通 Windows 桌面形成稳定平台行为。

首个测试版只生成 NSIS，并允许以明确标注的未签名 alpha 交付知情测试者；Actions Artifact 不自动等同于公开 Release。Windows 11 x86_64 是首版正式验收范围，Windows 10 在取得实测结论前只做尽力兼容。正式公开推广前重新评估受信任代码签名证书。

2026-08-22 完成第一批 W3 代码准备：导出字幕和烧录视频可通过 `explorer.exe /select,` 选中文件，界面不再在 Windows 显示 Finder 操作或无意义的 VideoToolbox 不可用状态；Windows 首版 MPEG-4 被记录为正常软件编码，只有 macOS 的硬件尝试失败才保存 VideoToolbox fallback reason。当前提交已通过[原生 Windows 编译与共享测试](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/32575359911)，实际 Explorer、WebView2 和窗口行为按 [`windows-testing.md`](windows-testing.md) 执行。

2026-08-22 第一轮 Windows 11 实机已确认安装包可启动、模型下载进入实际传输、翻译 provider API 可用；同时发现主程序启动会附带 Console 窗口。修复后 Release 主程序使用 Windows GUI 子系统，所有运行时 Whisper／FFmpeg／ffprobe 调用统一带 `CREATE_NO_WINDOW`，避免后续识别、媒体探测和烧录再次闪出终端。CI 已对安装后主程序增加 PE 子系统门禁；2026-08-23 修复候选实机测试未再发现其他问题，并以 `v0.1.0-alpha.6` 进入多人扩测。

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
