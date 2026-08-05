# 0009：固定 LGPL sidecar 与三级视频烧录回退

日期：2026-08-05
状态：已采用

## 背景

桌面 App 目前从环境变量、Homebrew 路径或 `PATH` 查找 `whisper-cli` 和 FFmpeg。这适合开发，但无法保证未安装 Homebrew 的设备可运行。当前 `ffmpeg-full` 还启用了 GPL 并链接 libx264；直接复制该二进制既不独立，也会把 GPL 分发义务带入安装包。

模型继续按设备下载，不进入 App Bundle；执行模型的 `whisper-cli`、媒体处理的 `ffmpeg` 及探测媒体的 `ffprobe` 体积较小，应成为按平台与 CPU 架构构建的 Tauri sidecar。

## 决定

- 首个 macOS sidecar 基线固定为 whisper.cpp `v1.8.6` 与 FFmpeg `8.1.2`，不跟随 Homebrew 或 `latest` 自动升级。升级必须重新执行真实媒体、Metal、VAD、libass 与 VideoToolbox 回归。
- whisper.cpp 使用静态运行时构建，启用 Metal/Accelerate，关闭 `GGML_NATIVE`，避免产物只适用于构建机的具体 Apple CPU。
- FFmpeg 使用 LGPL 配置构建：禁止 `--enable-gpl`、`--enable-nonfree` 和 libx264，保留 libass、VideoToolbox、AudioToolbox、原生 MPEG-4 Part 2 与当前输入输出格式。`ffmpeg` 和 `ffprobe` 一起分发。
- sidecar 文件按 Tauri target triple 命名并纳入 App Bundle。桌面运行时优先使用与主程序同目录的 sidecar；`ATOGAKI_FFMPEG` 与 `ATOGAKI_WHISPER_CLI` 只保留为显式开发覆盖。CLI 继续使用自己的参数和环境变量。
- 硬字幕视频采用三级结果：先尝试真实 `h264_videotoolbox`；失败或不可用时记录原因并使用 FFmpeg 原生 `mpeg4` 软件编码；两者均失败时任务明确失败，但不可变 ASS 快照和错误记录必须保留。每个视频编码层内部仍可把不兼容的音频 stream copy 回退为 AAC。
- Atogaki 不分发 libx264。即使开发者通过环境变量选择了带 libx264 的外部 FFmpeg，桌面默认烧录策略也不调用它，避免开发与正式分发产生不一致结果。
- 仓库保存版本、来源、构建脚本、构建参数、许可证和校验逻辑，不提交生成的多架构二进制。正式发布流水线负责为每个 target 生成 sidecar，并发布对应的完整源代码与许可证材料。

## 后果

- Finder 启动的正式 App 不再依赖 shell、Homebrew 或用户安装的媒体工具。
- MPEG-4 软件回退的编码效率低于 libx264；相同观感通常产生更大的 MP4，但避免 VideoToolbox 故障直接丢失视频导出能力。
- `RenderOptions` 中原有的 x264 CRF/preset 暂时保留给 CLI/API 兼容，内置 MPEG-4 回退使用独立质量参数；后续可在破坏性配置迁移中删除旧字段。
- LGPL 不代表没有分发义务。发布物仍需附带 FFmpeg 及静态依赖的许可证、精确源码、构建说明，并允许用户取得和替换相应组件。
- Windows sidecar 必须在 Windows 构建机上生成并完成 Credential Manager、CPU/GPU 与编码回归，不能把 macOS 二进制交叉复制过去。
