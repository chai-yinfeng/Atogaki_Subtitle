# Qwen 电台候选：同输入 ASR 筛选与听审材料

日期：2026-11-13。阶段：四档候选的计算／资源筛选及首批 10 段可用性反馈已完成，建议以 1.7B 8bit 作为后续工作基线；不是总体准确率验收，未更改 App 默认值。

## 冻结输入与比较条件

用户确认主要场景为ヨルシカ《後書き》日语节目到中文理解，允许数秒到十几秒等待、正确性优先。本轮只比较日语 ASR，不测试中文口语／英语，也不调用翻译或云端。后续两档连续在线比较依赖本轮听审，不能仅凭速度选择准确率候选。

- radio-01：Vol.1，原节目 10–190 秒，专辑感想与创作意图。
- radio-11：Vol.34，180–360 秒，人名、饭局回忆与转述。
- 两段共 360 秒，均为已接触的开发材料，不是未见保留集。radio-06 留作下一阶段已知故障回归，不混入本轮结果。
- 使用当前 worker 相同的 PCM16／mono／16 kHz、512-sample 输入、Silero ONNX、threshold .5、min-silence .5 秒、min-speech .25 秒、max-speech 12 秒及 .2 秒 pre-roll 规则，预先导出 63 个固定确认识别请求：radio-01 为 31 个、radio-11 为 32 个。
- 63 个请求合计 308.892 秒，互不重叠；其余 51.108 秒未被 VAD 送交本轮 ASR，不能归为某个 Qwen 的漏识别，也不证明其中没有轻声语音。
- 每个候选收到同一份 WAV／样本边界，输入 SHA-256 固定。各段前后约 5 秒的听审音频另存，原有参考的状态与估计时间保留；这些文字和额外上下文不进入 ASR。

四个候选的固定 revision、大小和逐文件 SHA-256 延续 [模型目录](../src-tauri/src/model-catalog.json)。已有两档原始 BF16 包只读复用；0.6B 4bit 和 1.7B 8bit 下载至忽略的本轮实验目录，共约 3.18 GB，逐文件校验通过。没有搬动模型、修改正式设置或更新本机 App。

硬件为 Apple M4、24 GiB RAM、macOS 27.0.1 arm64；Python 3.11.16、MLX／mlx-metal 0.32.4、mlx-qwen3-asr 0.4.4、sherpa-onnx 2.0.1。运行时 Python 模块摘要另存本地 manifest；worker 基线为 `58ec64f`，实验 runner 的实际内容摘要冻结，不能将未提交脚本冒充基线提交已有代码。

本轮 Qwen runner SHA-256 为 `d293fbc04abd54bf84b0561162fb2e597e2cf487a9bc0ff33f224aae88b0a1f4`，运行前后核对一致；manifest SHA-256 为 `5aed02ce814471dbafbdcccc9cd1fc6b7bcd398788161aab2a5fe52872a2de36`。

候选按顺序 0.6B 原始、0.6B 4bit、1.7B 8bit、1.7B 原始运行，第二轮反转顺序。每档每轮使用独立新进程，先完成相同首段的一次预热并另存，然后运行 63 个请求。模型串行、Metal 可用性显式检查；未发现其他已运行的 Atogaki／llama-server／Ollama 推理进程，但不是隔离实验室或物理冷缓存测试。

调用与在线确认 worker 相同的 `Session.transcribe((samples, 16000), language="Japanese")`，无词表／文本 context／draft／时间戳／投机解码。请求读取 WAV 和模型校验在计时外，计时包括特征、解码和 Metal 同步。加载／import 与预热分列，原始 BF16 权重在当前实现以 FP16 浮点格式运行；量化权重保留整数表示。

## 实际结果

下表总耗时是每轮全部 63 次请求的计算耗时总和，包括失败请求的已消耗时间。它不是节目结束后的等待，也不是连续字幕延迟。

| 候选 | 第一轮计算合计 s | 第二轮计算合计 s | 每轮完整／预期请求 | MLX 分配器峰值 GB |
| --- | ---: | ---: | ---: | ---: |
| 0.6B 原始 | 20.991 | 21.419 | 63 / 63 | 3.14 |
| 0.6B 4bit | 11.302 | 11.380 | 62 / 63 | 1.13 |
| 1.7B 8bit | 34.630 | 33.438 | 63 / 63 | 3.23 |
| 1.7B 原始 | 53.882 | 55.089 | 63 / 63 | 8.16 |

MLX 峰值包含该进程加载、预热及推理，单位为十进制 GB；不是 App 总内存、物理内存占用或纯模型大小。RSS 单独记录，未与 MLX 数字相加；没有同时驻留 MT。原始 1.7B 的首轮加载／import 为 2.044 秒、第二轮 1.104 秒，表明文件缓存等因素仍有影响，不能把 fresh process 称为物理冷启动。

完整请求的单次 compute p50／p95：

