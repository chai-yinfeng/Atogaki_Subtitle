export function videoRenderTemplate(platform: string): string { return `<dialog id="video-render-dialog" class="video-render-dialog">
      <div class="dialog-heading">
        <h2 id="video-render-title">烧录导出</h2>
        <button id="close-video-render" type="button" class="secondary">关闭</button>
      </div>
      <div id="media-capabilities" class="media-capabilities">正在检查内置 FFmpeg sidecar…</div>
      <div class="video-render-form">
        <label>字幕内容
          <select id="video-subtitle-track">
            <option value="bilingual">原文＋译文（推荐）</option>
            <option value="translation">仅译文</option>
            <option value="source">仅原文</option>
          </select>
        </label>
        <label id="video-render-format">输出格式
          <select id="video-render-quality">
            ${platform === "macos" ? '<option value="quality" selected>直接导出 · 画质优先 MP4</option>' : ''}
            <option value="prores">后期压制用中间文件 · ProRes 422 HQ MOV</option>
            <optgroup label="直接导出 · 按码率的 MP4">
              <option value="balanced"${platform !== "macos" ? " selected" : ""}>标准码率</option>
              <option value="compact">较低码率</option>
              <option value="high">较高码率</option>
            </optgroup>
          </select>
        </label>
        <div id="video-render-estimate" class="video-render-estimate" role="status">正在读取源视频规格…</div>
        <label>输出视频
          <input id="video-output-path" />
        </label>
        <button id="choose-video-output" type="button" class="secondary">选择位置</button>
      </div>
      <div class="video-render-footer">
        <span id="video-render-message" role="status"></span>
        <button id="submit-video-render" type="button">开始导出</button>
      </div>
      <section class="video-render-history" aria-labelledby="video-render-history-title">
        <div class="section-heading"><h2 id="video-render-history-title">本任务视频导出记录</h2><span id="video-render-count"></span></div>
        <div id="video-render-list" class="video-render-list"></div>
      </section>
    </dialog>`; }
