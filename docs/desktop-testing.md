# 桌面界面测试

_最后更新：2026-08-09_

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

首次打包或 sidecar 版本变化时，先在对应 macOS/CPU 架构运行：

```bash
./scripts/build-sidecars-macos.sh
```

脚本固定并校验 whisper.cpp、FFmpeg、libass 字体栈的源码，生成 Tauri 所需的 target-suffixed `ffmpeg`、`ffprobe`、`whisper-cli`，拒绝 GPL/nonfree/libx264 配置和 Homebrew 动态库依赖。网络异常时可先在交互式 zsh 执行 `proxy_on`，但代理地址和凭据不得写入仓库或日志。

`cargo check` 能验证 Rust/Tauri 命令和配置，但不能发现窗口启动时的 runtime、SQLite 路径或系统 WebView 问题，因此还必须执行下面的冒烟测试。

## 启动真实窗口

首次启动可以在设置引导中选择或下载 Whisper/VAD 模型并配置翻译。打包 App 默认使用 Bundle 内的 sidecar，不需要 Homebrew、shell `PATH` 或 Atogaki 环境变量。以下覆盖只供 `cargo run`、CLI、自动化和故障注入：

```bash
export ATOGAKI_FFMPEG="/path/to/ffmpeg"
export ATOGAKI_WHISPER_CLI="/path/to/whisper-cli"
export ATOGAKI_WHISPER_MODEL="/path/to/ggml-medium.bin"
export ATOGAKI_VAD_MODEL="/path/to/ggml-silero-v6.2.0.bin"
export DEEPL_AUTH_KEY="your-deepl-api-key"
export ATOGAKI_DATA_DIR="/absolute/path/to/isolated-atogaki-data"
npm --prefix ui run build
cargo run --manifest-path src-tauri/Cargo.toml
```

真实窗口回归必须设置绝对路径的 `ATOGAKI_DATA_DIR`。桌面端会在启动时创建并规范化该目录，将测试 SQLite、任务目录、词表和烧录快照全部隔离在其中；不设置时才使用系统正式应用数据目录。媒体、模型和用户选择的最终导出文件仍位于各自原路径，因此回归时也应选择独立导出目录。

当前直接使用 `cargo run` 时加载的是最近一次 `ui/dist`，所以修改前端后必须先运行前端构建。`tauri.conf.json` 刻意不配置 `devUrl`，避免普通 `cargo run` 在没有同时启动 Vite server 时显示白屏。开发窗口启动后，首页会显示实际的应用数据目录和 SQLite 任务列表。

打包后的真实窗口回归使用 Tauri CLI；`beforeBuildCommand` 显式把工作目录设置为 `../ui` 后执行 `npm run build`，避免调用位置改变时重复拼接前端路径。本地 ad-hoc 签名的 App Bundle 可用 `tauri build --bundles app` 生成，再从 `src-tauri/target/release/bundle/macos/Atogaki.app` 启动。配置声明最低 macOS 12.0，与 sidecar 的 deployment target 一致。

本机结构冒烟可以使用 `CI=true tauri build --bundles dmg`，但 CI 模式会跳过 Finder 图标定位与背景美化，不能作为最终发布产物。最终候选必须在非 CI 环境运行 `cargo tauri build --bundles dmg`，并实际确认 App、Applications 链接和窗口布局；2026-08-11 的 macOS 26 构建已能正常完成该流程。若以后 Finder AppleScript 再次挂起，应中止并排查，不能用 CI 简化包替代发布门禁。ad-hoc 签名只保证 Bundle 完整性，不代表 Developer ID 身份，也没有经过 Apple 公证。

从终端直接执行 Bundle 内二进制前，必须先退出同一 bundle identifier 的现有 Atogaki 进程。macOS 26 上同时直接执行第二个 GUI 二进制会在 AppKit `_RegisterApplication` 阶段触发 `SIGABRT`；这是重复实例的启动方式问题，不是数据目录初始化崩溃。Finder 的普通再次打开会交给 LaunchServices 激活现有实例。

