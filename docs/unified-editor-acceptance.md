# 统一字幕编辑器验收清单

更新：2026-10-30；全量审计点 8ab7c59；后继波形跟随交互修复 e8d9a8d 已补六组 WebKit／原生跨窗验证，并同步本机与独立 QA。原全量审计未覆盖的新范围以缺陷记录为准。

本页把用户四阶段计划与后继交互确认对应到证据，避免从长验收日志的历史计数推断整体完成。状态“有证据”只覆盖所列环境／用例；“部分”表示实现已存在但规定的验收范围未闭合；“待验证”不表示产品一定有故障。整体仍未验收完成。

当前交互以 [0058](decisions/0058-compact-editor-and-playback-follow.md) 为准，取代原计划的上方右侧编辑／中间波形，以及播放不切换编辑对象：左侧大视频，右侧上方波形、下方原译文上下排列，底部整宽列表；默认跟随、单击／聚焦后锁段、显式跟随恢复。不是完整复刻 Aegisub 的产品范围。

2026-10-30 收尾调整：用户明确排除本轮 Windows 场景，并要求减少保护性编程与过度测试。以下未验证项继续如实保留，但不自动派生更多压力／故障组合或预防性代码；后续围绕日常可复现问题做必要回归。已通过的核心编辑与人工试用项不反复重验。

## 一、面板与编辑流程

| 要求 | 实现／证据 | 审计状态与剩余范围 |
|---|---|---|
| 四区、工作区约束在窗口内、面板滚动、紧凑间距 | editor-template／editor.css／editor-layout；[窗口记录](editor-workspace-validation.md)与 0cf72c0 本机截图 | 有浏览器和默认原生窗口证据；2026-10-28 用户确认本机 e8d9a8d 紧凑布局／上下原译文“目前正常” |
| 三处分隔线、鼠标／键盘、比例记忆／重置、最小尺寸 | editor-layout；verify-editor-ui／compact；1180×760、960×640、大字号 | 部分：浏览器覆盖；原生已补三条分隔线键盘调整、折叠／比例重启记忆及恢复默认；原生 drag 接口仍 noWindowsAvailable，鼠标拖动及小窗口／大字号完整原生范围待验 |
| 视频／波形独立折叠，音频与编辑选择保留 | media-player／editor-layout；窗口与 timeline 回归 | 有证据；Windows 另验 |
| 完整识别（含无译文）直达编辑，处理中／部分结果只读 | task-navigation／editor-navigation／task-progress；对应 navigation／progress 回归 | 有合成状态与原生任务打开证据；不能把一个真实 done 任务外推全部状态 |
| 翻译、词表、识别、候选、导出按需挂载，返回原位置 | task-tools、处理工具；processing／review-tools 回归；c8afa5f／35181fb 原生往返 | 有证据；词表修正弹窗另经 9fc1efe 原生输入／按钮／Tab／取消重开验证；系统文件面板／成品覆盖专项仍保留 |
| 固定摘要行、只为当前段创建完整原译输入 | editor-cue-list／current-cue-editor；万段混合负载最大行数受视口限制 | 有浏览器证据，实际完整文本留在编辑框 |
| 搜索、未翻译／过期／待复核筛选与命中数、筛选后导航 | editor-session／editor-controls；ui／follow／context-status 回归 | 有证据；隐藏当前段时保留输入并提示 |
| 单击锁定、双击／时间码定位、播放与编辑独立标记 | editor-session／editor-follow；follow 回归及本机定位 | 有证据；同轨空白处不沿用旧字幕 |
| 默认跟随、恢复播放头、手动浏览暂停自动滚动 | editor-follow／editor-browse；万段后段与 wheel 回归 | 有证据；窗口验证覆盖手动锁段和恢复，原生 P95 不由此通过 |
| 650 ms 自动保存、IME、切段草稿、失败重试 | SubtitleDrafts／current-cue-editor；ui／follow／time-navigation | 部分：异步输入／IME 为浏览器回归；真实 SQLite ABORT、ENOSPC、正常退出恢复已验；e8d9a8d 原生 ABORT 后 SIGKILL／同库草稿恢复及显式重试已验，立即终止／满卷组合／其他冲突仍待验 |
| 结构操作先 flush，失败阻止且保留输入 | editor-structure-actions／editor-block-actions；structure-timing／insert | 有合成竞态、故障及部分原生结构操作证据；真实全部故障组合未穷尽 |
| 文字撤回与打轴撤销分别保留；插入／删除／Cut／Join | 唯一 token 历史；[0061](decisions/0061-transactional-structure-undo-metadata.md)及 SQLite／原生核对 | 有主要路径证据，包含来源／依赖恢复、单调 revision、重开／重启 token、冲突／回滚；学习沿用既有快照规则 |
| 候选采用冲突、完整来源保护及导出一致性 | recorded 事务、adoption／scope 测试、原生采用／撤销、既有导出验证 | 部分：采用主路径已验；1f61307 QA 直接选初始目录已验原文 SRT 单文件取消／确认覆盖与十段逐字段一致；778d936 QA 补译文／双语 SRT／ASS 三文件首次导出、十段内容一致与批量取消；后继显式父窗口候选在 Raise 后补过三文件确认覆盖及 SHA 一致；粘贴输出路径的原生烧录取消／确认覆盖与重复确认已补；目录／保存面板偶发禁用、运行中覆盖取消和崩溃窗口仍待验 |
| 候选说明可读，长差异／错误可滚动，焦点边界 | candidate-comparison-view；candidate-dialog-layout；c8afa5f 原生截图／End／Tab | 有小窗口／20 px 浏览器与默认尺寸原生证据；未冒充原生小窗口／大字号全覆盖 |

