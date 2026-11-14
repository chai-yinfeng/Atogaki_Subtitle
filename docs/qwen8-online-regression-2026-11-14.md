# Qwen 1.7B 8bit 在线基线回归

日期：2026-11-14。用户接受首批听审建议后，按 [0079](decisions/0079-qwen-17b-8bit-online-baseline.md)收敛在线确认识别默认。两段共 13 分钟的完整 worker 回放通过运行与关联完整性检查；中文语义质量仍未验收。

## 实际改动与条件

在线面板首次载入优先 `qwen-1.7b-8bit`，模型管理突出此档，其他 Qwen 收进可选项。刷新保留用户已选的其他可用模型；默认包未就绪时沿用既有可用模型回退。离线仍用原有 provider，没有把 Qwen 接入离线任务。已校验的 8bit 包以 APFS clone 导入本机受管目录，原包保留，未改数据库／旧 manifest，未覆盖 `/Applications/Atogaki.app`。

硬件／runtime 延续[候选筛选](qwen-radio-candidate-screen-2026-11-13.md)：Apple M4、24 GiB、macOS 27.0.1 arm64；Python 3.11.16、MLX／mlx-metal 0.32.4、mlx-qwen3-asr 0.4.4、sherpa-onnx 2.0.1。代码父提交为 `8bf91248dc017db69edb839a34c310987511f9de`；worker 未改，本次 UI／目录改动的实际文件摘要也在运行前冻结。

- 确认 ASR：目录固定 revision 的 Qwen3-ASR 1.7B 8bit，9 个文件逐一校验；语言 Japanese。
- 草稿：现有日语 SenseVoice FP32；Silero ONNX 使用生产默认边界。
- MT：本机 App 的 llama-server，Hy-MT2 1.8B Q4_K_M，`--ctx-size 8192 --jinja --n-gpu-layers 99 --parallel 1`；模型和二进制摘要冻结。默认预译开启，词表和场景为空。
- 输入：Vol.34 前 600 秒，包含此前 radio-11 的 180–360 秒；radio-06 为另一节目 600–780 秒的已知问题片段，共 180 秒。输入均 PCM16／mono／16 kHz，源媒体及 WAV 的 SHA-256 核对；是开发回归材料，不是未见保留集。
- 通过生产共用 `online_worker` 以 1 倍速送入 WAV，不给未来音频、参考或人工校正原文。每段一次，MT server 跨段保温，ASR 每段重载、加载结束后启动音频时钟。EOF 沿用 30 秒 drain，不改评估预算。

本轮使用当前桌面默认 Hy-MT2，不同于早期 Index-Translate 实验；没有同条件原始 1.7B A/B，不能把前后变化全归因于量化。

## 结果

延迟单位为秒。ASR 返回相对该段音频末尾；译文定稿相对组内第一／最后一个原文段的音频末尾，均包含现有等待、分组和调度。译文统计排除每段的一个 EOF 组，避免把节目停止后的提前封组混进稳态。

| 指标 | Vol.34 前 10 分钟 | radio-06 三分钟 |
| --- | ---: | ---: |
| 完整确认原文／最终译文组 | 114 / 65 | 36 / 18 |
| 确认队列峰值 | 1 | 1 |
| ASR 返回 p50 / p95 | 1.120 / 2.192 | .919 / 2.271 |
| ASR 排队 p50 / p95 | .050 / .917 | .00024 / 1.533 |
| ASR 计算 p50 / p95 | .380 / 1.526 | .297 / 1.276 |
| MT 请求返回 p50 / p95 | .387 / .813 | .291 / .698 |
| 定稿距组内首段末尾 p50 / p95 | 6.880 / 11.058 | 6.531 / 10.586 |
| 定稿距组内末段末尾 p50 / p95 | 3.576 / 9.551 | 3.015 / 10.586 |
| MT 拒绝／失败组／未完成组 | 0 / 0 / 0 | 0 / 0 / 0 |

MT 请求计量分别为 630／171 次，包括预译、修订与最终请求；它是调用返回时间，不能等同于纯 GPU 内核时间。150 条确认原文、83 组最终译文全部完成；封闭原文无漏确认，最终组对源段恰好覆盖一次，revision 精确对应、边界合法、译文非空、事件序号递增、无 worker error，两次进程正常退出，独立启动的 server 已回收。

用现有 `experiments/online/replay_presentation.mjs` 对事件再做生产展示逻辑回放，分别生成 829／232 个快照。草稿门控会跳过部分中间版本，这不等于丢失最终结果。没有运行系统音频采集、Tauri IPC 或原生窗口绘制，返回时间不能称作真实上屏时间。RSS 采样也不是 Metal 分配器／统一内存总峰值；本轮不据此报告整体内存结论。

## 语义观察与下一步

抽查此前听审锚点及其完整翻译组，仍发现至少两类问题：一处 ASR 的同音／误词让中文变成不相关的日常活动；另一处源组已有明确内容，MT 却替换成无依据的评价。人工建议原文的不确定处继续保留，不把它们当逐字 gold。某些看似多出的内容实际来自同组后续原文，核对完整组后不记为幻觉。

因此本轮支持“以 1.7B 8bit 继续开发，现有实现能在本机跟上这两段 1 倍速输入”，不支持“日语准确率已达标”或“所有问题来自算力”。ASR 确认 p95 约 2.2 秒，译文定稿距组内首段末尾 p95 约 11 秒；下一轮先保留这个基线，分别定位 ASR 误词传播与 MT 无依据改写，避免同时换模型、分组和提示后失去归因。Voxt 的只读实现调查见[源码评审](voxt-local-asr-review-2026-11-14.md)。

## 证据与验证

私有音频、逐句输出、参数、哈希、日志、事件和资源样本留在忽略的 `local-artifacts/online/qwen8-baseline-20261114/`，未提交语料或用户备注。首次运行命令为 `python3 local-artifacts/online/qwen8-baseline-20261114/run_regression.py`；它是冻结本机路径的单轮控制器，拒绝复用已有输入目录，复跑需要新产物目录。核心调用是私有 runtime 的 `python3 -s -B -u -m online_worker --audio <case.wav> --language ja --draft sensevoice --draft-model <installed-ja> --final qwen --final-model <installed-qwen8> --vad-model <installed-vad> --translation-url <local-loopback-v1> --translation-drafts`，由环境传入 MT context 和临时鉴权；密钥不写盘。汇总复用现有 `summarize_two_pass`，显式传入两个 case ID，避免仅匹配 `radio-*` 漏掉长片段。

冻结控制器 SHA-256：`65f674404b81d6a4cb7ee8b3692d0c1593edf37d573c8c2715ee083791f388db`；汇总器：`9c2afebe4a99b039ebacb9175ed81e9192c16d32fa836ba66c67bb271a4d43a1`；聚合检查报告：`b5d83d045829d14d9b84c80cb7338fae2daf501765529b308b64567e5d8efa01`。

默认收敛代码的验证：前端 372 项、生产构建、Tauri 8 项模型下载测试通过；WebKit 验证初始 8bit 选择、实际开始参数、手动选择刷新保留和模型分层入口。两套 Rust fmt、diff 与 Markdown 引用检查通过。上述合成 UI 检查与真实 worker 回放分别提供证据，未合称真实桌面 E2E。
