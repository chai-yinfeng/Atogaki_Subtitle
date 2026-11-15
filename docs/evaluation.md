# 当前评测：校正文本基线与 native MLX 对照

_更新：2026-11-15。文本基线已收用户反馈；下一阶段仅在同一正向名单的裁片上比较 runtime，不恢复历史候选池。_

## 本轮只验证什么

第一阶段固定用户已经校正的待译文字 G，以及明确的确认前文 H，检查现有翻译是否忠实。参考后文 R 始终为空，场景／术语 F 也保持空；该阶段不运行 ASR，不混入 runtime 对比。2026-11-15 用户确认这轮结果后，授权先比较现有 Python／MLX 与 Voxt native MLX，再看投机解码；两个阶段分别保存输入、结果及结论，不做整套离线／在线排名或录音功能。

H/G 是同一份校正文本的两个用途，不是两组音频。由于相邻未校正句子不能补入，这轮仅在同一校正段内按明确字符位置切出前文和待译文字；两者拼接逐字等于用户原文。音频仍为原听审裁片，时间只描述整个裁片，不伪造 H/G 的词级声学边界。

## 唯一当前测试集合

[当前配置](../experiments/online/datasets/japanese-radio/confirmed-text-baseline.json)采用正向名单，来源是用户首批听审导出中亲自填写的日文。每项绑定原导出／裁片 manifest 的 SHA-256、原文摘要和音频摘要；不从任何候选 ASR 输出补字，不把“可接受”评分替代明确原文。

| ID | 节目与裁片范围 | 本轮关注 |
| --- | --- | --- |
| radio-01-012 | Vol.1，69.382–83.308 s | 喜好与信任表达、跨句前文 |
| radio-11-008 | Vol.34，206.390–215.532 s | 人物回忆与饭局叙述 |
| radio-11-012 | Vol.34，248.150–254.444 s | 比喻与表情描写 |
| radio-11-018 | Vol.34，293.462–305.724 s | 前世身份表达，避免新增评价 |
| radio-11-020 | Vol.34，312.054–314.988 s | 广播转入下一封来信 |

共五个既有裁片约 44.558 秒，属于挑选后的开发诊断，不代表全片准确率。原文是用户语义校正稿，不声称每个字均为声学逐字 gold；本轮不计算 ASR CER、词级延迟或自动翻译准确率。

仍有待定读法的 radio-01-016／029／013 不进入这轮；只打分、未填写原文的 radio-01-028／radio-11-007 也不补成标准答案。H01 只保留此前明确核准的中间范围，不能扩大为整个 Whisper 裁片已核准；H02 听不清的排除结论保留。八个三分钟候选、十三分钟模型回归、英语电台、湖吉素材、未校正 Whisper／Qwen／Gemini 或 OCR 输出均退出当前正确性基线，不删除原始证据。

## 测试条件与实际执行

每个 G 先直接翻译，再在有同段校正前文时加入 H；G 完全相同，R 永远为空。四个片段有可分出的 H，读信那段没有已校正前文，H 保持空，不重复执行相同请求。所有 H 来自用户校正稿的较早字符，绝不补未核准邻句。

沿用生产 `online_worker.mt.HyMT2`，Hy-MT2 1.8B Q4_K_M、8192 context、单 slot／Metal，原模型／server 和适配器摘要冻结。实际只调用现有 MT 适配器，不经动态分组、预译／提交调度；因此这轮先验证文本条件下的翻译与上下文组织，尚未证明完整流式状态机或最终阅读正确率。没有合成暖机句、ASR、参考后文或云端调用。

2026-11-14 完成五项／九次本地请求，九次均正常返回，未发生 MT 拒绝，R 与实际后文块均为空，自启服务已回收。2026-11-15 用户确认：按标点切分的 H/G 没有切断意思，中文结果总体可接受；指出的例外是「オイラン」保留片假名，未译成「花魁」。加入 H 总体略有帮助但影响不大，很多结果一致，小场景集合不足以证明长期收益。该反馈取代此前助手较严格的待核对判断，不擅自生成逐项评分或总体准确率，也不改变原 prompt 或补入词表重跑。