桌面端优先使用显式的 `ATOGAKI_FFMPEG`、`ATOGAKI_FFPROBE` 和 `ATOGAKI_WHISPER_CLI` 开发覆盖，否则使用与主程序同目录的 sidecar。进入视频烧录测试前可检查目标 ffmpeg 是否能启动并包含 libass；Whisper 帮助信息的启动日志应列出 Metal 后端，且任务的 `recognition-options.json` 中 `no_gpu` 默认为 `false`：

```bash
"${ATOGAKI_FFMPEG:-ffmpeg}" -version
"${ATOGAKI_FFMPEG:-ffmpeg}" -hide_banner -filters | rg ' ass '
"${ATOGAKI_FFMPEG:-ffmpeg}" -hide_banner -encoders | rg 'h264_videotoolbox|mpeg4|libx264'
"${ATOGAKI_WHISPER_CLI:-whisper-cli}" --help
```

正式 sidecar 必须同时看到 `h264_videotoolbox`、`mpeg4` 和 `ass`，且不能看到 `libx264`。源码态 `cargo run` 找不到同目录 sidecar 时仍会回退 Homebrew/PATH，方便开发，但这不是正式分发路径。

模型选择器会优先打开 `ATOGAKI_WHISPER_MODEL` 或 `ATOGAKI_VAD_MODEL` 所在目录；未配置时，如果 `~/Models` 存在则从该目录打开。启动环境中配置的模型路径会自动填入；若 Whisper 模型所在目录包含文件名带 `silero` 的 `.bin`，也会自动选择首个候选 VAD 模型。所有路径输入框都允许直接粘贴完整路径，作为原生文件面板不可用时的降级方式。设置页下载可使用用户填写的 HTTPS 镜像并回退 Hugging Face 官方源；文件先进入应用数据目录 `models/` 下的 `.part`，完整文件通过固定 SHA-256 后才原子安装。失败或下次启动会清理该目录中的未完成文件。

如果将来安装 Tauri CLI 并恢复热更新模式，应由 `tauri dev` 同时启动 Vite，再重新配置 `beforeDevCommand` 与 `devUrl`；不要把该配置与普通 `cargo run` 的使用说明混在一起。

## 手工冒烟清单

建议分别准备几十秒的本地日语、英语音频或 MP4，以及一个小型 whisper.cpp ggml 模型。日语用于既有质量回归，英语用于验证新增语言链路：

配置与恢复预检：

1. 使用全新隔离数据目录启动；首次配置窗口应自动出现，显示系统凭据后端和应用管理的模型目录。没有 Whisper 模型时应明确阻止完成引导。
2. 网络配置分别测试“跟随启动环境”“直连”和本机 HTTP 代理；填写镜像时测试结果应同时列出镜像与官方源。保存后下载 VAD，期间应显示字节进度与实际来源，完成后路径和就绪状态自动更新，且目录中不留下 `.part`。将镜像指向返回错误内容的测试端点时，应因状态或 SHA-256 失败回退官方源。
3. 将翻译切换为“关闭”并保存，工作区翻译按钮应立即反映未配置状态，无需重启。分别切换 DeepL（传统翻译 API）、DeepSeek（LLM API）和 OpenAI-compatible（高级），确认模型／Base URL／风格字段按 provider 显示；Key 输入框不得回显既有值，SQLite 和任务目录中不得出现 Key。点击“检查所选 Key”时只读取当前 provider 的系统凭据条目，不调用翻译 API；存在、不存在和拒绝访问都要给出明确结果，同一进程后续翻译不得再次读取。真实 Key 写入测试只在明确允许修改本机系统凭据时执行。
4. 运行中关闭 App 后再次启动；旧任务应显示因上次退出而失败。点击重试应创建新 UUID，保留旧目录；若旧模型路径已失效，应使用设置页当前有效模型。

核心工作区回归：

1. 选择节目语言、媒体、Whisper 模型和 Silero VAD 模型；VAD 应默认开启。提交后任务应立即显示为 `queued`，随后显示执行阶段。
   - 连续提交至少两个任务：后一个等待单 worker 时显示“排队”，只有真正进入音频提取／转写后才显示“已运行”。完成后处理用时不包含排队时间。
