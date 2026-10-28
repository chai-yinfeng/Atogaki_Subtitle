# 本机产物清单与保留策略

日期：2026-10-22。盘点基线：`a519a87`。这是本机空间整理记录，不是产品数据清理或公开发行操作；此前安装备份轮换见[本机安装记录](local-editor-installation.md)。

## 范围与证据等级

盘点 32 个明确目录：当前／继承工作树和主检出的 local-artifacts、三套历史版本材料、当前及共享 incremental 缓存、已知隔离 QA／运行环境，以及正式 App／两份回退包。正式应用数据目录只记录目录元数据，不递归读取数据库、模型和任务内容。Downloads 只按 Atogaki 名称查找，本轮没有对应文件进入清理。

[只读盘点脚本](../scripts/inventory-local-artifacts.py)接受显式 roots 配置，记录用途、版本说明、保留策略、路径、大小、mtime、文件类型与链接目标。只有 `hash_contents: true` 的稳定材料计算文件 SHA-256；其他项为 null。metadata_manifest_sha256 仅用于核对目录元数据，不能据此认定文件内容相同或删除模型／数据库。脚本不跟随目录链接、不打开特殊文件、拒绝覆盖原清单；哈希时检查文件身份与大小／mtime，发现变化即失败。

完整路径与文件名可能包含私人数据，JSON 清单保存在忽略目录 `local-artifacts/artifact-retention-20261022/`：roots.json、inventory-before.json、cleanup-plan.json、逐项 cleanup-journal.jsonl、cleanup-result.json、inventory-after.json、apps-retained.json 和 verification.json。根目录有重叠，不能将各 root 的大小直接求和。

## 保留清单

下表的“继承工作树”指本次提供共享构建目录的 `7646/Atogaki_Sub`；当前工作树是 `3d0a/Atogaki_Sub`。完整绝对路径与元数据摘要见上述私有清单。

| 路径范围／用途 | 版本与校验 | 保留／处理 |
|---|---|---|
| 继承工作树 `local-artifacts/release/v0.1.0/` | 历史本地材料，DMG 与对应源码 SHA-256 见下表 | 原文件与 checksum 保留；没有按日期删除 |
| `release/v0.1.1/`、`releases/v0.1.2/` | 稳定发行材料；重新计算文件 hash，与各自 .sha256 相同；v0.1.2 还与 candidate-manifest.json 相同 | 权威 DMG、对应源码、checksum、manifest／说明全部保留 |
| v0.1.1 `download-check/`、v0.1.2 `downloaded/` | 8 个文件逐一与上级同名权威文件比对 size 与 SHA-256 | 已验证重复的内容改为相对链接，旧校验路径仍可读取同一字节 |
| 当前／继承工作树 `local-artifacts/install-backups/` | 独立恢复点与迁移／修复前数据；既有快照 hash／integrity 记录继续有效 | 全部保留，本轮没有再次合并或删除数据库 |
| `issue21*`、`recovery/`、`long-media-followup/`、`staged-pipeline/` | 故障与恢复证据、实际识别产物、已验证 checkpoint | 保留，不能只因目录陈旧视为缓存 |
| 主检出 `local-artifacts/evaluation/`、`hy-mt2/`，继承树 `mlx/` | 人工评审、冻结结果、研究模型；版本来自各自 manifest，未全量重算内容 hash | 保留；包含的模型、评审及来源不进入自动清理 |
| 当前树 `editor-*`、`action-confirmation-native-*`、docs-index 等 local-artifacts | 对应提交的截图、AX、trace、RSS、数据库核验 | 保留，不把验收证据当作构建缓存 |
| 已知 `/tmp/atogaki-*` 目录 | QA 数据、fixture、Instruments、格式修复备份、浏览器与 MLX 环境；部分用途未完全分类 | 全部保留，临时路径不等于可删除 |
| `/Applications/Atogaki.app` | 0.1.2；版本／identifier／主程序 hash 单独记录 | 保留，未覆盖或启动 |
| `.Atogaki.previous.20261021-modules` 与 `.Atogaki.previous-20261020T210406Z` | 0.1.2 上一版与 0.1.1 稳定回退；主程序 hash 单独记录 | 两份回退均保留 |
| 当前树 `target/debug/incremental/` | 仅 .o／.bin／.rmeta／.lock，活动构建为零且元数据与预检清单一致 | 本轮单独清理；下次编译可再生成，不删除 deps、测试日志或最终 bundle |
| 继承树 `src-tauri/target/debug/incremental/` | 多个 worktree 共用，约 4.3 GiB 逻辑内容 | 保留；不在本轮清理共享缓存或 target 整目录 |
| 正式应用数据目录及其中 SQLite、模型、采用的 ASR、媒体、主题原图 | 本轮仅目录元数据，未读取或重新哈希内容 | 保护，不自动清理；来源不明文件同样保留 |

