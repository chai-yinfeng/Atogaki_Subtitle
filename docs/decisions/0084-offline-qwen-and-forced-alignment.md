# 0084：离线 Qwen 与强制对齐

日期：2026-11-17。状态：用户明确要求补齐离线 Qwen，并确认首版增加 ForcedAligner 提供完整字幕打轴。源码接入与安装验证分别记录，不将功能冒烟当语义验收。

## 链路与配置

复用已有 Qwen MLX Session、AsrProvider、LocalTaskService、120 秒归属块／5 秒上下文检查点和字幕分段／导出。新增 QwenAsrProvider，先识别原文，再用官方 Qwen3-ForcedAligner-0.6B 生成词／字符时间；时间来源写为 forced_alignment。Qwen 不使用 Whisper GGML VAD，不自动加载 DFlash，不把在线预览状态作为离线资料。0.6B 完整／4-bit、1.7B 完整／8-bit 模型可从已安装目录选择；本轮真实裁片仅验证 1.7B 8-bit。

选择 Qwen 后任务冻结 model 与 alignment_model，二者内容都进入配置摘要；原有 Whisper 配置没有 alignment_model 字段时保持旧序列化与哈希合同。队列按保存的配置选择 provider，不把运行环境路径放进任务 JSON；Qwen 和本地翻译／在线共用设备互斥。暂停使用现有 future 取消与 kill_on_drop，继续／重试不能把缺失 Qwen 静默替换成 Whisper。局部／整段候选沿用任务模型，云端仍需原有明确上传授权。Qwen 首版完成识别／分段后再翻译；不使用现有基于 provider-segment ID 的云端提前翻译缓存，避免将词级单位逐词请求或与最终字幕 ID 错配。Whisper 原有提前翻译保留。

任务页识别模型是基础下拉项，按已安装／已配置模型显示；路径不再作为任务输入 UI。VAD 与词表进入高级项，Qwen 隐藏 Whisper 专用 VAD。设置中仍保留已有本地 Whisper 文件配置，新导入的完整命名注册表尚未新增。主进程再次解析模型 ID、检查受管文件、对齐模型与运行环境；不能只相信前端传入路径。没有 Whisper 但完整 Qwen 已就绪时，不再由启动引导强制要求 Whisper。

## 对齐投影

上游对齐会清除标点，并可能将极短词映射到同一个时间格。保留原始识别／对齐 JSON，用文字对应关系将标点归还原文；对零时长和相交词只在已有边界合并，保留所有文字，不插值补时间、不丢词。截断、文字错配、反向／非有限时间、全部无有效时长均拒绝。原始证据与生成字幕分开保留。

桥接器直接读取已有 16kHz／mono PCM16 WAV，向 MLX 传数组，避免其文件加载路径再调用 PATH 中的 FFmpeg。模型／音频／输出目录传绝对路径；Python 不依赖 CLI 的相对工作目录。模型推理设置离线模式，不在运行中隐式下载。

## 模型与运行环境

模型目录加入“字幕对齐”类别，固定官方 revision c7cbfc2048c462b0d63a45797104fc9db3ad62b7，权重约 1.84 GB，所有文件由原下载队列逐项 SHA-256 校验。对齐是明确新增的依赖，不使用 VAD 粗边界冒充完整打轴。

日语／韩语分词复用 upstream 的 nagisa／soynlp，按现有 uv 模式在 experiments/online/envs/qwen-alignment 固定版本。prepare-qwen-alignment.py 从已验证的 site-packages 导入独立 runtimes/qwen-alignment，拒绝覆盖；不替换原在线 Python 包。App 在缺少运行环境、对齐依赖标记或模型时禁用 Qwen 选项并说明缺项。全新机器的 Python runtime 自动安装仍未完成，不因本机导入成功宣称通用分发闭环完成。

CLI 使用已有 transcribe 命令新增 --engine mlx-qwen3-asr --python … --alignment-model …，仍用 --model 指定识别包；Whisper 默认为原路径。

## 验证范围

新增投影回归覆盖标点、零时长／重叠合并及失败拒绝；Rust 队列回归确认 Qwen 分派、冻结配置和 SRT；生产 WebKit 确认基础选择、不可用选项、VAD 分离、提交模型 ID 和偏好恢复。原有核心／Tauri／前端回归分别执行。

核准 radio-01-012 的隔离功能运行产生 50 个对齐单元、3 个字幕并导出 SRT；完整检查点副本使用不存在的 Python 路径再次成功，确认无需重复推理即可复用；初次相对路径／外部 FFmpeg 和过严零时长处理的失败证据保留。没有扩充语料或计算质量分数，debug 配置摘要计算包含大模型全文件哈希，不能用该运行给出产品时延结论。私有日志／输入／产物在 local-artifacts/evaluation/offline-qwen-20261117。

参考：[Qwen 官方对齐器](https://github.com/QwenLM/Qwen3-ASR#forcedaligner-usage)、[MLX 实现](https://github.com/moona3k/mlx-qwen3-asr)。
