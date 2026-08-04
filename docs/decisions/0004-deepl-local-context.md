# 0004：用 SQLite 局部上下文增强逐段翻译

日期：2026-08-04
状态：已采用

## 背景

字幕必须保持稳定的段 ID 和时间轴，所以不能把整篇转写合并翻译后再猜测如何切回原段。但完全孤立地翻译每段短句，容易丢失人物、指代和相邻话题。DeepL Translate API 支持为一组独立 `text` 提供共享的 `context`，上下文本身不会作为译文返回。

## 决策

- 每个字幕段继续作为独立 `text` 发送，DeepL 返回结果与稳定段 ID 一一对应。
- 全部重译每 12 段一批；每批从 SQLite 当前日文字幕读取批次前后 30 秒的局部上下文。
- 单段重译同样从 SQLite 读取目标段前后 30 秒，而不是只发送这一句。
- 上下文按时间顺序拼接，最多 2000 个字符；超长时保留以当前批次为中心的窗口。
- 所有批次成功后，才使用已有的原文校验事务一次性更新 SQLite。

## 影响

- 人工修正后的日文会立即进入下一次翻译的上下文。
- 翻译质量可以利用邻近话题，同时不改变分段、时间轴和导出映射。
- 请求中途失败不会留下部分更新；翻译期间原文发生变化时，整次写入仍会被拒绝。
- 当前窗口和批量大小是应用层常量。积累真实节目样本后，可以再决定是否暴露为高级设置。

DeepL 参数语义参考其官方 [context 使用说明](https://developers.deepl.com/docs/learning-how-tos/examples-and-guides/how-to-use-context-parameter) 与 [Translate API 请求文档](https://developers.deepl.com/api-reference/translate/request-translation)。
