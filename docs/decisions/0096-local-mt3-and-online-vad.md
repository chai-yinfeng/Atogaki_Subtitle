# 0096 混元本地翻译和实时 VAD 升级

2026-11-26。混元本地翻译采用 Hy-MT3 1.8B Q4_K_M 作为推荐下载。文件与实时字幕复用现有内置 llama-server，下载校验成功后沿用「设为当前」逻辑。已选 MT2 在新版模型准备好之前可继续使用。既有 provider ID、设置键、在线偏好和任务快照继续有效，模型以实际模型文件名标识；已保存的任务仍按快照指定的文件处理。MT2 退出常用下载目录。

Silero 的两份模型来自同一语音检测项目。Whisper CLI 在离线文件转录中读取 GGML，用于分割语音并映射回原文件时间；实时字幕 worker 通过 sherpa-onnx 用 ONNX 持续判定开口与停顿，调度识别和翻译。统一成一份模型文件需要改用同一运行时并重做时间轴验证。当前保留两条消费链路，实时增加可下载的 6.3 ONNX，校验后下次会话优先使用；旧运行环境的 ONNX 仍可回退。Whisper 保持现有 GGML 6.2.0，等有上游 6.3 转换产物并完成文件时间轴回归后再升级。[Silero 6.3](https://github.com/snakers4/silero-vad/releases/tag/v6.3) · [Whisper 上游跟踪](https://github.com/ggml-org/whisper.cpp/issues/4202)

模型下载使用固定 revision 和 SHA-256。ONNX 走 Silero 官方 GitHub 源，沿用 App 网络设定；混元仍支持 Hugging Face 镜像。此轮验收限于模型准备、迁移和本地接口兼容，云端实时服务测试暂缓。模型背景及上一轮兼容检查见[调研](../research/2026-11-18-subtitle-and-learning-options.md#本地模型)。

使用 v0.1.6 内置 llama-server 和模型目录固定的 MT3 权重，文件结构化翻译与实时 worker 翻译适配器均通过合成日语冒烟。桌面端回归覆盖旧快照继续使用原模型、新任务读取切换后的模型、ONNX 不完整时沿用旧路径和安装完整时优先使用 6.3。ONNX 下载的固定链接返回文件与已通过本机 sherpa-onnx 静音检查的文件一致。
