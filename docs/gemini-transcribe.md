# Gemini 3.5 Transcribe 接入与本地验证

_核对日期：2026-09-17_

Atogaki 将 Gemini 作为可选的云端文件 ASR provider。它与云端翻译授权分开：保存 Gemini Key 只表示凭据可用，每次局部重识别仍需对界面显示的音频范围单独勾选上传授权。本地 Whisper 失败不会自动改走 Gemini。

## 注册免费层 API Key

1. 打开 [Google AI Studio](https://aistudio.google.com/apikey)，使用 Google 账号登录并接受 Gemini API 条款。
2. 新用户通常会得到一个默认 Google Cloud project；已有 Cloud 账号也可以在 AI Studio 的 Projects 页面导入项目。
3. 在 API Keys 页面为该项目创建 Key。Key 始终属于一个 Cloud project；不要把它写入源码、命令行历史或任务目录。
4. 在 Atogaki 设置的“云端 ASR”中保存 Key。App 只将它写入系统凭据库，SQLite 仅保存“不含秘密的已配置标记”。

官方资料：[API key 指南](https://ai.google.dev/gemini-api/docs/api-key)、[Audio transcription](https://ai.google.dev/gemini-api/docs/transcribe)。

## 免费层和数据边界

官方价格页当前将 `gemini-3.5-transcribe` 和 `gemini-3.5-transcribe-live` 的 free tier 输入、输出都列为免费。免费不等于无限：rate limit 按 Cloud project 计算，包含 RPM、TPM 和 RPD 等维度，RPD 在太平洋时间午夜重置；Google 要求在 AI Studio 查看项目当前的 active rate limits，因此仓库不冻结一个可能失效的具体数字。

免费层的内容会用于改进 Google 产品，付费层价格表则标为不会用于此用途。用户必须在上传前看到这一差异。参考：[价格与数据使用](https://ai.google.dev/gemini-api/docs/pricing)、[Rate limits](https://ai.google.dev/gemini-api/docs/rate-limits)。

## 当前实现合同

- 使用 Files API resumable upload 分块上传提取后的 WAV，避免把完整长音频一次读入内存。
- 调用 Interactions API 的 `gemini-3.5-transcribe`，使用 `verbatim` 模式和 word timestamps；日语、英语、韩语分别发送 `ja-JP`、`en-US`、`ko-KR`。
- 返回的 `word_info` annotation 转成 `TimedUnit(kind=word)`，保留可选 speaker。缺失真实 word timestamps 时拒绝结果，不按字符比例伪造时间。
- provider 原始 JSON、word timeline、候选 cues、质量信号与 run 配置快照保留在任务 run artifact 中。
- 请求结束后主动调用 Files API 删除上传文件。删除失败会记录本地错误，但不能把远端删除描述成已经成功。
- 当前未启用 custom vocabulary。Gemini 官方不允许把 custom vocabulary 与 word timestamps 或 diarization 组合；后续需要通过对照实验决定专名提示和词级时间谁优先。
- 当前 UI 只开放 selected-range 候选，便于明确上传范围并与 Whisper 对照；完整任务云端上传在隐私和费用实测前不开放。

自动合同测试已经覆盖 annotation、speaker 和时间解析。真实网络验收需要用户自己的 Key，至少检查一次日语范围、一次英文范围、Files 删除、rate-limit 错误展示，以及返回内容是否与当前 API schema 一致。
