# 识别与翻译质量基线

_最后更新：2026-09-16_

评估使用用户有权处理的本机媒体。媒体、裁剪、人工参考和运行结果放在 Git 忽略的 `local-artifacts/evaluation/`，仓库不保存私人绝对路径或媒体内容。公开的格式示例见 [`evaluation-manifest.example.json`](evaluation-manifest.example.json) 和 [`evaluation-reference.example.json`](evaluation-reference.example.json)。

## 固定角色

- 《湖吉の庭 Vol.1》全片：日语开发集与历史翻译对照。
- 《後書きラジオ》`00:00–05:00`：日语人工参考保留集。
- `Daily English Podcast` `00:00–02:00`：英语 WER 与 word timeline 冒烟集。
- 京吹 YouTube LIVE 全片：长任务、短残片与恢复回归。
- 京吹广播特别篇全片，以及 `49:11.870–49:56.870`：Whisper 重复反馈专项回归。该范围由现有任务中连续的 `ま` 循环定位，仍需听审确认精确故障边界。

人工参考中的 `verified` cue 参与评分；`uncertain` 和 `overlap` 保留原时间与说明，但不进入 CER/WER。不能确认的内容不得为了形成完整答案而猜写。

## 识别评分

运行：

```sh
cargo run -- evaluate-asr \
  --reference local-artifacts/evaluation/references/sample.json \
  --hypothesis local-artifacts/evaluation/runs/sample/segments.json \
  --output local-artifacts/evaluation/runs/sample/asr-report.json
```

日语、韩语和中文使用字符错误率 CER；输入先执行 Unicode NFKC，移除空白和本文件实现中固定列出的布局标点。英语转为 NFKC 小写、移除非单词标点，以空白切分后计算 WER。报告保存归一化文本、编辑距离和参考单位数，便于发现规范变化。

普通字幕时间不能充当 word timestamp 真值。时间质量只有在人工参考提供对应粒度的边界后才计分。性能结果另记媒体时长、wall time、实时倍率、峰值内存、失败阶段和检测到的循环范围。

## 翻译评分

翻译比较必须使用同一份已校对 source cues，避免把 ASR 差异算作翻译差异。逐组记录遗漏、专名／占位符、指代、跨 cue 连贯、自然度和人工修改量。旧字幕和旧烧录输出标记为 `previous_output`；未经人工复核不提升为 gold。

manifest 中保存媒体 SHA-256、范围、角色、参考状态、历史任务目录、模型和参数快照。路径只出现在本机 manifest；提交前使用示例文件检查字段，不复制真实值。
