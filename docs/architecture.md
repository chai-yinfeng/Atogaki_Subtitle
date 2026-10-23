# 架构与目录约定

_最后更新：2026-10-22_

## 仓库组织

`Atogaki_Sub` 保持为 Rust/Cargo 项目根目录。不要为了放置文档而把 `src/` 再嵌套一层：根目录的 `Cargo.toml` 与 `src/` 是标准 Rust 布局，移动它们会增加构建、编辑器和未来 Tauri 集成的复杂度。

```text
Atogaki_Sub/
├── Cargo.toml                 # Rust package 与依赖
├── src/                       # 可复用的处理核心与当前 CLI
│   ├── lib.rs                 # 供 CLI 与桌面 App 复用的库入口
│   ├── application/           # 任务、工作区、学习、字幕样式与本地服务
│   ├── domain/                # 字幕段、样式、词表、分段与导出规则
│   ├── infrastructure/        # ffmpeg、Whisper、翻译 provider、SQLite 与文件适配器
│   └── interface/             # CLI / HTTP 等输入适配层
├── assets/                    # 版本化的示例和内置词表
├── migrations/                # 现有 Postgres 实验性迁移；不作为桌面 MVP 依赖
├── migrations/sqlite/         # 桌面本地 SQLite 迁移
├── src-tauri/                 # Tauri 桌面壳；依赖根 Rust library
│   ├── src/commands/          # 按用例组织的 IPC 命令与 wire DTO；渐进抽取
│   ├── binaries/              # 构建时生成、按 target triple 命名的 sidecar
│   └── third-party/           # sidecar 版本、构建清单与许可证
├── scripts/                   # 固定版本 sidecar 构建与分发校验
├── ui/                        # Tauri 内嵌的 TypeScript/Vite 前端
├── docs/                      # 产品方向、路线图、架构与决策记录
└── tests/                     # 跨模块集成测试（按需要新增）
```

当前 Tauri 桌面壳位于 `src-tauri/`，前端位于 `ui/`，并直接复用根 Rust library。后续平台适配继续保持这一边界，不把现有 `src/` 移入人为的子目录，也不让平台 UI 直接调用 sidecar 或具体 provider。

## 目标桌面架构

```text
桌面 UI（Tauri）
  └─ application：导入、任务、编辑、翻译、学习、样式、导出用例
       ├─ domain：时间轴字幕、任务级样式、词表、字幕格式
       └─ infrastructure：文件系统、SQLite、ffmpeg、ASR、翻译与词典服务
```

UI 不直接启动 ffmpeg、Whisper 或具体翻译服务。它只调用 `application` 中的用例并订阅任务状态。当前 CLI 同样是该应用层的一个适配器。

Tauri 工作区命令已进入 `src-tauri/src/commands/editor.rs`，main 保留应用构造、共享状态与显式注册；命令只借用原服务，不持有另一份工作区或事务。其余命令与 SQLite repository 继续按 [0059](decisions/0059-desktop-command-and-repository-modules.md) 逐条抽取，不能把第一条边界视为整个 Rust 模块拆分已完成。

`LocalDatabase` 的文字、时间与结构编辑进入 `infrastructure/local_db/subtitle_edits.rs`；父模块和该子模块仍使用同一连接池，候选采用继续复用同一结构事务辅助函数。数据记录、迁移与公开方法路径不变，辅助函数只在 repository 内可见；模块边界不能切开现有原子提交。

`subtitle_publication.rs` 负责 ASR 来源恢复、快照发布和候选范围采用；`subtitle_queries.rs` 负责字幕只读查询与事务一致的 revision／字幕快照。任务元数据同步将原事务借给发布模块，候选采用也继续共用编辑模块的结构持久化函数，没有独立提交或复制证据保留逻辑。

