# Voxt 本地 Qwen ASR 实现评审

日期：2026-11-14。在 [Qwen 8bit 完整链路回归](qwen8-online-regression-2026-11-14.md)之后，只读核对上游源码。结论是：Voxt 使用与本项目相同的 1.7B 8bit checkpoint，产品交付主要依靠音频与解码预算、预览／确认分工、runtime 管理、失败恢复和 App 打包。它另有可选的 DFlash 解码加速；其听写交付经验不能直接证明连续日语电台翻译的语义质量。

## 调查版本与模型

发行依据为 [Voxt 2.0.3](https://github.com/sgl-project/sglang-omni/releases/tag/voxt-v2.0.3)，发布于 2026-11-10，tag 解析到 `4e03928295bba17c8196644f50d69e58facff6ea`。同时冻结 main 快照 `13364eb45fd2ff09393fb0031576b2783a0edcf8` 用于参考，以下行为链接均钉在发行提交，不把 main 当成已发布实现。没有安装、构建或运行 Voxt；本轮没有 native 与 Python 的速度／质量对照。

发行[模型目录](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/models/catalog.go#L182)默认 `asr.accurate` 为 `mlx-community/Qwen3-ASR-1.7B-8bit`，revision `a8379a2e2f9e313c9292cdf1af4055ab56d50d55`。9 个文件的大小与 SHA-256 都和本项目目录及本机已安装包一致，权重文件为 2,463,307,541 bytes。没有发现一个专用于 Voxt、质量不同的 1.7B 主模型。相同权重不保证不同特征布局、MLX 版本或解码规则输出逐字一致。

## 如何组织听写

Voxt 是 App，SGLang Omni 提供其内部本地语音 runtime。Go 应用管理 native C++／MLX 服务和音频／会话，用户无需手工启动 Python 服务。[构建脚本](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/scripts/build-omni.sh#L2)把可执行文件、MLX dylib 和 Metal 资源装进 App；构建工具可以用 Python，不代表交付物需要用户准备 Python。

普通听写的流程是录音期间显示预览，停止后再确认剩余音频，随后按设置做文本整理／翻译并交付。原始识别、最终整理和耗时分别保留。长录音不是永远等到 Stop 才做所有工作：[live.go](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/omni/live.go#L323)会等后台确认片段完成，只识别剩余尾部再拼接。另有 `LiveResult` 模式可直接收实时结果，不能将所有功能都描述为停止后完整重识别。

实时预览调用 WebSocket `/v1/realtime`。默认首个可听音频后 100 ms 尝试解码、每新增约 1 秒音频刷新、当前段最多 30 秒；这些是调度目标，不是 100 ms 已有正确文字的承诺。[原生实现](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/sglang_omni_mlx/native/src/qwen3_asr/realtime.cpp#L262)在两次完整重解码后，有语言信息时保留已显示前缀、回退末尾 5 tokens 再续写。它仍对当前片段音频调用 Transcribe，后者重建 KV cache；不能称为跨刷新完全复用 encoder／KV 的真正增量识别。前缀策略主要减少重复生成并稳定显示，也可能保留错误前缀。

长于 60 秒的录音启动后台确认，[策略](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/omni/models.go#L153)以 30 秒为目标，在 20–45 秒范围寻找至少 300 ms 停顿，缺少停顿时选低能量窗口；[pieces.go](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/omni/pieces.go)管理已完成范围及失败回退。单次确认请求另限约 60 秒，切点在名义边界附近 ±5 秒内寻找安静的 100 ms 窗口，短尾部补至至少 1 秒；搜索窗口意味着不是严格每段不超过 60 秒。后台确认失败时保留音频供停止后的确认路径处理。它没有凭空消除计算，native 单 worker 在处理较大确认段时也会延迟预览。

本项目已经有草稿／确认两条 ASR 和可修订 MT。Voxt 的重要差别是，听写可以在用户停止这一明确意群边界交付，并容许预览被改写；连续电台没有这个交付时刻，还要承担日语到中文翻译、分组等待和错误传播。直接把本项目的 12 秒 VAD 上限换成 30／60 秒，会同时改变语义上下文和等待预算，不是本轮批准的实现。

## 解码、上下文与资源约束

[确认请求](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/omni/final.go#L47)明确发送 `stop_at_end_of_text`、`stop_on_token_loop` 与 generation metadata。token 预算为 `ceil(audio_seconds * 28) + 64`，限制在 256–8192。[解码器](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/sglang_omni_mlx/native/src/qwen3_asr/qwen3_asr_transcriber.cpp#L232)走 greedy argmax，使用异步求值让下一步 GPU 计算与主机停止判断重叠；检测循环后可提前停止并保留一次重复单元。循环处理有误删真实口语重复的风险，不能不加日语回归就复制。native 这个实现事实本身不提供比当前 Python 更快或更准的实测结论。

另有一个与本项目“草稿识别”不同的 draft：用户可从模型卡额外安装 [1.7B DFlash 辅助包](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/models/catalog.go#L339)，BF16 权重约 726 MB，供主模型校验一批候选 token。[App 在辅助包已安装时传给 runtime](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/models_omni.go#L83)；未安装则使用主模型。原生解码器有相应分支，启用 repetition penalty 时不走该分支。这是可选的推理加速机制，不是低质量字幕先上屏的 SenseVoice 草稿，也不证明能修正主模型误听。适合以后固定同一输入、主权重、解码口径和 MLX 环境再比较速度、内存、接受率与输出。

Qwen 1.7B 的词表提示最多 500 个 term／8000 字符，0.6B 最多 30 个；不无限累加历史全文。[echo.go](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/omni/echo.go#L161)发现疑似提示词表尾部回显时，会用同一音频、空 context 重跑作证据，再决定是否裁掉尾部。部分“确定回显”分支在重跑失败时也会裁剪，因此不能将它当无风险的正确性保证。我们目前 Qwen worker 未接入这套词表提示，本轮回归词表为空；它只为未来有依据地加入 ASR 提示提供参考，不会解决已经发生的 MT 错译。

MLX freed-buffer cache 明确设为 512 MB；请求限长与 cache 上限各自控制不同资源，512 MB 不是模型总内存。[上游内存调查](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/docs/eval.md#L273)记录长录音反复重解码造成 cache 膨胀，再用限长／限 cache 控制 footprint。这类长时稳态工程比模型文件大小更能说明产品是否可持续运行，但其 footprint 与我们前轮的 MLX 分配器峰值口径不同，不能直接拿数字排名。

## 进程恢复与分发

[Runtime](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/omni/runtime.go#L28)区分冷加载、活跃请求和闲置，默认启动预算 180 秒、闲置释放 10 分钟、无响应 stall 15 秒；持有 lease 的会话防止模型被闲置回收。确认请求响应有心跳，stall 监视看进展，不把一次较长识别简单当死锁；失联服务终止后标记可重试，预览失败不强制废掉最终识别。

[会话恢复](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/session/run.go#L435)在启用历史、存在音频等条件下，按设置保留音频，识别失败／取消也可限期留下供 Retry；不是默认无条件永久保存所有录音。[文本整理](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/internal/session/cleanup.go#L30)是可选 LLM 阶段，失败保留识别文本并告知降级。Translate／Rewrite 也有 LLM 路径，可以配置本地或远程；用户看到的润色成品不能都归给原始 ASR。

[CMake](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/sglang_omni_mlx/native/CMakeLists.txt#L10)固定 MLX 0.32.3，运行库／metallib 同包，使用 `@loader_path`；[Release workflow](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/.github/workflows/voxt-release.yaml#L181)处理 Developer ID 签名、公证与检查。发布脚本宣称最低 macOS 15.0，但本轮没有在 macOS 15 验证其依赖可运行，更没有证明本项目现有 MLX 二进制可运行。App 自管 native 或 Python runtime 都能继续使用 Release／DMG；选择内部 runtime 与是否拥有 Developer ID 是不同决策。

## 它的证据能给我们多少信心

上游[评测说明](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/docs/eval.md#L5)写明主要语料为 Qwen3-TTS 生成的中英文短句及拼接长句，缺少真实噪声、口音和自然犹豫；没有提供我们所需的日语多人电台到中文语义验收。其[长录音实验](https://github.com/sgl-project/sglang-omni/blob/4e03928295bba17c8196644f50d69e58facff6ea/Voxt/docs/eval.md#L249)在同一 4 分 46 秒合成输入上，用录音中后台确认将停止后等待从 32.3 秒降至 2.6 秒；这两个值指 Stop 之后，并非整段从说话开始到翻译交付，且文档披露交换内存压力和预览卡顿。它证明有可执行的产品工程路径，不证明日语语义已经解决。

对本项目的建议顺序：继续固定本次 1.7B 8bit 工作基线，先从已发现的两类语义错误定位边界；runtime 准备闭环时参考其模型清单、就绪检查、进程退出／重试和限期音频恢复；需要进一步节省算力时，再以同输入实验评估 native MLX 与可选 DFlash，而不是先重写整个 pipeline。cache、停止规则或 ASR 词表防回显也要分别测量，不将多项改动绑成一个无法归因的“优化”。这些是后续候选方案，本轮未迁移其代码或引入新模型。

源文件只读快照、逐文件摘要和 9 个模型文件的匹配结果保存在忽略的 `local-artifacts/research/voxt-20261114/provenance.json`，公共文档只保留固定版本链接和结论。
