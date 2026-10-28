# 字幕编辑器：性能与接口验证

日期：2026-10-21。分支：`codex/unified-subtitle-editor`。设计见 [0056](decisions/0056-editor-virtual-list-and-task-summary.md)。

## 方法与结果

同一台 Apple Silicon MacBook Air、24 GB 内存，1180×760 视口，Vite 生产构建，Playwright 官方 WebKit build 2336。改造前使用 `3459d46` 的独立源码构建，改造后使用本次实现；两者共用 `editor-ui-fixture.cjs` 和 `measure-editor-webkit.cjs`。300／3,000／10,000 段分别覆盖 10 分钟／30 分钟／三小时的字幕时间范围，文本与状态相同。IPC 和波形数据为合成返回；浏览器音频仅为 20 秒静音，因此浏览器后段导航测的是字幕模型，真实三小时 seek 另在原生窗口验证。

| 段数 | 首次可见行，前→后（ms） | 最大挂载行，前→后 | 输入事件，前→后（ms） | 搜索事件，前→后（ms） | 滚动帧 P95，前→后（ms） |
|---|---:|---:|---:|---:|---:|
| 300 | 182 → 133 | 300 → 20 | 2 → <1 | 3 → 2 | 27 → 30 |
| 3,000 | 406 → 121 | 3,000 → 20 | 11 → 1 | 45 → 3 | 28 → 30 |
| 10,000 | 1,160 → 148 | 10,000 → 20 | 29 → 1 | 122 → 6 | 26 → 27 |

首次可见行包含 Playwright 点击和等待的端到端耗时，不是纯 JavaScript 执行时间或真实磁盘任务加载耗时；事件耗时为同步 dispatchEvent 处理时间。单次采样适合确认规模增长趋势，不能作为稳定的绝对性能承诺。10,000 段页面元素数从 61,004 降到 1,072；三个规模的改造后 DOM 数一致。End 定位最后一句约 3 ms，搜索复用同一虚拟列表。

每个规模滚动 90 帧、往返两遍，采样前后均未出现 >50 ms 的滚动帧；P95 没有改善，不能把 DOM 减少表述为滚动帧性能已经改善。后续仍需真实任务的连续滚轮／触控板长时测量。WebKit 不提供可直接等同 Chromium Long Tasks 的完整采样，本报告也不把帧间隔当作主线程长任务。

关闭 trace 后另做四轮滚动，10,000 段进程树 RSS 前为 256,496 KiB（四轮相同），后为 124,352／124,416／124,464／124,416 KiB；短时观察未见持续上升。RSS 包含引擎／驱动和回收波动，不能替代 JS heap 或证明长期无泄漏；3,000 段也出现约 280 MiB 的瞬时 RSS。每个样本均创建独立 context 并关闭。

同一次任务打开，详情调用从两次降至一次；停留编辑页超过列表轮询周期后，旧 `list_jobs` 调用为 0，新摘要仅进入工作台时读取一次。后端测试同时验证：损坏 segments 快照、证据 JSON 和不可读取的 provider 输出不会影响卡片摘要；完整恢复校验拒绝该检查点，已有 SQLite 字幕保持不变。未改变恢复或候选采用的完整校验路径。

## 回归与原生窗口

- 前端 28 项测试、类型检查、生产构建通过；新增虚拟窗口边界与按 ID 更新后播放／时间窗索引一致性回归。
- `verify-editor-ui.cjs` 在 Chromium 和 WebKit 通过：快速切段、650 ms 自动保存、IME、失败重试、筛选隐藏当前段、工具返回、布局记忆、大字号及 1180×760／960×640；新增 End／Home 跨视口编辑保存与轮询调用断言。
- WebKit 曾复现失焦保存后下一次点击消失，原因是列表重挂仍在点击中的节点；改为保留已有行节点，回归通过。
- 核心 Rust 148 通过／5 忽略；Tauri 33 通过／4 忽略；格式与 diff 检查通过。忽略项需要真实 sidecar／模型或平台环境。
- 原生 QA App 使用独立 identifier `com.chai-yinfeng.atogaki.editor-qa`，专用数据库新增三小时静音 WAV／10,000 段字幕。End 定位到 #10000／02:59:58.920，时间码定位后播放推进，折叠视频与波形仍继续播放且编辑段保持。#9990 修改后切段，读 SQLite 确认正文与 source_edited 持久化；原有两句 QA 人工编辑不变。工具返回保留 #9991。
- 对独立 QA WebContent PID 记录 30.8 秒 Instruments Time Profiler；`potential-hangs`（≥250 ms）为空。这是单次导航样本，没有改造前原生 CPU 对照，也不能推断所有 ≥50 ms 长任务不存在。

首次无缓存打开三小时合成 WAV 时曾出现音频时长暂为 0、波形持续加载；缓存建立并重启后正常加载三小时时长与后段播放。随后将波形缓存移出并保留副本，最终 QA 包再次冷启动可正常加载三小时时长；该暂态未复现，仍保留观察，不宣称根因已修复。Windows、真实长视频解码／持续操作和正式导出未在本次性能采样中重新验收。

## 复现与产物

先运行 `npm --prefix ui run build`，使用本机静态 HTTP 服务提供 `ui/dist`。脚本只使用合成媒体和模拟 IPC，不访问正式任务、模型或 provider。

```sh
PLAYWRIGHT_BROWSERS_PATH=/path/to/test-browsers \
ATOGAKI_PLAYWRIGHT_MODULE=/absolute/path/to/node_modules/playwright \
ATOGAKI_EDITOR_URL=http://127.0.0.1:1430 \
ATOGAKI_EDITOR_QA_OUTPUT=local-artifacts/editor-performance/2026-10-21 \
ATOGAKI_MEASUREMENT_LABEL=after node scripts/measure-editor-webkit.cjs

ATOGAKI_EDITOR_ENGINE=webkit node scripts/verify-editor-ui.cjs
```

本机原始 JSON 与六份 Playwright trace 在忽略目录 `local-artifacts/editor-performance/2026-10-21/`；原生 Instruments trace 在 `/tmp/atogaki-editor-native/phase2-webcontent.trace`。这些不是公开发行资产，不进入 Git。临时 App 为 `/tmp/atogaki-editor-native/Atogaki.app`；未覆盖正式 `/Applications/Atogaki.app`。

虚拟列表、局部索引和摘要接口已交付；滚动帧改善与持续内存仍是剩余项，不能标记全部性能门禁完成。

## 第三阶段补充：工具生命周期

翻译／导出区域、识别复核 dialog 已按需连接与卸载文档，复核条目在关闭时释放；单份表单实例保留输入，尚未进行代码分包或延迟分配全部控件。WebKit 回归新增隐藏工具无连接 DOM、重复开关／Escape／编辑焦点和进行中任务只读页。此后界面布局已改变，前述测量保留为第二阶段当时的对比，不直接当作新布局的绝对指标。

