# 本地two-pass与Zipformer流式跑通

日期：2026-11-02；范围为独立CLI实际时钟回放与云端候选调查，不做pipeline调参或桌面集成。用户允许日语第一遍使用滚动重识别草稿；第一遍必须本地，第二遍可本地／云端。实现决策见[0064](decisions/0064-local-drafts-and-segment-finalization.md)。

## 各模型放在哪一层

| 模型／runtime | 本地草稿上屏 | 第二遍整段定稿 | 当前证据 |
| --- | --- | --- | --- |
| streaming Zipformer／sherpa-onnx CPU | 原生增量流式，中／英／韩各自checkpoint | 不是本轮质量定稿主选 | 三语言公开样本流式已跑；英语已接全链路，无已验证日语模型 |
| SenseVoiceSmall／sherpa-onnx CPU | 可滚动重复识别；不是模型原生流式 | 可作低延迟本地候选 | 整段对照已跑；本轮日语草稿接全链路 |
| Whisper small／medium／turbo Q8／large-v3 Q5 | 需窗口／LocalAgreement等提交策略 | 可作整段候选 | 旧回放证据保留；本轮未新增Whisper适配 |
| Qwen3-ASR 0.6B／1.7B MLX | 上游有流式API／滚动窗口与文本回退；仍需本机实际草稿验证 | 1.7B为当前偏好 | 整段已跑；本轮1.7B实际处理VAD段并回填定稿 |
| 云端Gemini／Qwen／Groq | 不进入第一遍 | 显式选择后可接 | 仅官方资料／价格核对；本轮没有新API调用 |

