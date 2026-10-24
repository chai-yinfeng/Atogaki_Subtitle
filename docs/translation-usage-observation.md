# 翻译消耗与缓存观测

2026-10-23。范围：保留当前翻译组、batching、上下文与 revision 合同，补充观测；不发起真实 API 调用，不改变分组或扩大上下文。

## 字段与归属

`TranslationUsage` 保留 input/output tokens，并增加可选的 `cached_input_tokens`、`uncached_input_tokens`、`local_cache_hit`。前两项取自 DeepSeek 的 `prompt_cache_hit_tokens`／`prompt_cache_miss_tokens`，只保留非负数；缺失仍为 unknown，不推断为零，也不反推命中率。[官方说明](https://api-docs.deepseek.com/zh-cn/guides/kv_cache/)描述的是云端输入前缀缓存，与本地整组结果复用分别记录。

batching 仍只把整次响应 usage 分配给一个接收组，包含新增缓存字段；其他组使用缺失值。不能把每组都赋予整次 usage，也不能从组数推导 HTTP 次数。

工作区新写入的翻译计划 `plan_json.execution_usage` 保存本次返回的完整 usage。原有 SQLite input/output 列继续保留；没有数据库迁移或字幕 schema 修改。旧记录不回写。缺失的新字段不序列化，使旧缓存响应反序列化后重新计算 checksum 仍兼容。

任务工具的最近记录会明确显示“本地缓存复用（本次无新增 API 消耗）”，或本次 token 与 provider 报告的云端输入缓存计数。旧记录没有新 metadata 时保留原 token 标签，无法解析的可选 metadata 不阻止打开工具；缺失计数不会显示为零。这仍然只描述一条记录，不是整个任务账单。

## 本地结果复用与历史成本

提前翻译缓存命中时，本次没有再次调用 provider，返回本次 input/output 为零并标记 `local_cache_hit=true`，不把缓存中的原调用消耗再次归到这次复用。原始缓存文件及 miss journal 中仍保留原调用的 usage，缓存键、checksum 和 revision 校验不变。最终工作区记录的本次零新增，不能理解成整个识别／翻译管线没有消耗；此前提前翻译的成本应从原调用记录读取，不能只汇总最终工作区的记录。

每次实际结果复用在 `early-translation/cache-hits/` 保存一个独立事件，包含请求摘要、配置摘要、时间和本次零新增 usage，不重复保存字幕正文。`process_prefix` 判断某组已完成而不再调度时不产生这种实际复用事件，因此这些事件数量不是所有缓存查询的总次数，也不能直接作为全局本地命中率。

## 离线查看

```sh
python3 scripts/summarize-translation-usage.py /absolute/path/to/job \
  --output local-artifacts/evaluation/new-usage-report.json
```

工具只读该任务的 pipeline group-attempt 与 cache-hit journal；拒绝覆盖报告，不读取凭据或发网络请求，不输出字幕正文。分别给出有／无 provider 响应的语义组、实际复用事件、已知 token 和报告了缓存 hit/miss 的范围；响应存在不等于结构／译文质量验收通过，缺失值保留 unknown。缺少作用域的历史 journal 另计，不当作 HTTP 记录或完整账单。

本轮仍没有真实 HTTP／重试逐次 journal，失败请求的潜在消耗也不可由成功响应推断；`http_requests` 明确为 unknown。编辑器单独重译的 usage 可在新 `plan_json.execution_usage` 中读取，但不属于这个 pipeline journal 汇总工具的覆盖范围。后续再补 wire 层计数和真实云端质量／吞吐样本，然后决定稳定前缀、场景上下文和缓存复用策略；不以新观测字段宣称命中率或 API 消耗已经改善。

## 验证与历史数据限制

共享库 158 passed／5 环境依赖 ignored，包含旧格式 response checksum 原样兼容、本地复用不调用 provider／不重写原缓存、48 个语义组打包成 4 个响应时 cache token 仅累计一次，以及计划 metadata 持久化。桌面 check 与 33 passed／4 ignored 通过。前端 106 项、类型／生产构建及 WebKit 翻译／跟随专项通过，覆盖本地复用与云端缓存标签、缺失／错误 metadata 回退、草稿 flush、失败续译及旧视图异步隔离。全部使用本机夹具，无真实云端请求。

离线汇总工具用独立夹具验证已知缓存比率、批量 usage 不重复、无响应成本 unknown、历史作用域 unknown、禁止覆盖报告。此前《教父》的隔离副本中 1,704 条旧 attempt 缺少新的作用域标记，工具单列为 unknown；provider cache hit/miss token 没有历史观测值，不能据此说旧任务命中率为零或没有费用。原记录不修改。

本机正式 App 仍为 `05b2e07`，本轮观测改动先保存在后续代码提交，不在用户手动验收期间再次覆盖。