## 紧凑布局后续

0058 将虚拟行高改为 28 px，DOM 仍按视口与 overscan 有界分配。新增 10,000 段后段跟随／手动浏览／恢复的功能回归，不把它作为滚动 P95 或长时内存门禁通过证据；本记录原 trace 对应当时的 42 px 行高。

## 2026-10-22：减少滚动中的重复行更新

以 `31fdde1` 的紧凑布局为基线，同机、同一 WebKit 和 1180×760 视口重新测量。`editor-cue-list.ts` 现在缓存已绘制的行范围；上下边界都未改变时不更新 DOM。范围改变只创建／释放进入／离开的行，保留行不重复格式化文本或写属性。显式 render 仍刷新数据、筛选后位置与状态，update／mark 仍局部更新保存和播放标记；clear 重置缓存。不回收正在点击的节点，不增加第二份字幕数据。

新增 `scripts/measure-editor-scroll.cjs`，每个规模三轮、每轮 180 帧、7 px／帧，从相同中段起点滚动。MutationObserver 只统计已连接列表树中的属性更新；新行在连接前的初始化不在此计数内。RAF 包装统计全页回调的同步累计时间，既包含应用也包含测量回调，精度约 1 ms，不能等同原生 CPU profile。

| 段数 | 每轮已连接节点属性更新，前→后 | 每轮新增／释放行 | RAF 累计耗时中位数，前→后（ms） | 帧间隔 P95，前→后（ms） |
|---|---:|---:|---:|---:|
| 300 | 12,147 → 0 | 45／45 | 89 → 32 | 17 → 17 |
| 3,000 | 12,147 → 0 | 45／45 | 83 → 23 | 17 → 17 |
| 10,000 | 12,147 → 0 | 45／45 | 88 → 24 | 17 → 17 |

表中 P95 是三轮各自 P95 的中位数；单轮范围 17–18 ms，所有样本均未超过 50 ms，最大挂载行始终 23。重复属性更新已消除，回调累计工作减少；帧率没有可证实的提升。

原 `measure-editor-webkit.cjs` 的 90 帧大幅跳转与 trace 也重跑：300／3,000／10,000 段 P95 分别为 30→29、27→29、27→27 ms，没有稳定改善；首次可见行分别为 147→140、141→113、122→120 ms，搜索为 3→3、3→4、6→7 ms。仍各读取一次详情／一次摘要、没有 list_jobs 轮询。短时四轮滚动 RSS 也不能替代真实任务长时内存门禁。

六份 trace 和前后 JSON 保留在忽略目录 `local-artifacts/editor-performance/2026-10-22/`。使用前述环境变量，分别运行 `node scripts/measure-editor-webkit.cjs` 与 `node scripts/measure-editor-scroll.cjs`；前后必须各自使用对应源码构建，避免将旧布局与新布局混为一次对比。

新增 `scripts/verify-editor-list.cjs` 验证固定窗口内零 DOM 变更、跨行保留节点／焦点、局部文字保存、筛选后索引／位置、空结果恢复、End 与离页重入。面板高度可能不是 28 px 的整数倍，因此测试同时检查上下视口边界，不能假设滚动 1 px 一定不改变行范围。64 项前端测试、类型／生产构建、格式及 diff 检查通过；列表专项和原编辑／跟随／紧凑布局三组 WebKit 回归均通过。本次未覆盖本机 App，真实任务持续内存、原生滚轮／触控板和 Windows 验收仍待补。

## 2026-10-22：真实长任务隔离窗口与内存观察

从 `511a8f9` 构建并验证签名的独立 identifier debug QA 包，复用 `/tmp/atogaki-modules-native-790957e/Atogaki.app`；工作数据另存 `long-task-data/`，没有新增安装回退副本或覆盖正式 App。来源为已保留的恢复快照；14 个任务的数据库存储路径全部改到隔离目录，只复制《教父》任务文件，原视频和两份任务音频只读使用。不是正式库上的破坏性测试，没有调用 ASR／翻译服务。

原生窗口确认 3,089 段真实双语字幕与 3:22:52 左右的原视频。通过 End 选择 #3089、时间码定位到 03:18:01.540 并播放，恢复跟随后编辑框与实际播放段对应；双折叠期间继续到 03:22 后。复核工具显示 435 项，成功完成八轮打开／关闭；搜索 America 命中 11 段，通过原生清除按钮恢复 3,089 段，再定位到 01:57:49 后继续播放。AX setValue 空字符串未清除搜索，改用可见的取消按钮；不把这一自动化限制认定为应用输入故障。

新增 `scripts/sample-editor-rss.py` 对已确认的四个 PID 每 5 秒读取 RSS，记录 Unix／单调时间、完整进程身份及缺失／变化，拒绝覆盖已有证据。600.023 秒共 121 个样本，四个进程均未退出或更换；WebKit XPC 的 PPID 为 1，不能只把主进程的子树当成整个 App。启动前没有其他同名 WebKit 进程，显式选定本次新增 PID，不将其他 App 内存算入。

| 进程 | 起点→终点（MiB） | 最小／最大（MiB） |
|---|---:|---:|
| QA 主进程 | 127.4 → 130.0 | 115.6／134.7 |
| WebKit GPU | 51.8 → 97.9 | 41.7／110.7 |
| WebContent | 69.2 → 126.2 | 62.7／145.8 |
| Networking | 11.2 → 248.1 | 5.0／308.5 |
| RSS 求和 | 259.5 → 602.2 | 225.1／611.2 |

这不是“没有持续增长”的通过证据。起点、双折叠、工具挂载和两次长距离视频定位属于不同负载；Networking 缓冲与 WebContent 分配的变化不能直接等同 JS 泄漏。RSS 还包括共享页，简单求和不等于系统独占物理内存。没有强制 GC，没有测 JS heap，也没有数小时稳定负载或旧包对照。中途五分钟观察低于启动前另一次读数，不能替代本表的完整采样起点；最终以原始记录为准。

离开编辑页回工作台后另采样 60.021 秒、13 个样本：RSS 总和 604.1→321.3 MiB，Networking 239.5→24.6 MiB、WebContent 133.8→110.7 MiB、GPU 101.2→55.6 MiB、主进程 129.6→130.3 MiB。媒体卸载后明显回收，不能单凭此证明稳定负载无泄漏。原生滚动／按键自动化间歇返回 `noWindowsAvailable`，截图也出现 ScreenCaptureKit -3812；继续读取同一窗口可用，未重启 App 来掩盖退出。失败批次的重复 Home／End 和连续滚动未计为通过，因此原生滚轮／触控板帧耗时门禁仍未完成。

采样后曾观察到编辑锁定 #1973、播放标记 #1978，本轮没有追溯到具体触发动作；显式恢复后，持续播放至 #1985 时编辑段、播放标记与黄色行一致。该观察保留待复查，不将已通过的浏览器跟随回归扩展成原生所有输入均已通过。

