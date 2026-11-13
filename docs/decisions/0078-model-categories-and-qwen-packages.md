# 0078：按用途管理模型与 Qwen 多文件包

日期：2026-11-13。状态：accepted，用户确认整理模型分类并保留按需测试的量化候选。

## 决策与用途

模型管理按转录、草稿识别、翻译、VAD 分组，默认折叠，按需展开。Whisper 和 Qwen3-ASR 是转录模型的不同系列；离线文件转录、在线确认识别是支持范围，不能因同属转录就假定接口已经互通。

Whisper Medium 保留为日常离线基线，Small 与 Large／Turbo 的既有量化档收进“其他 Whisper 模型”。用户在日语节目上观察到 Large 比 Medium 更频繁出现回环，这支持降低入口优先级；不把这一观察写成量化或 decoder 已被单独证明的根因。保留旧模型文件、选择与手动管理，不批量删除已下载资产。

Qwen 保留以下独立候选，均按需下载，不自动下载全部，也不更改在线默认选择：

- 0.6B 原始 BF16，约 1.88 GB。
- 0.6B MLX 4bit，约 713 MB。
- 1.7B MLX 8bit，约 2.47 GB。
- 原有 1.7B BF16，约 4.70 GB，收进其他 Qwen 模型供既有使用与比较。

原始权重通过现有 `mlx-qwen3-asr` worker 加载，以 FP16 运行；MLX 和量化是独立维度。量化候选的真实中日英质量、延迟、运行内存与当前 worker 的完整推理效果仍需后续对照，不能凭包体积或静态检查宣布质量相等。

Hy-MT2 1.8B 继续作为常用本地翻译；7B 退出新下载目录。已安装的受管 7B 仍能显示、选择与卸载，用户手动配置的路径不清空。用户此前比较的收益不足以抵消体积和延迟，且认为上下文可能更值得改进；不将此扩展为所有材料上的模型能力结论。

## 包管理与边界

目录定义集中在 `src-tauri/src/model-catalog.json`，记录用途、系列、能力、固定 revision、文件大小和 SHA-256。Qwen 沿用已有单下载任务、代理、镜像、HTTP Range 与取消机制，逐文件校验，在独立 `.part` 目录保存已验证文件与未完成下载。全部文件完成后改名为安装目录，在线选项才可能就绪；取消和失败保留可续传产物，显式卸载可以清理它们。

已导入的完整未量化包按原路径复用。若既有目录校验失败，下载操作报错并保留它，不合并或覆盖其中内容。就绪检测核对必需文件及大小；下载时校验逐文件 SHA-256，不在每次 UI 轮询中重新读取数 GB 权重。

Qwen 在模型管理中提供下载和卸载，在在线控制面板显式选择，不写入 Whisper 的离线路径设置。模型管理和在线选项共用目录，不要求手工编辑私有 manifest；旧 manifest 的有效路径继续保留，损坏的 manifest 报错而非静默重置。

SenseVoice／Zipformer 草稿和在线 ONNX VAD 先显示既有包的状态，明确标注“随在线环境准备”，不提供尚未完成的独立下载和卸载按钮。Whisper GGML VAD 与在线 ONNX VAD 分开，不能互换。

本次不完成 Python runtime 分发、草稿独立安装、新机完整在线闭环或 Qwen 离线任务 provider，不切换 native MLX，不增加自动更新器。Release／DMG 用户安装方式保持。

## Voxt 参考范围

Voxt 是开源语音输入 App，可学习其 native MLX 推理、缓存、模型管理和分发实现。用户主动停止口述通常提供较完整意群；该体验不能直接证明连续电台中的上下文边界、翻译修订和正确性已解决。性能比较必须注明模型 revision、量化、输入、硬件、冷／热启动以及停止后等待或连续字幕延迟的统计口径。

来源：[Voxt 实现与模型](https://github.com/sgl-project/sglang-omni/blob/main/Voxt/README.md)、[0.6B 4bit](https://huggingface.co/mlx-community/Qwen3-ASR-0.6B-4bit/tree/313d850181767edf09f00a9c289becca70e58cd0)、[1.7B 8bit](https://huggingface.co/mlx-community/Qwen3-ASR-1.7B-8bit/tree/a8379a2e2f9e313c9292cdf1af4055ab56d50d55)。原始 Qwen 和草稿文件延续 `experiments/online/model-pins.json` 的固定版本。
