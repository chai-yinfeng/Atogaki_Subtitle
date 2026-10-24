import type { DesktopSettings } from "./desktop-settings-types.ts";
import type { createTranslationProviderDrafts } from "./translation-provider-drafts.ts";

const unchecked = "尚未检查当前 provider；可点击卡片的“检查凭据是否存在”确认。";
type Options = { dirty(id: string): boolean; clearSecretDirty(id: string): void; changed(): void };

/** Shared field events and presentation; all non-secret data stays in the draft registry. */
export function mountTranslationProviderFields(root: HTMLElement, drafts: ReturnType<typeof createTranslationProviderDrafts>, options: Options) {
  const select = root.querySelector<HTMLSelectElement>("#settings-provider")!;
  const model = root.querySelector<HTMLInputElement>("#settings-provider-model")!;
  const endpoint = root.querySelector<HTMLInputElement>("#settings-provider-base-url")!;
  const style = root.querySelector<HTMLInputElement>("#settings-translation-style")!;
  const secret = root.querySelector<HTMLInputElement>("#settings-api-key")!;
  const status = root.querySelector<HTMLElement>("#api-key-status")!;
  const events = new AbortController();
  let disposed = false;
  drafts.select(select.value);
  const render = () => {
    if (disposed) return;
    const value = drafts.current();
    if (select.value !== drafts.selected) select.value = drafts.selected;
    for (const [field, text] of [[model, value.model], [endpoint, value.endpoint], [style, value.style]] as const) if (field.value !== text) field.value = text;
  };
  const choose = (id: string) => {
    if (id !== drafts.selected) { secret.value = ""; options.clearSecretDirty(secret.id); }
    drafts.select(id); render();
  };
  select.addEventListener("change", () => {
    choose(select.value);
    // An explicit provider selection always discards the previous secret intent.
    secret.value = ""; options.clearSecretDirty(secret.id);
    if (select.value !== "none") { status.textContent = unchecked; status.classList.remove("warning"); }
    options.changed();
  }, { signal: events.signal });
  for (const input of [model, endpoint, style]) input.addEventListener("input", () => {
    drafts.edit({ model: model.value, endpoint: endpoint.value, style: style.value }); options.changed();
  }, { signal: events.signal });
  return {
    render,
    load(settings: DesktopSettings) {
      if (disposed) return;
      drafts.fallback(settings.translationProviderId, { model: settings.translationModel ?? "", endpoint: settings.translationBaseUrl ?? "", style: settings.translationStyleInstruction });
      // A delayed default-provider read must not retarget unsaved parameters or a key.
      if (!options.dirty(select.id) && !drafts.dirty.has(drafts.selected) && !secret.value) choose(settings.translationProviderId);
      render();
    },
    unchecked() { if (!disposed) { status.textContent = unchecked; status.classList.remove("warning"); } },
    dispose() { disposed = true; events.abort(); },
  };
}
