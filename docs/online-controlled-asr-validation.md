# 受控 WS／SS／CPP 在线 ASR 验证

日期：2026-10-05。本轮只运行 ASR；MT、GUI、正式 offline 工作流未改变。下列结果为本机系统配置比较，参考核验未闭环时不宣布生产胜者。

## 复现结构与比较边界

本机 Apple M4、24 GiB RAM、10 CPU cores、arm64 macOS 27.0（26A428）。全部 ASR 串行执行，mono 16kHz PCM、日语 transcription、greedy／beam=1、temperature=0、无 fallback、空 initial prompt。模型加载与固定零音频 warmup 在 replay clock 外，分别记录。实验事件为独立 v3；旧 large-v3 v2 数据和适配探针不参与本轮排名。

small／base／tiny 从官方 multilingual checkpoint 转换，源摘要、alignment heads、转换工具／资产 revision 与产物摘要保存在各模型 provenance.json。WS 使用 MLX FP16／Metal；SS 使用 torch FP16／MPS，在 hooks 和 context 初始化前配置设备，CPU 明确负责 float32 STFT／mel；不启用隐式模型 CPU fallback。CPP 使用 GGML F16／Metal，converter 指定的小张量保留 FP32。GPU 调用计时包含完成同步，kernel 累积精度仍有 runtime 差异。

WS 保留 LocalAgreement-2 和原生上下文，读未确认 buffer 作为真实 draft。SS 保留 AlignAtt／frame_threshold=25，不启用 CIF，只评价实际暴露的 commit。CPP 是官方 stream 窗口策略的无丢包文件时钟 adapter，length=5s／keep=.2s／4 threads／context disabled；不是原封不动的 SDL capture 二进制。它的窗口 freeze 与 WS／SS provider commit 分列，不对漏词、跨窗重复做事后修复。

producer 每32ms供给，解码间隔500ms／1s独立，单个 consumer 忙碌时合并已到达输入，不排队补跑过期触发。VAD pause 保留原 PCM 和媒体坐标，普通停顿不默认 finish/reset。Silero threshold=.5／negative=.35／pre-roll=.2s／silence=.5s；cached 概率用于因果受控对照，live CPU worker 的计算／IPC／争用计入真实时钟。SS 原生 finish 本身清除 segment/context，因此其 finish 消融包含原生状态变化。

## 样本和参考状态

Development 为《湖吉の庭 Vol.1》四个60s窗口及完整268.534s节目。参数冻结后提取《後書きラジオ》120–300s holdout；既有首分钟不作为新独立证据。长回归为既有直播首10分钟。另保存纯静音、15s长停顿、轻声起声、句中截断 EOF 和既有重复片段的冻结 fixture／摘要／变换。

按用户授权，四段共240s的 Gemini 3.5 Transcribe 已生成；四个 Files 临时文件均删除，request／response／授权及删除证据保留在本地。它们属于 model_reference，provider word timestamps 尚未声学校准。20个短语保留独立的 semantic／verbatim／timing 状态；没有把模型一致或文字更流畅称为 gold。

短语锚点是从60s转录中抽出的定位片段，不是模型只识别约2.5s。原听审音频多留的边缘词曾让短语和播放范围不一致；用户备注保留在原 review 文件。后续核验提供连续节目较长上下文，以完整节目作主质量证据；冷启动切窗、跨 Gemini 分段边界和句中 EOF 另评边界行为。不确定锚点可显式排除，未输出的已核准锚点继续留在覆盖率分母。

有效显示 p50≤1s／p95≤2s 和≥95%覆盖必须使用经核准的文字／语义及声学末尾。下表的输入处理落后、decode 和 model-reference 精确匹配是诊断，不能替代这一验收。人工语义变体与逐字匹配分别计算；语义批准不会批准原来的错误模型短语。没有核验的 CER/WER、关键语义胜负和正式有效延迟保持未判定。

## 结果

筛选为 dev-0 单次回放，完整节目按轮换顺序重复三次；配置固定为 small／500ms／WS trimming15s／SS frame_threshold25／CPP length5s。尚未核准有效延迟，所以没有启动只在达标后进行的有限参数优化。base／tiny 提供下探诊断，不作为已经通过长样本质量验收的部署配置。

