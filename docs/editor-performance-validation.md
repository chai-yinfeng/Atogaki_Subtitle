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

虚拟列表、局部索引和摘要接口已交付；工具静态模板的延迟挂载、滚动帧改善与持续内存仍是剩余项，不能标记全部性能门禁完成。