| 候选 | 第一轮 p50 / p95 s | 第二轮 p50 / p95 s |
| --- | ---: | ---: |
| 0.6B 原始 | .288 / .760 | .288 / .775 |
| 0.6B 4bit | .148 / .365 | .150 / .368 |
| 1.7B 8bit | .423 / 1.264 | .427 / 1.190 |
| 1.7B 原始 | .663 / 2.054 | .679 / 2.094 |

4bit 的分位数只来自 62 个完整请求，不能与另三档的全请求分位数直接作为稳定性排名。总计 504 次计量调用，502 次完整，另有 8 次不计入统计的预热；0.6B 4bit 在 `radio-01-029`（10.294 秒输入）两轮均以 `finish_reason=length` 截断。保留两次原始输出，没有放宽 token 预算、删掉该段、做去重或重试后替换结果。当前在线 worker 遇到 `truncated` 会触发错误，因此它目前不能只凭计算更快就直接作为在线默认。

同一候选两轮的文字输出均完全相同，属于本次可重复性观察，不是准确性证明。以 NFKC、去空白／标点后的文字作差异定位：

- 0.6B 原始与 0.6B 4bit：22 / 63 段相同。
- 1.7B 原始与 1.7B 8bit：56 / 63 段相同，余下全部 7 段进入首批复核。
- 四档共同相同：12 / 63 段；仍保留一致输出的听审抽查。

这些是字符串一致数，不能解释成准确率或量化损失大小。两轮不足以提供总体显著性或置信区间。本轮没有 verified 日语逐字参考或声学末尾，因此 CER、关键语义胜负和首次正确可读延迟保持未判定。

## Whisper Medium 辅助参考

四档 Qwen 全部完成后，使用本机 App 自带 whisper-cli 和已安装 Medium，对完全相同的 63 个输入运行一次，63 份 JSON 均非空、进程成功退出。模型和二进制 SHA-256、完整参数、原始日志保留本地；日志确认 MTL0／Apple M4 Metal backend。

它使用桌面相应的 suppress-nst、beam 5／best-of 5、temperature 0／fallback increment .2、max-context 0，未给词表。它与 Qwen 解码默认不等价，因此只在听审页作为明确标名的辅助参考，不进入四档 Qwen 的速度表、不加入 A–D 匿名映射，也不自动作为人工 gold。

## 听审交付与后继

所有私人音频、模型、原文和结果保存在忽略的 `local-artifacts/online/qwen-radio-screen-20261113/`：

- `manifest.json`：固定源文件与输入摘要、模型目录、推理设置、VAD 规则、轮次顺序及实际 runner 摘要。
- `runs/round-{0,1}/{candidate}/`：加载／预热 metadata、逐请求原始结果、状态和资源观测。504 行的 ID 集合、重复项与输入 SHA-256 已交叉核对。
- `metrics.json`：计算／资源数值，`semantic_ranking=null`。
- `review.json`／`review.html`：全部 63 段，原冻结选择策略挑出 22 段优先听审，包含分歧、既有人工核准范围的重叠和一致输出控制。前后文音频、OCR 显示证据与估计声学时间分开，不生成假 gold。
- `review-with-medium.html`：补入明确标名的 Medium 参照，保留原始 Qwen-only 页面。
- `review-first.html`：结果出来后的诊断性首批 10 段，包含全部 7 段 1.7B 量化分歧、1 个截断段和每集 1 个一致输出控制。它不替代原 22 段选择，更不是未见验收或总体错误率抽样。页面另提供两个完整 3 分钟片段供长上下文回听。
- `answer-key.json`：单独保存逐段 A–D 映射。页面每段独立打乱，不显示模型名或各候选速度；Medium 另行标注。用户逐项选择可接受、关键错误、遗漏或不确定，可填写日语原文和备注并导出 JSON。核对结束前不打开 answer key。
- `audit.json`：504 次输入完整性核对、失败清单、全部量化分歧 ID、首批听审 ID 与材料摘要；没有自动语义判断。

听审时先区分“模型实际听到的输入”与“供人理解的前后文”。词语或句意在切段边界外才完整时，不将模型未收到的后文直接判为漏词；需要另记边界影响。否定、人物／转述关系、专名、遗漏和增添事实优先，允许多个合理写法，不确定就保留未判定。

原设计在听审后选两档继续比较；用户随后强调先选定一档相对可接受的工作基线。以下基线建议用于收敛后续实验，不是产品默认变更或正式质量验收。

## 首批人工可用性反馈与基线建议

已读取用户导出的首批 10 段、40 个 A–D 判断，逐段映射回同一批次的 answer key。导出原件、SHA-256、原始备注、模型映射和汇总另存忽略目录 `human-review-01/`；逐项 ID 集合与 A–D 标签均核对一致。用户填写的原文含问号、听感不确定与口语整理，原样保留为建议文本，不自动转换为 verified verbatim／CER gold。

