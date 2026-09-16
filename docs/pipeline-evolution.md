# 处理管线演进与质量验证计划

_最后更新：2026-09-16_

## 范围与当前基线

架构方向见 [0043](decisions/0043-layered-pipeline-and-task-scoped-asr-runs.md)。第一阶段保持源／译 cue 对应，完整和局部 ASR 重跑作为同一任务下的独立 run；本文件描述待实现工作，不表示新模型、接口或迁移已交付。

审阅基线为 `4b41597`，最近发行记录为 Windows `v0.1.0-alpha.10`。Rust library 回归 86 通过、3 忽略，格式和前端类型检查通过；这些检查不替代新模型质量、sidecar 或 Windows 实机验收。

| 当前代码边界 | 现状 | 演进职责 |
| --- | --- | --- |
| `infrastructure/whisper.rs` | 解析 offsets/text，直接输出 `TranscriptSegment` | ASR adapter 输出独立 artifact，保留可得的 token／时间／质量字段 |
| `application/transcription_options.rs` | 通用名字下包含 Whisper 模型路径与专用参数 | 通用请求与 provider 专用配置分开，旧选项可读 |
| `domain/segment.rs` | 合并／拆分字幕，长文本按字符比例分配时间 | 时间来源明确的 `SubtitleSegmenter`，旧算法作为兼容策略 |
| `application/job_runner.rs` | 直接调用 Whisper，CLI 翻译直接走 DeepL | 注入应用层 provider；CLI／桌面共享用例，保留兼容入口 |
| `application/local_workspace_service.rs` | 12 段批次，±30 秒／2,000 字上下文 | 语义 grouping、上下文计划和传输批次分开 |
| `application/local_retranscription_service.rs` | 会话候选、隔离文字历史、事务采用 | 独立 run 与持久审查证据，保留预览和冲突检查 |
| `infrastructure/local_db.rs` | cue 主数据与批次元数据；译文写回核对目标原文 | 增量迁移、来源关联和完整输入／人工编辑 revision 校验 |

## 数据流与接口草案

```text
Audio asset → Acoustic plan → ASR run → Raw artifact + TimedUnits
                                          ↓
                              Subtitle segmentation
                                          ↓
                                Editable source cues
                                          ↓
                         Translation groups + context dependencies
                                          ↓
                             Translation run → Cue translations
```

| 概念／接口 | 最小契约与边界 |
| --- | --- |
| `AcousticPlanner` / `AcousticChunk` | 音频身份、读取范围、负责输出范围、重叠策略、原媒体时间映射；VAD region 不等于推理 chunk |
| `OfflineAsrProvider` / `AsrRun` | 输入范围、语言、冻结配置、上下文策略、取消；返回 artifact 引用和能力信息；失败也保留 run 状态 |
| `TimedUnit` | 文本、token/word 等单位类型、可选起止时间、时间来源与精度、可选 provider 质量字段；未知不填零冒充有效值 |
| `SubtitleSegmenter` | 由 timed units 生成 cue 候选与来源映射；不直接覆写人工工作区 |
| `SourceCue` / revisions | 稳定 ID、原文／时间／人工译文等修订信息；机器证据与编辑后的文字分开 |
| `TranslationPlanner` | 有序目标 cue、语义 group、实际上下文及版本、术语／风格快照和模型预算 |
| `TranslationProvider` | 按 cue ID 输出、能力描述、冻结的配置、用量和错误；provider 不决定数据库写入 |
| `AsrReviewService` | `QualitySignal`、`ReviewDecision`、`RepairAttempt` 及其关联范围／版本 |
| `StreamingAsrSession` | 音频帧输入、hypothesis 更新、稳定性信号、提交、结束、取消与断线事件 |

名称为实现草案，先验证边界再确定 Rust 类型与 SQLite schema。部署位置（local/cloud）、模式（文件/实时）、时间精度、diarization、词汇提示、结构化输出等作为独立能力描述，不能仅从 provider 名称推断。