2. 查看任务目录的 `recognition-options.json`；`vad_model` 应为所选路径，VAD 阈值和分段参数应与实际命令一致。关闭 VAD 后新建的任务应记录 `vad_model: null`。
3. 打开“管理词表”，新建或编辑词表；每行必须选择“核心／内容包／仅修正”，内容包必须填写名称，仅修正必须填写规范写法。
4. 将 `スイ → suis` 设为核心；新建任务的最终 prompt 应包含日语读音和规范写法。添加一个仅修正规则后，它不应出现在 prompt。
5. 新建任务时选择词表；作品内容包默认不选，勾选后其词条才出现在最终 prompt。任务目录应同时包含解析后的 `recognition-glossary.txt` 和 `whisper-prompt.txt`，之后修改原词表不应改变这两个快照。
6. 应用保持响应，任务失败时卡片应显示 `failed` 和错误信息；成功后显示 `done`。
7. 重命名一个任务；刷新和重启后应显示自定义名称，UUID 任务目录与源媒体名称不应改变。名称留空应恢复显示媒体文件名。“处理用时”不应因重命名或重启增长；旧版本任务缺少开始／终止时间时应显示未记录，而不从最后更新时间估算。
8. 执行中任务的删除按钮应禁用；删除一个完成或失败任务后，SQLite 记录和任务目录应消失，原始媒体文件必须保留。
9. 点击任务进入工作区；媒体应能播放，字幕段按时间排序。
   - 顶部应按“翻译与词表／字幕校对／导出成品”显示三个任务模块，首次进入默认打开翻译与词表；返回、任务状态、重新读取和操作反馈始终位于模块外。鼠标点击及左右方向键都应切换模块并正确更新选中状态。
   - 在字幕校对中创建未保存的文字草稿，切到另外两个模块后再返回；草稿和按钮状态必须保留。切换模块不得强制暂停媒体、重新读取任务或丢失当前播放位置。
