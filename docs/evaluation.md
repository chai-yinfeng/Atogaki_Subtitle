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

### 原文锁定到译文完成的配对补算（2026-11-15）

按用户要求，仅分析上述既有事件，不发起新推理。逐条用 `segment_id` 和 `source_revision` 配对，计算 `translation_final.emission_s - source_final.emission_s` 后聚合；不能相减两个 EOF 延迟中位数来代替配对差值。每臂十条观测，明细与原事件摘要保存于私有 `lock-to-translation.json`。

| 条件 | 锁定到译文完成，中位数 | 同项范围 | MT 调用，中位数 | MT 排队，中位数 |
| --- | ---: | ---: | ---: | ---: |
| 单路 DFlash＋MT | 0.417 s | 0.254–0.814 s | 0.393 s | 0.026 s |
| SenseVoice preview＋Qwen final＋MT | 0.384 s | 0.235–0.800 s | 0.372 s | 0.012 s |

MT 调用计时包括本地 HTTP、server prefill／生成及客户端解析校验，不是纯 GPU kernel 时间；各列分别取中位数，不能严格相加。当前客户端 `stream=False`，只能得到完整译文，没有首 token／首个正确译文的计时。本轮是短裁片、空 H/R、每个 final 翻译一次；没有测预译重试或更长历史，不能据此断言生产链路由 MT 或 ASR 单独主导。自然边界等待也不包含在“锁定后”区间内。

