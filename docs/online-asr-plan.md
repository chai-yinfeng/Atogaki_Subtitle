# Online ASR 当前计划与模型清单

_更新：2026-11-06。[0073](decisions/0073-local-index-translation-and-paired-context.md)新增Index本地翻译选择、完整源译历史对及词表／场景提示；沿用0072分段与等待。日语MT跨短停顿重译及保守句末＋最终后文提交见[0072](decisions/0072-japanese-semantic-block-retranslation.md)，3s／8s为首版兜底，质量未验收；其他语言仍沿用[0070](decisions/0070-online-independent-prefix-commit.md)默认1.2s／4s。双框显示软窗口与语义块分开，ASR不变。会话底座见[0067](decisions/0067-online-session-owner-and-system-audio.md)；云端与轻量MT仅调查，vLLM继续搁置。_

本文是 online 分支的当前入口，决策见 [0063](decisions/0063-online-two-pass-model-selection.md)。2026-10-02 至 10-05 的 WS／SS／CPP 结果保留为历史证据，不再按旧矩阵继续选型。本文的“保留／新增／退出”是实验范围，不代表桌面模型目录或磁盘文件已修改。

## 分支现状与范围

2026-11-02已在main基线上准备 `codex/online-desktop-integration` 合入研究历史；产品接入方向见[0065](decisions/0065-online-desktop-integration.md)。下文分支计数是此前整理时点，不是当前整合分支状态。

整理前分支为 `codex/online-asr-experiments`，HEAD 为 `bd19b55`，工作区干净且与对应远端引用一致。相对本地 `origin/main`，分支独有16个提交、main独有234个提交，共同基点为 `fbbbcd95`。当时未合并main且未改写历史；分支中的旧桌面进度不能当作当前 main 状态。0045／0046 与 main 上同编号的不同 ADR 重号，引用必须带文件名；本次使用0063避免再占用主线既有编号。

已有能力：三套 Whisper 回放、独立媒体时钟、VAD／尾部动作实验、事件与资源日志、串行 MT 重放。最新旧报告记录39项 Python 回归及真实模型回放；本次没有重跑。Qwen MLX整段识别及串行Hy-MT2链路已跑通，见[记录](qwen-mlx-smoke-validation.md)；SenseVoiceSmall也已完成sherpa-onnx整段ASR→MT对照，见[记录](sensevoice-smoke-validation.md)；本地two-pass独立CLI已接通日语SenseVoice草稿和英语Zipformer草稿→Qwen1.7B→Hy-MT2，见[记录](two-pass-smoke-validation.md)；正式采集／App／云端尚未接入，vLLM继续搁置。

CLI／隔离产物继续独立于 offline 任务、SQLite 与 GUI；完整录音可另交全文 offline 精修，online 的段级定稿不等于人工核准资料。新方案仍须记录模型／runtime revision、配置、输入摘要与失败，但不沿用旧三 backend 全矩阵作为前置门禁。

## 模型清单

| 模型 | 本轮用途与范围 | 当前状态 |
| --- | --- | --- |
| Whisper small | 轻量识别对照，可评估第一遍草稿 | 旧受控实验已跑；不自动视为新 two-pass 已接入 |
| Whisper medium | 质量／速度中档对照 | 桌面目录已有；新 online 配置未验证 |
| Whisper large-v3-turbo Q8_0／large-v3 Q5_0 | 用户确认保留这两个large系列档位，与small／medium并存 | App目录移除其他large档位尚未实施；Qwen满意后可进一步削减 |
| Whisper tiny／base、未量化 large-v3 | 退出当前主动选型清单 | 仅保留旧实验、冒烟用途和已有文件，不删除 |
| SenseVoiceSmall | 第二遍整段识别候选；支持中／粤／英／日／韩 | sherpa-onnx FP32／CPU单线程已跑通18s／60s ASR→MT，质量供用户对照 |
| Qwen3-ASR 0.6B／1.7B | 第二遍整段识别主要候选 | MLX整段ASR→Hy-MT2已跑通，见[记录](qwen-mlx-smoke-validation.md)；vLLM按用户要求暂时搁置 |
| streaming Zipformer | 本地原生流式草稿 | 中／英／韩公开样本均已按时钟跑通，英语已接two-pass；不支持日语组合 |
| Silero VAD 6.2.3 | 因果起停声检测，触发第二遍音频段封口 | online lock 已固定6.2.3；本次不升级 |
| Hy-MT2 1.8B Q4_K_M | 唯一本地翻译候选 | 沿用已有 runtime 与权重；用户所称1.7B在此按正式名称1.8B记录 |
| Hy-MT2 7B | 退出本轮本地选型 | 已跑历史实验；不继续优化或要求完成7B盲评，不卸载已有文件 |
| 云端 ASR／MT | 本地质量或 E2E 不合适时的对照／替代 | 已梳理Gemini／Qwen3-ASR／Groq价格；本轮无新调用，不继承过去的单次上传授权 |