输入／实际 prompt／结果和清理记录保存于忽略的 `local-artifacts/evaluation/confirmed-text-no-lookahead-20261114/`；`inputs.json` 绑定用户导出、裁片和 H/G 字符范围，`inputs.md`、`review.md` 可直接核对；旧实验不参与这份结果。执行命令为私有 runtime 的 `python3 local-artifacts/evaluation/confirmed-text-no-lookahead-20261114/run_mt.py`，模型和 API 鉴权仍由本机临时服务管理，不记录密钥。工作树父提交为 `987a1b0`，环境沿用同机 Python／MLX／llama-server；具体摘要与版本见私有 metadata。

## 第二阶段：同裁片、无投机的 runtime 对照

只读取上述五个已裁切 WAV；不从旧候选池补音频，不把模型新输出升为标准答案。沿用同一 Qwen3-ASR 1.7B 8bit checkpoint，日语、无 context、greedy、512 token 上限，无 draft。完整裁片推理只回答实现栈的文字／耗时／内存差异，不能证明实时预览、ASR／MT 资源竞争、字幕上屏或长时内存稳定性。

Voxt 2.0.3 固定提交 `4e03928295bba17c8196644f50d69e58facff6ea` 的 native 依赖 MLX 0.32.3，本机原 runtime 为 0.32.4。隔离构建上游原代码，保留原安装环境；另建相同 Python 3.11.16、ASR 包 0.4.4 的 MLX 0.32.3 对照。除当前 FP16 和同版本 FP16 外，加入同版本 BF16 对照，以及 native reference／voxt_swift 两种布局，区分版本、计算精度与音频布局。native 直接保留 checkpoint 的 BF16 浮点张量，不能将它与现有 FP16 的全部差异归给 C++。

每臂三轮，第二轮倒序，首轮与后两轮分别保留；不额外加入暖机语料。两边设置 512,000,000 bytes allocator cache 上限并在请求间清 cache，但这不是总内存上限。Python 使用预载入 PCM，native CLI 的计时包含 WAV 解码、排除文件读取；循环／EOS 规则仍分别沿用各实现并记录，不能称为纯语言开销或逐 bit 等价测试。采样量小且执行臂串行，耗时只作同机诊断，不计算代表产品 SLO 的 p95。

计划、命令、环境与完整输出留在忽略的 `local-artifacts/evaluation/native-mlx-baseline-20261115/`；源码、构建日志与隔离 runtime 位于 `local-artifacts/research/voxt-native-20261115/`。DFlash／DSpark、R、动态分组和录音尚不进入本阶段；生产识别与翻译默认值保持。

### 2026-11-15 执行结果

同机 M4／24 GiB、macOS 27.0.1，使用上游 `sglang_omni_mlx/native/scripts/build_runtime.sh <isolated-runtime> <isolated-build>` 原样构建成功，构建时 Python 3.12；推理对照的两套 Python 均为 3.11.16、mlx-qwen3-asr 0.4.4、NumPy 2.4.6。执行 `python3 local-artifacts/evaluation/native-mlx-baseline-20261115/run_all.py`，编排器顺序启动隔离子进程；全部五臂各 15 次完成，Python 均 EOS 且不截断，native 均 stop 且 `verify_rounds=0`。原始命令、二进制／脚本摘要、每轮结果和输入摘要保留，运行期间没有 MT、网络推理或 App 安装。

| 配置 | 第二／三轮五段总计算时间均值 | 最大单请求 MLX peak active |
| --- | --- | --- |
| 当前 Python／MLX 0.32.4，FP16 | 5.059 s | 2.829 GB |
| 隔离 Python／MLX 0.32.3，FP16 | 4.785 s | 2.809 GB |
| 隔离 Python／MLX 0.32.3，BF16 | 4.847 s | 2.817 GB |
| native／MLX 0.32.3，reference | 4.423 s | 3.291 GB |
| native／MLX 0.32.3，voxt_swift | 4.571 s | 3.295 GB |