## 权威版本材料 SHA-256

路径位于上述各版本根目录。source 是 `Atogaki-VERSION-third-party-sources.tar.xz`，DMG 是 `Atogaki-vVERSION-macos-arm64.dmg`；未修改材料或重新打包。

| 版本／材料 | SHA-256 |
|---|---|
| 0.1.0 source | `b2d40520330c58f6524e0813dafba86daaab5fdb497602364337c0140f31ac1f` |
| 0.1.0 DMG | `73728a098eee943297e06b634f76ed669c2fa3da2db03719a81ad9cddee9cde7` |
| 0.1.1 source | `bccfcc0d4892f7084cd423591e60f0c2d0ec01d8b84e6afc608bbd07c976c3f0` |
| 0.1.1 DMG | `fc7c1082354603b5d280df287a6b11a5b5075d32b380d29878aea1c06ec09b5c` |
| 0.1.2 source | `1ccc7daa28b29ecdb343668b4d4f05349d99487909a90a68cc34a3c7cdaf72cc` |
| 0.1.2 DMG | `db313f4ae10def142cbe009526279f459220ecb38f83a8d39424b37e6f54014c` |

下载副本替换前再次验证全部 8 对原文件／副本及其 hash，使用临时相对链接原子替换副本，逐项写 journal，再核对链接可解析及原文件 hash。不是仅按文件名、版本号或大小去重。原文件位于同一版本目录上级，不指向临时下载、网络位置或新生成的包。

## 本轮结果与后续使用

重复下载内容 255,422,553 字节（约 244 MiB），当前树 incremental 缓存 614,769,676 字节（约 586 MiB），合计约 830 MiB 逻辑内容。APFS 克隆、压缩和共享块影响真实占用，不把这当成精确空闲空间增量。权威发行文件 hash 均不变；26 个保留 root 的元数据清单不变，这不能扩展为对未哈希模型或正式数据库逐字节核验。

清理缓存前检查没有 cargo／rustc／rust-analyzer 进程，并重新比对限定目录的路径、类型、大小和 mtime；未触碰共享构建目录、最终 App、模型或数据库。再次清理应重做实时检查，不能复用旧的“无活动构建”结论。

只读脚本已用独立小夹具验证已知文件 hash、目录／文件链接不跟随、FIFO 不读取、已有输出拒绝覆盖；fixture 与结果保留在私有证据下。清理前后目录盘点、权威材料 hash 与链接目标复核通过，Cargo 格式及 diff 检查通过；本轮没有前后端业务源码改动，不重复声明新的运行时验收。

只读盘点可使用自己的私有配置重复执行：

```sh
python3 scripts/inventory-local-artifacts.py \
  --config /absolute/path/to/roots.json \
  --output local-artifacts/retention/new-inventory.json
```

盘点脚本始终没有删除入口；根配置中的 retention 仅为记录。后续清理必须基于新清单的明确候选和现有授权，保留恢复点、人工评审、权威发行来源及已采用识别证据。编辑器性能、模块与 Windows 门禁继续见[路线图](roadmap.md)。

2026-10-23 本机候选更新后，App 回退轮换为 `.Atogaki.previous.20261023-candidate`（86c46c8）与稳定版 `.Atogaki.previous-20261020T210406Z`（0.1.1）；在新包／数据验证后按完整摘要清除被取代的 `.Atogaki.previous.20261021-modules` 及本次暂存副本，约 193 MiB 逻辑内容。新增一份删除旧任务后的一致性 SQLite 快照，未复制模型／媒体。新清单与核验材料在 `local-artifacts/install-backups/20261023-candidate/`，详见[安装记录](local-editor-installation.md)；本页较早盘点中的 App 路径为历史状态，不能据此再次删除。


## 2026-10-24：自建原生滚动诊断清理

本轮仅处理刚生成于 `local-artifacts/editor-native-scroll-7eadbef/` 的 `scroll-input-webcontent.trace` 和 `scroll-webcontent.trace`。用途为 7eadbef 隔离 release QA 的 Animation Hitches 实验；两个录制器、全部导出进程已终止，所需 XML 表／TOC／动作时间与摘要已保存。原始容器未建立 QA 窗口的可靠帧归属，不是产品发行物、任务证据或恢复点。

第二份录制收尾临时占用约 21 GiB，保存后逻辑 1,864,598,230 字节、分配 1,885,097,984 字节；初始无有效滚动 trace 经索引／导出后逻辑 1,158,892,282 字节、分配 1,168,240,640 字节。逐文件 SHA-256、路径、用途和来源版本分别保存在 `raw-trace-retention.json`／`initial-trace-retention.json`，仅在核验录制器停止及明确输出路径后删除这两份自建容器。合计逻辑内容约 2.82 GiB、分配口径约 2.84 GiB，不能视为文件系统空闲空间的精确等额增加。

