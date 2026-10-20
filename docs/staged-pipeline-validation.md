# 阶段队列与稳定前缀翻译验证

日期：2026-10-20。设计见 [0052](decisions/0052-staged-concurrency-and-early-translation.md)。发行版仍为 v0.1.1，本页改进尚未公开发行或替换 `/Applications`。

## 实现结构

- `stage_scheduler.rs`：默认 2 个 ASR 名额、4 个全局云端请求名额。自动翻译、普通工作区及任务级 provider 共用请求预算；全片／局部本地候选共用设备预算。本地 Hy-MT2 独占设备名额，保持完整 ASR 后执行。
- `local_task_service.rs`：每任务独立协调与取消，ASR 结束即释放资源，不等待翻译；暂停等待旧执行退出，排队暂停可继续原项。ASR 与提前翻译以独立 future 推进，翻译失败不停止 ASR，完整发布后停在翻译阶段等待继续。
- `acoustic_plan.rs`／`job_runner.rs`：当前执行验证配置后才发检查点通知；稳定前缀只读连续有效块，留出最新块、读取边缘和翻译后文。新 Whisper cue ID 复用实际 timed-unit ID，旧有效 final-cues 缓存不改号。
- `early_translation.rs`：冻结配置与分组请求；校验、原子提交响应缓存，记录每次实际请求的不可变快照、响应或错误。缓存包含完整输入与校验，不保存凭据。完整发布后的正常翻译入口只在原文、上下文、配置及初始 revision 都匹配时复用。手动重译不套用该缓存。
- `local_workspace_service.rs`／`local_db.rs`：DeepSeek 独立 group 有界并发；失败停止新派发并等待进行中 group 保存。SQLite 写回核对捕获的原文／译文 revision，包含修改后还原原文的情况。复用现有表及事务，无 schema 迁移。
- 任务卡显示提前翻译缓存计数，保留失败原因展开和只读部分原文。完整识别前不提供人工编辑、收藏或成品导出。

## 工程回归

核心完整测试：137 通过、5 忽略；其中真实 Metal 测试随后单独显式执行，1 通过。Tauri：32 通过、4 忽略；前端：17 通过。根目录及 Tauri 格式检查、`git diff --check`、前端类型检查／生产构建、Tauri 编译检查通过。

故障及并发测试覆盖：

- 两个 ASR 名额与四个云端请求独立限流；本地翻译互斥，取消归还名额。
- DeepSeek 首组失败时，其余三组照常落盘；继续只请求余下五组，成功部分不重复翻译。
- 三个模拟六分钟任务：先同时识别两个任务并阻塞提前翻译请求，前两者结束 ASR 后，第三者仍能开始识别；完整写回复用提前响应，每份任务的三个目标各请求一次。
- 另一个任务在提前翻译时注入云端失败，仍完成全部 ASR 并停在翻译阶段；移除原始音频和模型后继续，三个已提交块摘要保持不变，任务只完成缺失译文。
- ASR 中断后提前翻译结果可以跨会话重开；单个损坏响应只重做该组；原文未完整发布时不创建编辑工作区。
- 配置、后文变化导致缓存失效；原文改变后恢复相同文字也不复用旧 revision，事务拒绝过期的在途写回。
- 已有暂停／继续、删除、人工译文保护、候选和旧数据库回归保持通过。

## 本机真实 Metal

使用本机用户提供影片的前八分钟音轨，两份独立任务、Whisper medium、Silero VAD、真实 FFmpeg／whisper.cpp。翻译使用测试内的本地 Echo provider，不读取 Key、不访问云端；结果不能作为译文质量或 API 限额验收。

显式运行 `real_two_job_metal_pipeline_with_local_translation_fixture`，约 62.87 秒通过。两任务首块设备均为 GPU，运行区间重叠，首块分别约 11.208／11.176 秒。两任务各生成 128 段并完成译文写回，提前翻译摘要均为 85 段；测试按目标 timed-unit ID 确认确实在**自身 ASR 尚未完成**时发起提前翻译。报告位于忽略目录 `local-artifacts/staged-pipeline/real-two-job-8m/report.json`，原始产物及模拟译文不提交 Git。

初次六分钟开场样本，两份 GPU 识别与最终翻译都完成，但未满足“自身 ASR 完成前发起请求”的断言：开场空白及首个语义分组跨过了第一批稳定目标边界。改用八分钟样本保留后续识别块后通过。保留此失败记录，不能把短样本完成解释为提前翻译验收通过。

复现入口（输入至少八分钟并包含前段语音）：

```sh
ATOGAKI_STAGE_SMOKE_INPUT=/absolute/path/input.wav \
ATOGAKI_STAGE_SMOKE_MODEL=/absolute/path/ggml-medium.bin \
ATOGAKI_STAGE_SMOKE_VAD=/absolute/path/ggml-silero-v6.2.0.bin \
ATOGAKI_STAGE_SMOKE_SIDECARS=/Applications/Atogaki.app/Contents/MacOS \
ATOGAKI_STAGE_SMOKE_OUTPUT=/absolute/path/new-isolated-directory \
cargo test --release --lib real_two_job_metal_pipeline -- --ignored --nocapture
```

## macOS 窗口与交付边界