SQLite integrity_check=ok、foreign_key_check 为空；隔离副本的 9,976 段（含目标 3,089 段）逐字段与恢复快照相同。构建／fixture 身份在 `/tmp/atogaki-modules-native-790957e/long-task-before.json`，RSS、AX、截图、八轮工具结果和数据核验在 `local-artifacts/editor-performance/2026-10-22/native-*`。这次补充真实数据与释放观察，持续内存、原生滚动 P95 和 Windows 仍不能勾为全部通过。


## 2026-10-23：播放时钟中的重复行状态与标题更新

滚动路径复查发现另一个确定的热路径：每次 updateKaraokePosition 都调用 mark，即使编辑段和播放段 ID 完全不变，也会对同一行写 4 个 ARIA／tabIndex 属性；未变化的段落时间标题同样每次用 textContent 重建。mark 现仅在两个 ID 有变化时补丁行；文字、草稿与结构仍走 render／update。当前时钟和段落标题只在值变化时写入，播放头绘制及时间窗逻辑不变。

新增 `scripts/measure-editor-clock.cjs`，对 16ee370 未修改源码生产构建与本次构建依次测量，同机 WebKit 2336、1180×760、同一合成 IPC／静音媒体／三小时时间模型，分别 300／3,000／10,000 段。媒体暂停在固定时刻，每轮 120 个 RAF 间隔派发 timeupdate，三轮；MutationObserver 统计行和段落标题，performance.now 统计同步事件处理。没有运行解码播放、滚轮、原生 CPU profiler、JS heap 或 RSS。本机计时约 1 ms 精度，结果只适用于这一 UI 回调工作负载。

| 段数 | 每轮重复行属性写入，前→后 | 每轮标题重建，前→后 | 同步回调累计耗时中位数，前→后（ms） | 单回调 P95，前→后（ms） |
|---|---:|---:|---:|---:|
| 300 | 480 → 0 | 120 → 0 | 155 → 41 | 2 → 1 |
| 3,000 | 480 → 0 | 120 → 0 | 149 → 41 | 2 → 1 |
| 10,000 | 480 → 0 | 120 → 0 | 159 → 43 | 2 → 1 |

每轮均为 cue-0、14 个挂载行（列表头部单侧 overscan），没有行增删或页面错误，所有样本无 >50 ms 帧。该回调工作负载帧间隔 P95 的三轮中位数为 18→17 ms，后样本单轮 17–18 ms，不能据此将先前“大跨度滚动 P95”门禁改为通过。

104 项前端测试、类型／生产构建、两套 Cargo fmt 和 diff 检查通过。列表专项新增固定时钟零行／标题变更，同时保留局部保存、筛选后索引、空结果、End、重入和焦点验证；跟随专项验证实际播放过段、手动锁定、空白、IME、失败保存、筛选与 Cut；编辑器布局／保存回归同步通过。Rust 源码未变，未重跑 Rust 测试；本轮未更新原生 QA 或正式 App。

原始前后 JSON 和源码／夹具身份在忽略目录 `local-artifacts/editor-performance/2026-10-23-clock/`，脚本拒绝覆盖同名结果。复现沿用前述环境变量，将 ATOGAKI_MEASUREMENT_LABEL 分别设为唯一标签运行 `node scripts/measure-editor-clock.cjs`。这里关闭的是重复 UI 写入，不将结果外推为真实滚动、播放解码或稳态内存通过；上述原生门禁继续保留。

## 2026-10-23：固定循环原生 RSS，05b2e07

复用完整 release QA App、已有隔离长任务（3,089 段）和原始 1920×1080 H.264 视频，时长 12,172.744 秒；正式 App 继续供用户测试，不操作其窗口或数据。QA 二进制摘要与已记录的 05b2e07 完整包一致。循环固定为 76.740–80.310 秒，静音、1×、默认跟随，视频／波形展开；预热 60 秒后连续采样 600.012 秒，每 5 秒一次，共 121 样本。期间没有跳转、滚动、切面板、编辑或 CUA 观察，也没有运行构建／测试。

先保存启动前后进程差集，明确选择 QA 主进程及新出现的 GPU／WebContent／Networking PID（40702／40705／40706／40709），未把正式 App 或其他 WebKit 进程计入。命令、进程启动时间与存活时长前后核验，采样无 PID 丢失或身份变化；XPC 的 PPID 为 1，不能只按父子树采样。前后 AX 均确认同一句循环、静音、跟随和 3,089 段。

以下为 RSS，单位 MiB；每进程可能包含共享页，合计不是独占内存。前后两分钟中位数用于减少单点波动，不代表同机未测版本的对照。

| 进程 | 起点→终点 | 前 120 秒中位数 | 后 120 秒中位数 | 观测峰值 |
|---|---|---:|---:|---:|
| Main | 108.0→104.4 | 108.0 | 104.5 | 108.4 |
| GPU | 74.8→79.2 | 73.5 | 77.3 | 81.2 |
| WebContent | 100.6→160.7 | 114.5 | 147.8 | 186.2 |
| Networking | 410.1→410.2 | 421.7 | 281.7 | 447.6 |
| 合计 | 693.4→754.4 | 712.9 | 621.5 | 762.5 |

WebContent 前段抬升、后段回落；最后五分钟 OLS 斜率约 −2.61 MiB／min，不能据此证明不存在缓慢泄漏。Networking 大幅波动，单看起点／终点或合计会遗漏过程。固定循环后段没有合计 RSS 持续增长的证据，但这不是整个编辑器的无泄漏结论。

主采样结束后暂停 60 秒，合计 725.3→423.4 MiB；再离开编辑器观察 60 秒，472.0→470.9 MiB，Networking 最终约 111.5 MiB，WebContent 约 158.1 MiB。离页重新挂载任务卡片等也会改变占用，不能把两个阶段的起点差值直接解释为释放量。`vmmap -summary` 在主采样结束后取得：WebContent 暂停期 physical footprint 169.4 M、离页后 228.1 M，自启动峰值 653.9 M；此口径与 RSS 不同，峰值没有时间定位，也不是 JS heap 或 retained-object 分析。5 秒采样不能覆盖所有瞬态峰值。

QA 正常退出后，隔离数据库全部 20 张表内容摘要与测试前一致，integrity_check=ok。未发起识别／翻译请求、修改字幕或复制数据库／媒体。原始 warmup／steady／paused／left-editor JSONL、PID 身份、前后 AX、vmmap、二进制／媒体身份、summary.json、steady.csv、数据核验及可复算脚本在忽略目录 `local-artifacts/editor-steady-rss-05b2e07/`。

