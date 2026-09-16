# Hy-MT2 本地翻译验证

核对日期：2026-09-16。

本阶段先验证标准 `Q4_K_M` GGUF 和主线 llama.cpp 的完整离线链路。固定值位于 `scripts/hy-mt2-versions.zsh`：

- llama.cpp `v0.4.1`，commit `391fac16460f15233a7740550d858ac96df3419d`；源码包 SHA-256 `82977400c28b7486f90126a5592c9ff585f9b2ae0a3be2c9ab77464e5eef78cc`。
- `tencent/Hy-MT2-1.8B-GGUF` revision `a0c709d9fac510f2c807aa3af52872340dc37a4a`，`Hy-MT2-1.8B-Q4_K_M.gguf`，1,133,080,448 bytes，SHA-256 `dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699`。
- `tencent/Hy-MT2-7B-GGUF` revision `ab8472660ac61fac25f1af43fac2599d52a8a775`，`Hy-MT2-7B-Q4_K_M.gguf`，4,624,648,896 bytes，SHA-256 `9f96256500f3fc1ab4d64336b58f52a949a95ad7516b0c229476eef782f9f77b`。

官方模型卡给 1.8B 和 7B 的建议采样参数为 `temperature=0.7`、`top_p=0.6`、`top_k=20`、`repetition_penalty=1.05`。模型支持日语、简体中文和项目当前需要的其他主要语言，并提供术语、上下文、分隔符与结构保持提示示例。

## 选择标准 Q4_K_M 的原因

官方同时发布了 1.25-bit STQ 量化，但它依赖 llama.cpp PR `#22836`。截至核对日该 PR 仍未合并，说明中的 kernel 只覆盖 ARM NEON CPU；这不满足 Atogaki 的 macOS Apple Silicon 与 Windows x86_64 共用 runtime 要求。第一轮因此固定标准 `Q4_K_M`，STQ 只保留为后续 macOS 实验，不进入产品下载目录或推荐档位。

## 可复现验证

以下命令会在 Git 忽略的 `local-artifacts/hy-mt2/` 中下载并校验源码和模型、构建 `llama-server`、只监听 `127.0.0.1`、运行 health check 与结构／stable cue ID／术语占位符合同检查，并在退出时回收服务：

```console
./scripts/validate-hy-mt2.zsh 1.8b
./scripts/validate-hy-mt2.zsh 7b
```

可用 `ATOGAKI_HY_MT2_ROOT` 改变缓存目录，用 `ATOGAKI_LLAMA_PORT` 改变实验端口。模型和输出不提交 Git。合同验证通过只说明 runtime、请求结构和占位符链路可运行；质量、人工盲评、峰值内存、首次加载与 ASR 并发竞争需要单独记录。

## 1.8B macOS 首轮结果

2026-09-16 在 Apple M4（10 cores）、24 GB memory 上完成上述 1.8B 命令：

- `llama-server --version` 精确报告固定 commit；二进制只链接 macOS 系统 framework／library，没有 Homebrew 动态依赖。
- server 只监听 `127.0.0.1` 并要求 bearer API key；验证结束后进程正常回收。
- 两个目标 cue 均按原 stable ID 返回，JSON 可解析，译文非空，`[[ATOGAKI_TERM_0]]` 占位符完整保留。
- 本次 128 prompt tokens 的处理速度为 599.39 tokens/s，50 output tokens 为 60.14 tokens/s，请求总耗时 1.03 s。
- 模型载入并完成 health-ready 的 runtime 日志时间约 0.38 s；载入后和一次短翻译后的 RSS 均约 1.67 GiB。该值是当前机器的一次进程采样，不代替后续完整节目的峰值记录。

合同输出中的中文可以读懂，但单条合成测试不足以评价字幕自然度或与 DeepL／DeepSeek 的相对质量。下一步仍需使用固定 source cues 跑真实节目样本并盲评。

## 7B macOS 首轮结果

同一机器与 runtime 随后完成 7B 合同验证：

- 两个 cue、JSON 与术语占位符合同均通过；本次合成输入的译文为“加入 `[[ATOGAKI_TERM_0]]` 之后，／每一天都特别开心。”
- 123 prompt tokens 为 165.41 tokens/s，44 output tokens 为 21.05 tokens/s，请求总耗时 2.79 s；约为本次 1.8B output throughput 的 35%。
- runtime 日志中的模型加载时间约 2.85 s；一次短翻译后的 RSS 约 5.44 GiB。

两档在当前 24 GB Apple Silicon 机器上都能运行，1.8B 的资源和速度优势明显。7B 是否带来足够的真实字幕质量提升仍未验证，因此两者继续保持候选状态。

官方资料：[Hy-MT2 模型卡](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF)、[llama.cpp server API](https://github.com/ggml-org/llama.cpp/tree/master/tools/server)、[STQ PR #22836](https://github.com/ggml-org/llama.cpp/pull/22836)。