`LocalTaskService` 提交时创建 queued UUID 任务目录，后台由 `StageScheduler` 协调 ASR 与翻译资源，再调用应用层处理用例。桌面默认 2 路 ASR、最多 4 路云端翻译，本地翻译与 ASR 受设备互斥限制；烧录仍为独立单 worker。工作台读取轻量摘要，活动任务按需读取进度；完整任务编辑时停止全任务轮询，详情只同步当前任务。摘要不解析全部 segments 或重复校验原始 ASR 证据，恢复与采用仍执行完整校验。见 [0052](decisions/0052-staged-concurrency-and-early-translation.md) 与 [0056](decisions/0056-editor-virtual-list-and-task-summary.md)。

当前 Tauri 壳共享一个 `LocalDatabase` 实例，并注册 `LocalTaskService`、`LocalWorkspaceService`、`LocalLearningService`、`SubtitleStyleService` 与 `LocalRenderService`：任务服务负责创建、排队、同步识别任务和持久化工作台顺序；工作区服务负责读取任务详情、保存编辑、调用注入的翻译 provider 和导出；学习服务负责收藏字幕选区、管理来源例句、简明译义与多 provider 结果；字幕样式服务通过 `SubtitleFontService` 枚举系统字体、检查任务文字覆盖，并调用与正式输出相同的 FFmpeg/libass 路径生成预览；烧录服务负责冻结 SQLite 字幕与样式快照、持久化输出任务、进度与取消。识别、翻译和烧录的状态与资源预算分别管理。界面通过原生文件选择器获取媒体、Whisper 模型和 Silero VAD 模型路径；也可由设置页下载官方模型到应用数据目录。VAD 默认开启但允许显式关闭。打开任务后，原媒体和任务音频通过 `PlaybackMediaRegistry` 登记，以随机 token 暴露给仅监听 127.0.0.1 随机端口的会话媒体服务。Host／Origin 与登记路径受限，完整 GET 和 Range 按 64 KiB 流式读取；优先原媒体，解码错误最多回退一次到任务音频并保留播放意图。该实现由 [0054](decisions/0054-request-batching-and-loopback-media.md) 取代旧自定义协议的传输路径，与早期 Web Serve 外壳独立。

`DesktopSettingsService` 读取 SQLite 中的非敏感设置，并通过 `CredentialStore` 按 provider ID 访问平台系统密钥存储。`MutableTranslationProvider` 在不重建工作区服务的情况下原子替换当前翻译适配器。模型下载、词典包下载与云端 provider 共用可热切换的网络配置：跟随启动环境、强制直连或自定义 HTTP 代理；模型还可使用用户提供的 HTTPS 镜像根地址。DeepL、DeepSeek、自定义 OpenAI-compatible 与在线词典 provider 都延迟到首次实际请求才读取对应密钥，并在单个 App 进程内复用成功取得的值。`ModelDownloadService` 每次只运行一个下载，按镜像到官方源回退，写入应用管理目录中的 `.part` 文件；合法临时文件跨重启保留，后续请求只有在 HTTP Range／Content-Range 与本地边界一致时才追加，否则安全重启下载，最终通过固定 SHA-256 后原子安装。`DictionaryDownloadService` 在 `dictionaries/` 内按版本化临时文件边界工作，滚动发布包解析 GitHub Release 的 tag、资产与 SHA-256，ECDICT 固定文件校验精确字节数和 Git blob 对象摘要，更新失败时恢复旧包。`DictionaryLookupService` 隔离各来源错误，按需建立 JMdict／ECDICT 索引、读取 Tomoshi 数据库或调用 Merriam-Webster，并把结构化结果交给学习服务持久化。UI 只轮询进度，不直接访问网络、模型或词典文件。