本轮补齐固定负载的十分钟采样证据；WebContent 初段抬升的 heap／原生归因、跨任务重复进入、连续播放／滚动／输入的长期趋势仍待验证。滚动 P95、原生偶发锁定与 Windows 门禁不变，不能把这个循环场景外推到所有使用方式。


## 2026-10-24：真实 wheel 输入的跟随控件热路径

旧脚本设置 scrollTop，未触发 wheel／touchmove 的 browse 回调。实际回调在列表已暂停自动滚动后仍执行 sync，重复写入跟随按钮属性／文字，并对 visible cues 执行当前编辑段查找。编辑段位于末尾时，这条查找随字幕数量增长。现已改为暂停状态只记录新浏览意图，首次从自动滚动转为手动时才 sync；显式 sync 的相同属性／文字也不重复写入。不跳过虚拟窗口绘制或改变编辑／播放段身份。

新增 `scripts/measure-editor-wheel.cjs` 使用同一台机器、1180×760、生产构建与 headless Playwright WebKit，合成 IPC／三小时时间范围。基线为 9d09ca4，前后 main.ts 与 editor-cue-list.ts SHA 相同，仅 editor-follow.ts 改变。各规模先用 End 锁定末段，再从列表中点开始，每轮 60 次 page.mouse.wheel、14 px／次、间隔 20 ms，三轮向下／向上／向下；前后实际滚动位置与选中段逐项一致，每轮恰有 60 个 isTrusted wheel，未出现 untrusted 事件，最大挂载行均为 23。两个测量串行执行，期间没有运行构建或其他测试；回归在测量结束后执行。

测量分别记录三个跟随控件的 MutationObserver、实际列表 wheel 监听器的同步 wall time、连续 RAF 帧间隔。监听器通过测试 harness 包装，生产代码不加测量 hook；它不含浏览器默认滚动／后续虚拟窗口绘制，不等同 CPU profile。performance.now 最小观测步长为 1 ms，记录为 0 表示该次未被时钟分辨，不代表零 CPU 成本；量化累计也有采样误差。

| 段数 | 每轮跟随控件更新，前→后 | 每轮监听器量化累计耗时中位数，前→后（ms） | 三轮帧间隔 P95，前→后（ms） |
|---|---:|---:|---|
| 300 | 120→0 | 10→0（未分辨） | 18/18/18→18/18/18 |
| 3,000 | 120→0 | 13→1 | 18/17/17→17/18/17 |
| 10,000 | 120→0 | 21→0（未分辨） | 18/17/18→18/17/18 |

所有样本均无 >50 ms 帧间隔。这证明本条 wheel 回调的冗余更新与查找已减少，帧间隔 P95 没有可证实改善；不能据此关闭原生滚动、长任务、持续内存或 Windows 门禁。原始逐事件时间／逐帧间隔、运行环境、源码摘要与可复算 summary 在忽略目录 `local-artifacts/editor-performance/2026-10-24-wheel/`；复现沿用既有浏览器路径和 ATOGAKI_EDITOR_URL，设置 ATOGAKI_MEASUREMENT_LABEL 后运行新脚本，拒绝覆盖原证据。

同次修复 browse 的新意图使挂起的 resume 失效：等待草稿 flush 时用户再次手动滚动，旧请求不能在返回后开启自动滚动，即使列表原先已经处于暂停状态。该竞态有独立复现，但与早先真实长任务的偶发锁定尚未建立因果关系，原锁定问题仍待追溯。dispose 后同步／浏览／恢复入口不再回调已释放上下文。

248 项前端测试、类型／生产构建、根目录和 Tauri fmt、diff 检查及十七组 WebKit 回归通过。新增 `verify-editor-browse.cjs` 验证暂停后重复 browse 不写 DOM／不调用筛选查询、sync 幂等、新滚动取消挂起恢复、显式重试和 dispose；现有编辑跟随、重定位、任务选择与设置等十六组同步通过。本轮没有原生重装、真实任务写入或 RSS／heap 新测量；正式 App 仍为 05b2e07。


## 2026-10-24：7eadbef release 原生滚动与 Instruments 有效性

从当前 7eadbef 源码构建完整四 sidecar release QA 包，identifier 为 com.chai-yinfeng.atogaki.editor-qa，核验签名及 27 文件摘要。覆盖同一路径的临时 QA App，复用 long-task-data 隔离副本，没有新增 App 恢复点、数据库或媒体副本，也未覆盖正式 App。打开 3,089 段《教父》，视频／波形展开、媒体停在 0，End 锁定 #3089；截图中的波形仍显示准备状态，本轮不作为解码／波形完成或播放负载验收。

整张 AX table 作为滚动目标曾返回 noWindowsAvailable，但同一窗口仍可读。改为从当前 AX 中选取可见行后滚动成功；没有重启 QA 掩盖故障。该现象与表格整体和虚拟行的定位有关，但尚未证明自动化错误的底层原因。首次 45.887 秒 Animation Hitches trace 没有覆盖任何成功滚动，明确排除出滚动样本；其后单独记录的三页滚动仅作功能准备。

第二次附加本次新启动的 WebContent PID 76833，QA 主进程为 76822，GPU／Networking 为 76832／76834；启动前后进程差集、命令与启动时间有记录，未选用正式 App 的同名 XPC。记录 46.674 秒，其中 36 次 CUA scroll 调用全部落在录制窗口内，输入首尾跨度 28.945 秒，24 次向上／12 次向下，每次 pages 参数为 1。该参数不等于固定像素量或原生 wheel 事件数。每次先读取当前 AX 再选行，观测 22–23 行；列表最终浏览到约 #3031，编辑框、原译文和时间仍锁在 #3089。AX 与截图确认了手动浏览不切换编辑对象。AX 观察与滚动交错，会引入额外工作，不能作为纯触控板延迟样本。

导出检查：hitches-renders 有 2,750 行，displayed-surfaces-interval 有 2,738 行，但缺少可直接归属到 QA 进程的列；目标 hitches-updates／visual-connection 为空，compositor-process 表仅 1 行，未建立可靠的进程／surface 映射。potential-hangs 与 hitches 表也为空，这些空表不证明应用无卡顿。初始 frame-lifetimes 的 duration 是 frame lifetime，不是相邻 presentation interval；不得用约 65 ms lifetime 计算滚动帧 P95。最终 presentation_interval_p95_ms 保持 null，没有得到原生 P95 或前后改善结论。

录制结束后的 Instruments 收尾曾产生约 21 GiB 临时 attachment，数分钟高 CPU；正常中断后保存的第二份 trace 约 1.8 GiB。该收尾发生在输入记录之后，不是应用运行时内存／CPU 指标。初始 trace 也在索引／导出后增大到约 1.1 GiB。所有导出进程及录制器终止后，逐文件记录路径、用途、版本、大小和 SHA-256，再仅清理本轮自建的两份原始 trace，逻辑内容合计 3,023,490,512 字节；保留约 4.1 MiB 的 TOC、导出表、动作时间、包／进程身份、校验清单及摘要。没有删除以前的基线 trace、备份、ASR 证据、模型或数据库，详见[保留清单](local-artifact-retention.md)。

