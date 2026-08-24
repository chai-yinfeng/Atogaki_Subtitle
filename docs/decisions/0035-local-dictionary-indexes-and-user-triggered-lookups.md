# 0035：离线词典按包版本建立索引，在线词典只由用户触发

日期：2026-08-24

## 状态

已采纳。

## 背景

用户已经在正式应用数据目录安装 JMdict、Tomoshi 和 FreeDict，并为 Merriam-Webster 保存 API Key。三个离线包的格式不同：JMdict 是约 117 MiB 的 JSON，Tomoshi 是 Zstandard 压缩的 SQLite，FreeDict 是 StarDict。逐次扫描归档会让学习区查询不可用；把上游包转换后覆盖原件又会丢失版本与校验依据。

Merriam-Webster 的 Key 绑定具体 reference work。真实回归确认当前 Key 可访问 Advanced Learner's English Dictionary，但不能访问 Collegiate。其免费条款同时要求非商业、每 reference 每日不超过 1,000 次、最多两个 reference，并要求展示官方 Logo；查询不能成为后台自动流程。

## 决策

- 原始归档继续作为已校验的来源事实保留。JMdict、FreeDict 与后续决策 0036 引入的 ECDICT 基础 CSV 在第一次查询或包版本变化后，生成 `dictionaries/atogaki-dictionary-index.sqlite`；索引只保存查询所需词形、词头、读音和规范化义项，不复制无关字段。
- Tomoshi 首次使用时在同目录原子展开为派生 `.db`，以包的版本 sidecar 判断是否需要重建；查询直接使用其 `forms`、`entries` 和 `zh_defs` 表，不把其他许可不同的表笼统归为同一来源。
- 日语 provider 只做上游已收录词形的精确查询。FreeDict 英中先查原词，再按固定、保守的英语复数、过去式、进行式和比较级规则生成词元候选并依次精确查询；命中时展示上游实际词头，不做前缀或编辑距离猜词。未命中、包缺失、解压失败或某一 API 失败只影响当前标签页，不阻止收藏、简明译义或切换其他来源。
- Merriam-Webster 默认先尝试更符合学习场景且已实机验证的 `learners` reference；若官方明确返回当前 Key 未订阅，则尝试 `collegiate`，成功后把 reference ID 作为非敏感设置记住。不会枚举其他产品。
- Merriam-Webster 请求只在用户点击“查询／刷新来源”时发生。结果显示官方提供且未修改的 50px Logo、完整产品标题、音标、简明定义、例句和可用的远程音频 URL；SQLite 缓存 24 小时，到期即删除。Key 不进入 URL 日志、SQLite 或 UI 回显。
- 词典 API Key 的耐久副本继续只存在系统凭据库。每次 App 进程第一次实际使用该来源时读取一次，后续查询复用进程内存；保存／检查／删除凭据会同步刷新该缓存，退出 App 即丢弃。学习详情可把一条已经持久化的词典释义设为外层简明译义，并记录 provider 名称；用户手工编辑后清除来源字段。
- 网络请求复用桌面代理设置，并设 45 秒整请求上限；代理离线或第三方无响应必须回到当前来源的可重试错误卡片，不能让整个学习详情永久停在忙碌状态。
- FreeDict `2025.11.23` 的展示许可固定为 CC BY-SA 3.0 Unported；JMdict 显示 EDRDG 与 CC BY-SA 4.0；Tomoshi 对本次查询经过的 `forms/entries/zh_defs` 显示 CC BY-SA 4.0 及 Tomoshi／EDRDG 署名。
- XZ 解压通过 `xz2` 的 `static` feature 内建 liblzma，避免 macOS App 在签名后依赖 Homebrew 动态库；Windows 也不要求用户另装 XZ 运行时。最终 Bundle 必须用 `otool`／实机启动确认没有开发机路径依赖。
- Cambridge 与 Collins 保留 provider 和凭据入口，但在取得可验证协议与账户前不实现虚构请求，也不显示“可查询”。

## 后果

- 第一次 JMdict 查询在 Debug 构建和当前真实包上完成解析加索引约需半分钟；Release 会更快，但 UI 必须明确提示首次建索引。之后精确词形直接走本地 SQLite。
- 索引和 Tomoshi 解压数据库是可重新生成的派生数据，不进入仓库和安装包；更新原始包不会破坏旧包，首次新查询再重建。
- Merriam-Webster 仍属于开发者自带 Key 的非商业能力。若 Atogaki 将来收费、广告变现、超过额度或公开分发给更广泛用户，必须先重新取得相应授权；当前实现不把第三方来源合并成排行或基准比较。
- 规则词形回退覆盖常见规则变化，但不会假装解决 `went → go` 等不规则词、短语动词和 FreeDict 本身的收录缺口。若真实查询仍频繁失败，再引入可审计的离线 lemmatizer／不规则词表；不以模糊匹配自动替用户选词头。
