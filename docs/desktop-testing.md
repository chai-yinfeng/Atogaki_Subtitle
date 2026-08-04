# 桌面界面测试

_最后更新：2026-08-04_

## 构建回归

首次拉取或前端依赖变化后：

```bash
npm --prefix ui ci
```

每次桌面改动至少执行：

```bash
cargo test --offline
npm --prefix ui run build
cargo check --manifest-path src-tauri/Cargo.toml --offline
```

`cargo check` 能验证 Rust/Tauri 命令和配置，但不能发现窗口启动时的 runtime、SQLite 路径或系统 WebView 问题，因此还必须执行下面的冒烟测试。

## 启动真实窗口

确保 `ffmpeg` 和 `whisper-cli` 在 `PATH`，或显式设置：

```bash
export ATOGAKI_FFMPEG="/path/to/ffmpeg"
export ATOGAKI_WHISPER_CLI="/path/to/whisper-cli"
npm --prefix ui run build
cargo run --manifest-path src-tauri/Cargo.toml
```

当前直接使用 `cargo run` 时加载的是最近一次 `ui/dist`，所以修改前端后必须先运行前端构建。`tauri.conf.json` 刻意不配置 `devUrl`，避免普通 `cargo run` 在没有同时启动 Vite server 时显示白屏。开发窗口启动后，首页会显示实际的应用数据目录和 SQLite 任务列表。

模型选择器会优先打开 `ATOGAKI_WHISPER_MODEL` 所在目录；未配置时，如果 `~/Models` 存在则从该目录打开。两个路径输入框也允许直接粘贴完整路径，作为原生文件面板不可用时的降级方式。

如果将来安装 Tauri CLI 并恢复热更新模式，应由 `tauri dev` 同时启动 Vite，再重新配置 `beforeDevCommand` 与 `devUrl`；不要把该配置与普通 `cargo run` 的使用说明混在一起。

## 手工冒烟清单

建议使用几十秒的本地日语音频或 MP4，以及一个小型 whisper.cpp ggml 模型：

1. 选择媒体和模型，提交任务；任务应立即显示为 `queued`，随后显示执行阶段。
2. 应用保持响应，任务失败时卡片应显示 `failed` 和错误信息；成功后显示 `done`。
3. 点击任务进入工作区；媒体应能播放，字幕段按时间排序。
4. 播放时当前句应高亮；点击时间码应跳转并继续播放。
5. 修改日文并保存；刷新或重启应用后修改仍应存在。已有中文时，日文修改应标记“待重译”。
6. 修改中文并保存；“待重译”应清除，并显示中文已编辑。
7. 将原媒体临时移走后重新打开任务，应显示可操作错误；若任务目录已有 `audio.wav`，应回退到音频。

## 当前测试边界

- macOS WebView 通常可以直接播放 MP4/MOV 和常见音频；MKV、部分 WebM 或特殊编码可能失败，此时只保证 `audio.wav` 回听。
- 当前桌面入口只创建转写任务，中文可人工填写；DeepL 单段/全部重译尚未接入界面。
- SQLite 编辑已是桌面工作区主数据，但现有 CLI 导出仍读取任务 JSON；桌面导出接通前，不要用 CLI 文件链验证 SQLite 编辑结果。
- 开发态 `cargo run` 二进制不是注册安装的 `.app`，部分 macOS GUI 自动化工具无法枚举它；打包阶段需要补充可重复的窗口自动化测试。
