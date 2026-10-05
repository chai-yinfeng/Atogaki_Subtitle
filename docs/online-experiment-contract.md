# Online 实验控制、指标与目标行为

日期：2026-10-04。本文整理已实现的 ASR 实验与后续实验设计，不选择生产 backend，不启动新云端调用或 MT 回放。指标名称以当前代码为准；标为“待实现”的定义还没有测量结果。既有 offline 工作流和未来 online GUI 继续独立。

## 1. 比较的对象

模型权重、推理 runtime、流式提交算法、输入／解码调度、显示策略、翻译调度是不同变量。可以修改代码，不等于任意 checkpoint 都与 runtime 兼容；可以流式返回文本，也不等于流式接收音频或持续更新源文本。

| 路线 | 本轮状态 | 可以控制什么 | 边界 |
| --- | --- | --- | --- |
| Whisper-Streaming（WS） | 已跑：MLX large-v3 fp16／Metal | Whisper 模型、兼容 runtime、decode options、prompt、buffer trimming、LocalAgreement、调度 | 流式算法代码开放，使用预训练 Whisper，不需训练。当前 pinned 实现是相邻两次 hypothesis 的共同前缀确认；更改确认规则需改代码 |
| SimulStreaming（SS） | 已跑：large-v3 fp32／torch CPU | 兼容 Whisper 权重、greedy／beam、AlignAtt threshold、音频 buffer、上下文／prompt、调度、内部 attention 解码代码 | 上游已有修改过的 Whisper 实现，我们直接使用该实现；不是调用远端服务，也不是专用 SS checkpoint。当前 CIF checkpoint 为 None；若采用可选 CIF 模块需另查其权重来源与作用 |
| whisper.cpp stream policy adapter | 已跑：large-v3 Q5_0／Metal | 兼容 GGML 模型、decode options、窗口／overlap／rollover、显示替换 | 当前窗口直接重识别并替换，周期冻结没有稳定保证；文件 adapter 不等于原封不动的 SDL whisper-stream |
| Gemini Transcribe | 已做候选音频文件参考 | 请求模型／prompt／文件范围 | 过去上传完整片段的结果不是 streaming 时钟 baseline，未经听审不是 gold |
| Qwen-Audio-3.0-ASR | 候选，未接入／未调用 | 云端 API 暴露的配置、语言／上下文等；以实际 endpoint 文档为准 | `qwen-audio-3.0-asr-flash-streaming` 才是要评估的 streaming 服务；技术报告公开不代表服务权重／代码开放 |
| 开源 Qwen3-ASR | 单列可探索线索，未下载／未测 | 开放的模型与推理实现 | 官方有 0.6B／1.7B 权重及 streaming 工具，不能将其视为 Qwen-Audio-3.0 云端模型的同权重实现 |
| Hy-MT2 | 既有本地 MT 实验 | 模型、runtime、生成配置、输入上下文及调度 | 本轮 ASR／VAD 矩阵没有运行 |
| Qwen-MT | 候选，未接入／未调用 | API 模型、语言、terms／domain／translation memory 等专用参数 | 优先研究 flash 的输出 streaming；单轮翻译，不支持 system message。输出 delta 不等于输入可持续追加，也不自动处理 ASR 修订 |

本地 checkpoint 不限于当前 large-v3，但架构／格式／tokenizer／对齐支持必须与 runtime 匹配。修改 attention 算法、确认阈值、显示 draft 不需要重新训练；改变模型本身或新增依赖训练的模块则是另一项实验。SS 需要内部 attention／token 状态；不能把不暴露这些信息的任意 Whisper runtime 当作可直接替换的 adapter。保持上游原始 pin，代码改动记录 patch／revision 和 effective config，不悄悄修改已记录的 baseline。