新增只读工具 `scripts/summarize-native-scroll-evidence.py` 从已完成导出的 XML 与 actions.jsonl 对齐输入窗口、汇总表结构／行数，明确不计算未归属的 P95，拒绝覆盖已有输出。实际导出重算摘要一致，重复输出拒绝且原文件不变，Python 语法检查通过。私有证据目录为 `local-artifacts/editor-native-scroll-7eadbef/`；原始容器已按上述清单清理，保留导出不要求重新访问真实媒体。

QA 正常退出后 20 表行数及排序内容摘要全部一致，integrity_check=ok、foreign_key_check 为空，原视频 size／mtime_ns／inode 不变（未计算整片内容 hash）。本轮没有识别／翻译／字幕写入或导出任务。业务源码未改，原生打包运行类型／生产构建，两套 fmt／diff 检查通过；未重复跑 248 项前端测试／十七组 WebKit 或 Rust 全量测试，其结果仍对应 7eadbef。原生滚动 P95、长时内存、播放中交互、原偶发锁定和 Windows 门禁保持；下一次应先验证宿主窗口的帧归属与采集成本，避免再对空映射重复录制。

## 2026-10-26：结构撤销收据预算

新增 `measure_structure_undo` example，只使用自动清理的合成临时 SQLite；每段双语文本、四个 timed-unit ID、一个 ASR run 和线性 frozen-context 边，连续执行十次实际 recorded 时间修改。先构建，再对同一可执行文件分别运行 unbounded／bounded 模式；`/usr/bin/time -l` 只统计该子进程，编译器不在 RSS 范围内。单位为 MiB，核算容量包括收据 Vec／String 容量、标签／条目和队列备用槽位；不含索引存储、分配器、SQLite 或写入／撤销中的临时状态，因此不是整个进程的硬内存上限。

| 段数 | 无界十份收据容量 | 96 MiB 预算下保留条数／容量 | 无界／有界峰值 RSS |
|---|---:|---:|---:|
| 300 | 7.3 | 10／7.3 | 22.0／22.0 |
| 3,000 | 65.3 | 10／65.3 | 99.6／99.6 |
| 10,000 | 237.6 | 4／95.1 | 310.7／189.0 |

默认预算选 96 MiB，覆盖本样本 3,000 段十条历史，并限制更长字幕的保留深度；每任务仍最多十条，跨任务共用预算。真实文字及来源 JSON 长度不同会改变可留条数，不保证所有 10,000 段任务都是四条。已提交的超大单项会显式返回无 token，不能伪装为仍可撤销；本轮只实现管理器返回值，UI 呈现待后续接入。

有界三组清空后的管理器统计均为零条／零任务／零核算字节；七项测试覆盖全局最老队首淘汰、连续撤销重绑定、外部修改重置旧链、超大项只释放本任务、失败重试、错误 token、任务及会话释放。索引按现有队首维护，没有无限增长的失效 token 队列。清空计数与 Rust 所有权证明收据释放，不代表进程 RSS 立即下降，也不能替代原生多操作长期内存／泄漏验收。

初次沙箱运行子程序完成，但 time 读取 kern.clockrate 被拒而未取得完整 RSS；保留 300-sandbox 日志，按权限流程重跑后完成三组双模式测量。原始 JSON、time 资源统计和源码／二进制 checksum 位于 `local-artifacts/editor-undo-budget-20261026/`。全部为 debug 合成基准，数值不用于证明原生滚动 P95、首次交互、媒体解码或 Windows 改善；正式 App 与 QA 未变。


## 2026-10-27：混合负载发现并修复波形边界重复请求

新增 measure-editor-mixed-load.cjs，以生产 bundle／Vite preview（无 HMR）、Playwright WebKit 26.5、1180×760 和合成 IPC 运行。两任务各 10,000 段，声明三小时时间轴，实际音频为 20 秒静音；轮流打开、播放、8 次 trusted wheel、搜索后部字幕、选中／保存文字、识别工具开关、翻译工具 Escape、恢复跟随并离页。fixture 的调用调试数组每次消费清空、统计按命令聚合，避免测量自身无限保留调用日志；两份 cue 数组仍存在渲染进程，不等于实际 SQLite 后端的内存结构。

短时预跑第一轮约三秒 get_waveform_window 为 183 次，四轮累计 730 次。原因是素材边界无法再居中，播放头每帧仍命中边缘判断并重复读取同窗，见[缺陷记录](editor-waveform-boundary-reload-bug.md)。waveform-follow 先比较限制后的新起点与当前起点，不动则继续绘制；显式 resize／缩放／平移读取仍保留。修复后同样前四轮累计 2／4／6／8 次。

修复后 181.409 秒完成 62 轮，全部通过，无 pageerror：波形 124 次，详情／structure state／文字保存／上下文状态各 62 次。详情数等于显式打开数，没有周期性完整详情读取；本场景反复离页，仍有轻量摘要读取，未将其列为异常。编辑区始终 14 行、文档 710 个元素，离页始终零字幕行／零媒体节点、626 个元素，工具关闭后节点停放。节点数量不随轮数增长，但不证明脱离文档的对象或监听不存在泄漏。

进程采样通过本 Node 子进程树捕获 Playwright 宿主及启动 shell，宿主树 RSS 范围约 116.4–127.7 MiB；macOS XPC WebContent 不在该树内，故不能作为渲染内存或总内存趋势，也不能将其与之前原生 RSS 对照。此三分钟测试补充操作与 DOM／请求边界证据，不关闭长期内存、真实三小时媒体解码、原生 P95 或 Windows 门禁。四组专项／既有 WebKit（waveform-follow-boundaries、follow、timeline、gestures）通过，前端 346 项、类型／生产构建、两套 fmt／diff、两脚本语法检查通过；未更新原生 App。

复现设置既有 Playwright module／browser 路径、ATOGAKI_EDITOR_URL 指向生产 preview，ATOGAKI_EDITOR_QA_OUTPUT 为新目录，ATOGAKI_SOAK_SECONDS=180、ATOGAKI_CUE_COUNT=10000 后运行 node scripts/measure-editor-mixed-load.cjs。默认 600 秒；已有 samples.jsonl 或 summary.json 会拒绝覆盖。预跑、逐轮 JSONL、聚合计数、进程身份、日志、构建与来源 checksum 在 local-artifacts/editor-mixed-load-20261027/。


## 2026-10-27：原生 WebContent 短时归属与 PID 身份补强

