import type { ModelCatalogItem, ModelDownloadState } from "./desktop-settings-types.ts";

type Action = "test" | "check" | "save" | "delete";
type Options = {
  visit(): number;
  state(): { readiness: ReadonlyMap<string, string>; dirty: { has(id: string): boolean }; catalog: ModelCatalogItem[]; downloads: ModelDownloadState[] };
  localModel(): string;
  chooseLocalModel(path: string): void;
  action(action: Action, provider: string, current: () => boolean): Promise<void>;
  error(text: string): void;
};

export function translationModelChoices(catalog: ModelCatalogItem[], downloads: ModelDownloadState[], current: string) {
  const names = new Map(catalog.filter(item => item.kind === "hy-mt2").map(item => [item.id, item.name]));
  const entries: { path: string; name: string }[] = [];
  for (const item of downloads) {
    if (item.status === "done" && item.path && names.has(item.modelId) && !entries.some(entry => entry.path === item.path)) entries.push({ path: item.path, name: names.get(item.modelId)! });
  }
  if (current && !entries.some(item => item.path === current)) entries.unshift({ path: current, name: current.split(/[\\/]/).pop() || "当前模型" });
  return entries;
}

/** One shared editor, static cards and abortable delegated events. No credential store. */
export function mountTranslationProviderView(root: HTMLElement, options: Options) {
  const select = root.querySelector<HTMLSelectElement>("#settings-provider")!;
  const editor = root.querySelector<HTMLElement>(".provider-editor")!;
  const home = root.querySelector<HTMLElement>("[data-provider-editor-home]")!;
  const modelChoice = root.querySelector<HTMLSelectElement>("#local-translation-model-choice")!;
  const events = new AbortController(), pending = new Set<string>();
  let disposed = false, choicesKey = "";
  const labels: Record<string, string> = {
    disabled: "仅转录", unconfigured: "未配置凭据", resources_missing: "本地资源缺失",
    unverified: "凭据已配置，尚未验证连接", verified: "上次连接测试通过", validation_failed: "上次连接验证失败", configuration_missing: "配置不完整，请保存模型与端点",
  };
  const render = () => {
    if (disposed) return;
    const state = options.state(), selected = select.value, cloud = selected !== "none" && selected !== "hy-mt2-local";
    for (const card of root.querySelectorAll<HTMLElement>("[data-provider-card]")) {
      const id = card.dataset.providerCard!;
      card.classList.toggle("active", selected === id);
      const status = card.querySelector<HTMLElement>("[data-provider-readiness]")!;
      const readiness = state.readiness.get(id) ?? "";
      const text = (id === "hy-mt2-local" && readiness === "unverified" ? "本地资源就绪，尚未验证" : labels[readiness] ?? "正在读取配置状态…") + (state.dirty.has(id) ? "（当前草稿未保存）" : "");
      if (status.textContent !== text) status.textContent = text;
      if (id === selected && cloud) {
        const body = card.querySelector<HTMLElement>("[data-provider-editor]")!;
        if (editor.parentElement !== body) body.append(editor);
        const advanced = card.closest("details"); if (advanced) advanced.open = true;
      }
    }
    editor.classList.toggle("hidden", !cloud);
    root.querySelector("#api-key-field")?.classList.toggle("hidden", !cloud);
    root.querySelector("#llm-provider-fields")?.classList.toggle("hidden", !["gemini", "deepseek", "openai-compatible"].includes(selected));
    root.querySelector("#provider-base-url-field")?.classList.toggle("hidden", selected !== "openai-compatible");
    const model = root.querySelector<HTMLInputElement>("#settings-provider-model"); if (model) model.disabled = selected === "hy-mt2-local";
    const providerLabel = selected === "gemini" ? "Gemini（与语音识别共用）" : selected === "deepseek" ? "DeepSeek" : selected === "openai-compatible" ? "OpenAI-compatible" : "DeepL";
    const keyLabel = root.querySelector("#api-key-label"); if (keyLabel) keyLabel.textContent = `${providerLabel} API Key`;
    const currentModel = options.localModel();
    const entries = translationModelChoices(state.catalog, state.downloads, currentModel), key = JSON.stringify(entries);
    if (key !== choicesKey) {
      choicesKey = key;
      const items = [{ path: "", name: "选择已安装模型" }, ...entries].map(entry => {
        const option = document.createElement("option"); option.value = entry.path; option.textContent = entry.name; return option;
      });
      modelChoice.replaceChildren(...items);
    }
    if (modelChoice.value !== currentModel) modelChoice.value = currentModel;
  };
  const run = async (action: Action, id: string, button: HTMLButtonElement) => {
    const key = action === "save" || action === "delete" ? "credential" : `${action}:${id}`;
    if (disposed || pending.has(key)) return;
    const visit = options.visit(), focused = document.activeElement === button;
    const current = () => !disposed && visit === options.visit();
    const buttons = key === "credential" ? Array.from(root.querySelectorAll<HTMLButtonElement>("[data-save-provider-key], [data-delete-provider-key]")) : [button];
    pending.add(key); for (const item of buttons) item.disabled = true;
    try { await options.action(action, id, current); }
    catch (error) { if (current()) options.error(String(error)); }
    finally {
      pending.delete(key);
      if (!disposed) {
        for (const item of buttons) item.disabled = false;
        if (current() && focused && document.activeElement === document.body && button.getClientRects().length && !button.closest("[inert]")) button.focus();
      }
    }
  };
  root.addEventListener("click", event => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("button"); if (!button || button.disabled) return;
    const id = button.closest<HTMLElement>("[data-provider-card]")?.dataset.providerCard;
    if (button.hasAttribute("data-configure-provider") && id) {
      if (select.value !== id) { select.value = id; select.dispatchEvent(new Event("change", { bubbles: true })); }
      render();
    } else if (button.hasAttribute("data-test-provider") && id) void run("test", id, button);
    else if (button.hasAttribute("data-check-provider") && id) void run("check", id, button);
    else if ((button.hasAttribute("data-save-provider-key") || button.hasAttribute("data-delete-provider-key")) && select.value !== "none" && select.value !== "hy-mt2-local") void run(button.hasAttribute("data-delete-provider-key") ? "delete" : "save", select.value, button);
  }, { signal: events.signal });
  modelChoice.addEventListener("change", () => {
    options.chooseLocalModel(modelChoice.value);
  }, { signal: events.signal });
  render();
  return { render, dispose() { disposed = true; events.abort(); pending.clear(); home.append(editor); } };
}