GB 为十进制字节。MLX peak active 不是 RSS 或整个 App 总内存，不能相加；native 的 `/usr/bin/time -l` footprint 与 RSS、Python `ru_maxrss` 另存，不混成统一内存排名。首轮分别为 4.641／5.564／5.532／5.948／4.711 s；各臂先后执行，首轮包含不同的 kernel／系统缓存状态，不能从它推导冷启动排名。每臂仅两轮后续耗时且各段生成长度可能不同，表格不是显著性检验或稳定提速保证。

两套 FP16 的五段输出完全一致。BF16 相比 FP16 有一段措辞／标点变化；native reference 的五段则与同版本 Python BF16 逐字相同。各臂内部三轮文字均稳定。在此条件下，reference native 的总计算时间比同版本 BF16 Python 约低 8.7%，但 MLX 峰值分配更高；尚不能证明端到端字幕收益或将差异全部归给去掉 Python。

voxt_swift 相比 reference 有三段文字变化：radio-01-012 含关键误听词变化，radio-11-008 是标点／句界变化，radio-11-012 有句尾变化。关键误听并未因 native 自动解决，不能由五段输出差异得出质量更高的结论；逐段人工原文及五臂文本见私有 `review.md`。后继若测试 DFlash，先固定同一个 native 布局、主模型、token 预算和停止规则，只改变 draft，并再次检查输出、耗时和额外内存；生产迁移、实时前缀保留及长时稳态各需另行验证。

## 第三阶段：native DFlash 与 Python 兼容性

2026-11-15 用户要求先测 native DFlash，再看当前 Python／MLX 配合 DSpark 和 DFlash。native 部分只用原五个裁片、同一主模型与已构建二进制，MLX 0.32.3，日语、无 context、512 token 上限、greedy、相同停止规则、512,000,000 bytes allocator cache 和请求间清 cache；分别固定 voxt_swift／reference 布局，每种布局对照无 draft／有 DFlash，各三轮、第二轮倒序。此轮重新运行无 draft 对照，不拿前一天或另一精度的性能数字直接作分母。

DFlash 使用 Voxt 固定的 `wilsonzheng0327/Qwen3-ASR-1.7B-dflash`，revision `23a0db8ae3f0724ccb7227c969a57651ddade2e0`。下载并核验 config 与 BF16 权重：权重 725,684,336 bytes，SHA-256 `00c32b078e3613073c0437e69519eb7811cbf97fcbd0066d05d62be7273670b8`；不额外量化 draft，不改变 App 模型目录。执行 `python3 local-artifacts/evaluation/speculative-asr-20261115/run_native.py`。四臂共 60 次均正常 stop，无截断；baseline 的 verify_rounds 全为 0，draft 全大于 0，各臂内部三轮文字一致。

| native 配置 | 后两轮五段总计算时间均值 | 最大单请求 MLX peak active | 相对同布局 baseline 的文字差异 |
| --- | --- | --- | --- |
| voxt_swift，无 draft | 4.449 s | 3.295 GB | — |
| voxt_swift，DFlash BF16 | 3.050 s | 4.021 GB | 2 / 5 段不同 |
| reference，无 draft | 4.335 s | 3.291 GB | — |
| reference，DFlash BF16 | 2.949 s | 4.016 GB | 0 / 5 段不同 |

耗时分别减少 31.5%／32.0%，对应约 1.46×／1.47×；这与“速度增加 31%”不是同一口径。首轮四臂总时间依次为 4.623／3.492／4.365／3.018 s。voxt_swift 的 radio-01-012 有词句变化，radio-11-018 有用词／书写变化，原关键误听仍在；不将此当作纠错或准确率提升。reference 五段逐字相同，也只证明本样本没有观察到文字偏离。batch verify 与逐 token 的低精度数值路径可能不同，不能用算法层面的等价性替代输出检查。

