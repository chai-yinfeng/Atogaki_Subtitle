# 0044：稳定桌面能力范围，模型与管线实验先在 CLI 验证

**日期：** 2026-09-18  
**状态：** accepted

## 背景

P1–P4 为验证分层管线、本地翻译和云端 ASR，连续把实验入口加入桌面 App，并多次生成独立 Test App。这样可以快速完成真实窗口验证，但模型、prompt、分段策略或 provider 合同每变化一次，都要重新编译、签名、安装并区分多个同 bundle identifier 的 App，开发成本和误用风险已经高于收益。

## 决策

桌面 App 固定为个人日常使用的稳定工作台，能力边界见 [桌面能力矩阵](../app-capability-matrix.md)。macOS 正式本地构建包含 FFmpeg、whisper.cpp 和 `llama-server` sidecar，只安装为 `/Applications/Atogaki.app`；不再创建 P3/P4 等 Test App，也不为普通开发提交生成测试 DMG。

后续 ASR／translation 模型替换、实时协议、prompt、分组、commit policy、时间轴算法和质量基准先通过 CLI 或独立 evaluation harness 验证。实验必须复用 application provider／planner 合同和版本化 artifact，使用隔离输出目录，不直接修改正式 SQLite 工作区。

实验只有同时满足以下条件才讨论进入桌面 App：

1. 在固定开发集和 holdout 上有可复现的质量或工作流收益。
2. 错误、取消、隐私、凭据和 artifact 合同已经明确。
3. 不破坏 cue revision、人工编辑、学习来源和导出兼容性。
4. 用户确认该能力值得扩大桌面产品范围。

## 结果

- 桌面 UI 不再跟随每个研究分支变化，真实使用测试围绕一个正式 App 展开。
- CLI 成为模型和 pipeline 研究面，但不另建一套业务逻辑；可复用的改进仍下沉到 core/application。
- 公开 Release、Windows 打包和公证流程仍是独立门禁；本机覆盖安装不等于发布。
- Gemini 全任务转录、Muse／实时 ASR、E2E live translate 和新模型默认值继续留在 CLI／research，直到完成上述晋升条件。
