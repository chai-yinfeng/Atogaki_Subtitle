# 文档索引

_更新：2026-10-29_

当前产品行为先看产品方向、能力矩阵与最新有效 ADR；完成度看路线图，验证范围看对应记录。研究提案不表示功能已实现，单次窗口截图或测试计数也不表示全平台门禁已通过。

## 当前规范与开发入口

- [产品方向](product-direction.md)：用户、场景、平台和数据边界。
- [当前路线图](roadmap.md)：本次编辑器改造状态、下一步与未完成门禁。
- [统一编辑器逐项验收](unified-editor-acceptance.md)：原计划与后继确认的要求、证据范围及未闭合项，不能用单次通过数替代整体验收。
- [稳定能力矩阵](app-capability-matrix.md)：桌面范围和 CLI／研究路线的晋升边界。
- [架构与目录约定](architecture.md)：应用层、SQLite、媒体、IPC 和前端生命周期。
- [开发协作与回归约定](development-workflow.md)、[桌面测试](desktop-testing.md)：开发与隔离数据工作方式。
- [发布流程](releasing.md)、[第三方许可证审计](third-party-license-audit.md)：版本化构建、材料与目标平台边界。
- [Windows 兼容计划](windows-porting.md)、[Windows 实机清单](windows-testing.md)：平台专属结果与待验收项，不能用 macOS 结果替代。
- [主题材质清单](theme-material-assets.md)：资产用途、来源与保留信息。
- [本机产物保留清单](local-artifact-retention.md)：路径、用途、版本与 checksum 等级；明确重复下载和构建缓存的清理边界。

## 编辑器的有效决策

完整目录见[架构决策](decisions/)。ADR 保留原历史，不把被取代的交互重新当作默认行为。

| 决策 | 当前适用范围 |
|---|---|
| [0055 统一编辑器](decisions/0055-unified-subtitle-editor.md) | 任务入口、唯一草稿与四区工作面；其中旧布局和播放不切换编辑对象已被 0058 取代 |
| [0056 虚拟列表与摘要](decisions/0056-editor-virtual-list-and-task-summary.md) | 视口 DOM、局部更新与轻量任务摘要；原 42 px 行高后来由 0058 紧凑布局改成 28 px |
| [0057 视图生命周期](decisions/0057-editor-view-lifecycles.md) | 当前段输入和按需工具 mount／dispose，后继原生焦点修正见 0058 |
| [0058 紧凑布局与跟随](decisions/0058-compact-editor-and-playback-follow.md) | 左视频、右波形与上下双语编辑、底部列表；默认跟随、手动锁定、输入保护与工具焦点 |
| [0059 Rust 模块边界](decisions/0059-desktop-command-and-repository-modules.md) | IPC 与 repository 渐进拆分，保留方法、SQL、revision 与事务合同 |
| [0060 音频块接缝](decisions/0060-overlap-alignment-before-acoustic-selection.md) | 重叠区先对齐后选择，无法可靠匹配时保留两侧观测并进入复核 |
| [0061 完整结构撤销](decisions/0061-transactional-structure-undo-metadata.md) | 已确认的来源／依赖恢复、单调修订与后端收据；token／UI 已接入，原生元数据恢复与即时上下文回填主要路径通过；剩余门禁见路线图 |
| [0062 音频按需加载](decisions/0062-editor-audio-demand-loading.md) | 编辑页音轨首次播放／定位才加载，保留唯一位置边界、视频预览和原生传输证据范围 |

## 验收证据与运行记录

这些文件按提交和测试环境描述观察结果；阅读时同时看限制和待验证项。

- [2026-10-29 三档生产 WebKit 对照](editor-version-comparison-2026-10-29.md)：指定基线、协议审查和成对前端指标；合成 IPC 与原生性能分别记录。
- [原生 WKWebView 时间线预检](editor-native-webkit-inspector-2026-10-29.md)：实际页面时间线、原始回调采样与帧事件缺失；不替代成对性能验收。

- [编辑窗口与交互](editor-workspace-validation.md)、[性能与接口](editor-performance-validation.md)、[本机安装与备份轮换](local-editor-installation.md)。
- [原文保存后的上下文过期显示缺口](editor-context-stale-display-bug.md)：c360cc9 原生复现与修复边界，后台传播已生效，轻量回填及 2a82da1 原生复验通过，保留修复前后证据。
- [离线工作流验证](offline-workflow-validation.md)、[阶段队列与提前翻译验证](staged-pipeline-validation.md)。
- [识别／翻译质量基线](evaluation.md)、[Hy-MT2 验证](hy-mt2-validation.md)、[Gemini 接入与验证](gemini-transcribe.md)。工程合同与人工质量结论分开。
- [《教父》短片段质量基线](godfather-short-baseline.md)：旧任务删除、独立五分钟片段和识别／分段／翻译逐层排查计划。
- [翻译消耗与缓存观测](translation-usage-observation.md)：云端前缀缓存、本地结果复用及 token 记录范围，不能用语义组数代替 HTTP 次数。

截图、trace、模型输出、媒体与数据库通常位于被忽略的 local-artifacts 或隔离临时目录；文档给出定位与用途，不把它们都当成发行资产或可删除缓存。

## 研究、能力对照与待讨论方案

- [结构撤销元数据范围](structure-undo-metadata-proposal.md)：来源与翻译依赖丢失的隔离审计、已采纳方案与替代方案背景；实际实施以 0061 和验收记录为准。
- [字幕工作面调研](editor-workspace-review.md)：改造前的审计与布局备选，已采纳部分由 0055–0059 和当前路线图承接。
- [Aegisub 能力映射](aegisub-capability-map.md)：当前独立实现的能力与后续候选，不是完整复刻或扩大制作软件范围的承诺。
- [管线演进计划](pipeline-evolution.md)：保留研究阶段的接口与质量门禁；已落地部分应对照后继 ADR 和验收记录，不按原审阅基线推断当前实现。
- [Web Serve／Postgres 退役影响](legacy-web-retirement-impact.md)：调用、兼容与依赖审计已完成；隔离 feature 或删除尚待讨论，本轮未退役。
- [Muse 流式 ASR 研究](muse-voice-transcribe-research.md)、[翻译 Provider 候选](translation-provider-options.md)、[视觉参考讨论](visual-reference-notes.md)：研究时点的材料，不等同当前产品默认项。

## 历史记录

- [2026-10-25 路线图完整快照](roadmap-history-2026-10-25.md)：保留本轮逐次模块交付、测试计数和安装状态的原文；当前代码与本机包版本以路线图为准。
- [2026-10-22 路线图完整快照](roadmap-history-2026-10-22.md)：保留逐次交付、旧复选框、技术债与当时测试计数；过期未完成项不自动进入当前计划。
- [发行记录](release-notes/)：各版本已发布的范围；本机试装不等于新的公开发行。
- [历次 ADR](decisions/)：原决策及后继取代关系，旧记录继续可追溯。

更新当前规范时同步检查路线图和有关能力映射；新增研究应标清提案状态，新增验收应记录提交、环境、证据、失败与局限。历史材料的更正通过后继记录说明，保留原始结果的来源。
