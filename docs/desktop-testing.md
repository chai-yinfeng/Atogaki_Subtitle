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
export DEEPL_AUTH_KEY="your-deepl-api-key"
npm --prefix ui run build
cargo run --manifest-path src-tauri/Cargo.toml
```

当前直接使用 `cargo run` 时加载的是最近一次 `ui/dist`，所以修改前端后必须先运行前端构建。`tauri.conf.json` 刻意不配置 `devUrl`，避免普通 `cargo run` 在没有同时启动 Vite server 时显示白屏。开发窗口启动后，首页会显示实际的应用数据目录和 SQLite 任务列表。

模型选择器会优先打开 `ATOGAKI_WHISPER_MODEL` 所在目录；未配置时，如果 `~/Models` 存在则从该目录打开。两个路径输入框也允许直接粘贴完整路径，作为原生文件面板不可用时的降级方式。

如果将来安装 Tauri CLI 并恢复热更新模式，应由 `tauri dev` 同时启动 Vite，再重新配置 `beforeDevCommand` 与 `devUrl`；不要把该配置与普通 `cargo run` 的使用说明混在一起。

## 手工冒烟清单

建议使用几十秒的本地日语音频或 MP4，以及一个小型 whisper.cpp ggml 模型：

1. 选择媒体和模型，提交任务；任务应立即显示为 `queued`，随后显示执行阶段。
2. 打开“管理词表”，新建或编辑词表；每行必须明确选择“提示词”或“修正规则”，列表和下拉框应分别显示两类数量。
3. 新增 `スイ → suis` 修正规则；保存后快照应保留该映射，Whisper prompt 应同时包含日语读音和规范写法。
4. 新建任务时选择词表；任务详情应显示所用词表，任务目录应包含 `recognition-glossary.txt` 快照。之后修改原词表不应改变该快照。
5. 应用保持响应，任务失败时卡片应显示 `failed` 和错误信息；成功后显示 `done`。
6. 重命名一个任务；刷新和重启后应显示自定义名称，UUID 任务目录与源媒体名称不应改变。名称留空应恢复显示媒体文件名。
7. 执行中任务的删除按钮应禁用；删除一个完成或失败任务后，SQLite 记录和任务目录应消失，原始媒体文件必须保留。
8. 点击任务进入工作区；媒体应能播放，字幕段按时间排序。
9. 播放时当前句应高亮；点击时间码应跳转并继续播放。
10. 修改日文并保存；刷新或重启应用后修改仍应存在。已有中文时，日文修改应标记“待重译”。
11. 在工作区选择带修正规则的词表并预览；确认后只修改匹配段，已有中文应标记为待重译。
12. 修改日文时点击“修正加入词表”，缩短误识别和规范写法后保存；词表管理器中应出现该规则。
13. 修改中文并保存；“待重译”应清除，并显示中文已编辑。
14. 点击“翻译本段”；译文应写入 SQLite，刷新或重启后仍然存在。
15. 点击“全部翻译／重译”，确认覆盖提示；全部译文应一次性更新。没有 `DEEPL_AUTH_KEY` 时按钮应禁用并显示配置提示。
16. 修改日文但保留旧中文并保存；导出应拒绝，并提示先处理待重译字幕。
17. 重译后点击“从 SQLite 导出 SRT／ASS”；任务目录中的 `ja.srt`、`zh.srt`、`bilingual.srt` 和 `bilingual.ass` 应包含 SQLite 人工编辑后的内容。
18. 将原媒体临时移走后重新打开任务，应显示可操作错误；若任务目录已有 `audio.wav`，应回退到音频。

## 当前测试边界

- macOS WebView 通常可以直接播放 MP4/MOV 和常见音频；MKV、部分 WebM 或特殊编码可能失败，此时只保证 `audio.wav` 回听。
- 桌面翻译使用 DeepL 云端 API，会发送当前日文字幕；API key 只从启动环境读取，设置界面和 Keychain 存储尚未实现。
- 桌面 SRT/ASS 导出已使用 SQLite；现有 CLI `translate`/`export` 仍读取任务 JSON，两条入口的数据源不同，不要用 CLI 命令验证桌面人工编辑。
- 开发态 `cargo run` 二进制不是注册安装的 `.app`，部分 macOS GUI 自动化工具无法枚举它；打包阶段需要补充可重复的窗口自动化测试。
