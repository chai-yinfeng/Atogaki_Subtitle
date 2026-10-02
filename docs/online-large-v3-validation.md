# Online large-v3 第一轮本机验证

日期：2026-10-02。范围是一次正式模型开发实验，尚未完成质量验收。

## 固定条件

开发集《湖吉の庭 Vol.1》00:15–01:15，共 60 秒。`prepare_case.py` 校验原媒体摘要，提取同一份 16 kHz mono PCM16 WAV；未使用《後書きラジオ》holdout。两轮 ASR 顺序运行，无 VAD／词表／人工修正，chunk 为 1 秒，计时使用上游 computationally-aware 默认模式。翻译服务只在 ASR 两轮结束后启动。

机器沿用 Apple M4／24 GB。Whisper-Streaming 使用 MLX large-v3／float16；SimulStreaming 使用官方 large-v3 checkpoint，其修改版 torch Whisper 默认在 Mac 上选择 CPU／float32，AlignAtt frame threshold 为 25，greedy／1 beam。源码、Python 与依赖以 `experiments/online/upstream-pins.json` 和两套 uv.lock 固定；模型来源和 SHA-256 见 [`model-pins.json`](../experiments/online/model-pins.json)。官方 checkpoint 已验证发布 URL 中的 SHA-256。

## ASR 观测

| 指标 | Whisper-Streaming | SimulStreaming |
| --- | ---: | ---: |
| 已确认文本输出次数 | 15 | 19 |
| 首次输出时间，相对回放起点 | 5.37 s | 3.69 s |
| emission − 估计 audio end，p50 | 7.52 s | 5.45 s |
| emission − 估计 audio end，p95 | 12.79 s | 11.35 s |
| 进程 wall time，包含加载与 warmup | 72.29 s | 79.55 s |

两套模型均完成完整范围回放并输出可读日语。WS 的 MLX adapter 会在细粒度单元间加入空格，出现局部边界重复；SS 保留较多标点。未听审全部原音，因此不把文本差异作为准确率结论。

这是两个上游系统的本机比较，runtime、设备、精度与解码默认值不同，不能把差异单独归因于 LocalAgreement／AlignAtt。时间戳也是 provider 估计值，末段时间范围有差异；不能宣称词级时间准确，也不能从没有输出的范围推断完整性。样本仅 60 秒，不代表长节目或泛化能力。尚未测量 peak memory 和 ASR／翻译并发资源竞争。

## Hy-MT2 翻译策略重放

使用现有 pinned llama.cpp 与 Hy-MT2 1.8B Q4_K_M，Metal，4096 context；固定 temperature 0、1024 output tokens、Chinese target。分别运行 boundary／revisable，更新间隔 5 秒，原始字符预算 240。所有产物保持与 offline 工作区分离。

| 输入／策略 | 请求数 | 活动组数 | 组替换更新数 | 请求总计算时间 |
| --- | ---: | ---: | ---: | ---: |
| WS / boundary | 2 | 2 | 0 | 1.88 s |
| WS / revisable | 10 | 2 | 8 | 4.36 s |
| SS / boundary | 4 | 4 | 0 | 1.63 s |
| SS / revisable | 12 | 4 | 8 | 3.88 s |

译文生成耗时来自串行日志重放，不是在线 end-to-end latency。相同 ASR 输入下，两种策略最终翻译相同；revisable 的区别是较早产生草稿并替换，不能据此声称最终质量提升。两个 ASR 的标点和空格导致分组不同，因此跨 backend 翻译结果还混合了 grouping 差异。

人工阅读 source／target 发现需要进一步验证的语义错误：Hy-MT2 1.8B 将历史经历中的否定译为能力限制。该判断仅针对模型提供的 source text，不代表原音 ASR 已被听审确认。裁剪终点位于未完成句，译文对尚未输入的语义存在补全倾向；本次不能把 EOF 冻结等同完整句成立。Harness 已增加 `completion_reason`，区分句末、字符预算、provider voice boundary 与 input EOF；原始实验产物仍保存修改前版本。

## 证据与后续

私人录音、完整转写、译文、依赖清单、请求参数和日志保存在 Git 忽略的 `local-artifacts/online/`，未上传云端，未修改正式 SQLite、offline workflow 或 GUI。验证服务已回收。

1. 延长到自然句末并覆盖完整短节目，检查空格／标点分组、词级时钟漂移、边界重复和 EOF flush。
2. 对相同且已校对的原文比较 Hy-MT2 1.8B／7B，检查历史否定、未完成句和跨活动组上下文；不把 ASR 错误混入 translation backend 评分。
3. 随后加入真实媒体时钟驱动的并发翻译、过期响应、背压与资源测量。

Gemini 3.5 Transcribe 后续作为独立 `model_reference`，与实际音频范围和来源绑定，供分歧听审。用户本轮明确选择先跑本地实验；没有调用 Gemini 或读取系统凭据。未经听审的模型参考不提升为 gold。

## 单段 Gemini 翻译候选（追加）

2026-10-02 用户明确授权调用一次 Gemini 翻译，并计划简单校对。选取上述 SimulStreaming 结果中的一个完整活动组，用 Gemini 3.5 Flash 对候选文本执行一次翻译；未上传音频，未调用 Transcribe，未重试。Key 仅从现有系统凭据库读取到进程内，不写入请求 artifact。

Gemini 对历史经历否定的表达更贴近候选 source；ASR 中与旅行／本次相关的同音用字仍需回听。请求、响应、模型版本和三方校对材料保存到 Git 忽略的 `local-artifacts/online/gemini-paragraph-review/`，原文与译文仍标为 unreviewed。此次候选不能证明 Gemini 的 ASR 正确性，也未提升为 gold。