10. 播放时当前句应高亮；点击时间码应跳转并继续播放。
11. 由“字幕校对”进入“字幕编辑”；顶层导航不应出现字幕编辑或跨任务列表，编辑器必须直接打开当前任务。拖动字幕块主体时应保持时长并只移动当前块；拖动左右边缘时只修剪当前块。字幕之间可以保留空白，任何拖动都应在同轨相邻块处停止，不得新建或扩大重叠。松开后应原子保存，Escape 应放弃当前拖动。
12. 连续调整“时间缩放”和“波形强度”：前者应改变可见时间范围，后者只放大波形显示而不改变播放音量或时间。触控板横向滑动／画布拖动应低延迟平移时间窗口，捏合或 ⌘/Ctrl+滚轮应围绕指针位置缩放；普通纵向滑动不得触发缩放。跟随播放头关闭后不应自行跳回。
13. 在字幕编辑中修改当前段原文和译文，验证保存、放弃与“撤销上次文字保存”；原文变化应按既有规则标记译文待重译。存在文字草稿时，Cut、Join、打轴撤销和“返回字幕校对”必须阻止操作并提示先保存或放弃。保存时间后刷新、重启并重新读取同一任务，人工时间和“时间轴已编辑”标记应保留；当前句、字幕导出和新烧录快照都应使用新时间。
14. 将播放头置于字幕块内部后点击“在播放头切开”或按 ⌘B；段数应加一、右块获得新 ID、后续索引连续。没有文本光标时左右块暂时保留同一原文和译文，连续播放不应闪动；随后可以回到任务详情整理文字。
15. 选中与下一块边界完全相接的字幕后执行 Join；结果应保留左块 ID、按语言顺序拼接文字并使段数减一。两块之间存在任何空白时按钮必须禁用，不得静默填满无字幕区间；任一块没有译文时结果应为未翻译。
16. 连续执行 Cut、Join 和时间拖动后逐步点击“撤销上次打轴”；每次应原子恢复段数、ID、顺序、文字、译文、时间和编辑标记。结构操作后修改或翻译字幕时，陈旧撤销必须禁用。重新读取并重启 App 后结构仍存在，但会话撤销按钮不可用。
17. 在非输入框且没有对话框打开时，分别切换中文和英文键盘布局，验证空格/K 播放暂停、←/J 和 →/L 跳 5 秒、`[`/`]` 切换字幕、`,`/`.` 调速以及 O 开关悬浮字幕；在原文、译文、时间输入框中键入相同按键不得触发播放操作。
18. 点击“打开悬浮字幕”；主窗口、Dock 图标、菜单栏和当前 Space 不应自行隐藏、退后或切换。独立面板保留原生标题栏和边框，但不应出现红黄绿窗口按钮；必须能通过标题栏反复拖动、通过窗口边缘反复缩放且 App 不闪退。点击面板后，其鼠标控制以及与主窗口相同的播放、跳转、调速快捷键都应生效；打开面板本身不应抢走主窗口焦点。等比例放大窗口时，标题、控制条、语言标签和两行字幕应同步放大；缩小时应同步缩小，长字幕不得被裁掉。播放跨段、跳转和当前字幕编辑后应同步原文与译文。主动切换到另一普通桌面或全屏应用 Space 后，面板应仍然显示；回到 Atogaki 主窗口时 Dock 图标和菜单栏应正常可用。
19. 点击面板内“×”或返回任务列表；面板应立即隐藏且不得在切换微信、浏览器或其他 Space 后再次出现，Atogaki 的 Dock 图标和菜单栏应恢复。
20. 悬浮窗存在时点击主窗口左上角关闭按钮；App 进程应退出、Dock 图标应消失，悬浮窗不得遗留。随后从 `/Applications` 重新打开，应得到可用主窗口。
21. 修改原文并保存；刷新或重启应用后修改仍应存在。已有译文时，原文修改应标记“待重译”。
22. 在工作区选择同语言、带修正规则的词表并预览；确认后只修改匹配段，已有译文应标记为待重译。其他语言的词表不应出现在候选中。
23. 修改原文时点击“修正加入词表”，缩短误识别和规范写法后保存；词表管理器中应出现该规则并保留任务的源语言。
24. 修改译文并保存；“待重译”应清除，并显示译文已编辑。
25. 点击“翻译本段”；译文应写入 SQLite，刷新或重启后仍然存在。
26. 点击“全部翻译／重译”，确认覆盖提示；全部译文应一次性更新。当前 provider 没有可用 Key 时应保留完整错误提示且不得局部写入字幕。成功后“翻译与词表”应立即显示最近 provider、返回模型、批次段数、可得 token 与完成时间。
27. 使用 DeepSeek 或自定义 OpenAI-compatible 端点翻译包含任务保护词的片段；响应必须按稳定段 ID 写回，专名占位符完整恢复。模拟空内容、无效 JSON、重复／遗漏 ID 时应在一次自动重试后失败，并且 SQLite 字幕保持原状。
28. 修改原文但保留旧译文并保存；导出应拒绝，并提示先处理待重译字幕。
29. 重译后点击“导出字幕…”，选择一个目录；日语任务应生成以任务显示名称为前缀的 `.ja.srt`、`.zh-Hans.srt`、`.bilingual.srt` 和 `.bilingual.ass`，内容来自 SQLite 人工编辑后的状态。英语任务的原文文件应改为 `.en.srt`。
30. 再次导出到同一目录，应列出冲突文件并要求确认；取消后原文件保持不变，确认后才覆盖。导出成功后点击“在 Finder 中显示”，应选中双语 ASS 文件。
31. 点击“导出带字幕视频…”，确认能力面板显示 App Bundle 内的 `ffmpeg` 绝对路径、libass、VideoToolbox 和 MPEG-4 回退。分别选择双语、仅译文或仅原文以及 MP4 输出位置；对低码率源视频，VideoToolbox H.264 的目标码率应为源视频加约 20% 余量，而不是固定高码率。MPEG-4 回退编码效率较低，应记录实际大小与质量，不能假定同一比例。
32. 提交后进度应增加；关闭弹窗不影响烧录。取消后记录应变为“已取消”，目标目录不应留下 `.partial.mp4`。
33. 完成后记录应显示最终编码器和音频处理方式；VideoToolbox 运行时失败时，应显示原始回退原因与 “MPEG-4 软件编码”。点击“在 Finder 中显示”应选中最终 MP4。若 MPEG-4 也失败，任务才失败，并保留本次 ASS 快照和错误记录。
34. 任务目录 `renders/` 应保存本次不可变 ASS 快照；烧录期间继续编辑 SQLite 字幕只影响下次提交。
35. 将原媒体临时移走后重新打开任务；若任务目录已有 `audio.wav`，应回退到音频，同时明确提示原媒体缺失并显示“重新定位原媒体”。选择移动后的同一文件，确认画面、收听、字幕编辑和视频烧录恢复，已编辑字幕不变。

