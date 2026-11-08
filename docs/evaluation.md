# 识别与翻译质量基线

_最后更新：2026-11-08_

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

`evaluate-translation` 使用与桌面端相同的 `TranslationPlanner`、stable cue ID 和 provider 合同运行冻结的 source cues。命令只在所有 semantic groups 均通过结构校验后写出结果，后组失败不会留下可误认为完整运行的部分译文。以下示例使用本机 loopback OpenAI-compatible endpoint；API key 通过参数或 `ATOGAKI_TRANSLATION_EVAL_API_KEY` 提供：

```sh
cargo run -- evaluate-translation \
  --input local-artifacts/evaluation/source/segments.json \
  --output local-artifacts/evaluation/runs/provider-a.json \
  --base-url http://127.0.0.1:18080/v1 \
  --api-key "$ATOGAKI_TRANSLATION_EVAL_API_KEY" \
  --model model-name \
  --provider-name "Provider A" \
  --protected-term example-name
```

本地 Hy-MT2 评估固定使用官方采样参数、每组 2,048 output token 上限和严格 JSON schema。输出保存 grouping strategy、provider/model、token usage、总耗时和完整 cues；它是本地运行产物，不提交 Git。

多个 provider 使用相同 source timeline 完成后，用独立 answer key 生成匿名材料：

```sh
scripts/prepare-translation-blind-review.py \
  --candidate previous-output=local-artifacts/evaluation/runs/previous.json \
  --candidate local-small=local-artifacts/evaluation/runs/local-small.json \
  --candidate local-large=local-artifacts/evaluation/runs/local-large.json \
  --output local-artifacts/evaluation/reviews/review.json \
  --answer-key local-artifacts/evaluation/reviews/answer-key.json
```

脚本会先核对 cue ID、时间和原文完全一致，再按 cue 以固定 seed 打乱候选。review 文件预留遗漏／增译、专名、指代、跨 cue 连贯、自然度和修改量字段；完成评分前不要打开 answer key。

manifest 中保存媒体 SHA-256、范围、角色、参考状态、历史任务目录、模型和参数快照。路径只出现在本机 manifest；提交前使用示例文件检查字段，不复制真实值。

## Online 日语参考的准备

不要求先准备完整标准中文翻译。ASR 的 CER 需要少量听审后的日语原文，建议从现有开发样本选累计 5–10 分钟，覆盖否定、句末谓语、专名、长句和停顿；holdout 维持不参与调参。无法听清与重叠部分显式标记，不猜写。

粉丝字幕可作为 `external_candidate_reference` 辅助核对，记录媒体版本、来源与是否逐字；删减、释义、润色和字幕合并不直接作为 CER gold。只有回听确认后的原文片段提升为 verified。中文译文用于辅助审查，允许多个合理表达，不把字符匹配率当作翻译准确率。

翻译先固定已校对日语原文，分别检查信息完整性、否定／程度、专名、指代和自然度，再评估端到端结果，区分 ASR 错误传播。领域专名可单独分组报告；粉丝向样本本身符合当前个人场景，但不能据此宣称泛化能力。实时中间译文按当时已到达的上下文评审，不用尚未说出的信息惩罚模型；最终结果按完整上下文评审。

Gemini 3.5 Transcribe 可作为独立 `model_reference` 辅助基线，保留相同音频范围、模型／参数与未经人工复核的状态。与本地 ASR 的分歧是听审优先级，不是自动纠错依据；只有听审后的片段成为 verified。2026-10-02 用户选择先完成本地实验，Gemini 参考稍后补充；本轮不上传音频或调用 Gemini。

2026-10-04 经具体范围授权完成一次 60 秒 Gemini Transcribe，并删除云端临时音频；选出其中 18.9 秒问答作 evaluation-only 对照。用户核对发现精修画面的日文与实际听感不完全一致，模型也存在交叠文本顺序／词时间倒退，故均不自动提升为 gold。该 holdout 片段已经评估曝光，后续调参转到 development 样本；完整记录见 [候选问答验证](online-paragraph-reference-validation.md)。

