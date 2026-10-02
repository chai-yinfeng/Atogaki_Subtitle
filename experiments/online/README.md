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

`boundary` 等到标点、上游 voice boundary、字符预算或 EOF 后翻译；`revisable` 额外按上游输出时间间隔重译活动组。`translation_replace` 引用 group ID 和 source revision；final 仅表示本次实验策略冻结显示，不保证离线正确性。当前不带跨组上下文／词表，且不会重新解码 ASR committed 文本。标点规则是基线 heuristic；未针对小数、缩写和日语引语优化。

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