Zipformer 是模型架构，sherpa-onnx 是推理框架；语言范围依赖具体 checkpoint，不是框架天然限制。SenseVoice 支持日语是已公布能力，日语较弱是待验证假设。Qwen 0.6B与1.7B已在同输入上跑通，用户目前更偏好1.7B质量；SenseVoice新增对照不自动改变该偏好，也不冒充gold评测。

### Whisper large-v3 可选格式（2026-11-02核对，尚未选定）

turbo与量化是独立维度：large-v3约1.55B、32层decoder；turbo约809M、4层decoder，是剪枝后微调模型，不是large-v3的量化别名。未量化常用F16，F32可用于数值对照，不作为本轮常用部署档；F16仍不是无限精度。

以下为whisper.cpp常用候选，体积是十进制下载大小，不是推理峰值内存：

| 模型／格式 | 约体积 | 获取与比较意义 |
| --- | --- | --- |
| large-v3 F16 | 3.10GB | 上游现成文件；保留完整decoder，作为质量／量化对照 |
| large-v3 Q8_0 | 约1.66GB，实际以转换文件为准 | 可从原模型转换；本次核对的上游文件目录未列现成v3 Q8_0，不能误用v2 Q8_0 |
| large-v3 Q5_0 | 1.08GB | 上游现成文件，项目目录已有；压缩权重但仍保留32层decoder |
| large-v3-turbo F16 | 1.62GB | 上游现成文件；区分turbo结构变化与量化影响 |
| large-v3-turbo Q8_0 | 874MB | 上游现成文件，项目目录已有；值得评估的速度／体积折中 |
| large-v3-turbo Q5_0 | 574MB | 上游现成文件，项目目录已有；更省权重存储，质量／速度仍须实测 |

whisper.cpp当前上游转换工具还列出Q4_0／Q4_1、Q5_1及Q2_K至Q6_K等格式，但不代表项目固定runtime均已验证，也不代表都有现成权重。本轮不扩成全量化矩阵。Q8／Q5是分块量化格式，不是所有张量都严格占8／5bit，也不能仅凭文件更小推断必然更快或质量无损。

MLX可用F16／F32及其自身分组量化格式（上游提供4bit转换）；faster-whisper／CTranslate2可用FP16、INT8及INT8_FLOAT16等计算类型，实际取决于设备。GGML Q8_0、MLX 8bit与CTranslate2 INT8不视为同一产物或相同数值实现，文件不能跨runtime直接互换。turbo不保证每段日语都弱于完整v3，完整v3也不保证必然胜出；需固定输入和解码设置比较。

后续用户确认large系列仅保留turbo Q8_0与非turbo Q5_0，其他格式退出App目录；small／medium继续保留，已有文件不自动删除。上述完整格式表用于解释选择，不再要求新增F16对照。实际App目录改动待单独实施。

### SenseVoice公开版本

SenseVoice论文包含Small和Large；Large的研究指标不等于权重已经公开。当前官方仓库公开的是SenseVoiceSmall；维护者2026-07-15明确表示Large checkpoint尚未发布且无可靠时间表，本次未找到后续官方Large发布。Small的ONNX、INT8、GGUF等是同一基础模型的部署／量化转换，不是另一种容量型号。本轮保持Small，不为等待Large阻塞探索，也不把Fun-ASR等不同家族当成Large别名。

### Silero 是否升级

截至2026-11-02，公开 PyPI 最新包仍为6.2.3，与 `experiments/online/envs/simulstreaming/uv.lock` 一致；pyproject 是 `>=6.2.3`，复现仍使用 `uv sync --locked`。旧实验 JIT 模型 SHA-256 为 `e1122837f4154c511485fe0b9c64455f7b929c96fbb8d79fbdb336383ebd3720`。新 ONNX／sherpa 路径需另固定资产版本、摘要和状态处理，不能只复用包版本号。

桌面目录的 `ggml-silero-v6.2.0.bin` 是 whisper.cpp 使用的另一种格式／资产，本次不替换。没有证据表明旧长静音恢复缺陷由 Silero 模型版本导致；换 VAD 不能代替修正音频队列与调度。后续只有明确缺陷或新版本收益才安排有界配对验证。