每轮 voxt_swift 生成 157 tokens／42 次 verify，reference 为 153／40；它们可描述每轮完成的 token 数与校验次数，不是 proposal 接受率。MLX peak active 不等于整机内存、RSS 或 ASR＋MT 总量。本轮只测完整裁片计算，未验证在线首次正确上屏、音频滚动重算、MT 竞争或长时内存。

### 初次 Python 兼容性调查（后继适配见第四阶段）

当前 `mlx-qwen3-asr 0.4.4` 的 `draft_model` 走完整小 Qwen ASR 提议、自带音频编码和自回归解码，不是专用 DFlash／DSpark head。隔离 Python 3.11.16／MLX 0.32.3 的实际 DFlash 加载尝试报 `ValueError: Received 60 parameters not in model`。两种专用 draft 配置经过当前解析器都会落到默认 28 层完整 ASR，而非其真实 5 层结构；不能把这种默认配置解析视为兼容。

DSpark 候选 `mattymchen/Qwen3-ASR-1.7B-speculator.dspark` 固定 revision `89a931a76b25764cb00fba0c5b4a6e8ac2604d93`，本次只获取配置／模型卡；未下载权重、未运行推理。模型卡给的是 vLLM 调用，包含缩小到 32,000 的 draft 词表映射、Markov 与 confidence head。其存在不证明现有 MLX ASR 可以加载。

核对替代实现：`mlx-qwen3-asr` main `41878a11` 仍为完整 draft ASR；`mlx-audio` `b5fcbe96` 的 Qwen ASR 未发现专用 draft 接入；`mlx-dspark` `b4a40fb2` 提供文本／视觉目标的隐藏层提取与回退包装，未接 Qwen ASR 音频 prefill；`vllm-metal` `baf8a0c8` 的 STT runner 明确声明不运行 speculative decoding，其普通 Qwen3 DSpark loader 还要求完整目标词表，不能直接套用这个 32,000 词表候选。这些是固定源码下的兼容性结论，不是对未来支持或所有 MLX 实现的否定，也不是 Python 性能分数。

初次调查时 Python 两项性能测试尚未完成，缺口是实现合同，不是本机缺 GPU。后继用户已确认先 DFlash、再 DSpark 的实验适配，见第四阶段；当时提出的范围是：保持原主模型和音频处理，增加指定隐藏层提取、专用 draft 加载、block verify 与拒绝后的 KV 回退；先验证 DFlash，再复用目标适配层评估 DSpark 的词表映射及附加 head。应先通过无 draft 输出、被拒绝路径、EOS／循环／token 上限和重复调用的验证，再计时。不能把小模型自回归投机当作 DFlash，也不能从 Python 调 C++ 子进程后称作 Python／MLX 实现。

私有计划、命令、完整输出、逐段 `review.md`、兼容性探针和摘要位于 `local-artifacts/evaluation/speculative-asr-20261115/`；下载与固定源码证据位于 `local-artifacts/research/speculative-asr-20261115/`。没有修改生产依赖、解码器、pipeline、模型默认或安装包，没有云端调用和素材上传。

## 第四阶段：Python 专用 head 适配（2026-11-15）

用户确认在隔离实验目录先适配 DFlash、通过后接 DSpark，并进一步确认保留 Python 路线，不因 native 小样本结果切换语言。实现位于 `experiments/online/qwen_speculative.py`，入口 `run_qwen_speculative.py`；没有修改生产 worker、安装包或已安装 ASR 包。复用原模型的音频处理、层、MRoPE 和 KVCache，仅封装进入指定层前的 taps，并加载固定的专用 head。

DFlash 复用 native 的 context-fusion／causal-block 组织；DSpark 参照 Speculators `05f9ba2324fef505cef8700669f2bdc20f8ae219` 的 reduced-vocabulary 映射、anchor sampling、vanilla Markov 与 confidence head。两者均由目标模型逐块验证，拒绝尾部从所有目标 KV 层回退，draft 只追加已提交前缀的隐藏状态；EOS、循环与长度预算单独停止，每请求新建缓存。confidence 原始 logits 实际计算、记录并检查有限性，本轮不使用阈值或自适应缩块。

