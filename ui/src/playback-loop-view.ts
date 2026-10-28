export type LoopAction = "sentence" | "a" | "b" | "head" | "tail" | "off";
export type LoopMessage = { message: string; local?: boolean };
const closed = "循环已关闭";

/** Owns loop tool nodes and listeners; the shared player/session owns loop state. */
export function mountPlaybackLoopView(hosts: Iterable<Element>, action: (value: LoopAction) => LoopMessage | null) {
  const events = new AbortController(), nodes: HTMLElement[] = [];
  let disposed = false;
  const show = (message: string) => {
    if (disposed) return;
    for (const node of nodes) node.querySelector<HTMLElement>(".loop-state")!.textContent = message;
  };
  for (const host of hosts) {
    const tools = document.createElement("div");
    tools.className = "offline-tools loop-tools";
    tools.innerHTML = '<button type="button" class="secondary" data-loop="sentence">循环当前句</button><button type="button" class="secondary" data-loop="a">设置 A</button><button type="button" class="secondary" data-loop="b">设置 B 并循环</button><button type="button" class="secondary" data-loop="head">试听句首衔接</button><button type="button" class="secondary" data-loop="tail">试听句尾衔接</button><button type="button" class="secondary" data-loop="off">关闭循环</button><span class="loop-state">循环已关闭</span>';
    host.after(tools); nodes.push(tools);
    for (const button of tools.querySelectorAll<HTMLButtonElement>("[data-loop]")) {
      button.addEventListener("click", () => {
        const result = action(button.dataset.loop as LoopAction);
        if (!result || disposed) return;
        if (result.local) tools.querySelector<HTMLElement>(".loop-state")!.textContent = result.message;
        else show(result.message);
      }, { signal: events.signal });
    }
  }
  return {
    show,
    reset() { show(closed); },
    dispose() {
      if (disposed) return;
      disposed = true; events.abort();
      for (const node of nodes.splice(0)) node.remove();
    },
  };
}
