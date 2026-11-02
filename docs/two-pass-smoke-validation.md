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
