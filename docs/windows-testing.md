# Windows 11 实机测试清单

_适用候选：2026-08-22 Windows Actions `32575359901`；未签名 alpha，仅供知情测试_

## 测试边界

第一轮先验证安装包与 Windows 系统集成，不要求立即跑完整长节目。第二轮再使用一段英语或韩语真实媒体完成识别、翻译、编辑、导出和烧录。发现问题时记录测试步骤、实际结果、截图或完整错误文本；API Key 不进入截图或日志。

测试前记录 Windows 版本与 OS build、CPU、内存、显示器数量／缩放比例，以及设备是否安装过 Rust、Node、FFmpeg、Whisper 或 Atogaki。优先使用没有这些开发工具的 Windows 11 x86_64 设备、虚拟机、Windows Sandbox 或独立测试账户。

## 0. 获取并校验候选

1. 打开成功运行 [Windows sidecars 32575359901](https://github.com/chai-yinfeng/Atogaki_Subtitle/actions/runs/32575359901)，下载 `Atogaki-windows-x86_64-unsigned-nsis` Artifact 并解压。
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
