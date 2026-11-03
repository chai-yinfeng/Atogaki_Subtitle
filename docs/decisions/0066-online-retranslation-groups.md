# 0066：独立于ASR提交状态的在线译文回改与定稿

日期：2026-11-02。状态：accepted，用户确认按讨论推进pipeline；当前实现和验收范围为CLI实时文件回放，尚未进入桌面会话。

## 原因与选择

ASR前缀稳定不能证明译文前缀稳定。即使原文识别完全正确，日语句末否定等后续信息仍可能改变中文前部。因此不将Whisper-Streaming的LocalAgreement／committed状态直接当成MT提交策略，不逐VAD段独立翻译并立即冻结。保留本地草稿→Qwen整段定稿的two-pass ASR，MT独立维护可整体替换的活动翻译组。

已讨论的替代方案包括Google re-translation的mask-k／biased beam search、revision-window约束、候选一致前缀及stable-prefix训练。后几者需要解码控制、多候选成本或专门训练，不能假定现成Hy-MT2或通用云端API具备这些能力。当前先采用普通完整文本请求的re-translation，后端不承担硬性前缀约束。相关工作：[EACL 2023 self-training](https://aclanthology.org/2023.eacl-main.270/)、[ASRU 2023 revision-controllable decoding](https://arxiv.org/abs/2310.04399)、[ALTA 2024 RALCP](https://aclanthology.org/2024.alta-1.7/)、[WMT 2026 stable-prefix training](https://aclanthology.org/2026.wmt-main.15/)。

## 事件和边界

原文继续按音频segment维护source_revision、source_draft／source_final；译文使用独立group_id、group_revision、source_revisions映射、context_revision、request_id及实际source_text。translation_draft可以覆盖组内全部译文，不提交中文公共前缀。translation_final需要组成员已封口、所有成员取得最终ASR，并精确匹配这些revision。source_final仍仅表示模型结果，不代表句意完整或人工核准。

短VAD停顿只关闭声学段。翻译组允许跨段，初版曾用句末标点＋0.65s等待快速收口，质量检查发现模型会给未完成短语补句号；用户反馈确认优先改善MT上下文，现已取消这条规则。当前以1.2s长停顿、20s／400字符上限（在VAD段边界）及EOF收口；ASR分段不变。这些是启发式工程边界，不是语义完整证明或调优结论；时长／字符上限可超出至当前声学段结束（VAD当前最大12s），ASR积压也增加完成时间。强制收口保留completion_reason，不宣称句子完成。分组成员收口后等待最终ASR，不能提前冻结草稿译文。

## 调度与上下文

CPU草稿与音频时钟继续独立。Qwen与本地MT在同一worker串行调度，待处理Qwen段优先；MT一次一个请求，活动状态合并为最新快照，草稿最短请求间隔0.8s，不保存过时草稿请求FIFO。已在执行的HTTP生成不会中断，因此一次MT仍可能推迟新到Qwen任务。已封口组等待ASR齐备后直接请求最终翻译，不继续生成该组旧草稿。

译文按音频组顺序提交，只读最近一个已提交组作上下文，原／译分别最多1000字符；不跨历史无限回改。上下文使用[Hy-MT2官方background/source模板](https://huggingface.co/tencent/Hy-MT2-1.8B)，参考信息放在待译指令之前，不能将其拼进待译段。模板有效性仍需真实样本检查，不能靠协议验证保证翻译质量。

请求返回时若原文已变化，旧结果只允许以携带实际输入revision及stale_source的草稿显示；下一次请求追上最新状态，不把旧结果升级为定稿。相同原文／上下文可复用最近一次翻译，状态从草稿到定稿本身不触发重复生成。当前会话后端／配置固定，不支持中途热换。

## 实现和限制

[共享retranslation模块](../../src/infrastructure/online_worker/retranslation.py)将分组／版本／请求状态与模型运行分开；[run_two_pass.py](../../experiments/online/run_two_pass.py)接入真实时钟、SenseVoice／Zipformer草稿、Qwen最终ASR和Hy-MT2。默认启用译文草稿，`--no-translation-drafts`保留只翻译收口组的对照模式。当前仅本地HTTP翻译，无新云端调用或隐式回退。共享worker位于`src/infrastructure/online_worker/`，研究CLI为薄入口；`--pcm-stdin`支持单声道PCM16／16kHz，以EOF停止并排空。实时默认只输出JSONL，不写录音或事件文件；WAV回放与桌面共用同一识别／分组代码。音频环形缓冲30s，最终ASR队列最多8项；超负载可能反压输入，采集侧必须显式报错停止，不能无限积压。

不在本轮实现显示层mask-k、beam约束、前缀训练、系统音频采集、App UI或音频保存；模型worker打包、多文件模型管理仍是桌面接入待办。WAV回放也改为逐帧读取并使用同一环形缓冲。管道的音频时钟起点按首帧到达估计，与模型／权限等待分开，不冒充硬件时间戳。慢后端的最终任务积压、失败恢复和长会话仍需产品化，当前失败会终止试跑并记录worker_error。

后继于[0064](0064-local-drafts-and-segment-finalization.md)的逐VAD段MT最小demo；不改变其旧实验事实。运行证据与局限见[链路记录](../two-pass-smoke-validation.md)。