初期允许 Whisper 管理内部窗口，以 provider-managed 标识未知的内部切块，不因建立数据层而立即重写 VAD。后续显式切块时区分读取和输出范围，对重叠去重，并保留剪切／拼接后的时间映射。不能假定所有 chunk 严格长于所有 cue。

ASR artifact 的模型文本保留原样。词表规范化和人工编辑记录派生关系；无法保持文字对齐时标记 alignment stale。forced alignment 可以作为后续补充能力，但不作为旧任务迁移的前提。日语 token 不等于空格分词后的 word。

## 工作区、依赖与迁移

一个任务只有一个当前编辑工作区，但可引用多个 run 的局部结果，因此不应只用一个 `active_run_id` 代表所有 cue 来源。每次完整／局部重跑都有独立身份和冻结输入；中断重试产生关联的新尝试，不改写旧证据。采用候选记录受影响范围、父来源和当前工作区 revision。

迁移先增加 run／artifact／来源索引，再把旧 `TranscriptSegment` 留作兼容投影。旧任务没有 token 时标记 legacy/unknown；只有确实存在的原始产物才可导入，不从 cue 时间推导历史词级真值。工作区原文、译文、时间编辑、cue ID 和学习收藏来源快照必须保留。重分段需要预览；来源 cue 消失时保留学习快照与时间定位，不按相近文字静默重绑。

翻译计划冻结目标 cue、上下文内容与 revision、group 策略、语言对、术语、风格、provider/model 配置。运行期间切换全局 provider 不应改变已经开始的 run。写回除目标原文外，还需校验人工译文 revision，避免覆盖等待期间的校对。失败恢复按已完成输出检查点继续，并复核计划仍有效。

原文修改标记目标译文过期，并对实际引用它的上下文依赖失效；上下文失效与目标原文变化分别记录。时间修改若不改变文字、分组和已消费上下文，不必使译文失效；若策略依据时间选取上下文，则重算受影响计划。拆分／合并保留来源关系，并重建受影响分组。任何失效标记都不自动清除人工译文。

group 首先采用可解释的句末、停顿、长度／预算边界；未可靠取得的 speaker/topic 信息不作为硬条件。稀疏重译时也从完整时间轴建立 group，不能把相隔很远的待译 cue 当成连续语境。group 超出预算需明确拆分并记录，而非由网络 adapter 任意截断。批量承载方式依 provider 能力不同，但必须保持每个 cue 的可验证对应。

## ASR 质量审查与修复

质量信号记录范围、检测器版本、证据与可选分数，例如重复循环、边界重复、异常字速、疑似静音出字；人工评审也可主动创建问题，不要求先有自动检测。不同 provider 的 confidence 不直接横向比较。审查结论记录待审、确认问题、误报或已处理，以及人工／自动来源。

修复尝试关联原 run、问题范围和新 run。可读取更宽音频作为上下文，但实际替换范围明确且不静默扩大；采用仍检查当前 cue 边界和 revision。多次候选比较不覆盖原工作区，旧判断随源版本变化标记过期。第一阶段不自动重试无限循环、不自动采用高分候选。

## 模型路线与部署体验

| | 本地 translation | 云端 translation |
| --- | --- | --- |
| 本地 ASR | Whisper → Hy-MT2（待实现） | Whisper → 现有 provider |
| 云端 ASR | Gemini Transcribe → Hy-MT2（待实现） | Gemini Transcribe → 云端 provider（待实现） |

文件转录属于离线工作流，但云端 ASR 仍上传音频。选择云端翻译不等于授权上传音频；界面分别说明音频、文本、术语等发送内容和目的端点。不因本地失败自动切换云端。Gemini 免费层及数据条款见官方链接；免费额度、地区条件和模型可用性在实现／发布时复核，不写成长期保证。