首页滚动回归：将窗口滚动到任务列表底部并保持至少 6 秒；后台轮询在任务数据未变化时不得显示“正在读取任务”、重建列表或把页面拉回上方。任务状态确实变化时只更新列表内容，并保持当前可用滚动位置。

多语言专项回归：

1. 先建一个日语任务，再建一个英语任务；二者重启后应保持各自的源语言，目标语言都为简体中文。
2. 英语任务的 `recognition-options.json` 应记录 `source_language: "en"`，`status.json` 也应记录 `source_language: "en"` 与 `target_language: "zh-Hans"`。
3. 英语原文的单词间空格在分段、编辑、重译和导出后必须保留；DeepL 返回数量应与字幕段稳定对应。
4. 新建英语词表后只应出现在英语任务中；日语内置词表不应被英语任务选中或应用。
5. 英语任务导出应得到 `.en.srt` 与 `.zh-Hans.srt`；原文、译文和双语三种烧录轨道都应使用相同的任务语言语义。
6. 使用包含旧任务的隔离数据库升级一次；旧日语任务、人工编辑、词表和烧录记录应保留，并继续显示为日语到简体中文。

每轮真实窗口回归还应确认首页显示的数据目录等于本轮 `ATOGAKI_DATA_DIR`，并在结束后检查系统正式应用数据目录未产生本轮测试任务。

## 最近一次打包窗口回归

2026-08-09 使用语言抽象分支的 ad-hoc 签名打包 App、独立 `/private/tmp/atogaki-english-regression-20260809.Qctexv` 数据目录和 `Daily English Podcast.mp4` 的 90 秒只读片段完成真实英语闭环：

- 任务明确记录英语原文到简体中文，使用 large-v3 q5_0、Silero VAD 和 Bundle 内 Metal `whisper-cli`，23 秒完成 27 段初始转写；英文单词边界、`.en.srt` 命名和任务语言在 SQLite、状态文件及重启后均保持正确。
- DeepL 首次实际翻译只在需要读取系统凭据时触发 Keychain 授权；授权后 27 段分 3 批完成并原子写入 SQLite。人工修改第一段原文和译文后，编辑标记、译文和非过期状态均在重启后恢复。
- 界面导出 `.en.srt`、`.zh-Hans.srt`、双语 SRT 与 ASS，内容来自 SQLite 当前编辑状态。旧版固定质量参数的双语 MP4 为 90.4 秒、约 25 MiB；抽帧确认英文与简体中文字幕均已烙入画面。

2026-08-09 在同一 90 秒、6.0 MiB、视频流 467 kb/s 的隔离样本上补充烧录回归：

- 新版目标码率为 560,698 b/s（源视频码率加 20%）。当前机器的 VideoToolbox 创建会话返回 `-12908`，因此直接覆盖 MPEG-4 回退；回退成品为 11.8 MiB，而不是旧版固定质量下的约 25 MiB。MPEG-4 在该 720p 样本上达到量化器上限，不能精确遵守低目标码率；这说明无 H.264 硬件时不能承诺“仅增加 20–30%”，但新策略仍避免了旧固定质量导致的四至五倍放大。
- 打包 App 能打开、关闭独立悬浮字幕窗口，且窗口能取得当前英语段的英文与简体中文文本；但首轮仅验证了创建和内容同步，未验证拖动、可见缩放控制和跨桌面显示。该缺口导致 `v0.1.0-alpha.3` 被撤回，修复后必须按本节第 11 步完成完整打包窗口回归，才能再次发布。
- 原始 Whisper JSON 在 53.28 秒处包含三条共享 100 ms 时间范围的长文本，同时被后一条 2 秒范围覆盖。分段整理现会先合并相同范围，再丢弃这种不可能阅读且被后一段完整覆盖的时间戳伪影，并以该真实形态加入单元回归。
- 源媒体未被修改；隔离目录不会由 App 自动清理，本轮先保留用于问题修复和证据核对。

