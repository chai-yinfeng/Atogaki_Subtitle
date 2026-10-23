export function videoRenderTemplate(platform: string): string { return `<dialog id="video-render-dialog" class="video-render-dialog">
      <div class="dialog-heading">
        <div><p class="eyebrow">BURN SUBTITLES</p><h2>导出带字幕视频</h2></div>
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
        <label>导出质量
          <select id="video-render-quality">
            <option value="balanced">均衡（推荐，源码率约 120%）</option>
            <option value="compact">省空间（源码率约 75%）</option>
            <option value="high">高质量（源码率约 180%）</option>
          </select>
        </label>
        <div id="video-render-estimate" class="video-render-estimate">正在读取源视频规格…</div>
        <label>输出视频
          <input id="video-output-path" placeholder="选择 MP4 保存位置，或粘贴完整路径" />
        </label>
        <button id="choose-video-output" type="button" class="secondary">选择位置</button>
        <p>${platform === "macos" ? "优先使用 VideoToolbox，失败时回退" : "使用"}内置 LGPL MPEG-4 软件编码。三档质量保持原始分辨率，明确调整目标视频码率；预计大小包含音频和约 2% 封装余量，实际结果会随画面复杂度与编码器浮动。</p>
      </div>
      <div class="video-render-footer">
        <span id="video-render-message" role="status"></span>
        <button id="submit-video-render" type="button">开始烧录</button>
      </div>
      <section class="video-render-history" aria-labelledby="video-render-history-title">
        <div class="section-heading"><h2 id="video-render-history-title">本任务烧录记录</h2><span id="video-render-count"></span></div>
        <div id="video-render-list" class="video-render-list"></div>
      </section>
    </dialog>`; }