## 日语电台数据集扩充（2026-11-08）

用户指定後書きVol.1–4、Vol.33–37与2024-01-18长电台为主要来源。公开链接及浏览器观察登记在[来源清单](../experiments/online/datasets/japanese-radio/sources.json)；真实媒体、参考和本机路径统一放在忽略目录`local-artifacts/datasets/japanese-radio/`，不迁移或覆盖旧证据。当前只完成来源核对与旧资产索引，尚未取得新视频媒体或提取字幕，不代表已经建立可评分的新基准。

核对三个入口页面：Vol.1–4显示时长分别9:37／8:31／9:53／9:07；Vol.33–37为23:56／20:50／27:23／31:01／35:27；长电台约1:52:40。Vol.1画面为一页多句的日中双语稿，Vol.33画面含中文，长电台画面含中文且字幕菜单只观察到“中文AI”选项。不能将AI轨视为人工校对参考，也不能把一页稿件的显示时间套给每句发言。Vol.34–37的独立URL仍待逐项确认；不能将合集入口误作每集音频。公开元数据API三次均返回HTTP 412；浏览器可以正常播放，但未取得完整字幕轨清单，因此不能断言没有独立人工字幕。

旧基准并非完全缺失：旧evaluation manifest含5个开发／保留／英语／长回归／故障回归条目，媒体仍存在，原文reference路径状态为pending，其对应目录当前为空。早期受控ASR另有4个60秒development窗口、4份已成功生成并删除远端文件的Gemini词时间参考及20个待核验锚点，见[旧验证记录](online-controlled-asr-validation.md)。近期two-pass／日语重译主要重复使用《湖吉の庭Vol.1》15–75秒，不能代表内容覆盖充分。本次复制该60秒及四个窗口到新目录，连同Gemini参考、原review状态和旧manifest快照建立私有索引；共5个回放输入但独立音频只有240秒，不能相加成300秒或分到不同训练／验收角色。旧holdout及已曝光状态保持，不重新宣称盲测。

参考准备优先提取人工字幕；烧录字幕使用OCR后抽查，保留画面显示区间与声学发言区间两条时间轴。缺少原文时，Gemini Transcribe／Whisper large-v3可生成带时间的日语候选；Qwen3-ASR需单独启用并验证时间对齐，当前online默认输出没有词时间。VAD仅辅助找语音范围，不能替代句词对齐。每条源文赋稳定ID，DeepSeek或其他离线MT按ID输出译文并继承源文范围，不生成新的声学时间。OCR、ASR、MT候选分别保留来源和核验状态，已有离线字幕亦不自动成为gold。

2026-11-08复核[Gemini官方转录文档](https://ai.google.dev/gemini-api/docs/transcribe?hl=en)及[模型限制](https://ai.google.dev/gemini-api/docs/models/gemini-3.5-transcribe?authuser=57)：当前接入的`gemini-3.5-transcribe`支持verbatim词时间，但文档未提供直接中文翻译及译文时间轴合同；不能把通用Gemini音频理解与此转录接口混为一谈。词时间模式单请求最长30分钟，启用时间可能降低转录质量，且不兼容custom vocabulary。新参考宜先按选中片段生成，完整长节目不直接整集提交。本轮未读凭据、未上传媒体、未调用付费API。

下一步先完成新来源媒体获取与小范围字幕提取，选择含上下文的连续片段，再冻结开发／验收划分；同一集及其重叠片段保持同一角色，并核对是否与旧holdout同源。每例保留音频摘要、原媒体offset、原文／译文、词表／场景、独立文字与时间核验状态。完整离线上下文可用于制作参考，但回放只能访问当时已到达音频；不能把未来内容写入在线场景提示。

在线延迟以固定的源文锚点评估首次有效译文、最后实质改写、锁定及积压，分别保留后端返回与前端实际显示时间；未输出锚点仍计入覆盖率。分组末段延迟不能替代首段等待，provider估计时间与听审声学时间分别标识。首先补齐当前链路baseline，尚未实现新的自动评分器，也未因建立目录宣称延迟回归已闭环。