只读核对还发现，已安装 `mlx-qwen3-asr==0.4.4` 的 `streaming.py` 已有 `_WindowPrefixCache`／`reuse_window_prefix`，`encoder.py` 对齐模型 `n_window_infer=800`（约 8 秒）块对角注意力。双向依赖存在于块内，不意味着整个未锁定语义群都必须重新编码：完整块且前处理输入不变时，可复用 encoder 输出及相应音频前缀的 decoder KV；部分尾块仍重算，音频之后的生成文字不能因此沿用旧 KV。centered STFT 需要额外尾部样本，窗口全局 log-mel 最大值变化会使旧块缓存失效；浮点归约次序也须核对。机制见[包上游 Streaming 说明](https://github.com/moona3k/mlx-qwen3-asr#streaming)，本机源码已交叉检查，不把上游 benchmark 当成本机结果。

第五阶段入口没有调用该 streaming 路径，专用 DFlash decoder 每请求新建 target／draft cache；因此 4.25 倍是当前实现的实测成本，不能作为架构不可优化的下界。后继可先研究复用已有的音频块缓存，保持文字可回改，再单独评估文字前缀强制保留；两者不能绑定为一个无损优化。尚未适配或实测缓存＋DFlash，不更新生产默认，也没有实现云端在线 MT。用户提出的 API key 方向仍在讨论，未授权调用或上传材料。

## 第六阶段：完整音频块与 DFlash KV 缓存（2026-11-15）

用户在理解机制后要求继续缓存优化。范围为同五个裁片、同一主模型／DFlash checkpoint、计算精度和约一秒刷新；不改变生产默认、自然分组、MT、GPU 互斥或模型输入。`qwen_prefix_cache.CachedTranscriber` 直接复用已安装包的 `_encode_with_prefix_cache`，保留其 complete-block／centered STFT／mel-max 失效条件。新音频到来时只编码尚未缓存的完整块和部分尾块，完整块以前的 prompt/audio KV 因因果性保持有效；文字仍从头生成，随全部已到达音频修订。

`qwen_speculative.decode` 可从同边界的 target／draft KV 快照恢复。新 prefill 只提取未缓存位置的指定层进入激活，经 checkpoint 的 FC／norm／K/V 投影加入 DFlash context；保留完整音频位置以前的两套 KV，丢弃部分尾块和全部生成文字位置。无需跨请求保存完整隐藏层张量；候选拒绝回退仍只提交经 target 验证的 token。每个裁片重置，输入须为同一音频的增长前缀，最长 30 秒；不同输入／裁片不能沿用状态。

双向注意力的块内尾部依赖并未消失，也未缩小 8 秒块或改成因果 mask。全局 log-mel 截断阈值变化时保守重建旧块，而非用固定阈值改变 checkpoint 输入；前端仅占很少时间，先保持计算合同。此缓存只覆盖音频，和强制旧文字前缀是两项不同的质量取舍。原权重不变允许 runtime 优化，并不意味着任意改动都保持模型结果；本轮逐请求检查 token 和结束原因，不将文本一致当作总体正确。

先固定第五阶段无 MT 回放的 47 个实际前缀，按 plain／cached／cached／plain 顺序在同进程串行运行，完整比较文字、token、结束原因、target／draft offsets。最终计量 188 次一致，缓存每轮命中 9 次、全局 mel 改变后失效 1 次；平均累计耗时 20.118 → 18.423 s，减少约 8.4%。阶段计时显式求值并同步，encoder 平均约 1.947 → 1.578 s，prefill 6.341 → 5.143 s，生成 11.624 → 11.523 s；不能预期此优化消除重复生成文字的成本。allocator peak 约 3.536 → 3.514 GB，不包含 MT 或整机内存，也不是长时证明。

第一次计量的 generation 计时变量被 token 计数覆盖，且仅同步没有触发 features／encoder 惰性图求值，阶段占比无效。原数据和限制记录保留于 `prefix-attempt01`，不用其阶段数值推导结论；修正并增加计时合同回归后在新目录 `prefix-attempt02` 重跑，未覆盖失败证据。总耗时与逐 token 比较未受计时变量错误影响，最终结果统一使用第二次。

命令、源码快照、环境／输入／模型配置摘要和聚合位于忽略的 `local-artifacts/evaluation/dflash-prefix-cache-20261115/`。后继 1× 回放冻结缓存关／开 × MT 关／开四臂，各两轮、第二轮反向执行；和第五阶段一样使用独立语义上下文及已知裁片 EOF，只在原文锁定后翻译一次，不运行云端或新语料。

八轮 1× 回放全部完成，每轮 47 次 ASR，40 条原文定稿和 20 条译文完整，无错误、遗留线程、preview 需求覆盖或锁定后回写。所有原文事件均匹配固定前缀审计及第五阶段原实现的文字，中文也与原对照相同；逐事件检查输入因果性、revision、同输入不重复执行、阶段计时分解和 ASR／MT 互斥，四个实验 server 均已回收。每个缓存轮次仍为 9 次命中、1 次失效。

| 条件 | Qwen 累计计算／轮，均值 | 首次原文，中位数 | 锁定距 EOF，中位数 | 译文距 EOF，中位数 | 锁定到译文，中位数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 缓存关，无 MT | 20.722 s | 1.232 s | 0.812 s | — | — |
| 缓存开，无 MT | 19.819 s | 1.234 s | 0.671 s | — | — |
| 缓存关＋MT | 21.171 s | 1.600 s | 0.848 s | 1.411 s | 0.400 s |
| 缓存开＋MT | 19.764 s | 1.508 s | 0.668 s | 1.206 s | 0.394 s |

1× 回放的 Qwen 累计计算分别减少约 4.4%／6.6%；固定前缀无时钟等待计量为 8.4%，不混作一个百分比。带 MT 的调用中位数 0.379 → 0.377 s、排队 0.026 → 0.024 s，改善主要来自源文锁定更早，不能声称优化了翻译 kernel。所有延迟中位数来自两轮十条配对样本，分开取中位数的列不能相加。allocator peak 约 3.536 → 3.514 GB，仅为 Qwen 进程；观察没有峰值增长，不推导长时总内存安全。

结论限于本批最长 13.926 s 的裁片：缓存有效且保留完整文字回改，未消除块内尾部和重复文字生成的成本。固定前缀中的生成阶段仍约 11.5 s；这些累计工作量不等于单条字幕的关键路径。生产晋升还缺自然边界、长窗口、长时资源及失败恢复验证，GPU overlap／batching 和文字前缀保留也没有随缓存自动开启。无需为本轮另起 oMLX 服务。

验证：生产 Python／Metal 环境执行在线 `test_*.py` 共 185 项通过，新增覆盖可变尾部的 target／draft KV 一致、音频替换／缩短拒绝、全局 mel 变化联动失效、裁片重置和阶段计时合同。根目录及 Tauri fmt、diff 与相对文件引用检查通过；没有构建／安装 App，没有生产模块导入新缓存入口。

## 第七阶段：刷新、文字锁定与边界影响（2026-11-15）

用户补充理想延迟在 2 秒以内，要求先实测刷新率／边界／锁定的分别影响，再讨论云端校对、整理和翻译。后继明确目标为**说话期间持续看到 2 秒内的可修订译文**，不是原文预览或 EOF 后返回。仍只使用既有五个用户校正文本绑定裁片，主模型／DFlash／完整音频缓存及本地 MT 不变。首次非空原文、返回时输入年龄、锁定距 EOF 和译文距 EOF 分别记录，不把首 token、刷新周期或与 final 同文当成首次语义正确译文；既有语义校正不是逐字／词级时间 gold，不能据此宣称连续字幕 2 秒 SLO 达成。

刷新测试冻结 0.5／1／1.5／2 秒四臂；边界仍为相同 EOF，文字可完整回改，final 不等下一刷新时刻。单独增加 1 秒刷新＋前缀末尾 5 tokens 可改的锁定臂：参考已装包／Voxt，先完成两次解码，再把此前文字前缀放入下一次 prompt；前缀长度不回退，最终识别不额外解锁重跑。只复用音频位置的 KV，强制文字前缀在每次 prompt tail 重新 prefill；保留总 512 token 预算，候选拒绝与数值失败规则不变。该臂可能固化误听，不设置为默认。五臂各两轮，第二轮反序，保持相同输入时钟、final-only 整裁片 MT、空 H/R/F 和 GPU 任务级互斥，不混入并发／batching。

边界先复用生产 sherpa-onnx Silero 参数：threshold 0.5、最短静音 0.5 s、最短发声 0.25 s、最长发声 12 s、512 samples 输入。五段中只有 `radio-11-018` 有内部闭合，音频切点 9.740 s，检测在 10.272 s 才报告；其余依赖输入 EOF。不能据此发明新的已校准语义边界。单独比较完整裁片与按该内部切点拆分的 ASR，按连续范围保留全部样本和静音，不沿用 VAD 的首尾裁剪；MT 始终翻译整个拼接原文一次。该项只计隔离计算与文本差异，既不是自然连续回放 SLO，也不与刷新臂合并算质量分数。人工语义原文保持，不被新 ASR 覆盖。

方案、命令、源快照、环境、输入／VAD／模型摘要、事件和复核材料位于忽略的 `local-artifacts/evaluation/preview-policy-20261115/`，基于 `c95f69f`。本轮不访问个人云端服务或上传材料。

### 实测与限制

五臂各两轮均完成，共 50 条原文定稿和 50 条最终译文，无错误、遗留线程、重复同输入调用或锁定后回写；因果输入、revision、token 总预算及源码摘要审计通过。四种刷新率最终文字、tokens、结束原因及中文都与基线一致。前缀锁定两轮均改变同三段最终原文，完整源／译差异保留 `review.md`，没有将文字差异直接计作人工语义错误率。

| 条件 | Qwen 累计计算／轮，均值 | 首次非空原文，中位数／最大 | 首次原文超过 2 s，观测条数 | 原文锁定距 EOF，中位数 | 译文完成距 EOF，中位数 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 s 基线 | 20.115 s | 1.447／1.760 s | 0／10 | 0.695 s | 1.256 s |
| 0.5 s 刷新 | 32.327 s | 1.270／2.199 s | 2／10 | 0.660 s | 1.075 s |
| 1.5 s 刷新 | 14.876 s | 1.802／2.043 s | 2／10 | 0.854 s | 1.273 s |
| 2 s 刷新 | 11.969 s | 2.278／2.469 s | 10／10 | 0.604 s | 1.008 s |
| 1 s＋前缀末尾 5 tokens 可改 | 14.065 s | 1.227／1.452 s | 0／10 | 0.358 s | 0.861 s |

刷新四臂每轮识别次数分别为 47／83／33／25，0.5 s 两轮各出现 7 次待处理 preview 被更新版本覆盖，其余为 0；EOF 仍有独立优先定稿。更频繁请求没有保证更小最大延迟；更慢刷新虽节省计算，2 s 周期已不足以满足本批首次原文 2 s 理想。1 s 基线暂作后继隔离实验的参照，不因这十个小样本断言整机或连续正确译文 SLO 达成。所有臂锁定到译文完成的中位数约 0.40–0.47 s；前缀臂并未让 MT 本身变快，其源文与译文内容也不同。

前缀锁定的生成阶段累计计算约 11.851 → 5.076 s，prefill 约 5.514 → 6.107 s，总计算约降 30%。代价是后续音频不能修订已经强制保留的词形；受影响 ID 为 `radio-01-012`／`radio-11-008`／`radio-11-018`，其中可见开头连接词被改变，仍需结合人工语义原文和音频复核。没有开启生产文字锁定，也未以额外完整 final 识别掩盖该方案的质量差异。

边界隔离对照按 whole／split／split／whole 执行，22 次 ASR 均 EOS，覆盖全部原音频样本，完整窗口结果与 1 s 基线一致。五段每轮总计算均值 2.868／2.958 s，变化较小且两轮范围交叠，未见明确计算收益。唯一改变的 `radio-11-018` 在两次 split 中得到相同新原文，中文也改变，故分段不能当作只影响耗时的开关。每个独特的整裁片原文只调用一次本地 MT，共 6 次；保持整组 MT 范围，因此没有测试提前切开 MT／更早交付译文的收益。VAD 检测时刻不是本次孤立解码的真实端到端时间，不从该项计算 2 秒通过率。

完整在线回归 186 项、根目录与 Tauri fmt、diff 和相对文件引用检查通过。10 个回放 MT server 及 1 个边界实验 server 均回收。没有安装 App、生产路径迁移或调用云端；首次语义正确译文、自然连续节目与长时资源仍待验证。

用户明确中文预览目标后，本批 final-only MT 只作为三类策略的诊断，不能选出满足真实目标的产品方案。后继应复用已有 Retranslation 可修订预译链路，观察连续中文内容、队列／GPU 竞争和正确后退化；保留 1 秒仅作为当前 ASR 参照。云端“G 锁定后一次请求”可以服务最终交付，不能单独保证长句期间已有中文预览。保留本地预译＋云端最终译文，或允许受控云端预译更新，是尚未确定的两个选项，不在本轮直接新增混合策略。

### 云端能力调查与待测合同

Typeless 的[官方说明](https://www.typeless.com/data-controls)明确音频及有限应用上下文在云端处理，并使用第三方 LLM；没有公开足以复现其完整链路或保证本项目 2 秒延迟的数据。Voxt 固定发行源码的 [Translate／Rewrite](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/session/write.go)把清理指令并入 compose 请求，经语言模型流式返回；[听写清理](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/session/cleanup.go)也按片发送并逐 delta 展示。普通交付依赖用户停止边界，其停止后耗时不能当作内容发生到连续字幕可读的延迟。未运行 Voxt App／Typeless，也未引用宣传作为本机实测。

可先评估本地 ASR → 锁定 G＋短 H／词表 → 一次云端请求做谨慎校对兼翻译 → 流式中文；不先串联多个改写／翻译请求，也不因每次 ASR 更新重发。原始识别保留，语言整理不能静默覆盖来源；文本模型可利用上下文修复常见误词，但无法可靠恢复未输入的声学证据。和当前 Hy-MT2／Index 的可比测试仍需固定同一 G/H 及实际 prompt，分别测网络／排队、首内容、首个可读中文片段、完成时间、语义及实际 token 费用，暂不提供云端时延或质量承诺。

以上一次请求合同适用于最终交付；对已明确的持续可修订中文目标，还需另确定预译来源和触发规则。普通文本 SSE 逐步返回同一次请求的结果，不会自动接收后来新增原文或改写旧译文；根据新内容修订通常需要新 revision 请求，或另用支持连续输入的专用接口。不能同时默认“始终等整组锁定”“长句期间 2 秒内有中文”“绝不重发”；应在具体预译规则下实测质量和请求费用。

[DeepSeek](https://api-docs.deepseek.com/api/create-chat-completion/)支持 SSE 流式响应及思考模式开关；首 token 可早于整句完成，响应模式本身不证明延迟更低。应选择并记录适合延迟目标的模型／思考配置，不把默认行为当固定性能。Gemini 另有[音频理解](https://ai.google.dev/gemini-api/docs/audio)和[Live 音频翻译](https://ai.google.dev/gemini-api/docs/live-api/live-translate)路线；后者可连续接收音频并输出翻译语音／转写，与纯文本 MT 的计算、上传和计费边界不同，需作为独立候选，不在本轮接入或计量。

## 第八阶段：本地 DFlash 预译链路与 App 试用（2026-11-16）

用户授权暂缓云端，固定本地配置并编译安装。按[0080](decisions/0080-local-dflash-app-trial.md)，原实验 decoder／音频缓存迁入共享 `online_worker`，实验只作导入桥；新 `speculative_asr` 负责一条 Qwen owning thread、VAD 回退重置和同完整输入复用。显式 `--speculative-model` 才选择单路模式，桌面仅在日语／1.7B 8bit／完整受管 DFlash 包时传参。没有从源码自动下载、会话中静默降级或启用文字强制前缀。模型目录固定两文件的 revision／大小／SHA，App 打包共享模块。

本轮首次把加速预览送入真实 Retranslation，而非 EOF 后只翻译一次。只使用同五个已核准裁片，每段独立上下文、1× PCM，真实生产 VAD／pre-roll；旧 SenseVoice preview＋Qwen final 与单路 Qwen＋DFlash 各两轮，第二轮反序。两者均用 Hy-MT2 1.8B Q4_K_M、8192 context、单 slot、预译开启、空场景词表、既有确认原文 H／当前 G／已到达可选 R，维持 GPU 互斥。不同于固定校正 G/H、R 为空的 MT 忠实度对照，不混入该项的质量评分。

20 次回放全部 completed，MT 拒绝／失败和 worker error 均为 0；两种模式的 6 个 VAD 定稿范围和文字在两轮都相同，最终译文逐项绑定确认 revision，无迟到原文覆盖。新模式共 84 次原文预览、80 次中文预览、12 条原文定稿和 12 条最终译文；旧路径为 88／84／12／12。四个实验 server 已退出。当前命令、源码快照、输入／主模型／draft 摘要、原始事件和审计保存在忽略的 `local-artifacts/evaluation/local-dflash-app-20261116/worker-attempt01/`。

| 指标 | 原双模型 | 单路 DFlash＋音频缓存 |
| --- | ---: | ---: |
| 首次原文，中位数／最大 | 0.644／0.705 s | 0.768／1.015 s |
| 首次非空中文，中位数／最大 | 0.760／0.913 s | 0.884／1.145 s |
| 中文可见文本变化间隔，中位数／最大 | 1.070／1.912 s | 1.093／1.556 s |
| 原文定稿距对应音频末尾，中位数／最大 | 1.065／1.582 s | 0.577／1.175 s |
| 最终译文距请求音频末尾，中位数／最大 | 1.434／2.196 s | 0.924／2.272 s |

首次指标有两轮十个裁片观测，final 有十二个 VAD 段／译组；不是相同条目的字词级时钟。首次非空可能很短，文字变化间隔也受内容重复影响；请求音频范围可能先于／晚于语义所需内容，不能当作持续正确中文的滞后。新模式并非所有指标更快，更不是质量分数改善；它避免 CPU 草稿到 Qwen 定稿的模型切换，在本批保持最终原文一致并缩短中位定稿尾部，是否改善阅读由本机试用确认。Qwen allocator peak 约 3.508 GB，旧路径未计相同 allocator 指标，不提供错误的 0 GB 对照或长时内存保证。

189 项 Python（含同坐标复用、较短 VAD final 清缓存和 final／preview 取序）、4 项 Rust owner／参数／停止测试、5 项 Tauri 目录、372 项前端、TypeScript／生产构建和 Tauri offline check 通过。已从 `57f1fbd` 构建并安装，40 文件／共享资源／sidecar／deep strict 签名一致，安装包中的模块实际加载受管 DFlash 模型通过；原生面板配置就绪，未开始采集。数据库 20 表在替换前后和启动后均一致，旧 App 与一致性数据库备份保留。见[本机安装文档](local-editor-installation.md#2026-11-16单路-qwendflash-本地预译试用)；不以回放或就绪界面代替真实捕获／原生上屏或长期质量验收。

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
