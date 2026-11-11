# 文档索引

_更新：2026-11-11。_

当前行为看产品方向、能力矩阵、有效 ADR；当前进度看路线图；实验结论看对应提交和环境。在线实现仍在独立分支，不用主线文档否认其存在，也不把分支试装当作稳定发行。

## 当前规范与开发入口

- [产品方向](product-direction.md)：场景、平台、数据边界，以及在线正确性优先的需求。
- [路线图](roadmap.md)：离线／编辑／学习／在线各自状态与下一步议题。
- [架构](architecture.md)：模块、处理链、存储、会话与前端责任。
- [2026-11-11 系统回顾](system-review-2026-11-11.md)：跨分支事实、Graphify 实测、在线瓶颈证据和同类工具调研；建议尚未实施。
- [稳定能力矩阵](app-capability-matrix.md)：主线桌面能力及研究晋升边界。
- [开发协作](development-workflow.md)、[桌面测试](desktop-testing.md)、[发布流程](releasing.md)、[许可证审计](third-party-license-audit.md)。
- [Windows 计划](windows-porting.md)与[实机记录](windows-testing.md)：平台结果单独阅读；本轮不更新 Windows。
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

## 验收与质量证据

- [统一编辑器验收](unified-editor-acceptance.md)、[窗口／交互](editor-workspace-validation.md)、[性能](editor-performance-validation.md)、[本机安装](local-editor-installation.md)。旧未证明项不自动重开为当前待办。
- [三档 WebKit 对照](editor-version-comparison-2026-10-29.md)、[原生 Inspector 预检](editor-native-webkit-inspector-2026-10-29.md)：合成 IPC、回调间隔与实际呈现分别解释。
- [离线工作流](offline-workflow-validation.md)、[阶段管线](staged-pipeline-validation.md)。
- [评测方法与记录](evaluation.md)、[Hy-MT2](hy-mt2-validation.md)、[Gemini](gemini-transcribe.md)、[《教父》短片段](godfather-short-baseline.md)、[翻译消耗](translation-usage-observation.md)：工程合同、人工语义、时间与成本不能互相代替。

模型输出、媒体、数据库、截图与 trace 通常保存在忽略目录，文档归档不授权删除这些产物。在线最新评测在集成分支，本次主线回顾仅引用其已记录结果，未复跑。

## 仍可参考的研究

- [Aegisub 能力映射](aegisub-capability-map.md)：候选交互，不承诺完整复刻。
- [Web／Postgres 退役影响](legacy-web-retirement-impact.md)：已分析，尚未批准隔离／删除实现。
- [Muse Voice Transcribe 研究](muse-voice-transcribe-research.md)：保留研究时点的材料，不作为当前默认模型或可用性保证。

旧管线方案和翻译价格表已移出当前计划；不使用过期服务价格、模型推荐或“待实现”列表决定新开发。

## 历史与归档

[归档索引](archive/2026-11-11/README.md)列出已被取代的提案、已修复缺陷记录、路线图历史，以及清理前完整架构／路线图快照和后继依据。[发行记录](release-notes/)与[历次 ADR](decisions/)保留版本来源；归档正文中的状态是当时观察，不表示今天仍待完成。
