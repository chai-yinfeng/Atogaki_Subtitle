# 原文保存后，上下文消费者的过期状态未即时显示

日期：2026-10-26。状态：已在 release QA c360cc9 原生复现；轻量回填已修复，单元／WebKit 与 release QA 2a82da1 原生复验通过。属于编辑器数据呈现缺口，不是来源／依赖撤销再次丢失。

## 复现与证据

使用 `seed_structure_undo_qa` 创建唯一临时目录，十段合成双语字幕、已采用 ASR 来源和三条翻译依赖。第三段译文依赖第一、第二段；第一段译文依赖第二段。通过真实窗口执行删除／撤销和 Cut→Join→改时间→三次撤销，数据库均恢复来源与三条依赖。

随后在第一段原文追加“追加复核。”并保存：SQLite 的第三段 `translation_dependency_stale` 从 0 变为 1，依赖边仍有三条，第三段 cue 自身的 `translation_stale` 仍为 0。编辑页立即更新第一段“待重译”，第三段仍显示“已保存”；离开任务再重开，第三段才显示“待重译”。证明后台传播已生效，当前视图的上下文状态没有回填。

原生 AX、数据库阶段快照、比较脚本与 package checksum 位于忽略目录 `local-artifacts/editor-token-native-c360cc9/`，重点为 `context-edited.ax.txt`、`context-edited.json`、`context-propagation-verification.json`、`adoption-undone.json`；正式数据未参与。

## 已定位的代码边界

`ui/src/editor-backend.ts` 的 updateText 返回单 cue，`main.ts` 的 updateSubtitleDraftUI 只更新保存行及对应草稿。任务初始 detail 和结构提交／撤销均携带 translation_context_stale_ids，但文字保存未更新该集合，也未局部刷新受影响消费者。因此恢复后的依赖在后端正确，当前列表、上下文筛选和计数可能暂时仍使用旧集合。

下一步补齐文字保存后的轻量上下文状态回填，保留唯一草稿、650 ms 保存、IME 和 revision 校验；异步回执核对任务／访问／请求代次，并防止较旧读取覆盖新结构结果。不通过重新加载完整任务证据或重建全部字幕 DOM 解决。先增加可复现的失败用例，再确定最小接口边界并实施；新包通过原生复验前不替换正式 App。

## 2026-10-26 修复与回归

新增 get_translation_context_state，只在同一读取事务返回 workspace revision 与过期 cue ID，不读取字幕文本、run 或原始证据。文字保存／撤回、单段翻译及词表改动后按需读取；前端 editor-context-status 串行合并连续读取，核对任务／navigation／请求代次。新 detail、翻译或结构结果使旧读取失效；关闭释放待决工作，不轮询、不自动重试失败写入。

集合对比只刷新受影响的可见 cue；过期筛选开启时更新同一虚拟列表与命中数。编辑框、草稿和其他行节点保留，当前段保存状态同时考虑上下文标记。状态读取失败单独提示字幕已保存、上下文状态未同步，不将成功保存报为失败。

新增 WebKit 在旧代码以消费者行状态等待超时失败；修复后验证单行保存即时变更消费者标记、清除后更新当前过期筛选、保留输入及行／编辑节点、每次保存仅一次轻量读取和无完整任务重载。SQLite 回归验证真实依赖传播／人工修订清除、任务隔离及响应不含文本；前端回归覆盖合并、旧状态／结构结果、导航、失败和 dispose。全量共享库 183／5 ignored、桌面 Rust 35／4 ignored、前端 281、类型／生产构建与四组相关 WebKit 通过。日志在 local-artifacts/editor-context-state-20261026；正式与原生 QA 包尚未更新，新原生复验仍待完成。

## release QA 2a82da1 原生复验

以保留的默认 WAV 合成数据（十段、三条依赖、初始无上下文过期）启动已签名 release QA。第一段追加原文并保存后，没有离开或重开任务，第三段立即显示“待重译”。只读 SQLite 确认第三段 translation_dependency_stale=1、自身 translation_stale=0、三条边仍存在；原生过期筛选显示 2/10。

选择第三段，人工修改并保存译文后，其上下文标记与目标依赖清除，过期筛选即时变为 1/10；编辑框仍保留第三段的时间、原文和已保存人工译文，并提示“当前段不在筛选结果中”，未隐式切段。integrity_check=ok，正常退出进程 exit 0。证据在 local-artifacts/editor-context-state-20261026 的 native-*.json／AX／截图及 native-package.json。该显示缺口已关闭；真实磁盘失败、原生拖动、长期性能和 Windows 仍独立验收。
