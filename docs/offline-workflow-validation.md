# 离线工作流验证与使用边界

日期：2026-10-18。本文记录当前开发分支，不表示公开安装包已更新。

## 工作流

任务在约 120 秒音频区间提交检查点，前后各最多 5 秒作为读取上下文。文件证据先校验、同步并原子提交，SQLite 索引随后写入；恢复重新验证完成块并重建索引。模型身份按内容摘要冻结，只有模型路径缺失时才允许从设置重新定位相同内容。未完成块重新执行，已完成块保持原样。旧任务没有检查点时只能显式重新运行。

暂停使用任务控制信号取消当前 future，Whisper／FFmpeg 配置 kill-on-drop；新运行继承成功块的 CPU 回退状态。重启不自动启动工作。队列等待从本次排队时间计算；任务卡的“任务历时”沿用首次处理到结束的跨度，包含暂停间隔，不作为纯推理耗时。部分原文在只读窗口中定位试听，完整识别完成后才进入编辑。完成块和 ASR 发布标记分开，翻译失败或导出失败不需要重做识别；批量翻译恢复只处理缺失或过期的 group。

候选保留当前字幕快照、实际 provider／模型、时间粒度和配置，重启后可以重开。采用始终重新检查快照，不能覆盖候选生成后的人工修改。Gemini 授权只用于当次确切范围；取消云端候选先等待 provider 完成上传文件清理，本地 Whisper 取消会终止当前子进程。

## 自动回归

- Rust 核心：覆盖一小时模拟音频的中断恢复、只重做损坏块、模型移动／内容变化、原子文件发布、暂停终止解码器、源媒体删除后继续及重复继续。
- 持久候选：重建 service 后重开，原文改变后拒绝采用，新候选可以采用，重复采用不会再次覆盖。
- 翻译：执行快照不受全局 provider 替换影响；原文、上下文和人工译文的 revision 校验沿用既有回归。
- 学习：Anki TSV 保留引号、制表符、换行、全部来源及原任务删除后的快照。前端覆盖 NFKC 搜索、UTF-16 选区高亮、HTML 转义和英语词边界。
- 桌面：生产前端构建、Tauri 编译／测试和标准 Atogaki.app 调试构建；窗口使用 `/private/tmp` 隔离数据库，正式安装与正式资料不被覆盖。

检查命令：

```sh
cargo fmt --check
cargo test --lib
npm --prefix ui run test
npm --prefix ui run build
cargo test --manifest-path src-tauri/Cargo.toml
git diff --check
```

## 真实长节目与故障注入

可使用以下工具对至少一小时的本地节目复验。它创建独立目录和一次性 Whisper wrapper，故意让指定块失败，再恢复；随后制造字幕输出路径故障，确认导出恢复不运行 ASR、cue ID 不变化。完成证据在恢复前后进行摘要比较，原媒体与模型只读。

```sh
python3 scripts/verify-asr-recovery.py --input /absolute/program.wav \
  --model /absolute/ggml-medium.bin
```

2026-10-18 在 62 分 26 秒真实节目上完成 32 块验证：第 18 块注入解码失败，恢复时只重做该未提交块；此前 17 块的 51 个证据文件摘要完全不变。随后注入字幕导出路径失败，恢复未调用 ASR，cue ID 保持一致。独立报告保存在 Git 忽略的 `local-artifacts/recovery/6679d1d5-3201-4cd4-b1da-102ed97fee23/report.json`；四项恢复断言全部通过。模拟回归不能代替所有接缝的人工听审。时间证据超过读取音频时明确限制在真实边界并标记 `unknown`，原始输出保留，并生成待复核质量信号；不伪造词级精度。

## MLX 对照

在 Apple Silicon 开发机的隔离 Python 3.14 环境验证 `mlx 0.32.4`、`mlx-whisper 0.4.3`、`mlx-lm 0.32.2`。模型 revision 与完整目录摘要固定在 `scripts/mlx-models.json`，实验依赖固定在 `scripts/mlx-requirements.lock.txt`。不进入桌面 Bundle，不改变桌面最低系统版本。

