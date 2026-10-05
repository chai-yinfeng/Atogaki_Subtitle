# Online CLI 实验

此目录独立于 offline 两段式业务与 GUI。第一轮 ASR 比较 Whisper-Streaming／LocalAgreement 与 SimulStreaming／AlignAtt；翻译固定 Hy-MT2。当前是上游 ASR 回放 + **串行翻译策略重放**，不是并发实时系统。

## 环境与输入

```sh
python3 experiments/online/setup.py
```

该命令按 `upstream-pins.json` 检出两个固定 commit，再执行 `uv sync --locked`。环境位于 `experiments/online/envs/<backend>/.venv/`，上游与模型位于 Git 忽略的 `local-artifacts/online/`。Python 固定 3.11.16；不修改全局 Python 或桌面依赖。首次运行需要网络下载源码、Python 与 packages。

Python 标准库即可运行适配、翻译和测试。两个上游分别使用 uv 管理的独立环境、固定 Git checkout 和已提交的 uv.lock；不将 torch／MLX 加入桌面依赖。Whisper-Streaming runner 使用 Apple Silicon 的 `mlx-whisper`；SimulStreaming runner 使用上游默认 torch runtime。当前固定上游默认只选择 CUDA／CPU，在 Mac 上走 CPU；MPS 适配与正式实时速度尚未验证。相同模型名称不意味着相同 runtime，结果必须区分算法收益和实现差异。

