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
        <label>视频码率档位
          <select id="video-render-quality">
            <option value="balanced">标准码率</option>
            <option value="compact">较低码率（文件较小）</option>
            <option value="high">较高码率（文件较大）</option>
          </select>
        </label>
        <div id="video-render-estimate" class="video-render-estimate">正在读取源视频规格…</div>
        <label>输出视频
          <input id="video-output-path" placeholder="选择 MP4 保存位置，或粘贴完整路径" />
        </label>
        <button id="choose-video-output" type="button" class="secondary">选择位置</button>
        <p>烧录会重新编码视频，可能进一步损失画质；提高码率也无法恢复源视频已丢失的细节。建议优先导出 SRT/ASS，搭配原视频播放。</p>
        <p>${platform === "macos" ? "优先使用 VideoToolbox，失败时回退" : "使用"}内置 LGPL MPEG-4 软件编码。保持原分辨率；源视频码率可用时，三档目标约为其 75%／120%／180%，否则按分辨率估算。码率不保证画质，预计大小包含音频与封装余量。</p>
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
