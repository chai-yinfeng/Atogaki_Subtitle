# 0025：结构化翻译 Provider 与运行记录

- 日期：2026-08-16
- 状态：采用

## 背景

DeepL 的 `context` 参数适合机器翻译，但 LLM provider 需要同时接收前后文、稳定字幕段 ID、口语风格和术语保护。若让工作区直接拼接各家请求，不同 provider 会把网络协议、重试和响应解析扩散到字幕持久化逻辑，也容易在 LLM 合并、遗漏或重排段落时把译文写错位置。

同时，真实节目 A/B 需要知道某次译文实际来自哪个 provider、模型和端点，并估算 token 成本；API Key 仍必须只保存在系统凭据库或当前进程环境中。

## 决策

1. 应用层只传递结构化 `TranslationRequest`：语言对、前文、带稳定 ID 的目标段、后文、风格提示和受保护术语。Provider 必须返回带相同 ID 的结构化结果。
2. 工作区统一校验结果数量、ID 唯一性、ID 完整性和非空译文；整次工作区写入继续使用 SQLite 原子更新，不能按 LLM 返回顺序静默对齐。
3. DeepL adapter 将同一结构化上下文编码到 DeepL `context`，目标字幕仍通过 `text` 参数翻译；OpenAI-compatible adapter 使用 Chat Completions JSON 输出，显式传递目标段 ID，并对空响应、无效 JSON 或网络失败重试一次。
4. DeepSeek 是首个国内 LLM 预设，使用官方固定 Base URL、可配置模型和默认关闭思考模式。另保留高级的 OpenAI-compatible Base URL／模型入口；Atogaki 不保证所有第三方的非标准兼容扩展。
5. 任务按成功的 provider 批次记录 provider ID、展示名、返回模型、端点类型、段数、可得的输入／输出 token 和完成时间。记录不包含请求正文、译文副本或 API Key。
6. API Key 按 provider ID 延迟读取系统凭据库，并缓存到当前进程；启动和只读浏览不会触发凭据读取。SQLite 只保存“该 provider 曾保存 Key”的布尔标记和非敏感配置。
7. 任务词表的受保护词先替换为不透明占位符；只有响应完整保留每个占位符时才恢复原词并允许写入。

## 影响

- 新 provider 可以复用同一工作区和 UI，不需要改变字幕写库协议。
- DeepL 与 LLM 的上下文编码仍由各自 adapter 决定；不能假定所有 provider 都支持 DeepL 式 `context`。
- 一次“全部翻译”可能产生多条批次记录。它们描述真实 API 调用，不等同于一份可恢复的翻译版本历史。
- 自定义 OpenAI-compatible 入口会把字幕发送到用户填写的第三方端点，设置界面必须持续明确这一点。
- 真实质量、费用和限流结论必须使用同一节目片段 A/B；契约测试只能证明对齐和失败边界，不能替代质量验收。
