# 原文保存后，上下文消费者的过期状态未即时显示

日期：2026-10-26。状态：已在 release QA c360cc9 原生复现，待修复。属于编辑器数据呈现缺口，不是来源／依赖撤销再次丢失。

## 复现与证据

使用 `seed_structure_undo_qa` 创建唯一临时目录，十段合成双语字幕、已采用 ASR 来源和三条翻译依赖。第三段译文依赖第一、第二段；第一段译文依赖第二段。通过真实窗口执行删除／撤销和 Cut→Join→改时间→三次撤销，数据库均恢复来源与三条依赖。

随后在第一段原文追加“追加复核。”并保存：SQLite 的第三段 `translation_dependency_stale` 从 0 变为 1，依赖边仍有三条，第三段 cue 自身的 `translation_stale` 仍为 0。编辑页立即更新第一段“待重译”，第三段仍显示“已保存”；离开任务再重开，第三段才显示“待重译”。证明后台传播已生效，当前视图的上下文状态没有回填。

原生 AX、数据库阶段快照、比较脚本与 package checksum 位于忽略目录 `local-artifacts/editor-token-native-c360cc9/`，重点为 `context-edited.ax.txt`、`context-edited.json`、`context-propagation-verification.json`、`adoption-undone.json`；正式数据未参与。

## 已定位的代码边界

`ui/src/editor-backend.ts` 的 updateText 返回单 cue，`main.ts` 的 updateSubtitleDraftUI 只更新保存行及对应草稿。任务初始 detail 和结构提交／撤销均携带 translation_context_stale_ids，但文字保存未更新该集合，也未局部刷新受影响消费者。因此恢复后的依赖在后端正确，当前列表、上下文筛选和计数可能暂时仍使用旧集合。

下一步补齐文字保存后的轻量上下文状态回填，保留唯一草稿、650 ms 保存、IME 和 revision 校验；异步回执核对任务／访问／请求代次，并防止较旧读取覆盖新结构结果。不通过重新加载完整任务证据或重建全部字幕 DOM 解决。先增加可复现的失败用例，再确定最小接口边界并实施；新包通过原生复验前不替换正式 App。