## 第一阶段 two-pass 行为

后继pipeline采用[0066](decisions/0066-online-retranslation-groups.md)：译文按跨VAD活动组整体重译，ASR定稿不直接提交译文；CLI已接入草稿、收口及全组最终revision检查。正式采集与App尚未接入。

用户已确认第一遍必须本地，允许滚动窗口形成可回改草稿；定稿可本地／云端。当前CLI实现、模型分工、云端价格与局限以[0064](decisions/0064-local-drafts-and-segment-finalization.md)及[链路记录](two-pass-smoke-validation.md)为准。

持续音频同时进入完整录音保存、第一遍草稿识别及因果 VAD。第一遍负责实时上屏；VAD 提供起声／停声信号，第二遍对已封口的完整音频段重新识别，替换对应活动段草稿。第二遍以原始音频为依据，初期不把第一遍文字作为强提示。

这里的“整段”是一次 VAD／长度策略封口的音频段，不是等待整个会话结束。第一阶段不做第二遍流式；快慢模型同时流式的方案留待整段延迟实测后再决定。日语当前先用SenseVoiceSmall CPU滚动草稿，Whisper及Qwen MLX流式接口仍是后续对照候选，不把Zipformer作为跨语言必需组件。

VAD 边界、ASR 定稿、翻译语义组是不同事件。停顿可以触发第二遍，但不自动保证句意完成，也不要求第一遍每次短停顿清空状态。第二遍定稿后再按句意／上下文和等待上限送翻译；草稿提前翻译及回改成本后置。

长独白需最大段长和有限重叠；pre-roll、尾部静音等待、长段切点、跨段上下文和去重策略尚待实测确定，不在文档中假称已有默认值。完整录音与有限 ASR 工作上下文分离，分段记录原媒体坐标；不能要求起声后先解码全部历史静音。旧长静音回放仍可作为回归素材，但不必修完所有旧 adapter 才接新模型。

段级定稿及译文绑定 segment ID／source revision；新定稿取代草稿，迟到译文不能写到新的原文版本。该合同已在独立CLI事件中实现并测试，不代表已迁入正式任务／字幕schema或GUI。

## Runtime 与验证顺序

后续用户已要求接链路：已完成独立CLI日语／英语回放及三语言Zipformer；第一遍本地草稿，第二遍Qwen1.7B定稿后再译文。云端先梳理价格／准确率证据，不发起新音频上传；当前只有Gemini凭据由用户确认提供。当前分段仍较碎、没有语义合段优化。

2026-11-02用户收窄当前工作：先跑通Qwen MLX并比较音频到原文／译文耗时，不建设优化pipeline或展开质量矩阵。已完成两档模型的18s／60s同输入试跑及本地MT，结果见[跑通记录](qwen-mlx-smoke-validation.md)。下面保留后续方向，不把全部条目作为本次前置要求。

1. 固定 SenseVoiceSmall、Qwen 0.6B／1.7B 的模型 revision、格式、runtime pin和最小音频输入／结果合同。复用已有音频工具与隔离日志，先做同一批自然结束的日语音频段识别，不展开全参数矩阵。
2. **当前搁置vLLM：** 用户在2026-11-02明确选择本机缺少适用GPU时先搁置，不继续自动准备或运行。以下仅保留恢复后的原方向：在具备受支持设备的测试环境实际运行vLLM，记录操作系统、GPU／显存、版本、启动参数，以及具体可用的整段音频请求 schema、语言／上下文选项、响应、错误和取消／超时行为。不假定通用 OpenAI 接口直接支持本模型；不把 MLX 服务包装成 vLLM 支持，也不要求非 MLX 同学改用 Mac。无对应环境时明确标“接口调查完成／运行未验证”。SGLang 不作为本轮额外必选路线。
3. 比较第二遍漏词、专名、否定和句尾，分别记录冷启动、热识别耗时、段长与内存。时间戳若需要精确到词，另验证 forced aligner；VAD 起止时间不冒充词级对齐，也不默认将 aligner 放进实时关键路径。
4. 给有希望的候选接第一遍及媒体时钟回放；先验证草稿可读性、段末定稿、长静音恢复、长独白和 EOF，不必把历史80组矩阵全部重跑。中／英／韩 Zipformer 与日语草稿路线分开标注。
5. 接 Hy-MT2 1.8B，并与选定云端 MT 做有界对照。测真实 ASR／MT 并发时的资源争用和队列增长；若整段定稿等待不可接受，再讨论第二遍流式或其他提交策略。

