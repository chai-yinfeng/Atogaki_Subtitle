# 0015：使用签名的 GitHub Release 应用内更新

_日期：2026-08-07_

## 状态

已采纳。

## 背景

Atogaki 通过 GitHub Releases 直接分发，不进入 Mac App Store。仅提供 DMG 会让用户每次都重新下载并手工替换 App；后台强制更新或自动重启又可能打断转写、模型下载和视频烧录。

## 决策

- 使用官方 Tauri v2 updater 和 GitHub Release 中的静态 `latest.json`。
- App 启动后异步检查一次；检查失败或离线时不影响现有初始化，也不显示错误弹窗。
- 只有发现更高版本时才显示更新按钮，下载和安装必须由用户主动触发。
- 安装完成后不自动重启；用户在当前任务安全结束后自行退出并重新打开 App。
- 更新产物必须由独立的 Tauri updater 私钥签名，并通过编译时注入的 `TAURI_UPDATER_PUBKEY` 验证。未配置公钥的构建不注册 updater 插件，也不会发起更新请求。
- `createUpdaterArtifacts` 只放在正式发布显式加载的 `tauri.updater.conf.json` 中，避免没有更新私钥的日常开发和结构 smoke build 失败。
- 前端只授予检查以及“下载并安装”两项 updater 权限，不开放不需要的独立下载或安装命令。

## 后果

发布者必须长期安全保存 updater 私钥；私钥丢失后，已经安装旧公钥版本的客户端无法验证后续更新。该签名只保护更新链路，不能替代 macOS Developer ID 签名与公证。Windows 安装器会在安装阶段自动退出应用；macOS 不会被 Atogaki 主动重启。