Hy-MT2 验证阶段由开发流程完成 llama.cpp 下载／构建、模型下载、校验、启动和配置，并提供可复现脚本；不要求产品用户手动编译或长期维护后台服务。先从 1.8B 做启动与协议验证，再按实测资源比较 7B，不预先锁定量化档或默认质量结论。固定 runtime commit、模型 revision、文件摘要、模板和采样参数；官方说明中的 STQ kernel 依赖需按实际选用的 GGUF 逐项验证。

产品化再加入应用管理的 runtime／模型下载、取消／续传／完整性校验、进程生命周期和卸载；提供现有模型路径／自备端点作为高级入口。runtime 是否随包或首次下载由平台构建、体积和许可证审查决定，模型保持按需下载。对本地服务验证仅监听 loopback、进程健康与退出回收；不假设 OpenAI-compatible 就能保证 cue JSON 和术语占位符可靠。评估 CPU／Metal、首次加载、峰值内存、与 ASR 并发竞争，Windows 单独验收。

Muse 指 **Meta Muse Voice Transcribe**，不是 MUSE 词向量项目，也不等同于 SeamlessStreaming。Meta 官方开发者目录确认其流式 speech-to-text 与 diarization；详细模型页本次读取失败，官方介绍正文返回 429，因此不把时间粒度、endpointing 协议或开放权重状态写成已核实能力。研究重点是如何表达流式转录、说话人信息和结束／稳定信号，暂不承诺新增正式 provider。SeamlessStreaming 留作独立研究线索。

## 实时边界

先以录音模拟流式输入，再接入真实采集。维护 candidate → stable → committed；稳定策略可替换，provider finalized 是输入信号，不等于离线正确性。原文与译文分别维护提交边界。离线精修创建新 run／revision，不覆写实时证据。

SimulStreaming 的 LocalAgreement 采用连续输出公共前缀，AlignAtt 使用 attention 信号；只在后端提供所需信号时采用，第一阶段不锁定算法。会话须保存媒体时钟与音频，处理背压、重连、重放去重、gap 和结束 flush；音频缺口不能伪装成完整录制。Gemini Live Transcribe 可作为实时 ASR 候选；Gemini Live Translate 单列 experimental，E2E 结果不替代两段式正式资料。

## 质量基线：原视频先行，少量人工参考随后补齐

不需要先收集大量完整标准字幕。建立三层验证：

1. **原媒体回放集**：比较耗时、资源、失败恢复、长音频循环和结果变化。没有参考文本不能报告 CER／WER 或绝对准确率；结果差异本身也不是质量提升。
2. **小规模人工参考集**：从真实节目选 5–10 分钟，逐段复核原文、专名与必要时间边界。不确定／听不清和多人重叠单独标记，不强行猜写。日语 CER 需固定标点、空格、数字和正字规范；WER 只有分词规则固定后才可比较。逐词时间准确度需要相应人工时间标注，普通字幕时间不能直接充当词级真值。
3. **翻译审查集**：先用相同且已校对的原文和 cue 固定 ASR 因素，盲评完整性、专名、指代、跨 cue 连贯、自然度及人工修订成本；可有多个合理译法，不把某一份中文字幕作为唯一答案。再做端到端测试，区分 ASR 错误传播和翻译错误。自动模型评审只作辅助并记录版本。

2026-09-16 已只读查看用户提供的本机节目目录，并用现有 ffprobe 读取时长：排除“翻译”目录后有 13 个 MP4。以下只是按文件名、时长与既有故障记录选出的候选，尚未听审或冻结测试片段：

| 候选媒体 | 约时长 | 初始用途 |
| --- | --- | --- |
| 湖吉の庭 Vol.1 | 4.5 分钟 | 与历史 DeepL／DeepSeek 测试联系；短任务完整回放 |
| 後書きラジオ | 9.6 分钟 | 首要用户场景、专名和口语样本 |
| 京吹完結直前 YouTube LIVE | 62.4 分钟 | 长任务、短残片翻译与恢复回归 |
| 京吹广播特别篇 | 55.0 分钟 | 对照历史循环问题，具体故障区间待确认 |
| Daily English Podcast | 16.6 分钟 | 英语分词和跨语言冒烟回归 |