SenseVoice是ASR，不是MT；非原生流式不等于不能实时上屏。sherpa官方也提供VAD／模拟流式示例。本轮实时更新的是日文／英文原文草稿；中文在第二遍完成后逐段更新，没有让Hy-MT2对每个草稿变化重译。原文草稿、模型定稿和人工核准状态分开。[SenseVoice示例](https://k2-fsa.github.io/sherpa/onnx/sense-voice/pretrained.html)、[Zipformer目录](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/online-transducer/zipformer-transducer-models.html)、[Qwen MLX接口](https://github.com/moona3k/mlx-qwen3-asr)。

## Zipformer实际流式结果

M4／24GB、sherpa-onnx2.0.1、CPU1线程，复用SenseVoice环境。使用官方发布包里的首个公开test_wav，每32ms按真实时钟供给，encoder／joiner优先INT8、decoder浮点，保留stream状态。EOF额外供给0.66s零音频供模型排空，明确计作flush动作，不伪装成真实语音。

| 语言／checkpoint | 样本长 s | 草稿更新次数 | 首次非空到达 s |
| --- | --- | --- | --- |
| 中文 zh-int8-2025-06-30 | 5.612 | 14 | 0.840 |
| 英语 en-2023-06-26 | 6.625 | 16 | 1.145 |
| 韩语 korean-2024-06-16 | 3.527 | 6 | 0.773 |

三项均产生持续更新和非空EOF文本。首次到达相对整个样本起点，不是声学末尾到正确词的延迟，不能作为准确率／覆盖率通过证明。下载源、archive摘要及实际使用文件摘要见[model-pins](../experiments/online/model-pins.json)；中文archive同时匹配GitHub公布digest。英语／韩语仍需真实用户节目验证，不外推为成熟产品支持。日语不会自动套用这些模型，CLI直接拒绝Zipformer＋ja。

复现：`uv run --locked --project experiments/online/envs/sensevoice python experiments/online/run_zipformer.py --model-dir /path/to/model --audio /path/to/sample.wav --output /new/output/dir`。原始事件／文本在忽略目录 `local-artifacts/online/zipformer-smoke-20261102/`，没有上传媒体。

## two-pass实际链路

`run_two_pass.py`使用独立音频时钟、CPU草稿worker和串行定稿／MT worker。输入完整WAV在本地保留，VAD和草稿只消费已经按时钟到达的前缀；第二遍对VAD封口段重新识别。第一遍待处理快照最多一个，落后时合并为最新快照；Zipformer从快照中继续供给未处理PCM，不重复解码旧帧。第二遍队列不丢段，当前仅记录队列深度，没有引入优化策略。

Silero使用既有6.2.3包的ONNX资产（摘要已记录），不增加torch到Qwen环境。ONNX由sherpa运行，使用公开setter配置threshold=0.5、min_silence=0.5s、min_speech=0.25s、max_speech=12s；negative threshold沿用runtime自动默认值，没有把之前JIT实验的显式0.35冒称为本次已设置。额外0.2s前滚不跨过上一段结束，范围保存原媒体坐标，不把它当成精确词级时间。

本地Qwen1.7B／MLX FP16与Hy-MT2 1.8B Q4_K_M沿用前次版本，Hy-MT2使用固定llama.cpp、Metal、ctx4096，ASR完成后再请求翻译。worker加载在回放起点之外。自启server只监听loopback，完成后已正常退出。

| 实际链路 | 输入 | 草稿事件 | 封口／定稿／译文段数 | 首次非空草稿 s | 封口到定稿中位 s | 封口到完整中文中位 s |
| --- | --- | --- | --- | --- | --- | --- |
| SenseVoice滚动→Qwen1.7B→Hy-MT2 | 日语60s | 64 | 20／20／20 | 0.537 | 0.448 | 0.660 |
| Zipformer流式→Qwen1.7B→Hy-MT2 | 英语6.625s | 17 | 1／1／1 | 1.052 | 1.331 | 1.670 |

日语从VAD段音频末尾算到定稿的中位为0.986s、最大2.441s；到完整中文中位1.197s、最大2.593s。前一表“封口后”剔除了VAD等停顿，后一口径包含等待；二者都不是以人工声学gold为基准的正确词延迟。日语全部回放及排空约60.065s，英语约8.328s，峰值待处理定稿队列均为1。不同内容／语言不可据此比较模型准确率，数据也不是P95统计实验。

实际JSONL验证：所有封口段都有定稿和译文；译文source_revision匹配其定稿，定稿后没有迟到草稿覆盖。两个时序回归测试另覆盖迟到返回和旧revision译文拒绝。没有把滚动草稿当成稳定提交。

**当前质量边界：** 60s输入被VAD切为20段，中位1.75s，选中音频合计42.734s。分段较碎，第二遍上下文比前次整文件少，个别短语可能定稿后反而更差；VAD未选范围不自动等于真实静音。原音保留供回听，本轮未优化合段、重叠对齐、语义翻译分组或轻声召回，也未证明完整语义覆盖。source_final表示模型结果，不表示人工gold。EOF和最大段封口也不保证句子完成。

## 运行与查看

```sh
uv sync --locked --project experiments/online/envs/qwen-mlx
uv run --locked --project experiments/online/envs/qwen-mlx \
  python experiments/online/run_two_pass.py \
  --audio /absolute/path/to/mono-pcm16-16khz.wav \
  --draft sensevoice --language ja \
  --draft-model local-artifacts/online/sensevoice-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17 \
  --final-model local-artifacts/online/qwen-mlx-models/1.7B \
  --vad-model local-artifacts/online/silero-v6.2.3/silero_vad.onnx \
  --translation-url http://127.0.0.1:PORT/v1 \
  --output local-artifacts/online/two-pass-new-run
```

服务由启动者管理，鉴权沿用 `ATOGAKI_ONLINE_LLAMA_KEY`。不提供translation-url则只输出原文草稿与定稿。`--draft zipformer --language en/zh/ko --draft-model /对应模型目录`选择本地原生流式；`--final sensevoice`是低开销替代代码路径，当前本轮实测全链路只用Qwen定稿，SenseVoice定稿证据仍限于前次整文件对照。默认只有本地模型选项，不存在隐式云端回退。

本地 `local-artifacts/online/two-pass-smoke-20261102/review.html` 可播放原音并按真实事件时间重放草稿／定稿／中文，或拖动时间轴查看各段。它是已记录推理的回放页，不会重新推理／上传。`review.md`保留文字版本，JSONL保留全部事件；原始模型文本和录音不提交Git。

## 云端ASR候选与价格（2026-11-02官方资料）

以下只针对第二遍ASR，不包含翻译费用，不把厂商整体WER或不同语料成绩视为我们的日语质量排名。

| 服务 | 官方标准价／折算 | 本项目判断与接入状态 |
| --- | --- | --- |
| Groq whisper-large-v3-turbo | $0.04／音频小时；每请求最低计费10s | 最低标价基线；继承Whisper路线，未必比本地Qwen更准确；未配置Key |
| Groq whisper-large-v3 | $0.111／音频小时；同样最低10s | 完整Whisper云端对照，未配置Key |
| Qwen3-ASR-Flash | 新加坡／美国 $0.000035／s，即$0.126／音频小时；北京$0.000032／s，即$0.1152／小时 | 可优先考虑的另一家定稿；支持本地文件／Base64等输入，尚无Key；不假定云端flash就是本地1.7B同权重 |
| Gemini 3.5 Transcribe | 免费层输入／输出免费但受限；付费输入$2／百万audio tokens、输出$12／百万text tokens，官方估计合计约$0.005／min，即约$0.30／小时 | 用户确认已有Gemini Key；本轮未读取／验证该Key，沿用现有Files＋Interactions实现可先做下一次实际比较 |

价格来源：[Groq语音文档](https://console.groq.com/docs/speech-to-text)、[阿里云价格表](https://www.alibabacloud.com/help/en/model-studio/model-pricing)、[Gemini价格](https://aistudio.google.com/docs/pricing#gemini-3.5-transcribe)。Gemini的每分钟值是假定25音频tokens/s及175文本tokens/min的估计，日语实际输出token量可能不同；免费层内容用于改进产品，付费层表中为不用于。Qwen和Gemini区域／项目额度分别确认，报价不等于当前账户可调用或实际账单。

Groq的短段最低计费不能忽略：当前60s demo若20段分别提交，全部都不足10s，会按200s计费，turbo约$0.00222，而非整分钟文件的$0.000667。这里只是按本地日志推算，没有发生云端调用。重叠／重试还会增加费用，所以不能仅用小时标价选择短段pipeline。

官方价格表另有Qwen-Audio-3.1-ASR-Flash（新加坡输入$0.15／百万tokens、输出$0.47／百万tokens），但其token账单不能未经换算就直接宣称比时长计费更便宜；与Qwen3-ASR-Flash是不同model ID／接口。当前先保留为额外线索，不同时接入全部候选。[Qwen录音API](https://help.aliyun.com/en/model-studio/qwen-asr-api-reference)、[Google文件转录](https://ai.google.dev/gemini-api/docs/transcribe)。

建议先用已有Gemini凭据做同范围的云端定稿对照，再决定是否申请Qwen或Groq Key；没有现成同输入API实测前，不宣称某家兼得最低价格与最高日语准确率。此次未读取凭据、上传新音频、调用付费API，也没有恢复vLLM。新增云端适配和实际范围上传在选择具体服务后继续。

## 验证与剩余工作

通过三语言Zipformer实际时钟流式回放、日语／英语两条完整链路、实际事件覆盖／revision核对、两个时序测试、py_compile、CLI help、回放页JS语法、cargo fmt --check、git diff --check和文档链接检查。初始回放HTML生成的换行转义错误已修正，JS语法检查重跑通过。旧模型、环境和日志保留；新选型稳定后统一清理。

这是CLI回放链路，尚未接麦克风／系统音频、App UI或云端定稿；暂停／恢复会话和长期积压策略也未验收。下一步先回看分段和草稿效果，再决定分段收口与选定云端对照；不以跑通记录冒充实时产品已完成。

## 独立翻译组与re-translation后继验证

2026-11-02，用户确认[0066](decisions/0066-online-retranslation-groups.md)后，在整合分支接入跨VAD翻译组、可整体回改的中文草稿和独立定稿。上文逐VAD段MT表格是旧版本证据。当前运行命令沿用上面的模型／输入参数，默认开启草稿翻译；增加`--no-translation-drafts`可仅翻译收口组，不提供translation-url仍只做ASR。译文事件改为group_id＋source_revisions映射，旧单segment消费端必须适配，不能将组译文重复贴给每个ASR段。

**版本／条件：** 基于`27a633c`实现本次变更；实际执行源码副本与SHA256、完整命令、JSONL、summary和validation保存在忽略目录`local-artifacts/online/retranslation-20261102/verified/`，不提交私有媒体／转写。M4／24GB、Qwen1.7B MLX FP16、SenseVoice／Zipformer CPU1线程、Hy-MT2 1.8B Q4_K_M及固定llama.cpp沿用此前资产。本次本地server单slot、ctx4096、Metal99层，源／译分别最多1000字符的前组上下文；MT提示新增官方background/source结构，因此不是严格同配置性能对照。模型加载在媒体时钟之前，服务只监听127.0.0.1，试跑完成已退出。

日语使用原基线`vol1-15-75.wav`，SHA256仍为`1b61285b9499d4d56fd22f2d6bb90dd251fdc5c0b5a7ae4f4b1cf2a4f62c3d69`；英语使用原Zipformer公开6.625s测试音频。第一轮误用了另一个60s片段，并发现初版上下文prompt把参考指令译入结果，故不用于下表；修正为Hy-MT2官方背景在前、待译文本在后的结构后，重新执行下列两条链路。原始首轮产物保留在上层目录，不覆盖或混入正式验证记录。

| 运行结果 | 日语60s SenseVoice→Qwen→Hy | 英语6.625s Zipformer→Qwen→Hy |
| --- | ---: | ---: |
| 原文草稿／最终ASR段 | 64／20 | 18／1 |
| 译文草稿／最终翻译组 | 51／17 | 7／1 |
| 首次原文草稿，距播放开始 s | 0.433 | 1.060 |
| 首次中文草稿，距播放开始 s | 0.584 | 1.179 |
| ASR段音频末尾→最终原文，中位／最大 s | 0.961／1.641 | 1.903／1.903 |
| 组音频末尾→最终中文，中位／最大 s | 1.034／1.326 | 2.243／2.243 |
| 中文草稿生成耗时，中位／最大 s | 0.139／0.347 | 0.201／0.339 |
| 回放及排空总时长 s | 60.063 | 8.866 |

首次草稿不代表首个正确词；组末尾延迟不表示组内开头内容的等待时间，也不能直接与旧逐段指标排名。日语3组包含多个ASR段，17组都由“最终标点＋等待”收口，英语由EOF收口。所有20／1个声学段均恰好归属一个最终翻译组，全部source_revisions匹配最终ASR，没有提交后的迟到草稿。6／3个中文草稿返回时原文已推进，事件携带实际输入和stale_source，未升级为最终结果。峰值最终ASR队列仍为1。

**质量和回改限制：** 对最终结果作本地文字抽查，修正后的模板未出现首轮那种参考指令翻译，但这不是全面质量验收。模型常给不完整短语补句号，如“この旅は。”“一人で。”，当前启发式仍会较早切组；17组只比原20段略有合并，尚未证明语义边界可靠。后续应优先检查这种过早收口，不能靠更短延迟宣称质量提高。

按Unicode字符、连续版本最长公共前缀计算的删除字符数／最终译文字符数，日语为1.730、英语为1.806；被回改字符的最长已显示时间约3.325s／5.952s。这是事件流的字符回改统计，不等同于论文按token和特定分词计算的NE，也没有真实字幕窗可读性验收。当前无mask-k，翻译草稿确实会明显回改；保留这些数据作为后续显示策略基线。

47项Python回归覆盖旧实验及新增组定稿、迟到结果、相同文本缓存复用、顺序上下文和上限／EOF收口；py_compile、CLI help、cargo fmt --check、git diff --check通过。真实运行覆盖日语滚动草稿和英语原生流式两条路径；没有重跑旧全矩阵，没有新云端调用。系统音频、受管runtime、桌面显示、长期积压和失败恢复仍未完成。


## 收口质量检查：待人工听辨

2026-11-02，用户授权先检查收口质量，疑点交人工校对，确认无问题再推进桌面会话。检查基于`6183455`的已记录日语60s事件及同音频SHA256，没有修改分组参数、ASR／MT实现或重新运行完整识别。审查全部17个翻译组的最终文本、提交时刻和相邻组媒体范围，并对四处可疑范围用相同最终ASR文本及相同前组上下文执行Hy-MT2合组对照；只改变目标分组，不更换／人工修正原文。模型、server启动参数沿用verified运行，服务退出码0，无云端请求。

**结论：当前不能宣称收口质量通过。** 有多处日语修饰／条件结构被拆成已冻结短语。模型补出的句号与0.65s等待不足以证明语义完成；相邻组范围含0.2s前滚，范围间隔不能冒充精确的静音长度。较长上下文的既有Qwen输出能揭示部分短段识别差异，但它同样是模型候选，不能用作人工gold。

合组对照仅是诊断：部分表达更连贯，但动作时态／语气错误仍存在，样本末尾甚至补入原文尚未表达的否定。不能将“合组后流畅”当作正确率提升，也不能用后续节目可能出现的内容为当前提前补译背书。固定WAV末尾截断与真实发言完成需要分开判断，EOF可以结束处理，但不表示句意完整。

当前工具不能直接听辨本地音频，本轮完成的是事件时序、文本和真实本地翻译对照；未冒称完成了人耳回听。已裁出四个上下文片段，总计44.1s，位置为样本9.8–18.4、17.5–26.7、33.6–46.3、46.4–60.0s（对应原媒体加15s），分别检查修饰结构、强调语与识别歧义、连续从句、样本末尾未完成句。

人工材料在忽略目录`local-artifacts/online/closure-review-20261102/`：`review.html`可直接播放四个片段、查看现有分组／合组译文并导出本地意见；`review.md`为文字入口，`review.json`含实际输入、输出和音频摘要，`boundaries.json`保留全部17组，`comparison-commands.json`和`cleanup.json`保留命令与退出状态。原音／转写／模型输出不提交Git。页面JS语法、片段参数／长度及摘要检查通过，尚未人工验证播放器操作。

下一步先收集这四处人工意见。候选调整是取消仅依赖句末标点的快速提交，并重新评估已有长停顿边界；这仍可能增加定稿等待，也无法单独解决ASR短段错误和MT提前补全。该行为修改尚未实施，不预设一套只针对当前样本的日语后缀规则。收口门禁未通过，因此本轮不启动桌面会话实现；用户确认的桌面方向与既有授权继续保留。


## 用户反馈后的上下文调整与PCM会话输入

用户确认合组翻译整体优于碎片翻译，并确认B处词语的ASR错误；这是人工反馈，不意味着全部短段ASR正确。按“先改善MT上下文、ASR暂不改”的方向取消句号＋0.65s快速收口，保留1.2s停顿、20s／400字符上限和EOF。源代码移至`src/infrastructure/online_worker/`供桌面与研究CLI共用，旧入口仅导入共享模块，不复制另一套算法。

相同日语60s、相同模型／VAD／本地MT参数，仍为20个最终ASR段，最终翻译组从17降为10，产生63次译文草稿。首次中文草稿0.674s，组音频末尾至最终中文中位1.245s、最大1.550s，总回放及排空60.037s；旧基线对应中位1.034s。原先拆开的三个短语已进入同一组。英语公开6.625s链路仍有1个最终组，组末尾至中文2.520s。输入、命令、实际执行源码副本及摘要在忽略目录`local-artifacts/online/long-pause-20261102/verified/`。这是合组行为与延迟验证，不声称准确率提升或所有边界正确。

共享worker增加`--pcm-stdin`，默认不指定`--output`，只通过stdout输出JSONL。加载完成后发session_ready，首批音频另发audio_started，stdin关闭后排空VAD／最终ASR／MT并发session_stopped。音频存于30s环形缓冲，不再依赖整份WAV；最终ASR队列上限8段。既有WAV CLI参数继续可用。48项Python回归通过，新增覆盖模型补句号不得触发快速提交；真实NumPy检查验证环形缓冲跨界和旧音频淘汰。

日语60s按32ms实际时钟从stdin供给，经过一次环形缓冲回绕，得到64次原文草稿、20个最终ASR、61次中文草稿和10个最终翻译组；所有段恰好归属一组且revision匹配，session_stopped标记completed，供给及排空60.088s，无新增音频文件。首轮在结束摘要处发现stdin模式仍假定存在WAV路径，修正后重新完整执行。后续将音频首帧时钟与模型加载／权限等待分开，并用英语6.625s真实管道验证（含不足32ms的末尾帧）：18次原文草稿、7次中文草稿、1个最终组、正常停止，总9.034s。最终版本的首帧时钟更新未重复日语60s，环形缓冲实现未再改变。命令、源码摘要、事件和正常子进程退出记录在`local-artifacts/online/session-20261102/verified-pipe/`及`verified-pipe-en/`。

质量限制仍保留：长停顿也可能发生在未完成句中，ASR内部标点不在本轮人工改写，MT仍可能改变语气或提前补全；中文草稿可回改不等于已解决显示闪烁。用户确认继续推进桌面会话底座，不将这次小样本称为完整产品质量验收。


## 日语跨段重译首版（2026-11-04）

实现见[0072](decisions/0072-japanese-semantic-block-retranslation.md)。复用本页原60秒development样本的Qwen最终原文、草稿和到达时间，未纠正已知识别分歧后伪装成同源比较。事件文件SHA-256记录在私有comparison.json；baseline来自263705e的生产retranslation.py，candidate为0072首版。M4／macOS arm64；沿用原pinned llama.cpp、Hy-MT2 1.8B Q4_K_M、Metal、temperature=0、max_tokens=1024，context=8192、单slot。两种策略共用已warmup服务，先baseline后candidate，各一次；不运行ASR、录音或云端，不足以声称E2E／统计性能改善。

| 固定ASR条件下的观测 | 0070 baseline | 日语首版 |
| --- | ---: | ---: |
| 最终块／草稿更新 | 17／65 | 6／67 |
| 本地MT请求／累计计算 | 82／12.93s | 73／14.60s |
| 最终结果距组末段结束，中位 | 2.48s | 1.80s |
| 最终结果距首段结束，中位／最大 | 2.76／5.06s | 9.37／9.79s |
| Qwen源段覆盖 | 20段，各一次 | 20段，各一次 |

组末段指标降低不能解释为更快：首段实际上等得更久。5/6个最终块仍由8秒兜底触发，仅1个命中句末＋定稿后文；本样本尚未证明启发式能很好定位自然语义边界。草稿更新次数没有降低，不能宣称此改动消除了闪烁。20秒／400字符是安全上限，不是保证最长可见内容或延迟。

阅读复核：更多片段进入同一个Source Text，但仍有授受／时态被译成请求、未发生经历被译成“不需要”、以及未完成末句被补足等错误。整60秒联合翻译也有相似错误，不是gold。对初轮候选6块进行历史源译／仅历史源文／无左上下文的小消融，移除历史译文可改善个别句子，却不能统一改善其余句子，故未据此改默认prompt。完整上下文不等于可靠翻译，既有已曝光development结果也不代表新盲测。

原始事件、命令、code SHA-256、逐段源／译、请求快照及人工可读report保存在忽略目录`local-artifacts/online/japanese-mt-20261104/`。正式策略数据只使用`comparison-verified/`。初轮`comparison/`把滚动草稿尾部静音带入范围、旧实现首段计时还取到早期草稿范围；修正重建、增加范围回归后重跑，失败口径与初轮消融仍保留，不混入表格。尚未量化用户阅读体验、正确译文首次出现时间或token级NE／TL。

后续先由用户检查逐段结果与桌面白蓝尾部体验；优先比较完整语义块下MT／prompt／领域参考，避免继续仅加等待。若模型质量可用而等待仍长，再考虑训练型分割或更好的边界候选。轻量离线后端仅完成[调查](local-translation-backends.md)，没有替换。
