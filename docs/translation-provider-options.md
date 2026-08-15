# 翻译 Provider 候选调研

_调研日期：2026-08-11；产品选择与实现状态更新：2026-08-16。免费额度、模型名和服务条款会变化，发布前必须重新核对官方页面。_

Atogaki 的场景不是一般的短句查词，而是把较长的日语、英语或韩语口语节目分段翻译为简体中文。候选服务除了价格，还必须比较口语自然度、长任务限流、术语保护、数据处理条款和注册复杂度。

## 当前可用选项

| 方案 | 官方免费额度 | 注册与账单 | 适合程度 |
| --- | --- | --- | --- |
| [DeepL API Free](https://developers.deepl.com/docs/resources/usage-limits) | 每月 50 万字符 | 独立 API 账号与 Key | 已集成；文档翻译稳定，但电台口语可能偏书面。 |
| [DeepSeek API](https://api-docs.deepseek.com/zh-cn/quick_start/pricing/) | 按 Token 低价计费，具体价格随模型变化 | 国内可访问、人民币充值；官方提供 OpenAI-compatible API | 当前首个 LLM provider 预设；重点验证电台口语自然度、结构化段 ID 和术语保护。 |
| [阿里云机器翻译](https://help.aliyun.com/zh/machine-translation/product-overview/billing-overview) | 当前每月 100 万字符 | 国内账号与人民币账单 | 国内专用机器翻译备选；先与 DeepL／DeepSeek 做真实节目质量对比，再决定是否实现。 |
| [Azure Translator F0](https://azure.microsoft.com/en-us/pricing/details/cognitive-services/translator/) | 每月 200 万字符 | 需要 Azure 账号、订阅、资源 Key 与 region；F0 不产生超额翻译费用，但开户条件因地区而异 | 专用机器翻译 API 中最值得先做的第二个 provider；额度大，REST 接口明确，但自然度仍需真实节目 A/B 测试。 |
| [Google Cloud Translation](https://cloud.google.com/translate/pricing) | 每月前 50 万字符由 10 美元抵扣 | 必须创建 Google Cloud 项目并启用 Billing；不是独立的“免费翻译 API 注册” | 更像带免费抵扣的云计费服务；额度不优于 DeepL，暂不优先。 |
| [Amazon Translate](https://aws.amazon.com/translate/pricing/) | 首次请求起 12 个月内每月 200 万字符 | 需要 AWS 账号与云账单配置；到期或超额后按量收费 | 适合短期测试，不是长期永久免费方案。 |
| [Gemini Developer API](https://ai.google.dev/gemini-api/docs/pricing) | 部分 Flash / Flash-Lite 模型的免费层输入和输出不收费，另有限流 | 可从 Google AI Studio 获取 Key；免费模型和限流可能变化 | LLM 能按提示把口语译得更自然，也能控制语气和术语；但免费层内容会用于改进 Google 产品，不应默认用于敏感节目。 |
| [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/platform/pricing/) | 每天 10,000 Neurons | 免费 Workers 账号；超过每日额度会直接失败，付费计划才可超额 | 可作为 LLM 实验端点，但额度单位不直观，且需要处理模型选择、结构化输出和限流。 |
| [LibreTranslate](https://docs.libretranslate.com/) | 自托管时没有 API 费用 | 用户自行安装服务和语言模型；托管服务不是免费公共额度 | 隐私和可控性好，但会把模型下载、服务生命周期和 AGPL 合规带入桌面分发；质量需单独评估。 |
| [Ollama OpenAI-compatible API](https://docs.ollama.com/api/openai-compatibility) | 本机推理没有 API 费用 | 用户安装 Ollama 并下载合适模型；由本机算力承担 | 很适合未来的可选离线 LLM 翻译。Atogaki 可复用通用 OpenAI-compatible 接口，而不把 Ollama 或模型打进 DMG。 |

MyMemory 提供无需复杂 SDK 的公共 REST 接口，但[官方技术规格](https://mymemory.translated.net/doc/spec.php)限制单段输入为 500 bytes，返回内容还混合公共翻译记忆。它不适合作为长节目和隐私可预期的默认 provider，因此不进入首批实现。

## 推荐的实现顺序

1. 先把“LLM 翻译”设计成通用 OpenAI-compatible provider，而不是为每家 LLM 写一套核心逻辑。配置至少包括 Base URL、模型名、API Key、风格提示和是否允许把内容发送给第三方。
2. 首个预设使用国内可直接访问和充值的 DeepSeek，默认模型为 `deepseek-v4-flash`，关闭思考模式并要求带字幕段 ID 的 JSON 输出；对真实日语／英语电台与 DeepL 做 A/B，重点验证口语自然度、段落连续性、占位术语恢复、输出段 ID、重试和实际成本。
3. 保留自定义兼容端点和模型入口，让有条件的用户连接 OpenAI、Gemini 兼容网关或其他海外服务；Atogaki 不内置共享 Key，也不假设所有测试者具备海外网络和支付渠道。
4. 阿里云机器翻译作为国内传统机器翻译备选；Azure、Google Cloud 与 Amazon 不作为当前默认接入顺序。后续可用同一兼容接口接 Ollama，提供不把模型随 App 分发的本机翻译路径。

## 当前实现与待验收

- 已记录每个成功批次的 provider ID、返回模型、端点类型、段数、可得 token 和完成时间；API Key 继续只进入系统凭据库或当前进程环境。
- 已强制校验输入输出段 ID 一一对应；LLM 返回自由文本、重复 ID、遗漏 ID 或空译文时不会写入字幕。
- DeepL 继续使用其共享 `context` 参数；OpenAI-compatible adapter 显式发送前文、目标段和后文。工作区不假定 provider 具有相同的上下文 API。
- 风格提示与术语保护是两层能力：术语占位保证“不误译”，风格提示负责“更像自然口语”，不能互相替代。
- 仍需在同一真实节目片段上完成 DeepL／DeepSeek A/B。当前只完成了请求响应契约、失败重试、专名恢复和元数据持久化的自动化验证；没有 DeepSeek Key 时不能据此宣称翻译质量通过。