多选、批量平移、频谱、逐帧和复杂 ASS 按原确认留待后续，不作为缺失实现补入本轮。

## 二、性能与接口

| 要求 | 已有证据 | 未闭合部分 |
|---|---|---|
| 改造前基线、同机约 300／3,000／10,000 段 trace | [性能记录](editor-performance-validation.md)保留多规模前后 WebKit 数据与来源 checksum | 已新增同一生产 WebKit 的六案例／18 事件窗口对照；合成 IPC／20 秒音频，不代替原生三小时数据 |
| 首次打开、滚动、输入、搜索、跳转和三小时后段 | WebKit 多规模；原生三小时静音／万段后段保存与定位 | 部分：原生同素材全指标对照、TTI／长任务／解码暂态未闭合 |
| 虚拟列表、搜索复用、按 cue ID 局部保存、隐藏旧列表不占 DOM | cue-list／current-cue-editor／task-tools；保存节点保留、万段 DOM、三分钟 62 轮生产 WebKit | 有对应功能／DOM 证据，不能据此证明整体无泄漏 |
| 时间索引、波形静态层、独立播放头、resize 合并／旧请求保护 | subtitle-timeline／editor-timeline-view／editor-waveform；gestures／timeline／waveform 边界回归 | 有证据；修复素材首尾逐帧同窗请求，未在原生量化 IPC |
| 完成任务编辑时不周期读取全详情／原始证据，摘要轻量 | task summary 接口、task-progress；62 轮显式打开 62 次详情 | 有对应调用边界证据；恢复／采用仍保留完整校验 |
| selectedCueId／playbackCueId／processingRange／timelineViewport 分离 | editor-session；请求任务身份／代次；后端 revision 约束 | 有源码和多组迟到返回／冲突回归；不把某一组当成所有异步路径全覆盖 |
| 布局偏好独立版本化，不写字幕产物 | editor-layout v3 本机偏好；跨任务／折叠恢复回归 | 有证据 |
| 同机原生滚动改善、帧耗时 P95、长任务 | [生产 WebKit 对照](editor-version-comparison-2026-10-29.md)有大列表事件窗口改善；原生已获短时关联与操作覆盖 | 旧坐标／AX 滚动受限，后继三档键盘预检已有真实位移，3,000 段两版同步短按已落入 trace，但当前版缺少宿主到显示 surface 的桥接，匹配 P95 仍不可用；浏览器 rAF 不作原生呈现／严格 TTI，Long Tasks API unsupported；原生 Inspector 已有页面 trace 和单次 rAF 预检，缺 Rendering Frame／时钟对应与历史重复，不能充当渲染 P95；采集成本仍高 |
| 持续内存无增长、WebContent 初段抬升归因 | 十分钟固定循环 RSS；三分钟混合 DOM；90 秒原生工具；新增十分钟／207 轮双万段生产 WebKit XPC 启动／身份／共同退出 | 部分证据，整体仍未通过：原样本 RSS 约 297→414 MiB；后继 202 轮加两分钟空闲的跟踪数组／行／媒体均可回收，RSS 自然回落后 GC 反升，不能等同 retained heap；新增原生 616 秒／53 轮三档混合加 127 秒空闲，WebContent 后半段中位约 221–233 MiB、空闲后 151 MiB，十次实际写入正确；该负载未见逐轮持续增长，全部保留对象及真实视频负载未归因 |