macOS 完整桌面 App 把固定版本的 `whisper-cli`、`ffmpeg`、`ffprobe` 和 `llama-server` 作为按平台／CPU 架构生成的 Tauri sidecar 放在主程序同目录；Finder 启动不依赖 shell、Homebrew 或用户 `PATH`。模型仍按设备下载到应用数据目录或由用户选择，不进入 Bundle。开发环境可用 `ATOGAKI_WHISPER_CLI`、`ATOGAKI_FFMPEG` 和 `ATOGAKI_FFPROBE` 覆盖 sidecar。macOS FFmpeg 从固定源码构建 libass 字体栈与 LGPL 配置，发布物附带许可证和构建清单，不包含 libx264。

macOS Apple Silicon 是日常开发与完整体验的质量基线。Windows 11 x86_64 已通过 `v0.1.0-alpha.10` 建立包含 CPU Whisper、Credential Manager、WebView2、平台应用数据目录、学习与词典、字幕样式、可恢复翻译、局部重新识别、Range 媒体播放和 LGPL MPEG-4 烧录的独立预发布基线；字幕样式预览已完成实机确认，其他未记录矩阵仍继续扩测。Windows 悬浮字幕不稳定且不建议使用。Windows sidecar、依赖声明和对应源码归档继续按目标平台独立构建与审计，不能把 macOS arm64 产物或验收结果直接视为跨平台结论。

启动时核对数据库与任务目录，将遗留非终态任务标为中断并等待用户动作。具有合法检查点的任务可显式 resume，在原任务下保留关联尝试并只补算未完成范围；旧任务或无法恢复的配置仍可 retry，依据冻结参数创建新 UUID 任务，原目录保留证据。恢复校验不由轻量摘要代替，不反向补造旧任务缺失参数；ffmpeg／Whisper 子进程随异步任务销毁而终止。见 [0045](decisions/0045-resumable-offline-workflows.md)。

`LocalGlossaryService` 管理 SQLite 词表、任务范围 prompt 预览、差异预览和对工作区的应用。词条分为始终提示的“核心”、按任务选择的“内容包”和不占 prompt 的“仅修正”。仓库 TXT 的 SHA-256 是内置词表版本：内容变化时启动流程完整刷新只读内置基线，用户保存内置词表则写时复制为独立自定义词表。新任务选择词表和内容包后，`LocalTaskService` 会在排队前把解析后的词条冻结为任务目录中的 `recognition-glossary.txt`，把最终 prompt 写入 `whisper-prompt.txt`，再将快照路径交给 Whisper。带规范写法的核心或内容词条会以类似 `スイ（表記: suis）` 的形式提示 Whisper，并在 ASR 后执行 `スイ → suis` 规范化；仅修正规则只执行后一阶段。SQLite 保存词表关联、名称和快照路径，因此以后编辑、升级或删除原词表不会改变旧任务实际使用的内容。

任务显示名称只保存在 SQLite，不改变 UUID 任务目录和原媒体。任务删除仅允许 `done` 或 `failed` 状态：`LocalTaskService` 会校验目标确实是应用 `jobs` 根目录下与任务 ID 同名的目录，先将其原子移动为待删除目录，再删除 SQLite 记录和派生文件；数据库删除失败时恢复目录。任务记录中的原媒体路径永远不参与删除。

字幕文件导出由 `subtitle-export.ts` 协调单次会话，`subtitle-export-view.ts` 管理按需连接的格式与覆盖确认节点，`subtitle-export-backend.ts` 适配原四条 Tauri IPC。请求绑定任务身份与 navigation，每次异步返回核对代次；离页释放等待和 busy，不把迟到的导出路径展示给另一任务。唯一草稿、播放器及后端导出服务保持原边界。

单句与批量字幕翻译通过 `subtitle-translation.ts` 协调，后端调用集中在 `subtitle-translation-backend.ts`；每任务活动独立、共享一个进度 timer，页面退出 dispose 释放 timer。调用绑定任务与 navigation，后台落库与视图呈现分开：旧视图返回不更新当前内容，失败仍读取先前成功批次。草稿 flush 和 revision 校验沿用原服务。

