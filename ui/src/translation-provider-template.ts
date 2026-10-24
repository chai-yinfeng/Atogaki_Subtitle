const entries = [
  ["deepseek", "DeepSeek", "云端文本翻译；只发送字幕与上下文。"],
  ["gemini", "Gemini", "只发送字幕文本和上下文；与 Gemini 语音识别共用 Key。保存或删除 Key 会同时影响两处，音频上传仍需单独授权。"],
  ["deepl", "DeepL", "云端文本翻译；使用独立的 DeepL 凭据。"],
  ["hy-mt2-local", "Hy-MT2 · 本地翻译", "离线翻译，无需上传字幕。1.8B 占用较少，7B 需要更多内存和处理时间。"],
  ["openai-compatible", "OpenAI-compatible", "高级：自定义端点、模型与独立凭据。"],
] as const;

export function translationProviderTemplate(): string {
  const cards = entries.map(([id, title, description]) => {
    const card = `<article class="provider-card" data-provider-card="${id}"><h4>${title}</h4><p>${description}</p><p data-provider-readiness role="status"></p><button type="button" class="secondary" data-configure-provider>配置此后端</button><button type="button" class="secondary ${id === "hy-mt2-local" ? "hidden" : ""}" data-test-provider>测试已保存配置（发送 Hello.，可能计费）</button>${id === "hy-mt2-local" ? '<label class="local-model-choice">翻译模型<select id="local-translation-model-choice"></select></label><p>模型详情与下载位于“模型与存储”。</p>' : '<button type="button" class="secondary" data-check-provider>检查凭据是否存在</button>'}<div data-provider-editor></div></article>`;
    return id === "openai-compatible" ? `<details><summary>高级：自定义兼容服务</summary>${card}</details>` : card;
  }).join("");
  return `<section class="settings-section hidden" data-settings-page="translation" data-translation-settings>
    <div class="settings-section-heading"><div><strong>翻译</strong><span>Hy-MT2 完全离线运行；云端 provider 只发送原文字幕和上下文。</span></div><span id="credential-store-label"></span></div>
          <label>下次新任务的默认翻译方式（保存后生效）
            <select id="settings-provider">
              <option value="hy-mt2-local">Hy-MT2（本地翻译）</option>
              <option value="deepl">DeepL（传统翻译 API）</option>
              <option value="deepseek">DeepSeek（LLM API）</option><option value="gemini">Gemini（LLM API）</option>
              <option value="openai-compatible">OpenAI-compatible（高级）</option>
              <option value="none">不翻译／仅保留原文</option>
            </select>
          </label>

    <div class="provider-cards">${cards}</div>
    <div class="hidden" data-provider-editor-home><div class="provider-editor hidden">
          <div id="llm-provider-fields" class="settings-grid hidden">
            <label id="provider-base-url-field">OpenAI-compatible Base URL
              <input id="settings-provider-base-url" type="url" placeholder="https://api.openai.com/v1" />
            </label>
            <label id="provider-model-field">模型
              <input id="settings-provider-model" type="text" placeholder="模型 ID" />
            </label>
            <label class="settings-span">翻译风格
              <input id="settings-translation-style" type="text" placeholder="准确、自然的简体中文口语字幕" />
            </label>
          </div>
          <label id="api-key-field"><span id="api-key-label">DeepL API Key</span>
            <input id="settings-api-key" type="password" autocomplete="off" placeholder="留空则保持当前密钥" />
          </label>
          <p id="api-key-status" class="settings-message"></p>
      <div class="offline-tools"><button type="button" data-save-provider-key>保存此后端凭据</button><button type="button" class="secondary" data-delete-provider-key>删除此后端凭据</button></div>
    </div></div>
  </section>`;
}
