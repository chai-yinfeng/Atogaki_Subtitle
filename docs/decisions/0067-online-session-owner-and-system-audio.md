# 0067：在线会话拥有采集、推理进程与设备时段

日期：2026-11-02。状态：沿用户确认的桌面推进方向实现首个会话底座；未交付桌面可用候选。

后继：[0068](0068-online-desktop-controls-and-live-context.md)已接控制面板／字幕窗、受管本地MT，并按用户要求覆盖本机Atogaki；下文保留底座阶段的证据边界。

## 结构

用户反馈确认本轮优先补足MT上下文、保留现有ASR分段，然后继续推进桌面会话。[0066](0066-online-retranslation-groups.md)取消模型句号触发的快速提交，CLI和桌面使用同一个`online_worker`，避免把研究回放脚本直接当成另一个产品服务。

Rust `OnlineSessionService`拥有会话ID、状态、模型worker、系统音频helper和进程间PCM管道；窗口只订阅`online-session-event`，不拥有采集。先加载模型，收到session_ready后才启动采集。状态依次为loading、requesting_audio、running、stopping／draining、stopped或failed；所有事件带sessionId，与离线播放浮窗状态隔离。当前只建立IPC（status／start／stop），未增加控制面板或在线字幕窗。

输入为单声道PCM16／16kHz；样本序号是媒体坐标，首帧到达估计的音频起点与模型加载／权限等待分开，不将其称为精确硬件时戳。worker默认不传`--output`，不保存音频、JSONL或正式SQLite任务；音频环形缓冲30s，VAD自身保留有界缓冲，最终ASR快照队列最多8段。译文以独立group／source revisions提交，不把source_final视为翻译完成或人工核准。

## 采集与停止

原生Swift helper使用[Apple Core Audio Process Tap](https://developer.apple.com/documentation/coreaudio/capturing-system-audio-with-core-audio-taps)，单独以arm64／macOS14.2为目标构建，不提高离线App的12.0最低要求。private tap-only aggregate只读取系统输出，不把物理麦克风设备加入aggregate；AVAudioConverter明确混合立体声至单声道并重采样。保持系统输出不静音。helper和App均提供NSAudioCaptureUsageDescription，由系统完成音频权限流程；首次授权／拒绝的实际App交互仍待验收。

helper stdout仅输出PCM，stderr输出结构化状态。stdin关闭／收到停止字节及SIGTERM／SIGINT均触发退出并销毁IOProc、aggregate与tap。音频pipe非阻塞；慢消费者造成满管道时显式capture_error并停止，不静默丢帧或无限积压。

停止会话时先停止采集，再由PCM EOF让worker排空VAD、最终ASR和翻译，30s内未完成则标failed并清理子进程。加载阶段可停止，失败也清理capture／worker；关闭主窗口或请求退出App先停止会话。隐藏未来字幕窗不应停止会话，关闭控制面板也不隐式停止，此处尚无新的窗口实现。

helper可排除调用者的Core Audio process object；调用者当时没有音频对象时列表为空。**尚未证明Tauri／WebKit音频子进程全部排除**，不能宣称App自身播放输出已完整排除。输出设备／采样格式中途变化及蓝牙设备切换仍需实机验收。

## 设备与运行准备

复用既有StageScheduler：启动在线前尝试独占全部本地ASR／MT槽位；已有离线工作占用时拒绝启动，不终止旧任务，也不边排队边积累录音。在线会话期间新本地离线阶段等待，已有独立云端文本翻译预算保持原样。是否需要更复杂的优先级／抢占策略留待实际体验，不增加尚无需求的调度框架。

桌面工厂定位受管Python于App数据目录`runtimes/online/bin/python3`、worker于bundle资源`online_worker/`、原生helper于App可执行文件目录`atogaki-audio-tap`。模型路径与源语言在启动时冻结；日语选SenseVoice，其余已验证语言选Zipformer，最终ASR为Qwen。当前MT只支持显式提供的本地loopback服务，没有云端回退或新的音频上传。

源码资源映射与helper构建脚本已加入；**私有Python分发／安装、模型包管理、helper完整发行接线、受管MT服务与现有云端provider复用尚未完成**。缺少运行资产时start返回not-ready，不展示为可运行能力。首版不要求普通用户手动装Python或启动llama-server；这些普通用户要求尚待后继打包完成，不能把本机已有研究环境视为产品交付。

## 当前验证范围

- 实际本地模型PCM回放：日语60s跨环形缓冲回绕，20段均有最终ASR／译文依赖；英语6.625s覆盖首帧时钟、非整帧EOF和完整停止，见[链路记录](../two-pass-smoke-validation.md#用户反馈后的上下文调整与pcm会话输入)。
- Rust真实子进程fixture验证模型ready后采集、设备忙拒绝／会话期占用、重复开始拒绝、EOF排空、会话ID一致与设备释放；不把fixture当作真实ASR。已有两个StageScheduler回归通过。fixture曾出现等待running超时，定位为Tokio Child::wait自动关闭自己持有的stdin，误触发helper停止。已把控制管道独立持有，并增加显式stop之前保持running的回归；不据此宣称长期稳定性。
- 原生helper arm64／14.2目标编译、ad-hoc签名、Info.plist语法及本机三秒采集／停止通过。最终tap-only版本正常输出非零PCM并退出0，只保存字节统计／状态，不保存音频。没有用已知播放刺激验证波形内容或音频质量。
- Tauri IPC／资源／退出代码通过cargo check。首次普通检查因本worktree缺少旧发行sidecars而失败；用TAURI_CONFIG临时清空externalBin并指定devUrl完成类型检查。这不等于完整App构建、安装或资源发行验收。

私有证据保留于`local-artifacts/online/session-20261102/`。尚未把原生系统音频→真实模型→实际字幕窗作为一次完整App会话验收；停止／退出的原生UI交互、长期积压与文本状态内存、录音开关及手动离线精修仍待后继。当前main及已安装v0.1.3不变。
