# 0045：独立 online 会话与 Hy-MT2 翻译实验

日期：2026-10-02

状态：accepted（边界与探索顺序；模型效果待实测）

## 决策

用户确认 online 与既有 offline 两段式工作流分离。Online 首先比较 Whisper-Streaming 的 LocalAgreement 与 SimulStreaming 的 AlignAtt，translation backend 使用 Hy-MT2。SeamlessStreaming 暂不进入第一轮实现，不自行训练。

Online 的 session、音频时钟、活动文本、显示 revision、commit policy、翻译调度和回改窗口独立于 offline 的任务、cue、TranslationPlanner、编辑状态与 SQLite 写回。共享基础设施限于媒体工具、模型文件／下载校验、runtime 进程管理和本地 transport 等实际稳定的边界；不为复用而要求 online 实现 offline cue 合同。未来 online GUI 单独设计，当前只有 CLI 实验。

Online 原文与译文各自维护版本与提交状态；已显示不等于已提交。保留上游 append-only 输出的真实语义，不能把它包装成可回改的 ASR hypothesis。活动组译文可整体替换；第一阶段以日志重放验证该策略，尚未支持 ASR 已提交前缀重新解码。

会话结束后可把完整录音交给独立的全文 offline ASR → translation run。该过程重新识别完整音频，保留与 online session 的来源关联，不将 online 文本当成最终原文，不覆盖 online 原始证据。自动调度与采用策略稍后实现，当前不要求自动运行。

## 首个里程碑

`experiments/online/` 提供上游文件回放启动、输出适配、Hy-MT2 本地 HTTP 翻译和指标。实验产物位于 `local-artifacts/online/`；不读取正式工作区、不修改 GUI 或 offline provider。

串行翻译重放用于比较 boundary／revisable 策略，不能证明实时端到端延迟、背压或 ASR／翻译并发资源竞争。上游 runtime 和模型需独立安装；记录实际 commit、参数、音频摘要和状态。后续再从经过验证的实验中提取生产接口，不提前引入通用 pipeline 框架。

## 替代方案

把 online 状态加入离线 cue 与 TranslationPlanner 可少写适配代码，但会把等待／替换语义与持久编辑、人工修订耦合；不采用。完整复制 runtime／下载基础设施同样不必要；研究脚本允许最小 transport 原型，生产化时共享基础设施。

本决策细化 0043 的实时边界，并限定 0044 中 provider／planner 复用要求为兼容的业务模式；offline CLI 仍复用原有用例，online 不强制依赖 offline planner。
