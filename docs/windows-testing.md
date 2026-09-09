# Windows 11 实机测试清单

_当前公开基线：`v0.1.0-alpha.6` Windows x86_64 预发布；提交 `3af0728` 的预览修复候选已生成，正等待 alpha.6 后增量矩阵实测；未签名 alpha，仅供知情测试_

## 测试边界

第一轮先验证安装包与 Windows 系统集成，不要求立即跑完整长节目。第二轮再使用一段英语或韩语真实媒体完成识别、翻译、编辑、导出和烧录。发现问题时记录测试步骤、实际结果、截图或完整错误文本；API Key 不进入截图或日志。

测试前记录 Windows 版本与 OS build、CPU、内存、显示器数量／缩放比例，以及设备是否安装过 Rust、Node、FFmpeg、Whisper 或 Atogaki。优先使用没有这些开发工具的 Windows 11 x86_64 设备、虚拟机、Windows Sandbox 或独立测试账户。

## 0. 获取并校验候选

1. 本轮测试从 [Windows 预览修复候选流水线 34308936563](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/34308936563) 下载 `Atogaki-windows-x86_64-unsigned-nsis` Artifact；它对应提交 `3af0728`，保留至 2026-09-23。公开旧基线仍可从 [`v0.1.0-alpha.6` GitHub Release](https://github.com/chai-yinfeng/Atogaki_Subtitle/releases/tag/v0.1.0-alpha.6) 获取，但不要用旧包执行本轮增量矩阵。
2. 在解压目录打开 PowerShell，校验安装器旁的 SHA-256：

```powershell
$installer = Get-ChildItem "*-setup.exe" | Select-Object -First 1
$expected = (Get-Content "$($installer.FullName).sha256").Split()[0].ToLowerInvariant()
$actual = (Get-FileHash -Algorithm SHA256 $installer.FullName).Hash.ToLowerInvariant()
$actual -eq $expected
```

结果必须是 `True`。该包没有商业代码签名，SmartScreen 提示是当前 alpha 的已知交付边界；确认文件来自上述 Actions 运行且哈希匹配后，知情测试者可在“更多信息”中选择继续运行。不要为测试全局关闭 SmartScreen 或杀毒软件。

## 1. 快速安装与窗口冒烟

- 以普通用户运行 NSIS，确认不要求管理员权限；安装后从开始菜单启动 Atogaki。
- 确认主窗口图标、中文界面、设置弹窗、工作台和收听入口正常；窗口缩放和最小化／恢复没有空白或崩溃；启动 App、识别和烧录时不应同时出现终端窗口。
- 打开悬浮字幕，检查置顶、拖动、缩放、关闭、任务栏表现，以及主窗口退出后不残留进程；有多显示器时移动到不同缩放比例的屏幕。
- 未配置 provider 时打开设置和工作台不应弹出 Credential Manager 错误，也不应要求系统安装 FFmpeg／Whisper。

## 2. Windows 系统集成

- 保存一个测试 provider Key，执行检查、更新和清除；确认 Key 不出现在界面错误、SQLite、任务目录、截图或日志。需要检查系统存储时，只确认 Windows Credential Manager 中存在 Atogaki 凭据，不复制密钥值。
- 分别选择带空格、中文文件名、非系统盘和较长目录下的媒体／模型；确认原生文件面板返回正确路径。
- 验证环境、直连和自定义代理模式；模型镜像失败后应回退官方源，下载完成必须通过 SHA-256。第一轮可只下载较小模型，长模型留到闭环测试。
- 导出任意字幕或完成一次视频烧录后点击“在 Explorer 中显示”，确认 Explorer 选中正确文件，而不只是打开父目录。
- 安装、运行和卸载全程不应依赖开发工具 `PATH`。如果设备已经安装开发工具，另在 Windows Sandbox／干净账户复核一次。

## 3. 真实媒体闭环

- 使用一段英语或韩语真实音频和一段常见 MP4 视频，记录媒体时长、模型、CPU 处理耗时和内存体感。
- 完成 CPU Whisper／VAD 识别、一个云 provider 翻译、原文与译文编辑、待重译、词表应用和重启恢复。
- 导出原文 SRT、译文 SRT、双语 SRT、双语 ASS；抽查中文、日文或韩文字体 fallback 和时间轴。
- 使用内置 FFmpeg 烧录原文、译文或双语视频。Windows 首版应显示 `MPEG-4 软件编码`，不应把正常结果描述为 VideoToolbox 失败；记录输出体积、画质和音频处理方式。
- 移动原媒体后重新定位，验证播放、字幕编辑和烧录恢复；再验证任务重命名、从冻结快照重试、失败取消和任务删除边界。

## 4. 卸载与结果记录

- 卸载 Atogaki 后确认应用程序和开始菜单入口被移除；用户原媒体、主动导出的字幕／视频不得删除。
- 第一轮不要求卸载时删除 Atogaki 应用数据。记录应用数据是否保留，后续再决定是否提供显式“同时清除本地数据”选项。
- 将结果按“通过／失败／未测”记录，并附设备信息。阻塞问题优先级依次为：无法安装或启动、凭据泄露／数据损坏、sidecar 无法运行、识别／导出闭环失败、窗口与文案问题。

## 下一候选的 alpha.6 后增量矩阵

以下项目不能因为 Windows 编译门禁通过而标记完成；必须在最终 NSIS 安装版本中逐项记录“通过／失败／未测”：

- 从 alpha.6 覆盖安装，确认学习收藏、词典、字幕样式和任务排序 migration 不丢失原任务、字幕、词表、provider 设置或人工修改。
- 收藏词／语法／整句并从学习区返回来源播放；分别测试日语、英语分册和无词典包时的降级行为。
- 下载一个离线词典包并重启查询；ECDICT 需记录首次索引耗时、峰值内存体感和索引文件大小。Collins／Merriam-Webster 只验证各自 Credential Manager 项目，不复制或记录 Key。
- 浏览 Windows 系统字体，检查不存在字体、fallback 和缺字提示；用同一任务比较 libass 预览、ASS 文件及 MPEG-4 烧录成品。
- 用实体鼠标拖动任务卡、键盘调整顺序并重启；高 DPI 或触控板结果单独记录。
- 中断一次模型下载后续传，并测试不支持 Range 的源回退为完整重下，不留下误判为可用的临时文件。
- 用短日语片段检查《響け！ユーフォニアム》词表快照和 prompt；确认不会产生“表記”说明文本，U4／UFO 修正只按确定性规则应用。
- 让 DeepSeek 至少一批成功后制造后续失败，重启再续跑；确认已有译文保留、空译文有段号提示、另一个任务可以独立翻译。
- 对一个明确范围执行局部重新识别，先核对预览，再确认替换；取消预览不得改动 SQLite，失败不得遗留活动子进程或临时字幕。
- 用有声长视频验证开头、后段、随机 seek 和任务切换均有声音；再用只可回退 `audio.wav` 的样本。必须区分媒体本身静音与 WebView2 音轨未解码。
- 分别选择三档视频质量，核对原分辨率、目标码率、预计大小和最终大小；Windows 应显示 MPEG-4 软件编码，不出现 VideoToolbox 文案。
- 用 Windows 自带“媒体播放器”打开烧录成品；若先提示编码格式不受支持但仍能播放，记录成品详情中的视频编码器与音频处理方式，并保留媒体播放器版本。当前 LGPL 基线的视频是 MPEG-4 Part 2，源音频优先直通；在没有成品 stream 信息前，不把提示单独归因于视频或音频，也不以“能继续播放”视为兼容性通过。

悬浮字幕异常在本轮源码同步中暂缓，不作为其他 alpha.6 后功能进入 Windows 编译基线的阻塞项。当前实机反馈包括点击无反应，以及点击后主窗口消失、无法从界面唤回、只能通过任务管理器结束进程；在修复前，Windows Release 必须明确标注该功能不稳定且不建议使用。若后续候选仍要专门测试，则在 100%、125%、150% 和 200% 缩放下记录窗口是否创建、是否空白、首条字幕是否出现、播放跨段是否更新、置顶、拖动、缩放、关闭、Alt+Tab／任务栏、多显示器移动和主窗口退出行为；若决定隐藏入口，也必须确认快捷键和旧窗口状态不能绕过限制重新打开。

## 当前实机记录

2026-09-09 在提交 `3af0728` 上完成[预览修复候选流水线 34308936563](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/34308936563)。除既有配置、构建、许可证、PE、sidecar 与卸载门禁外，安装后的 Windows FFmpeg 已实际通过 DirectWrite/libass 把测试 ASS 渲染为 MJPEG 图片；约 26.9 MiB 的新安装包 Artifact 保留至 2026-09-23。发布 job 因未提供 release tag 按预期跳过。仍需在实机字体界面确认任务视频帧、CJK 字体选择与反复刷新。

2026-09-09 对提交 `f7e5651` 的候选实测发现字幕样式／字体界面无法生成预览，错误摘要只显示 `ffmpeg version 8.1.2`。候选构建清单确认 Windows sidecar 在 `--disable-autodetect` 下没有 PNG 所需的 zlib，而预览固定请求 PNG；烧录使用 MPEG-4，不经过 PNG，因此最终视频仍可成功。修复让预览在 PNG 不可用时使用 FFmpeg 原生 MJPEG，并让错误摘要跳过版本 banner、展示实际错误；安装器门禁新增一次真实的 libass 图片渲染，不再只检查 `ass` filter 名称。该候选同时观察到 Windows 自带媒体播放器先报告编码不支持、随后仍可播放并显示烧录字幕；视频为预期的 MPEG-4 Part 2，但音频是否直通以及具体 stream 信息尚待从成品记录确认。

2026-09-09 在提交 `f7e5651` 上手动运行[未发布 Windows 候选流水线 34302977783](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/34302977783)。固定源码构建的 MSVC CPU Whisper、LGPL-only FFmpeg／ffprobe、Windows 配置断言、前端与 Rust 许可证、Tauri release 编译、current-user NSIS、安装后 PE／sidecar／合规资源检查及静默卸载全部通过。已上传约 26.9 MiB 的 `Atogaki-windows-x86_64-unsigned-nsis` 和约 72.4 MiB 的 `Atogaki-windows-x86_64-sidecars`，均保留至 2026-09-23；未提供 release tag，发布 job 按预期跳过。该记录只证明候选包可构建并在 CI runner 安装，不替代本页实机矩阵。

2026-09-08 将 alpha.6 后共享功能和 Windows 配置同步到提交 `10df74d`；[Windows 原生编译门禁 34280369794](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/34280369794)通过配置断言、前端构建、Rust 格式、共享核心测试和 Tauri 桌面壳编译。本轮刻意未生成安装包，也没有把悬浮字幕标记为已修复；后续实机结果从本页增量矩阵继续记录。

2026-08-22 第一轮候选可正常启动，模型下载已进入实际传输，翻译 provider API 调用通过。该候选启动时会同时出现无用途的终端窗口，已在 `32581475065` 修复并增加 CI PE GUI 子系统门禁；覆盖安装后优先复核 App 启动、识别和烧录三个阶段均不出现终端，再继续其余清单。

2026-08-23 修复候选在当前实机测试范围内未再发现其他问题，允许将同一代码基线发布为 `v0.1.0-alpha.6`，由更多 Windows 用户扩大覆盖。尚未逐项记录的设备、路径、悬浮字幕、多显示器、真实英语／韩语媒体和卸载边界继续按本清单反馈，不因进入 prerelease 自动视为通过。

2026-08-23 `v0.1.0-alpha.6` 已由[固定 tag 发布流水线](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/32622817441)完成冷构建、哈希核对、静默安装／卸载冒烟并发布。后续测试统一从 Release 获取带版本名的最终安装包，不再转发短期 Actions Artifact。

2026-08-24 扩测报告悬浮字幕存在显示异常；具体表现、触发窗口状态、显示器与缩放配置仍待补充。修复前请保留截图、Windows OS build、显示器数量／缩放、打开悬浮字幕前后的操作，以及问题是否随播放跨段变化；该缺陷留待下一次 Windows 稳定候选集中处理。
