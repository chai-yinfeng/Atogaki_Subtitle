# 0059：桌面命令与 SQLite repository 的渐进模块边界

日期：2026-10-22

状态：已采纳；落实统一编辑器计划的 Rust 分拆，命令与 repository 分别提交，不改持久化合同。

## 决策

Tauri 命令按应用用例放到 `src-tauri/src/commands/`，保留现有命令名、参数键、serde 命名、返回字段和错误转换。模块只适配 IPC 与应用服务，不增加保存器、数据库连接池、任务状态或第二层事务；调用方仍使用现有前端 adapter。

第一条边界是 `commands/editor.rs`：详情读取、文字保存／恢复、插入删除、Cut／Join／结构恢复、时间保存、播放位置与波形窗口，共 11 条命令及其 9 个请求／返回类型。详情仍只 refresh 当前任务，继续使用进程唯一的 PlaybackMediaRegistry 登记文件。`DesktopState` 的构造与服务注册仍由 main 持有，子模块借用同一实例；共用任务展示类型也暂留原位置，不借抽取另造 DTO 副本。

main 的 generate_handler 使用显式模块路径，保持可审阅的完整注册表。模块函数与跨模块 DTO 只开放 crate 可见性，不扩大根 library API。服务层继续负责校验和用例，SQLite repository 继续负责 revision 与事务；命令层不直接执行 SQL。

下一步 repository 使用同一个 LocalDatabase 的内聚子模块，按查询、文字／时间／结构写入、来源／候选等边界逐条移动。一次抽取保留原 SQL、事务开始／提交顺序、辅助函数和错误行为；不把现有单次原子操作拆成多次独立提交，也不在模块内重建连接池。先完成命令注册与真实 IPC 验证，再独立提交 repository 拆分。

## 取舍与验证

继续留在 main 可以避免移动，但会继续混合应用构造、平台窗口、设置和工作区合同；一次搬走所有命令或替换成通用 dispatch／新框架，会放大宏注册与序列化兼容风险。因此每次只迁移一条完整领域边界，以编译确认宏路径，以现有服务测试和隔离原生调用确认运行时合同。不能把 source diff 无业务改动或 browser mock IPC 单独当作真实 Tauri 注册已通过。

本决策不改变 schema、导出格式、原译 cue 对应、稳定 ID、revision、草稿、候选采用事务、SQLite 迁移及媒体服务。前端余下工具、repository 与文档整理仍逐阶段推进；Windows、滚动与持续内存门禁继续单独记录。
