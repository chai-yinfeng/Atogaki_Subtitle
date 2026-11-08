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

### 首轮素材筛选（2026-11-08）

按用户要求先挑5–10段、每段约3分钟，并优先提高离线参考质量。已通过本地视频画面与浏览器烧录字幕抽查选出[8段候选](../experiments/online/datasets/japanese-radio/screening.json)，核心范围总计24分钟。话题是抽查点的观察，不代表完整听审或每段均覆盖全部目标语言现象；精确句界在转录后复核，不用模型补标点冒充自然终点。

| ID | 来源 | 初选范围 | 初筛内容 |
| --- | --- | --- | --- |
| radio-01 | Vol.1 | 00:10–03:10 | 专辑感想、创作意图、口头语与接话 |
| radio-02 | Vol.1 | 04:00–07:00 | 曲名、发音与录制安排 |
| radio-03 | Vol.2 | 00:45–03:45 | 生活经历、引语与人物互动 |
| radio-04 | Vol.2 | 04:30–07:30 | 下载、安装与使用经历 |
| radio-05 | Vol.33 | 03:30–06:30 | 视觉共享、知识与假设性讨论 |
| radio-06 | Vol.33 | 10:00–13:00 | 身体动作与刷屏日常话题 |
| radio-07 | 2024-01-18长电台 | 29:00–32:00 | 感官描述、听众问答转场 |
| radio-08 | 2024-01-18长电台 | 59:00–62:00 | 歌曲写作、《晴る》来信 |

本机另找到Vol.1的576.341秒双语视频，与网页577秒显示不同，尚未验证两版本逐段时间偏移。已按本机媒体坐标准备前两段180秒mono／16kHz／PCM16 WAV，并分别提取前后最多30秒的参考制作上下文，核对音频格式、精确时长和摘要。私有`selection.json`保存实际路径／hash／context起止／核心offset，`selection.md`为人工入口，`selection/radio-01`与`radio-02`保存音频和case。网页其余六段尚未取得媒体：正常播放可用，页面资产工具不支持此DASH类型，video导出失败。不能把8个候选称为8段已冻结的回放数据。没有绕过访问限制、改App或运行新模型。

Vol.1与旧保留集属于同节目，按已曝光development管理；Vol.33拟作development，Vol.2与长电台拟保留evaluation，按集隔离且先核对旧来源身份。它们已经过视觉筛查，不称未曝光盲测。参考制作允许利用后文，但在线回放必须因果供给；30秒前滚策略应固定并记录，仅核心180秒评分，后滚只供离线参考，不提前输入评分窗口。

