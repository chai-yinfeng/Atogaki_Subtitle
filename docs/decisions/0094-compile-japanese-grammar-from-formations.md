# 0094 按目录接续批量生成日语句型规则

日期 2026-11-21。接续 [0093](0093-contextual-japanese-grammar-matching.md) 和[规则编译调查](../research/2026-11-18-subtitle-and-learning-options.md#批量生成规则和口语验证)。

## 规则生成

用 OpenJLPT 自带的英文接续字段作为开发输入。生成器解析加号和斜线，用共享词类表把「Verb-plain」「i-Adj」等接续编译成 Sudachi 谓词，用固定语法标记表表达「ん／の＋です／だ」等同义变体。不同实现共用条目 ID。句末缩略和排除条件进入生成结果，例如裸「かも」需要句末证据，「のでは」不计入因果「ので」。

生成结果以 Rust 数据随 App 打包，沿用现有匹配器、55 条人工规则、目录 ID、高亮和收藏。显式选择已经核验含义和词形的条目。编译遇到任何不认识的结构会拒绝生成该条目，`--audit` 可列出更多语法上能编译的候选并统计缺失的模板。当同形有不同语义时仍需限定语境，例如推测「らしい」暂用谓语接续，名词性「典型的……」另行处理。第一批启用 23 条，共 78 条自动识别。程序运行时不解析教材说明，也不需要 LLM。

可选方案包括继续逐条手写 Rust、直接导入 `nnj-grammar` 生成的 Hanabira 规则，以及在运行时解释 OpenJLPT 的接续。选用离线编译以复用现有分词器、稳定 ID 与条目的许可来源，并使生成结果可审查和复现。采用 Tsutsuji 的规范表达与派生表达共用条目思路。上游备选的授权与粗粒度变体误报见[调研记录](../research/2026-11-18-subtitle-and-learning-options.md#批量生成规则和口语验证)。

```sh
python3 scripts/generate-japanese-grammar-rules.py
python3 scripts/generate-japanese-grammar-rules.py --check
python3 scripts/generate-japanese-grammar-rules.py --audit
ATOGAKI_DICTIONARY_SMOKE_DIR=/absolute/path/to/qa-dictionaries \
  cargo test --locked --offline --manifest-path src-tauri/Cargo.toml japanese_grammar -- --include-ignored
```

## 真实字幕回归

用本机 Apple Silicon 和 Sudachi Core 20261015 比较基线 `6c13ea8` 与本次实现。只读取得同一期电台的 149 条已保存日文字幕。逐条调用真实分词器与匹配器，同一台机器、同一词库、同一选区。第 0 至 108 条用于开发和诊断。第 109 至 148 条在规则形成后取出，对新增 23 个条目先按字幕文本人工标注，再运行候选预测。另有一项来自独立合成句的「と思っている」高亮修订，重跑后下表不变。

| 字幕范围 | 基线有命中 | 新版有命中 |
| --- | ---: | ---: |
| 开发段 | 30/109 | 50/109 |
| 保留段 | 13/40 | 17/40 |
| 整期 | 43/149 | 67/149 |

保留段的 23 类标注中有 13 个「字幕段 × 句型 ID」预期结果。新版报出 8 个，均对应标注，其中 6 个高亮范围与标注吻合。漏项包括带句末语气词或口语引语的「と思う」、省去判断助动词的说明句，以及接在过去形后的「だけ」。两处高亮少含了复合谓语的前部。统计只涉及一集转写和此次新增的 23 类，整期有命中条数不能当作准确率。评估逐条处理，字幕跨行和收藏继续由已有集成用例覆盖。49 条新增条目自带例句均通过真实 Sudachi 回归。

本地运行记录、规则和输入摘要、标注与比较脚本保存在忽略目录 `local-artifacts/evaluation/radio-grammar-20261121/`。从仓库根目录执行 `python3 local-artifacts/evaluation/radio-grammar-20261121/compare.py` 可用保留的两个同机探针重跑。节目原文和逐条标注仅在本机使用。
