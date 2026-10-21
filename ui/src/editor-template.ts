export function editorTemplate(primaryShortcutModifier: string): string {
  return `    <section id="karaoke-view" class="karaoke-view hidden" aria-labelledby="karaoke-job-title">
      <div class="editor-command-bar"><button type="button" class="secondary" data-editor-action="back">← 任务</button><span class="editor-workspace-label">字幕编辑</span><h2 id="karaoke-job-title">正在打开任务…</h2><button type="button" class="secondary" data-editor-action="follow" aria-pressed="true">正在跟随播放</button><button id="relink-job-media" type="button" class="secondary hidden">重新定位媒体</button><button id="karaoke-open-workspace" type="button" class="secondary" disabled>翻译与词表</button><button type="button" class="secondary" data-editor-action="export">导出…</button><button type="button" class="secondary" data-editor-action="repair">识别与复核…</button><button type="button" class="secondary" data-editor-toggle="video" aria-controls="editor-video-panel">收起视频</button><button type="button" class="secondary" data-editor-toggle="waveform" aria-controls="editor-wave-panel">收起波形</button><button type="button" class="secondary" data-editor-reset>恢复默认布局</button></div>
      <div class="editor-panels karaoke-editor">
        <div class="editor-top"><section id="editor-video-panel" class="editor-video-panel" aria-label="媒体预览"><div id="karaoke-media-host" class="media-host karaoke-media-host"></div><div class="karaoke-transport playback-tools">
              <div class="karaoke-clock"><span>当前时间</span><strong id="karaoke-current-time">00:00:00.000</strong></div>
              <div class="karaoke-transport-buttons">
                <button id="karaoke-step-back-small" type="button" class="secondary" title="后退 10 ms（Shift+←）">−10 ms</button>
                <button id="karaoke-step-back" type="button" class="secondary" title="后退 100 ms（←）">−100 ms</button>
                <button id="karaoke-toggle-playback" type="button">播放</button>
                <button id="karaoke-step-forward" type="button" class="secondary" title="前进 100 ms（→）">+100 ms</button>
                <button id="karaoke-step-forward-small" type="button" class="secondary" title="前进 10 ms（Shift+→）">+10 ms</button>
              </div>
              <div class="karaoke-transport-options">
                <label>速度<select id="karaoke-playback-rate"><option value="0.5">0.5×</option><option value="0.75">0.75×</option><option value="1" selected>1×</option><option value="1.25">1.25×</option><option value="1.5">1.5×</option><option value="2">2×</option></select></label>

              </div>

              <p id="karaoke-media-message" class="media-message">正在打开当前任务的媒体。</p>
            </div></section><div class="editor-splitter editor-column-splitter" tabindex="0" role="separator" aria-orientation="vertical" aria-label="视频与右侧工作区比例" aria-valuemin="25" aria-valuemax="65" data-editor-resize="videoRatio"></div>
        <div class="editor-right"><section id="editor-wave-panel" class="editor-wave-panel" aria-label="声音与时间轴">          <section class="waveform-panel" aria-labelledby="waveform-heading">
            <div class="panel-heading compact">
              <h3 id="waveform-heading">波形与时间轴</h3><span id="editor-wave-follow-state" role="status">视窗跟随中</span>
              <div class="waveform-navigation"><button id="karaoke-window-back" type="button" class="secondary">← 前一屏</button><button id="karaoke-window-forward" type="button" class="secondary">后一屏 →</button><button id="undo-subtitle-structure" type="button" class="secondary" disabled>撤销打轴</button></div>
            </div>
            <div id="karaoke-waveform-status" class="waveform-status">选择任务后生成本地波形缓存。拖动字幕块可移动，拖动左右边缘可单独修剪；空白合法，同轨不能重叠。</div>
            <canvas id="karaoke-waveform" class="karaoke-waveform" width="1200" height="260" aria-label="声音波形与字幕时间轴：拖动顶部播放头定位，拖动波形背景平移"></canvas>
            <div class="editor-wave-controls">                <label>时间缩放<input id="karaoke-zoom" type="range" min="0" max="100" value="66" aria-label="时间轴缩放"><output id="karaoke-zoom-label">30 秒</output></label>
                <label>波形强度<input id="karaoke-waveform-gain" type="range" min="1" max="8" step="0.25" value="1" aria-label="波形显示强度"><output id="karaoke-waveform-gain-label">1×</output></label></div>
          </section></section>
        <div class="editor-splitter editor-wave-splitter" tabindex="0" role="separator" aria-orientation="horizontal" aria-label="右侧波形与编辑比例" aria-valuemin="20" aria-valuemax="55" data-editor-resize="waveRatio"></div>
          <section class="karaoke-current-segment">
            <div class="editor-cue-toolbar"><span id="karaoke-segment-time">当前没有字幕</span><div class="editor-cue-navigation"><button type="button" class="secondary" data-editor-action="previous">上一句</button><button type="button" class="secondary" data-editor-action="next">下一句</button></div><div class="editor-timing-fields"><label>开始<input id="editor-cue-start" aria-label="当前字幕开始时间"></label><label>结束<input id="editor-cue-end" aria-label="当前字幕结束时间"></label><button type="button" class="secondary" data-editor-action="save-time">保存时间</button></div>            <div class="karaoke-segment-actions"><button id="karaoke-save-text" type="button" disabled>保存文字</button><button id="karaoke-cut-segment" type="button" class="secondary" disabled>切开 ${primaryShortcutModifier}B</button><details class="editor-menu"><summary>更多编辑</summary><div class="editor-menu-items"><button type="button" class="secondary" data-editor-action="translate">重译当前句</button><button type="button" class="secondary" data-editor-action="glossary">加入词表修正</button><button id="karaoke-discard-text" type="button" class="secondary" disabled>放弃文字修改</button><button id="karaoke-undo-text" type="button" class="secondary" disabled>撤回上次文字编辑</button><button id="karaoke-join-segment" type="button" class="secondary" disabled>连接下一块</button><button id="karaoke-insert-block" type="button" class="secondary">插入字幕块</button><button id="karaoke-delete-block" type="button" class="secondary">删除当前块</button></div></details></div></div>
            <div class="karaoke-text-editor"><label>原文<textarea id="karaoke-current-source" rows="3" placeholder="选择字幕后编辑原文" disabled></textarea></label><label>译文<textarea id="karaoke-current-translation" rows="3" placeholder="尚无译文" disabled></textarea></label></div>
            <p id="karaoke-timing-message" role="status">拖动按 10 ms 网格吸附；默认不会移动相邻字幕，修改会自动保存。</p>
          </section>
        </div></div><div class="editor-splitter editor-top-splitter" tabindex="0" role="separator" aria-orientation="horizontal" aria-label="上方工作区与字幕列表比例" aria-valuemin="45" aria-valuemax="78" data-editor-resize="topRatio"></div>
        <section class="editor-list-panel" aria-label="字幕列表"><div class="editor-list-toolbar"><input id="editor-search" type="search" aria-label="搜索字幕" placeholder="搜索原文或译文"><select id="editor-filter" aria-label="筛选字幕"><option value="all">全部字幕</option><option value="untranslated">未翻译</option><option value="stale">译文过期</option><option value="review">待复核</option></select><span id="editor-match-count" role="status"></span><button type="button" class="secondary" data-editor-action="reveal-playing">回到播放位置</button></div><p id="editor-selection-message" role="status"></p><div class="editor-cue-header" aria-hidden="true"><span>#</span><span>起止时间</span><span>原文</span><span>译文</span><span>状态</span></div><div id="editor-cue-list" class="editor-cue-list" role="grid" aria-label="字幕列表"></div></section>
      </div>

      <dialog id="editor-repair-dialog" class="editor-repair-dialog"><div class="dialog-heading"><h2>重新识别与复核</h2><button type="button" class="secondary" data-editor-action="close-repair">关闭</button></div><div class="karaoke-repair-drawer">            <div class="retranscription-range" aria-labelledby="retranscription-heading">
              <div><p class="eyebrow">RECOGNIZE SELECTED RANGE</p><h4 id="retranscription-heading">局部重新识别</h4><p>选一段音频重新识别，先对照原字幕，再决定是否替换。</p></div>
              <label>开始<input id="retranscription-start" inputmode="decimal" placeholder="00:49:00.000" /></label>
              <label>结束<input id="retranscription-end" inputmode="decimal" placeholder="00:55:00.000" /></label>
              <label>识别 provider<select id="retranscription-provider"><option value="whisper.cpp">Whisper.cpp（本地）</option><option value="gemini-transcribe">Gemini 3.5 Transcribe（云端）</option></select></label>
              <label id="gemini-upload-consent" class="clear-secret hidden"><input id="authorize-gemini-upload" type="checkbox" /><span>允许将这次所选范围的音频上传给 Google；免费层数据可能用于改进产品</span></label>
              <div class="retranscription-actions"><button id="retranscription-use-current" type="button" class="secondary">使用播放头所在字幕段</button><button id="retranscription-use-window" type="button" class="secondary">使用当前波形范围</button><button id="retranscription-expand-segments" type="button" class="secondary">对齐到完整字幕块</button><button id="preview-retranscription" type="button">识别并预览</button></div>
              <p id="retranscription-message" class="media-message">范围边界不能切穿已有字幕块；需要时请显式扩展。</p>
              <details id="asr-quality-review" class="asr-quality-review"><summary id="asr-quality-summary">转录待复核提示</summary><div id="asr-quality-signals" class="asr-quality-signals"></div></details>
            </div>
</div></dialog>
    </section>
`;
}
