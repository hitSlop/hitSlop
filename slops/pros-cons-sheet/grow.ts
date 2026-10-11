import { tick } from "svelte";

export function grow(node: HTMLTextAreaElement, _value: string) {
  let active = true;
  const resize = () => {
    if (!active || !node.getClientRects().length) return;
    node.style.height = "auto";
    node.style.height = `${node.scrollHeight}px`;
  };
  let width = 0;
  let frame = 0;
  const observer = new ResizeObserver((entries) => {
    const next = entries[0]?.contentRect.width ?? 0;
    if (next !== width) {
      width = next;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(resize);
    }
  });
  observer.observe(node);
  void document.fonts.ready.then(resize);
  resize();
  return {
    update() {
      void tick().then(resize);
    },
    destroy() {
      active = false;
      cancelAnimationFrame(frame);
      observer.disconnect();
    },
  };
}
