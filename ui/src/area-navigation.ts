export type MainArea = "workbench" | "listening" | "learning" | "karaoke";
type Options<D> = {
  begin(): number;
  current(navigation: number): boolean;
  area(): MainArea | "workspace";
  stop(): Promise<void>;
  apply(area: MainArea): void;
  refresh(): void;
  learning(): Promise<void>;
  dictionary(): Promise<void>;
  renderDictionary(): void;
  selectedEditor(): string | null;
  editor(jobId: string, detail?: D): Promise<number | null>;
  scrollTop(): void;
};

/** Top-level presentation follows the existing navigation clock, including child visits. */
export function createAreaNavigation<D>(options: Options<D>) {
  let disposed = false;
  return {
    async show(area: MainArea, editorDetail?: D, editorJobId?: string): Promise<void> {
      if (disposed) return;
      const navigation = options.begin();
      const current = (visit = navigation) => !disposed && options.current(visit) && options.area() === area;
      await options.stop();
      if (disposed || !options.current(navigation)) return;
      options.apply(area);
      if (area === "workbench" || area === "listening") options.refresh();
      if (area === "learning") {
        await Promise.all([options.learning(), options.dictionary()]);
        if (!current()) return;
        options.renderDictionary();
      }
      if (area === "karaoke") {
        const target = editorJobId ?? options.selectedEditor();
        if (target) {
          // The editor opens a child visit on the same clock; use its actual navigation ID.
          const visit = await options.editor(target, editorDetail);
          if (visit === null || !current(visit)) return;
          options.scrollTop();
          return;
        }
      }
      if (current()) options.scrollTop();
    },
    dispose() { disposed = true; },
  };
}
