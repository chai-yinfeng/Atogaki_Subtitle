import type { CueFilter } from "./editor-session";
type Options = {
  query(value: string): void;
  filter(value: CueFilter): void;
  action(name: string): void;
  togglePlayback(): void;
  rate(value: number): void;
  step(milliseconds: number): void;
  gain(value: number): void;
  cut(): void;
  join(): void;
  insert(): void;
  remove(): void;
  undo(): void;
  tools(): void;
};

/** Event bindings only; draft, playback and structure state remain in their services. */
export function mountEditorControls(root: HTMLElement, options: Options) {
  const events = new AbortController();
  const click = (selector: string, handler: () => void) => root.querySelector(selector)?.addEventListener("click", handler, { signal: events.signal });
  const search = root.querySelector<HTMLInputElement>("#editor-search");
  const filter = root.querySelector<HTMLSelectElement>("#editor-filter");
  search?.addEventListener("input", () => options.query(search.value), { signal: events.signal });
  filter?.addEventListener("change", () => options.filter(filter.value as CueFilter), { signal: events.signal });
  root.addEventListener("click", event => {
    const button = event.target instanceof Element ? event.target.closest<HTMLElement>("[data-editor-action]") : null;
    if (button && root.contains(button)) options.action(button.dataset.editorAction!);
  }, { signal: events.signal });
  click("#karaoke-toggle-playback", options.togglePlayback);
  click("#karaoke-cut-segment", options.cut);
  click("#karaoke-join-segment", options.join);
  click("#karaoke-insert-block", options.insert);
  click("#karaoke-delete-block", options.remove);
  click("#undo-subtitle-structure", options.undo);
  click("#karaoke-open-workspace", options.tools);
  for (const [selector, milliseconds] of [
    ["#karaoke-step-back-small", -10], ["#karaoke-step-back", -100],
    ["#karaoke-step-forward", 100], ["#karaoke-step-forward-small", 10],
  ] as const) click(selector, () => options.step(milliseconds));
  const rate = root.querySelector<HTMLSelectElement>("#karaoke-playback-rate");
  rate?.addEventListener("change", () => options.rate(Number(rate.value)), { signal: events.signal });
  const gain = root.querySelector<HTMLInputElement>("#karaoke-waveform-gain");
  gain?.addEventListener("input", () => {
    const value = Number(gain.value), label = root.querySelector<HTMLOutputElement>("#karaoke-waveform-gain-label");
    if (label) label.value = `${value.toFixed(value % 1 ? 2 : 0)}×`;
    options.gain(value);
  }, { signal: events.signal });
  return { dispose() { events.abort(); } };
}
