import { translationProviderTemplate } from "./translation-provider-template";

export function desktopSettingsTemplate(): string {
  return `    <dialog id="settings-dialog" class="settings-dialog">
      <form id="settings-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">SETTINGS</p><h2>设置</h2></div>
          <button id="close-settings" type="button" class="secondary">稍后再说</button>
        </div>
        <p class="dialog-help">媒体和模型默认保留在本机；选择云端翻译 provider 时，只有原文字幕和上下文会发送到对应服务。</p>
<nav class="settings-tabs" aria-label="设置分区"><button type="button" class="secondary" data-settings-tab="appearance" aria-pressed="false">外观</button><button type="button" class="secondary active" data-settings-tab="transcription" aria-pressed="true">转录</button><button type="button" class="secondary" data-settings-tab="translation" aria-pressed="false">翻译</button><button type="button" class="secondary" data-settings-tab="models" aria-pressed="false">模型与存储</button><button type="button" class="secondary" data-settings-tab="dictionary" aria-pressed="false">词典</button><button type="button" class="secondary" data-settings-tab="network" aria-pressed="false">网络与高级</button></nav>
<div class="settings-content">
<section data-settings-page="appearance" class="settings-section hidden appearance-settings"><h3>外观</h3><p>选择页面主题，即时生效并自动保存。字幕导出样式保持独立。</p><div class="theme-options"><button type="button" data-app-theme="moonlight"><span class="theme-swatch moonlight-swatch"></span><strong>月夜</strong><span>颜料笔触、月光与青灰纸面</span></button><button type="button" data-app-theme="letter"><span class="theme-swatch letter-swatch"></span><strong>来信</strong><span>纸纤维、折痕与信笺层次</span></button></div></section>
        <section data-settings-page="transcription" class="settings-section">
          <div class="settings-section-heading"><div><strong>本地转录</strong></div><span id="model-readiness"></span></div>
          <div class="settings-path-row">
            <label>Whisper 模型<input id="settings-whisper-model" placeholder="选择已有 ggml-*.bin，或从下方下载" /></label>
            <button id="settings-choose-whisper" type="button" class="secondary">选择文件</button>
          </div>
          <div class="settings-path-row">
            <label>Silero VAD 模型（推荐）<input id="settings-vad-model" placeholder="选择已有 ggml-silero-*.bin，或从下方下载" /></label>
            <button id="settings-choose-vad" type="button" class="secondary">选择文件</button>
          </div>

          <details class="onboarding-optional"><summary>可选：Gemini 局部修复</summary>
        <section class="settings-section">
          <div class="settings-section-heading"><div><strong>3. 云端 ASR（可选）</strong><span>Gemini Key 只存入系统凭据库。配置 Key 不会授权上传；每次运行仍需单独确认音频范围。</span></div><span id="asr-credential-store-label"></span></div>
          <p>与翻译区 Gemini 共用同一把 Key；保存或删除会同时影响两项功能。音频上传仍需逐次授权。</p><label>Gemini API Key<input id="settings-gemini-api-key" type="password" autocomplete="off" placeholder="从 Google AI Studio 创建；保存后立即清空" /></label>
          <div class="credential-check-row"><button id="save-gemini-api-key" type="button">保存 Key</button><button id="check-gemini-api-key" type="button" class="secondary">检查</button><button id="clear-gemini-api-key" type="button" class="secondary">删除</button></div>
          <p id="gemini-api-key-status" class="settings-message">尚未读取配置。</p>
          <p class="settings-message">Atogaki 上传提取后的 WAV，并在转录请求完成后调用 Files API 删除上传文件。免费层价格为 $0，但存在项目级 rate limits，且免费层数据会用于改进 Google 产品。</p>
        </section>
          </details>
        </section>
${translationProviderTemplate()}
<section data-settings-page="models" class="settings-section hidden"><h3>模型与存储</h3><p>统一下载、校验与卸载。模型按需安装。</p><span id="models-directory"></span>
          <div id="model-catalog" class="model-catalog"></div>
          <p id="model-download-message" class="settings-message" role="status"></p>
          <div class="settings-path-row">
            <label>Hy-MT2 模型<input id="settings-hy-mt2-model" placeholder="选择已有 Hy-MT2 *.gguf，或从下方下载" /></label>
            <button id="settings-choose-hy-mt2" type="button" class="secondary">选择文件</button>
          </div>
</section>
        <section data-settings-page="dictionary" class="settings-section hidden">
          <div class="settings-section-heading"><div><strong>词典</strong><span>离线包由你明确点击后下载到正式应用数据目录；商业词典的 Key 分来源保存。</span></div><span id="dictionary-directory"></span></div>
          <div>
            <strong class="settings-subheading">离线词典包</strong>
            <p class="settings-message">JMdict 与 Tomoshi 用于日语；ECDICT 基础数据用于英语。ECDICT 属于社区汇总数据，下载会复用上方代理设置并校验固定版本摘要。</p>
          </div>
          <div id="dictionary-catalog" class="model-catalog"></div>
          <p id="dictionary-download-message" class="settings-message" role="status"></p>
          <div>
            <strong class="settings-subheading">在线词典 API</strong>
            <p class="settings-message">每个来源独立配置；Key 仅保存在系统凭据库，不写入日志或任务目录。检查只验证凭据是否存在，不调用词典 API。</p>
          </div>
          <div id="dictionary-credentials" class="dictionary-credentials">
            ${[
              ["merriam-webster", "Merriam-Webster", "保留英英来源标签页；正式展示需满足 Logo 与署名要求。"],
            ].map(([id, name, hint]) => `<article class="dictionary-credential-card" data-dictionary-provider="${id}">
              <div><strong>${name}</strong><span>${hint}</span></div>
              <input type="password" autocomplete="off" data-dictionary-key="${id}" aria-label="${name} API Key" placeholder="粘贴 API Key；保存后立即清空" />
              <div class="model-card-action"><span data-dictionary-status="${id}">尚未读取配置</span><div><button type="button" class="secondary" data-check-dictionary-key="${id}">检查</button><button type="button" data-save-dictionary-key="${id}">保存</button><button type="button" class="secondary" data-clear-dictionary-key="${id}">删除</button></div></div>
            </article>`).join("")}
          </div>
          <p id="dictionary-credential-message" class="settings-message" role="status"></p>
        </section>
        <section data-settings-page="network" class="settings-section hidden">
          <div class="settings-section-heading"><div><strong>网络与高级</strong><span>代理同时用于模型下载与所选云端翻译；镜像只用于模型下载，失败会回退官方源。</span></div><span>每个模型都会校验 SHA-256</span></div>
          <div class="network-settings-grid">
            <label>代理模式
              <select id="settings-proxy-mode">
                <option value="environment">跟随启动环境</option>
                <option value="direct">直连（忽略系统与环境代理）</option>
                <option value="custom">自定义 HTTP 代理</option>
              </select>
            </label>
            <label id="proxy-url-field">HTTP 代理地址
              <input id="settings-proxy-url" inputmode="url" placeholder="http://127.0.0.1:7897" />
            </label>
            <label>Hugging Face 镜像根地址（可选）
              <input id="settings-model-mirror" inputmode="url" placeholder="https://hf-mirror.com" />
            </label>
          </div>
          <div class="network-test-row"><button id="test-network" type="button" class="secondary">测试当前网络配置</button><span>测试只检查当前输入，不会切换或保存模式；修改后请点击“保存配置”。该代理也用于云端翻译。</span></div>
          <div id="network-test-results" class="network-test-results" role="status"></div>
        </section>
</div>
        <div class="settings-footer">
          <span id="settings-message" role="status"></span>
          <div><button id="save-settings" type="submit" class="secondary">保存</button><button id="finish-settings" type="button">保存并开始使用</button></div>
        </div>
      </form>
    </dialog>`;
}
