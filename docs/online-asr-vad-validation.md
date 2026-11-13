# 三套 ASR baseline 与 VAD 回放验证

日期：2026-10-04。范围仅 ASR；没有启动 Hy-MT2 或调用云端。

## 固定条件与实现边界

Development 节目《湖吉の庭 Vol.1》00:00–01:00，包含音乐片头、短停顿和日语独白。`prepare_case.py` 校验来源摘要与 development role，使用同一 mono／16 kHz／PCM16 WAV。此次没有再次使用或调试 holdout。

独立 `replay_asr.py` 使用真实媒体时钟，推理落后时合并已经到达的 PCM，不丢包、不压缩静音；最多一次供给 30 秒。模型加载及固定一秒零音频 warmup 在 replay clock 外单列。每轮有 180 秒 wall-time 上限，顺序执行；与之前直接调用上游文件 CLI 的数据不混合排名。

- Whisper-Streaming：既有 pinned upstream、MLX large-v3 fp16、Metal，LocalAgreement；同时观测未确认 buffer 的显示文本。
- SimulStreaming：既有 pinned upstream、large-v3 fp32、torch CPU，AlignAtt；接口只暴露已提交文本。
- whisper.cpp：项目固定 v1.8.6 source archive，SHA-256 已验证，独立构建 worker；只读复用正式 large-v3 Q5_0 模型。默认关闭上下文，greedy、4 threads、Metal／flash attention。沿用官方 stream 的窗口拼接／周期 rollover／重叠，由文件时钟替代 SDL 捕获，采用无丢包输入；不称作原封不动的官方二进制测试。

默认 step 为 1 秒，CPP length 为 5 秒、keep 为 200 ms；另测 CPP 500 ms step。Runtime、精度和解码差异明确保留，本轮是系统 baseline 对照，不能孤立归因于提交算法。

## VAD 对照与指标

Silero 6.2.3、CPU、单线程，每帧 512 samples，因果顺序计算概率并存档；模型 SHA-256 为 `e1122837f4154c511485fe0b9c64455f7b929c96fbb8d79fbdb336383ebd3720`。回放只消费当前媒体时间已经到达的检测事件，不读取未来 endpoint；VAD 推理本身单独测量，本段合计约 0.189 秒，不包含到 ASR replay clock，尚未验证 live VAD 资源争用。

三套 ASR 分别比较 off、gate、endpoint 500 ms；CPP 另测 endpoint 250／800 ms。共同 onset threshold=.5、negative threshold=.35、pre-roll=200 ms。Gate 在非语音期间暂停推理，并用零音频保留被跳过的媒体时间；endpoint 额外 flush 和重置 ASR 状态，因此也改变了跨段上下文。

首次可见文本与首次算法提交分开。只有明确的 commit event 参与提交延迟；draft、周期窗口冻结、endpoint／EOF flush 均不自动成为算法确认。输入处理落后 `emission − consumed PCM end` 可共用口径，但不是相同话语的显示延迟；词级 provider 时间只作为附加观测。CPP 无可靠 provider speech end，指标保持 null。显示撤回统计不能补造 SS 未暴露的草稿。

音乐段出现的非空结果不算有效语音识别优势。最终文本、显示版本、失败和 EOF 证据保留；未听审，不计算 CER/WER，也不宣称质量胜负。

## 磁盘与 offline 隔离

本轮前 `du` 报告实验模型约 5.9 GB、两套 uv 环境约 2.0 GB，总计约 7.9 GB；这不是“几乎不占空间”。不同 ASR runtime 的模型格式与 Python 依赖占绝大多数，已有音频片段／全文结果／截图／日志为 MB 级。`du` 不精确反映 APFS clone 的共享 extents，数值作为目录占用口径。

CPP 复用已有模型，没有再下载一份大型权重；新源码约 38 MB、build 约 20 MB、source archive 约 1.6 MB，Silero package 约 10.8 MB。上轮 OCR 专用 `/tmp` Swift module cache 约 78 MB 已清理，截图证据保留。

本轮修改范围为实验脚本、SS uv dependency／lock 和文档。没有更改 offline core／provider、GUI、桌面依赖或正式数据；对现有 sidecar 与复用模型记录摘要／mtime，用于结束后校验只读性。实验运行时会占用 CPU／GPU／RAM，可能影响同时运行的 offline 任务速度，结束后回收进程。

## 收敛要求

选定方案后保留一个具体实现：所选 ASR adapter、最小 session／时钟／VAD／输出协议，并保持 online／offline 的业务边界。其他 backend、参数矩阵和研究脚本作为实验记录保留，不全部转为生产框架或桌面 dependency。

保留 pins／lock、样本摘要、可读评估结论和失败证据；随后清理可重建 clone／build／venv 及未采用的模型格式。正式共享模型不随实验清理。

## 本轮结果

12/12 组完成；同一 60 秒输入、顺序执行，每组只测一次，没有统计置信区间。以下为秒，`输入落后` 是 consumed PCM 对应的系统处理落后，`endpoint 完成等待` 是从 VAD 估计 speech end 到 flush 完成的时间，均为 p50。表中结果不能转换为同一个词的字幕延迟。