2026-08-06 使用 Tauri CLI 2.11.4 生成新的 `0.1.0` Apple Silicon 候选 App/DMG，并完成发布结构审计：

- App 采用 ad-hoc identity `-`，主程序及三个 sidecar 均通过 `codesign --verify --deep --strict`；Bundle identifier 为 `com.chai-yinfeng.atogaki`，最低系统已与 sidecar 统一为 macOS 12.0。该签名不代表 Developer ID，也未公证，`spctl` 不会把它评估为已认证开发者版本。
- 四个可执行文件均为单架构 arm64，只链接 macOS 系统框架；FFmpeg 8.1.2 含 `ass`、`h264_videotoolbox` 和原生 `mpeg4`，不含 `libx264`。一秒 SQLite/ASS 真实烧录再次验证 VideoToolbox `-12908` 后自动回退 MPEG-4 并成功完成。
- App Resources 与仓库 `LICENSE`、完整 `third-party/` 一致；没有打包 Whisper/VAD 模型、`.part` 或非系统 dylib。Tauri 签名会改变 Bundle 内 Mach-O 哈希，构建清单因此明确记录打包前 sidecar 哈希，签名后文件由 `codesign` 与 DMG SHA-256 保护。
- `Atogaki_0.1.0_aarch64.dmg` 通过 `hdiutil verify`，只读挂载后包含 `Atogaki.app` 和指向 `/Applications` 的链接；挂载内容与构建 App 逐文件一致。候选 DMG SHA-256 为 `e207f0fc8326a5e1f39c3396f2273a292ebd00d2c4e0907775b97629c035af52`。
- 对应源码包及内部七份上游源码全部通过 SHA-256；源码包 SHA-256 为 `d2d8335747198847da16976ce63ec37a879c8ba77d311d97d9890bd9a887c5cc`。
- 候选 App 已在清除代理、Atogaki 和 DeepL 环境变量后，以 `/private/tmp/atogaki-rc-20260806.KDUTfK` 启动；隔离 SQLite migration/integrity 检查通过，初始化 1 个内建词表、0 个任务。窗口内的完整用户操作回归等待手工确认。

2026-08-06 使用未签名打包 App 和 `/tmp/atogaki-network-ui-20260806` 完成网络与模型下载回归：

- 清除进程中的 `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY`、`ATOGAKI_*` 与 `DEEPL_AUTH_KEY` 后，设置页正确显示隔离模型目录。
- 自定义代理 `http://127.0.0.1:7897` 下，自定义镜像与 Hugging Face 官方源连通性测试均返回 HTTP 206，并显示实际重定向主机。
- 从 `https://hf-mirror.com` 路径下载 Silero VAD 成功，完整文件为 885,098 字节，SHA-256 为 `2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987`，没有残留 `.part`。
- 把镜像改为返回 404 的 `https://example.com` 后重新下载，应用自动回退 Hugging Face 官方源并再次通过相同 SHA-256；界面最终来源显示为官方源。
- 隔离 SQLite 只保存代理模式、无凭据代理 URL、镜像 URL、provider ID 和模型路径，没有 API key。本轮读取但没有写入、回显或删除现有 Keychain 凭据。
- 同日三份启动崩溃报告均为已有 Atogaki 运行时再次直接执行 Bundle 内二进制，在 AppKit `_RegisterApplication` 触发 `SIGABRT`；关闭旧进程后相同构建可稳定启动并完成上述测试。

2026-08-05 使用固定版本 Apple Silicon sidecar 与全新 `/tmp` 数据目录，在清除 `ATOGAKI_*` 和 `DEEPL_AUTH_KEY` 的进程环境后启动未签名 `Atogaki.app`：

- 首次引导显示空配置和隔离模型目录；通过界面选择现有 medium/VAD 模型后，App 使用 Bundle 内 `whisper-cli` 对 `湖吉の庭 Vol1.mp4` 的 12 秒只读片段完成 Metal/VAD 转写，得到 1 段 `こんばんは`，任务状态为 `done`。
- 视频能力面板显示的二进制是 `Atogaki.app/Contents/MacOS/ffmpeg`，而非 Homebrew；随包的许可证与构建清单位于 `Contents/Resources/third-party/`。
- 当前机器能枚举 VideoToolbox，但实际创建硬件会话返回 `-12908`。持久化烧录服务正确记录失败并自动回退内置 MPEG-4；真实 1280×720 视频使用正式 Hiragino 日中 ASS 样式烧录成功，ffprobe 确认为 MPEG-4 视频、AAC 音频。该回归明确覆盖“编码器存在但运行时不可用”的路径。