改进 sample-editor-rss.py：原脚本只比较 PID／command，相同程序路径重启并复用 PID 可能被误记为内存降低；现同时保存／比较 ps lstart，变化就记录 reused_pids 并退出。秒级启动时间仍有精度限制，不等于内核级不可复用身份；旧证据不补造该字段。抽出解析与身份判断以做故障回归，不改变 RSS 口径或覆盖保护。两项 Python 回归覆盖同路径 PID 重启／普通 RSS 变化、带空格路径和日期空格。首次因脚本文件名含连字符导致测试 import 失败，改用显式文件 import 后通过；原失败日志保留。

使用现有独立 QA c8afa5f 与已有十段合成目录。启动前 ps 没有 WebKit XPC，启动后新增 QA 16014、GPU 16016、WebContent 16017、Networking 16018；记录全部命令与启动时间。launchctl procinfo 需要 root，未取得直接 responsible PID，未提权或修改系统保护。footprint 仅针对 QA／新 WebContent 成功读取分类，没有提供直接归属字段。因此本轮归属依据为受控启动、唯一新增组及退出对照；若日后已有其他 WebKit 进程，不能直接复用这些 PID 或只凭名称认领。

每 3 秒采样，90.01 秒共 31 样本；期间通过 CUA 执行十轮打开任务→识别工具→历史→候选对照→Escape→Escape→返回任务，共 70 个有界面证据的动作。没有播放、编辑、采用或模型请求，十段任务不是多规模压力负载；每步 AX 读取也是负载的一部分。进程命令／启动时间全程一致。RSS 首→尾：主进程 108.6→116.7 MiB、GPU 45.1→73.8、WebContent 70.0→95.1、Networking 9.3→18.1。footprint 的不同口径显示 WebContent 约 74→95 MB，WebKit Malloc dirty 约 22→46 MB；分类不足以区分首次缓存、可回收对象和泄漏，不将短时上升判定为泄漏或稳定。

正常退出 QA 后原始句柄退出 0，ps 中该 QA 与全部 WebKit XPC 均消失，关联证据闭合；此前五张任务／字幕／来源／依赖／采用表逐字段未变，revision 56、integrity_check=ok。未更新 App。长期混合操作、持续播放／输入／滚动、跨任务、heap 保留对象和 P95 门禁仍未通过；下一步使用该启动身份检查，先稳定预热，再固定可复现的更长原生混合负载，并把采集成本单列。

源码／Python 日志、31 个 JSONL 样本、70 次原生界面状态、前后进程身份、footprint JSON／分类、五表 snapshot 与 summary 在 local-artifacts/editor-process-attribution-20261027。不得将本轮结果与未同负载重测的历史版本作性能改善对照。


## 2026-10-28：宿主进程帧映射的短时诊断

此前 7eadbef 附加 WebContent 得不到 hitches-updates。此次核验现有独立 QA e8d9a8d 的 27 文件摘要与合成目录，启动唯一 QA 主进程 34383，播放十段合成音频后改为附加宿主 PID，Animation Hitches 请求录制 5 秒。TOC 确认目标 PID，实际记录 6.971519 秒；主进程更新表含 394 行，全部明确归属该 PID，surface 为 283／95。58 条 display-surface-swap 中有 56 条的 surface-id 与 swap-id 二元组精确匹配该主进程更新，另有 57 条 displayed-surfaces-interval。宿主附加因此补出了此前缺失的归属桥接。

但更新表末尾为 7.004665416 秒，swap／displayed 记录分别只到 4.120848208／4.104181708 秒，不能把不完整显示流的相邻导出时间当成完整帧间隔。presentation_interval_p95_ms 继续为 null；本次也没有滚动负载或前后对照，不作为编辑器帧率／滚动改善证据。

录制达到时间限制后收尾仍膨胀，观察到临时约 6.2 GiB；向已核对的本次 recorder PID 发 SIGINT 后成功保存并退出 0，整体墙钟 57.407 秒。最终 trace 逻辑大小 120,584,552 字节，481 文件逐一 SHA-256 保留；未删除旧证据或创建 App／数据库备份。QA 正常 Cmd+Q、原启动句柄退出 0，正式 App 未操作。

目录 local-artifacts/editor-native-frame-map-20261028/ 保留包／启动身份、确切命令、recorder 结果、TOC、四张导出表、mapping-summary 与完整 trace 清单。本次确认下一次应附加宿主，而非重复只附加 WebContent；在显示流缺失和收尾成本得到控制前，不扩大 Animation Hitches 长时录制。原生滚动门禁仍未通过。


## 2026-10-28：混合负载 WebKit XPC 关联采样准备

measure-editor-mixed-load 新增可选 ATOGAKI_CAPTURE_WEBKIT_XPC=1。在浏览器启动前后读取 PID／comm／lstart，要求差集中恰好各一个 WebContent、GPU、Networking，且均位于该 Playwright WebKit runtime 的真实目录；允许此 runtime 实际使用的 .Development 可执行名。每轮对准确 PID 再核对命令和启动时间，缺失／重用立即失败。browser.close 后十秒内确认三个关联进程均退出，独立 xpc-exit.json 才闭合启动关联。这是受控启动差集、稳定身份和共同退出的证据，不能冒称 OS responsibility／父子链证明；宿主树 RSS 与 XPC RSS 分列，均非 retained JavaScript heap 或 Tauri 原生 App 内存。

两项辅助工具回归覆盖预存进程排除、带空格命令解析、同名 PID 重用、缺失进程拒绝和正常 RSS 更新。首次十秒预检准确捕获三进程，但分类只接受无 .Development 名，主动拒绝并正常关闭；修正分类及 runtime 范围后，第二次生产 bundle 预检实际 12.103 秒／四轮双万段混合操作通过，三进程共同退出确认。证据分别在 local-artifacts/editor-mixed-xpc-preflight-20261028/ 与 editor-mixed-xpc-preflight2-20261028/。使用生产 preview，未将 Vite dev 当成生产测量；前端构建、脚本语法、两套 fmt／diff 通过。

随后已启动 600 秒双万段混合负载，输出到 local-artifacts/editor-mixed-xpc-soak-20261028/；本段只交付采集工具与短预检，长时结果需在原进程完成并检查 xpc-exit 后单独记录。输入包括真实浏览器播放／滚轮、搜索、文字保存、两个任务往返、识别和翻译工具开关；后端为合成 IPC、音频只有二十秒，声明的三小时时间轴不等于三小时媒体解码。没有正式数据、模型／云端请求或 App 安装变更。


## 2026-10-28：十分钟双万段混合负载的 XPC RSS 结果

沿用同一个采样进程完成 600.886 秒／207 轮，生产 bundle、两个各 10,000 cue 的合成任务交替进入；每轮真实浏览器播放／滚轮、搜索后段、文字保存、恢复跟随、识别与翻译工具往返并离页。全程无 pageerror，活动列表始终 14 行／710 元素、一个媒体、零已挂载工具；离页零列表行／零编辑媒体、626 元素。详情读取 207 次、文字写入 207 次，均等于显式轮数；波形请求 414 次，没有恢复逐帧请求风暴。三类 runtime 内 XPC 的 PID／命令／lstart 全程一致，browser.close 后共同退出、原始执行句柄返回 0，启动关联闭合。

