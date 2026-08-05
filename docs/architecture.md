# 架构与目录约定

_最后更新：2026-08-05_

## 仓库组织

`Atogaki_Sub` 保持为 Rust/Cargo 项目根目录。不要为了放置文档而把 `src/` 再嵌套一层：根目录的 `Cargo.toml` 与 `src/` 是标准 Rust 布局，移动它们会增加构建、编辑器和未来 Tauri 集成的复杂度。

```text
Atogaki_Sub/
├── Cargo.toml                 # Rust package 与依赖
├── src/                       # 可复用的处理核心与当前 CLI
│   ├── lib.rs                 # 供 CLI 与未来桌面 UI 复用的库入口
│   ├── application/           # 用例、任务规格、状态与编排
│   ├── domain/                # 字幕段、词表、分段与导出规则
│   ├── infrastructure/        # ffmpeg、Whisper、DeepL、文件与数据库适配器
│   └── interface/             # CLI；未来会新增桌面 UI 适配层
├── assets/                    # 版本化的示例和内置词表
├── migrations/                # 现有 Postgres 实验性迁移；不作为桌面 MVP 依赖
├── migrations/sqlite/         # 桌面本地 SQLite 迁移
├── src-tauri/                 # Tauri 桌面壳；依赖根 Rust library
│   ├── binaries/              # 构建时生成、按 target triple 命名的 sidecar
│   └── third-party/           # sidecar 版本、构建清单与许可证
├── scripts/                   # 固定版本 sidecar 构建与分发校验
├── ui/                        # Tauri 内嵌的 TypeScript/Vite 前端
├── docs/                      # 产品方向、路线图、架构与决策记录
└── tests/                     # 跨模块集成测试（按需要新增）
```

未来引入 Tauri 时，优先在本仓库根目录新增 `src-tauri/` 和前端目录（例如 `ui/`），并让当前 Rust 处理核心转为独立库 crate 或 workspace member。不要把现有 `src/` 移入一个人为的子目录。

## 目标桌面架构

```text
桌面 UI（Tauri）
  └─ application：导入、任务、编辑、翻译、导出用例
       ├─ domain：时间轴字幕、词表、字幕格式
       └─ infrastructure：文件系统、SQLite、ffmpeg、ASR、翻译服务
```

UI 不直接启动 ffmpeg、Whisper 或具体翻译服务。它只调用 `application` 中的用例并订阅任务状态。当前 CLI 同样是该应用层的一个适配器。

`LocalTaskService` 是桌面端长任务的第一层服务：提交时立即创建带 `queued` 状态的 UUID 任务目录，后台 worker 再调用 `JobRunner`。UI 通过 `JobSnapshot` 轮询持久化状态。默认仅启动一个 worker，避免本地 ASR 模型争抢 CPU、内存或 GPU；多 worker 只能由显式配置启用。

当前 Tauri 壳共享一个 `LocalDatabase` 实例，并分别注册 `LocalTaskService`、`LocalWorkspaceService` 与 `LocalRenderService`：前者负责创建、排队和同步识别任务；工作区服务负责读取任务详情、保存编辑、调用注入的翻译 provider 和导出；烧录服务负责冻结 SQLite 字幕快照、持久化输出任务、进度与取消。识别和烧录各使用一个本地 worker，状态互不污染。界面通过原生文件选择器获取媒体、Whisper 模型和 Silero VAD 模型路径；也可由设置页下载官方模型到应用数据目录。VAD 默认开启但允许显式关闭。打开任务后，Tauri 只将该任务登记的媒体文件临时加入 asset protocol 范围，前端不能任意读取文件系统。

`DesktopSettingsService` 读取 SQLite 中的非敏感设置，并通过 `CredentialStore` 访问平台系统密钥存储。`MutableTranslationProvider` 在不重建工作区服务的情况下原子替换当前翻译适配器。`ModelDownloadService` 每次只运行一个下载，写入应用管理目录中的 `.part` 文件，校验后原子安装并更新模型设置；UI 只轮询进度，不直接访问网络或模型文件。

正式桌面 App 把固定版本的 `whisper-cli`、`ffmpeg` 和 `ffprobe` 作为按平台/CPU 架构生成的 Tauri sidecar 放在主程序同目录；Finder 启动不依赖 shell、Homebrew 或用户 `PATH`。模型仍按设备下载到应用数据目录或由用户选择，不进入 Bundle。开发环境可用 `ATOGAKI_WHISPER_CLI`、`ATOGAKI_FFMPEG` 和 `ATOGAKI_FFPROBE` 覆盖 sidecar。macOS FFmpeg 从固定源码构建 libass 字体栈与 LGPL 配置，发布物附带许可证和构建清单，不包含 libx264。

启动恢复采用显式失败而非静默续跑：数据库中仍为非终态的识别任务会与任务目录快照核对，未完成者标记为上次退出导致的失败。重试从旧任务的输入、识别参数和词表快照创建新 UUID 任务，旧目录保持只读证据；旧模型路径不可用时才使用当前设备设置中的替代模型。ffmpeg/Whisper 子进程设置为随异步任务销毁而终止。