```sh
python3 -m venv local-artifacts/mlx/env
local-artifacts/mlx/env/bin/pip install -r scripts/mlx-requirements.lock.txt
local-artifacts/mlx/env/bin/python scripts/prepare-mlx-models.py --download
local-artifacts/mlx/env/bin/python scripts/compare-local-engines.py probe
local-artifacts/mlx/env/bin/python scripts/compare-local-engines.py asr \
  --input /absolute/sample.wav --model local-artifacts/mlx/models/whisper-medium \
  --baseline-model /absolute/ggml-medium.bin
local-artifacts/mlx/env/bin/python scripts/compare-local-engines.py translation \
  --input /absolute/source-cues.json --model local-artifacts/mlx/models/Hy-MT2-1.8B-4bit \
  --baseline-model /absolute/Hy-MT2-1.8B-Q4_K_M.gguf
```

CLI 也可以直接运行 `transcribe --engine mlx-whisper --python <env-python> --model <local-MLX-directory>`。推理桥接强制离线，使用与 Whisper.cpp 相同的检查点执行器。模型必须预先存在，不会在转录中下载。

本次 60 秒日语片段，Whisper.cpp medium 与 MLX medium 均完成。Whisper.cpp 的块识别约 6.14 秒；MLX 记录的模型加载与识别约 8.09 秒。两侧时间戳模式、Python 导入和测量边界不同，不能据此断言哪条路线更快；调试 CLI 的完整时间还包含模型摘要校验。MLX 记录的 allocator 峰值约 1.11 GiB，RSS 与 GPU allocator 指标不能直接互换。没有人工 reference，不报告 CER／WER 或准确率收益。

12 条冻结 source cues 的本地翻译结构测试：llama.cpp 的 1.8B 与 7B 均通过，共用 TranslationPlanner／stable cue ID／术语和 JSON 校验；对应请求运行约 10.61／45.05 秒，server 进程树采样峰值约 1.87／5.53 GiB。该小样本不代表完整长节目的峰值。

**Hy-MT2 MLX 未通过当前产品合同。** 同一输入经 mlx-lm server 返回多于目标数的译文，并在重试后出现 JSON 尾部额外内容。结果被拒绝，未放宽校验或写入工作区。模型能加载和输出中文，不代表能可靠替代现有翻译后端。1.8B／7B／历史译文已生成独立匿名材料和答案，人工评分待完成；不修改模型推荐档位。

## 仍需验收

- 完整真实长节目接缝听审与少量人工 reference；翻译盲评仍需人工判断。
- Windows 本机的暂停／继续、原子文件替换、路径和窗口行为；本机未宣称 Windows 通过。
- 公开发行、签名与公证未执行。优化 CLI 的 sqlx proc-macro LINKEDIT 阻塞已通过编译时依赖 profile 修正，见下方恢复工作记录；这不等同于安装包发行验收。

## 2026-10-18 暂停 checkpoint

用户因 5h 额度即将用尽要求在稳定节点暂停。产品实现 checkpoint 为 `4c7da20`，评测工具 checkpoint 为 `66b2b4a`；此前三个实现提交保留在同一分支历史。当前分支为 `codex/offline-workflow-recovery`，没有合入 main，也没有发布安装包。旧 `codex/docs-product-refresh` 已确认被 main 包含，并删除本地与远端分支。

本轮实现已具备长音频恢复、任务翻译快照、持久候选、搜索／循环试听／批量预设与学习导出的主要代码。Rust 核心 120 项通过、4 项忽略；Tauri 28 项通过、4 项忽略；前端 4 项通过，类型检查及生产构建通过。真实隔离打包窗口已确认新任务创建、暂停、重启后同一任务继续并完成；未完成全部交互验收，不将代码实现等同于全量验收完成。

恢复工作时先读取 `git status`、`git log`、本文件与路线图，不重复真实长节目识别。按以下顺序继续：

1. 审查当前提交并补完整隔离窗口回归：候选重开／对照／revision 冲突、设置草稿与任务级翻译、批量任务与预设、搜索与循环、同收藏多来源及 Anki 保存面板。已有测试数据库 `/private/tmp/atogaki-offline-window-20261018`、测试输入 `/private/tmp/atogaki-window-fixtures` 可复用；不操作正式数据。
2. 对 62 分钟结果的全部切块接缝听审，补人工 reference；核对发布／拼接／磁盘提交故障注入覆盖。现有真实故障覆盖解码与导出，其他故障主要依赖自动化模拟，不能宣称完整真实验收。
3. MLX ASR 已跑通 CLI；Hy-MT2 MLX 因输出结构合同失败继续保持实验。复查评测边界，完成人工盲评、取消／恢复与完整任务资源报告，再决定实验里程碑是否完成。不得改桌面默认 runtime 或降低 JSON 校验。
4. 解决 release CLI 的 sqlx proc-macro LINKEDIT 工具链问题，再做正式构建；Windows 需要独立实机验收。无需自动发布安装包或执行新的云端上传。