| 候选 | 用户判为可接受 | 用户判为关键错误 |
| --- | ---: | ---: |
| 1.7B 8bit | 9 | 1 |
| 1.7B 原始 | 7 | 3 |
| 0.6B 原始 | 7 | 3 |
| 0.6B 4bit | 6 | 4 |

这 10 段是针对模型分歧、失败及 2 个一致控制挑出的诊断材料，不能解释成 90% 等总体准确率。可接受也不意味着逐字无误或中文翻译正确：用户容忍部分专名／同音字、插话遗漏与表记差异，同时要求保留关键意思；还有四档共同误识的例子。

结合本批反馈、计算／资源观测及 4bit 的重复截断，建议将 **Qwen3-ASR 1.7B 8bit** 收敛为下一阶段工作基线。0.6B 原始可留作资源／速度备选；1.7B 原始作诊断对照，当前没有必要继续与默认一起日常横评。这个建议不是量化普遍提高准确率的结论，也没有应用到 App 配置。

后续建议固定该 ASR，沿用现有草稿、VAD、MT、prompt、分组与预译设置，做一次约 10 分钟连续回放及 radio-06 故障回归。优先评估最终中文关键事实、同音误识的传播、正确后退化、重复／遗漏和队列积压；必要时只针对失败例，用用户建议原文作明确标记的诊断输入，区分识别错误与翻译上下文问题。不默认扩大模型／参数矩阵，不对不确定语音强行制造唯一标准答案。

角色、否定、数字等若能从音频／语境确认，翻反仍算关键错误；同音或吞字导致的合理歧义应单独记录。关于“只有端到端才能解决”，本轮没有相应对照证据，不作架构结论。后续先验证现有 ASR→MT 的实际错误传播，再决定是否需要独立的端到端对照。正式保留集继续使用未参与选型的另一集并记录曝光。

## 复现与验证

入口：[固定输入／Qwen 运行及匿名材料](../experiments/online/screen_qwen_candidates.py)、[Medium 辅助参考](../experiments/online/screen_whisper_reference.py)、[完整性核对与首批材料](../experiments/online/audit_qwen_screen.py)。它们复用既有 runtime、`PCMInput`／`AudioRing`、模型目录与 `Session`；现有翻译盲审脚本要求相同原文 cue，不适合直接比较不同 ASR 文字，因此单独提供这个有限的筛选入口。

先按模型目录的 revision／文件清单，用已有 `huggingface_hub.snapshot_download(..., token=False)` 准备两个量化目录，逐文件核对 SHA-256；原始模型只读复用。`STUDY`、`DATASET`、`ORIGINAL_MODELS`、`VAD_MODEL`、`RUNTIME_PACKAGE` 和 `RUNTIME_PYTHON` 分别指向新的忽略输出目录、本机数据集、已有模型及私有 runtime。不要复用已有 manifest 或输出目录覆盖证据。

```sh
"$RUNTIME_PYTHON" -s -B experiments/online/screen_qwen_candidates.py --study "$STUDY" prepare \
  --dataset "$DATASET" --original-models "$ORIGINAL_MODELS" --vad "$VAD_MODEL" \
  --runtime-package "$RUNTIME_PACKAGE"
for candidate in qwen-0.6b qwen-0.6b-4bit qwen-1.7b-8bit qwen-1.7b; do
  "$RUNTIME_PYTHON" -s -B experiments/online/screen_qwen_candidates.py --study "$STUDY" run --round 0 --candidate "$candidate"
done
for candidate in qwen-1.7b qwen-1.7b-8bit qwen-0.6b-4bit qwen-0.6b; do
  "$RUNTIME_PYTHON" -s -B experiments/online/screen_qwen_candidates.py --study "$STUDY" run --round 1 --candidate "$candidate"
done
"$RUNTIME_PYTHON" -s -B experiments/online/screen_qwen_candidates.py --study "$STUDY" report
"$RUNTIME_PYTHON" -s -B experiments/online/screen_whisper_reference.py --study "$STUDY" \
  --cli "$WHISPER_CLI" --model "$WHISPER_MODEL"
python3 -B experiments/online/audit_qwen_screen.py --study "$STUDY"
```

运行中保持原始模型、脚本与参考快照不变；`prepare` 后实际 runner 内容变化会拒绝继续。可用 `HF_HUB_OFFLINE=1`／`TRANSFORMERS_OFFLINE=1` 强制本地识别，本轮推理即采用这些设置。

验证包括实际 Metal 504 次计量请求及 8 次预热、63 次 Medium 辅助请求、3 项 CPU-only 完整性／匿名化／抽样回归、两套 `cargo fmt --check`、`git diff --check`。独立 WebKit 核对首批 10／完整 63／优先 22 段、WAV 元数据加载、匿名评分与虚构评分的 JSON 导出、1120／460 像素布局，页面没有 HTTP 请求；621 个相对 Markdown 引用无缺失。没有上传音频、修改 App 行为、覆盖旧语料或把浏览器导出测试中的虚构评分当成人工核验。