WebContent RSS 分钟中位数（第 1–10 分钟，MiB）为 296.92、316.17、334.33、320.72、353.66、361.89、374.77、387.12、400.14、413.60；离页采样全程范围 234.14–420.22 MiB。GPU 为 103.66–121.64 MiB、中位 115.84；Networking 为 33.61–45.66 MiB、中位 35.16。宿主树与 XPC 分列，未将 XPC 遗漏后宣称总内存稳定。

结论：这一固定负载的节点、显式请求边界通过，但 WebContent 后半段 RSS 仍抬升，因此“持续内存无增长”门禁不能通过。RSS 无法区分存活 JS 对象、原生媒体／图形缓存、引擎分配高水位等原因，也不能仅凭末段增长断言泄漏。下一步应在相同 workload 加入停止操作后的空闲／回收观察和保留对象归因；不更换素材或只看关闭进程后释放来冒充运行中稳定。

证据 local-artifacts/editor-mixed-xpc-soak-20261028/ 包括 207 条逐轮样本、启动／共同退出身份、源码 hash、原始 summary、逐分钟 analysis.json、仅在完成且退出证据通过后可运行的 analyze.py 及文件 SHA manifest。该分析脚本检查实际 600 秒、样本轮数、每次身份、列表／工具／媒体边界；每轮采样在离页后，不代表操作内分配峰值。源码为 877ef70 前端、采集工具为 40ecc8d；生产构建通过，长期运行没有源码变动。正式 App／QA 包未更新，未写真实字幕、调用云端或模型；合成二十秒音频／三小时声明时间轴不等于三小时解码，也不是原生 Tauri 或前后 P95 改善证明。


## 2026-10-28：WeakRef 与空闲／显式 GC 诊断预检

本机 Playwright 1.62.1 的公开 page.requestGC 在 WebKit 后端实际调用 Heap.gc；短负载实测完成，没有使用私有协议或向生产模块加入测试接口。混合脚本新增可选 ATOGAKI_COOLDOWN_SECONDS，合成 fixture 仅用 WeakRef 跟踪各轮返回的字幕数组、首个虚拟行与媒体元素；负载结束、空闲后、请求 GC 一秒后分别读取存活索引及进程 RSS。WeakRef 本身不持有目标，但观测会增加每轮一次 evaluate，不能与原负载当成零成本精确对照。

30.223 秒／10 轮预检加五秒空闲通过：负载结束时十份字幕数组、十个媒体包装均已回收，仅最后一个行包装存活；五秒后相同，请求 GC 后三类全部零存活。WebContent RSS 约 330.52→306.59→306.61 MiB；GPU 120.78→116.72→116.72，Networking 37.97→37.59→37.59。三类 XPC 身份稳定、共同退出确认。此短预检只证明诊断方法可用和这些已知对象在短样本可回收，不能解释上一轮十分钟增长或证明不存在其他保留对象。

证据 local-artifacts/editor-mixed-retention-preflight-20261028/ 的 retention／summary／XPC 身份。首次语法检查因工作目录为 ui 而相对 scripts 路径不存在，返回根目录后语法检查通过；preview 服务使用原生产 bundle，没有开发包替代。工具另将源码／生产 bundle SHA 在启动前冻结，负载时长与含空闲总时长分开记录，避免末尾读取修改后的源码导致证据错误。两项归属辅助测试、语法、两套 fmt／diff 通过。

后继已启动 600 秒原工作流加 120 秒空闲及公开 GC，输出 local-artifacts/editor-mixed-retention-soak-20261028/；完成、检查存活及共同退出前不作长期结论。本次未改生产代码、重新打包或替换正式 App。


## 2026-10-28：十分钟 WeakRef 复测与两分钟空闲结果

同一进程完成 602.796 秒／202 轮操作及 120.049 秒空闲，总计 724.021 秒；后继调用公开 page.requestGC 后观察一秒，正常关闭浏览器，三个关联 XPC 身份全程一致并共同退出。启动时七个源码摘要和九个生产 bundle 文件摘要在结束后仍全部匹配，没有重启测试、换包或中途修改生产逻辑。原始句柄退出 0。

活动列表仍为 14 行／710 元素、一个媒体、无挂载工具，离页零行／零媒体、626 元素；详情／文字保存各 202 次，波形 404 次，无页面错误。对 202 轮分别跟踪的字幕数组、首个行和媒体包装，负载结束时仅第 202 轮数组与媒体存活，行全部回收；120 秒空闲后相同；公开 GC 后三个集合的 202 个目标均零存活。因此没有发现这些被跟踪对象跨轮累计保留，不能把这轮 RSS 直接归因到旧整份字幕数组或旧播放器包装泄漏。

WebContent RSS 分钟中位数（MiB）为 347.09、343.77、344.50、356.78、359.06、365.56、353.27、284.55、278.20、353.27。负载结束／空闲后／GC 后分别为 361.83／157.28／215.73 MiB；对应 GPU 为 87.28／60.86／60.88，Networking 为 22.83／18.66／18.66。GC 后包装对象减少而 RSS 反升，说明 RSS 与可达 JS 对象不能简单等同；本记录不猜测换页、缓存或某个引擎分配机制就是原因。此前无 WeakRef 的十分钟样本后半段上升仍保留，两个独立运行不能拼接为严格前后优化对照。

结论：已跟踪的整份数组／行／媒体包装释放有长样本证据，且这轮运行中出现自然 RSS 回落；不据此宣布所有 retained heap、原生 Tauri 长期内存或性能门禁完成，也不往产品加入强制 GC。后续优先匹配原生实际负载、区分 RSS／物理占用与未观测保留对象，避免继续凭 RSS 修改已证明可回收的对象边界。

证据 local-artifacts/editor-mixed-retention-soak-20261028/ 包含逐轮样本、三阶段 retention.json、负载与空闲分开的 summary、源码／bundle 校验、启动／共同退出、仅对完整结果运行的 analyze.py／analysis.json 及逐文件 SHA manifest。分析检查 600 秒、120 秒、准确身份、总轮数与每个跟踪索引范围。WeakRef 为测试观测且每轮增加一次 evaluate，公开 GC 不保证收尽所有不可达对象；三类零存活只证明本集合，不能替代所有堆对象或原生资源分析。正式 App／QA 未改动；此期间用户另确认了 e8d9a8d 面板／跟随交互，目前正常，见窗口验收。

## 2026-10-28：原生三档十分钟混合负载与空闲回落

