export function taskToolsTemplate(fileManagerLabel: string): string {
  return `    <section id="workspace-view" class="workspace hidden" aria-labelledby="workspace-title">
      <div class="workspace-heading">
        <button id="back-to-jobs" class="secondary" type="button">← 返回任务</button>
        <div>
          <p class="eyebrow">TASK WORKSPACE</p>
          <h2 id="workspace-title">任务进度</h2>
          <p id="workspace-message" class="workspace-message"></p>
        </div>
        <button id="reload-detail" type="button" class="secondary">重新读取</button>
      </div>
      <nav class="workspace-sections" role="tablist" aria-label="任务工具">
        <button id="workspace-tab-translation" class="workspace-section-tab active" type="button" role="tab" aria-selected="true" aria-controls="workspace-translation-panel" data-workspace-section="translation">
          <strong>翻译与词表</strong><span>批量翻译与识别修正</span>
        </button>
        <button id="workspace-tab-review" class="workspace-section-tab" type="button" role="tab" aria-selected="false" aria-controls="workspace-review-panel" data-workspace-section="review">
          <strong>任务进度</strong><span>识别进度与只读预览</span>
        </button>
        <button id="workspace-tab-export" class="workspace-section-tab" type="button" role="tab" aria-selected="false" aria-controls="workspace-export-panel" data-workspace-section="export">
          <strong>导出成品</strong><span>字幕文件与烧录视频</span>
        </button>
      </nav>
      <p id="workspace-action-message" class="workspace-action-message" role="status"></p>
      <section id="workspace-review-panel" class="workspace-panel hidden" role="tabpanel" aria-labelledby="workspace-tab-review" data-workspace-panel="review">
        <div class="module-heading compact">
          <div><p class="eyebrow">SUBTITLE REVIEW</p><h2>任务进度</h2><p>识别完成后直接进入字幕编辑。</p></div>
          <button id="open-subtitle-editor" type="button">进入字幕编辑 →</button>
        </div>
        <div class="review-grid">
          <section class="media-panel" aria-label="媒体播放器">
            <div class="panel-heading compact"><div><p class="eyebrow">PLAYBACK</p><h3>媒体与当前字幕</h3></div></div>
          <div id="media-host" class="media-host"></div>
          <div class="playback-tools" aria-label="播放快捷操作">
            <button id="previous-subtitle" type="button" class="secondary" title="上一句（[）">上一句</button>
            <button id="rewind-media" type="button" class="secondary" title="后退 5 秒（← 或 J）">−5 秒</button>
            <button id="toggle-playback" type="button" title="播放／暂停（空格或 K）">播放</button>
            <button id="forward-media" type="button" class="secondary" title="前进 5 秒（→ 或 L）">+5 秒</button>
            <button id="next-subtitle" type="button" class="secondary" title="下一句（]）">下一句</button>
            <label>速度
              <select id="playback-rate" aria-label="播放速度">
                <option value="0.5">0.5×</option>
                <option value="0.75">0.75×</option>
                <option value="1" selected>1×</option>
                <option value="1.25">1.25×</option>
                <option value="1.5">1.5×</option>
                <option value="2">2×</option>
              </select>
            </label>
          </div>
          <p class="shortcut-help">快捷键：空格/K 播放暂停 · ←/J 与 →/L 跳 5 秒 · [ ] 切换字幕 · , . 调整速度 · O 开关悬浮字幕</p>
          <p id="media-message" class="media-message"></p>
          <div class="current-caption" aria-live="polite">
            <p id="current-source">播放时将在这里显示当前原文。</p>
            <p id="current-translation">简体中文翻译</p>
          </div>
          </section>
          <section class="timeline" aria-labelledby="timeline-title">
            <div class="section-heading">
              <h2 id="timeline-title">任务进度</h2>
              <span id="segment-count"></span>
            </div>
            <div id="subtitle-list" class="subtitle-list"></div>
          </section>
        </div>
      </section>
      <section id="workspace-translation-panel" class="workspace-panel" role="tabpanel" aria-labelledby="workspace-tab-translation" data-workspace-panel="translation">
        <div class="offline-tools" data-task-provider>
          <label>本任务后续翻译<select id="task-provider-choice"><option value="none">仅原文</option><option value="deepseek">DeepSeek</option><option value="gemini">Gemini</option><option value="deepl">DeepL</option><option value="hy-mt2-local">Hy-MT2 本地翻译</option><option value="openai-compatible">高级兼容服务</option></select></label>
          <button type="button" class="secondary" data-apply-task-provider>设为本任务后端</button><span>更改只影响下一次翻译，不自动重译已有字幕。</span>
          <span data-task-provider-message role="status"></span>
        </div>
        <div class="module-heading">
          <div><p class="eyebrow">LANGUAGE WORKFLOW</p><h2>翻译与识别修正</h2><p>这里的操作会更新当前字幕工作区；原始识别快照保持不变。</p></div>
        </div>
        <section class="workspace-toolbar module-card" aria-label="翻译与词表">
          <div>
            <strong>简体中文翻译</strong>
            <span id="translation-status">正在读取翻译配置…</span>
            <span id="translation-run-status" class="muted"></span>
          </div>
          <div class="workspace-action-buttons">
            <button id="translate-pending" type="button">补译缺失／过期段</button>
            <button id="translate-all" class="secondary" type="button">全体翻译／重译</button>
          </div>
          <div class="workspace-glossary-row">
            <div>
              <strong>识别词表修正</strong>
              <span id="job-glossary-status">当前任务未记录识别词表</span>
            </div>
            <select id="workspace-glossary"><option value="">选择词表…</option></select>
            <button id="preview-glossary" type="button" class="secondary">预览应用</button>
            <button id="manage-workspace-glossary" type="button" class="secondary">管理完整词表</button>
          </div>
          <div class="workspace-glossary-inspection">
            <section>
              <strong>可应用的修正映射</strong>
              <p>这里只列出会修改当前原文的规则；纯提示词不会改写已有字幕。</p>
              <div id="workspace-glossary-mappings" class="glossary-mapping-list muted">请选择词表。</div>
            </section>
            <details>
              <summary>查看转写时冻结的词表快照</summary>
              <p>该快照只用于说明本任务当时的识别输入，不会随当前词表修改而变化。</p>
              <pre id="job-glossary-snapshot">当前任务没有词表快照。</pre>
            </details>
          </div>
          <div id="glossary-preview" class="glossary-preview hidden"></div>
        </section>
      </section>
      <section id="workspace-export-panel" class="workspace-panel hidden" role="tabpanel" aria-labelledby="workspace-tab-export" data-workspace-panel="export">
        <div class="module-heading">
          <div><p class="eyebrow">DELIVERABLES</p><h2>导出成品</h2><p>每次导出都读取当前已保存的字幕；未保存草稿不会进入文件或视频。</p></div>
        </div>
        <div class="export-grid">
          <article class="export-card">
            <div><span class="module-number">01</span><h3>字体、样式与预览</h3><p>设置任务级原文／译文样式，并用最终烧录所用的 FFmpeg/libass 生成真实预览帧。</p></div>
            <div class="export-card-actions"><button id="edit-subtitle-styles" type="button">设置并预览…</button></div>
          </article>
          <article class="export-card">
            <div><span class="module-number">02</span><h3>字幕文件</h3><p>推荐搭配原视频播放，保留原视频画质。按需要导出原文、译文或双语字幕；SRT 保留文字和时间，ASS 还保留字幕样式。</p></div>
            <div class="export-card-actions"><button id="export-subtitles" type="button">导出字幕…</button><button id="reveal-export" type="button" class="secondary hidden">在 ${fileManagerLabel} 中显示</button></div>
          </article>
          <article class="export-card">
            <div><span class="module-number">03</span><h3>带字幕视频</h3><p>需要将字幕固定到画面时使用；重新编码可能损失画质。烧录使用提交时已保存的字幕和样式，提交后可继续编辑。</p></div>
            <div class="export-card-actions"><button id="render-video" type="button">导出带字幕视频…</button></div>
          </article>
        </div>
      </section>
    </section>
`;
}