DSpark 使用第三阶段固定 revision 的 754,200,578 bytes BF16 权重，SHA-256 `1b68cb103639861bb78aa99baa067bc8553197c0ba4281f836af83c80bbe378c`，下载与摘要已核验。DFlash 仍用同一固定包，未再量化。入口严格限定当前五个校正裁片及输入／音频摘要，固定 ASR 包 0.4.4，原接口与新封装无 draft 分别保留为控制，不拿新 ASR 输出替代校正原文。

适配门禁覆盖：原层 forward 等价、拒绝回退后与干净前缀一致、首个／部分／全部提议接受、EOS 在初次预测或块内出现、最后一个 token 的预算、循环与预算同时触发、词表映射加载／不一致拒绝、DSpark Markov／confidence 数值输出，以及非有限 draft 拒绝。五段真实裁片的新封装无 draft 也与原 Session 逐字一致。首次 DFlash 加载因额外 `t2d` 元数据失败，补齐校验后使用新目录重跑，旧失败保留。

首次计时中，将 DSpark head 随 target 一并转换为 FP16 后出现 NaN 和零接受率。定位发现权重转换本身均有限，目标 taps 也有限，但 draft 输入 FC 的 FP16 输出溢出到 Inf，后续 context KV／block 成为 NaN；这组耗时作废，不当成 DSpark 性能分数。最终保留 checkpoint 的 BF16 draft，进入 head 的 target 隐藏状态／embedding 显式转换为 BF16；target 仍按各自 FP16／BF16 条件运行。对 logits／confidence 增加有限性检查，失败明确中止，避免靠逐个目标 token 兜底后冒称成功投机解码。

### 最终 v2：同裁片、同条件的 Python 结果

执行私有 `run_benchmark_v2.py`，调用公开实验入口；M4／24 GiB、macOS 27.0.1，Python 3.11.16、ASR 包 0.4.4、NumPy 2.4.6。当前 runtime 条件为 MLX 0.32.4／target FP16，控制为 MLX 0.32.3／target BF16；两边 draft 均保持 BF16。每环境分别运行原 Session、新封装无 draft、DFlash、DSpark，各三轮，第二轮倒序，共 120 次；固定五个输入、日语、无 context、512 token、512,000,000 bytes allocator cache，每请求清 cache，无云端或 MT。

| target 条件／路径 | 后两轮五段总计算时间均值 | 最大单请求 MLX peak active | 与同条件无 draft 的文字差异 |
| --- | --- | --- | --- |
| 0.32.4 FP16，原 Session | 4.745 s | 2.829 GB | 0 / 5 |
| 0.32.4 FP16，新封装无 draft | 4.619 s | 2.829 GB | — |
| 0.32.4 FP16，DFlash BF16 | 2.870 s | 3.555 GB | 0 / 5 |
| 0.32.4 FP16，DSpark BF16 | 5.607 s | 3.583 GB | 0 / 5 |
| 0.32.3 BF16，原 Session | 4.724 s | 2.817 GB | 0 / 5 |
| 0.32.3 BF16，新封装无 draft | 4.575 s | 2.817 GB | — |
| 0.32.3 BF16，DFlash BF16 | 3.049 s | 3.542 GB | 0 / 5 |
| 0.32.3 BF16，DSpark BF16 | 5.685 s | 3.570 GB | 1 / 5 |

120 次均 EOS，未截断，draft logits／confidence 有限，目标与 draft 提交前缀长度一致；各臂内部三轮文本稳定。新封装无 draft 在两环境均逐字等于原 Session。DFlash 相对新封装无 draft 的总计算耗时减少约 37.9%／33.4%（约 1.61×／1.50×），额外峰值约 0.725 GB；没有观察到本样本文字偏离。两个无 draft 入口几个百分点的差异也说明小样本计时存在实现／系统波动，不能由此宣布另一种语言更快。