核验现有隔离 QA 9fc1efe 的 27 文件摘要，复用真实三小时 16 kHz 单声道稀疏静音与 300／3,000／10,000 段合成 SQLite。通过原生 AX 操作完成 616.430 秒／53 轮任务交替：搜索后段、锁段、时间码播放、恢复跟随、清搜索、列表滚动、翻译词表工具往返及离页；每五轮编辑并保存译文，共十次写入。时长含自动化观察与批次停顿，不是连续播放或统一吞吐基准，不能把操作墙钟当成 TTI。原生初始 AX 每轮均 14 行，滚动后 16–18 行；未在原生注入 JS 计数，DOM 有界继续依赖先前浏览器证据。

每两秒采样准确宿主及启动差分唯一关联的 WebContent、GPU、Networking，核对 PID／路径／启动时间，376 个样本。以下是负载期完整前十分钟中位 RSS（MiB），第十一分钟只有 16 秒，原始分析单列：

- 宿主：141.19、148.75、149.70、153.88、149.94、145.69、148.70、150.22、153.84、144.98。
- WebContent：144.48、191.67、202.91、223.78、223.03、220.82、225.66、232.03、233.48、221.75。
- GPU：106.58、110.33、111.15、111.80、110.28、107.69、108.58、107.67、109.98、116.23。
- Networking：134.86、324.30、377.63、349.75、350.38、398.82、360.95、351.78、345.59、285.61。

负载结束／126.930 秒任务页空闲后，宿主 145.53→130.92 MiB、WebContent 195.08→151.27、GPU 115.25→77.20、Networking 333.98→91.03。Networking 负载采样最高 510.25 MiB、WebContent 257.78 MiB，应保留这些暂态，不只报告末值。未主动 GC。原生任务页空闲后仍可响应，正常退出 0，关联四 PID 均消失，采样进程退出 0。进程关联来自受控启动差分／稳定身份／共同退出，仍非 OS ownership API 证明。

本轮后半段 RSS 波动并自然回落，没有发现这一 53 轮原生负载中的逐轮持续增长；不能从中归因媒体缓存、引擎分配或某类泄漏，也不能否定此前更快的双万段浏览器负载增长。三小时静音没有真实视频解码压力，未测所有 retained heap 或物理占用；原生同素材前后滚动 P95、TTI 与长任务保持未测。

实际数据库逐段核对 13,300 段原文／时间／ASR run 与 timed unit 对应，三段译文分别成功保存 3／4／3 次，translation revision 与十次写入一致，其余译文未变。20 表中仅字幕、来源 revision、任务 revision 三表预期变化，17 表内容摘要一致；空闲及退出期间没有新增变化，integrity=ok、foreign key 空。只读核对曾误用不存在的表名，查询失败后读取 sqlite_master 改正，没有写数据。

证据 `local-artifacts/editor-native-mixed-20261028/` 保留包清单、启动前进程、原始样本／身份／操作／阶段时间、数据库前后摘要、逐字段验证脚本和结果、分析脚本／analysis.json、SHA manifest；README 记录操作步骤与测量边界。正式 App、正式数据库、公开发行及模型／云端均未涉及。

## 2026-10-28：三小时音轨请求基线与可选传输量诊断

沿用已核验 QA 9fc1efe，以 `ATOGAKI_MEDIA_DIAGNOSTICS=1` 打开万段三小时稀疏 WAV；保持暂停，仅加载 metadata 时原生日志出现 `bytes=0-1`、`bytes=0-345600043`、尾部 `bytes=345571328-345600043`，后段定位播放再出现中间和尾部范围。正常退出 0，证据 `local-artifacts/editor-native-media-requests-20261028/app.log` 与 metadata／tail AX。这仅证明 WebKit 请求了覆盖全文件的范围，原有 opened bytes 是文件长度，不能证明完成传输或解释 Networking RSS。

补强现有默认关闭的媒体诊断：本次访问生成无路径／token 的请求编号，流结束或释放时记录 expected、file_read、body_yielded、outcome。file_read 是已完成 Rust 文件读取返回的字节，不是底层磁盘 IO／取消中的后台读；body_yielded 是交给 HTTP body 的字节，不是客户端确认接收量或缓存占用。outcome=complete 表示读到流结束，error 表示读取失败，dropped 只表示未观察到结束；最后一块已经交出但消费者未再 poll 也可能显示 dropped，不能一律解释为取消。头部和响应语义、64 KiB 流式读取边界不变，不按猜测截断 Range 或更改缓存策略。

专项验证未 poll／只消费 multipart 前缀／一块文件／完整结束／EOF 错误时的计数，原有 HTTP Range、HEAD、多段、访问边界、9.2 GB 稀疏文件和断开测试保持通过。媒体专项 4 项、完整 Tauri 36 项通过／4 项显式忽略，两套 fmt／diff 通过。可选日志尚未用于新版原生包，传输与暂态内存结论待后继实测；正式 App 不变。

## 2026-10-28：按需音频策略的实现与浏览器回归

计数版 QA 1d63545 正常退出 0，四个请求均有释放报告：暂停 metadata 阶段依次读取／交出 2、124,059,648、28,716 bytes；其中全范围请求预期 345,600,044 bytes，未完整交出。后段定位另读／交出 393,216 bytes。所有报告为 dropped，包括长度完整交出的短范围，印证不能把此字段直接当作取消。前后 20 表摘要一致。包签名、27 文件摘要、AX、阶段记录与 verification-counted 在原媒体请求证据目录。

用户确认按需加载取舍后，依 [0062](decisions/0062-editor-audio-demand-loading.md) 为编辑页音频设置 preload=none，首次应用定位显式启动 metadata，原生播放继续直接触发资源选择；视频保留首帧，收听策略不变。positionMs 复用原播放器意图边界，未加载／排队定位不被 currentTime=0 覆盖；插入可以使用波形已返回的实际总时长。没有截断 HTTP Range、替换媒体格式或增加另一播放器。

349 项前端测试、类型和生产构建通过；新增三项播放器测试覆盖未加载恢复／退出、暂停 seek 单次 load、播放中不打断及回退／旧事件。真实 HTTP WebKit 专项证明打开／选段／同任务重开无播放器请求，首次暂停定位加载但保持暂停、后段播放与恢复位置正确。既有直接设置 native currentTime 的测试现在显式准备 metadata，不作为按需证明；原媒体回归因重开后直接等 currentTime=18 超时，按新合同先准备加载后通过，失败日志保留。另复测紧凑布局、控件、跟随、手势、列表、修复、结构时间、跨任务恢复、时间轴、UI、波形跟随、播放器、循环及波形边界共十四组通过。首次本地服务启动受 sandbox EPERM 阻止，经授权启动后运行；一次工具 cwd 为 ui 导致根目录脚本路径不存在，改回准确目录。

证据 `local-artifacts/editor-audio-demand-20261028/` 保留前端／构建、专项和十四组日志，未把数据 URL 原生行为当作 HTTP 不读取的证明。真实三小时原生新版包尚待集成验证，正式 App 未更新。
