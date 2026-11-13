# Qwen3-ASR MLX 跑通与端到端耗时

日期：2026-11-02。目的仅为跑通和观察等待时间，不做pipeline优化或完整质量实验。

## 实际完成

Apple M4／24GB／arm64 macOS，Python 3.11.16、MLX及mlx-metal 0.32.4；社区 `mlx-qwen3-asr` 0.4.4固定到 `41878a11cf338c59e13edc84bf4cb35f4b0f3ff6`。独立uv环境见[pyproject](../experiments/online/envs/qwen-mlx/pyproject.toml)及lock。官方Qwen 0.6B／1.7B快照revision与逐文件摘要见[model-pins](../experiments/online/model-pins.json)，runtime加载为float16；没有做低位量化。

调用上游Session／transcribe，不启动HTTP ASR服务，不修改上游、不做VAD／第一遍或流式适配。输入为既有development节目：开场10–28s裁成18s WAV，另用原15–75s的60s WAV。它们是跑通输入，切点不保证完整句末，不重新声称是自然句末或人工gold。60s被runtime默认拆成3个chunk，18s为1个；所有8次有效请求均非空、eos、无token预算截断。未将输出做CER／质量评分。

原文直接通过既有 `harness.HyMT2` 交给本地Hy-MT2 1.8B Q4_K_M，temperature=0、max_tokens=1024，英文指令目标Simplified Chinese。复用已固定的llama.cpp `391fac16460f15233a7740550d858ac96df3419d`，Metal、ctx4096；Qwen和MT权重同时驻留，ASR完成后串行请求MT，没有并发优化。服务只监听loopback，模型／server摘要及日志留本地；两次自启服务退出码均为0，未修改正式App或工作区。

## 时间口径和结果

计时起点是完整WAV已存在、调用ASR；终点是完整原文或完整中文译文返回。ASR计时包含读取WAV、特征和解码，并执行Metal同步。E2E直接在同次调用链测量，包含本地HTTP请求／翻译，不相加不同轮次的统计值。

每模型两轮，每轮先18s再60s。首轮不是所有环节冷启动：模型已加载，60s请求前已跑18s；第二轮作为一次热重复。MT沿用默认server缓存，未强制清空，故热翻译耗时可能受已有前缀／调度影响。顺序固定0.6B先于1.7B，没有轮换或统计置信区间。

| 模型 | 音频 | 首轮ASR s | 首轮到完整译文 s | 热重复ASR s | 热重复MT s | 热重复到完整译文 s |
| --- | --- | --- | --- | --- | --- | --- |
| 0.6B | 18s | 2.655 | 3.059 | 0.756 | 0.365 | 1.120 |
| 1.7B | 18s | 1.683 | 2.106 | 1.682 | 0.358 | 2.039 |
| 0.6B | 60s | 2.462 | 3.895 | 2.508 | 1.200 | 3.708 |
| 1.7B | 60s | 5.910 | 7.380 | 5.889 | 1.333 | 7.221 |

模型加载与import分别为0.6B 1.606s、1.7B 1.388s，不含权重下载，也未混入表中的请求耗时。0.6B首请求有明显额外开销，不能用该单次反常次序推断1.7B普遍更快。runtime的MLX allocator累计峰值约3.14／8.16GB，是包含加载过程的进程内观测，不是纯权重大小或全系统／两进程总峰值。

表中不包括实时录音等待、VAD停顿确认、第一遍草稿、字幕上屏或App IPC。尤其60s整文件结果不能解释为从句首起只等3.7／7.2s，也不是实时会话端到端指标。本轮证明本地ASR→MT链路可用；1.7B在这些热样本上较慢，质量收益仍需实际听看，未据此选定最终模型。

## 复现

从仓库根目录：

```sh
uv sync --locked --project experiments/online/envs/qwen-mlx
```

使用该环境的 `hf download` 下载 `model-pins.json` 内指定repository和revision，仅需 `*.json`、`*.safetensors`、`*.txt`、`*.model`，分别放入忽略的 `local-artifacts/online/qwen-mlx-models/0.6B` 和 `1.7B`。不要使用浮动main替代已记录revision。

ASR-only示例（换用本机合法输入；输出目录必须不存在）：

```sh
uv run --locked --project experiments/online/envs/qwen-mlx \
  python experiments/online/run_qwen_mlx.py \
  --model-dir local-artifacts/online/qwen-mlx-models/0.6B \
  --audio /absolute/path/to/audio.wav \
  --output local-artifacts/online/qwen-mlx-new-run
```

默认repeat=2。启动现有本地Hy-MT2 server后，加 `--translation-url http://127.0.0.1:PORT/v1` 即可测同次ASR→译文；有鉴权时沿用 `ATOGAKI_ONLINE_LLAMA_KEY` 环境变量。服务准备／关闭由启动者负责；ASR脚本本身不启动服务器、不读系统凭据、不调用云端。换成1.7B目录并使用新的输出目录即可比较。默认未开启timestamps或forced aligner。

私有原文、译文、输入摘要、metadata、ASR中间结果、完整results和server日志保存在 `local-artifacts/online/qwen-mlx-smoke-20261102/`，不提交媒体／私人转写。下载约6.5GB官方权重与配置，未复制MT模型；旧模型及环境保留，待选型后统一清理。早期1.7B在下载尚未完成时被提前启动，缺少一个分片而失败；日志保留于 `1.7B.log`，完整权重后的有效结果在 `1.7B-complete/`，没有覆盖失败。

验证：实际Metal识别与loopback MT共8条完整链路、py_compile、CLI help、cargo fmt --check、git diff --check及本地文档链接检查。未接第一遍／VAD、未验证vLLM，也未做完整参考听审。本轮不增加参数矩阵或专门benchmark框架。
