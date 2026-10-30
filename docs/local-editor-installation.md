# 统一字幕编辑器本机试装与备份整理

日期：2026-10-21。用户明确授权覆盖本地 App，并清理过早的安装备份；不发布新版公开资产。

当前安装为 2026-10-29 的 `28e2909`，见[最新交付](#2026-10-29按需音频与词表修复候选28e2909)；以下按日期保留历史记录。

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

## 2026-10-23：接缝修复与后续编辑模块候选（05b2e07）

从干净提交 `05b2e07` 完整 release 构建，正式 identifier `com.chai-yinfeng.atogaki`、版本号 0.1.2 保持，包含 ffmpeg、ffprobe、whisper-cli、llama-server。前端 104 项测试、类型检查和生产构建通过；本提交共享库 157 passed／5 环境依赖 ignored、桌面 33 passed／4 ignored 的结果见[短片段基线](godfather-short-baseline.md)。这次本机更新不修改公开 v0.1.2 资产。

为避免正式 WebView 存储混入测试，另构建同提交的完整 `editor-qa` identifier 包，复用原 QA App 位置和两任务／40 段合成数据库。两包四个 sidecar 摘要一致；原生检查播放行与编辑框同步跟随、单击锁定后继续播放、恢复跟随、视频折叠期间播放时钟持续推进、导出格式 Escape 返回焦点、真实 libass 样式预览及关闭后的焦点，以及返回编辑时仍处于原暂停时间／当前段。QA 正常退出后 20 张表摘要完全一致，integrity_check=ok。没有保存样式、修改字幕、触发 ASR 或调用云端 API。

正式 App 在空闲任务列表正常退出，13 个任务均已完成。使用 SQLite backup API 新增一份 34,852,864 字节的一致性快照；此前用户授权删除旧《教父》任务，所以本次快照不是之前 14 任务快照的重复内容。没有复制模型、媒体或整个数据目录。

暂存正式包核验后原子替换 `/Applications/Atogaki.app`。完整 27 文件摘要与候选包一致，主程序和四个 sidecar 均存在，深度签名通过。启动后及打开 Daily English Podcast（405 段）、定位第一句并短暂播放后，正式数据库 20 张表的逐表内容摘要均与安装前一致，13 个任务／6,887 段保留，integrity_check=ok。最后暂停于 00:00:09.788／第 1 段、开启跟随供用户试用；没有修改正式字幕或自动重建既有识别结果。接缝修复由[真实片段 CLI／原始证据重放](godfather-short-baseline.md)验证，本次原生检查不冒充又执行了一次识别。

保留 `/Applications/.Atogaki.previous.20261023-candidate`（86c46c8）和 `/Applications/.Atogaki.previous-20261020T210406Z`（0.1.1）。新包与数据通过核验后，按旧清单完整摘要核对并删除被取代的 `.Atogaki.previous.20261021-modules`（8a85af0）和本次已安装的暂存包，共 202,448,538 字节、约 193 MiB 逻辑副本内容；APFS 共享块使它不等于精确空闲空间增量。仍只保留两份 App 回退，不逐步追加备份；其他独立数据库恢复点和 ASR 证据保留。

本次候选／正式／QA 包摘要、20 表前后摘要、一致性快照、轮换清单，以及隔离／正式截图与 AX 在忽略目录 `local-artifacts/install-backups/20261023-candidate/`。剩余滚动 P95、稳定负载 RSS、偶发锁定、系统文件面板、Windows 和翻译质量／缓存观测门禁继续保留，不能把试装成功当作全部验收完成。


## 2026-10-25：输入保护与后续模块候选（1020e05）

从干净提交 `1020e05` 分别构建隔离与正式 identifier 的完整四 sidecar release 包。两包 ffmpeg／ffprobe／whisper-cli／llama-server 摘要一致，正式包版本仍为 0.1.2，完整 27 文件与安装包一致，deep／strict 签名通过。继承当前 248 项前端及八组相关 WebKit 结果；本次打包重新运行类型／生产构建，没有重复跑 Rust 全量测试。公开发行资产未改变。

复用原 QA App 和两任务／40 段合成数据，原生验证开始时间改为 0.100 秒后返回任务受阻，输入保留并提示保存或 Escape；导出工具往返仍保留 0.100 秒，Escape 恢复到 0 后可以离页，同任务重开显示已保存时间。QA 正常退出，20 表内容摘要不变，integrity_check=ok、foreign_key_check 为空。文字 flush 的延迟／故障竞态仍由合成 IPC 回归覆盖，不将本次原生操作扩大为所有异步失败验收。

正式 App 安装前未运行，13 任务均 done，共 6,887 段。SQLite backup API 创建一致性快照后，完整 SHA-256 与 20261023-candidate 的恢复点相同；新路径改为相对链接复用旧文件，没有新增 34,852,864 字节的数据库副本，也未复制媒体、模型或整个数据目录。再次确认进程停止、数据库摘要未变、原 App 与 05b2e07 安装清单一致后，经暂存包核验与重命名原子替换 `/Applications/Atogaki.app`，原包作为上一版回退保留。

直接执行主程序的首次启动一度只显示空白窗口；同一进程随后加载任务列表，未通过重启或修改数据使其恢复。三秒主进程 sample 在采样区间显示事件循环等待，不能据此证明整个启动无阻塞或断言根因。正常退出并确认原启动进程 exit 0 后，再从系统应用入口打开，首次工具返回已显示工作台和 13 任务；0.779 秒是工具调用耗时，不是应用 TTI。首次空白的原因继续保留观察，不能宣称已修复启动性能。

启动、打开 405 段英语任务及第二次启动／打开后，20 张表内容摘要全部与安装前一致，完整性检查通过；正式字幕未执行文字、时间、结构、候选或翻译操作。时间码定位短暂播放后已暂停，最终停在 Daily English Podcast 第 1 段 00:00:09.847，跟随开启，供用户手动试用。未自动重建旧识别或更改翻译分组／上下文。

现只保留 `/Applications/.Atogaki.previous.20261025-time-guards`（05b2e07）和 `/Applications/.Atogaki.previous-20261020T210406Z`（稳定 0.1.1）。核对新包、两份保留回退签名及旧回退全文件摘要后，删除被取代的 `.Atogaki.previous.20261023-candidate`（86c46c8），清理 101,227,093 字节、约 96.5 MiB 逻辑副本内容；不等同于 APFS 实际空闲增量。

安装／快照链接／27 文件／20 表／轮换／调用栈和 AX／截图清单保存在忽略目录 `local-artifacts/install-backups/20261025-time-guards/`；QA 证据在 `local-artifacts/editor-time-native-1020e05/`。原生帧 P95、持续多操作内存、偶发锁定、首次启动空白、文件面板及 Windows 门禁仍保留，本机可试用不等于整体计划验收完成。


## 2026-10-27：统一编辑器与完整来源撤销候选（0cf72c0）

按用户此前“覆盖本地 App 手动测试”的授权，将干净 0cf72c0 构建为正式 identifier 的 complete 四 sidecar release。候选包含紧凑上下双语布局、默认跟随／手动锁段、后端 token 来源／依赖撤销、上下文即时状态、近期生命周期拆分、波形首尾重复读取和候选说明遮挡修复。后两项界面及近期工具已在独立 QA c8afa5f 等节点验证；之后两次 repository 纯抽取已逐字核对并通过共享库回归。公开 macOS v0.1.2 资产未变，本机版本号仍为 0.1.2。

安装前重跑前端 346 passed、桌面 Rust 35 passed／4 环境依赖 ignored；正式 release 内含类型／生产构建，两套 fmt／diff 通过，源／暂存／安装包 deep strict codesign 与完整 27 文件一致。共享库 183／5 ignored 对应 0cf72c0 之前的本轮实现检查，没有在文档提交重复运行。首次进程预检误匹配自身命令文本而停止，改成 comm 可执行路径精确匹配后确认正式 App 无进程；此时未替换文件或数据。

正式目录 13 个任务均 done，6,887 段。SQLite backup API 创建本次一致性恢复点，SHA-256 与旧快照不同，因此保留一份新快照，没有复制媒体、模型或整个数据目录。替换前再核对旧 1020e05 包与原安装清单一致、20 表摘要没有变化；源包暂存校验后原子重命名安装，旧包保留为上一版回退。恢复点、hash 与核验脚本见 local-artifacts/install-backups/20261027-unified，不将数据库放入 Git。

经系统应用入口启动后直接显示 13 任务和正式数据路径；只读打开 Daily English Podcast（405 段），时间码定位首句后短暂播放、暂停并恢复跟随。最终停在第 2 段 00:00:13.572，视频画面／波形／播放行／上下双语一致，窗口保持打开供用户手动验收。没有修改正式文字、时间、结构、采用或触发模型。安装前／替换前／打开后全部 20 表逐表内容摘要一致，integrity_check=ok、foreign_key_check 为空；这个读写边界不等于将正式数据用于故障测试。

核对新包、上一版和稳定版签名，以及旧回退的完整文件摘要后，只删除被取代的 05b2e07 回退 `/Applications/.Atogaki.previous.20261025-time-guards`，约 101,221,445 字节逻辑内容；APFS 实际回收量不作推断。当前保留 `/Applications/.Atogaki.previous.20261027-unified`（1020e05）及 `/Applications/.Atogaki.previous-20261020T210406Z`（稳定 0.1.1），没有持续追加 App 副本。数据库恢复点、模型和采用证据未清理。

新旧包／恢复点／20 表摘要、构建／测试／签名、回退轮换、AX／截图与 verification 在 local-artifacts/install-backups/20261027-unified。剩余原生滚动 P95、长期混合负载内存、拖动工具、其他 IO／异常终止及 Windows 等门禁仍保留；本机交付给用户试用不等于四阶段总体完成。


## 2026-10-28：波形定位跟随修复（e8d9a8d）

0cf72c0 用户反馈普通波形定位后视窗停止跟随，已按[缺陷记录](editor-waveform-follow-interaction-bug.md)修复并经六组 WebKit、346 项前端、类型／构建及独立 QA 实际跨窗播放验证。旧正式 App 已退出，13 任务均 done，核对旧包完整清单后，以暂存／签名／原子重命名更新 /Applications/Atogaki.app。当前源码与本机 e8d9a8d 同步，版本号仍为 0.1.2，本次不更新公开发行。

一致性恢复点 SHA 与 20261027-unified 相同，使用相对符号链接复用，没有新增数据库副本。安装前和系统入口打开后的 20 表内容摘要一致，13 任务／6,887 段保留，integrity ok、foreign key 空；正式只启动任务页供用户继续验收，没有进行字幕写入、故障或模型请求。

保留 /Applications/.Atogaki.previous.20261028-wave-follow（0cf72c0）和稳定 .Atogaki.previous-20261020T210406Z。核对新包、两份回退签名与旧回退完整 checksum 后，清理被取代的 .Atogaki.previous.20261027-unified（1020e05），逻辑 101,233,557 字节；其他数据／恢复点／模型／采用证据保留。日志、签名／27 文件、恢复点链接、20 表摘要、QA 原生截图与 rotation／verification 在 local-artifacts/wave-follow-fix-20261028。用户复验和其余性能／平台门禁不由本次安装代替。

## 2026-10-29：按需音频与词表修复候选（28e2909）

按既有覆盖本机 App 授权，从干净 28e2909 构建正式 identifier 的 complete 四 sidecar release；产品代码对应已通过原生 QA 的 778d936，后继仅为测试与验收记录。包含编辑页音频按需加载／恢复意图保护、词表迟到返回及修正弹窗访问／单次保存修复，保留已手动验收的紧凑布局与跟随。前端 349、按需专项及十四组受影响 WebKit、Tauri 36／4 ignored 对应源码已通过；本次正式包再次运行类型与生产构建、两套 fmt／diff，不重复冒称新一轮全量测试。公开 macOS v0.1.2 资产未改，本机版本号仍为 0.1.2。

精确 comm 核对正式 App 无进程，13 个任务均 done。SQLite backup API 一致性快照与 20261027-unified 的 SHA 完全相同，改为相对链接复用，没有新增数据库副本或复制模型／媒体。核对原 e8d9a8d 完整清单及正式库 20 表不变后，源／暂存／安装包 deep strict 签名和 27 文件摘要通过，四个 sidecar 与 QA 相同；原子重命名更新 `/Applications/Atogaki.app`。

系统应用入口启动后显示 13 任务；只读打开 405 段英语视频，初始 0 秒画面为黑色，定位首句短暂播放后显示视频画面，继而暂停到 00:00:16.379／第 3 段，恢复跟随。视频／波形／当前上下原译文及列表一致，窗口保留供用户试用；没有执行文字、时间、结构、词表应用、ASR 或翻译写入。前后 20 表内容摘要完全一致，13 任务／6,887 段保留，integrity=ok、foreign key 空。首次黑色画面不单独作为首帧解码已完成的证明；媒体播放由实际后续帧验证。

新包／两份回退签名及旧回退全清单核验后，只删除被替代的 0cf72c0 `/Applications/.Atogaki.previous.20261028-wave-follow`，逻辑 101,571,957 bytes；APFS 实际回收不作推断。保留上一版 e8d9a8d `/Applications/.Atogaki.previous.20261029-audio-demand` 与稳定 0.1.1 `.Atogaki.previous-20261020T210406Z`，未持续增加 App 回退。其他数据库恢复点、模型、媒体和采用证据保留。

私有清单、构建／安装脚本、恢复点复用、签名／27 文件、20 表与轮换、AX／实际查看截图、verification／manifest 位于 `local-artifacts/install-backups/20261029-audio-demand/`。本机试装通过不关闭原生 P95、TTI、首次实际加载暂态、其他 IO／原生拖动和 Windows 等门禁；新版本人工反馈待收集。


## 2026-10-30：视频覆盖确认与画质说明（ac321b8）

沿用既有本机覆盖授权，从干净 ac321b8 构建正式 identifier 的 complete 四 sidecar release。本次包含视频输出父窗口绑定、粘贴／重复输出路径的实时存在性检查与覆盖确认、请求期间选项／任务变化保护，以及推荐 SRT／ASS 搭配原视频、烧录重新编码和码率不保证保真的说明。既有紧凑布局、上下原译文、跟随和按需音频保持；351 项前端测试、类型／生产构建、Tauri 36 passed／4 环境依赖 ignored、WebKit 烧录专项及隔离实际视频取消／确认覆盖均已通过。正式构建再次完成类型／生产构建、两套 fmt／diff；公开 v0.1.2 资产未改。

安装前精确进程检查确认正式 App 已退出，旧 28e2909 的 27 文件与安装清单相同，13 任务均 done。SQLite backup API 新快照 34,852,864 bytes；与上次快照 checksum 不同，旧／新表摘要仅 local_settings 改变，因此本轮保留新恢复点，未复制整个数据目录、模型或媒体。替换前再核对全部 20 表不变；源包、暂存、安装包签名与 27 文件一致，四 sidecar 与本轮 QA checksum 相同；原子重命名更新 `/Applications/Atogaki.app`，版本号仍为 0.1.2。

经系统 App 入口启动显示正式数据路径和 13 任务，只读打开 Daily English Podcast 的 405 段，检查导出页和烧录界面的新说明／标准码率标签后返回编辑器；未提交烧录、字幕写入、识别或翻译请求。保留暂停 0 秒／跟随开启的窗口供用户试用；首段从 9.320 秒开始，0 秒空白区没有当前字幕符合跟随规则，不把禁用输入框误判为加载失败。安装前／替换前／打开后全部 20 表摘要相同，integrity=ok、foreign key 空，13 任务／6,887 段保留。

核对新包和两份回退签名、旧包全清单后，仅移除被取代的 e8d9a8d `.Atogaki.previous.20261029-audio-demand`（101,572,213 bytes 逻辑内容）及本轮已正常退出的 `/tmp/atogaki-render-overwrite-qa/Atogaki QA.app`（101,635,326 bytes）；不推断 APFS 实际回收量。保留上一版 28e2909 `.Atogaki.previous.20261030-render-output` 和稳定 0.1.1 `.Atogaki.previous-20261020T210406Z`。其他 QA、恢复点、模型、媒体、采用证据与未知文件保留。

私有安装／恢复脚本、27 文件 checksum、数据库摘要、实际窗口 AX／截图与轮换清单位于 `local-artifacts/install-backups/20261030-render-output/`；修复前后隔离证据在 `local-artifacts/editor-native-render-overwrite-20261030/`。本机更新不代表原生性能、异常恢复组合、系统 Save 面板或 Windows 门禁全部完成；新版人工反馈待收集。


用户随后对 ac321b8 的“粘贴已有 MP4 路径 → 开始烧录 → 取消覆盖”回答“正常”，该新增交互的人工复验通过；不外推系统保存面板、确认覆盖、运行中取消或其他未验边界。