当前保留材料约 4.1 MiB，包含原生 AX／截图、导出 XML、动作及进程身份、包摘要、数据核验与可复算报告。最初曾计划保留较小的首次 trace，索引使其增大后按第二份独立清单继续清理；清理时序分别留痕。既有 WebKit 基线 trace、正式／隔离数据库、原视频、模型、已采用 ASR 证据及安装回退点均未进入本次清理。


## 2026-10-25：输入保护候选的本机回退轮换

正式安装 1020e05 后保留上一版 05b2e07 与稳定 0.1.1 两份 App；完整摘要匹配旧清单、保留包签名及正式 20 表验证通过后，清理 `/Applications/.Atogaki.previous.20261023-candidate`（86c46c8），逻辑内容 101,227,093 字节。暂存正式包经原子重命名成为当前 App，没有遗留另一份暂存包。现有 QA App 原位覆盖，没有追加 QA 副本。

本次一致性 SQLite 快照 SHA-256 与 20261023-candidate 完全一致，新恢复路径使用相对链接复用旧 34,852,864 字节文件；没有新增数据副本。保留目录、版本、用途、27 文件校验、轮换前后及快照链接凭据在 `local-artifacts/install-backups/20261025-time-guards/`，隔离原生证据在 `local-artifacts/editor-time-native-1020e05/`。没有清理模型、任务、媒体、采用证据或其他独立恢复点；逻辑内容不视为精确 APFS 空闲增量。

## 2026-10-26：token 原生验收现场

新增临时 QA `/tmp/atogaki-token-native-c360cc9/Atogaki.app`，来自 c360cc9 complete release bundle，签名通过，主程序 SHA-256 为 `dfcfb9455da289fd5e750e7ac19214e17febcac8308982803c9a70874773f2bd`；完整逐文件摘要见 `local-artifacts/editor-token-native-c360cc9/package.json`。这是当前原生 token 验收候选，正式 App 和旧 QA 均保留，新包尚未满足替换门禁。

独立视频现场为 `seed.json` 指向的 atogaki-token-qa-6eb49ab2-b019-49c7-99ec-0744bc68ebb4，另保留默认 WAV 种子的成功复跑目录（见 seed-audio-only.json），用于下一轮上下文状态显示缺口复验。只含合成媒体、字幕、来源和依赖；不是正式数据备份。AX、截图、SQLite 只读阶段 JSON 与核验脚本保留在上述证据目录，不另建整份 App／数据备份。

本轮两个种子失败目录（46efd2ae… 与 b6a035c9…）经路径、ID 和逐文件 checksum／用途核对后清理，共 3,172,916 字节逻辑内容；清单在 failed-fixtures-inventory.json，失败日志保留。它们均为本轮刚生成的合成测试数据，未涉及正式／既有 QA 数据、模型、真实媒体、人工评审、采用证据或恢复点；逻辑字节不等于精确磁盘释放量。

### 2a82da1 状态回填 QA 更新

c360cc9 QA 主程序 hash 与上一清单一致且已退出后，将 App 迁到固定临时路径 `/tmp/atogaki-editor-token-qa/Atogaki.app` 并原位更新为 2a82da1。新包签名和原生状态复验通过；没有追加另一份 App 或正式数据库备份。旧 `/tmp/atogaki-token-native-c360cc9/Atogaki.app` 路径已被取代，旧包 checksum、视频现场与元数据验收材料仍保留；较早路径不再表示当前 App 版本。

本轮复用前一轮 seed-audio-only.json 指向的成功合成数据，没有重新生成媒体／任务。新包的逐文件 checksum、版本、来源、原生阶段 JSON／AX／截图在 `local-artifacts/editor-context-state-20261026/native-package.json` 及同目录 manifest。正式 App、安装回退点、旧 QA 2950a6d、采用证据及真实媒体未修改或清理。

### 独立编译 QA 身份及旧临时包清理

2a82da1 业务代码以本轮 tauri.qa.conf.json 独立覆盖构建为 `/tmp/atogaki-editor-token-qa/Atogaki QA.app`；compiled identifier 为 com.chai-yinfeng.atogaki.qa，原生验证无环境参数也只进入独立空目录。原 `Atogaki.app` 临时包曾仅改 Info 标识以排查窗口绑定，已被新包取代；新包签名、默认目录、合成任务及 SQL 故障恢复通过并确认全部 QA 进程退出后，盘点并删除该旧包，101,571,045 字节逻辑内容。此前清理检查因 /tmp→/private/tmp 别名断言停止，修正为真实路径并复核后才删除。

