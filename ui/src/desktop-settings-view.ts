type Theme = "moonlight" | "letter";
type Options = {
  openButton: HTMLButtonElement | null;
  visit(): number;
  opening(): void; opened(): void; closed(): void;
  dirty(id: string): void;
  save(finish: boolean): void;
  applyTheme(theme: Theme): void;
};

/** Static settings structure; this view owns common navigation, form and theme events. */
export function mountDesktopSettingsView(dialog: HTMLDialogElement, options: Options) {
  const events = new AbortController(), form = dialog.querySelector<HTMLFormElement>("#settings-form")!;
  const content = form.querySelector<HTMLElement>(".settings-content")!;
  const pages = Array.from(content.querySelectorAll<HTMLElement>(":scope > [data-settings-page]"));
  const tabs = Array.from(form.querySelectorAll<HTMLButtonElement>("[data-settings-tab]"));
  const saveButtons = Array.from(form.querySelectorAll<HTMLButtonElement>('button[type="submit"], #finish-settings'));
  let disposed = false, restore: { button: HTMLButtonElement; visit: number } | null = null;
  const syncTheme = () => {
    if (disposed) return;
    for (const button of form.querySelectorAll<HTMLButtonElement>("[data-app-theme]")) button.setAttribute("aria-pressed", String(button.dataset.appTheme === document.documentElement.dataset.theme));
  };
  const selectPage = (key: string) => {
    if (disposed || !pages.some(page => page.dataset.settingsPage === key)) return;
    for (const page of pages) page.classList.toggle("hidden", page.dataset.settingsPage !== key);
    for (const tab of tabs) { const selected = tab.dataset.settingsTab === key; tab.classList.toggle("active", selected); tab.setAttribute("aria-pressed", String(selected)); }
    content.scrollTop = 0;
  };
  const open = (reload = true) => {
    if (disposed) return;
    if (reload) options.opening();
    if (!dialog.open) dialog.showModal();
    if (reload) options.opened();
  };
  options.openButton?.addEventListener("click", () => open(), { signal: events.signal });
  dialog.addEventListener("close", () => options.closed(), { signal: events.signal });
  form.addEventListener("click", event => {
    const target = event.target;
    if (!(target instanceof Element)) return;
    const button = target.closest<HTMLButtonElement>("button"); if (!button || button.disabled) return;
    if (button.id === "close-settings") dialog.close();
    else if (button.id === "finish-settings") options.save(true);
    else if (button.dataset.settingsTab) selectPage(button.dataset.settingsTab);
    else if (button.dataset.appTheme === "moonlight" || button.dataset.appTheme === "letter") { options.applyTheme(button.dataset.appTheme); syncTheme(); }
  }, { signal: events.signal });
  const dirty = (event: Event) => {
    const target = event.target;
    if (target instanceof HTMLInputElement || target instanceof HTMLSelectElement) options.dirty(target.id);
  };
  form.addEventListener("input", dirty, { signal: events.signal });
  form.addEventListener("change", dirty, { signal: events.signal });
  form.addEventListener("submit", event => { event.preventDefault(); options.save(false); }, { signal: events.signal });
  window.addEventListener("atogaki-theme-change", syncTheme, { signal: events.signal });
  syncTheme();
  return {
    open,
    selectPage,
    onboarding(value: boolean) {
      if (disposed) return;
      dialog.classList.toggle("onboarding", value);
      const title = dialog.querySelector("h2"); if (title) title.textContent = value ? "首次配置" : "设置";
    },
    saving(busy: boolean) {
      if (disposed) return;
      if (busy) { const button = saveButtons.find(item => item === document.activeElement); restore = button ? { button, visit: options.visit() } : null; }
      for (const button of saveButtons) button.disabled = busy;
      if (!busy) {
        const focus = restore; restore = null;
        if (focus && focus.visit === options.visit() && dialog.open && focus.button.getClientRects().length && !focus.button.closest("[inert]") && document.activeElement === document.body) focus.button.focus();
      }
    },
    dispose() { disposed = true; events.abort(); restore = null; if (dialog.open) dialog.close(); },
  };
}