| 模型 | pipeline | 间隔 ms | 状态 | decode p95 s | 输入落后 p95 s |
| --- | --- | --- | --- | --- | --- |
| small | CPP | 500 | completed | 0.309 | 0.328 |
| small | SS | 500 | completed | 1.439 | 1.607 |
| small | WS | 500 | completed | 1.114 | 1.138 |
| small | CPP | 1000 | completed | 0.312 | 0.342 |
| small | SS | 1000 | failed_or_cancelled | — | — |
| small | WS | 1000 | completed | 1.884 | 1.907 |
| base | CPP | 500 | completed | 0.198 | 0.246 |
| base | SS | 500 | completed | 0.855 | 0.927 |
| base | WS | 500 | completed | 1.557 | 1.562 |
| base | CPP | 1000 | completed | 0.216 | 0.295 |
| base | SS | 1000 | completed | 0.890 | 0.993 |
| base | WS | 1000 | completed | 1.492 | 1.519 |
| tiny | CPP | 500 | completed | 0.411 | 0.435 |
| tiny | SS | 500 | completed | 0.590 | 0.607 |
| tiny | WS | 500 | completed | 1.436 | 1.450 |
| tiny | CPP | 1000 | completed | 0.154 | 0.186 |
| tiny | SS | 1000 | completed | 0.839 | 0.960 |
| tiny | WS | 1000 | completed | 1.468 | 1.486 |

三轮完整节目（268.534s），失败不从分母删除：

| 运行 | 状态 | 输入落后 p95 s | decode p95 s | 撤回字符 | 修订次数 | EOF s |
| --- | --- | --- | --- | --- | --- | --- |
| full-r0-simulstreaming | completed | 16.707 | 14.799 | 0 | 0 | 13.500 |
| full-r0-whisper-cpp | completed | 0.351 | 0.324 | 1551 | 167 | 0.000 |
| full-r0-whisper-streaming | completed | 0.958 | 0.930 | 373 | 52 | 0.000 |
| full-r1-simulstreaming | completed | 15.490 | 12.617 | 0 | 0 | 7.503 |
| full-r1-whisper-cpp | completed | 0.340 | 0.318 | 1184 | 186 | 0.000 |
| full-r1-whisper-streaming | completed | 0.907 | 0.888 | 388 | 52 | 0.000 |
| full-r2-simulstreaming | failed_or_cancelled | — | — | — | — | — |
| full-r2-whisper-cpp | completed | 0.338 | 0.317 | 1277 | 177 | 0.000 |
| full-r2-whisper-streaming | completed | 0.877 | 0.850 | 406 | 51 | 0.000 |

SS 的零撤回只描述 append-only 接口；CPP 的撤回是当前窗口内显示变化，freeze 之后的词也可能遗漏／重复。首分钟筛选中 SS small／1s 达到预算退出；完整第三轮的 SS 在 Python Unicode grouping 阶段越界。没有证据把该异常归为 MPS 算子不兼容，也没有打开 CPU fallback 或中途重写 decoder。

十分钟持续回放；积压差为最后2分钟与第2–3分钟的输入落后中位数之差，≤1s、无丢包、无失败才通过。RSS 列是同一时间范围的 runner current RSS 中位数之差：

| pipeline | 状态 | 积压差 s | 积压增长条件通过 | runner RSS 差 MiB | child RSS 差 MiB | RSS样本数 |
| --- | --- | --- | --- | --- | --- | --- |
| CPP/live | completed | -181.030 | True | -35.438 | -65.578,300.750 | 20 |
| WS/live | completed | -164.890 | True | -183.438 | -13.594,-191.125 | 21 |
| SS/off | completed | 2.884 | False | 1217.172 | — | 21 |
| CPP/off | completed | 0.030 | True | 0.281 | 2.219 | 20 |
| WS/off | completed | 0.324 | True | -720.484 | -8.719 | 20 |

live VAD 长片头暴露 runner 调度缺陷：静音期间 PCM 保留在队列，恢复时每次最多30s追赶旧音频。WS 首次 VAD start 为299.616s，首次处理到该坐标为352.692s，差53.076s；CPP 差4.720s。二者负积压增长只说明旧队列被追赶，不代表恢复实时可用；新增独立2s输入恢复诊断阻止将其选为部署胜者。这不是正确文本延迟，也不能将该缺陷归为算法本身的速度。未通过恢复诊断的 pause 配置不应作为默认。原音频及失败输出完整保留，没有填零、跳音频或重置后重跑来改善排名。

SS preprocessing 单列；WS／CPP 的预处理包含在 native inference service 内，未隔离测量，原 metadata 的0是占位值，分析器显示 null，不称为零开销。

RSS 每30s采样，child RSS、MLX peak、MPS allocator 终点与进程 peak 分开保存；不是精确系统／GPU 总峰值。采样在第一轮完整节目结束后加入，未采到的早期轮次不会补造内存曲线。

| 冻结 holdout | 状态 | 输入落后 p95 s | EOF s |
| --- | --- | --- | --- |
| SS | completed | 98.298 | 19.232 |
| CPP | completed | 0.556 | 0.000 |
| WS | completed | 2.066 | 0.000 |

