# 原生 WKWebView 时间线采集预检

本次打通实际编辑页的 Web Inspector 时间线导出和真实按键触发的 rAF 原始采样。**这是当前版单次预检，不是新旧版本性能验收通过。** 物理显示帧归属、Rendering Frame 数据、严格 TTI 和同协议多规模对照仍未闭合。

## 包与环境

业务源码为 `451e8ac`，通过 `cargo tauri build --features tauri/devtools` 构建独立 release 包 `Atogaki Inspector QA.app`，identifier 为 `com.chai-yinfeng.atogaki.inspector.qa`。只在本地诊断包开启 Inspector，没有改 Cargo 清单、lockfile、前端源码或正式 App；生产前端 dist 全文件 SHA 与构建前相同，包 27 文件和 deep strict 签名核对通过。配置、命令及完整日志保留在下述证据目录。

使用既有独立三档合成库中的 3,000 段任务和三小时 WAV，读取真实 SQLite／Tauri 服务，不使用 mock IPC。Inspector 脱离到独立窗口，编辑器内视口为 1180×728（外窗 1180×760）。第一次 CUA 绑定调用耗时 549.657 秒，但同一 PID 91043 随后可正常交互；这是工具调用延迟，不作为 App 首次可交互时间。启动日志的 sandbox extension 警告保留，未据此断言根因或修改系统权限。

## 时间线与缺失数据

只保留 Layout & Rendering、JavaScript & Events；截图、网络、CPU、内存及分配采集关闭。记录通过 Inspector 的原生 Export／保存面板写入文件；面板初次 Save 灰显，Raise 后可用，沿用已记录的隔离自动化现象，不推翻用户确认正式 App 目录选择正常的结果。

| 记录 | 窗口时长 | 布局／绘制记录 | 脚本记录 | animation-frame-fired |
|---|---:|---:|---:|---:|
| Events 模式短滚动 | 3.011 s | 24 | 19 | 0 |
| Frames 模式另起录制 | 3.546 s | 350 | 60 | 8 |
| 附带 rAF 探针的录制 | 44.025 s | 526 | 416 | 130 |

三份导出的 instrumentTypes 都包含 Rendering Frame，但该类型的实际记录均为 **0**。不能从启用项、图中的默认帧范围或 paint 数量推导帧耗时。Frames 模式另起录制仍无该记录，所以本次不继续用相同设置重复采集，也不将所有脚本条目当作 Long Tasks API 事件。

[WebKit 的说明](https://webkit.org/web-inspector/timelines-tab/)将 Frames 定义为按渲染帧组织的活动；[事件循环说明](https://webkit.org/blog/3996/introducing-the-rendering-frames-timeline/)也区分实际帧内工作与空闲间隔。本机缺少这些帧记录，不等于帧成本为零，也不能用空闲间隔代替执行时长。

## 原生页面 rAF 单样本

通过原生 Console 安装一次性诊断探针，等待 `isTrusted=true` 的真实 PageDown 后才开始两秒采样。没有程序触发 DOM 滚动或修改业务状态。测量期间保持编辑器前台，结束后才切回 Inspector 读取完整数组；Console 以编号 base64 分块输出，13 块连续校验后还原 JSON。探针源、原始数组及解析结果均保留。

该样本有 122 次回调／121 个间隔，实际窗口 2003 ms，nearest-rank 间隔 P95 **18 ms**、最大 **23 ms**；原始时间戳为毫秒整数，不报告亚毫秒精度。列表实际移动 122 px，clientHeight 153 px；当前 DOM 836 元素、2 个 textarea，结束时列表 18 行。单个 PageDown 不是持续滚动负载，这些数字也不证明规模独立或长期无泄漏。

第三份时间线有 130 个 animation-frame-fired 条目，但全记录含切换窗口、工具往返及等待，长约 44 秒；它不能整体当作两秒探针窗口。Inspector 时间线与页面 performance.now 使用不同的时间原点，本次没有建立逐事件时钟／request ID 对应。因此不声称 130 条与探针 122 次逐一匹配，不将 rAF 间隔称作完整渲染耗时或屏幕呈现间隔；Inspector、脚本采样和探针本身也有开销。

下一步先给探针开始／结束增加公开 console.timeStamp 标记及 request ID 记录，验证时钟对应；再以相同诊断 feature、采集配置、视口和真实素材建立历史版及三档重复对照。已有生产浏览器结果和本机人工正常反馈继续有效，但不能补写本轮缺失的原生指标。

## 数据与保留

原启动句柄正常退出 0，精确宿主 PID 消失。全部 20 表内容／行数摘要相同，integrity=ok、foreign key 空；包 27 文件不变。三个原生时间线 JSON、AX／已查看截图、构建与包身份、探针、Console 分块、原始 rAF 数据、数据库摘要及 SHA manifest 位于 `local-artifacts/editor-native-inspector-20261029/`。没有读取正式任务或调用模型／翻译 API。

Inspector 包临时保留用于后继对照。用途已完成的 `/tmp/atogaki-export-parent-qa/Atogaki QA.app` 在确认无进程、27 文件准确匹配后移除，逻辑 101,605,358 bytes，保留完整 checksum、已提交源码及原验收证据；不是宣称相同数量的物理空间已释放。固定 QA、正式 App、两份正式回退、数据库、媒体与模型未清理。根目录／Tauri fmt 及 diff 检查通过；构建包含前端类型／生产检查，无业务改动，未重复全量测试。
