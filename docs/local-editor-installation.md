# 统一字幕编辑器本机试装与备份整理

日期：2026-10-21。用户明确授权覆盖本地 App，并清理过早的安装备份；不发布新版公开资产。

## 安装结果

源码 `ed48fc2`，正式 identifier `com.chai-yinfeng.atogaki`，release 包包含 ffmpeg、ffprobe、whisper-cli、llama-server。暂存包核验后原子替换 `/Applications/Atogaki.app`，深度签名有效，主程序及四个 sidecar 的 SHA-256 与构建产物一致。版本号仍为 0.1.2，属于本机试装，不代表公开 v0.1.2 资产发生变化。

正式数据目录仍为 `~/Library/Application Support/com.chai-yinfeng.atogaki`。安装前使用 SQLite backup API 保存一致性备份，integrity_check=ok。启动后 14 个任务、9,976 段字幕保留；全部 20 张表按行排序的内容摘要与安装前相同。原生窗口打开 3,089 段长任务，确认视频／时间轴在上方、居中双语正文及其上方两行紧凑操作栏。没有修改正式字幕或调用识别／云端翻译。

## 本次保留与清理

- 保留本次安装前完整 App：`/Applications/.Atogaki.previous.20261021-editor`。
- 保留较早稳定版：`/Applications/.Atogaki.previous-20261020T210406Z`（0.1.1）。两份回退包均通过深度签名核验。
- 移除 21 份已过时的开发安装 App 副本。清理前记录路径、用途、版本、主程序摘要及完整包文件清单摘要；不删除发行 DMG／源码包、模型、主题、任务媒体或 ASR 证据。
- 17 份历史 SQLite 文件中有三组 SHA-256 完全相同的副本。校验保留文件 integrity_check=ok 后，将三份重复文件改为指向保留副本的相对链接，旧恢复路径与字节内容保持可用。其余独立恢复点、修复前数据库及人工评审不变。

移除的 App 与重复数据库合计逻辑大小约 1.93 GiB；APFS 共享块会影响实际可回收物理空间，不将此数表述为精确磁盘空闲增量。

本次一致性数据库、20 张表摘要、五个安装可执行文件摘要、清理前后清单位于当前工作树忽略目录 `local-artifacts/install-backups/20261021-editor/`：`before-install.sqlite`、`before-install.json`、`after-launch.json`、`installed.json`、`backup-inventory.json`、`cleanup-plan.json`、`cleanup-result.json`。不将个人任务数据库或原始清单提交 Git。

## 后续本机试装约定

普通界面迭代轮换保留一份安装前 App，并保留一个已验证稳定回退版，不再每一步追加完整 App 副本。数据备份使用 SQLite 一致性快照；完全相同副本可合并并保留原路径链接，不能仅按日期删除修复／迁移前的独立证据。常规安装不复制模型、媒体和整个数据目录。

现有 `install-current-macos-app.sh` 是构建与短暂回退替换助手，不负责上述 SQLite 备份和保留清单；不能把单独运行该脚本视为已经完成数据备份流程。Web Serve／Postgres 退役审计与其他产物整理仍独立推进。