本机模型和评测输入、输出位于 Git 忽略目录，未提交媒体、模型、凭据。评测 Python 环境 `/private/tmp/atogaki-mlx-validation-env` 与临时数据库可能被系统清理；脚本、锁定依赖和模型 revision 已入库，可按本文件重建。

## checkpoint 恢复后的工程验证（2026-10-18）

用户已确认先完成工程验证，接缝人工听审、reference 与翻译盲评后补。核心新增回归通过后为 125 项通过／4 项忽略，Tauri 为 29 项通过／4 项忽略；前端 4 项通过，类型检查与生产构建通过，标准调试 App 构建通过。

修正与结构：

- 重做损坏块前失效旧的 final-cues 缓存；完整证据不变时保留缓存和稳定 cue ID。增加拼接输出路径故障的断言，恢复只重新拼接、不调用 ASR。
- GPU 失败保留原始 stdout／stderr、原因与实际设备；这些内容进入摘要保护的块证据。故障注入验证仅尝试一次 CPU，CPU 再失败即停止。
- 任务音频与原始输出用可写句柄同步，满足 Windows `FlushFileBuffers` 要求；增加跨平台保留文件内容的同步测试。[Microsoft API 合同](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-flushfilebuffers)。
- 从完整 ASR run 首次重开时使用真实 WAV 总时长，包含尾部静音；时间粒度从持久 TimedUnit 恢复。采用冲突仍检查冻结工作区，不自动刷新对照快照。
- 修正预设跨语言词表重载、内容包清空和流水线摘要过期；新的可疑范围清除旧 Gemini 授权。搜索空结果与真正空单词本分开显示，候选预览改为通用标题并保留可见采用按钮。
- 翻译设置提供独立后端卡片；兼容服务折叠到高级项，Hy-MT2 两档保持实验，可从卡片下载所需模型。凭据独立保存／删除不改变全局默认后端。就绪状态区分未配置、资源缺失、未验证、已验证与失败。验证结果绑定配置和凭据版本摘要，晚到的旧配置响应不能验证新配置；列卡片不读取系统凭据。测试只在显式点击后发送固定 `Hello.`，本轮未执行真实云端请求。

真实窗口记录（全部使用 `/private/tmp/atogaki-offline-window-20261018`）：

- 从两个已翻译测试任务收藏同一句，合并为一个条目、两个来源；展开全部例句，并从第二个来源返回正确任务播放。重启后条目与来源保留。
- 原生 Anki 保存面板成功导出 UTF-8 TSV，校验一行八列与两个任务来源。学习检索命中／无命中及总数文案通过。
- 当前句循环与 A/B 区间状态、关闭循环和恢复普通播放通过；全文搜索命中定位通过。
- 预设跨重启保留，从空词表恢复到指定词表后仍保留勾选的内容包，翻译摘要同步。
- 原生多选导入坏 WAV 与有效 WAV，同时排队两个任务。坏文件提取失败后有效文件继续识别并完成；未上传媒体或自动切云端。
- 历史候选可以重开并显示双栏原文、模型、范围和差异入口。原生自动化在部分 HTML 模态弹窗内缺失 AX 控件树，坐标点击返回 `noWindowsAvailable`；使用可见键盘焦点与截图继续完成候选采用冲突、显式重新对照及采用。确认冲突保留人工修改，重新对照不新增 ASR run。设置卡片中 DeepSeek 模型草稿保留，切到 DeepL 再切回时两侧 Key 草稿均为空；输入不中断。未保存测试 Key，也未调用连接测试。

