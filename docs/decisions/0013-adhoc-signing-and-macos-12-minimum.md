# 0013：预发布采用 ad-hoc 签名并要求 macOS 12

日期：2026-08-06
状态：已采用

## 背景

Atogaki 当前没有 Apple Developer Program 账号。完全跳过 Tauri 签名时，Apple Silicon 链接器只会给主二进制留下临时签名，整个带资源和 sidecar 的 App Bundle 无法通过 `codesign --verify --deep --strict`。同时，Tauri 默认生成的 `Info.plist` 宣称最低 macOS 10.13，但 sidecar 统一按 `MACOSX_DEPLOYMENT_TARGET=12.0` 构建，安装声明与实际可运行范围不一致。

## 决定

- macOS 预发布包使用 Tauri `signingIdentity: "-"` 对 App Bundle 做 ad-hoc 签名。
- App 的 `minimumSystemVersion` 固定为 `12.0`，与 FFmpeg 和 whisper.cpp sidecar 的 deployment target 一致。
- 构建命令不再使用 `--no-sign`。每个候选包必须通过 `codesign --verify --deep --strict`，并检查所有 Mach-O 都是目标架构且不链接 Homebrew 路径。
- Release notes 必须明确说明该包是 ad-hoc 签名、未公证；不得把它描述为 Apple 认证或已验证开发者版本。
- 取得 Apple Developer Program 资格后，用 Developer ID Application 签名与公证替换 ad-hoc identity，不改变 Bundle identifier。

## 后果

- 不需要开发者账号也能得到内部完整性有效的 Apple Silicon Bundle。
- 外部下载仍可能被 Gatekeeper 拦截，测试者需要按 Release notes 在系统设置中明确允许；ad-hoc 签名不能建立开发者身份或消除该提示。
- macOS 10.13–11 用户不会被误导为受支持；当前正式最低系统为 macOS 12.0。
