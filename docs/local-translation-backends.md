# 轻量本地翻译后端调研

2026-11-06补充：用户选择新增Index-Translate 2B，已完成官方GGUF Q4接入与本机调用验证，见[0073](decisions/0073-local-index-translation-and-paired-context.md)。以下保留2026-11-04的原始调研时点；其中候选未因此自动下载或替换。

日期：2026-11-04。范围：本地离线日／英／韩→简体中文，以日语口语为重点。仅调查公开模型与runtime文档；未下载、实测、接入或替换现有Hy-MT2 1.8B Q4_K_M。官方总体分数不等于本项目节目质量。

## 候选与限制

| 候选 | 规模与语言 | 本项目判断 |
| --- | --- | --- |
| NiuTrans LMT-60 | 0.6B／1.7B／4B／8B；中英为中心，覆盖60语言，含日语／韩语；Apache-2.0 | 优先把0.6B列入轻量实测，1.7B作接近当前规模的候选。官方发布translation-tuned与Base两条线，应该比较非Base版本。未证明日语口语或上下文指令优于Hy-MT2。 |
| NLLB-200 distilled | 600M；覆盖日／中／韩等200语言；CC-BY-NC-4.0 | 适合作为小型逐句NMT对照；官方定位研究模型，不针对文档翻译，训练输入不超过512 tokens。不能直接承接现有背景prompt／上下文合同。 |
| M2M100 | 418M，另有1.2B；支持日／中／韩直接互译；MIT | 更小的encoder-decoder基线。需要语言token、分组和runtime适配；没有证据它能解决跨句省略或优于当前模型。 |
| TranslateGemma | 最小4B，另有12B／27B；55语言；Gemma条款 | 可列入较大但仍本地可选的专用MT对照，不能称比Hy-MT2 1.8B轻。官方输入context为2K tokens，使用特殊翻译chat template；不是直接替换当前普通chat prompt的模型。 |
| LiquidAI LFM2-350M-ENJP-MT | 350M，专门英日双向短中等文本；LFM条款 | 很小，但没有直接日→中任务支持。日→英→中会引入第二段推理与错误传播，不作为本轮日中替换候选。 |

主要来源：[LMT官方仓库](https://github.com/NiuTrans/LMT)、[0.6B模型卡](https://huggingface.co/NiuTrans/LMT-60-0.6B)、[NLLB模型卡](https://huggingface.co/facebook/nllb-200-distilled-600M)、[M2M100模型卡](https://huggingface.co/facebook/m2m100_418M)、[TranslateGemma模型卡](https://huggingface.co/google/translategemma-4b-it)、[LFM2 ENJP模型卡](https://huggingface.co/LiquidAI/LFM2-350M-ENJP-MT)。以上许可证是模型卡标注，不代表发行集成已完成审查。

## 本机运行路径

LMT-60-0.6B的官方[config](https://huggingface.co/NiuTrans/LMT-60-0.6B/blob/main/config.json)是Qwen3ForCausalLM；据此推断值得先探索已有llama.cpp／Metal路径，但没有验证当前固定版本的转换、量化、模板和输出终止合同。不可把Qwen3架构兼容直接写成该checkpoint已跑通。官方例子使用beam=5，也不能用greedy速度冒充官方质量条件。

[CTranslate2模型文档](https://opennmt.net/CTranslate2/guides/transformers.html)列有M2M100／NLLB等转换支持，[硬件文档](https://opennmt.net/CTranslate2/hardware_support.html)列有ARM64 CPU，GPU为NVIDIA／AMD，未列Metal。本机可先探索CPU INT8，但需要新增与验证推理适配，未证明比现有Metal Hy-MT2快。其他机器亦不能从M4结果外推。

“更小参数”不等于“当前配置更省内存”：0.6B BF16裸权重约1.2GB，还需runtime／KV／激活；当前1.8B已经是Q4。应比较一致精度下的实际常驻内存、加载与生成耗时，并单独评价量化质量损失。上述数字只是参数乘位宽的估算，不是实测。

## 建议的后续比较

若启动替换实验，优先Hy-MT2当前基线、LMT-60-0.6B、LMT-60-1.7B；资源允许再测TranslateGemma 4B，NLLB 600M可作CPU NMT参照。模型先使用各自官方模板，在相同人工检查过的完整语义块比较否定、授受／时态、人物关系、专名与遗漏，再检查跨块上下文、结构输出和恢复合同。不能单凭FLORES均分或参数量承诺日语节目更准确。

本轮保留现有模型列表与离线／在线翻译配置，不添加下载入口，不调用云端作为隐藏兜底。
