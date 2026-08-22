# 0031：Windows CI 分层并按稳定候选更新安装包

日期：2026-08-22
状态：已采用

## 背景

Windows 原生编译和 NSIS 自动化已经建立。一次缓存命中的 Windows 编译基线约需五分钟，完整许可证、Release 编译、安装冒烟和 Artifact 上传约需八至九分钟；后者即使没有重编 FFmpeg／Whisper，也明显重于普通提交所需的反馈。

Atogaki 当前发行与交互质量基线仍是 macOS Apple Silicon。Rust 业务层、Tauri 命令和前端由两个平台共享，但 Windows 实机测试和版本更新预计集中在少量稳定候选节点，而不是跟随每个 macOS 开发提交。

## 决定

- 不建立长期 Windows 产品分支。平台共享同一业务代码，功能分支按产品能力或具体修复划分。
- Windows 编译基线作为合并门禁：修改共享 Rust、Tauri、前端或构建依赖的 pull request，以及进入 `main` 的对应提交自动运行；普通功能分支 push 和纯文档变更不运行。任何 commit 仍可用 `workflow_dispatch` 手动验证。
- 完整 Windows sidecar／许可证／NSIS 流水线不因普通产品代码变化自动运行。只有 Windows 构建脚本、sidecar 版本、合规生成器、安装冒烟或 Windows 专属打包配置变化时，pull request 和 `main` 自动验证。
- 准备 Windows 稳定候选时，从选定的 commit 手动触发完整流水线，下载带校验值的 Artifact，并在 Windows 11 x86_64 实机完成发布清单。通过前不得把 Artifact 当作公开 Release。
- macOS 继续承担日常核心开发和窗口体验回归；这不允许共享代码引入已知的 Windows 不可编译路径，也不降低 Windows 候选的许可证、安装与真实媒体门禁。

## 后果

- 日常 macOS 开发不会为每次分支 push 生成 Windows 安装包，Actions 时间和无效 Artifact 数量显著减少。
- Windows 编译问题最迟在 PR／`main` 合并边界暴露，而不是拖到发布候选才首次发现。
- 普通产品代码合并后不会自动产生最新 Windows 安装包；Windows 发版负责人必须显式选择候选 commit 并手动启动完整流水线。
- 长期分支漂移和跨平台重复实现被避免，但稳定候选可能一次集中暴露多个 Windows 运行时问题，因此实机回归仍是不可省略的发布门禁。