DSpark 在当前 checkpoint、固定整块和参考适配下反而多花约 21.4%／24.3% 时间。提交接受的提议／送验提议比例约 9.2%，DFlash 约 40%–42%；这些是该实验的算法计数，不是识别准确率。BF16 target 的 radio-11-020 出现词形变化，未获人工复核，不标为改善；FP16 target 的五段文字一致。没有运行 confidence 自适应截断、宽度遍历或 fused Markov kernel，因此不能据此判断所有 DSpark 实现和主模型搭配的上限。

数据来自新冻结的 `benchmark-v2/`，未筛掉其中的慢样本。第一次计时及首轮／失败尝试全部保留，不参与本表。v1 的 control-plain 首轮与一次约两秒合同回归重叠，也只作为历史；最终 v2 计时期间未并行运行测试。原始输入、权重、适配器／脚本摘要、命令、环境、逐次结果和 `review.md` 保存于忽略的 `local-artifacts/evaluation/python-speculative-asr-20261115/`。FP16 draft 溢出的真实拒绝另存 `expected-fp16-rejection/`，返回 FloatingPointError 而非成功结果。

新增四项测试（含多组接受／拒绝／停止边界）与现有实验回归合计 177 项通过，两套 cargo fmt 和 diff 检查通过。三轮同裁片的 allocator active 变化有记录，但这不是长时内存或并行 ASR＋MT 验收。本轮只支持继续评估 Python DFlash，生产迁移、实时音频前缀、取消和端到端上屏仍需各自验证。

### 布局含义与后置工程

`reference`／`voxt_swift` 是 native 音频输入兼容路径，不是两份主模型或 UI 布局。固定源码的 `qwen3_audio.cpp` 中 reference 去掉最后一个 centered STFT 帧，按 checkpoint processor 的整块／卷积输出计算音频 token；Swift 路径保留最后一帧，token 长度公式使用 float32 true division，并按旧公式保留 padded convolution 的行。它可能预留更多 audio placeholder；`BuildPrompt` 对超出 encoder 行数的位置保留 audio_pad embedding，不是自动补真实音频。因此输入内容与长度均可能不同，不能把它们当成单纯的内存排布开关或必然等价的优化。

这五段中 reference 略快，不能据此认定所有录音都更快或更准。本轮 Python 保留自己的 reference 音频处理，以减少与原接口比较的输入差异，不改变生产默认。oMLX 前缀复用、缓存观测和总内存预算按用户补充继续作为后续研究／验证项；先测已内置 llama-server 的命中与 prefill，再决定是否需要新的缓存机制，此轮不迁移服务或改变资源调度。

## 第五阶段：单路 Qwen＋DFlash 的持续预览验证（2026-11-15）

用户确认验证 `持续音频 → 单路 Qwen＋DFlash → 可修订原文 → 边界锁定 → MT`，生产默认先不替换。实现为实验入口 `replay_single_qwen.py` 和有界需求邮箱 `live_asr_schedule.py`，复用原 PCMInput／AudioRing、事件 revision 校验、StageAdmission、SenseVoice CPU 适配和 Hy-MT2 客户端；Qwen 的短音频处理与上一阶段共用 `qwen_speculative.transcribe`。

本轮先冻结结构，随后计时：每新增约一秒音频请求一次当前裁片的完整前缀，PCM 以 512 samples、1× 时钟到达，模型只收到已到达的快照；不复用上次文字前缀，不把尚未到达的完整裁片提前送入模型。一个 Qwen owning thread 串行处理，待处理 preview 只保留最新需求；裁片闭合后优先处理 final，锁定后拒绝迟到 preview。若同一完整音频已经计算或正在计算，则复用／升级这次结果，不再重复解码同一输入来“确认”。

