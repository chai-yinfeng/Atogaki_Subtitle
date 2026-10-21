# 统一字幕编辑器：面板阶段验收

日期：2026-10-21。继承基线：`df5aa24`。实现分支：`codex/unified-subtitle-editor`。

## 交付边界

第一阶段统一四区工作面、视频／波形折叠、三处分隔线、版本化本机布局记忆、紧凑列表与独立双语编辑框。完整任务默认进入编辑器，任务工具按需打开；编辑选择与播放高亮分离。搜索、未翻译／过期／待复核筛选共用列表，隐藏编辑段时保留草稿。继续复用既有 SQLite、cue ID、自动保存和两类撤销。

代码最小边界为 `editor-session.ts`（选择／过滤／拖动时间重基）、`editor-layout.ts`（布局与事件生命周期）、`editor-cue-list.ts`（按 ID 更新行）、`editor-template.ts` 与 `editor.css`。主入口继续协调已有媒体、草稿、时间轴与应用层用例；系统性拆分尚未进行。

第一阶段列表仍挂载全部紧凑行，没有虚拟化；不宣称长字幕性能问题已经解决。300／3,000／10,000 段同机 WebKit trace、内存、首次可交互和后台轻量摘要属于用户验收面板后的第二阶段。研究报告的静态节点数是旧粗修结构证据，不是采样结果。

## 自动化

- `npm --prefix ui test`：25 项通过，包含编辑选择与播放分离、过滤导航、布局校验、时间拖动保留期间保存的新文本及拒绝陈旧边界；原草稿／IME／恢复冲突／时间索引测试继续通过。
- `npm --prefix ui run build`：类型检查与生产构建通过。
- `cargo fmt --check`、`git diff --check`：通过。本阶段不修改 Rust 或数据库 schema。
- `scripts/verify-editor-ui.cjs`：用模拟 Tauri 接口、300 段合成字幕及内存 WAV 验证整个前端。覆盖默认入口、快速切段独立保存、筛选保留编辑段、播放不抢编辑、工具返回、折叠不换播放器、分隔线键盘操作、时间保存、IME 切段保护、保存失败／重试和重新加载布局偏好。
- 在 1180×760 和 960×640 验证页面高度等于视口、列表独立滚动，旧粗修不再生成 textarea。曾发现旧 shell 外边距导致底部裁切，已修复。

复现浏览器测试（需本机安装 Chrome 与 Playwright；可用已有依赖，不添加生产依赖）：

```sh
npm --prefix ui ci
npm --prefix ui run dev
# 另一终端；如果 Playwright 不在默认查找路径，指定它的绝对模块目录。
ATOGAKI_PLAYWRIGHT_MODULE=/absolute/path/to/node_modules/playwright node scripts/verify-editor-ui.cjs
```

支持 `ATOGAKI_EDITOR_URL`、`ATOGAKI_CHROME`、`ATOGAKI_EDITOR_QA_OUTPUT`，截图默认写到 `/tmp/atogaki-editor-ui/`。脚本只访问本机前端、模拟 IPC 和合成音频，不读取正式任务、模型或凭据。

## macOS 原生验证

使用独立 bundle identifier `com.chai-yinfeng.atogaki.editor-qa` 的 debug 包和绝对数据目录 `/tmp/atogaki-editor-native/data`；未替换 `/Applications/Atogaki.app`，正式实例保持运行。合成 60 秒 WAV 与 20 段字幕直接写入隔离 SQLite，不重新运行 ASR、不调用云端。

已验证完整任务直接进入四区编辑、两段中文快速切换后各自保存，并读 SQLite 确认 source_text 与编辑标记持久化；折叠视频和波形后媒体继续播放至 60 秒，编辑选择仍保持第二段。原生工具浮层发现 modal 语义在 WKWebView 隐藏动态复用控件的可访问性内容；工具改用具名 region、限定键盘焦点与 Escape 返回，保留识别／导出确认所需的原生 dialog。

此包是面板验证产物，不是正式发行包；构建未打包 sidecar。Windows 原生窗口、真实视频的解码／自动回退、真实翻译与候选采用、实际导出和大规模 WebKit 性能仍需后续专项验收，不能从合成前端回归推断通过。

## 后继阶段

用户已确认面板大体正常；入口统一称「字幕编辑」、完整任务直达及内外间距修正已交付。第二阶段实现与剩余门禁见 [性能验证](editor-performance-validation.md)，再做系统性模块拆分、unused 检查、文档归档、Web／Postgres 独立退役影响报告和产物保留清单。本阶段未清理模型、恢复备份、发行资产或 ASR 来源证据。

## 2026-10-21 布局调整

按用户要求改为上方左视频／右波形时间轴，中间双语编辑，下方列表。布局偏好 v2 保留 v1 折叠与左右比例，重新初始化纵向比例。前端 29 项、类型与生产构建通过；WebKit 在 1180×760／960×640 验证区域空间顺序、折叠／恢复、三处分隔线、保存／IME／播放／筛选闭环。后续第三阶段以这一结构抽取视图与生命周期，不新建保存或媒体逻辑。

## 第三阶段首批验证

当前句视图、持久化适配和任务工具生命周期已抽出；TypeScript unused 检查已启用。29 项前端测试、类型／构建及 Chromium／WebKit 回归通过；新增工具反复挂载、焦点约束、后台 inert 恢复与只读进度页。macOS QA 包保留原数据库，实际验证视频／时间轴并排、三处分隔线、居中编辑和翻译工具返回。按需挂载的复核 modal 曾隐藏 AX 内容，改为具名区域与显式焦点／背景管理后，原生范围／provider／复核控件均可访问，Escape 返回编辑。播放器／时间轴和 Rust repository 的系统性拆分仍在后续范围。

## 紧凑编辑工具栏

导航、时间字段与文字／结构操作集中在编辑区上方两行工具栏，状态提示保留在下方，双语正文填满剩余高度。1180×760 验证导航与时间共用一行、所有编辑操作在正文上方及正文高度至少 70 px；960×640、IME、切段、失败重试、工具返回继续通过。旧全局样式曾覆盖新工具栏 display，已通过实际截图与空间断言纠正，未仅依据构建成功判断布局通过。

## 正式本机试装（2026-10-21）

用户授权后，从 `ed48fc2` 构建正式 identifier 的 release App 并覆盖 `/Applications/Atogaki.app`。四个 sidecar、完整签名及五个可执行文件与构建产物 SHA-256 一致。原生窗口确认 14 个正式任务，打开 3,089 段真实长任务检查紧凑工具栏与扩展正文。启动后全部 20 张数据库内容表摘要与安装前一致，未执行真实识别、翻译或字幕修改。安装／清理记录见 [本机安装记录](local-editor-installation.md)。

## 0058 第一阶段：跟随与锁定

默认跟随句子、点击／键盘选择／聚焦输入后锁定；列表手动浏览只暂停滚动。原生播放行样式增强为整行底色与符号，Cut 限定编辑段。前端 30 项、类型与生产构建、cargo fmt --check、git diff --check 通过。`scripts/verify-editor-follow.cjs` 在 WebKit 合成任务验证默认跟随、编辑锁定、独立浏览、空白区、输入框聚焦、IME、异步保存期间继续输入／取消恢复、失败保存、时间输入保护、筛选隐藏播放句及 Cut 目标；现有 `verify-editor-ui.cjs` 仍验证锁定后的播放不切走编辑、工具与布局、草稿和失败重试。未用这些前端结果替代原生窗口和真实素材长时性能验收。
