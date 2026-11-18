import { translationProviderTemplate } from "./translation-provider-template";

export function desktopSettingsTemplate(): string {
  return `    <dialog id="settings-dialog" class="settings-dialog">
      <form id="settings-form">
        <div class="dialog-heading">
          <div><p class="eyebrow">SETTINGS</p><h2>设置</h2></div>
          <button id="close-settings" type="button" class="secondary">关闭</button>
        </div>
<nav class="settings-tabs" aria-label="设置分区"><button type="button" class="secondary" data-settings-tab="appearance" aria-pressed="false">外观</button><button type="button" class="secondary active" data-settings-tab="transcription" aria-pressed="true">识别</button><button type="button" class="secondary" data-settings-tab="translation" aria-pressed="false">翻译</button><button type="button" class="secondary" data-settings-tab="models" aria-pressed="false">存储</button><button type="button" class="secondary" data-settings-tab="dictionary" aria-pressed="false">词典</button><button type="button" class="secondary" data-settings-tab="network" aria-pressed="false">网络与高级</button><button type="button" class="secondary" data-settings-tab="updates" aria-pressed="false">更新</button></nav>
<div class="settings-content">
<section data-settings-page="appearance" class="settings-section hidden appearance-settings"><h3>外观</h3><div class="theme-options"><button type="button" data-app-theme="light"><span class="theme-swatch light-swatch"></span><strong>亮色</strong></button><button type="button" data-app-theme="dark"><span class="theme-swatch dark-swatch"></span><strong>暗色</strong></button><button type="button" data-app-theme="system"><span class="theme-swatch system-swatch"></span><strong>跟随系统</strong></button></div></section>
        <section data-settings-page="transcription" class="settings-section">
          <div class="settings-section-heading"><div><strong>本地转录</strong></div><span id="model-readiness"></span></div>
          <input id="settings-whisper-model" type="hidden" />
          <input id="settings-vad-model" type="hidden" />
          <section id="custom-models" class="custom-models"></section>
          <div id="model-catalog" class="model-catalog"></div>
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
<section data-settings-page="models" class="settings-section hidden"><h3>存储</h3><dl class="settings-storage-paths"><div><dt>应用数据与任务</dt><dd id="data-path">正在读取…</dd></div><div><dt>下载的模型</dt><dd id="models-directory">正在读取…</dd></div><div><dt>离线词典</dt><dd id="dictionary-directory">正在读取…</dd></div></dl></section>
        <section data-settings-page="dictionary" class="settings-section hidden">
          <div class="settings-section-heading"><div><strong>词典</strong><span>离线词典按需下载；目录可在「存储」查看。</span></div></div>
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
              <input id="dictionary-key-${id}" type="password" autocomplete="off" data-dictionary-key="${id}" aria-label="${name} API Key" placeholder="粘贴 API Key；保存后立即清空" />
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
<section data-settings-page="updates" class="settings-section hidden"><h3>App 更新</h3><div class="actions"><button id="check-app-update" type="button" class="secondary">检查更新</button><button id="install-app-update" type="button" hidden>下载并重启</button></div><p id="app-update-status" class="settings-message" role="status" aria-live="polite">从 GitHub 检查正式版本。</p></section>
</div>
        <div class="settings-footer">
          <div class="settings-status"><span id="settings-message" role="status"></span><span id="model-download-message" class="settings-message" role="status"></span></div>
          <div><button id="save-settings" type="submit" class="secondary">保存</button><button id="finish-settings" type="button">保存并开始使用</button></div>
        </div>
      </form>
    </dialog>`;
}
