# 2026-11-11 文档归档索引

本目录将已取代的计划和执行历史移出当前规范。正文保留当时的讨论、错误与证据，新增归档提示并修正相对链接；其中“待实现／未安装／未发布”不表示今天仍处于该阶段。当前入口见[文档索引](../../README.md)。

| 归档材料 | 归档原因／当前依据 |
| --- | --- |
| [旧管线演进](pipeline-evolution.md) | 2026-09-16 基线及待实现表已经过时；当前数据层与阶段执行见[架构](../../architecture.md)、0043／0045／0052／0060 |
| [Provider 候选](translation-provider-options.md) | 旧额度、价格、接入顺序未重新核验，不作为当前选型；已实现能力见[矩阵](../../app-capability-matrix.md) |
| [编辑器工作面提案](editor-workspace-review.md) | “尚未批准”状态已被 0055–0059 和 v0.1.3 取代；当前交互见 0058 与[验收](../../unified-editor-acceptance.md) |
| [结构撤销提案](structure-undo-metadata-proposal.md) | 旧前端迁移描述已经过时；当前合同见[0061](../../decisions/0061-transactional-structure-undo-metadata.md) |
| [视觉参考](visual-reference-notes.md) | 旧建议／试用讨论，不再作为独立实施计划；主题角色见 0050／0051 与[资产清单](../../theme-material-assets.md) |
| [上下文过期显示](editor-context-stale-display-bug.md) | 已修复缺陷及修复前后证据；当前状态见[窗口验证](../../editor-workspace-validation.md) |
| [波形跟随](editor-waveform-follow-interaction-bug.md) | 已修复并随发行交付，不再生成新待办 |
| [波形边界重读](editor-waveform-boundary-reload-bug.md) | 开头“正式包未更新”为阶段状态，后续已交付；[性能记录](../../editor-performance-validation.md)保留证明范围 |
| [10 月 22 日路线图](roadmap-history-2026-10-22.md)／[10 月 25 日路线图](roadmap-history-2026-10-25.md) | 旧执行快照，原文与 fragment 保留 |
| [清理前路线图](roadmap-before-review.md) | 完整保存发布、安装、测试与旧门禁流水，消除当前页面先后互相覆盖的状态 |
| [清理前架构](architecture-before-review.md) | 完整保存逐次模块抽取说明，当前[架构](../../architecture.md)聚焦系统责任与数据边界 |

ADR、发行说明、质量基线和详细验收不因文件较旧而删除；被取代 ADR 的历史继续可追溯。实验原始输出、模型、媒体、截图、trace 与数据库均未清理。尚未实施的 Web／Postgres 退役不是已淘汰代码，继续在当前研究入口保留。

在线分支的计划／评测和 ADR 尚存在阶段状态混杂，本轮只读核对并在[系统回顾](../../system-review-2026-11-11.md)注明，未跨工作区修改。未来合入或整理该分支时应沿用同一当前／历史边界，不能把主线的归档操作描述成已清理全部在线文档。