2026-08-05 针对启动配置又使用新的隔离数据目录完成一轮打包窗口回归：首次引导正确显示已有 Whisper/VAD 路径、应用管理模型目录和 macOS Keychain 后端；将 provider 切换为关闭后立即生效并在重启后保持。通过交互式 zsh 的 `proxy_on` 环境，从 whisper.cpp 官方来源下载 Silero VAD 成功，885,098 字节文件自动设为默认且没有残留 `.part`。隔离 SQLite 只包含模型路径、provider ID 和引导状态，没有 API key；本轮没有写入、覆盖或删除真实 Keychain 凭据。中断识别与派生重试由 Rust 集成测试覆盖，本轮没有重复提交完整视频识别。

2026-08-05 使用未签名的 `Atogaki.app`、独立临时数据/导出目录和 `湖吉の庭 Vol1.mp4` 完成了一轮真实窗口回归：

- 45 秒片段生成 6 段字幕，完整 268.5 秒视频生成 63 段字幕；两者均启用 Silero VAD、Metal Whisper、Yorushika 任务词表与不可变 prompt/修正规则快照。
- 已验证媒体播放、当前句同步、时间码跳转、SQLite 日中字幕编辑、待重译阻断、四种字幕文件导出、VideoToolbox 双语视频烧录、任务重命名和重启持久化。
- 打包 App 中浏览器原生 `window.prompt` 不会可靠显示；“修正加入词表”已改为应用内对话框，并在隔离词表中验证保存成功。旧任务词表快照保持不变，新任务只读取提交时的最新规则。
- DeepL 单段翻译与 6 段批量重译均已通过真实 API 验证，批量结果原子写入 SQLite，并在 App 重启后完整恢复。批量重译、文件覆盖、删除和烧录取消已统一改用应用内确认窗口；真实窗口已验证取消与确认两条路径。
- 完整视频从 Desktop 原路径只读处理成功，源文件大小和修改时间未改变。执行期间窗口可继续刷新和操作；首页任务、活动详情和烧录进度现在会自动轮询，音频提取与 Whisper 阶段仍没有百分比进度。

真实窗口回归使用的 `ATOGAKI_DATA_DIR` 不会由 App 自动删除。本轮目录应保留到证据核对和问题修复结束；确认不再需要后，先关闭 App，再只清理本轮明确创建的隔离根目录。不要把清理目标指向正式应用数据目录。单个 Rust 测试使用自己创建的临时目录，并在成功结束时自行清理。

真实一秒视频烧录回归可单独运行：

```bash
ATOGAKI_FFMPEG="$PWD/src-tauri/binaries/ffmpeg-$(rustc --print host-tuple)" \
  cargo test lgpl_sidecar_renders_a_persisted_sqlite_workspace -- --ignored --nocapture
```

该测试会经过 SQLite 字幕快照、持久化烧录队列、libass 和最终 MP4 安装，并在结束后清理临时目录。

## 当前测试边界

- macOS WebView 通常可以直接播放 MP4/MOV 和常见音频；MKV、部分 WebM 或特殊编码可能失败，此时只保证 `audio.wav` 回听。
- 桌面翻译使用 DeepL 云端 API，会发送当前任务的原文字幕；单段和全部重译还会发送 SQLite 中前后约 30 秒的原文作为局部上下文。API key 可由设置界面写入系统凭据库，环境变量仅作兼容回退；设置加载只返回是否已配置和来源，不回显密钥。
- 桌面 SRT/ASS 与 MP4 烧录均使用 SQLite，并支持选择目标位置、覆盖确认与 Finder 定位；现有 CLI `translate`/`export` 仍读取任务 JSON，两条入口的数据源不同，不要用 CLI 命令验证桌面人工编辑。
- 当前候选包以 macOS ad-hoc 签名、未公证的 `.app` 为主；Windows Credential Manager、Linux Secret Service、安装包权限与可重复窗口自动化需要在对应平台补测。
