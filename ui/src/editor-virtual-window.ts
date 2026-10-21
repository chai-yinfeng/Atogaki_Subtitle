export const cueRowHeight = 42;
export function cueWindow(count: number, scrollTop: number, viewportHeight: number, overscan = 8): { start: number; end: number } {
  if (count <= 0) return { start: 0, end: 0 };
  const top = Math.max(0, Math.min(scrollTop, Math.max(0, count * cueRowHeight - Math.max(0, viewportHeight))));
  return {
    start: Math.max(0, Math.floor(top / cueRowHeight) - overscan),
    end: Math.min(count, Math.ceil((top + Math.max(cueRowHeight, viewportHeight)) / cueRowHeight) + overscan),
  };
}
