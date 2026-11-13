# SenseVoiceSmall sherpa-onnx 跑通与对照

日期：2026-11-02。按用户要求继续做最小跑通与端到端观察，不展开pipeline优化。用户目前更偏好Qwen3-ASR 1.7B的质量；这是用户对现有输出的判断，不等于本轮已有统一gold评分。

## 实际配置

Apple M4／24GB、Python3.11.16、sherpa-onnx及sherpa-onnx-core 2.0.1，独立uv环境见[配置](../experiments/online/envs/sensevoice/pyproject.toml)及lock。使用sherpa官方2024-07-17五语言SenseVoiceSmall发布包里的 `model.onnx`（FP32），CPU、默认单线程、language=ja、use_itn=True、greedy_search。文件／archive摘要见[model-pins](../experiments/online/model-pins.json)。包中的INT8模型只保留，没有运行量化矩阵。

通过上游 `OfflineRecognizer.from_sense_voice` 调用，每份WAV完整供给一个stream，读取PCM、float32归一化、特征／decode均在ASR计时内；未接VAD或另做分段。复用前次[Qwen输入](qwen-mlx-smoke-validation.md)的18s／60s WAV，逐文件SHA-256与Qwen metadata完全一致。Qwen前次60s由runtime拆成3块，SenseVoice本次不拆；这是可用系统配置的比较，不是同算法或同分段控制实验。

同样通过既有 `harness.HyMT2` 接本地Hy-MT2 1.8B Q4_K_M，llama.cpp固定commit与前次一致，Metal、ctx4096、temperature=0、max_tokens=1024。两模型常驻、ASR→MT串行，不优化并发／分组／缓存。完整WAV已就绪后开始计时，完整中文返回为终点；不含音频采集、VAD静音等待、第一遍或字幕UI。加载import另记1.215s。各跑两轮，顺序18s→60s；本次4条链路均返回非空日文及完整stop中文。

## 结果

| 模型／配置 | 输入 | 热运行ASR s | 热运行MT s | 热运行到完整译文 s |
| --- | --- | --- | --- | --- |
| SenseVoiceSmall FP32／CPU1线程 | 18s | 0.283 | 0.332 | 0.615 |
| Qwen 0.6B FP16／MLX（前次） | 18s | 0.756 | 0.365 | 1.120 |
| Qwen 1.7B FP16／MLX（前次） | 18s | 1.682 | 0.358 | 2.039 |
| SenseVoiceSmall FP32／CPU1线程 | 60s | 1.211 | 1.381 | 2.593 |
| Qwen 0.6B FP16／MLX（前次） | 60s | 2.508 | 1.200 | 3.708 |
| Qwen 1.7B FP16／MLX（前次） | 60s | 5.889 | 1.333 | 7.221 |

SenseVoice首轮18s为ASR0.280s／到译文0.683s，60s为1.281s／2.845s。当前样本上SenseVoice更快，但不能据此宣布质量胜出；不同原文导致不同译文／token数，MT耗时也不能单独归因于ASR速度。沿用server默认缓存，未清空前缀；Qwen数据来自前次而非同轮随机交错，不提供统计置信区间。SenseVoice进程观测最大RSS约1.50GB，不含独立MT进程，不与MLX allocator峰值当作同口径比较。

阅读输出可见名词／短语和句尾处理存在差异，且会影响中文语义；私有目录中的review.md提供同音频、三模型原文和译文，供回听。未作逐字听审，不计算CER或给正式准确率排名。

## 复现与保留

```sh
uv sync --locked --project experiments/online/envs/sensevoice
uv run --locked --project experiments/online/envs/sensevoice \
  python experiments/online/run_sensevoice.py \
  --model-dir local-artifacts/online/sensevoice-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17 \
  --audio /absolute/path/to/mono-pcm16-16khz.wav \
  --output local-artifacts/online/sensevoice-new-run
```

模型从[官方发布包](https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-sense-voice-zh-en-ja-ko-yue-2024-07-17.tar.bz2)解压，核对model-pins内摘要。默认repeat=2；已有本地Hy-MT2 server时加 `--translation-url http://127.0.0.1:PORT/v1`，鉴权沿用 `ATOGAKI_ONLINE_LLAMA_KEY`。未提供该参数则只测ASR。

原始日文、译文、输入摘要、metadata和server日志在忽略目录 `local-artifacts/online/sensevoice-smoke-20261102/`；所有隐私原文和媒体不提交Git。加载／调用只读使用共享Hy-MT2模型，自启服务已正常退出（returncode=0）。旧环境／模型、下载包与本次产物暂保留，统一清理仍在选型完成之后。本次不改正式App、模型目录或SQLite。

验证：4条实际ASR→MT链路、输入摘要一致、脚本py_compile与CLI help、cargo fmt --check、git diff --check和公开文档本地链接检查。无新增参数搜索或mock质量测试。

## vLLM状态：按用户要求搁置

本轮只读取并固定了官方源码checkout `QwenLM/Qwen3-ASR@7c6daf77a2421100f5fb066495372c00129d39ff`，其extra指定vLLM0.14.0。已知有进程内与HTTP接口，但没有执行GPU推理或服务请求，不声称接口实机通过。

用户询问本机无可用vLLM GPU环境时是否能先搁置，并明确选择先搁置。本机有Apple Metal GPU，但不是上述vLLM GPU路径；没有提供可访问的测试GPU主机或endpoint。CPU路径与其他runtime不是这条GPU部署的性能证明。停止安装、运行脚本和依赖准备，当前提交只包含SenseVoice代码及该暂停状态；不把vLLM继续作为下一步自动待执行项。未来用户要求恢复时再按实际设备确认运行路径。
