import type { SubtitleSegment } from "./editor-types";
import type { SubtitleStyle, SubtitleStyleSet, SubtitleFontFamily, SubtitleFontCoverage, SubtitleFontReport, SubtitleStyleState, SubtitleStylePreview } from "./subtitle-style-types";
import { subtitleStyleTemplate } from "./subtitle-style-template";
import { mountToolDialog } from "./task-tools";
type Actions = { open(): Promise<void>; close(): void; changed(delay?: number): void; preview(): Promise<void>; save(): Promise<void> };
/** Own one set of style controls and its global listeners. The session owns IPC. */
export function mountSubtitleStyleView(root: HTMLElement, options: {
  context(): { segments: readonly SubtitleSegment[]; sourceLanguage: string } | null;
  fileUrl(path: string): string; time(ms: number): string;
}) {
  const holder = document.createElement("template"); holder.innerHTML = subtitleStyleTemplate();
  const dialog = holder.content.firstElementChild as HTMLDialogElement; root.append(dialog);
  const events = new AbortController();
  const on = <K extends keyof GlobalEventHandlersEventMap>(target: EventTarget | null | undefined, type: K, callback: (event: GlobalEventHandlersEventMap[K]) => void) => target?.addEventListener(type, callback as EventListener, { signal: events.signal });
  let actions: Actions = { open: async () => {}, close: () => {}, changed: () => {}, preview: async () => {}, save: async () => {} };
  let subtitleFonts: SubtitleFontFamily[] | null = null;
  let activeSubtitleFontTrack: "source" | "translation" | null = null;
  const boundary = mountToolDialog(dialog, () => { closeSubtitleFontPicker(); actions.close(); });
  const subtitleStyleDialog = dialog;
  const subtitleFontPicker = dialog.querySelector<HTMLElement>("#subtitle-font-picker");
  const subtitleFontPickerCount = dialog.querySelector<HTMLSpanElement>("#subtitle-font-picker-count");
  const subtitleFontPickerSample = dialog.querySelector<HTMLParagraphElement>("#subtitle-font-picker-sample");
  const subtitleFontPickerOptions = dialog.querySelector<HTMLDivElement>("#subtitle-font-picker-options");
  const subtitleStylePreviewTrack = dialog.querySelector<HTMLSelectElement>("#subtitle-style-preview-track");
  const subtitleStylePreviewHost = dialog.querySelector<HTMLDivElement>("#subtitle-style-preview-host");
  const subtitleFontEvents = dialog.querySelector<HTMLDetailsElement>("#subtitle-font-events");
  const subtitleStyleMessage = dialog.querySelector<HTMLParagraphElement>("#subtitle-style-message");
  const previewSubtitleStyleButton = dialog.querySelector<HTMLButtonElement>("#preview-subtitle-style");
  const saveSubtitleStyleButton = dialog.querySelector<HTMLButtonElement>("#save-subtitle-style");

  function subtitleStyleEditor(track: "source" | "translation"): HTMLElement {
    const editor = subtitleStyleDialog?.querySelector<HTMLElement>(`[data-style-editor="${track}"]`);
    if (!editor) throw new Error(`missing ${track} subtitle style editor`);
    return editor;
  }

  function subtitleStyleField<T extends HTMLInputElement | HTMLSelectElement>(
    track: "source" | "translation",
    field: string,
  ): T {
    const input = subtitleStyleEditor(track).querySelector<T>(`[data-style-field="${field}"]`);
    if (!input) throw new Error(`missing ${track} subtitle style field ${field}`);
    return input;
  }

  function subtitleFontSampleText(track: "source" | "translation"): string {
    const text = options.context()?.segments
      .map((segment) => track === "source" ? segment.source_text : segment.translated_text ?? "")
      .find((candidate) => candidate.trim().length > 0)
      ?.replace(/\s+/g, " ")
      .trim();
    const sample = text ? Array.from(text).slice(0, 22).join("") : "字幕样张";
    return `${sample} · Aa 123`;
  }

  function subtitleFontPriorityFamilies(track: "source" | "translation"): string[] {
    if (track === "translation") {
      return ["Hiragino Sans GB", "PingFang SC", "Songti SC", "Kaiti SC", "STHeiti", "Arial Unicode MS"];
    }
    switch (options.context()?.sourceLanguage) {
      case "ja":
        return ["Hiragino Sans", "Hiragino Mincho ProN", "Yu Gothic", "Yu Mincho"];
      case "ko":
        return ["Apple SD Gothic Neo", "Nanum Gothic", "Nanum Myeongjo"];
      default:
        return ["Helvetica Neue", "Arial", "Avenir Next", "Georgia", "Times New Roman"];
    }
  }

  function closeSubtitleFontPicker(): void {
    activeSubtitleFontTrack = null;
    subtitleFontPicker?.classList.add("hidden");
    subtitleFontPickerOptions?.replaceChildren();
    subtitleStyleDialog?.querySelectorAll<HTMLInputElement>('[data-style-field="font_family"]')
      .forEach((input) => input.setAttribute("aria-expanded", "false"));
  }

  function renderSubtitleFontPicker(): void {
    if (!activeSubtitleFontTrack || !subtitleFonts || !subtitleFontPickerOptions) return;
    const input = subtitleStyleField<HTMLInputElement>(activeSubtitleFontTrack, "font_family");
    const query = input.value.trim().toLocaleLowerCase();
    const priority = subtitleFontPriorityFamilies(activeSubtitleFontTrack);
    const current = input.value.trim();
    const matches = subtitleFonts
      .filter((font) => {
        if (!query) return true;
        return font.family.toLocaleLowerCase().includes(query)
          || font.aliases.some((alias) => alias.toLocaleLowerCase().includes(query));
      })
      .sort((left, right) => {
        const rank = (font: SubtitleFontFamily): number => {
          if (font.family.localeCompare(current, undefined, { sensitivity: "accent" }) === 0) return -3;
          if (query && font.family.toLocaleLowerCase().startsWith(query)) return -2;
          const commonIndex = priority.findIndex((family) => family.toLocaleLowerCase() === font.family.toLocaleLowerCase());
          return commonIndex >= 0 ? commonIndex : 100;
        };
        return rank(left) - rank(right) || left.family.localeCompare(right.family);
      });
    const visible = matches;
    subtitleFontPickerOptions.replaceChildren();
    if (visible.length === 0) {
      const empty = document.createElement("p");
      empty.className = "subtitle-font-picker-empty";
      empty.textContent = "没有匹配的本机字体；可以继续输入 family 名，但导出时可能发生 fallback。";
      subtitleFontPickerOptions.append(empty);
    } else {
      const sampleText = subtitleFontSampleText(activeSubtitleFontTrack);
      for (const font of visible) {
        const option = document.createElement("button");
        option.type = "button";
        option.className = "subtitle-font-option";
        option.setAttribute("role", "option");
        option.setAttribute("aria-selected", String(font.family === current));
        const heading = document.createElement("span");
        heading.className = "subtitle-font-option-heading";
        const name = document.createElement("strong");
        name.textContent = font.family;
        const meta = document.createElement("small");
        const common = priority.some((family) => family.toLocaleLowerCase() === font.family.toLocaleLowerCase());
        meta.textContent = `${common ? "常用 · " : ""}${font.face_count} 个字重${font.monospaced ? " · 等宽" : ""}`;
        heading.append(name, meta);
        const sample = document.createElement("span");
        sample.className = "subtitle-font-option-sample";
        sample.style.fontFamily = `"${font.family.replace(/\\/g, "\\\\").replace(/"/g, '\\"')}"`;
        sample.textContent = sampleText;
        option.append(heading, sample);
        option.addEventListener("click", () => {
          input.value = font.family;
          input.dispatchEvent(new Event("input", { bubbles: true }));
          input.focus();
          closeSubtitleFontPicker();
        });
        subtitleFontPickerOptions.append(option);
      }
    }
    if (subtitleFontPickerCount) {
      subtitleFontPickerCount.textContent = `${matches.length} 个结果 · 本机 ${subtitleFonts.length}`;
    }
    if (subtitleFontPickerSample) {
      subtitleFontPickerSample.textContent = `样张：${subtitleFontSampleText(activeSubtitleFontTrack)}（快速参考，最终以左侧 libass 预览为准）`;
    }
  }

  function positionSubtitleFontPicker(): void {
    if (!activeSubtitleFontTrack || !subtitleFontPicker) return;
    const input = subtitleStyleField<HTMLInputElement>(activeSubtitleFontTrack, "font_family");
    const rect = input.getBoundingClientRect();
    const width = Math.min(520, Math.max(360, rect.width + 90));
    subtitleFontPicker.style.width = `${width}px`;
    subtitleFontPicker.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`;
    const height = Math.min(380, subtitleFontPicker.offsetHeight || 380);
    const below = rect.bottom + 7;
    subtitleFontPicker.style.top = `${window.innerHeight - below >= height ? below : Math.max(12, rect.top - height - 7)}px`;
  }

  function openSubtitleFontPicker(track: "source" | "translation"): void {
    if (!subtitleStyleDialog.open || !subtitleFonts || !subtitleFontPicker) return;
    activeSubtitleFontTrack = track;
    subtitleStyleField<HTMLInputElement>(track, "font_family").setAttribute("aria-expanded", "true");
    subtitleFontPicker.classList.remove("hidden");
    renderSubtitleFontPicker();
    positionSubtitleFontPicker();
  }

  function splitSubtitleColor(value: string | null, fallback: string): { rgb: string; alpha: number } {
    const normalized = value && /^#[0-9a-f]{6}([0-9a-f]{2})?$/i.test(value) ? value : fallback;
    const rgb = normalized.slice(0, 7);
    const alphaByte = normalized.length === 9 ? Number.parseInt(normalized.slice(7), 16) : 255;
    return { rgb, alpha: Math.round(alphaByte / 255 * 100) };
  }

  function joinSubtitleColor(rgb: string, alphaPercent: number): string {
    const alpha = Math.round(Math.max(0, Math.min(100, alphaPercent)) / 100 * 255);
    return `${rgb.toUpperCase()}${alpha.toString(16).padStart(2, "0").toUpperCase()}`;
  }

  function populateSubtitleStyleEditor(track: "source" | "translation", style: SubtitleStyle): void {
    subtitleStyleField<HTMLInputElement>(track, "font_family").value = style.font_family;
    for (const field of ["font_size", "letter_spacing", "outline_width", "shadow", "margin_left", "margin_right", "margin_vertical"] as const) {
      subtitleStyleField<HTMLInputElement>(track, field).value = String(style[field]);
    }
    subtitleStyleField<HTMLSelectElement>(track, "alignment").value = String(style.alignment);
    subtitleStyleField<HTMLInputElement>(track, "bold").checked = style.bold;
    subtitleStyleField<HTMLInputElement>(track, "italic").checked = style.italic;
    const primary = splitSubtitleColor(style.primary_color, "#FFFFFF");
    const outline = splitSubtitleColor(style.outline_color, "#00000080");
    const background = splitSubtitleColor(style.background_color, "#000000CC");
    subtitleStyleField<HTMLInputElement>(track, "primary_color_rgb").value = primary.rgb;
    subtitleStyleField<HTMLInputElement>(track, "primary_color_alpha").value = String(primary.alpha);
    subtitleStyleField<HTMLInputElement>(track, "outline_color_rgb").value = outline.rgb;
    subtitleStyleField<HTMLInputElement>(track, "outline_color_alpha").value = String(outline.alpha);
    subtitleStyleField<HTMLInputElement>(track, "background_enabled").checked = style.background_color !== null;
    subtitleStyleField<HTMLInputElement>(track, "background_color_rgb").value = background.rgb;
    subtitleStyleField<HTMLInputElement>(track, "background_color_alpha").value = String(background.alpha);
  }

  function readSubtitleStyleEditor(track: "source" | "translation"): SubtitleStyle {
    const number = (field: string): number => Number(subtitleStyleField<HTMLInputElement>(track, field).value);
    const primaryRgb = subtitleStyleField<HTMLInputElement>(track, "primary_color_rgb").value;
    const outlineRgb = subtitleStyleField<HTMLInputElement>(track, "outline_color_rgb").value;
    const backgroundEnabled = subtitleStyleField<HTMLInputElement>(track, "background_enabled").checked;
    return {
      font_family: subtitleStyleField<HTMLInputElement>(track, "font_family").value.trim(),
      font_size: number("font_size"),
      primary_color: joinSubtitleColor(primaryRgb, number("primary_color_alpha")),
      outline_color: joinSubtitleColor(outlineRgb, number("outline_color_alpha")),
      outline_width: number("outline_width"),
      shadow: number("shadow"),
      background_color: backgroundEnabled
        ? joinSubtitleColor(
          subtitleStyleField<HTMLInputElement>(track, "background_color_rgb").value,
          number("background_color_alpha"),
        )
        : null,
      bold: subtitleStyleField<HTMLInputElement>(track, "bold").checked,
      italic: subtitleStyleField<HTMLInputElement>(track, "italic").checked,
      alignment: number("alignment"),
      margin_left: number("margin_left"),
      margin_right: number("margin_right"),
      margin_vertical: number("margin_vertical"),
      letter_spacing: number("letter_spacing"),
    };
  }

  function currentSubtitleStyles(): SubtitleStyleSet {
    return { source: readSubtitleStyleEditor("source"), translation: readSubtitleStyleEditor("translation") };
  }

  function renderSubtitleFontCoverage(track: "source" | "translation", coverage: SubtitleFontCoverage): void {
    const host = subtitleStyleDialog?.querySelector<HTMLElement>(`[data-font-report="${track}"]`);
    if (!host) return;
    if (!coverage.installed) {
      host.className = "subtitle-font-report warning";
      host.textContent = `本机没有“${coverage.requested_family}”；libass 会选择 fallback。`;
      return;
    }
    if (coverage.needs_fallback) {
      host.className = "subtitle-font-report warning";
      host.textContent = `已匹配 ${coverage.matched_family ?? coverage.requested_family}，但任务字符 ${coverage.missing_characters.join(" ")} 缺少字形，将发生 fallback。`;
      return;
    }
    host.className = "subtitle-font-report ready";
    host.textContent = `已匹配 ${coverage.matched_family ?? coverage.requested_family}${coverage.post_script_name ? ` · ${coverage.post_script_name}` : ""}；任务字符覆盖完整。`;
  }

  function renderSubtitleFontReport(report: SubtitleFontReport): void {
    renderSubtitleFontCoverage("source", report.source);
    renderSubtitleFontCoverage("translation", report.translation);
  }

  function subtitleStyleDraftIsValid(): boolean {
    if (!subtitleStyleDialog) return false;
    return Array.from(
      subtitleStyleDialog.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-style-field]"),
    ).every((field) => field.checkValidity() && (field instanceof HTMLSelectElement || field.type === "checkbox" || field.value.trim() !== ""));
  }



  dialog.addEventListener("keydown", event => {
    if (event.key !== "Escape" || !activeSubtitleFontTrack || subtitleFontPicker?.classList.contains("hidden")) return;
    event.preventDefault(); event.stopPropagation();
    subtitleStyleField<HTMLInputElement>(activeSubtitleFontTrack, "font_family").focus();
    closeSubtitleFontPicker();
  }, { capture: true, signal: events.signal });

  on(previewSubtitleStyleButton, "click", () => {
    void actions.preview();
  });
  on(saveSubtitleStyleButton, "click", () => void actions.save());
  on(dialog.querySelector<HTMLButtonElement>("#close-subtitle-style"), "click", () => actions.close());
  subtitleStyleDialog?.querySelectorAll<HTMLInputElement | HTMLSelectElement>("[data-style-field]").forEach((field) => {
    on(field, "input", () => actions.changed());
  });
  for (const track of ["source", "translation"] as const) {
    const input = subtitleStyleField<HTMLInputElement>(track, "font_family");
    on(input, "focus", () => openSubtitleFontPicker(track));
    on(input, "input", () => openSubtitleFontPicker(track));
    on(input, "keydown", (event) => {
      if (event.key === "ArrowDown" && !subtitleFontPicker?.classList.contains("hidden")) {
        event.preventDefault();
        subtitleFontPickerOptions?.querySelector<HTMLButtonElement>(".subtitle-font-option")?.focus();
      } else if (event.key === "Escape") {
        closeSubtitleFontPicker();
      }
    });
    on(subtitleStyleDialog?.querySelector<HTMLButtonElement>(`[data-font-picker-toggle="${track}"]`), "click", () => {
      if (activeSubtitleFontTrack === track && !subtitleFontPicker?.classList.contains("hidden")) {
        closeSubtitleFontPicker();
      } else {
        input.focus();
        openSubtitleFontPicker(track);
      }
    });
  }
  on(document, "pointerdown", (event) => {
    const target = event.target;
    if (!(target instanceof Node) || subtitleFontPicker?.contains(target) || (target instanceof Element && target.closest(".style-font-picker"))) return;
    closeSubtitleFontPicker();
  });
  on(subtitleStyleDialog?.querySelector<HTMLElement>(".subtitle-style-controls"), "scroll", closeSubtitleFontPicker);
  on(window, "resize", closeSubtitleFontPicker);
  on(subtitleStylePreviewTrack, "change", () => actions.changed(0));
  on(subtitleStyleDialog, "close", () => {
    if (!subtitleStyleDialog.open) actions.close();
  });

  const view = {
    open: () => { subtitleStylePreviewHost?.replaceChildren(); subtitleFontEvents?.classList.add("hidden"); boundary.open(); },
    close: () => { boundary.close(); closeSubtitleFontPicker(); },
    valid: subtitleStyleDraftIsValid,
    read: () => ({ styles: currentSubtitleStyles(), track: subtitleStylePreviewTrack?.value ?? "bilingual" }),
    loaded: (state: SubtitleStyleState, fonts: SubtitleFontFamily[]) => {
      subtitleFonts = fonts; populateSubtitleStyleEditor("source", state.styles.source);
      populateSubtitleStyleEditor("translation", state.styles.translation); renderSubtitleFontReport(state.font_report);
    },
    saved: (state: SubtitleStyleState) => renderSubtitleFontReport(state.font_report),
    previewed: (preview: SubtitleStylePreview, automatic: boolean, saved: boolean) => {
      renderSubtitleFontReport(preview.font_report);
      if (subtitleStylePreviewHost) {
        const image = document.createElement("img"); image.alt = `libass 字幕预览，时间 ${options.time(preview.timestamp_ms)}`;
        image.src = options.fileUrl(preview.output_path); subtitleStylePreviewHost.replaceChildren(image);
      }
      if (subtitleFontEvents) {
        const pre = subtitleFontEvents.querySelector("pre");
        if (pre) pre.textContent = preview.font_events.length ? preview.font_events.join("\n") : "本次 FFmpeg 日志没有报告字体替换事件。";
        subtitleFontEvents.classList.remove("hidden");
      }
      if (subtitleStyleMessage) subtitleStyleMessage.textContent = `${automatic ? "已自动刷新" : "已刷新"} ${options.time(preview.timestamp_ms)} 的真实渲染帧；${saved ? "当前样式已保存。" : "预览草稿尚未保存。"}`;
    },
    busy: (value: boolean, ready: boolean) => {
      if (previewSubtitleStyleButton) previewSubtitleStyleButton.disabled = value || !ready;
      if (saveSubtitleStyleButton) saveSubtitleStyleButton.disabled = value || !ready;
      subtitleStyleDialog?.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLButtonElement>("[data-style-field], [data-font-picker-toggle], #subtitle-style-preview-track").forEach(field => { field.disabled = !ready; });
    },
    message: (text: string) => { if (subtitleStyleMessage) subtitleStyleMessage.textContent = text; },
  };
  return { ...view, bind(controller: Actions, trigger: HTMLButtonElement | null) { actions = controller; on(trigger, "click", () => { trigger?.focus(); void actions.open(); }); }, dispose() { events.abort(); closeSubtitleFontPicker(); boundary.dispose(); subtitleFonts = null; dialog.remove(); } };
}