五段裁片按既定顺序连续回放，但分别作为独立语义上下文，H／R／F 空；不会把节目之间及未核准间隙当作连续上下文。边界固定为已核准裁片 EOF，不声称验证了自然 VAD 或真实语义分组。MT 只翻译锁定后的整个裁片原文，使用现有 Hy-MT2 1.8B Q4_K_M、8192 context、单 slot；同 StageAdmission 禁止 ASR／MT GPU 重叠，已排队的 final MT 优先于后续 ASR。输入／分组与先前固定 G/H 文本测试不同，不能直接比较其中文来归因 R 或历史策略。

条件为现有 Python 3.11.16／MLX 0.32.4、Qwen 1.7B 8bit、target FP16／DFlash BF16、512 token、512 MB allocator cache 上限，请求间清 cache。比较单路无 MT、单路＋MT、受控双模型＋MT 三臂，双模型复用当前 SenseVoice CPU rolling preview＋Qwen final；它冻结相同裁片边界与翻译策略，不冒称完整生产 worker／App 的 A/B。每臂两轮，第二轮反向执行臂顺序；五个裁片顺序保持，拒绝扩大名单。

先报告首次非空原文、返回结果所依据音频的年龄、原文锁定／MT 完成距裁片末尾的时间、文字回改、计算／排队及 allocator 观测。首次非空不等于首次正确；文字与 final 相同也不等于人工确认正确。最大文本变化间隔不等于模型刷新耗时，静默重复同文不强制制造 UI 更新。不从五个挑选裁片计算总体准确率、CER、自然语音 SLO 或长时内存稳定性。

命令、计划、源码快照、完整事件及 cleanup 位于忽略的 `local-artifacts/evaluation/live-single-qwen-20261115/`。回放仅写私有产物，没有系统采集、原生 paint、云端、录音保存或正式字幕工作区写回。oMLX 前缀／内存能力继续后置研究，本轮没有引入磁盘 KV 缓存或另一个本地服务。

### 结果与判断

三臂各两轮均正常结束，每轮 44.558 秒音频、42 次 preview 与 5 次 final。共 30 条原文定稿、20 条最终译文，无空定稿、线程遗留、需求覆盖或锁定后原文回写。逐请求审计通过输入因果性、同模型／同裁片／同音频长度无重复调用、完整输入只解码一次、译文 revision 与原文匹配；四个自建 MT server 均已退出。完整输入的结果复用／在途升级分支有单元验证，但本轮裁片长度未触发，不能当作真实回放覆盖。

下表首次原文／定稿／译文为两轮十个裁片观测的中位数；Qwen 计算为每轮五段合计的两轮均值。首次原文从裁片开始计，后两项从裁片结束计，不含模型加载与原生绘制。

| 条件 | 首次非空原文 | 原文锁定距 EOF | 最终译文距 EOF | Qwen 累计计算／轮 | Qwen allocator peak |
| --- | ---: | ---: | ---: | ---: | ---: |
| 单路 DFlash，无 MT | 1.232 s | 0.858 s | — | 21.060 s | 3.535 GB |
| 单路 DFlash＋MT | 1.601 s | 0.852 s | 1.415 s | 20.837 s | 3.535 GB |
| SenseVoice preview＋Qwen final＋MT | 1.090 s | 1.040 s | 1.425 s | 4.897 s | 2.829 GB |

单路＋MT 的首次非空原文范围 1.191–2.075 s，对照 1.074–1.122 s；最终译文距 EOF 分别为 0.571–1.972 s、0.669–2.500 s。单路无 MT 的首次原文中位数 1.232 s，说明串行 MT 在裁片接续处影响后续预览；没有用重叠 GPU 推理掩盖这一竞争。对照另有每轮约 5.338 s 的 SenseVoice CPU 计算；表中 Qwen allocator peak 不包括 ONNX、另一 MT 进程或整机统一内存占用。

