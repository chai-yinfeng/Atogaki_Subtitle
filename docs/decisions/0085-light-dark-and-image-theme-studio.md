# 0085：亮暗主题与图片配色编辑器

日期：2026-11-17。状态：按用户已确认的基础亮／暗／跟随系统、移除月夜／来信、保留图片主题及 Material Color／完整预览方向实施。源码验证与本机安装分别记录。

## 颜色与迁移

基础主题仅为 light、dark、system。root 的 themePreference 表示用户选择，theme 表示当前实际亮暗；system 响应系统配色变化。旧 neutral／moonlight／letter 迁移为 light，首次迁移将旧选择和原始图片配置留在 beforeThemeV3；更早 beforeShellV2 备份继续保留。不删除图片、模型、任务或署名。

图片在本机缩采样后使用官方 Material Color Utilities 的 QuantizerCelebi 量化、Score 评分，排除透明像素并选择少量种子候选。单一种子产生 HCT 色阶和固定语义角色：低彩度 neutral 用于背景／阅读面，primary 与 onPrimary 用于按钮，primaryContainer 与 onPrimaryContainer 用于选中，neutralVariant 用于边框及次要文字；错误色保持独立。亮暗分别选择对应色阶，不把原图 RGB 随意分配给不同组件。

固定依赖 @material/material-color-utilities 0.3.0 并更新随包许可；0.4.0 发布包的根入口包含 Node 原生 ESM 无法解析的扩展名缺失导入，未修改依赖源码或增加解析兼容层。使用当前需求所需的量化／评分／色阶 API。

## 编辑与应用

外观页提供基础主题三选一和图片编辑入口。图片主题使用独立宽面板，左侧原图、名称、种子候选和明暗方式，右侧为完整的亮暗界面预览，底部应用栏固定。署名、来源与路径导入留在可折叠选项。导入和编辑只改变草稿，显式应用后才更新全局和保存；取消丢弃草稿，恢复基础主题保留导入配置和原图。

不再提供手工分配底／面／钮三种颜色的旧控件，删除无人调用的 picker 与样式。旧角色配置在原始备份中保留，并可提供初始种子。原图仍保留与可定位；本轮将其作为配色来源，不把图片铺到阅读文字下方。字幕浮窗保持自己的实底高对比外观，字幕导出样式独立。

异步恢复、导入、图片解析和应用均有当前请求检查；切换基础主题、取消、离开或释放后，过期结果不能覆盖新决定。Blob URL 在完成和释放时回收。在线视图共用种子角色映射，跨窗口偏好和系统亮暗变化均会刷新颜色。

## 验证

379 项前端测试及类型／生产构建通过，新增固定语义配色在白／黑／灰／高饱和种子下的亮暗文字、按钮、选中对比度检查。生产 WebKit 覆盖旧数据恢复、预览不应用、名称与取消、恢复保留、系统明暗变化、跨窗口同步、过期成功／失败以及释放期间的 URL 回收；独立检查设置、节目抽屉及运行中在线折叠。两种配色与宽面板截图已查看。本轮没有更改模型推理或运行语义评分。

参考：[Material Color Utilities](https://github.com/material-foundation/material-color-utilities)、[Raycast Themes](https://manual.raycast.com/themes)、[shadcn 语义主题](https://ui.shadcn.com/docs/theming)。