holdout 只按冻结配置运行一次，没有用它改参；语义审核尚未完成。

## VAD 与边界行为

| 策略／pipeline | 状态 | 调用数 | decode合计 s | 输入落后 p95 s | live VAD CPU s | EOF s |
| --- | --- | --- | --- | --- | --- | --- |
| ablation-finish | completed | 112 | 36.326 | 0.857 | 0.000 | 0.332 |
| ablation-mask | completed | 53 | 35.612 | 1.771 | 0.000 | 0.647 |
| ablation-reset | completed | 110 | 36.518 | 0.881 | 0.000 | 0.285 |
| ablation-silence250 | completed | 46 | 32.181 | 1.519 | 0.000 | 0.643 |
| ablation-silence800 | completed | 54 | 37.243 | 1.786 | 0.000 | 0.646 |
| ablation-tail | completed | 70 | 40.588 | 1.740 | 0.000 | 0.663 |
| pause-cached-simulstreaming | completed | 44 | 37.589 | 2.170 | 0.000 | 0.939 |
| pause-cached-whisper-cpp | completed | 84 | 21.711 | 0.335 | 0.000 | 0.000 |
| pause-cached-whisper-streaming | completed | 56 | 31.841 | 0.858 | 0.000 | 0.000 |
| pause-live-simulstreaming | completed | 42 | 38.445 | 2.693 | 1.353 | 1.040 |
| pause-live-whisper-cpp | completed | 88 | 21.773 | 0.333 | 1.990 | 0.000 |
| pause-live-whisper-streaming | completed | 44 | 32.784 | 1.618 | 1.693 | 0.000 |

cached 和 live 的 classifier／阈值一致；普通 pause 不 finish/reset。尾部 ladder 每次只增加一项动作；SS finish 已包含原生 context 清理，reset 再清 online 时间／显示状态，不能把两者当作完全独立的纯 buffer 操作。250／800ms 和填零保留为消融，不改主线默认。

| 边界运行 | 状态 | 调用数 | 最终字符数 | EOF s |
| --- | --- | --- | --- | --- |
| boundary-long-gap-live-simulstreaming | completed | 23 | 51 | 0.626 |
| boundary-long-gap-live-whisper-cpp | completed | 26 | 38 | 0.000 |
| boundary-long-gap-live-whisper-streaming | completed | 22 | 94 | 0.000 |
| boundary-long-gap-off-simulstreaming | completed | 55 | 74 | 0.728 |
| boundary-long-gap-off-whisper-cpp | completed | 63 | 64 | 0.000 |
| boundary-long-gap-off-whisper-streaming | completed | 47 | 932 | 0.000 |
| boundary-mid-sentence-eof-live-simulstreaming | completed | 9 | 14 | 0.361 |
| boundary-mid-sentence-eof-live-whisper-cpp | completed | 8 | 14 | 0.000 |
| boundary-mid-sentence-eof-live-whisper-streaming | completed | 5 | 26 | 0.000 |
| boundary-mid-sentence-eof-off-simulstreaming | completed | 10 | 15 | 0.492 |
| boundary-mid-sentence-eof-off-whisper-cpp | completed | 9 | 14 | 0.000 |
| boundary-mid-sentence-eof-off-whisper-streaming | completed | 5 | 26 | 0.000 |
| boundary-music-intro-live-simulstreaming | completed | 0 | 0 | 0.000 |
| boundary-music-intro-live-whisper-cpp | completed | 0 | 0 | 0.000 |
| boundary-music-intro-live-whisper-streaming | completed | 0 | 0 | 0.000 |
| boundary-music-intro-off-simulstreaming | completed | 19 | 8 | 0.464 |
| boundary-music-intro-off-whisper-cpp | completed | 18 | 5 | 0.000 |
| boundary-music-intro-off-whisper-streaming | completed | 12 | 14 | 0.000 |
| boundary-quiet-onset-live-simulstreaming | completed | 22 | 53 | 0.627 |
| boundary-quiet-onset-live-whisper-cpp | completed | 26 | 38 | 0.000 |
| boundary-quiet-onset-live-whisper-streaming | completed | 21 | 84 | 0.000 |
| boundary-quiet-onset-off-simulstreaming | completed | 29 | 56 | 0.660 |
| boundary-quiet-onset-off-whisper-cpp | completed | 34 | 52 | 0.000 |
| boundary-quiet-onset-off-whisper-streaming | completed | 20 | 84 | 0.000 |
| boundary-repeat-regression-off-simulstreaming | completed | 13 | 359 | 1.607 |
| boundary-repeat-regression-off-whisper-cpp | completed | 82 | 166 | 0.000 |
| boundary-repeat-regression-off-whisper-streaming | completed | 28 | 620 | 0.000 |
| boundary-silence-live-simulstreaming | completed | 0 | 0 | 0.000 |
| boundary-silence-live-whisper-cpp | completed | 0 | 0 | 0.000 |
| boundary-silence-live-whisper-streaming | completed | 0 | 0 | 0.000 |
| boundary-silence-off-simulstreaming | completed | 56 | 220 | 42.576 |
| boundary-silence-off-whisper-cpp | completed | 61 | 28 | 0.000 |
| boundary-silence-off-whisper-streaming | completed | 13 | 1533 | 0.000 |