先选约 20–30 分钟代表性片段用于日常 A/B，另保留至少一个完整长节目验证上下文累积故障；从短片段中建立上述人工参考。具体区间需听审后冻结，并保留未用于调参的片段。带可信字幕的媒体可以后续补充，但要确认字幕是否逐字、是否删改、是否同版本；现有双语烧录和导出只能先标记 previous-output，未经人工复核不能当 gold。

本机媒体路径、裁剪、模型和结果放在 Git 忽略的 `local-artifacts/` 或既有任务目录，原视频只读、不上传；仓库仅保存流程、可公开的清单格式和指标定义。评估 manifest 后续记录媒体摘要、范围、来源权限、参考状态／版本、模型／参数／策略版本和结果路径。本轮没有生成参考字幕、调用付费模型或下载模型。

## 分阶段执行与验收

| 阶段 | 交付 | 完成条件 |
| --- | --- | --- |
| P0 基线 | 媒体清单、固定片段、旧版结果、少量参考与评分规则 | 原媒体只读；参考来源明确；长节目和未调参片段均覆盖 |
| P1 ASR 边界 | provider trait、Whisper adapter、run／artifact／TimedUnit、兼容迁移 | 旧任务打开不丢编辑；新增 run 不改当前 cue；不改变现有切块策略；失败／重启证据可查 |
| P2 cue 与翻译依赖 | 来源映射、revision、group planner、批次恢复与写回校验 | 上下文和人工译文并发编辑受保护；保持 cue 对应；CLI／桌面通过共享用例 |
| P3 本地翻译 | 固定 llama.cpp／Hy-MT2 组合、自动配置脚本和 adapter；随后应用模型管理 | 离线运行；结构／术语验证；资源与质量对照；不要求用户手工维护服务 |
| P4 审查与第二 ASR | 审查数据、持久候选、任务内 run 对照、Gemini 文件转录 | 音频上传选择清楚；候选过期拒绝采用；范围外编辑和收藏保留 |
| P5 实时研究 | 录音回放 harness、提交策略、采集／Live ASR、E2E 实验 | 可测延迟与回改；断线／结束无静默漏段；离线精修独立 revision |

P0/P1 先推进；P3 的 runtime 兼容性小实验可以提前，不能据此提前替换默认翻译路线。每阶段按小提交交付，先运行相关回归再提交／推送；模型真实质量与合同／schema 测试分别报告。当前未完成项包括具体 schema、质量阈值、模型档位、Muse 协议细节和实时 policy，不将它们写成已验证承诺。

## 官方研究来源

核对日期：2026-09-16。未进行任何外部媒体上传或真实模型调用。

- [Hy-MT2 仓库与部署说明](https://github.com/Tencent-Hunyuan/Hy-MT2)：本地模型与提示／部署路线；实际 runtime 兼容性仍需实测。
- [Gemini API 价格](https://ai.google.dev/gemini-api/docs/pricing)、[数据条款](https://ai.google.dev/gemini-api/terms)、[Live transcription](https://ai.google.dev/gemini-api/docs/live-api/live-transcribe)：免费层、云端数据边界和 interim/finalized 协议。
- [Gemini Live Translate](https://ai.google.dev/gemini-api/docs/models/gemini-3.5-live-translate-preview)：speech-to-speech 实验路线。
- [Meta 官方开发者目录](https://developers.meta.com/resources/blog/)：Muse Voice Transcribe 的 streaming ASR／diarization 公告；[公告正文](https://developer.meta.com/ai/resources/blog/meet-muse-voice-transcribe-streaming-speech-to-text/)本次返回 429，后续补查详细协议。
- [SimulStreaming](https://github.com/ufal/SimulStreaming)、[SeamlessStreaming](https://github.com/facebookresearch/seamless_communication)：独立研究线索，不表示引入其运行依赖。