## 三、模块与历史设计

| 要求 | 当前证据 | 范围 |
|---|---|---|
| 会话、播放器、列表、编辑框、时间轴、处理工具逐步离开 main | [架构责任说明](architecture.md)及相应 src 模块，唯一草稿／播放器保持 | 主要编辑边界已交付；剩余入口协调和其他区域尚在 main，不宣称巨型入口已全部消失 |
| 明确 mount／dispose，释放 timer／监听／observer／请求返回／媒体 | 视图生命周期、启动／常驻资源、导航及对照的独立回归 | 部分：大量资源边界已测，仍需检查剩余全局／非编辑区域；非取消式 IPC 以过期返回隔离，不谎称已中止后端 |
| 不再追加旧粗修 DOM，渐进替换 karaoke 命名 | task-tools 停放唯一工具、虚拟列表；新模块 editor 命名 | 部分：兼容 DOM ID／函数名仍有 karaoke，不做全局机械重命名 |
| Rust 命令／repository 单独提交且不重写事务 | [0059](decisions/0059-desktop-command-and-repository-modules.md)：26 命令、编辑／发布／查询／撤销／审查／run／翻译回填 | 已交付这些边界；根 repository 的学习／词表／任务等不以本次抽取冒充全部拆完 |
| 确认 unused 后开启检查 | ui/tsconfig 的两项 unused 已开启；当前显式检查通过 | 有证据；本轮未重复添加开关 |
| 产品方向／架构／能力映射和取代旧导航的 ADR | 产品方向、架构、Aegisub 映射、0055–0061、桌面核心清单 | 已交付；历史现场保留原样，不把旧清单重当默认 |

## 四、文档、产物和交付约束