MLX：130 秒真实节目片段的两块识别经同一 CLI 执行器完成。第二块故意失败，恢复仅重做未提交块；导出故障恢复未调用 ASR，cue ID 与第一块全部证据摘要保持一致。报告在 Git 忽略的 `local-artifacts/mlx/recovery/a7a3152a-0acb-4fcc-8f5a-edd2e6887afd/report.json`。该短样本补 MLX runtime 恢复证据，不替代上一轮 Whisper.cpp 62 分钟验收。MLX 显式拒绝尚未实现的 Silero VAD、CPU 和有限 token context 参数；no-speech threshold 传入真实 runtime。恢复 harness 固定 CLI 副本及摘要，保留 venv 启动路径，不把符号链接解析为系统 Python。Hy-MT2 MLX 的输出结构失败结论保持不变。

Release 工具链：根 CLI 与 Tauri 的独立 Cargo manifest 设置 `[profile.release.build-override] debug = 2`，仅对编译时依赖保留调试信息，最终二进制维持 release 优化。普通 `cargo build --release --bin atogaki-subtitle --offline` 与 `cargo check --release --manifest-path src-tauri/Cargo.toml --offline` 已通过；无需修改系统工具链。原因与 [Rust 上游 LINKEDIT 记录](https://github.com/rust-lang/rust/issues/157750) 一致。

额外的发布与采用保护：采用绑定 SQLite 工作区 revision 的事务快照，自动化包含改动后还原为相同内容的 ABA 冲突。历史 artifact 缺少 revision 时允许重开，但必须显式重新对照后才能采用，不伪造生成时版本。恢复补记初始 ASR 来源仅修改未编辑的 legacy 行，不覆盖人工来源、时间修改或 stale 对齐。旧 schema 使用原 SQLx 迁移及校验历史构建，升级并重复打开后验证人工字幕、cue ID、学习来源和 TSV 导出保留。翻译阶段恢复不再要求已完成 ASR 的模型／媒体仍存在；注入全部识别资源缺失后仅翻译待完成 group，并保留人工译文。

当前后续门禁：人工接缝听审、参考文本与翻译盲评由用户后补；Windows 继续只按 CI 共享核心／壳层结果记录，不代替原生窗口和 sidecar 实机回归。真实云端 Gemini 上传／删除及错误验收仍需另行授权样本。没有发布安装包，也没有将 MLX 或实验翻译模型设为桌面默认。

最终窗口补测确认：未保存草稿阻止候选采用；显式重新对照后的 artifact 保存工作区 revision（测试值 86），采用后同 ID 文本即时刷新，ASR run 数量仍为 2。模型重新定位先验证冻结摘要，选错模型不会写坏原识别配置，随后定位相同内容仍能继续。

## 2026-10-19：用户盲评与本机试用准备

用户已完成 12 条盲评并修正第 11 条自然度为 5。按匿名映射，7B 可接受 12/12、明确偏好 4 条；1.8B 可接受 11/12、明确偏好 1 条；7 条无明确偏好。第 6 条 1.8B 对未完成句补充了与语境相反的含义。历史候选均为 null，属于评测材料缺失，不能视为翻译失败或有效云端基线。原始评审保留在 Git 忽略的 local-artifacts/mlx/translation-e3fe3956-4781-46f6-ab1b-ef995dcc5fe7/blind-review.json，不修改用户评价。

用户授权将当前修改安装至 /Applications 直接试用，桌面维持 C++，MLX 暂缓。安装前重新执行格式、前端检查及 release App 构建、sidecar 与签名验证；安装与启动结果随后记录。正式数据库启动前通过 SQLite backup 保存一致性快照，保留旧 App 副本用于回退；数据库可能在首次启动时增量迁移，回退旧 App 时不得直接假设新 schema 兼容。

本机安装完成：`cargo fmt --check`、`git diff --check`、前端 4 项测试、类型检查及生产构建通过；`cargo tauri build --config tauri.macos-complete.conf.json --bundles app` release 构建通过，四个 sidecar（ffmpeg、ffprobe、whisper-cli、llama-server）齐全，源 bundle 与安装 bundle 均通过 `codesign --verify --deep --strict`，主程序逐字节一致。未执行公证或公开发行。

`/Applications/Atogaki.app` 已替换并通过原生窗口启动：正式数据目录和原有 12 个任务可见，新版任务预设／翻译选择／标题搜索及五个设置分区可见，窗口留在设置页供用户试用。旧 App 保留为 `/Applications/.Atogaki.previous.20261019`；另有 App 副本、完整性检查为 ok 的 SQLite 一致性备份与已完成盲评副本保留在 `local-artifacts/install-backups/20261019/`（Git 忽略）。不将启动检查视为完整人工功能验收。