用户体验分别记录正确草稿显示、第二遍定稿和可理解译文。句末延迟拆为停顿确认、排队、第二遍推理、翻译分组等待、MT排队与生成；另报告距句首的等待。直接测E2E，不相加不同轮次中位数。原有效draft p50≤1s／p95≤2s仅留作历史目标参考，新定稿和译文预算待实测与使用体验确认；不因重设方向而宣称旧目标已经通过。

云端可以减少本地算力负担，但上传／网络／服务排队仍算入E2E；选择前确认具体服务、语言、费用和数据边界，真实上传／付费调用按具体实验授权。

## 旧工作如何处理

- 保留四份验证报告、原始音频／参考／失败、pins／lock和历史 commits，结果不覆盖、不重新解释为新模型结论。
- 0045的独立 online／offline 边界继续有效；其中WS／SS优先顺序已被0063取代。0046的旧矩阵、small固定候选、旧选型前置门禁停止作为当前推进合同。
- WS／SS／CPP保留为旧实现／必要对照，不继续全面调参或逐一生产化。旧20个锚点、large-v3完整审核、SS Unicode／时钟修复不再是开启新选型的强制前置项；有助于具体比较的样本再复用。
- 早期整理阶段只改文档；后续已新增独立Qwen MLX环境和最小试跑脚本，没有删除旧模型／venv／日志，也未改桌面目录。清理已有大文件另做明确盘点。
- 后续只提取实际选中的实现。是否同步 main 的共享工具／目录结构按具体缺口处理，不为模型实验整体合并234个主线提交。

## 新方案确定后的整体清理（待执行）

用户明确要求旧决策、测试结果和环境暂时保留；新探索确定后必须完成整体清理，作为选型收尾的一部分，不无限累积旧实现。

先完成所选模型／runtime的实际质量、延迟和部署验证，保存足够复现结论的配置、版本、摘要与精简结果。随后统一盘点文档、旧runner与adapter、依赖／lock／venv、源码clone／build、模型格式、缓存与重复产物；移除不再使用的代码和可重建环境，归档仍有解释价值的结果，修正过期入口、重号ADR与失效命令。Git保留历史，不需要将所有旧实验继续留在活跃目录。

清理按文件用途和引用关系处理；私人原始音频、人工参考、不可重建证据及正式offline共享模型／任务数据不因退出候选而批量删除。归档范围与可删除项届时明确，执行后记录清单、回收空间和仍保留的理由。最后从保留的pin／lock检查所选路径能重建、运行和通过相关回归，确认无旧进程／失效入口／无用依赖遗留。本次仅记录该要求，没有执行清理。

## 上游核对来源（2026-11-02）

- [Qwen3-ASR官方模型与推理接口](https://github.com/QwenLM/Qwen3-ASR)：0.6B／1.7B，Transformers和vLLM后端；官方流式路径当前依赖vLLM。第二遍整段推理不要求流式接口。
- [社区MLX实现](https://github.com/moona3k/mlx-qwen3-asr)：Apple Silicon候选，不等于官方实现或本机已验证。
- [SenseVoiceSmall](https://github.com/QwenAudio/SenseVoice)及[sherpa导出支持](https://k2-fsa.github.io/sherpa/onnx/sense-voice/export.html)。
- [sherpa streaming transducer目录](https://k2-fsa.github.io/sherpa/onnx/pretrained_models/online-transducer/index.html)：已列中／英／韩对应模型，未见日语条目；不是声称全世界不存在日语Zipformer权重。
- [Silero PyPI](https://pypi.org/project/silero-vad/)与[上游](https://github.com/snakers4/silero-vad)。
- [Hy-MT2 1.8B官方模型](https://huggingface.co/tencent/Hy-MT2-1.8B)。
- [Whisper官方规格](https://github.com/openai/whisper)、[turbo模型卡](https://huggingface.co/openai/whisper-large-v3-turbo)及[GGML文件目录](https://huggingface.co/ggerganov/whisper.cpp/tree/main)。
- [whisper.cpp量化类型](https://github.com/ggml-org/whisper.cpp/blob/master/examples/common-ggml.cpp)、[MLX转换](https://github.com/ml-explore/mlx-examples/tree/main/whisper)及[faster-whisper计算类型](https://github.com/SYSTRAN/faster-whisper)。
- [SenseVoiceLarge发布状态：维护者答复](https://github.com/modelscope/FunASR/discussions/1910)及[FunAudioLLM论文](https://arxiv.org/abs/2407.04051)。