无语音的 live VAD 会话不为 EOF 强制运行 ASR；warmup 不计入调用数。音频完整供给与原媒体坐标保留，最后检测静音未交 ASR 的范围单列，不能用空文字宣称轻声质量正确。音乐／静音的非空输出、停顿恢复、截断尾部和既有重复片段原文保留供审核。

| 取消 pipeline | 下一次解码开始后取消 | 强制 group kill | 进程组释放 | 通过 |
| --- | --- | --- | --- | --- |
| WS | True | False | True | True |
| SS | True | False | True | True |
| CPP | True | False | True | True |

取消仅向父 runner 发 SIGTERM，要求它自己关闭 ASR/VAD children；强制 kill 只用于失败清理，不算通过。

## 未闭环项与收敛建议

所有参考仍保留真实核验状态。当前尚未完成完整的人工核验：声学末尾、正确原文／认可语义、逐候选关键语义、审核过的 large-v3 基线及 holdout 含义。因此 winner 为 `null`，没有正式的1s／2s验收、CER/WER或质量胜负结论。

CPP small／500ms 是低计算开销候选，优先核对其跨窗漏词／重复和草稿修订；WS small／500ms 提供 LocalAgreement 草稿，优先完成20个上下文锚点核验。SS small／MPS 本轮有预算退出、长独白开销和 Unicode 异常，当前不作为可部署默认；base／tiny 的短筛选不能替代完整稳定性与质量证据。

时间坐标审计：pinned SS 多 segment 淘汰只返回最后一个移除时长，finish 后 online offset 也未完整同步。原生 provider timestamp 保留为带警示的诊断，共同指标使用独立媒体时钟；未来具体 adapter 收敛时修复 bookkeeping 并回归，不能把坐标修正当成模型变快。

下一轮首先修复长静音恢复的解码调度：完整录音保存与 ASR 的有限声学上下文如何分离、跳过已判静音时如何保持媒体坐标，仍需确定；本轮没有悄悄引入新的裁剪／reset 策略。随后完成人工上下文核验再选择具体配置；有已核准延迟达标候选后，再做计划内单变量 trimming／frame_threshold／window 优化及同等信息的专名 prompt 配对。没有把 ASR 预算通过视作 MT 或在线翻译 E2E 已达标。选定后只提取一个 adapter、最小 session／时钟／VAD／输出合同，offline 全文增强作为独立运行，不扩展成桌面多 backend 框架。

## 证据、验证与隔离

原始 run／events／logs、模型 provenance、冻结配置、resource samples、授权／删除回执和参考状态位于忽略的 `local-artifacts/online/controlled-*`。候选核验材料分别保留原用户备注及连续上下文。每次运行保存 source／lock／model／worker 摘要；筛选期间存在少量审计／holdout 校验代码 revision 差异，逐行按实际摘要保留，三轮正式回放使用已提交的稳定模型驱动。旧 v2 和适配探针不覆盖、不混合排名。

验证：39项 Python 回归、项目 Python 模块语法编译、CPP private worker 构建、实际 FP16 模型路径、独立供给／忙碌合并／EOF／取消、`cargo fmt --check` 与 `git diff --check`。实际模型失败在表中完整保留。正式 offline sidecar 和共享模型的摘要／大小／mtime 检查另保存在 isolation-check.json；没有修改 GUI、正式任务 SQLite 或 offline 流程，也没有另跑完整 offline GUI 验收。

新增受控权重约2.0GB，样本约49MB，日志／参考／上下文为MB级；既有 large-v3研究模型约5.9GB、两个uv环境约2.0GB仍保留，总实验占用约10GB。尚未选型时不删除候选权重，后续保存证据后清理未选且可重建的格式；不处理正式共享模型。

检查备注：最初对整个 experiments/online 递归 compileall 会扫入 uv 环境，torch/testing/_internal/py312_intrinsics.py 使用 Python 3.12 测试语法，在本轮 Python 3.11 环境下失败。改为仅编译项目 Python 模块后通过；没有修改第三方测试文件。普通模型执行与39项回归已验证。