官方入口：[Whisper-Streaming](https://github.com/ufal/whisper_streaming)、[SimulStreaming](https://github.com/ufal/SimulStreaming)。上游 commit 和依赖已固定；正式 large-v3 模型与质量基线仍待完成。不要使用浮动 main 的结果作为可复现质量结论。

用现有 FFmpeg 将固定样本转为 16 kHz mono PCM16 WAV，媒体与输出都保存在 Git 忽略的 `local-artifacts/online/`。同一段 WAV、语言、chunk size 和模型档位先运行两个 backend；初期两者均禁用 VAD，避免额外混淆。随后单独比较 VAD。

```sh
python3 experiments/online/run_asr.py --backend whisper-streaming \
  --checkout /path/to/whisper_streaming --python experiments/online/envs/whisper-streaming/.venv/bin/python \
  --audio local-artifacts/online/sample.wav --model large-v3 \
  --output-dir local-artifacts/online/ws-run
python3 experiments/online/run_asr.py --backend simulstreaming \
  --checkout /path/to/SimulStreaming --python experiments/online/envs/simulstreaming/.venv/bin/python \
  --audio local-artifacts/online/sample.wav --model /path/to/large-v3.pt \
  --output-dir local-artifacts/online/ss-run
```

Runner 使用上游 computationally-aware 默认回放，保存 stdout、stderr、实际 Git commit、命令、音频 SHA-256、时长和退出状态。同时保存实际安装 packages、uv.lock 摘要和显式本地模型文件摘要；MLX 本地快照可用 `--mlx-model-dir` 指定。设备／精度和模型转换差异仍需在正式基线中补充。Python executable 保留 venv 路径，不解析 symlink 到环境外。失败保留证据，不能把未完成的 raw 文件当成功样本。

## 翻译与回改

复用已有 `scripts/hy-mt2-versions.zsh` 的模型／llama.cpp pins，启动只监听 loopback 的 llama-server。已有 `validate-hy-mt2.zsh` 会在验证结束时回收进程，不能把它当常驻服务。模型 ID 由加载的服务器配置确定；需要鉴权时使用 `ATOGAKI_ONLINE_LLAMA_KEY`，不写入 artifact。

```sh
python3 experiments/online/harness.py translate \
  --input local-artifacts/online/ws-run/source.jsonl \
  --output local-artifacts/online/ws-run/revisable.jsonl \
  --model Hy-MT2-1.8B-Q4_K_M.gguf --policy revisable --interval 5
python3 experiments/online/harness.py report \
  --input local-artifacts/online/ws-run/source.jsonl \
          local-artifacts/online/ws-run/revisable.jsonl
```

`boundary` 等到标点、上游 voice boundary、字符预算或 EOF 后翻译；`revisable` 额外按上游输出时间间隔重译活动组。`translation_replace` 引用 group ID 和 source revision，`completion_reason` 区分句末／字符预算／provider voice boundary／input EOF；final 仅表示本次实验策略冻结显示，不保证离线正确性。当前不带跨组上下文／词表，且不会重新解码 ASR committed 文本。标点规则是基线 heuristic；未针对小数、缩写和日语引语优化。

上游只输出已确认文本：Whisper-Streaming 的三列毫秒文本转为 `source_append`；SimulStreaming JSONL 的 `is_final` 保留为 voice boundary，不将其解释成全文正确性。Whisper 文本格式缺少 voice boundary，因此目前 boundary 对比会包含 endpoint 信号差异，需要在正式算法比较前统一策略或单独报告。

指标目前包括 ASR emission 减估计 audio end 的 p50/p95、翻译请求耗时与组替换次数。时间戳不是人工 gold；译文耗时是独立重放测量，**不能相加冒充在线端到端延迟**。缺少 hypothesis 的更新不计入稳定性；不能据此宣称 ASR 没有回改。

## 回归与后续

```sh
python3 -m unittest discover -s experiments/online -v
```

下一阶段：固定日语否定／专名／长句与长节目样本，使用正式模型测量 Apple Silicon 兼容性，再加入媒体时钟驱动的并发翻译、过期响应拒绝、背压、取消与 gap／EOF 合同。在线结束后的全文 offline 精修是独立 run，本目录不调用工作区写回。

2026-10-02 验证：7 项标准库回归通过；使用既有 pinned Hy-MT2 1.8B Q4_K_M／llama-server 完成合成日语否定句的真实 loopback 翻译请求并回收进程。仅证明 transport／输出合同，未测试真实节目、ASR 或并发性能。

## SimulStreaming 的侵入性边界

Runner 调用上游 `simulstreaming_whisper.py`，其内部导入仓库自带的修改版 Python Whisper；我们没有在现有 offline whisper.cpp 中移植 AlignAtt。固定源码中，`PaddedAlignAttWhisper` 给 decoder cross-attention 安装 hooks，收集 attention score 并做 softmax；给 key/value 安装 hooks 管理 KV cache；取 alignment heads，经标准化、median filter 和 head 聚合，逐 token 检查最关注 frame 与音频末端的距离。修改版 `model.py` 默认禁用 SDPA，并为缓存增加 `cache_id`。侵入性位于独立上游 runtime 内，不能用普通 ASR 最终文字接口替代。

2026-10-02：两个上游 CLI import/help 与模型加载通过。同一开发节目开头 12 秒 PCM16 WAV、tiny／1 秒 chunk／无 VAD 的实际 computationally-aware 回放均完成；WS 使用 MLX，SS 使用默认 CPU。WS 出现明显音乐段重复，SS 输出音乐标记和问候。只有 3／2 个输出事件，且 runtime、精度和处理策略不同，这只是调用链冒烟，不能推断模型质量或正式性能胜负。原始证据保存在本地 `ws-tiny-smoke/`、`ss-tiny-smoke/`。

## 正式模型开发样本

使用既有 evaluation manifest 的开发集，`prepare_case.py` 会校验完整媒体 SHA-256、范围和 role，拒绝将 holdout 用于本轮实验：

```sh
python3 experiments/online/prepare_case.py \
  --manifest /path/to/evaluation/manifest.json \
  --case-id ja-kokichi-vol1-development --start-ms 15000 --end-ms 75000 \
  --output-dir local-artifacts/online/cases/vol1-15-75
```

两个 `run_asr.py` 必须使用该目录的同一 `audio.wav`。Whisper-Streaming 使用 MLX large-v3 本地快照，SimulStreaming 使用官方 `large-v3.pt`；分别记录模型文件摘要与转换快照 revision。正式性能回放顺序运行，翻译服务在 ASR 比较期间不启动。初轮仅覆盖 60 秒开发样本，不代表长节目、holdout 或泛化验收。

```sh
python3 experiments/online/compare_runs.py \
  --runs local-artifacts/online/ws-large-v3-vol1-15-75 \
         local-artifacts/online/ss-large-v3-vol1-15-75 \
  --output local-artifacts/online/large-v3-comparison.json
```

比较工具拒绝不同音频和不含计算耗时的模拟；输出最终原文、观测指标和限制。不将模型文本标记为 verified，不自动生成 CER/WER。

Gemini 3.5 Transcribe 可后续作为 `model_reference` 记录，需与相同音频范围对应，并保留模型、参数、时间戳与来源。模型参考用于定位分歧和缩短听审成本，未经听审不能成为 gold，也不能用相对于它的差异率描述绝对准确率。本轮只跑本地实验，不调用云端。

第一轮 large-v3／Hy-MT2 1.8B 的实际结果和限制见 [验证记录](../../docs/online-large-v3-validation.md)。

## 三套 ASR 与 VAD 的统一回放（不运行翻译）

`replay_asr.py` 是独立的 v2 ASR 实验入口，使用单调时钟按真实媒体时间供给 PCM；推理落后时合并已到达的音频（最多 30 秒），不丢包、不改变原始时间轴。与旧 computationally-aware CLI 的指标分别保存，不混合排名。录音必须来自 `prepare_case.py` 校验过的 development 范围。

第三个 baseline 使用 `setup_cpp.py` 按既有 sidecar 源码 pin／SHA-256 构建私有 inference worker。窗口拼接、周期 rollover、重叠和默认关闭上下文沿用该版本官方 `examples/stream/stream.cpp`；文件时钟替代 SDL 麦克风采集，并采用无丢包供给。因此称为 **whisper.cpp stream policy 文件适配版**，不是未经修改的 `whisper-stream` 二进制实测。当前 worker 固定 greedy、4 threads、Metal／flash attention、single segment，不生成可用于词级延迟的时间戳；periodic rollover／endpoint freeze 都不等于稳定提交。

```sh
python3 experiments/online/setup_cpp.py
uv sync --locked --project experiments/online/envs/simulstreaming
experiments/online/envs/simulstreaming/.venv/bin/python experiments/online/vad_plan.py \
  --audio local-artifacts/online/cases/development/audio.wav \
  --output local-artifacts/online/cases/development/vad.json
experiments/online/envs/simulstreaming/.venv/bin/python experiments/online/run_asr_suite.py \
  --audio local-artifacts/online/cases/development/audio.wav \
  --vad-plan local-artifacts/online/cases/development/vad.json \
  --cpp-model /path/to/existing/ggml-large-v3-q5_0.bin \
  --output-dir local-artifacts/online/asr-vad-round
```

仅新增实验用 silero-vad dependency，锁在 SS 的 uv 环境，WS／CPP 读取相同概率文件。Silero 每 512 个 16 kHz samples（32 ms）因果处理，保存模型 SHA-256、概率、每帧耗时、输入摘要。回放只消费当前已到达的 frame decisions；预先计算不会提前提供未来 speech endpoint。检测计算耗时单独报告，不冒充 live VAD＋ASR 的资源争用测量。

矩阵为三套 backend × off／gate／endpoint（500 ms），另对 CPP 测 250／800 ms endpoint 及 500 ms step。统一默认 step=1 秒、CPP length=5 秒／keep=200 ms；threshold=.5、negative threshold=.35、pre-roll=200 ms，参数是待评估候选。每个 child 有 wall-time 限制，顺序运行，不启动 Hy-MT2、不调用云端。输出目录存在时只复用已完成、同音频且调用配置完全相同的证据，不覆盖未完成轮次。

- `off`：所有 PCM 均进入 ASR。
- `gate`：非语音时不推理；第一次起声从 pre-roll 开始，以后将跳过的时间填零，保留原媒体坐标和 ASR 状态。它测量过滤＋暂停推理，不将静音压缩掉。
- `endpoint`：使用同一 gating，但检测到持续静音后处理尾部、调用 finish，并在下次起声重新初始化 ASR 状态。这同时改变结束等待和跨段上下文，不能把质量变化只归于 VAD classifier。

`events.jsonl` 保存 inference、commit、flush、display、VAD decision 和 finish。WS 另外读取 pinned upstream 的未确认 buffer 用于 draft 显示观测；SS 只暴露 AlignAtt 输出，不伪造未暴露的 hypothesis。Only `commit` events contribute to commit latency；EOF／endpoint 强制 flush 不纳入稳定提交。Display snapshot 可以含已确认前缀与未确认尾部，不能仅靠 snapshot 的 stage 推断整段稳定。

`asr_metrics.py` 汇总首次可见／首次算法提交、模型推理次数与耗时、输入处理落后 p50／p95、显示撤回字符数、EOF flush、endpoint 服务等待及最终显示文本。输入落后是 `emission − consumed PCM end`，与所有 backend 的相同文件时钟对应，**不是单词说完到显示的延迟**。Provider commit delay 另列，CPP 没有该值；SS 的零显示撤回只表示 append-only 可观测接口，不证明其内部没有修改。

可额外用 `--vad-reference vad.json` 报告参考起声前的非空显示，以及起声后的首个更新；它们只帮助发现音乐段输出，仍需人工判断文本是否有效。没有听审 reference 的轮次不输出 CER/WER，不使用空白占位补造时间戳。失败轮次保留 metadata／日志，并从成功指标聚合中剔除，失败率单独说明。

### 实验结束后的收敛

实验源码与文档留在 `experiments/online/`，大文件和所有媒体产物保持忽略。本轮只读复用 offline 的 GGML 模型，使用独立 build、worker 和 uv 环境；不替换 offline sidecar，不读写正式 SQLite 或任务目录。运行期间仍占用 CPU／GPU／RAM，可能与同时进行的 offline 任务争用资源；进程结束后释放，不代表功能耦合。

选定方案后，只把所选 ASR adapter、最小 session／时钟／VAD／输出合同提取为具体一版；未选 runtime 不进入桌面依赖。保留输入摘要、pins／lock、评估结果与失败证据，再清理可重建的 clone、build、venv 和未采用模型格式。共享模型仅在确认其他功能无引用后才处理，绝不随实验清理删除正式模型。

本轮 12 组 ASR-only 实测与存储／隔离核验见 [验证记录](../../docs/online-asr-vad-validation.md)。

参数控制、指标语义、VAD／调度配对设计与 Qwen 候选边界见 [online 实验合同](../../docs/online-experiment-contract.md)。该合同区分已有观测与待实现指标，不代表已经接入新 provider。

## v3：同源模型与独立供给时钟

本轮实现遵循 [0046](../../docs/decisions/0046-controlled-online-asr-performance.md)。`prepare_models.py` 在 SS uv 环境下载并校验官方 small／base／tiny checkpoint，转换 MLX FP16 和 GGML F16（指定小张量仍为 FP32），记录源／工具／资产摘要及 alignment heads。MLX 使用 greedy，不传会触发未实现 beam search 的 `beam_size=1`。SS 的局部 compatibility shim 在 hooks／context 创建前指定 MPS／FP16，CPU 明确负责 float32 STFT／mel，不允许隐式模型 CPU fallback。GPU 完成同步包含在调用计时内；实际 activation／logit dtype 与 kernel 累积精度的未知部分单列。

```sh
experiments/online/envs/simulstreaming/.venv/bin/python experiments/online/prepare_models.py --size small
experiments/online/envs/simulstreaming/.venv/bin/python experiments/online/prepare_controlled_cases.py \
  --manifest /path/to/evaluation/manifest.json --root local-artifacts/online/controlled-cases
experiments/online/envs/simulstreaming/.venv/bin/python experiments/online/run_controlled_suite.py \
  --root local-artifacts/online/controlled-final --cases local-artifacts/online/controlled-cases \
  --phase screen --sizes small base tiny
```

`replay_controlled.py` 读冻结的 case，而不是裸音频路径。单独 producer 按每帧 32ms 的到达时刻更新 cursor／VAD decisions，consumer 一次只执行一个 ASR 调用，过期触发合并，原媒体坐标不变。VAD pause 保留原始 PCM，不自动 tail decode、finish 或 reset；`--tail-decode`、`--finish-on-pause`、`--reset-on-pause` 分开。明确的一项限制：SS 上游 finish 本身会清除 segment/context，因此 SS 的 finish 实验包含该原生状态变化；不能宣称其为“纯 buffer flush”。WS finish 只预览尾部，不把它重复追加进已确认前缀。

Cached VAD 为控制实验；live VAD 在独立的 SS uv CPU worker 中逐帧运行，因此无需把 torch 加进 WS runtime。其计算／IPC 与 ASR 并发开销包含在 replay 时钟内。EOF 排空最后语音尾部；VAD 完全未起声时不为了 EOF 对整段静音强制解码；最后停声后的已检测静音可不交 ASR，另列 `asr_unprocessed_trailing_silence_s`，完整输入供给和原始录音仍保留。不能把这项与无声学证据的丢包混淆。

每次运行保存 resolved config、driver/shim hash、lock、packages、upstream pin、实际设备／dtype、模型／worker／library 摘要。旧 v2、适配探针和正式 v3 保存在不同目录；调试时资源重叠的探针不用于性能排名。父进程 RSS、CPP/VAD child 最大 RSS、MLX peak 和 MPS allocator 观测分别保存，不能简单相加声称系统总峰值。

后续 `--phase evaluation` 使用冻结候选，按轮换顺序重复完整 development 三次，再跑10分钟和一次 holdout。提取 holdout 必须传 `prepare_controlled_cases.py --frozen-config`，回放还会核对摘要和模型／解码／原生参数；不能评估完继续改它。`--phase vad` 比较 cached／live pause，及 SS 尾部动作／静音阈值消融；`--phase boundaries` 使用确定性派生 fixture 和既有重复片段。运行目录不覆盖失败；同名复用要求命令和 driver 都一致。

`controlled_metrics.py` 只把人工明确核验的文字／语义及声学时间用于有效延迟；模型文字、provider 时间保持 diagnostic。未输出／多处匹配进入缺失或歧义分母；窗口重复不事后修正。`summarize_controlled.py` 另外检查3次完整回放、长样本积压、逐候选关键语义与绑定 run 摘要的 holdout 核验，未闭环则 winner=null。参考 schema 中 `acceptable_texts` 用于人工认可的语义等价表达，`semantic_observations[run_name]` 为 correct／critical_error，`large_v3_baseline` 必须绑定历史输出摘要、review_status=reviewed 以及逐 anchor 的 correct／critical_error；不能用 Gemini 是否正确替代 large-v3 基线审核。

Gemini batch 入口严格限制为授权的四个 development 范围；Keychain 凭据只留在内存，逐段删除上传文件，失败和删除结果保留。`make_gemini_review.py` 从真实 word annotations 提出20个短语和本地听审音频，不自动标 verified。分析时用 `--reference` 指定新参考文件；核验更新不覆盖原模型响应或原运行事件。没有听审的字符差异不是绝对 CER，也不能把模型时间当作人工精确时间。

`check_cancellation.py` 使用独立 cancel_probe，在三个冻结候选完成首个真实 inference、下一次解码开始后，只向父 runner 发 SIGTERM，并检查 metadata 及其进程组释放；强制 group kill 仅用于失败清理，不算取消验收通过。`make_candidate_review.py` 生成绑定原始 run 摘要的 +2s 显示、最终邻近文本、模型短语匹配和听审上下文；保留各 repeat／holdout 的实际输出。分析器分别匹配逐字原文和人工批准的语义表达，明确标 uncertain 的锚点可排除，但未输出的已核验锚点仍留在覆盖率分母；pending 参考不通过验收。

`sample_resources.py` 每30s只观察本轮 runner 和其子进程的 current RSS／VSZ，不记录其他应用 argv；`controlled_metrics.py` 报告10分钟最后2分钟与第2–3分钟的 runner RSS 中位数变化。模型 allocator 终点、进程峰值和 RSS 采样各有不同口径，保留原始进程序列，不宣称精确系统／GPU 总峰值。

时间坐标审计另发现 pinned SS `insert_audio` 在一次移除多个 segment 时只返回最后一个移除时长，native finish 清空模型 buffer 后也没有完整重设 online timestamp offset。原生 provider timestamps 因此保留为带警示的诊断，不能作为统一声学末尾；本轮 anchor／输入落后仍用独立原媒体时钟，不通过修补 timestamp 伪装延迟改善。该上游 bookkeeping 问题留待选型后的具体 adapter 修复。

取消探针不改主 runner／模型驱动，只在独立子进程内包裹 run 写开始 marker，用于验证忙碌时释放；该轮源摘要单独保存，不进入性能排名。

CPP 的模型在 native worker 内，RSS 增长另外按每个 child PID 计算并报告；Python parent、worker 和 VAD child 保持不同列，不把 parent 小占用当成 ASR 总占用。

`--phase live-long --live-backends whisper-streaming whisper-cpp` 在已冻结 native 参数上补 live VAD 十分钟资源／积压对照。纯静音 off 的 hallucination 及短 live 的时钟开销说明不能只用 cached 概率外推部署效果；失败配置可明确排除，不重复 holdout。选择器分别检查 off 和 live 的持续性证据，不把两种运行合成一条长样本。

本轮固定配置、80组模型回放、3组忙碌取消及参考状态见 [受控验证报告](../../docs/online-controlled-asr-validation.md)。当前 pause runner 在长静音后会按30s追赶旧 PCM；live 长回归已暴露此调度问题，不能作为可部署默认。`sustained.passed` 只代表原增长条件；`deployment_diagnostics_passed` 还要求 VAD 起声坐标的输入恢复不超过2s。这是保守的调度诊断，不是有效文本延迟。完整录音与有限 ASR 上下文的分离策略尚未收敛，后续修复必须保留真实媒体坐标并单独复测。WS／CPP 预处理没有隔离测量，分析报告用 null 表示，原 metadata 中的0为占位值。