任务样式的异步读取、保存与预览通过 `subtitle-style-session.ts` 协调，四条调用集中到 `subtitle-style-backend.ts`。每次打开绑定任务／导航代次，修改递增草稿版本，串行渲染只接受最新版本；关闭释放 debounce 和迟到呈现。字体与颜色视图暂留 main，FFmpeg/libass 与实际样式服务保持原实现。

## 数据边界

- 媒体、模型、任务产物：默认本地文件系统。
- 任务元数据（包括源语言和目标语言）、字幕段、带源语言的词表与编辑状态：桌面 MVP 使用 SQLite；生成快照首次导入后，人工编辑和 SQLite 生成的机器译文以 SQLite 为准，后续快照同步不会用旧 `segments.json` 清空它们。
- 密钥：通过统一凭据接口按 provider ID 写入 macOS Keychain、Windows Credential Manager 或 Linux Secret Service，不写入 SQLite、任务 JSON 或日志；字幕翻译使用自身 provider ID，Merriam-Webster 使用 `dictionary:merriam-webster` 隔离命名。Collins 未取得可测试账户、Cambridge API 当前不可用，均已退出产品路径。`DEEPL_AUTH_KEY`／`DEEPSEEK_API_KEY` 仅作为兼容环境变量回退，且不会被自动迁移或回显。
- 桌面设置：模型路径、翻译 provider ID、非敏感模型／端点／风格、引导状态、无凭据代理配置、可选镜像地址与“曾保存凭据”标记写入 SQLite；模型二进制位于用户选择的位置或应用数据目录的 `models/`，用户触发下载的词典归档与版本 sidecar 位于 `dictionaries/`，两者都不打入 App Bundle。
- `status.json`：保留为任务产物与故障恢复副本，不作为唯一长期数据库。

## 核心数据约定

