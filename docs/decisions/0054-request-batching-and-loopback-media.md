# 0054：短语义组请求打包与本机流式媒体

日期：2026-10-20

状态：accepted。用户明确同意两条实现方向；不改变云端上传边界或本地模型路线。

## 翻译

保留 planner 的句末／停顿语义组、前后文、原文与人工译文 revision、逐组事务和缓存身份。在桌面 DeepSeek 的冻结 provider 与阶段调度器之间增加请求打包层；其他 provider 保持原路径，不扩大本地模型输入。

每个执行快照拥有独立队列，收集窗口为 8 ms。一个请求最多打包 12 个目标、1,800 个目标字符、32,000 字节的原始组输入估算；本来就超出打包预算的单个语义组独立执行，不截断内容。不同 options、style、重复目标 ID 或同 ID 不同原文不混入同一请求。上下文文本去重，单独携带各组的 before／target／after ID 映射；adapter 保留冻结模型、风格、术语保护及逐目标 JSON 输出合同。

最多允许 48 个语义组等待，但实际 HTTP 仍由共享调度器限制为 4 路。结果可乱序，必须完整且没有重复／额外 ID，之后分发回原语义组分别落盘；整个响应结构无效则该请求中的组全部失败，其他请求成功结果保留。人工修改导致的 revision 冲突只影响对应原组，不回滚其他成功组；继续只补缺失／过期项。所有调用者取消后丢弃在途 HTTP future，释放请求名额；部分调用者取消不阻止其余组接收结果。

提前翻译缓存位于打包层之外，键仍来自原始语义请求；新增 wire 字段为空时不参与序列化，因此旧缓存身份兼容。组缓存与 journal 保存各自输入和结果，不依赖本次与哪些组一起发送。新 journal 的 `attempt_scope=semantic_group` 明确表示组尝试，数量不能再解释为 HTTP 次数，耗时包含排队／打包／调用。整次响应 token usage 仅分配给一个接收组，避免同一次 HTTP 被累计多次；它不是逐句账单。

不采用增大语义组跨度来减少请求，因为会改变原组的上下文与失败边界。模拟吞吐结果不等于真实 DeepSeek 延迟改善，更不证明译文质量提高；真实云端与人工审查仍后补。

## 媒体

移除返回 `Vec<u8>` 的自定义媒体协议，改为应用生命周期内的只读 HTTP 服务。只绑定 `127.0.0.1:0`，系统分配端口；只访问后端登记并 canonicalize 的文件，URL 使用会话随机 UUID token，不接受路径参数。没有目录列表、上传或 LAN 监听。检查 Host 与提供的 Origin；原生 AVFoundation 请求可以没有 Origin，CORS 只回显允许的 Tauri origin，不使用 `*`。URL 不写入任务持久配置，重启后重新生成。

完整 GET 和 Range 都按需求流式读取，每次最多 64 KiB；不再整片读取，也不将合法大 Range 静默截为 2 MiB。支持 HEAD、单段／开放／尾部／最多 16 个多段 Range、416 与真实长度。文件被删除返回 404，在传输中截短返回流错误；客户端停止消费后释放流与文件。最后一个 registry 释放时关闭监听，既有连接优雅结束。

macOS 增加 `NSAllowsLocalNetworking`，使系统媒体栈可连接本机 HTTP；保留其他 ATS 默认保护，不启用全局 arbitrary loads。依据 [Apple NSAllowsLocalNetworking 文档](https://developer.apple.com/documentation/bundleresources/information-property-list/nsapptransportsecurity/nsallowslocalnetworking)。播放器保留任务 WAV 回退与播放状态；显式启用 `ATOGAKI_MEDIA_DIAGNOSTICS` 时只记录方法、Range、Origin 和大小，不输出 token、路径、媒体内容。

继续使用同步整片响应会随电影大小占用内存；另行转码完整视频会增加磁盘与等待，且不能解决通用服务的内存风险，因此本轮不采用。流式传输不扩展系统解码能力：不支持的编码仍需任务音频回退。Windows 共享代码经编译／自动测试后仍需原生实机验收，不能用 macOS 结果替代。
