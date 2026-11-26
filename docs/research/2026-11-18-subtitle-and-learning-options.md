# 在线同传、日语学习与离线校对调研

_最近核查：2026-11-26。以下选型均未冻结。采用或淘汰候选时，更新本页的状态、日期和证据，并同步路线图与相关决策。_

## 2026-11-26 模型和服务复核

### 本地模型

- **Hy‑MT3 1.8B 是同档替换候选。** 腾讯 11 月 24 日公开 1.8B、7B 和 30B-A3B。官方 Apache-2.0 的 1.8B Q4_K_M GGUF 为 1,348,432,160 字节，约 1.26 GiB。当前 MT2 同档约 1.06 GiB，文件约增 19%。核对 MT3 GGUF 头的 `hunyuan-dense` 架构及项目固定的 llama.cpp 源码，已确认该架构受支持。固定模型 revision `7f97e076a8622b09f3a6f14cff38a733441ace43` 并核验 SHA-256 `bae6e591103654ff43ef0de7d1492f6e6df1f955505f6bb2cc52e86fc399a942` 后，以 Atogaki v0.1.6 内置的 `llama-server` 运行 `cargo test --locked --offline --lib managed_runtime_translates_and_shuts_down -- --ignored`，一条合成日语通过了加载、字幕 ID、非空译文和进程退出检查。后续用同一份校正日文和确认前文对照 MT2，检查多段对应、漏译、否定、指代、专名和术语。在线另测与 ASR 并行时的等待、峰值内存及译文反复。默认选型仍为 MT2，发布会继续独立保存模型身份和快照。[腾讯发布](https://github.com/Tencent-Hunyuan/Hy-MT3) · [官方 GGUF](https://huggingface.co/tencent/Hy-MT3-1.8B-GGUF) · [现有 MT2 验证](../hy-mt2-validation.md)
- **Silero VAD 6.3 可以先复核实时路径。** 项目实时字幕使用 6.2.3 ONNX，文件 Whisper 使用 6.2.0 GGML。6.3 更新了 16 kHz／8 kHz 权重，发布说明报告其短停顿和长音频检测改善。在项目现有本机 Python／sherpa-onnx 运行时用当前 16 kHz 阈值与时长设置，两个 ONNX 版本均能处理 65 个 512 样本静音块，均未产生语音事件。真实节目应保持 ASR、阈值和会话设置一致，测背景音乐误触发、吞掉短语、句首切掉、分段密度与等待。Whisper 的 6.3 GGML 转换仍是上游待办。[Silero 发布说明](https://github.com/snakers4/silero-vad/releases/tag/v6.3) · [whisper.cpp 跟踪项](https://github.com/ggml-org/whisper.cpp/issues/4202)
- **Qwen3-ASR 权重继续作为基线。** 官方开源专用 ASR 仍列 0.6B、1.7B 和 0.6B ForcedAligner。Atogaki 实时可用 MLX 量化版和 1.7B 的 DFlash 加速，文件流程可用 Qwen 加强制对齐。Whisper 已核对到的官方权重仍是 large-v3／turbo 这一代，项目内的 whisper.cpp v1.8.6 可单独对照上游 v1.9.6，优先看 Apple Silicon 耗时、字幕时间及相同输入文字差异。[Qwen3-ASR](https://github.com/QwenLM/Qwen3-ASR) · [OpenAI Whisper](https://github.com/openai/whisper) · [whisper.cpp v1.9.6](https://github.com/ggml-org/whisper.cpp/releases/tag/v1.9.6)

### 云端入口与候选

- 文件／编辑区已有 DeepL、DeepSeek、Gemini 和 OpenAI-compatible 文本翻译，云端音频局部重识别已有 Gemini 3.5 Transcribe。持续收听系统声音的「在线字幕」使用本地识别和本地翻译，云端实时会话仍是候选。DeepSeek 代码默认 `deepseek-v4-flash`，官方仍接收该别名，实际由 V4.1-Flash-1017 响应，推荐标识为 `deepseek-flash`。通用 OpenAI-compatible 可以尝试通用千问模型。专用 Qwen-MT 必须提交 `translation_options`，并需适配现有按字幕 ID 返回结构化数据的合同。[DeepSeek 模型与价格](https://api-docs.deepseek.com/quick_start/pricing/) · [Qwen-MT API](https://help.aliyun.com/zh/model-studio/qwen-mt-api)
- **实时同传。** 大陆首选仍是百炼 Qwen3.8 LiveTranslate。11 月 23 日的 WebSocket 指南确认可以只要文本，原文和译文有各自的增量事件。北京区音频输入 ¥40／百万 token、文本输出 ¥100／百万 token，建连需对应业务空间的 API Host 和 Key。腾讯云日中实时语音翻译可作国内备选，公开价 ¥5／小时。Google Gemini 3.5 Live Translate 仍标为 Preview，可取得原文和译文，官方示例也返回译音，字幕模式的计费待连接验收。OpenAI 另有专用 `gpt-realtime-translate`，持续返回译音和字幕增量，公开价 $0.034／分钟音频。Google 和 OpenAI 的 API 支持地区均不含中国大陆，留作受支持地区的对照。[百炼 WebSocket](https://help.aliyun.com/zh/model-studio/livetranslate-realtime-websocket-access) · [百炼价格](https://help.aliyun.com/zh/model-studio/qwen3-8-livetranslate-flash-realtime) · [腾讯日中接口](https://cloud.tencent.com/document/product/1093/127565) · [腾讯价格](https://cloud.tencent.com/document/product/1093/35686) · [Google Live 指南](https://aistudio.google.com/docs/live-api/live-translate) · [Google 地区](https://ai.google.dev/gemini-api/docs/available-regions) · [OpenAI 模型](https://developers.openai.com/api/docs/models/gpt-realtime-translate) · [OpenAI 地区](https://developers.openai.com/api/docs/supported-countries)
- **纯转写。** 新增国内候选 Qwen-Audio 3.1 ASR Flash Streaming，官方列日语，支持热词和上下文，北京标价每百万 token 输入 ¥6、输出 ¥4.5。它适合作为日文原文的云端对照。若目标是给实时中文字幕兜底，先测完整音频到译文链路。未调用付费接口，本轮未读取现有 Key。[Qwen-Audio 3.1 价格](https://help.aliyun.com/zh/model-studio/qwen-audio-3-1-asr-flash-streaming) · [语种和能力](https://help.aliyun.com/zh/model-studio/asr-model/)

### 学习功能的下一轮验证

v0.1.6 已交付查词、词形说明、句型分析、526 张中文句型卡、21 张基础概念卡、收藏回听、Anki TSV 和本地误报／漏报反馈。78 个条目可自动匹配。上一轮 40 条保留字幕中，13 个标记位置命中 8 个，范围均已对齐。下一期独立电台先固定条目和应高亮范围，逐项核对漏识别、误识别及高亮边界。反馈先按 ASR 原文有误、形态解析、通用接续规则和候选排序归因，再决定规则层改动。另有两个使用候选：把口语活用和同形词的上下文证据直接展示在查词结果里，以及复习时先隐藏释义、听完原句再揭示。两项先确认使用需求，沿用现有词条、语法条目和节目时间点。[匹配与反馈](../decisions/0095-grammar-in-context-and-local-feedback.md)

### 从节目里理解，再用原句复习

| 产品 | 从素材到学习的做法 | Atogaki 可借鉴的环节 |
| --- | --- | --- |
| Language Reactor | 从字幕即时查词，可收藏整句或带上下文的词，导出到 Anki 时可挖掉目标词。 | 点选后展示本句用法。保存时保留原句、所选义项和节目时间点。 [导出文档](https://www.languagereactor.com/help/export) |
| LingQ | 阅读和听原内容时记录词义、词汇状态；在其他内容中再次遇到收藏词汇时可直接复习，也提供词汇练习。 | 同一义项可以累积多个节目例句。用学过标记辅助回看，允许暂停标记。 [学习流程](https://www.lingq.com/how-to-use-lingq/) |
| Migaku | 点字幕即看词义和上下文解释，生成带句子、音频或画面的卡片，再安排间隔复习。 | 在现有学习区优先验证听一句、回忆、揭示释义的短流程。 [功能说明](https://migaku.com/faq/features) |

建议先验证三个任务。收听时能尽快恢复播放；从收藏能回到原句对照前后文；复习时能按词、短语或句型在多个真实例子中回忆用法。日语准确性问题需要检查字幕文本、选区边界、Sudachi 形态候选、词义排序和语法规则作用域。工程上可复用同一句的本地形态分析，并在缓存键中包含字幕版本及词典版本；是否有明显耗时收益先测点击响应。质量上允许展示候选、切换义项、回听或报告匹配问题。用户操作和材料身份由 App 主导，词典与分析工具提供候选。新增复习流程和词汇状态仍是讨论中的选项。

英语已有可用的起点：在收听中框选词、短语或收藏整句，再在学习区查 ECDICT 英中释义。当前即时点词预览和按钮式句型分析只用于日语。英语最小闭环可以把现有本地释义提前到收听预览，在点击时定位单词，同时保留手动框选短语。优先验证屈折和不规则形式、常用多词表达及义项选择。例如 `the plane took off` 需要将 `took off` 归到短语 `take off`，并用原句区别「起飞」与其他义项。同一个收藏可以累积多个上下文。现有 ECDICT 精确查询及保守词形候选可先试；若覆盖不足，再测按词性还原词元的小型英语解析器。英语语法候选还要单独定义讲解目录，避免把分析器的词性或依存标签直接当教学解释。[现有本地词典](../decisions/0035-local-dictionary-indexes-and-user-triggered-lookups.md) · [spaCy 词元模式](https://spacy.io/api/lemmatizer)

## 在线字幕

用户在实际使用中认为当前本地 Qwen 小模型的识别通常已经够准。轻微的识别偏差仍可能在翻译侧变成明显误解，加入长上下文也使延迟波动更大。所以下一轮付费试验优先让同传模型直接接收音频，先看说话期间能否持续提供正确中文。本地 ASR＋云端文本翻译保留为对照或可选路径。目前延后实施，主要时间成本是多平台账号与额度管理。自建网关、额度及续费重置、与 GitHub 的 SMG 比较另立项目，本仓库暂不调研或实现。

| 候选 | 能力与状态 | 低用量公开价格及大陆接入 |
| --- | --- | --- |
| 阿里百炼 Qwen3.8 LiveTranslate | **首选验证**。实时音频到日中双语文本，可禁用译音。3.8 直接输出原文，3.5 的原文使用独立 ASR，协议也不同。 | 北京区 ¥40／百万音频输入 token、¥100／百万文本输出 token。每秒输入 7 token，约 ¥1.01／计费音频小时，再加文本。控制台开通百炼并创建北京区 Key，国内端点可直连。 [模型价格](https://help.aliyun.com/zh/model-studio/qwen3-8-livetranslate-flash-realtime) · [协议和计量](https://help.aliyun.com/zh/model-studio/qwen3-5-livetranslate-flash-realtime) · [申请](https://help.aliyun.com/zh/model-studio/get-api-key) |
| Google Gemini 3.5 Live Translate | **首选海外对照**。实时语音到译音，可取得原文和译文转写。Preview。对照时核查是否能省去译音计费，并记录时间轴稳定性。 | Gemini Developer API 约 $0.0053／输入分钟、$0.0315／输出分钟。中国大陆不在支持地区，测试使用受支持地区账号及网络。 [协议](https://aistudio.google.com/docs/live-api/live-translate) · [价格](https://aistudio.google.com/docs/pricing#gemini-3.5-live-translate-preview) · [支持地区](https://ai.google.dev/gemini-api/docs/available-regions) |
| 腾讯云实时语音翻译 WebSocket | **大陆备选**。接口接收实时音频，返回日文、中文、句 ID、起止时间和定稿状态。另一个「同声传译大模型」接口仅支持中英。 | ¥5／小时后付费；100 小时包 ¥480。腾讯云实名并开通语音识别，使用 AppID、SecretID、SecretKey 计算签名。 [接口](https://cloud.tencent.com/document/product/1093/127565) · [价格](https://cloud.tencent.com/document/product/1093/35686) · [另一个同传接口](https://cloud.tencent.com/document/product/1093/140336) |
| 阿里千问实时 ASR／腾讯实时 ASR／Google ASR | **只做转写**。作为两段式对照。阿里和腾讯上述地域可直接开通。 | Qwen3-ASR-Flash-Realtime 北京约 ¥1.19／小时；腾讯日语多语种大模型 1.0 约 ¥4.80／小时；Google Gemini Transcribe Live 官方估算约 $0.54／小时。另有 Cloud Speech-to-Text V2 标准档约 $0.96／小时，加 Cloud Translation NMT $20／百万输入字符。 [阿里价格](https://help.aliyun.com/zh/model-studio/model-pricing) · [腾讯语种](https://cloud.tencent.com/document/product/1093/35682) · [腾讯价格](https://cloud.tencent.com/document/product/1093/35686) · [Google 价格](https://aistudio.google.com/docs/pricing#gemini-3.5-transcribe-live) · [Cloud STT](https://cloud.google.com/speech-to-text/pricing) · [Cloud Translation](https://cloud.google.com/products/translate/pricing) |
| 混元 Hy-MT2／千问 Qwen-MT | **纯文本翻译**。可复用现有本地 ASR，并保留自管术语和上下文。国内平台自助开通。 | 混元 Pro／Plus 广州 ¥0.50／¥2，Lite ¥0.30／¥1.20；Qwen-MT-Flash 北京 ¥0.70／¥1.95、Plus ¥1.80／¥5.40。单位为每百万输入／输出 token。 [腾讯价格](https://cloud.tencent.com/document/product/1823/130055/) · [腾讯申请](https://cloud.tencent.com/document/product/1823/130058) · [阿里价格](https://help.aliyun.com/zh/model-studio/model-pricing) |
| B 站 Index-Translate 35B | **纯文本对照**。公开免费接口另支持图片。开源 Omni 公布的语音能力为中英。 | 无需申请真实 Key。项目明确免费服务面向体验和研究，配额、持续性、稳定性没有保证，不作为面向用户的兜底。 [模型与接口](https://github.com/bilibili/Index-Translate/blob/main/README_zh.md) · [服务边界](https://github.com/bilibili/Index-Translate/issues/27) |

Gummy 旧版价目约 ¥0.54／小时／单项，转写与翻译都开约 ¥1.08／小时。原中文接口已跳转到 LiveTranslate，当前总价表未列该模型，暂待确认新账户可用性。Claude 商业 API 的支持范围排除中国大陆，已排除。没有实测上述 API 的大陆链路，价格以实际使用地域与账单为准。 [Gummy 旧模型页](https://help.aliyun.com/en/model-studio/gummy-realtime-v1) · [Anthropic 支持地区](https://www.anthropic.com/supported-countries)

恢复试验时，先测合成日语的建连、计费、断网恢复、停止回收和末句。实际质量比较使用允许上传且有参考的音频；现有五个核准裁片是当前研究输入边界，上传权限单独确认。固定文本只用现有校正原文及确认前文。记录首条正确可读中文、说话期间译文的滞后、修订退化、专名／否定／指代错误、长会话失败与成本。原有两秒语义目标不变，厂商最低时延和首次非空输出作为辅助指标。详见[评测合同](../evaluation.md)。

### 凭据与首次连通测试

桌面现有 Gemini 文本翻译与局部音频转录共用 macOS 钥匙串项，由应用设置保存。仓库只保存 provider 实现与非秘密设置约定。2026-11-18 本机核对存在配置标记和对应钥匙串项，未读取或输出 Key，未做联网鉴权。现有文本和局部转录入口不会打开 Gemini Live。端到端测试要另开 Live WebSocket，用合成日语验证该 Key 对应项目的模型权限、地域、配额和转写回包。Google 当前会拒绝未限制的标准 Key，若旧凭据握手失败应一并核对类型与限制。百炼测试需正常开通的北京区 API Key、对应业务空间 ID（Workspace ID）和同地域端点；当前 App 无 DashScope 配置入口，可用官方示例或隔离测试脚本。临时本机密码框和 Gemini 使用同一个 Atogaki 钥匙串服务，百炼保存为独立项 `asr.bailian-livetranslate`。建 Key 的默认业务空间可以直接使用，连接前从控制台查看实际地域及业务空间 ID。两个云端方案都应先用短合成音频。 [共享凭据](../decisions/0046-gemini-text-translation-and-shared-credential.md) · [Gemini Live 指南](https://aistudio.google.com/docs/live-api/live-translate) · [Google Key 类型与限制](https://aistudio.google.com/docs/api-key) · [百炼 WebSocket 与业务空间](https://help.aliyun.com/zh/model-studio/qwen3-5-livetranslate-flash-realtime)


## 日语学习

当前从收听全文选择日文或收藏整句，学习条目保存来源时间及对应文本快照。已有 JMdict 和 Tomoshi 本地字典索引，可复用释义、读音和来源回听。主要缺口是从连续日文确定词界、把变形归到同一词条，以及用上下文判断句型。详见[现有词典路径](../decisions/0035-local-dictionary-indexes-and-user-triggered-lookups.md)。

| 候选 | 怎么用 | 边界 |
| --- | --- | --- |
| Sudachi.rs／SudachiDict Core | 用整句在本地分析词界、原形、读音、词性和活用。保留字幕字符范围，在现有 JMdict／Tomoshi 中查候选义项。先验证 Rust 版本与字典版本配对。 | ASR 错字、专名、口语缩略和跨词形态会产生错切。词本去重用已确认字典入口或原形＋读音＋词性，保留每次实际遇到的变形。 [Sudachi.rs](https://github.com/WorksApplications/sudachi.rs) · [字典版本](https://github.com/WorksApplications/SudachiDict) |
| Yomitan 的词尾规则 | 参考其逆活用思路：从选区或鼠标位置生成候选，允许叠加词尾变化，再用词典筛词。适合作为分词有歧义时的补充。 | 复制实现或规则之前核对许可。先用整句分析和现有词典做小样本比较。 [Yomitan 实现指南](https://github.com/yomidevs/yomitan/blob/master/docs/development/language-features.md) |
| 《新标日》与 Irodori | 参考公开课程的「辞书形／て形／ない形／た形」及各自常见句型，做 Atogaki 自己撰写的短释义、自编例句、反例、前置概念和相关条目。教材引用可记录版次、册次、课次及公开链接。 | 人教社页给出公开目录，详细讲解在激活码配套电子书内。Irodori 有公开文法笔记及独立使用条件，本轮可作引用及体系参考。 [人教社目录](https://www.pep.com.cn/products/jc/dwhytsh/201711/t20171107_1920490.shtml) · [公开课程大纲](https://www.chinaooc.com.cn/course/67ae7670225d72705e1c671a) · [Irodori 文法资源](https://www.irodori.jpf.go.jp/resources.html) · [使用条款](https://www.irodori.jpf.go.jp/en/privacy.html) |

可试的交互是「点词看原形，框选看整句语法」。比如「まだ食べていなかった」中的「食べ」归到词汇「食べる」，同时显示「食べて」是て形；语法卡把「食べていなかった」整体标为「～ている」的过去否定，点开后可跳转到「て形」「～ている」「否定・过去」。书签同时保留原句、播放位置和用户选择的义项。音频听不清时可收藏待确认，避免自动把错字加入词本。遇到「そうだ」时应结合词形和前后句区分样态／传闻，缺少证据时列候选。

语法候选可先用本地形态序列、接续规则和稳定条目 ID。按需使用 AI 为候选排序或解释歧义，提供它可见的句子证据和有限候选，禁止凭空新造条目。可按文本修订和语法库版本缓存。首批覆盖范围、解析器和资料来源都待小样本验证。样本至少含活用、惯用搭配、歧义、口语缩略和故意的 ASR 误听，记录原形命中、候选覆盖、误报率及从选句到理解的操作量。

### 试用与选型顺序

- 先体验 [Yomitan](https://yomitan.wiki/getting-started/) 的悬停查词和卡片流程。词典从原形给释义，选区保留真实用例。要测试的交互是点词当下能否看到足够信息，以及同一原形的不同变形能否归在一处。
- 用同一组短字幕比较 [Sudachi.rs](https://github.com/WorksApplications/sudachi.rs) 和 [MeCab／UniDic](https://clrd.ninjal.ac.jp/unidic/en/about_unidic_en.html) 的原形、读音与范围。Sudachi 的多粒度分词适合同时检查复合词和组成，UniDic 的标准短单位可作对照。优先确定词条身份如何关联字典及每条来源，保留人工选义。
- 先选少量高频语法卡，验证「选句 → 高亮触发片段 → 查看接续、释义、前置概念和例句」。可用《新标日》作为用户熟悉的导航，以原创短释义和可引用教材作为内容来源。需要知道修饰关系时，再评估 [GiNZA](https://github.com/megagonlabs/ginza) 的句法分析能否减少误匹配。它提供依存关系和文节信息，教学语法点仍需要自己的条目与匹配规则。
- 主线先用已有收藏、回听和文本 Anki 导出串起来。首轮选择本地解析器与语法卡的内容边界后，再决定要不要引入 AI 歧义解释和更完整的句法模型。


### 选区容错、词义歧义与全级覆盖

**选区。** 用户确认点词、分词、还原原形、句型理解四项均有价值。点词输入应是字幕偏移，先分析完整句子，再用偏移定位一组可点击候选。框选只标记感兴趣的范围，解析器继续读取整句及必要的相邻字幕行；界面回显实际命中范围。在「昨日は本を読んでいたけど」中，选成「を読んでい」也可建议「読む」或完整「読んでいた」的语法范围；选区跨多个有意义的单位时同时列出，不静默丢弃否定、助词或条件。Sudachi 的多粒度切分和未登录词标记有助于容错，Yomitan 的从点击位置扫描、词尾逆活用和词典匹配有助于解决后缀过长。半词起点、口语、省略、错字和跨字幕断句仍需依赖整句、候选和可更正的高亮。 [Sudachi](https://github.com/WorksApplications/Sudachi) · [Yomitan 词典扫描](https://github.com/yomidevs/yomitan-api/blob/master/docs/api_paths/tokenize.md)

**词典歧义。** 本机 JMdict 日英归档 `3.6.2+20261116213322` 有 219,127 条，其中 23,432 条含多个义项。以同一完整假名读音指向不同 entry ID 计算，236,951 种读音有 13,935 种指向多个词条（5.9%）；只保留 JMdict 标为 common 的读音记录，20,397 种中有 1,772 种（8.7%）。这是词典类型统计，和字幕里实际碰到同音词的比例不同。Tomoshi `v2026-10-13` 有 217,538 条、对应简体中文释义记录 217,538 行，两套数据含重叠。当前 Tomoshi 桌面入口用精确词形查询并取首条，资料源结构本身支持词形、读音与义项。后续可用原文写法、原形、读音、词性及邻近搭配来排序候选，区分不同词条和同一词条的多个义项。词本保存已选词条／义项与实际用例。 [JMdict 数据结构](https://www.edrdg.org/jmdict/jmdict_dtd_h.html) · [Tomoshi 发布库](https://github.com/tomoshi-app/tomoshi-dict-data)

**N5 至 N1。** JLPT 现行考试没有发布官方逐条语法清单。OpenJLPT 有 526 条（N5 到 N1 为 81／98／101／123／123），按 CC BY-SA 4.0 发布，包含接续说明、例句和稳定 ID，是候选底库。Bunpro 公开目录实数为 979 个独立链接（132／185／220／221／221），可作为一千条量级的参照。两者划分粒度不同，目录规模只用于估算。如果每条人工核对接续、中文解释及例句用 5–10 分钟，约 500 条需 42–83 小时、约 1,000 条需 83–167 小时，粗估未含解析器开发、命中规则、专项复核和失败案例。优先核对候选数据来源及许可后再试导入。 [JLPT 官方说明](https://jlpt.jp/sp/e/faq/index.html) · [OpenJLPT 数据](https://github.com/evanclan/OpenJLPT) · [Bunpro 公开目录](https://bunpro.jp/grammar_points)

可按 N5 至 N1 建立可查条目，再用两阶段完成句型识别。第一阶段使用本地形态词性、接续和规则优先匹配，并展现实际命中范围；第二阶段按需解释歧义、补全未命中句子的分析，并标注来源与推断。可先把 150–250 个高频句型作为强化本地规则的预算，按电台与口语中的实际出现次数排名，跨越 JLPT 等级。这个预算和语法覆盖率分别测量。用人工标注的节目与日常会话抽样，分别报告主要句型召回、准确命中比例、整句有帮助的比例和 AI 触发比例；必要时用 NINJAL CEJC 辅助排名。验收完整交互时，未命中仍可显示词形、可查的条目及按需解释。解析器、目录来源及首批规则数量待验证。 [NINJAL 日常会话语料](https://www.ninjal.ac.jp/english/research/cr-project/project-4/spoken-language/)

### 词法样本对照（2026-11-19）

同一组 11 条手写字幕用 SudachiPy 0.7.0／Core 20261015（C 与 A）和 fugashi 1.5.2／unidic-lite 1.0.8 对照。样本为「昨日は本を読んでいたけど」「その本はもう読みました」「まだ食べていなかった」「それ買っちゃったんだよね」「東京大学のラジオ番組です」「聞いてる人いますか」「ここで箸を使う」「端に置いといて」「はしを渡ってきた」「ヨルシカの新曲を聴いた」「本をよんｄでた」。

开发分支选择 Sudachi 的双粒度结果。对「東京大学」同时保留复合词和组成词，且「ヨルシカ」没有切成「ヨル／シカ」；在「聴いた」保留「聴く」写法。UniDic 将「はしを渡ってきた」中的「はし」分析为「箸」，Sudachi 仍给假名「はし」。两家都没有解析出故意错写的「よんｄ」所指的「読む」。基于这些样本，产品显示候选，由用户选义，释义继续使用 Tomoshi／JMdict。详见[决策 0089](../decisions/0089-local-japanese-dictionary-lookup.md)。



日语句型已选择 Sudachi A 粒度与本地接续规则，并采用固定版 OpenJLPT 语法目录。首轮 24 个自动识别句型、3 个中文概念及来源收藏已进入开发分支。150–250 条仍只是未来规则覆盖的研究预算。见[决策 0090](../decisions/0090-local-japanese-grammar-and-catalog.md)。

### 日语句型匹配的参考实现（2026-11-20）

- **形态序列和复合成分。** 土屋雅稔、佐藤理史的《文法の規格とその自動判定》将教学接续表示成 JUMAN 形态素序列，匹配词性与词形，并把复合词、词缀等组合成接续所需的单位。论文在 1,110 条教材例句上报告精确率 89%、召回率 51%。这组数字只适用于论文实验。将「イタリア製」识别为一个名词性成分，以及「開けっぱなし」被分词器合为一词，说明规则槽位和分词粒度需要分别处理。[论文](https://imc.tut.ac.jp/~tsuchiya/pub/nlp2003.pdf)
- **规范形、派生形和语境消歧。** Tsutsuji 相关研究用层级词典关联同一个功能表达的规范形及派生形式。日语阅读辅助的后续研究用形态规则处理通常是功能性用法的表达，另用统计分块处理需要消歧的用法。这里可借鉴对同一条目归并礼貌体、活用和缩略的做法。论文数据的使用条件需要另核。[规范形和派生形检测](https://aclanthology.org/L12-1537/) · [阅读辅助的混合识别](https://aclanthology.org/W10-3712/)
- **可直接查看的规则代码。** `nnj-grammar` 在 Lindera／UniDic 之上使用 TOML 描述语法规则。已检查提交 `dbc22e0` 的 matcher，核心范围、前后文条件、捕获槽位和独立匹配的重叠处理可分别表达。规则数据依赖 Hanabira，第三方声明指出上游 CC 的具体变体不明确。Atogaki 参考其结构与原理，没有导入它的代码或规则数据。当前没有运行对照基准。[匹配器源码](https://github.com/alejoseed/nnj-grammar/blob/dbc22e0e3074c2ee91ecd130ef51805e01ebf851/src/matcher.rs) · [数据署名](https://github.com/alejoseed/nnj-grammar/blob/dbc22e0e3074c2ee91ecd130ef51805e01ebf851/THIRD_PARTY_NOTICES.md)
- **文节和依存关系。** GiNZA 提供分句、文节范围及文节主辞的依存关系。它可以帮助判断哪个否定谓语管辖某个助词，或「のに」两边是什么成分，仍需 Atogaki 将这些结构与教学条目 ID 对齐。国立国语研究所的 CEJC 口语转写和 NPCMJ 句法标注可分别用于找口语样本及核对句法现象，采用前核对使用条件。[GiNZA 文节 API](https://github.com/megagonlabs/ginza/blob/develop/docs/bunsetu_api.md) · [NINJAL 语料入口](https://clrd.ninjal.ac.jp/en/)

在本机用 SudachiPy 0.7.0 和 Core 20261015 的 A 粒度检查了 5 条手写句，验证接续所需的形态字段。「パンしか買わない」和「パンしか買いません」分别以「ない」及「ませ＋ん」满足同一个句型所需的否定条件。「『パンしか』とは言っていない」的否定位于引语之外，只有把槽位限制在合适的范围内才能避免乱配。「雨が降るのに、外で撮るの？」和「細かい字を読むのに便利だ」中的「のに」均分成「準体助詞＋格助詞」，前接动词普通形，用途、转折要看右侧谓语和分句关系。这是形态探查，未测现有规则识别率。

首个对照可用当前 37 条为基线，让规则声明已知锚点、有限跨度、词性和句界，再分别评估文节／依存约束与 LLM 候选消歧。LLM 可在开发阶段起草接续和反例，离线规则用真实 Sudachi 结果逐一检查；如运行时使用，限制它只选择候选条目 ID 并引用原文范围，允许返回证据不足。已在本地验收的命中可直接显示。用同一份独立人工标注字幕统计条目 ID 和范围的精确率、召回率、延迟及模型触发率，并记录由 ASR 或分词引起的失败。句型目录的中文覆盖单独统计。

#### 批量生成规则和口语验证

在固定提交 `dbc22e0` 中，`nnj-grammar` 用导入器解析 Hanabira 条目的接续字段，必要时尝试标题。词类槽位由 `hosts.json` 映射到 UniDic 谓词或有限通配范围，固定表达分解成核心与前后文后输出 TOML。项目将助动词的词典清单与人工维护的语法分类分开，并用测试检查每个词典项目都得到分类。其文档也记录了整类扩充带来的误报，例如把不同意义的「てる」和「ておく」视为同一类候选。接续编译可批量完成，允许互换的变体应由同义和作用范围约束。[导入器](https://github.com/alejoseed/nnj-grammar/blob/dbc22e0e3074c2ee91ecd130ef51805e01ebf851/tools/import_hanabira.py) · [项目的误报记录](https://github.com/alejoseed/nnj-grammar/blob/dbc22e0e3074c2ee91ecd130ef51805e01ebf851/docs/PIPELINE.md)

Atogaki 已有匹配器和 OpenJLPT 的接续字段。可在开发阶段将可解析的接续批量转换为当前 Sudachi 谓词，显式留出需要判别的条目，再按口语用例评估命中与误报。字幕任务库可在本机只读取得原文、时间和修订标记。用同一节目相邻片段找漏项，再留出独立片段作评估。单纯统计出结果的字幕条数只反映命中密度，精确率和召回率需标注。节目原文留在本机。

## 离线字幕校对

本轮比较官方文档和源码目录描述，未运行三个项目的同素材精度基准。以日语原文、中文语义、时间轴三个问题分别判断该工具能解决什么。

| 项目 | 如何提高准确度与控制开销 | 如何定位和修复 |
| --- | --- | --- |
| Subtitle Edit | Improve Time Codes 用 Crisp ASR 对已有文字做 forced alignment；勾选 ASR 检查时，可先处理整体偏移或漂移，再局部对齐。大幅移动或 ASR 有异议的行默认留给人工。规则检查可捕获重叠、长短显示时长和间隙，不能证明语义对。 | 修复先给候选列表。待检查行可按前后跳转，试听并比较旧／新时间，勾选后才应用。Fix Common Errors 按规则分组，可批量应用明确修正并查看无法自动修的项目。 [打轴](https://github.com/SubtitleEdit/subtitleedit/blob/main/docs/features/improve-time-codes.md) · [常见错误](https://github.com/SubtitleEdit/subtitleedit/blob/main/docs/features/fix-common-errors.md) |
| pyVideoTrans | 分阶段识别、翻译和合成，支持可选人声分离、降噪、标点恢复和 LLM 纠错。已有 SRT 可跳过对应阶段；翻译带缓存，检查源／译条数。其配音后的二次识别针对生成的配音音轨，不提供原视频通用修复。 | 单视频可在原文和译文阶段暂停，用户改文本、时间或分配说话人后继续；批量模式自动推进。可借鉴阶段成果可单独重用，Atogaki 的轻校对优先在全流程完成后把少数问题交给用户。 [架构与校对对话框](https://github.com/jianchang512/pyvideotrans/blob/main/docs/architecture.md) · [独立模式与已有 SRT](https://github.com/jianchang512/pyvideotrans/blob/main/docs/faq.md) |
| stable-ts | 基于非语音区修剪时间或微调边界；align 对可信文字重对齐，align_words 限制在已有时间窗内，快但修不了窗外的大偏移。保存 JSON 后可复用推理结果重跑断句和时间修正。低音量、音乐与语音混合的误判需试听。 | 主要是 CLI／库，结果可检查和编辑，没有面向普通用户的疑点工作流。上游写明开发暂停，先借鉴算法与可复现的中间产物。 [实现与状态](https://github.com/jianfch/stable-ts) |

Atogaki 已有[疑点提示、局部候选和重译](../offline-workflow-validation.md)。未来试验先复用这些入口及 Qwen ForcedAligner，导入后完成 ASR → 对齐 → 翻译。完成页给出按严重性排列的小清单，点疑点播放带前后余量的音频。文本正确但偏轴时可「只对齐」并试听前后变化；文本确实错时可「重识别这一小段」后选择采用，或直接编辑并重译受影响分组；纯译文错误只处理对应分组。始终保留查看全文和打开完整编辑器的入口。初次比较既要测每小时节目耗时、局部修复等待，也要人工抽查疑点之外的漏报，避免用数量减少替代质量提升。离线字幕流程改动排在在线服务验证之后，待有完整节目和人工校对时间时再启动。
