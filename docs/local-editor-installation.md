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


## 紧凑工作台试装（0058）

继上述历史试装，本次从 `8a85af0` 完整 release 构建覆盖 `/Applications/Atogaki.app`，正式 identifier 与 0.1.2 版本号保持，包含四个 sidecar；完整包文件摘要与构建产物一致，深度签名通过。启动后及打开 3,089 段真实任务并短暂定位播放后，20 张表的内容摘要均与安装前一致（14 个任务／9,976 段）。正式字幕未执行文字或结构编辑，未调用识别或云端翻译。App 已停在字幕编辑页并暂停播放，供用户验收。

最终 QA 包先以 `/tmp/atogaki-compact-native/data` 的隔离数据库和合成 H.264 视频验证大视频／右侧上下双语布局、中性色波形、播放跟随，以及 Cut→撤销由 20→21→20 段恢复；读 SQLite 核验 cue ID、时间、原译文和编辑标记与操作前完全一致，integrity_check=ok。最终截图与 QA 结果保留在 `/tmp/atogaki-compact-native/`。

本次仅新增一份一致性 SQLite 快照，不复制模型、媒体或整个数据目录。保留上一版 `/Applications/.Atogaki.previous.20261021-compact` 和稳定版 `/Applications/.Atogaki.previous-20261020T210406Z`。验证新包和两份回退签名后，移除被取代的 `.Atogaki.previous.20261021-editor` 及三份过时临时 QA App；合计约 310 MiB 逻辑副本内容，物理释放量受 APFS 影响。旧 QA 数据、trace 与证据全部保留。

本次记录位于忽略目录 `local-artifacts/install-backups/20261021-compact/`：一致性快照、20 表前后摘要、构建／安装／回退文件清单，以及轮换清理前后记录。没有新增公开发行资产。后续整体模块拆分、滚动 P95／长时内存和 Windows 原生验证仍按路线图推进，不能把本机试装当作这些项目全部完成。

## 模块与处理工具试装（86c46c8）

从干净提交 `86c46c8` 完整构建四 sidecar release 包，正式 identifier 与 0.1.2 版本号保持；五个可执行文件、完整文件清单与深度签名均通过核验。已原子覆盖并启动 `/Applications/Atogaki.app`。安装前应用暂停且无未保存文字，使用正常退出路径；正式 14 个任务均已完成，没有中断后台运行。

启动后与打开 3,089 段真实电影任务后，全部 20 张表摘要均与安装前一致，14 个任务／9,976 段保留，integrity_check=ok。原文／译文上下排列，视频与右侧波形＋编辑框、底部列表以及原有布局比例保留；媒体播放和跟随正常，最后暂停于原任务 00:01:46.991／第 15 段供用户试用。正式字幕未执行文字、时间、结构或候选操作，也没有调用识别或云端翻译。清理确认 AX、候选历史自动保护、焦点返回及采用／撤销使用隔离合成数据验证，64 项前端、九组 WebKit 和完整原生构建通过。

本次保留上一版 `/Applications/.Atogaki.previous.20261021-modules`（8a85af0）及稳定版 `/Applications/.Atogaki.previous-20261020T210406Z`（0.1.1）。新包启动与数据一致性通过后，核对清单再删除被取代的 `.Atogaki.previous.20261021-compact` 和旧 `/tmp/atogaki-compact-native/Atogaki.app`，两份 App 合计 175,985,912 字节，约 168 MiB 逻辑内容；旧 QA 数据、合成媒体和证据仍保留。

新的一致性 SQLite 快照与上次 `20261021-compact/before-install.sqlite` 的 SHA-256 完全相同。保留旧文件并校验 integrity_check 后，新快照路径改为相对链接，恢复路径和字节内容不变，不再保留另一份 34,852,864 字节（约 33 MiB）的副本。其他独立数据恢复点、模型、主题、媒体与 ASR 来源均未清理；APFS 共享块使上述逻辑大小不能视为精确空闲增量。

预检、安装、两次 20 表验证、签名文件清单、截图／AX、轮换清理与快照去重记录在忽略目录 `local-artifacts/install-backups/20261021-modules/`。本次为本机试装，没有生成或更新公开 Release。剩余 Rust／repository 与处理工具拆分、滚动 P95／长时内存、文档整理和 Windows 验收继续按路线图推进。
