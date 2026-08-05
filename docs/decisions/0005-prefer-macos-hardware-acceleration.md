# 0005：macOS 本地处理优先使用硬件加速

日期：2026-08-05
状态：已采用

补充：本记录中的 libx264 分发回退已由决策 0009 取代；Metal 与 VideoToolbox 优先原则继续有效。

## 背景

Atogaki 的离线识别和带字幕视频输出都是长任务。Apple Silicon 提供 Metal 推理和 VideoToolbox 编码，但外部二进制可能未编译对应后端，硬件设备也可能在运行时不可用。完全禁止 CPU 回退会降低任务可靠性；静默使用 CPU 又会造成明显的性能误判。

## 决策

- Whisper 默认显式请求 GPU device 0；在当前 macOS whisper.cpp 构建中对应 Metal。只有错误包含 Metal/GPU 特征时，才自动重试一次 `--no-gpu`。
- 硬字幕烧录必须使用带 libass 的 ffmpeg，不再静默改成软字幕封装。
- H.264 烧录优先选择 `h264_videotoolbox`，并设置 `allow_sw=0`，确保这次尝试确实使用硬件编码。
- VideoToolbox 不可用或编码失败时，明确记录原因并回退到 libx264。现有 CRF 和 preset 只控制这一软件回退路径。
- ASS/libass 滤镜仍在 CPU 上执行；当前只把 H.264 编码交给 VideoToolbox，避免硬件解码帧与 CPU 字幕滤镜之间产生不稳定的格式传输。

## 影响

- 正常 Apple Silicon 环境会优先获得 Metal 识别和 VideoToolbox 视频编码性能。
- CPU 回退仍保证可用性，同时终端日志会明确显示实际编码路径。
- 桌面烧录界面接入时应展示检测到的二进制、libass 能力以及最终使用的编码器；是否允许用户强制 CPU 作为高级选项，留待真实质量对比后决定。
