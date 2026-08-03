# 架构与目录约定

_最后更新：2026-08-03_

## 仓库组织

`Atogaki_Sub` 保持为 Rust/Cargo 项目根目录。不要为了放置文档而把 `src/` 再嵌套一层：根目录的 `Cargo.toml` 与 `src/` 是标准 Rust 布局，移动它们会增加构建、编辑器和未来 Tauri 集成的复杂度。

```text
Atogaki_Sub/
├── Cargo.toml                 # Rust package 与依赖
├── src/                       # 可复用的处理核心与当前 CLI
│   ├── lib.rs                 # 供 CLI 与未来桌面 UI 复用的库入口
│   ├── application/           # 用例、任务规格、状态与编排
│   ├── domain/                # 字幕段、词表、分段与导出规则
│   ├── infrastructure/        # ffmpeg、Whisper、DeepL、文件与数据库适配器
│   └── interface/             # CLI；未来会新增桌面 UI 适配层
├── assets/                    # 版本化的示例和内置词表
├── migrations/                # 现有 Postgres 实验性迁移；不作为桌面 MVP 依赖
├── docs/                      # 产品方向、路线图、架构与决策记录
└── tests/                     # 跨模块集成测试（按需要新增）
```

未来引入 Tauri 时，优先在本仓库根目录新增 `src-tauri/` 和前端目录（例如 `ui/`），并让当前 Rust 处理核心转为独立库 crate 或 workspace member。不要把现有 `src/` 移入一个人为的子目录。

## 目标桌面架构

```text
桌面 UI（Tauri）
  └─ application：导入、任务、编辑、翻译、导出用例
       ├─ domain：时间轴字幕、词表、字幕格式
       └─ infrastructure：文件系统、SQLite、ffmpeg、ASR、翻译服务
```

UI 不直接启动 ffmpeg、Whisper 或 DeepL。它只调用 `application` 中的用例并订阅任务状态。当前 CLI 同样是该应用层的一个适配器。

`LocalTaskService` 是桌面端长任务的第一层服务：提交时立即创建带 `queued` 状态的 UUID 任务目录，后台 worker 再调用 `JobRunner`。UI 通过 `JobSnapshot` 轮询持久化状态。默认仅启动一个 worker，避免本地 ASR 模型争抢 CPU、内存或 GPU；多 worker 只能由显式配置启用。

## 数据边界

- 媒体、模型、任务产物：默认本地文件系统。
- 任务元数据、字幕段、词表与编辑状态：桌面 MVP 使用 SQLite。
- 密钥：macOS Keychain（或同等系统密钥链），不写入任务 JSON。
- `status.json`：保留为任务产物与故障恢复副本，不作为唯一长期数据库。

## 核心数据约定

- 任务目录以 UUID 命名，避免并发任务冲突。
- 字幕段拥有稳定 ID、开始与结束时间、原文、译文、来源编辑状态和翻译过期状态；读取旧 JSON 时会自动补齐 ID 并迁移写回。
- 应用层选项不得引用 `clap`、HTTP 或桌面框架类型。接口层负责转换。
- 外部工具和 API 是可替换基础设施：ASR、翻译和媒体处理分别通过应用层选项接入。CLI 默认日语识别、简中翻译，但不将该默认值写死到应用层或 DeepL 适配器。
