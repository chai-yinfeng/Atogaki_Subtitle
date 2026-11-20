# 0092 OpenJLPT 中文目录与语法基础概念

日期 2026-11-20。接续 [0090](0090-local-japanese-grammar-and-catalog.md) 与 [0091](0091-reusable-grammar-patterns-and-chinese-cards.md)。

目录中每个 N5 至 N1 条目对应 OpenJLPT 的一个稳定 ID。它介绍具体句型的含义、可接的成分和例句。基础概念是 Atogaki 的前置知识页，例如连用形、动词分类、助词和授受视角，可由多个句型共同引用，也可以直接浏览。基础概念没有 JLPT 等级，范围按目录中反复出现的入门接续术语和阅读所需的句法概念定义。

采用两份中文数据。既有中文卡及原创基础概念保留在 `zh-Hans.json`。来自 OpenJLPT 的中文译文保存在 `zh-Hans-openjlpt.json`，按 ID 覆盖含义、接续、用法及例句译文，原始句型、等级、读音、罗马字和日文例句来自上游。译文和原创讲解在条目中以不同来源展示。依据本机 Hy-MT2 译稿做字段对齐、术语归一和抽查，未逐条做专家审校。两份数据随 App 离线提供，按 CC BY-SA 4.0 保留署名和许可。

本轮补齐 384 条译文并增加 18 张基础概念。最终目录包含 526 条中文语法卡和 21 张基础概念卡。自动匹配覆盖仍由 37 条已有规则定义。

匹配器继续使用 Atogaki 的 Sudachi 适配、OpenJLPT ID、字幕高亮范围和收藏接口。Tsutsuji 的规范形与派生体组织可作为归并变形的参考，`nnj-grammar` 的核心范围、前后文条件、有限槽位和歧义组可作为对照。后续实现见 [0093](0093-contextual-japanese-grammar-matching.md)，参考实现和授权边界见[匹配调研](../research/2026-11-18-subtitle-and-learning-options.md#日语句型匹配的参考实现2026-11-20)。
