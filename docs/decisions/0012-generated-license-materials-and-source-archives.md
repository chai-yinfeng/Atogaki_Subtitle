# 0012：生成许可证材料并独立发布对应源码

日期：2026-08-06
状态：已采用

## 背景

App 同时包含 Rust/Tauri 依赖、编译后的前端依赖和独立 sidecar。只保留上游链接无法证明某个 Release 对应的版本，也容易在依赖升级时漏掉许可证。把所有第三方源码塞进 DMG 会显著增加每位用户的下载量；只依赖 GitHub 自动生成的仓库源码包又不包含 FFmpeg 等上游源码。

## 决定

- 使用固定版本的 `cargo-about` 从 Tauri lockfile 生成 Rust 依赖许可证 HTML，并用仓库脚本从 npm lockfile 生成前端许可证 HTML；两份结果提交到 `src-tauri/third-party/` 并随 App Bundle 分发。
- sidecar 构建和源码归档共用 `scripts/sidecar-versions.zsh` 中的版本、URL 与 SHA-256；二进制构建清单也记录每份源码哈希。
- 每个 Release 单独生成包含 FFmpeg、静态字幕栈和 whisper.cpp 精确源码、许可证、构建脚本和清单的压缩包。
- 对应源码包与 DMG 放在同一 GitHub Release，作为独立 asset，不写入 Git 历史，也不塞入 DMG。
- 许可证生成和源码归档采用 fail-closed：未知许可证、缺失许可文本、target/lockfile 漂移或哈希不一致时停止发布。

## 后果

- 普通用户只下载 App；需要检查或重建 sidecar 的用户可以取得同版本源码包。
- Git 仓库会增加约 1 MiB 的可审查 HTML 声明，但不会增加数十 MiB 的源码归档和平台二进制。
- 每次升级 Rust/npm lockfile 或 sidecar 都必须重新生成材料并审阅 diff。
- 当前自动化只覆盖 macOS arm64。新 target 必须生成自己的依赖报告、构建清单和对应源码资产，不能直接复用本报告的结论。