Qwen-Audio-3.0 技术报告描述增量 provisional transcript 和句末完整音频再识别后刷新，chunk 与 right context 可在其实现中调节；这解释了一种“已显示字幕回改”机制。论文可调不意味着云端 API 对用户暴露同样参数，接入前需逐项确认。不能用它推断 Google 内部机制，也不能直接搬用论文宣称的延迟数值。[技术报告](https://arxiv.org/html/2609.07549v2)、[官方项目页](https://qwenaudio.github.io/qwen-audio-3.0-asr/)、[ASR 模型目录](https://www.alibabacloud.com/help/zh/model-studio/asr-model/)。开源 Qwen3-ASR 的可获取模型／工具见 [官方仓库](https://github.com/QwenLM/Qwen3-ASR)。

Qwen-MT 的 flash／lite 提供增量输出，plus／turbo 的流式响应为累计文本；增量协议需要 adapter 正确拼接，累计协议需要替换。术语及示例控制使用 translation_options，不能照搬 Hy-MT2 chat prompt。快慢与质量尚未在本项目测量。[官方 MT 文档](https://www.alibabacloud.com/help/en/model-studio/machine-translation)。

## 2. 当前真正控制到的参数

| 层 | 当前值／行为 | runner 已暴露 | 尚未暴露或缺口 |
| --- | --- | --- | --- |
| 公共输入 | 同一 60 秒 development WAV，mono 16 kHz PCM16，原媒体坐标、无丢包 | audio、model、language、step、wall limit | sample role 目前靠 prepare_case 校验，replay 不独立验证；需 case/config 绑定 |
| 推理触发 | 默认 step=1s；落后时合并已到达 PCM，一次最多 30s；VAD 尾部另触发一次 | step | 输入供给周期与推理触发间隔耦合，尾部触发无法独立关闭 |
| WS | LocalAgreement-2、segment trimming=15s，显示 committed＋draft | checkout、model、language | agreement 规则、trimming、prompt 和 decode 配置在代码／上游固定，尚无独立实验 flags |
| SS | frame_threshold=25、audio_max_len=30s、audio_min_len=0、segment_length=step、beams=1、greedy；CIF=None；prompt=None | checkout、model、language、step | threshold／beam／context 等在代码固定；模型设备本轮 CPU；没有观测内部 draft |
| CPP | length=5s、keep=.2s；greedy、4 threads、Metal／flash-attn、single segment、无上下文／timestamps；rollover 按调用次数 | length、keep、step、worker、model、language | decode options 在 worker 固定；coalescing 与调用次数 rollover 交互需复核 |
| VAD | Silero 6.2.3、CPU1线程、512 samples=32ms；threshold=.5、negative=.35、pre-roll=.2s | off／gate／endpoint、silence-ms、probability plan | threshold／negative／pre-roll 未做 flags；预计算因果概率，计算不计入 ASR replay clock |
| 输出 | WS 有草稿，SS 只有 commit，CPP 可改当前窗口 | 事件日志 | 不具备共同词级 reference 和统一历史回改窗口 |

本轮变量矩阵：三 backend × off／gate／endpoint500，共 9 组；CPP 另测 endpoint250／800、off-step500，共 12 组。不是 WS agreement、SS AlignAtt threshold、模型大小／量化或硬件的完整参数搜索。模型／设备／精度同时不同，只能比较系统配置，不能归因“CPP 算法比 AlignAtt 更快”。专名／作品名提示另做无提示与同等信息提示的配对测试，固定词表内容及预算；不能给一个 backend 额外答案后将效果归为基础 ASR 能力。

记录过 command、audio/model/worker/lock/upstream digest、package 和时间信息；effective config 中的硬编码参数尚未全量独立序列化，后续应保存 resolved config 和驱动脚本 revision/hash。只保存 command 不足以复现修改过脚本后的运行。

## 3. 用什么时间衡量

以本机单调时钟为准，媒体起点映射到 replay 起点。人工复核的词／短语声学末尾为 `t_audio_end`；provider timestamps 只作附加信号，不作为不同 backend 共用 gold。分辨不清的词不用虚构时间。模型加载／warmup、云端建连／握手分开记录；用户实际首次启动耗时另列。

| 指标 | 定义 | 状态／用途 |
| --- | --- | --- |
| 首次非空显示／首次 commit | 相对会话起点的时间 | 已有；音乐标记和 hallucination 也可能触发，不能代表有效首字 |
| 输入处理落后 | 每次 inference 返回时间 − 本次供给 PCM 末尾 | 已有；本地处理进度诊断，有返回不一定有字幕；不作为主要体验排名 |
| decode cost／EOF flush／endpoint 完成 | 调用计算耗时、EOF 服务耗时、VAD speech end 到段结束 | 已有；flush 仅真正调用模型时计入 decode，endpoint 不等于算法稳定 |
| 有效 draft 延迟 | 某个 reference 单元首次正确显示时间 − t_audio_end | 待实现；需要文本与人工音频 reference 对齐，不能拿整段首次非空代替 |
| 最终稳定显示延迟 | 某个正确 reference 单元最后一次被修订的显示时间 − t_audio_end | 待实现；回看事件流验证此后保持不变，是事后指标，运行时不知道未来 |
| provider commit／final 延迟 | provider 的 commit／final 到达时间 − 对应 reference t_audio_end | 只有部分上游粗估计；需 reference 对齐。算法 commit、云端 final、窗口 freeze 分列 |
| 质量与覆盖 | final CER、有效单元覆盖、漏词／重复／错词、语义错误 | 待人工 reference；错词／未输出不从分母消失，报告 miss 和 coverage，防止只对成功词计算延迟 |
| 显示稳定性 | 撤回字符／次数、每分钟修订、修订跨度 | 已有部分；按同一归一化规则，新窗口不能虚报为撤回；SS 无 draft 不能推断零内部修改 |
| 持续实时能力 | 积压随时间的变化、处理进度／丢包／失败、资源占用 | 现有无丢包回放仅 60s；待长音频验证；低首次延迟不能掩盖持续积压 |

共同主指标可以统一为用户侧“正确文本何时显示、何时不再改、多少内容成功识别”，本地和云端都记录 receipt/display 的本机时间。云端内部 decode／队列不可观测时标 unavailable，不能用 `返回时间 − 最新已发送音频末尾` 冒充它实际处理到了哪里；网络往返留在用户侧延迟内，地域／连接／packet 记录为控制条件。对 draft 与 commit-only 路线的指标缺失保持 null，不伪造同语义事件。

MT 后续另测 request queue、TTFT（从实际发送请求到首 token）、完整响应、首个可理解译文／最终可用译文、过期响应率与重译成本。完整音频到可用译文的 E2E 延迟直接测，不相加不同单元／不同轮次的中位数。ASR 可以继续、MT 可以并发，两者也可能争用资源；串行 replay 不是 E2E 性能结果。Qwen-MT 输出 streaming 的首 token 也不必是完整可理解译文。

## 4. 拆开 VAD 与调度变量

VAD classifier 只输出因果活动概率／起声和停声事件。实验应分别配置：输入供给（完整 PCM／静音填零）、推理是否暂停及唤醒条件、decode 频率／batching、尾部是否强制 decode、是否 finish、是否 reset／保留上下文。未来 MT 分组可以使用停顿信号，但不能要求 ASR 为每次翻译分组 reset。

当前 gate 的精确定义：起声后供给音频、停止期间暂停；恢复时填零保留跳过的时间；停声时额外处理尾部，ASR 状态保留。当前 endpoint 在它上面加 finish 与下一段 reset。所以此前 off → gate 同时改了输入、唤醒和额外尾部推理；gate → endpoint 同时改了 finish 和上下文。这是策略实验，不是纯 classifier 的 A/B。

下一轮建议逐项做配对对照，固定模型／设备／decode／显示算法／reference 与因果概率文件：

1. 对同一 backend 先拆开 feed clock 与 decode trigger，关闭停声额外触发；以当前 off 作为控制，验证新的 gate 是否减少误触发且不制造积压。静音填零与保留原 PCM 明确分列。
2. 保持 gating、decode 节奏与上下文不变，只加入尾部强制 decode，量化这一次调用的代价。
3. 在同一尾部策略上加入 finish；区分非 decode 的 buffer flush 与真正模型调用。
4. 再单独加入 reset，测跨段上下文与 warm state 损失；250／500／800 ms 仅在固定这一策略后比较。
5. 确定调度控制后，分别调 WS agreement／trimming、SS frame_threshold／beam/context、CPP window／rollover。不同 runtime 不强求同一物理参数，但共享输入和用户侧评分。

云端另设 full-audio stream 的服务原生 endpoint 基线；先不要叠加客户端 gate 切掉声音。服务内部 VAD／endpoint 不能关闭或参数不开放时如实记录，不与本地外置 VAD 标记为相同实现。论文里的 chunk/right-context 参数，需确认云端实际可控后才加入矩阵。

## 5. 要得到的行为与实施顺序

Online 目标是音频持续到达时尽早展示有用原文，并为后续翻译留出预算；低 ASR 延迟与质量／覆盖／撤回共同评价。说话期间可以产生可修订的原文，停顿不应默认清空所有上下文；段末更准确的识别允许回改活动段，但保留版本和 provider 的真实承诺。窗口冻结不被标作“确定正确”，模型 final 也不被标作人工 verified。若修改 WS／SS 已 committed 的前缀，应作为新增 revision 策略独立测试，不伪装成原算法自带功能。

初步建议以有效 draft p50 约 1s、p95 约 2s 作为开发目标，配合明确的覆盖率／语义错误和积压验收；这是尚未确认或实现的预算提案，不是已测性能，也不是强迫每个 commit 在 1s 内产生。稳定原文和译文的具体预算待同样本测量后确定。本轮不以产品旧文档中的“可接受十几秒”作为纯 ASR 延迟目标。

推进顺序：先固定上述合同并修正本地调度／resolved config；人工复核少量 development 音频，建立有效显示与稳定显示测量；对三套本地 baseline 配对复测并验证长音频积压；再在相同媒体时钟下接 Qwen-Audio streaming。Gemini 文件转录继续作为参考，若测 Gemini online，必须另选并验证其真实音频流协议。最后独立比较 Hy-MT2 与 Qwen-MT（先相同人工原文与语义组，再接相同 ASR 事件流），避免把 ASR 错误、分组和翻译模型差异混在一起。首轮 Qwen-MT flash 候选不改变 Hy-MT2 现有 baseline 地位。

云端接入前确认具体 model ID、地区、费用／限额、实际暴露参数和获授权音频范围；之前对 Google 的一次 60 秒上传授权不自动适用于新云端实验。此次仅浏览公开文档，无上传、无付费模型调用、无新模型安装。方案选定后收敛为一套 online 实现，研究矩阵留作 evidence，不晋升成桌面多 backend 框架。