| 要求 | 证据／结果 | 审计状态 |
|---|---|---|
| 当前规范／提案／证据／历史分开，路线图保留当前与下一步 | [文档索引](README.md)、两次 roadmap 历史快照、本页要求映射 | 有证据；窗口长日志为历史证据，不再复制为当前完成度 |
| Web Serve／Postgres 调用／兼容性／依赖报告，先不退役 | [退役影响报告](legacy-web-retirement-impact.md) | 报告已交付；移除仍待独立讨论，不擅自删除 |
| 路径、用途、版本、checksum、保留及清理记录 | [保留清单](local-artifact-retention.md)、[安装记录](local-editor-installation.md) | 已分批交付；只清核验重复 App／自建诊断／构建缓存，数据库、模型、采用证据等保留 |
| 正式数据不用于破坏性测试；回退及数据恢复点 | SQL／ENOSPC 均隔离；本机更新前后一致性恢复点和 20 表摘要 | 有证据；正式仅只读打开／播放，用户之后操作不作假设 |
| 基线 df5aa24、保留媒体优化、不混 online ASR 实验 | 当前 HEAD 祖先检查通过；对基线 migrations／ui/package.json／src-tauri/Cargo.toml 无 diff | 上述可直接核实；本页不据祖先关系单独证明每段代码的历史来源 |
| 不改字幕 schema、导出格式、源译关系、框架和桌面技术 | 无迁移／依赖清单差异，现有序列化与导出回归、SQLite／Tauri／TypeScript 保持 | 有相应源码／回归证据；不把“无迁移”当作导出全部场景证明 |
| 小提交、适用测试、fmt／diff、里程碑推送 | Git 日志、对应验收段与 local-artifacts 日志 | 有已交付提交证据；183／5 ignored 共享库为最新 Rust，当前本机 28e2909 继承 349 前端／36 passed、4 ignored 桌面；对应提交与版本单列 |
| 独立真实窗口、Windows 结果单列 | 独立 QA identifier／临时目录，macOS 原生截图／数据核对 | 部分：Windows 最新编辑器实机未验，本机 macOS 不能代替 |
| 用户先验收面板，再推进性能／模块；正式 App 手动试用 | 用户已确认继续性能／第三阶段；0cf72c0 已覆盖本机并保留回退 | 用户于 2026-10-28 确认 e8d9a8d 紧凑布局、上下原译文及跟随／锁段“目前正常”；这组面板手动验收通过，整体性能／后继源码未据此通过 |

## 合并排查范围

《教父》旧任务删除、独立短片段、重叠区先对齐后选择及真实检查点重放见[短片段基线](godfather-short-baseline.md)。撤销删除的来源／依赖丢失已按 0061 修复，不能将其当成 ASR 接缝丢句原因。人工听辨、长片质量与翻译合适程度仍待验证。

[翻译消耗观测](translation-usage-observation.md)已交付，但本轮没有调整 chunk／语义分组、上下文或缓存键；实际请求次数、真实费用／命中率改善未证明。8ab7c59 只是事务实现搬移，不解决模型质量问题。

按 2026-10-30 收尾调整，后续只处理日常使用中可复现的问题，不继续自动扩大性能采集、故障组合或 Windows 验收；剩余生命周期也只按实际问题整理。无需为了“模块拆分完成”继续把非编辑产品区域逐方法机械搬移，也不得在这些证据缺失时标记整个目标完成。

2026-10-28 补充：用户确认 [0062](decisions/0062-editor-audio-demand-loading.md) 编辑页音频按需加载。349 前端、HTTP 专项及十四组受影响 WebKit 回归通过；QA 778d936 同三小时样本原生确认打开／选段／未加载重开零播放器请求、暂停定位和首次恢复播放，插入取消及 20 表不变。首次实际加载仍会大范围预读，原生 P95／TTI 与峰值内存归因仍未闭合；正式 App e8d9a8d 未更新。

2026-10-29 本机交付：正式 App 更新至 28e2909（产品代码 778d936），27 文件／签名／四 sidecar、13 任务／6,887 段及 20 表一致性通过，只读媒体播放正常并保留窗口供用户验收。相同数据库快照链接复用，App 只保留 e8d9a8d 上一版及稳定 0.1.1；新改动用户反馈与性能／Windows 门禁仍单列，见[安装记录](local-editor-installation.md)。

2026-10-29 后继：用户确认本机 28e2909 的首次播放／定位、重开恢复及词表修正“目前正常”，该组手动验收通过。独立 QA 四次短滚动已获得跨操作首尾的宿主／surface／swap／显示关联，窗口内匹配呈现间隔 P95 16.67 ms、Hitches Updates P95 5.91 ms；采集仍有较大收尾成本，未建立连续固定输入基线，不作为同机滚动改善或完整性能门禁通过。

2026-10-29 基线补充：df5aa24 原源码的独立包已构建，同库三档顺序打开后的万段暂停短窗，WebContent CPU 累计 6.22 秒／约 1,436 MiB 对当前 0.02 秒／167 MiB；仅证实本次默认工作面短时资源差异。旧版滚动指令被 CUA 窗口／AX frame 错误阻止，录制不作有效滚动基线，P95 对照仍未完成；正式 App 未改变。
