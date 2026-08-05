# 桌面界面测试

_最后更新：2026-08-05_

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
export ATOGAKI_WHISPER_MODEL="/path/to/ggml-medium.bin"
export ATOGAKI_VAD_MODEL="/path/to/ggml-silero-v6.2.0.bin"
export DEEPL_AUTH_KEY="your-deepl-api-key"
npm --prefix ui run build
cargo run --manifest-path src-tauri/Cargo.toml
```

当前直接使用 `cargo run` 时加载的是最近一次 `ui/dist`，所以修改前端后必须先运行前端构建。`tauri.conf.json` 刻意不配置 `devUrl`，避免普通 `cargo run` 在没有同时启动 Vite server 时显示白屏。开发窗口启动后，首页会显示实际的应用数据目录和 SQLite 任务列表。

桌面端优先使用 `ATOGAKI_FFMPEG` 和 `ATOGAKI_WHISPER_CLI`，不一定等同于终端中 `PATH` 找到的程序。进入视频烧录测试前可检查实际配置的 ffmpeg 是否能启动并包含 libass；Whisper 帮助信息的启动日志应列出 Metal 后端，且任务的 `recognition-options.json` 中 `no_gpu` 默认为 `false`：

```bash
"${ATOGAKI_FFMPEG:-ffmpeg}" -version
"${ATOGAKI_FFMPEG:-ffmpeg}" -hide_banner -filters | rg ' ass '
"${ATOGAKI_FFMPEG:-ffmpeg}" -hide_banner -encoders | rg 'h264_(videotoolbox)|libx264'
"${ATOGAKI_WHISPER_CLI:-whisper-cli}" --help
```

未设置 `ATOGAKI_FFMPEG` 时，macOS 桌面端会优先查找 `/opt/homebrew/opt/ffmpeg-full/bin/ffmpeg`，因此从 Finder 启动也不依赖 zsh 的 `PATH`。

模型选择器会优先打开 `ATOGAKI_WHISPER_MODEL` 或 `ATOGAKI_VAD_MODEL` 所在目录；未配置时，如果 `~/Models` 存在则从该目录打开。启动环境中配置的模型路径会自动填入；若 Whisper 模型所在目录包含文件名带 `silero` 的 `.bin`，也会自动选择首个候选 VAD 模型。所有路径输入框都允许直接粘贴完整路径，作为原生文件面板不可用时的降级方式。

如果将来安装 Tauri CLI 并恢复热更新模式，应由 `tauri dev` 同时启动 Vite，再重新配置 `beforeDevCommand` 与 `devUrl`；不要把该配置与普通 `cargo run` 的使用说明混在一起。

## 手工冒烟清单

建议使用几十秒的本地日语音频或 MP4，以及一个小型 whisper.cpp ggml 模型：

1. 选择媒体、Whisper 模型和 Silero VAD 模型；VAD 应默认开启。提交后任务应立即显示为 `queued`，随后显示执行阶段。
2. 查看任务目录的 `recognition-options.json`；`vad_model` 应为所选路径，VAD 阈值和分段参数应与实际命令一致。关闭 VAD 后新建的任务应记录 `vad_model: null`。
3. 打开“管理词表”，新建或编辑词表；每行必须选择“核心／内容包／仅修正”，内容包必须填写名称，仅修正必须填写规范写法。
4. 将 `スイ → suis` 设为核心；新建任务的最终 prompt 应包含日语读音和规范写法。添加一个仅修正规则后，它不应出现在 prompt。
5. 新建任务时选择词表；作品内容包默认不选，勾选后其词条才出现在最终 prompt。任务目录应同时包含解析后的 `recognition-glossary.txt` 和 `whisper-prompt.txt`，之后修改原词表不应改变这两个快照。
6. 应用保持响应，任务失败时卡片应显示 `failed` 和错误信息；成功后显示 `done`。
7. 重命名一个任务；刷新和重启后应显示自定义名称，UUID 任务目录与源媒体名称不应改变。名称留空应恢复显示媒体文件名。
8. 执行中任务的删除按钮应禁用；删除一个完成或失败任务后，SQLite 记录和任务目录应消失，原始媒体文件必须保留。
9. 点击任务进入工作区；媒体应能播放，字幕段按时间排序。
10. 播放时当前句应高亮；点击时间码应跳转并继续播放。
11. 修改日文并保存；刷新或重启应用后修改仍应存在。已有中文时，日文修改应标记“待重译”。
12. 在工作区选择带修正规则的词表并预览；确认后只修改匹配段，已有中文应标记为待重译。
13. 修改日文时点击“修正加入词表”，缩短误识别和规范写法后保存；词表管理器中应出现该规则。
14. 修改中文并保存；“待重译”应清除，并显示中文已编辑。
15. 点击“翻译本段”；译文应写入 SQLite，刷新或重启后仍然存在。
16. 点击“全部翻译／重译”，确认覆盖提示；全部译文应一次性更新。没有 `DEEPL_AUTH_KEY` 时按钮应禁用并显示配置提示。
17. 修改日文但保留旧中文并保存；导出应拒绝，并提示先处理待重译字幕。
18. 重译后点击“导出字幕…”，选择一个目录；应生成以任务显示名称为前缀的 `.ja.srt`、`.zh.srt`、`.bilingual.srt` 和 `.bilingual.ass`，内容来自 SQLite 人工编辑后的状态。
19. 再次导出到同一目录，应列出冲突文件并要求确认；取消后原文件保持不变，确认后才覆盖。导出成功后点击“在 Finder 中显示”，应选中双语 ASS 文件。
20. 点击“导出带字幕视频…”，确认能力面板显示实际 `ffmpeg-full`、libass 和可用编码器。分别选择日中双语、仅中文或仅日文以及 MP4 输出位置。
21. 提交后进度应增加；关闭弹窗不影响烧录。取消后记录应变为“已取消”，目标目录不应留下 `.partial.mp4`。
22. 完成后记录应显示最终编码器和音频处理方式；VideoToolbox 运行时失败时，应显示回退原因与 `libx264`。点击“在 Finder 中显示”应选中最终 MP4。
23. 任务目录 `renders/` 应保存本次不可变 ASS 快照；烧录期间继续编辑 SQLite 字幕只影响下次提交。
24. 将原媒体临时移走后重新打开任务，应显示可操作错误；若任务目录已有 `audio.wav`，应回退到音频。

真实一秒视频烧录回归可单独运行：

```bash
cargo test ffmpeg_full_renders_a_persisted_sqlite_workspace -- --ignored --nocapture
```

该测试会经过 SQLite 字幕快照、持久化烧录队列、libass 和最终 MP4 安装，并在结束后清理临时目录。

## 当前测试边界

- macOS WebView 通常可以直接播放 MP4/MOV 和常见音频；MKV、部分 WebM 或特殊编码可能失败，此时只保证 `audio.wav` 回听。
- 桌面翻译使用 DeepL 云端 API，会发送当前日文字幕；单段和全部重译还会发送 SQLite 中前后约 30 秒的日文作为局部上下文。API key 只从启动环境读取，设置界面和 Keychain 存储尚未实现。
- 桌面 SRT/ASS 与 MP4 烧录均使用 SQLite，并支持选择目标位置、覆盖确认与 Finder 定位；现有 CLI `translate`/`export` 仍读取任务 JSON，两条入口的数据源不同，不要用 CLI 命令验证桌面人工编辑。
- 开发态 `cargo run` 二进制不是注册安装的 `.app`，部分 macOS GUI 自动化工具无法枚举它；打包阶段需要补充可重复的窗口自动化测试。
