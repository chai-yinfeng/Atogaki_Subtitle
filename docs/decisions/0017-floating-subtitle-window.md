# 0017：悬浮字幕使用独立、临时的桌面窗口

日期：2026-08-09

## 背景

学习电台或播客时，用户常把节目挂在一侧，同时浏览网页、笔记或其他应用。主工作区内的播放器字幕不能覆盖这种多任务场景；把主窗口设为置顶又会遮住时间轴、编辑和其他内容。

## 决定

- 用户在已打开的任务工作区显式点击后，创建或显示一个独立的 Tauri Webview 窗口。它只展示当前播放位置的原文与译文，并应支持跨普通桌面的置顶、可靠拖动、可见缩放控制和单独关闭。
- 悬浮窗是临时播放视图，不写入 SQLite，不创建新的任务，也不接管媒体播放。主工作区仍是播放、跳转、编辑、翻译和导出的唯一控制面。
- 主窗口仅在字幕段变化或当前字幕文本被编辑后向悬浮窗同步内容；新开的悬浮窗从主进程读取最近一次快照，以避免窗口加载时丢失首条字幕。
- macOS 使用真正的 AppKit `NSPanel` 承载原有 WebView 内容，面板采用 `NonactivatingPanel`、`NSScreenSaverWindowLevel`、`CanJoinAllSpaces`、`Stationary`、`FullScreenAuxiliary`，且仅在 macOS 13+ 加 `CanJoinAllApplications`。打开悬浮字幕时主 App 继续保持 `Regular`，不主动降低主窗口层级；用户切换离开主窗口后才临时切为 `Accessory` 以覆盖其他 App 的全屏 Space，主窗口再次获得焦点或关闭悬浮字幕时恢复 `Regular`。
- 不启用 Tauri 的 `macos-private-api`，也不使用通过 Objective-C runtime 把现有 `NSWindow` 强制改类为 `NSPanel` 的第三方插件。原生面板通过公开初始化器创建，再接管 Tauri 已创建的 WebView content view。

## 后果

- 2026-08-11 两轮使用普通 `NSWindow` 的“全屏 Space”打包实测都未显示窗口；第二轮的失焦重新置前还造成关闭后再次出现的回归，因此该补偿已删除。`NSPanel + Accessory` 的 DMG 实测已确认可以覆盖其他应用的全屏 Space。首轮面板错误复用了原 Tauri `NSWindow` 的 tao delegate，拖动或缩放会在 tao 的窗口回调中 abort；原生面板现不设置该 delegate，并隐藏无对应产品行为的系统红黄绿按钮。
- 刚打开悬浮字幕时 Atogaki 的 Dock 图标、菜单栏和主窗口保持不变；主动切换到其他 App 后，Dock 图标和菜单栏才会随临时 `Accessory` 身份隐藏，回到主窗口或关闭悬浮字幕后恢复。若未来要求悬浮期间主 App 在其他 App 前台时也始终保持普通 Dock 身份，则需拆出独立的 accessory helper 进程。
- 屏幕保护级别意味着悬浮字幕可能高于其他 App 的全屏控制条或弹窗；这是“始终覆盖全屏应用”的明确取舍。窗口保留没有红黄绿按钮的原生标题栏与边框，用于稳定拖动和边缘缩放；面板内“×”是唯一关闭入口，Web 内容继续按可用宽高同步缩放。当前不提供全局快捷键、点击穿透、跨设备记住位置或独立播放控制。
- 悬浮窗口首先在 macOS 打包 App 回归。Windows/Linux 的窗口层级和透明效果不能仅凭 macOS 结果推断。
