# 在线同传、日语学习与离线校对调研

_核查日期：2026-11-18。以下选型均未冻结。采用或淘汰候选时，更新本页的状态、日期和证据，并同步路线图与相关决策。_

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

## 离线字幕校对

本轮比较官方文档和源码目录描述，未运行三个项目的同素材精度基准。以日语原文、中文语义、时间轴三个问题分别判断该工具能解决什么。

| 项目 | 如何提高准确度与控制开销 | 如何定位和修复 |
| --- | --- | --- |
| Subtitle Edit | Improve Time Codes 用 Crisp ASR 对已有文字做 forced alignment；勾选 ASR 检查时，可先处理整体偏移或漂移，再局部对齐。大幅移动或 ASR 有异议的行默认留给人工。规则检查可捕获重叠、长短显示时长和间隙，不能证明语义对。 | 修复先给候选列表。待检查行可按前后跳转，试听并比较旧／新时间，勾选后才应用。Fix Common Errors 按规则分组，可批量应用明确修正并查看无法自动修的项目。 [打轴](https://github.com/SubtitleEdit/subtitleedit/blob/main/docs/features/improve-time-codes.md) · [常见错误](https://github.com/SubtitleEdit/subtitleedit/blob/main/docs/features/fix-common-errors.md) |
| pyVideoTrans | 分阶段识别、翻译和合成，支持可选人声分离、降噪、标点恢复和 LLM 纠错。已有 SRT 可跳过对应阶段；翻译带缓存，检查源／译条数。其配音后的二次识别针对生成的配音音轨，不提供原视频通用修复。 | 单视频可在原文和译文阶段暂停，用户改文本、时间或分配说话人后继续；批量模式自动推进。可借鉴阶段成果可单独重用，Atogaki 的轻校对优先在全流程完成后把少数问题交给用户。 [架构与校对对话框](https://github.com/jianchang512/pyvideotrans/blob/main/docs/architecture.md) · [独立模式与已有 SRT](https://github.com/jianchang512/pyvideotrans/blob/main/docs/faq.md) |
| stable-ts | 基于非语音区修剪时间或微调边界；align 对可信文字重对齐，align_words 限制在已有时间窗内，快但修不了窗外的大偏移。保存 JSON 后可复用推理结果重跑断句和时间修正。低音量、音乐与语音混合的误判需试听。 | 主要是 CLI／库，结果可检查和编辑，没有面向普通用户的疑点工作流。上游写明开发暂停，先借鉴算法与可复现的中间产物。 [实现与状态](https://github.com/jianfch/stable-ts) |

Atogaki 已有[疑点提示、局部候选和重译](../offline-workflow-validation.md)。未来试验先复用这些入口及 Qwen ForcedAligner，导入后完成 ASR → 对齐 → 翻译。完成页给出按严重性排列的小清单，点疑点播放带前后余量的音频。文本正确但偏轴时可「只对齐」并试听前后变化；文本确实错时可「重识别这一小段」后选择采用，或直接编辑并重译受影响分组；纯译文错误只处理对应分组。始终保留查看全文和打开完整编辑器的入口。初次比较既要测每小时节目耗时、局部修复等待，也要人工抽查疑点之外的漏报，避免用数量减少替代质量提升。离线字幕流程改动排在在线服务验证之后，待有完整节目和人工校对时间时再启动。