逐文件旧包 checksum、Info 原件、默认目录 AX、配置 hash 与新包清单在 `local-artifacts/editor-native-gestures-2a82da1/`；固定临时目录中的 build.json 已更新新路径。默认 `.qa` 目录是测试存储，不是正式数据备份；既有合成任务目录、原生阶段证据、旧 2950a6d 性能基线、正式 App／数据和安装回退点保留。没有新增媒体、模型或正式数据库备份。


## 2026-10-27：磁盘满故障卷收尾

本轮专属 `/private/tmp/atogaki-editor-diskfull-20261026/test.dmg` 为 33,554,432 字节合成故障镜像。真实满卷验证后仅释放其中 32,260,096 字节填充文件；QA 正常退出、卸载完成、设备与标记复核后，记录镜像 SHA-256 并删除镜像。没有保留满卷，也没有新增正式数据备份。清理清单与 checksum 在 `local-artifacts/editor-diskfull-native-20261026/cleanup.json`；单份恢复合成库、JSON、AX、截图、日志与受约束操作脚本继续保留。既有合成媒体现场、当前 QA、旧性能基线及正式安装／回退点均未修改。


### 草稿重启验收现场

`local-artifacts/editor-draft-restart-20261027/seed.json` 指向本轮唯一合成数据库副本，复用既有合成媒体／证据，没有新增 App 或正式恢复备份。失败草稿缓存的只读快照只读取 QA identifier 的 WebKit 路径；成功显式保存并退出后缓存为空。临时故障 trigger 已移除，QA 已退出；小型合成库及恢复／冲突证据保留供复核。正式 WebKit、数据库、媒体、安装回退点未修改。


## 2026-10-27：近期模块集成 QA 原位更新

固定 `/tmp/atogaki-editor-token-qa/Atogaki QA.app` 从 2a82da1 更新为 7849794；旧包 27 文件清单匹配、无运行进程，新源包与更新后的包均签名通过后完成核对。没有增加 QA App 备份；共享 release bundle 是构建产物，固定路径用于实际窗口验收。旧包 manifest 与新 checksum 保留在 `local-artifacts/editor-native-integration-20261027/`，固定 build.json 更新到新版本。

新增唯一 integration 合成 SQLite 副本，复用原合成媒体与证据；插入／撤销后保留阶段 JSON、AX、截图及核验摘要。原合成库未变，QA 正常退出。没有新增正式数据库备份，正式 App、恢复点、模型／真实媒体／采用证据不在本轮清理范围。初始误建的 src-tauri/local-artifacts 测试日志空目录已用 rmdir 清理；没有删除未知内容。


## 2026-10-27：本机统一编辑器候选轮换

正式 `/Applications/Atogaki.app` 更新为 0cf72c0（仍为 0.1.2 本机候选）。已核对新包、上一版和稳定版签名／旧清单，只清理被取代的 `/Applications/.Atogaki.previous.20261025-time-guards`（05b2e07），逻辑 101,221,445 字节；保留 `/Applications/.Atogaki.previous.20261027-unified`（1020e05）及 `.Atogaki.previous-20261020T210406Z`（稳定 0.1.1）。暂存包通过重命名成为正式包，不遗留额外发行副本。

一致性数据库恢复点与旧 SHA-256 不同，保留新快照及旧恢复点；未把模型、媒体、数据库、采用证据或来源不明文件作为此次清理对象。完整路径／checksum／逻辑大小／回退轮换与数据验证在 local-artifacts/install-backups/20261027-unified，交付现场见[安装记录](local-editor-installation.md#2026-10-27统一编辑器与完整来源撤销候选0cf72c0)。


## 2026-10-28：波形跟随修复候选轮换

本机更新为 e8d9a8d；一致性数据库快照与上一 SHA 相同，链接复用 20261027-unified，未新增副本。App 保留 0cf72c0 上一版 `.Atogaki.previous.20261028-wave-follow` 与稳定 `.Atogaki.previous-20261020T210406Z`；完整核对后清理被取代的 1020e05 `.Atogaki.previous.20261027-unified`，逻辑 101,233,557 字节。路径／checksum／用途与操作记录在 local-artifacts/wave-follow-fix-20261028/rotation.json；正式数据库、模型、媒体及采用证据不参与清理。


### 2026-10-28：宿主帧归属短时诊断（保留）

`local-artifacts/editor-native-frame-map-20261028/host.trace` 为独立 QA e8d9a8d 的五秒 Animation Hitches 诊断，481 文件、逻辑 120,584,552 字节；路径／逐文件 SHA-256 见同目录 trace-files.json。保留源 trace、TOC、更新／swap／显示／queue 导出与采集成本记录，未进行清理。收尾暂态曾达约 6.2 GiB，准确中断 recorder 后保存完成；不作为 App 内存指标。没有新增 App 回退或数据库副本。
