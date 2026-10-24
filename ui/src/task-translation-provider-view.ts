import type { createTaskTranslationProvider } from "./task-translation-provider";

export function mountTaskProviderView(root: HTMLElement, session: ReturnType<typeof createTaskTranslationProvider>) {
  const select = root.querySelector<HTMLSelectElement>("#task-provider-choice")!;
  const apply = root.querySelector<HTMLButtonElement>("[data-apply-task-provider]")!;
  const notice = root.querySelector<HTMLElement>("[data-task-provider-message]")!;
  const events = new AbortController();
  let wasBusy = false, restoreFocus = false;
  let visitKey = "";
  select.addEventListener("change", () => session.choose(select.value), { signal: events.signal });
  apply.addEventListener("click", () => void session.apply(), { signal: events.signal });
  const render = () => {
    const state = session.state();
    if (visitKey !== state.visitKey) { notice.textContent = ""; notice.classList.remove("warning"); visitKey = state.visitKey; }
    if (state.busy && !wasBusy) restoreFocus = document.activeElement === apply;
    if (select.value !== state.selected) select.value = state.selected;
    apply.disabled = !state.canApply;
    apply.textContent = state.busy ? "正在设置…" : "设为本任务后端";
    if (wasBusy && !state.busy && restoreFocus && state.canApply && root.isConnected && apply.getClientRects().length && !apply.closest("[inert]")) {
      if (document.activeElement === document.body) apply.focus();
    }
    wasBusy = state.busy;
    if (!state.busy) restoreFocus = false;
  };
  render();
  return {
    render,
    message(text: string, error = false) { visitKey = session.state().visitKey; notice.textContent = text; notice.classList.toggle("warning", error); },
    dispose() { events.abort(); },
  };
}
