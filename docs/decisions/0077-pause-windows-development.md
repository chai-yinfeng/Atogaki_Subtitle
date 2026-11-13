# 0077：暂停 Windows 开发与自动 CI

日期：2026-11-13。状态：accepted，用户明确要求暂不考虑 Windows，停止每次推送触发的 Windows CI 与邮件提醒。

## 决策

当前开发、验证与后续发行聚焦 macOS Apple Silicon。暂停 Windows 共享代码适配、编译／sidecar 自动 CI、候选打包和专项实机验收；不因 Windows 尚未跟进而阻塞当前 macOS 工作。

两条 workflow（Windows compile baseline、Windows sidecars）在 GitHub 仓库端停用；源码移除 push 与 pull_request 触发，仅保留 workflow_dispatch。未来恢复必须先显式启用，再手动运行并重新确认维护范围，不会因普通提交自动恢复。

Windows 源码、平台配置、脚本、许可证材料、alpha.10 发行资产及历史验证保留。暂停维护不等于已删除 Windows 支持代码，也不把旧版本称为当前已验收版本。语言与存储兼容边界不随平台暂停而删除。

## 原因与范围

用户当前使用与开发以 macOS 为主，后续可能参考 Voxt 的 native MLX 路线。Windows 已长期未跟进，自动构建失败会持续发邮件，并不代表这一平台正在维护。

确认最近两次失败来自本仓库 main 的 e085fa8／6c8e21d：运行 43360675955／43361256945，均已结束，无在途任务需要取消。本次停止触发，不排查或修复 Windows 编译错误，不更改用户 GitHub 通知偏好，也不修改外部上游仓库。

## 后继关系

本记录取代 0031、产品方向和发布流程中“选定候选继续更新 Windows”的当前维护承诺；既有历史结果不变。恢复 Windows 需另行确认需求、runtime、平台依赖与验收范围。

macOS 既有 Release／DMG 安装和 ad-hoc 签名流程保持。runtime 分发形式、量化模型默认值、自动更新器与更高在线系统要求仍需在相应讨论后确定，本记录不批准这些实现变更。
