# 翻译 Provider 候选调研

_调研日期：2026-08-11。免费额度和服务条款会变化，实现前必须重新核对官方页面。_

Atogaki 的场景不是一般的短句查词，而是把较长的日语、英语或韩语口语节目分段翻译为简体中文。候选服务除了价格，还必须比较口语自然度、长任务限流、术语保护、数据处理条款和注册复杂度。

## 当前可用选项

| 方案 | 官方免费额度 | 注册与账单 | 适合程度 |
| --- | --- | --- | --- |
| [DeepL API Free](https://developers.deepl.com/docs/resources/usage-limits) | 每月 50 万字符 | 独立 API 账号与 Key | 已集成；文档翻译稳定，但电台口语可能偏书面。 |
| [Azure Translator F0](https://azure.microsoft.com/en-us/pricing/details/cognitive-services/translator/) | 每月 200 万字符 | 需要 Azure 账号、订阅、资源 Key 与 region；F0 不产生超额翻译费用，但开户条件因地区而异 | 专用机器翻译 API 中最值得先做的第二个 provider；额度大，REST 接口明确，但自然度仍需真实节目 A/B 测试。 |
| [Google Cloud Translation](https://cloud.google.com/translate/pricing) | 每月前 50 万字符由 10 美元抵扣 | 必须创建 Google Cloud 项目并启用 Billing；不是独立的“免费翻译 API 注册” | 更像带免费抵扣的云计费服务；额度不优于 DeepL，暂不优先。 |
| [Amazon Translate](https://aws.amazon.com/translate/pricing/) | 首次请求起 12 个月内每月 200 万字符 | 需要 AWS 账号与云账单配置；到期或超额后按量收费 | 适合短期测试，不是长期永久免费方案。 |
| [Gemini Developer API](https://ai.google.dev/gemini-api/docs/pricing) | 部分 Flash / Flash-Lite 模型的免费层输入和输出不收费，另有限流 | 可从 Google AI Studio 获取 Key；免费模型和限流可能变化 | LLM 能按提示把口语译得更自然，也能控制语气和术语；但免费层内容会用于改进 Google 产品，不应默认用于敏感节目。 |
| [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/platform/pricing/) | 每天 10,000 Neurons | 免费 Workers 账号；超过每日额度会直接失败，付费计划才可超额 | 可作为 LLM 实验端点，但额度单位不直观，且需要处理模型选择、结构化输出和限流。 |
| [LibreTranslate](https://docs.libretranslate.com/) | 自托管时没有 API 费用 | 用户自行安装服务和语言模型；托管服务不是免费公共额度 | 隐私和可控性好，但会把模型下载、服务生命周期和 AGPL 合规带入桌面分发；质量需单独评估。 |
| [Ollama OpenAI-compatible API](https://docs.ollama.com/api/openai-compatibility) | 本机推理没有 API 费用 | 用户安装 Ollama 并下载合适模型；由本机算力承担 | 很适合未来的可选离线 LLM 翻译。Atogaki 可复用通用 OpenAI-compatible 接口，而不把 Ollama 或模型打进 DMG。 |

MyMemory 提供无需复杂 SDK 的公共 REST 接口，但[官方技术规格](https://mymemory.translated.net/doc/spec.php)限制单段输入为 500 bytes，返回内容还混合公共翻译记忆。它不适合作为长节目和隐私可预期的默认 provider，因此不进入首批实现。

## 推荐的实现顺序

1. 先增加 Azure Translator：它是专用机器翻译 API，长期免费额度在当前候选中最实用，适合与 DeepL 对同一节目做 A/B 对比。
2. 同时把“LLM 翻译”设计成通用 OpenAI-compatible provider，而不是为每家 LLM 写一套核心逻辑。配置至少包括 Base URL、模型名、API Key、风格提示和是否允许把内容发送给第三方。
3. 首批 LLM 实测可以使用 Gemini Flash-Lite，重点验证口语自然度、段落连续性、占位术语恢复、输出段数一致和重试行为；免费层的数据使用提示必须在配置页明确展示。
4. 后续用同一 OpenAI-compatible 接口接 Ollama，提供不把模型随 App 分发的本机翻译路径。

## 实现前需要补齐的持久化

- 翻译批次应记录 provider ID、模型名、端点类型和完成时间，但绝不记录 API Key。
- 每次批量翻译必须校验输入输出段 ID 一一对应；LLM 返回自由文本时不能依赖数组顺序静默写库。
- 不同 provider 的语言代码、请求大小、并发和重试策略放在各自 adapter 中，工作区继续只依赖通用 provider 接口。
- 风格提示与术语保护是两层能力：术语占位保证“不误译”，风格提示负责“更像自然口语”，不能互相替代。