通过 `cargo tauri build --config tauri.macos-complete.conf.json --bundles app` 生成完整四 sidecar、ad-hoc 签名 App，并通过 `codesign --verify --deep --strict`。在 `/private/tmp/atogaki-stage-window` 隔离数据库中使用明确的测试任务与检查点：最终构建实见新任务的云端提前翻译说明、失败任务的缓存计数、错误详情展开以及只读原文预览，无编辑／收藏／导出入口；关闭后重开状态保留，测试窗口已关闭。模拟任务没有音频，窗口测试不作为媒体播放回归。

在该次工程验收节点尚未生成新 DMG、发布或替换正式 App；后续本机安装与候选打包见下节。真实云端限流／断网验收仍需用户授权样本；完整长节目双任务的持续内存与吞吐、所有接缝的人工听审、Windows 原生窗口尚未验证。现有单份 202 分钟影片恢复验收属于前一修复，不能替代本轮双任务长时间压测。本轮不宣称吞吐翻倍或翻译质量提升。

## 2026-10-20：本机安装与 v0.1.2 候选

按用户要求先安装功能测试构建，再准备发行包。退出旧应用后，使用 SQLite backup API 备份数据库，并保存旧 App；安装 `/Applications/Atogaki.app` 后通过 deep/strict 签名检查，主程序与已验证构建逐字节一致。原生窗口确认新云端提前翻译说明和可展开失败原因，14 个任务仍在。未自动继续正式电影任务或调用云端。备份在忽略目录 `local-artifacts/install-backups/20261020-162429/`，另保留 `/Applications/.Atogaki.previous-20261020-162429`。

安装测试构建暂保留 0.1.1 元数据；待打包的候选统一为 0.1.2，运行代码相同，仅版本与发行文档、许可证索引更新。版本更新后核心 137 通过／5 忽略、Tauri 32 通过／4 忽略、前端 17 通过，类型检查、生产构建、双 Rust 格式与差异检查通过。许可证重新生成仅更新自身版本及前端 lockfile 摘要。用户正在正式数据上试用；尚未创建 tag 或公开 Release。


从固定提交 `575a9426afa734c4c17fd127755f19e1132c68c7` 非 CI 构建 0.1.2 完整 App／DMG。App 与四个 sidecar 通过签名检查；DMG 通过 hdiutil 校验，只读挂载的五个可执行文件与构建 App 摘要一致，Applications 链接正确，许可证齐全。实际 Finder 窗口确认 App 左／Applications 右、图标充分分隔，检查后已卸载测试卷。对应源码归档的八项摘要及重建材料核对通过。

候选四项资产保存在忽略目录 `local-artifacts/releases/v0.1.2/`，另含提交／校验／剩余门禁清单。DMG 为 45,902,505 字节，SHA-256 `b2bcd21592ec9a35045f981a13e1868cc07e5dcd90c514ebf83ea8f0fe1193ef`；对应源码为 81,923,880 字节，SHA-256 `1ccc7daa28b29ecdb343668b4d4f05349d99487909a90a68cc34a3c7cdaf72cc`。未上传资产、创建 tag 或 Release。为避免打断用户正在运行的正式任务，未退出已安装测试构建；最终 0.1.2 元数据 App 的独立原生窗口复核留待用户试用结束后、公开发行前完成。

## 2026-10-20：用户试用反馈后的计时／进度修复

计时在状态文件增加兼容字段 `processing_time`（累计执行秒数、当前区间起点），原始开始／结束时间保留，不改 SQLite 既有字段或字幕。各次执行在暂停、失败、完成时结算，排队不计入。旧失败任务首次继续时用原开始／结束计算已知用时；此前已被旧版本累计污染的历史记录没有足够区间证据时不猜测修正。五秒独立心跳为崩溃恢复提供终止时间；没有心跳的旧记录保守使用最后状态时间。

提前翻译每个成功 group 发布进度，不再等整个批次结束；新执行和新一轮有效前缀处理清除旧错误提示，正在等待剩余翻译显示正常阶段。新增门控 provider 回归：一个组完成、其余组阻塞时已能读到已完成计数；原故障／缓存／继续回归保持通过。没有改变云端授权、触发稳定边界或错误后停止重试策略。

验证：核心 141 通过／5 忽略、Tauri 32 通过／4 忽略、前端 19 通过，类型检查、生产构建、双 Rust 格式和差异检查通过。核心包含暂停／恢复排队时间、旧终止记录兼容、崩溃按最后观察停止、分组进度提前可见的回归。旧候选 manifest 已标为 superseded，不能发布旧 DMG；本轮未退出用户正在测试的 Application 或修改正式任务计时。新 App 原生窗口与重打包验收尚待完成，未发布。

已从修复提交 `1603986` 非 CI 重打完整 App／DMG，四 sidecar 与主 App 签名通过。新 DMG 为 45,877,972 字节，SHA-256 `dc300205530fbbe728cf11427c1317c28faf425fee6769b3d6a60631fcf62227`，覆盖本地未公开候选资产；先前 DMG 摘要仅作历史记录，不得发布。hdiutil 校验、只读挂载版本与五个可执行文件逐字节摘要、许可证、Applications 链接及 Finder 布局元数据检查通过，已卸载卷。对应源码无变化。重打后的 Finder 实际窗口和新 App 原生窗口仍作为公开前门禁，等待不干扰用户当前任务的时机复核。本机 Application 未再次替换。
