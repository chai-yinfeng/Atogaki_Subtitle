# 0034：词典包解析发布元数据，网络词典凭据按来源隔离

日期：2026-08-24

## 状态

已采纳。

## 背景

学习区需要让用户自行下载 JMdict、Tomoshi 等离线数据，并分别配置 Cambridge、Collins、Merriam-Webster API。离线数据更新频率不同，固定“最新版”裸链接无法同时保证版本可追溯与内容完整性；商业 API Key 若与字幕翻译共用一个设置项，则容易覆盖、误读或扩散到 SQLite。

英文到中文也存在开放离线数据。FreeDict 当前提供由 WikDict/Wiktionary 导入的 `eng-zho` 包、版本化下载地址与 SHA-512，但词头、校订深度和学习型结构不能等同于 Cambridge 或 Collins。ECDICT 词量更大，但其汇总数据的逐项来源与再分发许可暂不足以作为默认正式包。

## 决策

- 正式应用数据目录下建立 `dictionaries/`；离线包只在用户点击下载后进入该目录，不打入 App Bundle，也不写入临时测试目录。
- JMdict 使用 `scriptin/jmdict-simplified` 的日英 JSON 发布包，Tomoshi 使用其开放 SQLite 发布包。点击下载时从 GitHub Releases API 解析最新 tag、预期资产、大小与发布方 SHA-256；缺少摘要时拒绝安装。
- FreeDict 英中先作为可选离线补充，固定到 `2025.11.23` StarDict 包并校验 FreeDict 数据库公布的 SHA-512。UI 明确它不是商业学习词典的等价替代；包内 `.ifo`／COPYING 已确认该版本使用 CC BY-SA 3.0 Unported。
- 所有下载先写同目录 `.part`，流式计算摘要；通过后才替换稳定文件名，更新失败会恢复旧包；版本另存为不含用户数据的 sidecar。App 启动清理残留 `.part`，但不自动更新或删除已安装词典。
- Cambridge、Collins、Merriam-Webster 使用 `dictionary:<provider>` 凭据 ID 分别写入平台系统凭据库。SQLite 只保存“曾由本 App 成功保存”的布尔标记；打开设置和列出状态不读取 Keychain，只有用户主动保存、删除或检查时访问。
- Merriam-Webster 继续作为独立来源标签页，与其他来源并列切换；不做自动排名、合并或评价。请求只由用户点击触发，结果显示官方未修改 Logo、完整产品名与署名，并使用有期限的本地缓存；公开分发前仍须重新确认非商业用途和当时有效条款。

## 后果

- 资源准备与实际查询由后续决策 0035 衔接；设置页的“已安装／已保存”只说明输入可用，是否命中仍由各 provider adapter 独立报告。
- GitHub 发布元数据和包下载都复用桌面非敏感代理设置；模型镜像只属于 Hugging Face，不改写词典来源。
- 下载器直接保存上游归档，因此后续查询层必须支持相应格式，或在保持许可证与可追溯信息的前提下增加受校验的安装转换步骤。
- 开放数据的下载入口不代表 Atogaki 对释义质量作背书；公开发行前仍需完成逐包许可证与署名审计。
