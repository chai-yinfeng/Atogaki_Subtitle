type Options = {
  useCurrent(): void;
  useWindow(): void;
  expand(): void;
  preview(): void;
  closeCandidate(): void;
  confirm(): void;
  adopting(): boolean;
};

/** Range/provider presentation and bindings only; processing owns requests and adoption. */
export function mountEditorRepairControls(repair: HTMLElement, candidate: HTMLDialogElement, options: Options) {
  const events = new AbortController();
  const click = (root: HTMLElement, selector: string, handler: () => void) => root.querySelector(selector)?.addEventListener("click", handler, { signal: events.signal });
  click(repair, "#retranscription-use-current", options.useCurrent);
  click(repair, "#retranscription-use-window", options.useWindow);
  click(repair, "#retranscription-expand-segments", options.expand);
  click(repair, "#preview-retranscription", options.preview);
  click(candidate, "#close-retranscription-preview", options.closeCandidate);
  click(candidate, "#cancel-retranscription", options.closeCandidate);
  click(candidate, "#confirm-retranscription", options.confirm);
  const provider = repair.querySelector<HTMLSelectElement>("#retranscription-provider");
  const authorization = repair.querySelector<HTMLInputElement>("#authorize-gemini-upload");
  provider?.addEventListener("change", () => {
    const cloud = provider.value === "gemini-transcribe";
    repair.querySelector<HTMLElement>("#gemini-upload-consent")?.classList.toggle("hidden", !cloud);
    if (!cloud && authorization) authorization.checked = false;
  }, { signal: events.signal });
  for (const selector of ["#retranscription-start", "#retranscription-end"]) {
    repair.querySelector(selector)?.addEventListener("input", () => {
      if (authorization) authorization.checked = false;
    }, { signal: events.signal });
  }
  candidate.addEventListener("cancel", event => {
    if (options.adopting()) event.preventDefault();
  }, { signal: events.signal });
  return { dispose() { events.abort(); } };
}
