# 旧 Web Serve／Postgres 退役影响报告

日期：2026-10-22。审计基线：`511a8f9`。状态：影响分析已完成，退役方案待讨论；本报告不授权删除入口、迁移或数据库。

当前桌面工作流无需 Postgres，但旧实现仍是可调用的 CLI 和公开 Rust API。它增加编译依赖、许可证材料和维护范围；不能仅凭桌面没有调用，就将其认定为无人使用的死代码。建议后续先讨论非默认 `legacy-web` feature 隔离，再决定兼容期与删除时机。本轮不新增 feature，也不改产品默认行为。

## 调用与兼容范围

| 边界 | 仓库内证据 | 退役影响 |
|---|---|---|
| CLI | `src/interface/cli.rs` 的 `Command::Serve`／`ServeArgs`，`src/main.rs` 分派到 `interface::web::serve` | 删除会使既有 `atogaki serve` 启动命令及其参数失效；改成非默认 feature 也会改变默认 CLI 帮助与可用命令 |
| HTTP | `src/interface/web/mod.rs` 只注册 `GET /` 与 `GET /api/health` | 已有探活、启动脚本或首页访问会失效；未发现任务、上传、认证或编辑 API 已在这条路由树实现 |
| 环境与参数 | `DATABASE_URL`、`ATOGAKI_BIND`、`ATOGAKI_JWT_SECRET`、`ATOGAKI_JOBS_DIR`、`ATOGAKI_UPLOADS_DIR`、`ATOGAKI_WHISPER_MODEL`、`ATOGAKI_VAD_MODEL`、`ATOGAKI_WEB_WORKERS` | 均属于 ServeArgs 的兼容面，不能因部分参数目前未执行对应业务就静默忽略；`jwt_secret` 已解析但 serve 未使用，首页的未来功能文案也不代表已有实现 |
| Postgres API | `src/infrastructure/mod.rs` 公开 `db`；`Database`／`CreateFile`／`models` 在公开 library 下导出 | 仓库内外部调用无法完全推断；移除会破坏 Rust 下游编译。仓库内只有 web 模块引用此适配器，其他同名 Database 是字体数据库 |
| 持久结构 | `migrations/202606160001_create_core_tables.sql` 与 `202608090001_add_task_languages.sql`；`sqlx::migrate!("./migrations")` | 涉及 users、jobs、files、subtitle_segments、glossaries、glossary_terms、job_events。保留已有 schema 迁移来源与历史版本；不执行 DROP、不转换或删除用户可能存在的服务端数据库 |

`serve` 在配置 DATABASE_URL 时连接并执行迁移；未配置时也可启动上述两条 HTTP 路由。health 的 `database_connected` 仅反映启动时持有 Database，并没有持续 ping 数据库。jobs／uploads／模型路径和 worker 数目前只是传入状态并展示于 health。CORS 为 permissive，不能把该早期外壳当成已完成认证与权限边界的公共服务。

仓库 README 与 architecture 已将它标为早期探索；检索 `.github/`、`scripts/` 未发现维护中的 Serve 部署流程或 Postgres 服务测试。没有在本次审计连接任何 Postgres 实例，也没有收集仓库外使用情况，因此“外部无人使用”与“现存数据库可清理”均未获证明。

## 依赖与必须保留的边界

根 `Cargo.toml` 无条件启用 `sqlx/postgres`，并声明 axum／tower-http；桌面依赖根 library，因此共享 feature 使桌面仍编译 Postgres driver。实际执行：

```sh
cargo tree --offline --manifest-path src-tauri/Cargo.toml -i sqlx-postgres --edges normal
```

结果包含 `sqlx-postgres → sqlx → atogaki-subtitle → atogaki-desktop`，并包含 `sqlx-macros-core` 的构建分支。这证明依赖存在，不能据此推断最终二进制保留所有代码或承诺安装包能缩小多少。未来隔离后还需重新测量构建、依赖树、锁文件与签名包。

根 crate 中 axum／tower-http 的源代码调用目前集中在旧 web 模块，存在可选依赖候选；但 `src-tauri/Cargo.toml` 也直接依赖 axum，`src-tauri/src/media_server.rs` 用它提供已登记媒体的 loopback Range 流式访问。该媒体服务的 token、Host／Origin、生命周期和 64 KiB 读取与旧 Web 产品外壳无关，必须保留。不能按依赖名称或函数名 `serve` 批量删代码。

桌面 SQLite 则由 `src/infrastructure/local_db.rs` 和 `sqlx::migrate!("./migrations/sqlite")` 持有独立迁移；任务、稳定 cue ID、revision、候选采用事务与恢复证据继续保留。根目录两份 Postgres SQL 与 `migrations/sqlite/` 不共用一套 schema，禁止删除整个 migrations 目录。日期与 UUID 等共享依赖也不能只因旧 db 模块使用就移除。

`src-tauri/third-party/rust-licenses.html` 和 Windows 对应材料包含 sqlx-postgres。未来实际改变 feature 时，须从选定 target 的真实依赖重新生成许可证，不手动删列表条目；macOS 与 Windows 的依赖／打包结果分别验收。

## 待讨论的实施选择

1. **保持现状。** CLI 兼容性不变，继续承担默认构建与公开 API 的维护成本；至少更新首页误导性未来计划文案。
2. **非默认 feature 隔离。** 同时 gate CLI variant／参数、main 分派、interface::web 和 infrastructure::db，并把根 axum／tower-http／Postgres feature 对齐到这一开关。旧使用者需明确启用；过渡期保留源码与 SQL。这是兼容迁移候选，不是本轮已采纳的架构决策。
3. **删除旧产品外壳。** 在确认使用情况和公告兼容变更后单独提交；保留可定位的历史版本与迁移档案。不会顺带删除服务端用户数据、SQLite 或桌面媒体服务。

选择方案后新增 ADR，明确默认构建、旧 CLI 的迁移提示、library API 和兼容期限。若选择 feature，验证默认与显式启用两套 CLI 帮助和编译，并检查两套 macro feature；若需要证明 Postgres 迁移兼容性，另用专用空实例及隔离测试数据，不能借用未知服务端库。

实际改动的最低回归还应包括共享 Rust／Tauri 检查、SQLite 恢复与候选采用事务、原媒体与 WAV 后段播放及 Range 服务、前端构建、依赖／许可证重生成与 Windows 编译。当前报告的静态调用检索与 cargo tree 不替代这些未来实施验收。