`LocalGlossaryService` 管理 SQLite 词表、任务范围 prompt 预览、差异预览和对工作区的应用。词条分为始终提示的“核心”、按任务选择的“内容包”和不占 prompt 的“仅修正”。新任务选择词表和内容包后，`LocalTaskService` 会在排队前把解析后的词条冻结为任务目录中的 `recognition-glossary.txt`，把最终 prompt 写入 `whisper-prompt.txt`，再将快照路径交给 Whisper。带规范写法的核心或内容词条会以类似 `スイ（表記: suis）` 的形式提示 Whisper，并在 ASR 后执行 `スイ → suis` 规范化；仅修正规则只执行后一阶段。SQLite 保存词表关联、名称和快照路径，因此以后编辑或删除原词表不会改变旧任务实际使用的内容。

任务显示名称只保存在 SQLite，不改变 UUID 任务目录和原媒体。任务删除仅允许 `done` 或 `failed` 状态：`LocalTaskService` 会校验目标确实是应用 `jobs` 根目录下与任务 ID 同名的目录，先将其原子移动为待删除目录，再删除 SQLite 记录和派生文件；数据库删除失败时恢复目录。任务记录中的原媒体路径永远不参与删除。

## 数据边界

- 媒体、模型、任务产物：默认本地文件系统。
- 任务元数据、字幕段、词表与编辑状态：桌面 MVP 使用 SQLite；生成快照首次导入后，人工编辑和 SQLite 生成的机器译文以 SQLite 为准，后续快照同步不会用旧 `segments.json` 清空它们。
- 密钥：通过统一凭据接口写入 macOS Keychain、Windows Credential Manager 或 Linux Secret Service，不写入 SQLite、任务 JSON 或日志；`DEEPL_AUTH_KEY` 仅作为兼容环境变量回退，且不会被自动迁移或回显。
- 桌面设置：模型路径、翻译 provider ID 和引导状态写入 SQLite；模型二进制位于用户选择的位置或应用数据目录的 `models/`，不打入 App Bundle。
- `status.json`：保留为任务产物与故障恢复副本，不作为唯一长期数据库。

## 核心数据约定

- 任务目录以 UUID 命名，避免并发任务冲突。
- 新任务把 Whisper/VAD 模型路径、VAD 阈值、分段和运行选项写入 `recognition-options.json`。该文件是识别结果的可复现记录；既有任务不反向补造当时未记录的参数。
- 字幕段拥有稳定 ID、开始与结束时间、原文、译文、来源编辑状态和翻译过期状态；读取旧 JSON 时会自动补齐 ID 并迁移写回。
- SQLite 另外记录中文是否人工编辑；只修改日文时保留原译文并标记为过期，同时修改中文时视为已人工校正。
- SQLite 词表是可编辑主数据；每个转写任务使用不可变文件快照。对已有字幕应用词表前先基于稳定段 ID 预览，确认后在单个事务中更新日文并把已有中文标记为过期。
- 词表分类只存在于 SQLite 主数据和桌面应用层；处理核心读取已解析的文本快照，避免把 UI 的内容包概念耦合进 Whisper 适配器。
- 翻译 provider 接收有序的字幕文本和可选上下文，并必须返回数量、顺序一一对应的译文。应用层保留稳定段 ID，校验返回数量，并在所有批次完成后使用带原文校验的 SQLite 事务一次性写入；翻译期间若日文已被修改，本次结果整体拒绝，避免译文错配。
- 当前 DeepL provider 按 12 段分批；单段和批量请求都会从 SQLite 当前日文读取前后 30 秒、最多 2000 字的共享局部上下文。上下文用于消歧，但每个字幕段仍是独立翻译单元，不负责修复 ASR 跨段断句。
- 桌面 SRT/ASS 是 SQLite 工作区的派生输出。每次用户导出会先刷新任务目录内的固定名称投影，再将日文、中文和双语 SRT/ASS 复制到用户选择的目录；目标文件使用经过文件系统安全化的任务显示名称作为前缀，已存在时必须由界面显式确认覆盖。存在过期译文时拒绝导出；缺失中文时允许导出并显式报告缺失段数。
- 桌面视频烧录是独立派生任务。提交时把 SQLite 当前日文、中文或双语内容冻结到任务目录 `renders/` 的 ASS 快照，再写入 `local_render_jobs` 并交给单 worker；最终 MP4 写到用户选择的位置。进度、取消、VideoToolbox 回退原因和实际编码器均持久化，临时文件不会直接暴露为最终输出。
- 应用层选项不得引用 `clap`、HTTP 或桌面框架类型。接口层负责转换。
- 外部工具和 API 是可替换基础设施：ASR、翻译和媒体处理分别通过应用层选项接入。CLI 默认日语识别、简中翻译，但不将该默认值写死到应用层或 DeepL 适配器。
- macOS 本地长任务采用硬件优先、显式回退：Whisper 请求 GPU device 0，GPU/Metal 失败才重试 CPU；硬字幕要求 libass，并优先用 VideoToolbox H.264 编码，失败后记录原因并回退 FFmpeg 原生 LGPL MPEG-4 软件编码；两层均失败才让任务失败并保留 ASS 快照。libass 字幕合成仍在 CPU 执行。
