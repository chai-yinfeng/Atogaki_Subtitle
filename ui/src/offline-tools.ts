/** Text helpers keep selection offsets in UTF-16, matching browser Range. */
export function matchesQuery(
  query: string,
  values: Array<string | null | undefined>,
): boolean {
  const needle = query.trim().normalize("NFKC").toLocaleLowerCase();
  return (
    !needle ||
    values.some((value) =>
      value?.normalize("NFKC").toLocaleLowerCase().includes(needle),
    )
  );
}
export function escaped(text: string): string {
  return text.replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ]!,
  );
}
export function highlightSaved(
  text: string,
  saved: string[],
  anchors: Array<[number, number]> = [],
  language = "",
): string {
  const intervals = anchors.filter(
    ([start, end]) => start >= 0 && end > start && end <= text.length,
  );
  for (const value of new Set(saved.filter(Boolean))) {
    let index = text.indexOf(value);
    while (index >= 0) {
      const before = text[index - 1] ?? "",
        after = text[index + value.length] ?? "";
      const insideEnglishWord =
        language === "en" &&
        ((/[A-Za-z0-9_]/.test(value[0]) && /[A-Za-z0-9_]/.test(before)) ||
          (/[A-Za-z0-9_]/.test(value.at(-1)!) && /[A-Za-z0-9_]/.test(after)));
      if (!insideEnglishWord) intervals.push([index, index + value.length]);
      index = text.indexOf(value, index + value.length);
    }
  }
  intervals.sort((a, b) => a[0] - b[0]);
  const merged: Array<[number, number]> = [];
  for (const interval of intervals) {
    const last = merged.at(-1);
    if (last && interval[0] <= last[1])
      last[1] = Math.max(last[1], interval[1]);
    else merged.push([...interval]);
  }
  let cursor = 0,
    html = "";
  for (const [start, end] of merged) {
    html +=
      escaped(text.slice(cursor, start)) +
      '<mark class="saved-expression">' +
      escaped(text.slice(start, end)) +
      "</mark>";
    cursor = end;
  }
  return html + escaped(text.slice(cursor));
}

/** A conservative text diff: expose the changed middle without claiming cue alignment. */
export function candidateTextDiff(before: string, after: string): string {
  const left = Array.from(before),
    right = Array.from(after);
  let prefix = 0,
    suffix = 0;
  while (
    prefix < left.length &&
    prefix < right.length &&
    left[prefix] === right[prefix]
  )
    prefix++;
  while (
    suffix < left.length - prefix &&
    suffix < right.length - prefix &&
    left[left.length - 1 - suffix] === right[right.length - 1 - suffix]
  )
    suffix++;
  const common = escaped(left.slice(0, prefix).join(""));
  const ending = suffix ? escaped(left.slice(-suffix).join("")) : "";
  const removed = escaped(left.slice(prefix, left.length - suffix).join(""));
  const added = escaped(right.slice(prefix, right.length - suffix).join(""));
  return (
    common +
    (removed ? `<del>${removed}</del>` : "") +
    (added ? `<ins>${added}</ins>` : "") +
    ending
  );
}

/** Only the top handle owns scrubbing; the waveform and subtitle track keep their gestures. */
export function playheadHandleContains(x: number, y: number, width: number,
  currentMs: number, startMs: number, endMs: number): boolean {
  if (width <= 0 || endMs <= startMs || currentMs < startMs || currentMs > endMs) return false;
  const headX = (currentMs - startMs) / (endMs - startMs) * width;
  return x >= 0 && x <= width && y >= 15 && y <= 37 && Math.abs(x - headX) <= 12;
}
export function timelineTimeAtX(x: number, width: number, startMs: number, endMs: number): number {
  if (width <= 0 || endMs <= startMs) return startMs;
  return Math.round(startMs + Math.max(0, Math.min(1, x / width)) * (endMs - startMs));
}
