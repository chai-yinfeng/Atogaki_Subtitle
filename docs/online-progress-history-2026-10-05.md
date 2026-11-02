# Online进度历史：截至2026-10-05

2026-11-02从路线图原样归档。未勾选项是当时待办，当前取舍见[online计划](online-asr-plan.md)，不要求全部完成后才开始新选型。

## 3. 实时辅助

2026-10-02 确认独立 online 路线：Hy-MT2 翻译，先比较 Whisper-Streaming 与 SimulStreaming。Session／调度／未来 GUI 与 offline 分离；全文 offline 精修另建 run。见 [0045](decisions/0045-independent-online-experiments.md) 和 [CLI 实验说明](../experiments/online/README.md)。

- [x] 建立上游回放 runner、版本化输出适配、Hy-MT2 串行翻译策略重放与基础回归。
- [x] uv 管理两套独立 Python 3.11.16 环境并提交 lock；固定上游 commits，完成同一 12 秒日语片段的 tiny 模型真实回放冒烟。
- [x] 第一轮正式 large-v3 实验：同一 60 秒开发样本完成 WS／MLX 与 SS／CPU 顺序回放及 Hy-MT2 1.8B 四组翻译策略重放；见 [验证记录](online-large-v3-validation.md)，无人工 gold，不作准确率结论。
- [x] 按用户授权完成一次 Gemini 3.5 Flash 候选段落文本翻译，生成原文／Gemini／Hy-MT2 校对材料；尚未听审，不作为 gold，未上传音频。
- [x] 2026-10-04 完成候选问答的视觉字幕核对、一次授权 Gemini Transcribe、WS／SS large-v3 与 Hy-MT2 1.8B／7B／Gemini 翻译对照；保留字幕与听感分歧、音乐开场失败及模型错误，见 [验证记录](online-paragraph-reference-validation.md)。holdout 仅作评估，未用于调参。
- [x] 2026-10-04 补齐 whisper.cpp stream policy 文件适配版，统一真实时钟／draft／commit／flush 指标和 Silero 因果 VAD；完成三套 ASR 的 off／gate／endpoint500 及 CPP 阈值／step 共 12 组 development 回放，本轮不运行 Hy-MT2。见 [ASR／VAD 验证](online-asr-vad-validation.md)。
- [x] 2026-10-04 梳理 [online 实验控制与指标合同](online-experiment-contract.md)：区分模型／runtime／提交策略与解码调度，列明当前硬编码参数和未实现的有效 draft／稳定显示延迟；Qwen-Audio-3.0 streaming、开源 Qwen3-ASR、Qwen-MT 作为不同候选记录，尚未调用或接入。
- [x] 2026-10-05 实现 v3 受控 ASR runner：官方同源 small／base／tiny 未量化转换、SS MPS／FP16 局部适配、32ms 独立因果供给、单消费者解码、显式尾部动作和 live Silero worker；resolved config、source／lock／model／runtime 精度审计与 reference-aware 指标分开保存。见 [0046](decisions/0046-controlled-online-asr-performance.md)。
- [x] 按用户授权完成四段 development 共240s的 Gemini 3.5 Transcribe，四个远端临时文件均删除；生成20个本地候选听审片段，语义／逐字／声学时间仍待分别核验。
- [x] 完成 v3 18组同源 small／base／tiny 顺序筛选：17组完成，SS small／1s 预算退出；固定 small／500ms 为临时资源对照配置，不作为质量胜者。短语听审改用连续节目前后文，保留用户原备注，边缘后缀差异不直接算识别错误。
- [x] 完成 v3 80组模型回放与3组实际解码中取消：三轮完整 development（SS 一轮 Unicode 异常）、冻结 holdout、off／live 十分钟积压、12组 VAD／尾部消融和33组边界。39项回归通过，失败与未核验参考保留；见 [受控验证报告](online-controlled-asr-validation.md)。
- [x] 解耦 VAD classifier／解码暂停与尾部 decode／finish／reset，保存同源概率及 live 计算开销；不默认短暂停重置。正式共享模型／offline sidecar 的摘要、大小和mtime核验未变化。
- [ ] 修复长静音恢复的 runner 调度缺陷：当前每次最多30s追赶未解码 PCM，live 长回归 WS／CPP 的首次输入恢复分别53.076s／4.720s。积压增长下降不等于实时通过；增加独立恢复诊断，尚未决定如何分离完整录音保存与有限 ASR 声学上下文，不用跳音频／填零／reset 改写本轮结果。
- [ ] 完成20个连续上下文锚点的语义／声学核验、逐候选关键语义及独立 large-v3／holdout 审核；当前不选生产胜者。SS MPS 完整路径可运行，但small长样本开销、Unicode分组和原生provider时钟仍需处理。选定后只提取一套 adapter／session／VAD，清理未选且可重建的产物，保持 offline／GUI 分离。
- [ ] 在 development 样本增加音乐开场、交叠说话、专名与过去经历回归；验证 VAD／会话重置、异常 EOF 和专名上下文，再进行独立 holdout 验收。
- [ ] 在线翻译阶段的自然句末、分组差异、Hy-MT2 否定与截断句处理仍待验证；本轮 ASR 已完成独立短节目／长样本执行，翻译和并发资源竞争未测。
- [ ] 媒体时钟驱动的并发翻译、过期响应、背压、取消、gap 与 EOF 回归；当前串行重放不能作为实时性能结论。

实施顺序见本轮 P5：先录音流式回放与提交策略，再接入采集。candidate／stable／committed 和离线精修 revision 分开；Muse Voice Transcribe、SeamlessStreaming、SimulStreaming 分别研究。

- [ ] macOS 录音与系统音频采集方案。
- [ ] 分段录制与低延迟粗转写。
- [ ] 会话结束后独立运行全文 offline 精修；自动调度方式待定。
- [ ] 清晰区分临时结果和最终结果。
