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

## 日语学习

当前从收听全文选择日文或收藏整句，学习条目保存来源时间及对应文本快照。已有 JMdict 和 Tomoshi 本地字典索引，可复用释义、读音和来源回听。主要缺口是从连续日文确定词界、把变形归到同一词条，以及用上下文判断句型。详见[现有词典路径](../decisions/0035-local-dictionary-indexes-and-user-triggered-lookups.md)。

| 候选 | 怎么用 | 边界 |
| --- | --- | --- |
| Sudachi.rs／SudachiDict Core | 用整句在本地分析词界、原形、读音、词性和活用。保留字幕字符范围，在现有 JMdict／Tomoshi 中查候选义项。先验证 Rust 版本与字典版本配对。 | ASR 错字、专名、口语缩略和跨词形态会产生错切。词本去重用已确认字典入口或原形＋读音＋词性，保留每次实际遇到的变形。 [Sudachi.rs](https://github.com/WorksApplications/sudachi.rs) · [字典版本](https://github.com/WorksApplications/SudachiDict) |
| Yomitan 的词尾规则 | 参考其逆活用思路：从选区或鼠标位置生成候选，允许叠加词尾变化，再用词典筛词。适合作为分词有歧义时的补充。 | 复制实现或规则之前核对许可。先用整句分析和现有词典做小样本比较。 [Yomitan 实现指南](https://github.com/yomidevs/yomitan/blob/master/docs/development/language-features.md) |
| 《新标日》与 Irodori | 参考公开课程的「辞书形／て形／ない形／た形」及各自常见句型，做 Atogaki 自己撰写的短释义、自编例句、反例、前置概念和相关条目。教材引用可记录版次、册次、课次及公开链接。 | 人教社页给出公开目录，详细讲解在激活码配套电子书内。Irodori 有公开文法笔记及独立使用条件，本轮可作引用及体系参考。 [人教社目录](https://www.pep.com.cn/products/jc/dwhytsh/201711/t20171107_1920490.shtml) · [公开课程大纲](https://www.chinaooc.com.cn/course/67ae7670225d72705e1c671a) · [Irodori 文法资源](https://www.irodori.jpf.go.jp/resources.html) · [使用条款](https://www.irodori.jpf.go.jp/en/privacy.html) |

可试的交互是「点词看原形，框选看整句语法」。比如「まだ食べていなかった」中的「食べ」归到词汇「食べる」，同时显示「食べて」是て形；语法卡把「食べていなかった」整体标为「～ている」的过去否定，点开后可跳转到「て形」「～ている」「否定・过去」。书签同时保留原句、播放位置和用户选择的义项。音频听不清时可收藏待确认，避免自动把错字加入词本。遇到「そうだ」时应结合词形和前后句区分样态／传闻，缺少证据时列候选。

语法候选可先用本地形态序列、接续规则和稳定条目 ID。按需使用 AI 为候选排序或解释歧义，提供它可见的句子证据和有限候选，禁止凭空新造条目。可按文本修订和语法库版本缓存。首批覆盖范围、解析器和资料来源都待小样本验证。样本至少含活用、惯用搭配、歧义、口语缩略和故意的 ASR 误听，记录原形命中、候选覆盖、误报率及从选句到理解的操作量。

## 离线字幕校对

本轮比较官方文档和源码目录描述，未运行三个项目的同素材精度基准。以日语原文、中文语义、时间轴三个问题分别判断该工具能解决什么。

| 项目 | 如何提高准确度与控制开销 | 如何定位和修复 |
| --- | --- | --- |
| Subtitle Edit | Improve Time Codes 用 Crisp ASR 对已有文字做 forced alignment；勾选 ASR 检查时，可先处理整体偏移或漂移，再局部对齐。大幅移动或 ASR 有异议的行默认留给人工。规则检查可捕获重叠、长短显示时长和间隙，不能证明语义对。 | 修复先给候选列表。待检查行可按前后跳转，试听并比较旧／新时间，勾选后才应用。Fix Common Errors 按规则分组，可批量应用明确修正并查看无法自动修的项目。 [打轴](https://github.com/SubtitleEdit/subtitleedit/blob/main/docs/features/improve-time-codes.md) · [常见错误](https://github.com/SubtitleEdit/subtitleedit/blob/main/docs/features/fix-common-errors.md) |
| pyVideoTrans | 分阶段识别、翻译和合成，支持可选人声分离、降噪、标点恢复和 LLM 纠错。已有 SRT 可跳过对应阶段；翻译带缓存，检查源／译条数。其配音后的二次识别针对生成的配音音轨，不提供原视频通用修复。 | 单视频可在原文和译文阶段暂停，用户改文本、时间或分配说话人后继续；批量模式自动推进。可借鉴阶段成果可单独重用，Atogaki 的轻校对优先在全流程完成后把少数问题交给用户。 [架构与校对对话框](https://github.com/jianchang512/pyvideotrans/blob/main/docs/architecture.md) · [独立模式与已有 SRT](https://github.com/jianchang512/pyvideotrans/blob/main/docs/faq.md) |
| stable-ts | 基于非语音区修剪时间或微调边界；align 对可信文字重对齐，align_words 限制在已有时间窗内，快但修不了窗外的大偏移。保存 JSON 后可复用推理结果重跑断句和时间修正。低音量、音乐与语音混合的误判需试听。 | 主要是 CLI／库，结果可检查和编辑，没有面向普通用户的疑点工作流。上游写明开发暂停，先借鉴算法与可复现的中间产物。 [实现与状态](https://github.com/jianfch/stable-ts) |

Atogaki 已有[疑点提示、局部候选和重译](../offline-workflow-validation.md)。未来试验先复用这些入口及 Qwen ForcedAligner，导入后完成 ASR → 对齐 → 翻译。完成页给出按严重性排列的小清单，点疑点播放带前后余量的音频。文本正确但偏轴时可「只对齐」并试听前后变化；文本确实错时可「重识别这一小段」后选择采用，或直接编辑并重译受影响分组；纯译文错误只处理对应分组。始终保留查看全文和打开完整编辑器的入口。初次比较既要测每小时节目耗时、局部修复等待，也要人工抽查疑点之外的漏报，避免用数量减少替代质量提升。离线字幕流程改动排在在线服务验证之后，待有完整节目和人工校对时间时再启动。