| Backend／策略 | 解码累计 | 输入落后 | endpoint 完成等待 | 显示撤回次数 |
| --- | ---: | ---: | ---: | ---: |
| cpp-off | 60.76 | 1.35 | — | 11 |
| ws-off | 62.74 | 2.89 | — | 5 |
| ss-off | 68.51 | 3.36 | — | 0 |
| cpp-gate | 53.14 | 2.59 | — | 9 |
| ws-gate | 69.86 | 9.48 | — | 6 |
| ss-gate | 75.04 | 10.89 | — | 0 |
| cpp-endpoint500 | 53.40 | 2.67 | 3.22 | 6 |
| ws-endpoint500 | 61.59 | 9.55 | 10.06 | 1 |
| ss-endpoint500 | 118.53 | 35.46 | 35.47 | 0 |
| cpp-endpoint250 | 52.88 | 3.00 | 3.05 | 5 |
| cpp-endpoint800 | 52.90 | 2.95 | 3.64 | 5 |
| cpp-off-step500 | 61.49 | 1.72 | — | 22 |

CPP 的 1 秒 off 配置中输入落后为 1.35 秒，WS 为 2.89 秒，SS 为 3.36 秒；这是当前设备／runtime／精度／调度的系统结果。首次非空显示分别在 2.30／3.20／6.06 秒，但当时尚未起声，分别含音乐段 hallucination 或音乐标记。Silero 参考起声为 10.56 秒。起声后的首个非空更新也未经逐字对齐，不能当作“有效识别首字延迟”；完整事件保留供听审。

Gate 消除了参考起声前的非空显示（off 各有 4／2／2 次），但 WS／SS 解码累计从 62.74／68.51 秒上升到 69.86／75.04 秒，输入落后上升到 9.48／10.89 秒。当前策略对每个检测到的尾部强制处理，且保留媒体静音坐标；它改变了 batching 和推理频率。不能据此认定 VAD 本身计算很慢，也不能把 VAD 默认视为延迟优化。

500 ms endpoint 检测增加约 512 ms 的因果静音等待，后续排队／解码／flush 才是本轮等待的主要部分：CPP endpoint 完成等待 3.22 秒，其中 detection 后约 2.71 秒；WS 为 10.06／9.55 秒；SS 为 35.47／34.96 秒。SS 的 finish 确实调用模型，本轮解码累计达到 118.53 秒；WS finish 只是提交 buffer，二者成本没有混为同一次真实解码。频繁 flush/reset 的代价需要在调度与上下文策略中解决。

CPP 250／500／800 ms endpoint 分别完成 20／18／13 次段结束，完成等待 p50 为 3.05／3.22／3.64 秒。250 ms 另有一个尚未 endpoint 的末段在 EOF 处理；阈值加长会合并段落并增加检测等待。观察到 reset 后文本与其他策略有分歧，尚未人工听审，不能按文本更流畅直接判断准确率。

CPP off step 从 1 秒缩短到 500 ms 后，输入落后从 1.35 增到 1.72 秒，撤回次数从 11 增到 22，最终窗口拼接文本也明显不同。在推理无法跟上更新频率时，按调用次数 rollover 与无丢包合并输入的交互值得修正；当前不推荐直接用 500 ms step。窗口拼接可能有漏词／重复，冻结窗口也不能充当 LocalAgreement 的稳定提交。

原始证据在忽略目录 `local-artifacts/online/asr-vad-round/`：每组 run metadata、事件流、日志，以及 `comparison-activity.json`。保存实际模型／worker／upstream／lock 摘要，不提交媒体和私人文本。本轮事件的 WS display stage 沿用 append 的 stage，snapshot 可能含 draft；指标只以 commit event 判定提交。完成矩阵后已修正该显示标签并加强 worker 初始化失败清理、复用配置校验，未改变解码或供给逻辑；旧事件保持原样。

验证：私有 CPP worker 成功构建；12 组模型回放完成；21 项 Python 回归、`cargo fmt --check`、`git diff --check` 通过。结束后正式 offline sidecar 与共享 GGML 模型的 SHA-256、大小、mtime 均与运行前一致。本轮目录占用约 8 GB，其中新结果不足 1 MB，没有复制第三份大型权重。功能隔离依据为修改范围、独立环境／build、只读摘要检查；本轮未另跑 GUI／完整 offline 流程验收。

## 下一步

先修正 VAD 调度：让 gate 只决定供给和是否唤醒，不因每个短尾部额外解码；把 ASR 的状态保留／flush／reset 与未来翻译的分组 endpoint 分开。继续保留三套 baseline，CPP 固定 1 秒 step 作为当前低成本对照；SS CPU 的 endpoint500 配置暂不作为实时默认。修正后用同一 development 输入做配对复测，再扩展连续讲话／多停顿／长静音样本，并人工对齐少量语音段测有效首字和稳定词延迟。此次没有足够证据选定生产实现，也没有启动 Hy-MT2；方案选定后按上文收敛要求只提取一套。
