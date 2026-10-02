// A canvas of silver foil that the pointer rubs away. Scratching is presentation only:
// once enough is cleared the foil fades and `onreveal` saves that the item is revealed.
export type ScratchOptions = { onreveal: () => void; reduced: boolean };

const CLEAR_TO_REVEAL = 0.5;

export function scratch(canvas: HTMLCanvasElement, options: ScratchOptions) {
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) return {};
  let opts = options;
  let touched = false;
  let last: { x: number; y: number } | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let width = 0;
  let height = 0;
  const dpr = Math.min(2, window.devicePixelRatio || 1);

  const color = (name: string) => getComputedStyle(document.documentElement).getPropertyValue(`--slop-${name}`).trim() || "#c8ccd4";

  function paint() {
    const box = canvas.getBoundingClientRect();
    width = Math.max(1, Math.round(box.width));
    height = Math.max(1, Math.round(box.height));
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    ctx!.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx!.globalCompositeOperation = "source-over";
    const base = ctx!.createLinearGradient(0, 0, width, height);
    base.addColorStop(0, color("foil"));
    base.addColorStop(0.5, "#f4f5f8");
    base.addColorStop(1, color("foilDark"));
    ctx!.fillStyle = base;
    ctx!.fillRect(0, 0, width, height);
    ctx!.strokeStyle = "rgba(255,255,255,0.35)";
    ctx!.lineWidth = 1;
    for (let x = -height; x < width; x += 7) {
      ctx!.beginPath();
      ctx!.moveTo(x, height);
      ctx!.lineTo(x + height, 0);
      ctx!.stroke();
    }
    ctx!.fillStyle = color("foilDark");
    ctx!.font = `800 ${Math.round(Math.min(width, height) * 0.46)}px ui-rounded, "Avenir Next", sans-serif`;
    ctx!.textAlign = "center";
    ctx!.textBaseline = "middle";
    ctx!.globalAlpha = 0.55;
    ctx!.fillText("?", width / 2, height / 2 + 2);
    ctx!.globalAlpha = 1;
  }

  function point(event: PointerEvent) {
    const box = canvas.getBoundingClientRect();
    return { x: event.clientX - box.left, y: event.clientY - box.top };
  }

  function rub(from: { x: number; y: number }, to: { x: number; y: number }) {
    ctx!.globalCompositeOperation = "destination-out";
    ctx!.lineWidth = 30;
    ctx!.lineCap = "round";
    ctx!.beginPath();
    ctx!.moveTo(from.x, from.y);
    ctx!.lineTo(to.x, to.y);
    ctx!.stroke();
  }

  function clearedShare(): number {
    const pixels = ctx!.getImageData(0, 0, canvas.width, canvas.height).data;
    let clear = 0;
    let total = 0;
    for (let y = 0; y < canvas.height; y += 4) {
      for (let x = 0; x < canvas.width; x += 4) {
        total += 1;
        if (pixels[(y * canvas.width + x) * 4 + 3]! < 40) clear += 1;
      }
    }
    return clear / total;
  }

  function finish() {
    canvas.dataset.clearing = "";
    timer = setTimeout(() => opts.onreveal(), opts.reduced ? 0 : 260);
  }

  const down = (event: PointerEvent) => {
    canvas.setPointerCapture(event.pointerId);
    touched = true;
    last = point(event);
    rub(last, last);
  };
  const move = (event: PointerEvent) => {
    if (!last) return;
    const next = point(event);
    rub(last, next);
    last = next;
  };
  const up = () => {
    if (!last) return;
    last = undefined;
    if (clearedShare() >= CLEAR_TO_REVEAL) finish();
  };

  paint();
  const resize = new ResizeObserver(() => { if (!touched) paint(); });
  resize.observe(canvas);
  canvas.addEventListener("pointerdown", down);
  canvas.addEventListener("pointermove", move);
  canvas.addEventListener("pointerup", up);
  canvas.addEventListener("pointercancel", up);

  return {
    update(next: ScratchOptions) { opts = next; },
    destroy() {
      clearTimeout(timer);
      resize.disconnect();
      canvas.removeEventListener("pointerdown", down);
      canvas.removeEventListener("pointermove", move);
      canvas.removeEventListener("pointerup", up);
      canvas.removeEventListener("pointercancel", up);
    },
  };
}