本轮仅调查参考模型，不下载或替换：Qwen官方[公开系列](https://github.com/QwenLM/Qwen3-ASR)为0.6B／1.7B，没有同系列7B权重。制作参考优先现有Qwen1.7B FP16与完整Whisper large-v3的独立转录及人工字幕交叉核对，纠正专名／否定／角色分歧后再对齐；参数更多或模型一致不自动等于正确。[VibeVoice-ASR](https://huggingface.co/microsoft/VibeVoice-ASR)提供长音频、说话人与时间输出，另有[社区MLX 4bit](https://huggingface.co/mlx-community/VibeVoice-ASR-4bit)及[MLX runtime示例](https://github.com/Blaizzy/mlx-audio/blob/main/docs/getting-started/quickstart-cli.md)，可作较大模型复核候选；未验证本机运行、日语质量或峰值内存，不先把它指定为参考真值。

用户“在线追平离线80%或部分能力”作为产品方向保留，尚未定义为通过阈值；后续以经核验语义锚点的信息覆盖、关键错误和延迟共同判断，不以参考译文的逐字相似度代替理解质量。本轮JSON／范围／音频摘要校验及fmt／diff检查通过；没有完成新ASR、翻译、OCR或延迟基线。

### 本地下载后的候选调整（2026-11-08）

用户将Vol.1–4、33–35下载到既有後書き的翻译目录，36／37继续补充，长电台暂缓。本轮以已存在且可读取的7期重选8段，均已裁出核心PCM音频及独立参考上下文，全部180秒／合计24分钟，不再依赖长电台。版本2[清单](../experiments/online/datasets/japanese-radio/screening.json)沿用旧ID；radio-02／04转备选，radio-07／08延期，旧清单副本与裁片保留。新范围如下：

| ID | 来源 | 本机媒体范围 | 初筛内容 |
| --- | --- | --- | --- |
| radio-01 | Vol.1 | 00:10–03:10 | 专辑感想、创作意图与接话 |
| radio-03 | Vol.2 | 00:45–03:45 | 生活经历、引语与人物互动 |
| radio-05 | Vol.33 | 03:30–06:30 | 视觉共享、知识与假设性讨论 |
| radio-06 | Vol.33 | 10:00–13:00 | 身体动作与刷屏日常话题 |
| radio-09 | Vol.3 | 06:00–09:00 | 方言误听与生活态度 |
| radio-10 | Vol.4 | 02:00–05:00 | 虚构世界、角色设定与假设 |
| radio-11 | Vol.34 | 03:00–06:00 | 人名、饭局回忆与转述 |
| radio-12 | Vol.35 | 20:00–23:00 | 饮食习惯与文化差异 |

本机原文件只读，先校验音视频轨、时长并冻结源媒体摘要，记录大小／mtime确保索引时未变化；抽样画面保留明确seek时间。裁片校验mono／16kHz／PCM16格式、核心精确180秒、上下文范围与摘要。网页秒数、旧原录音时间不自动映射到这些下载版本，实际坐标以本机源hash为准。话题仍是稀疏画面字幕初筛，目标语言现象未逐项核验，完整听审与自然句界尚未完成。所有版本的媒体、画面、私人路径及模型输出仅在忽略目录中保存。

用户确认参考制作流程后，顺序运行既有Qwen3-ASR1.7B MLX FP16和完整Whisper large-v3 MLX FP16，均使用相同的8份参考上下文WAV，无ASR词表prompt、无翻译、无新下载、无云端调用。核心共1440秒，加参考前后文后模型各处理1900秒；这包含未来参考上下文，不是在线因果回放或E2E延迟基准。Qwen沿用`run_qwen_mlx.py --repeat 1`，8份均非空且未触发截断；Whisper固定日语／transcribe／temperature=0／condition_on_previous_text／word_timestamps，8份均正常返回。8组输入摘要对应，Whisper共6830个词时间条目通过有限数值、起止顺序与音频范围检查；该检查不证明声学精度、文字准确或覆盖完整。

私有`reference-local-20261108/`保存执行脚本／命令、实际模型与lock摘要、选择快照、两套输出、退出状态及`validation.json`；`review.md`提供核心音频、原视频位置、完整上下文转录、核心附近Whisper段时间和最多3处归一化文字分歧，差异仅供核对、不作为CER或质量排名。既有Yorushika词表另存快照供后续专名复核，本轮ASR未读取。所有输出均为`model_reference_unreviewed`，没有自动升为gold；OCR人工字幕提取、语义／专名校对、声学校准、中文参考和当前在线延迟回归仍待完成。原有App、在线配置与模型均未修改。

### 烧录字幕候选与重复复核（2026-11-08）

用户授权继续OCR和局部复核；Vol.36／37现已确认存在，当前仍冻结原8段、不等待长电台。新增独立[提取入口](../experiments/online/burned_reference.py)与[macOS Vision helper](../experiments/online/vision_ocr.swift)，使用系统原生能力而不安装OCR依赖，不接入App。日语／中文字幕分来源处理，保留原始文字、候选、置信度、框位置和颜色深浅。灰色历史／未来文字仅作context；Vol.35较深旧行按当前最深色带区分，左右双侧可见文字不自动解释为当前说话人。文字不作模糊合并，真实分开发言不全局去重；片头／片尾与失败帧附近保留未知显示边界，不补造发声时间。

正式批次检查2826张图，实际调用792次OCR、静态近似帧复用其余结果，得到956条连续显示文字候选，0帧识别失败。该数量含日中双语行、分行和OCR波动，不是956句或源译对；参考仍为`ocr_candidate_unreviewed`。帧时间使用实际PTS，约0.5秒采样，不能作为词级声学真值；所有帧均在各核心范围内、图像／事件一一对应。原型曾因沙箱CVPixelBuffer限制失败；裁区坐标需转回原图后才能测色。抽帧也发现输出时限和视频起始／分数PTS会导致尾帧日志与落盘计数不同，已由过滤器单独裁定起止、移除冗余输出时限；失败轮次保留且不混入正式批次。

Whisper候选不仅有相邻重复，也有同一segment内部回绕。radio-06／09保持相同音频、模型、temperature与词时间设置，只关闭`condition_on_previous_text`复核，最长连续相同段从23／15降为1／1；radio-10追加同配置复核后，两个诊断短语的输出次数从3／2降为1／1。原结果保留，不靠直接删重补造参考；三份新结果均为未核验修复候选，重复减少不证明语义正确，也不能外推为通用策略或实时性能提升。

私有`ocr-20261108/verified/`保存2826张采样图、实际PTS、全部OCR与956条候选，`provenance.json`保存源码／helper摘要、系统／ffmpeg版本与失败轮次。`whisper-repair.json`与原模型记录分开。人工核对首轮仅选2处、合计33.5秒音频，材料在`ocr-20261108/human-review.md`／JSON：H01核对一处整段遗漏的关键原文，H02核对有语义分歧的问句；附音频、截图、模型节选和pending状态，不要求用户手工标时间。另有少量已视觉核对的同音词／方言互动证据，但不将它们提升为已听审或声学gold；未列入首轮的内容也不自动视为正确。

12项回归覆盖当前／灰色文字、分开发言保留、双侧同文、否定差异、开放／失败边界、罗马字专名，以及真实ffmpeg的起始／尾部PTS计数；8帧原生试点与2826帧正式批次验证通过。模型输出、图像和私人路径全部保持本地，云端调用0；原文语义核准、声学对齐、完整源译对应和在线延迟baseline仍待后继完成，没有修改在线策略或App。

首轮人工反馈已收到：H01确认Qwen头尾正确、中间确实遗漏，缺失范围采用用户认可的Whisper文本，只核准该处而非整段节目。Whisper估计的完整缺失范围约353.20–363.38秒，越过当前核心360秒终点；另保留核心内已认可前缀，不能把窗外内容算入核心覆盖率。人工只核对文字，所有声学时间仍为provider估计。H02用户也无法从suis笑声中辨清原句，保留uncertain；粗略中文大意可供语义阅读，但不选定原文、不当作精确译文gold。保守将整个H02听审窗口1284.00–1300.50秒标为逐字／精确MT／精确延迟评分排除，该范围不是笑声声学边界。原反馈、采用范围与排除策略写入私有`ocr-20261108/adjudications.json`／MD，尚未运行使用该记录的新评分。其他参考仍未自动核准；后续继续声学对齐和源译对应。
