# 桌面 App 稳定能力矩阵

_冻结日期：2026-09-18_

当前桌面 App 面向本地文件的转录、翻译、校对、学习和导出。它保留四种 provider 组合需要的基础能力，但云端 ASR 目前只用于用户明确选择范围的候选修复，不作为新任务的全片默认入口。

| 层级 | 本地路线 | 云端路线 |
| --- | --- | --- |
| ASR | Whisper.cpp：新任务全片识别、selected-range 独立 run、VAD、词表提示、GPU→CPU 回退 | Gemini 3.5 Transcribe：selected-range 独立 run、真实 word timestamps、可选 speaker 数据、每次范围单独授权上传 |
| Translation | Hy-MT2 1.8B／7B：App 管理模型，受管 loopback `llama-server`，translation groups 与结构门禁 | DeepL、DeepSeek、OpenAI-compatible：系统凭据、translation groups、上下文／revision 校验 |

因此当前可验证的组合是：

1. Whisper → Hy-MT2：完整本地路线。
2. Whisper → 云端 translation：本地音频识别，只有原文、上下文和词表保护数据发送给翻译 provider。
3. Gemini selected range → Hy-MT2：云端生成候选原文，经用户采用后在本地翻译。
4. Gemini selected range → 云端 translation：云端 ASR 与云端翻译分别授权；选择其中一个不会自动授权另一个。

桌面 App 同时稳定保留任务队列、run／质量信号审查、字幕与时间轴编辑、translation stale 保护、学习资料、SRT／ASS 导出和视频烧录。

以下能力不进入当前桌面范围：Gemini 全片新任务、Gemini Live、Muse Voice Transcribe、实时音频采集、E2E live translate、新 ASR／MT 模型试验、可变 prefix-commit policy，以及只为跑 benchmark 暴露的参数。它们先通过 CLI/evaluation harness 产出可复现 artifact 和报告。

## 桌面晋升门禁

CLI 实验进入 App 前必须给出固定输入、配置快照、质量对比、性能和资源数据、失败恢复、隐私边界及迁移方案。结构合同测试不能替代真实质量结论。晋升需要单独的产品决策，不因 core 已实现 provider 就自动增加 UI 入口。
