# 0073：Index-Translate本地后端与成组翻译上下文

日期：2026-11-06。状态：用户确认实施。沿用0072的日语分组与提交时机，更新其上文组织方式；不修改ASR、白色内容不可回改或显示软窗口。

## 模型与接口

新增`index-translate-local`离线后端与在线`index-translate`选择，保留原有默认后端。首版使用[Index官方GGUF](https://huggingface.co/IndexTeam/Index-Translate-2B-GGUF)的`Index-Translate-2B.Q4_K_M.gguf`，revision `3a0eca6ce931d88d00614d427ab3ed8ff0743902`，1,312,164,352 bytes，SHA-256 `044b313d29342bd3b2c77cbb64023ca0d209bd9b3247763f9d162767ef2d746a`。

现有固定llama.cpp支持Qwen3.5，已实测加载与Metal翻译，因此复用受管loopback服务、设备独占、停止回收和模型下载校验。未新增MLX runtime或转换模型；用户允许的MLX 4bit作为后续可选比较，不是当前已交付路径。

Index使用[官方约束翻译格式](https://huggingface.co/IndexTeam/Index-Translate-2B)：单user turn、源文与约束分开、temperature=0、显式`enable_thinking=false`。离线沿用严格JSON schema、稳定cue ID、专名占位恢复和既有事务；在线输出纯译文，移除完整thinking块并拒绝未闭合thinking块。Hy-MT2模板与采样参数保持各自路径。Index与Hy同属本地设备工作，不能误入云端提前翻译队列。

## 上下文与可靠提示

在线历史保留完整`(source, translation)`对，按音频提交顺序编号；最多8组且源译合计2000字符，超预算时淘汰最旧的完整组，不独立截断原文和译文。只有MT最终提交才进入历史，草稿不积累为背景。

提示分为用户提供场景、专名／术语、历史源译对、后文参考和当前待译文本。历史译文明确标为可能错误的措辞参考，解释指代／省略优先参考原文；不得复述背景、擅自补足未完句或用场景覆盖明确的原文事实。实际模型遵循仍需人工检查，提示文字不是解码保证。

控制面板支持现有词表与可选场景提示。词表使用离线`glossary_for_task`同一核心范围，排除correction_only及未选择的内容组；源语言必须一致。其作用是保护专名原样，不将ASR纠错箭头当作双语译法，也不改写ASR文本。会话开始时冻结核心专名列表与最长1000字符场景，通过worker环境传递；后续词表编辑不改变正在运行的会话，不新建任务或保存录音。

每次只注入出现在当前／历史／后续原文中的相关专名，最多1200字符。完整prompt另以6000 UTF-8 bytes作保守长度保护：先移除完整旧组，再移除可选后文；静态场景／词表仍过大时报错，不截断专名或场景事实。这不是精确token计数，服务仍配置8192-token context和独立输出预算。不会因为增加提示而自动扩大MT分段或音频等待。

## 验证与限制

完整对齐组淘汰、草稿不进历史、角色优先级、相关专名筛选、Index模板／thinking和prompt长度回归通过。实际受管Index离线调用验证了两条cue ID及`n-buna`专名恢复并回收进程；在线适配器也完成三条合成日文请求及12秒development文件的完整two-pass回放与EOF排空。测试证明接入合同，不代表总体翻译质量：未完句仍可能被补足，人物关系、授受语气与上下文收益待节目人工比较。记录见[链路验证](../two-pass-smoke-validation.md#index与成组上下文2026-11-06)。

未采用自动生成节目总结或人物关系，也未硬编码电台／艺人知识。当前只使用用户明确填写的场景和既有词表核心条目；自动总结、词表内容组选择、MLX后端及上下文token精确分配另行评估。