- 任务目录以 UUID 命名，避免并发任务冲突。
- 新任务把 Whisper/VAD 模型路径、VAD 阈值、分段和运行选项写入 `recognition-options.json`。该文件是识别结果的可复现记录；既有任务不反向补造当时未记录的参数。
- 字幕段拥有稳定 ID、开始与结束时间、原文、译文、来源编辑状态、时间轴编辑状态和翻译过期状态；读取旧 JSON 时会自动补齐 ID 并迁移写回。SQLite 是人工时间轴的主数据源，手动时间在任务快照再次同步时必须保留；时间改变不使译文过期，但后续字幕导出和烧录使用新时间。
- 当前字幕段属于隐式单轨，同轨不允许新增重叠。多人同时发言出现真实需求后，先通过单独决策引入轨道实体和 `track_id`，再设计说话人样式；当前 schema 不提前加入未被使用的逐段排版字段。
- 每个任务保存一份版本化的原文／译文 `SubtitleStyleSet`；字体、颜色、描边、背景、对齐、位置和边距先经过统一校验与 ASS 序列化，再供 libass 预览、双语 ASS 和烧录快照复用。SRT 不承诺保留视觉样式，系统字体文件不复制进 App 或任务目录。
- `local_jobs.sort_position` 只表达用户任务顺序，不改写业务更新时间。尚未排序的旧数据库继续按更新时间展示；第一次排序在事务中保存当前完整 ID 顺序，新任务在下一次人工排序前默认置顶。
- 每个任务用规范代码持久化源语言和目标语言；首批桌面组合为 `ja`、`en` 或 `ko` 到 `zh-Hans`，旧任务迁移为 `ja` 到 `zh-Hans`。Whisper 与翻译 provider 只在各自基础设施适配器中转换服务专用代码。
- SQLite 另外记录译文是否人工编辑；只修改原文时保留旧译文并标记为过期，同时修改译文时视为已人工校正。
- 学习资料使用 `local_learning_items`、`local_learning_occurrences` 和 `local_learning_lookup_results` 分层：第一层按语言对、类型和规范化原文合并词／短语／语法或整句条目，第二层保存每次出现的任务／字幕引用、UTF-16 选区、收藏时双语快照和整段时间范围，第三层按 provider 保存命中词形、读音、结构化义项、署名、数据版本和可选缓存期限。任务删除后来源外键置空而快照继续存在；媒体与音频不写入 SQLite。用户简明译义、字幕语境翻译和标准词典内容相互独立，不能把字幕翻译 provider 的结果标记为词典释义。
- SQLite 词表是带源语言的可编辑主数据；每个转写任务只能选择同语言词表，并使用不可变文件快照。对已有字幕应用词表前先基于稳定段 ID 预览，确认后在单个事务中更新原文并把已有译文标记为过期。
- 词表分类只存在于 SQLite 主数据和桌面应用层；处理核心读取已解析的文本快照，避免把 UI 的内容包概念耦合进 Whisper 适配器。
- 翻译 provider 接收任务语言对、带稳定 ID 的目标字幕段、语义分离的前后文、风格提示和受保护术语，并返回带相同 ID 的结构化译文。应用层校验结果数量、重复／遗漏 ID，每个成功批次立即用带原文校验的 SQLite 事务写入；批次失败后再次执行只选择缺失或过期段。空译文单独补充请求一次，仍为空则保持待翻译并返回具体段落提醒。
- `TranslationPlanner semantic-v1` 按连续性、句末、停顿与预算构造语义 group，稀疏目标保持各自上下文。DeepSeek 可将短 group 打包成一次请求，仍按 group 保存冻结输入、缓存、revision 和失败状态；HTTP 请求数与语义组数分别统计，不再把固定 12 段当作当前分组合同。
- `local_subtitle_provenance` 保存来源／证据单元和 source／translation revision，translation dependency edges 保存实际上下文依赖。机器写回校验冻结输入、人工编辑与 revision；上下文变化可使消费者 dependency stale，已有人工译文不被后台响应覆盖。
- SQLite 的 translation run 保存 provider／模型、grouping strategy、选项和 plan_json（冻结目标／上下文及 revision）、段数、可得用量与完成时间，不保存 API Key。这是运行来源与恢复依据，不等于完整可恢复的人工文字历史。详见 [0043](decisions/0043-layered-pipeline-and-task-scoped-asr-runs.md)、[0048](decisions/0048-explicit-translation-scope.md) 与 [0054](decisions/0054-request-batching-and-loopback-media.md)。
- 桌面 SRT/ASS 是 SQLite 工作区的派生输出。每次用户导出会先刷新任务目录内的 `source.srt`、`translation.srt` 等固定投影，再将原文、译文和双语 SRT/ASS 复制到用户选择的目录；单语文件以规范语言代码命名（如 `.en.srt`、`.zh-Hans.srt`）。目标文件使用经过文件系统安全化的任务显示名称作为前缀，已存在时必须由界面显式确认覆盖。存在过期译文时拒绝导出；缺失译文时允许导出并显式报告缺失段数。
- 桌面视频烧录是独立派生任务。提交时把 SQLite 当前原文、译文或双语内容冻结到任务目录 `renders/` 的 ASS 快照，再写入 `local_render_jobs` 并交给单 worker；最终 MP4 写到用户选择的位置。进度、取消、VideoToolbox 回退原因和实际编码器均持久化，临时文件不会直接暴露为最终输出。导出提供省空间／均衡／高质量三档，保持源分辨率并分别把目标视频码率设为源码率约 75%／120%／180%；源流不报告码率时使用明确显示的分辨率基准。规格预览与实际任务复用同一计算函数，并以视频、音频码率和 2% 封装余量估算成品大小。
- 应用层选项不得引用 `clap`、HTTP 或桌面框架类型。接口层负责转换。
- 外部工具和 API 是可替换基础设施：ASR、翻译和媒体处理分别通过应用层选项接入。CLI 默认日语识别、简中翻译，但不将该默认值写死到应用层或 DeepL 适配器。
- macOS 本地长任务采用硬件优先、显式回退：Whisper 请求 GPU device 0，GPU/Metal 失败才重试 CPU；硬字幕要求 libass，并优先用 VideoToolbox H.264 编码，失败后记录原因并回退 FFmpeg 原生 LGPL MPEG-4 软件编码；两层均失败才让任务失败并保留 ASS 快照。libass 字幕合成仍在 CPU 执行。
- 收听区和任务工作区可按需打开独立的悬浮字幕 WebView 窗口。它由 Rust 主进程创建为无边框、置顶、可缩放的正常窗口，当前播放工作区按字幕段切换发送原文与译文；窗口本身不读取 SQLite、媒体或密钥。为维持公开 Tauri 配置，它不使用 macOS 私有 API，因此当前不支持透明背景和鼠标穿透。收听区同时承担学习采集的全文阅读器：文本选择与独立时间码跳转分离，学习区通过来源记录回到同一播放器，不复制第三套预览或播放器。

