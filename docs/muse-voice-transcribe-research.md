# Muse Voice Transcribe 流式 ASR 研究

_核对日期：2026-09-17；主要资料为 Meta 官方发布说明。_

Muse Voice Transcribe 是 Muse Spark 系列的 autoregressive multimodal streaming ASR。Meta 公布的能力包括实时 ASR、20+ speaker diarization、endpointing、code-switching，以及 language、keyword、context biasing。模型训练覆盖 70+ 语言，其中 25 种经过重点验证，日语在验证语言列表中。官方发布页称可通过 Meta Model API、Meta AI for Mac 和 Muse Code 使用，但当前公开说明不足以冻结 Atogaki 所需的 websocket 消息协议、时间戳粒度、错误恢复合同和正式数据条款，因此 P4 不直接接入。

官方资料：[Introducing Muse Voice Transcribe](https://research.meta.ai/blog/introducing-muse-voice-transcribe)。

## 流式生成机制

音频以 80 ms chunk（12.5 Hz）进入模型，每个 chunk 形成一个 soft token。模型在每一步自行选择输出文字 token，或输出 `<|next_audio|>` 继续等待下一段音频。流结束时，调用方插入 `<|empty_audio|>`，模型据此排出剩余文字，且不再请求下一段音频。

这个设计把等待时长交给模型。简单词可以较早输出，需要消歧的词等待更多声学上下文。训练时将 WER reward 和 delay reward 结合，使模型学习 per-word adaptive delay；Meta 用 time to final transcription 衡量速度与准确率的折中。

Diarization 和 endpointing 与文字共享同一条 token stream：

- `<|start_of_turn|>` 在可能发生 speaker switch 时尽早出现。
- `<|speaker_A-Z|>` 延迟到该 chunk 末尾才确定身份，因此 turn 边界和 speaker identity 不必同时提交。
- `<|speech_onset|>` 与 `<|speech_endpoint|>` 表示语音开始和说话结束，是独立于纯声学 VAD 的语义事件。

## Atogaki 可以借鉴的部分

1. 实时状态仍采用 provider-neutral 的 `candidate → stable → committed → offline-refined`。Muse 的文字 token、turn、speaker、onset 和 endpoint 只作为事件输入，不能直接成为 UI 的最终状态。
2. commit policy 应支持 adaptive delay。LocalAgreement 可先作为无模型内部信号时的通用策略；如果 provider 给出更强稳定性事件，policy 再缩短特定词的等待时间。
3. speaker 使用 late binding。文字可以先稳定，speaker label 后补；不能因为 speaker 尚未确定而阻塞整句，也不能在 speaker 更新时重新生成 cue ID。
4. endpoint 是一等事件。它可以帮助 translation group 和 subtitle cue 收口，但不等同于 VAD 切块，二者都应保留证据来源。
5. 流结束必须显式 flush。回放 harness、断线恢复和真实采集都要测试等价于 `<|empty_audio|>` 的结束语义，防止最后一句静默丢失。
6. 指标使用 per-word time-to-final、稳定延迟和回改率，首 token 延迟只能描述响应速度的一部分。
7. context/keyword biasing 与 Atogaki 词表直接相关，但必须记录实际发送的词表 revision，不能只在会话级隐式追加。

## 接入前仍需核实

- Meta Model API 的认证、endpoint、音频编码、chunk/ack、背压、重连和结束消息。
- provider 是否返回 word timestamps、token timestamps，还是只有按事件到达时间推断的近似时间。
- speaker 修订是否带稳定 ID，重连后如何去重。
- 音频和 transcript 的保存期限、训练使用、区域与删除机制。发布页中“audio is not stored”只描述页面 demo，不能外推为 Model API 的合同。
- 日本语、多人重叠和一小时节目在真实 API 上的限额、费用和稳定性。

在这些信息可由官方协议或真实调用验证前，Muse 保持 P5 research provider，不出现在正式 ASR provider 选择中。
