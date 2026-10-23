function subtitleStyleEditorMarkup(track: "source" | "translation", label: string): string {
  return `<fieldset class="subtitle-style-editor" data-style-editor="${track}">
    <legend>${label}样式</legend>
    <label class="style-font-field">字体族
      <span class="style-font-picker">
        <input data-style-field="font_family" autocomplete="off" role="combobox" aria-autocomplete="list" aria-expanded="false" aria-controls="subtitle-font-picker-options" required />
        <button type="button" class="style-font-picker-toggle secondary" data-font-picker-toggle="${track}" aria-label="浏览${label}字体">浏览</button>
      </span>
    </label>
    <label>字号<input data-style-field="font_size" type="number" min="8" max="200" step="1" required /></label>
    <label>字间距<input data-style-field="letter_spacing" type="number" min="-20" max="50" step="0.5" required /></label>
    <label>主色<input data-style-field="primary_color_rgb" type="color" /></label>
    <label>主色不透明度<input data-style-field="primary_color_alpha" type="number" min="0" max="100" step="1" required /></label>
    <label>描边色<input data-style-field="outline_color_rgb" type="color" /></label>
    <label>描边不透明度<input data-style-field="outline_color_alpha" type="number" min="0" max="100" step="1" required /></label>
    <label>描边宽度<input data-style-field="outline_width" type="number" min="0" max="20" step="0.5" required /></label>
    <label>阴影<input data-style-field="shadow" type="number" min="0" max="20" step="0.5" required /></label>
    <label>对齐与位置<select data-style-field="alignment">
      <option value="7">左上</option><option value="8">上方居中</option><option value="9">右上</option>
      <option value="4">左侧居中</option><option value="5">画面居中</option><option value="6">右侧居中</option>
      <option value="1">左下</option><option value="2">下方居中</option><option value="3">右下</option>
    </select></label>
    <label>左边距<input data-style-field="margin_left" type="number" min="0" max="1000" step="1" required /></label>
    <label>右边距<input data-style-field="margin_right" type="number" min="0" max="1000" step="1" required /></label>
    <label>垂直边距<input data-style-field="margin_vertical" type="number" min="0" max="1000" step="1" required /></label>
    <label class="style-check"><input data-style-field="bold" type="checkbox" />粗体</label>
    <label class="style-check"><input data-style-field="italic" type="checkbox" />斜体</label>
    <label class="style-check"><input data-style-field="background_enabled" type="checkbox" />启用背景框</label>
    <label>背景色<input data-style-field="background_color_rgb" type="color" /></label>
    <label>背景不透明度<input data-style-field="background_color_alpha" type="number" min="0" max="100" step="1" required /></label>
    <p class="subtitle-font-report" data-font-report="${track}"></p>
  </fieldset>`;
}

export function subtitleStyleTemplate(): string { return `<dialog id="subtitle-style-dialog" class="subtitle-style-dialog">
      <div class="dialog-heading">
        <div><p class="eyebrow">SUBTITLE STYLE & LIBASS PREVIEW</p><h2>字体与成品预览</h2></div>
        <button id="close-subtitle-style" type="button" class="secondary">关闭</button>
      </div>
      <p class="dialog-help">这里保存的是任务级原文／译文样式。预览、双语 ASS 和视频烧录复用同一个 ASS 生成器；另一台电脑缺少同名字体时仍可能 fallback。SRT 不保存这些样式。</p>
      <div class="subtitle-style-workspace">
        <section class="subtitle-style-preview-panel">
          <div class="subtitle-preview-toolbar">
            <label>预览内容<select id="subtitle-style-preview-track"><option value="bilingual">原文＋译文</option><option value="source">仅原文</option><option value="translation">仅译文</option></select></label>
            <button id="preview-subtitle-style" type="button" class="secondary">立即刷新</button>
            <button id="save-subtitle-style" type="button">保存任务样式</button>
          </div>
          <div id="subtitle-style-preview-host" class="subtitle-style-preview-host"><span>预览会使用当前播放位置附近的实际字幕和画面；纯音频任务使用中性背景。</span></div>
          <p class="subtitle-auto-preview-note">自动预览已开启：停止修改约 700 毫秒后通过 FFmpeg/libass 重新渲染；草稿不会自动保存。</p>
          <details id="subtitle-font-events" class="subtitle-font-events hidden"><summary>查看 libass 实际字体选择</summary><pre></pre></details>
          <p id="subtitle-style-message" role="status"></p>
        </section>
        <div class="subtitle-style-controls" aria-label="字幕样式属性">
          <div class="subtitle-style-grid">
            ${subtitleStyleEditorMarkup("source", "原文")}
            ${subtitleStyleEditorMarkup("translation", "译文")}
          </div>
        </div>
      </div>
      <aside id="subtitle-font-picker" class="subtitle-font-picker hidden" aria-label="字体候选">
        <div class="subtitle-font-picker-heading"><strong>选择本机字体</strong><span id="subtitle-font-picker-count"></span></div>
        <p id="subtitle-font-picker-sample"></p>
        <div id="subtitle-font-picker-options" class="subtitle-font-picker-options" role="listbox"></div>
      </aside>
    </dialog>`; }