<a id="下一阶段架构已决策尚未实现"></a>

## 管线演进与实验边界

[0043](decisions/0043-layered-pipeline-and-task-scoped-asr-runs.md) 的 acoustic chunks、TimedUnit、subtitle cues、translation groups、任务内 run／artifact 与 provenance／revision 已分批实现，当前 SQLite 工作区仍是人工编辑主数据。候选采用独立于识别运行，继续核对范围、comparison 与 workspace revision，并保留采用来源。

真实接缝与模型质量、盲评、完整资源压力、部分云端网络与 Windows 结果仍待补。实时 session、采集与 E2E 是研究路线，不能因离线 provider 已存在而直接晋升桌面。原[管线演进计划](pipeline-evolution.md)保留提案基线，当前完成度看[路线图](roadmap.md)与对应验收，不将旧的“尚未实现”描述用于全部现有能力。

## 字幕编辑前端的渐进边界

四区默认布局为左侧视频、右侧波形与上下双语编辑、下方整宽虚拟字幕列表。`editor-session` 区分跟随／锁定、编辑段、播放段与列表自动滚动；`editor-layout` 持有 v3 本机布局偏好，分别调整左右、工作区／列表、右侧波形／编辑比例，折叠只隐藏媒体预览；`editor-menus` 管理紧凑操作菜单的关闭和焦点生命周期；`editor-cue-list` 只呈现视口行。`media-player` 是编辑／收听共用的单一媒体所有者，管理回退、事件、RAF 与卸载，Tauri 资源 URL 解析通过回调注入；`editor-follow` 协调编辑锁定、恢复请求代次与时间输入保护；`editor-timeline-view` 持有静态波形缓存、播放头与播放段独立绘制及 ResizeObserver，绘制与拖动命中共用几何规则；`editor-waveform` 管理请求代次、窗口快照与 16 ms 合并定时器，新视窗意图立即失效旧返回，Tauri 请求经 `editor-backend`；`editor-timeline-gestures` 持有可取消的指针／平移／缩放预览，`timeline-timing` 限制时间边界，提交仍由会话 flush 和文本重基；`current-cue-editor` 使用原 SubtitleDrafts 管理输入交互，`editor-backend` 集中适配文字／时间持久化，`task-tools` 持有工具 DOM 的挂载与焦点生命周期。工具模板与视图逻辑独立于主入口，关闭工具不重建媒体或草稿。

主入口仍协调播放器、时间轴和部分处理用例，后续每次抽取一条边界，不能将当前结构写成已经完成全模块拆分。任务卡片通过 SQLite 摘要读取，恢复与采用仍完整验证来源证据；见 [0056](decisions/0056-editor-virtual-list-and-task-summary.md) 和 [0057](decisions/0057-editor-view-lifecycles.md)。

前端类型检查启用 `noUnusedLocals`／`noUnusedParameters`，删除项先通过调用搜索确认。旧粗修保存／撤销入口已移除；现有两类历史继续由唯一字幕编辑工作面使用，不统一命令历史。
