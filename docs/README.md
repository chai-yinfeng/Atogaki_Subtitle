# 文档索引

_更新：2026-11-14。_

当前行为看产品方向、能力矩阵、有效 ADR；当前进度看路线图；实验结论看对应提交和环境。在线实现已整合到开发主线，已发布稳定版仍为 v0.1.3；代码合入不等于在线质量验收或发布。

## 当前规范与开发入口

- [产品方向](product-direction.md)：场景、平台、数据边界，以及在线正确性优先的需求。
- [路线图](roadmap.md)：离线／编辑／学习／在线各自状态与下一步议题。
- [架构](architecture.md)：模块、处理链、存储、会话与前端责任。
- [2026-11-11 系统回顾](system-review-2026-11-11.md)：跨分支事实、Graphify 实测、在线瓶颈证据和同类工具调研；建议尚未实施。
- [稳定能力矩阵](app-capability-matrix.md)：主线桌面能力及研究晋升边界。
- [开发协作](development-workflow.md)、[桌面测试](desktop-testing.md)、[发布流程](releasing.md)、[许可证审计](third-party-license-audit.md)。
- [Windows 计划](windows-porting.md)与[实机记录](windows-testing.md)：Windows 已暂停维护，以下仅保留历史与恢复参考，见[0077](decisions/0077-pause-windows-development.md)。
- [主题资产](theme-material-assets.md)、[本机产物保留](local-artifact-retention.md)：来源与清理边界，未知数据不自动删除。

## 当前决策与实现依据

完整历史见[decisions/](decisions/)。后继记录取代旧合同的对应部分，不能将所有旧 ADR 并列视为当前默认。

| 主题 | 主要有效依据 |
| --- | --- |
| 本地优先／存储／密钥 | 0001、0002、0008、0014 |
| 来源、run、译文对应与修订 | [0043](decisions/0043-layered-pipeline-and-task-scoped-asr-runs.md)、[0048](decisions/0048-explicit-translation-scope.md)、[0049](decisions/0049-subtitle-autosave-and-scoped-adoption.md) |
| 恢复与阶段执行 | [0045](decisions/0045-resumable-offline-workflows.md)、[0052](decisions/0052-staged-concurrency-and-early-translation.md) |
| 统一编辑器／跟随 | [0055](decisions/0055-unified-subtitle-editor.md)–[0058](decisions/0058-compact-editor-and-playback-follow.md)；布局与跟随以后继 0058 为准 |
| Rust 模块／接缝／结构撤销 | [0059](decisions/0059-desktop-command-and-repository-modules.md)、[0060](decisions/0060-overlap-alignment-before-acoustic-selection.md)、[0061](decisions/0061-transactional-structure-undo-metadata.md) |
| 媒体传输／按需加载 | [0054](decisions/0054-request-batching-and-loopback-media.md)、[0062](decisions/0062-editor-audio-demand-loading.md) |
| 学习来源与词典 | [0032](decisions/0032-learning-reader-and-source-backed-collections.md)–[0036](decisions/0036-ecdict-pinned-base-data.md) |
| 研究晋升 | [0044](decisions/0044-stable-desktop-scope-and-cli-experiments.md)；在线分支的后继接入与限制见[系统回顾](system-review-2026-11-11.md#审阅范围与分支事实) |

## 在线实现与研究入口

- [在线处理计划](online-asr-plan.md)：当前 F／H／G／R、源时钟分组与阶段调度；早期提案按后继记录阅读。
- [0065 接入边界](decisions/0065-online-desktop-integration.md)、[0067 会话与采集](decisions/0067-online-session-owner-and-system-audio.md)、[0074 关联阅读](decisions/0074-paired-caption-reading-and-window-toggle.md)、[0075 失败隔离](decisions/0075-online-mt-validation-and-group-failure.md)、[0076 原文驱动调度](decisions/0076-source-driven-retranslation-and-stage-admission.md)。0076 取代早期历史／后文／调度的冲突部分。
- [在线分支整合前完整记录](archive/2026-11-12/online-roadmap-before-integration.md)、[two-pass 回放](two-pass-smoke-validation.md)、[Qwen MLX](qwen-mlx-smoke-validation.md)、[SenseVoice](sensevoice-smoke-validation.md)。
- [2026-11-13 Qwen 电台候选筛选](qwen-radio-candidate-screen-2026-11-13.md)：四档同输入计算／资源结果及首批可用性反馈。
- [0079 在线确认基线](decisions/0079-qwen-17b-8bit-online-baseline.md)、[2026-11-14 完整链路回归](qwen8-online-regression-2026-11-14.md)：开发默认收敛到 1.7B 8bit，13 分钟运行与结构检查通过，语义质量仍未验收，已安装 App 未覆盖。
- [Voxt 本地 ASR 源码评审](voxt-local-asr-review-2026-11-14.md)：发行版本的同权重核验、预览／确认、可选 DFlash、解码／资源限制、恢复与分发；只读调查，未迁移 runtime。
- [旧研究 0045](research/decisions/0045-independent-online-experiments.md)、[旧研究 0046](research/decisions/0046-controlled-online-asr-performance.md)：与主线同号不同决策，继续分目录保留。
- [模型分类与 Qwen 多文件管理](decisions/0078-model-categories-and-qwen-packages.md)已进入开发主线；runtime 分发、草稿独立准备和新机完整在线闭环仍未完成。

## 验收与质量证据

- [统一编辑器验收](unified-editor-acceptance.md)、[窗口／交互](editor-workspace-validation.md)、[性能](editor-performance-validation.md)、[本机安装](local-editor-installation.md)。旧未证明项不自动重开为当前待办。
- [三档 WebKit 对照](editor-version-comparison-2026-10-29.md)、[原生 Inspector 预检](editor-native-webkit-inspector-2026-10-29.md)：合成 IPC、回调间隔与实际呈现分别解释。
- [离线工作流](offline-workflow-validation.md)、[阶段管线](staged-pipeline-validation.md)。
- [评测方法与记录](evaluation.md)、[Hy-MT2](hy-mt2-validation.md)、[Gemini](gemini-transcribe.md)、[《教父》短片段](godfather-short-baseline.md)、[翻译消耗](translation-usage-observation.md)：工程合同、人工语义、时间与成本不能互相代替。

模型输出、媒体、数据库、截图与 trace 通常保存在忽略目录，文档归档不授权删除这些产物。实验是否实际运行及其口径，以各条带日期的记录为准。

## 仍可参考的研究

- [Aegisub 能力映射](aegisub-capability-map.md)：候选交互，不承诺完整复刻。
- [Web／Postgres 退役影响](legacy-web-retirement-impact.md)：已分析，尚未批准隔离／删除实现。
- [Muse Voice Transcribe 研究](muse-voice-transcribe-research.md)：保留研究时点的材料，不作为当前默认模型或可用性保证。

旧管线方案和翻译价格表已移出当前计划；不使用过期服务价格、模型推荐或“待实现”列表决定新开发。

## 历史与归档

[归档索引](archive/2026-11-11/README.md)列出已被取代的提案、已修复缺陷记录、路线图历史，以及清理前完整架构／路线图快照和后继依据。[发行记录](release-notes/)与[历次 ADR](decisions/)保留版本来源；归档正文中的状态是当时观察，不表示今天仍待完成。