六轮的 30 条原文定稿均与上一阶段整裁片 Qwen／DFlash 输出一致，四轮 MT 的每段中文也相同。原有「信量」「今北にとなぶな」及「オイラン」等误识别／词形仍在，不因加速获得语义修复。每轮原文回改后缀字符合计为单路 483、对照 857，包含空格、标点及两模型切换，不能解释成语义错误率或准确率改善。逐条人工语义原文、预览时间线、原文定稿与中文保存在私有 `review.md`；`aggregate.json` 和 `audit.json` 保存聚合及证据核对。

**单路可作为后继候选，但尚不足以替换默认。** 本批最长 13.926 s 的裁片在 1× 回放下没有积压到覆盖 preview；相比对照，首次出字较晚，最终译文完成时间基本相当。反复编码／prefill／解码让 Qwen 累计计算约为 final-only 对照的 4.25 倍，DFlash 的单请求加速没有消除窗口重算。后继应先讨论音频窗口、预览刷新与锁定规则如何限制重算，再冻结验证；不能直接套用生成文字前缀缓存而锁住误听。本轮没有验证自然 VAD／语义分组、预译、长至 30 秒窗口、长时内存、失败恢复或模型调用中的取消，生产 SenseVoice／Qwen 与 MT 策略保持。

实现验证：生产 Python 环境执行 `python3 -m unittest discover -s experiments/online -p 'test_*.py'` 共 182 项通过，包含新增的需求合并、EOF 升级／复用、终止唤醒和指标合同；根目录及 Tauri 的 `cargo fmt --check`、`git diff --check` 通过。实验基于 `fad236e`，运行前保存参与源码快照与摘要，运行后逐一一致；没有重打包／安装 App。

## 已停用的历史入口

旧素材角色、CER／WER 方法、ASR 实验、未校正模型参照、离线诊断、runtime 候选和时钟／资源验证均见[历史归档](archive/2026-11-14/evaluation-before-confirmed-text.md)。它们只解释当时工作，不是当前待跑任务；恢复旧实验需要重新明确范围，不能通过旧状态自动混入当前结果。

<a id="index同版本q4与f16精度路径基线2026-11-09"></a>

<a id="qwen原生context词表与提示复述检查2026-11-09"></a>

<a id="qwen排队区间与在途mt阻塞审计2026-11-09"></a>

<a id="qwen提示复述的有限恢复与固定mt对照2026-11-09"></a>

<a id="下一版任务交错与一对并发容量对照2026-11-11"></a>

<a id="下一项验证需要先确定的候选合同提案未实施"></a>

<a id="两个失败请求的状态与提示隔离排查2026-11-11"></a>

<a id="停用歧义平假名别名2026-11-09"></a>

<a id="历史选择的固定请求与连续确认2026-11-09"></a>

<a id="原文时钟分组的调度不变性参考2026-11-09"></a>

<a id="参考证据台账与当前链路基线2026-11-08"></a>

<a id="可见草稿与定稿退化检查2026-11-09"></a>

<a id="同模型离线诊断与在线材料导出2026-11-14"></a>

<a id="固定分组的真实mt与后验词形诊断2026-11-09"></a>

<a id="固定源钟的草稿预算回放与结构分析优先2026-11-09"></a>

<a id="在线mt边界与失败隔离候选2026-11-08"></a>

<a id="定稿后文策略的扩大对照与反例2026-11-09"></a>

<a id="接续语等待与带id的联合输出实验2026-11-09"></a>

<a id="整套产品对比方向2026-11-14尚未运行"></a>

<a id="日语电台数据集扩充2026-11-08"></a>

<a id="本地mt单请求取消能力验证2026-11-09"></a>

<a id="本地下载后的候选调整2026-11-08"></a>

<a id="消息角色与历史译文传播的受控检查2026-11-09"></a>

<a id="烧录字幕候选与重复复核2026-11-08"></a>

<a id="离线与在线词表输入审计2026-11-09"></a>

<a id="语义筛选复核与保留片段确认2026-11-09"></a>

<a id="首轮素材筛选2026-11-08"></a>
