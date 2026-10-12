import { WidgetViewports, WindowBounds as B } from "../schema/constants";

export const previewResizeScript = String.raw`
// Preview chrome stays outside the clipped surface. No authored code or native bridge.
const stage = document.querySelector(".stage");
const surface = document.querySelector(".window");
const handle = document.querySelector(".resize-preview");
const frame = document.querySelector("iframe");
const selector = document.querySelector(".preview-modes select");
const status = document.querySelector(".preview-modes output");
const viewports = ${JSON.stringify(WidgetViewports)};
let requested = "editor";
let generation = 0;
let activeToken;
let updating;
let stopOverflow = () => {};
let widgetMode = false;
const original = { surface: surface.style.cssText, stage: stage.style.cssText };
async function updatePreview() {
  const revision = ++generation;
  const previous = updating;
  updating = (async () => {
    await previous;
    if (revision !== generation) return;
    const page = frame.contentWindow?.__slop;
    if (!page?.capture) return; // The ready message retries after a load/rebuild.
    try {
      stopOverflow();
      if (activeToken) await page.dispatch({ method: "capture.restore", token: activeToken });
      activeToken = undefined;
      status.textContent = "";
      if (requested === "editor") {
        if (widgetMode) {
          surface.style.cssText = original.surface;
          stage.style.cssText = original.stage;
          widgetMode = false;
        }
        frame.style.pointerEvents = "";
        if (handle) handle.hidden = false;
        return;
      }
      const viewport = viewports.find(v => v.family === requested);
      if (!viewport) return;
      if (!widgetMode) {
        original.surface = surface.style.cssText;
        original.stage = stage.style.cssText;
        widgetMode = true;
      }
      // A widget has its own rectangular canvas, independent of the editor's skin/mask.
      surface.style.cssText = "width:" + viewport.width + "px;height:" + viewport.height + "px;overflow:hidden;background:transparent;mask:none;-webkit-mask:none;clip-path:none;border-radius:0;backdrop-filter:none;-webkit-backdrop-filter:none";
      stage.style.filter = "none";
      frame.style.pointerEvents = "none";
      if (handle) handle.hidden = true;
      activeToken = crypto.randomUUID();
      await page.dispatch({ method: "capture.widget", token: activeToken, viewport });
      const target = frame.contentDocument.querySelector('[data-slop-capture-target="widget"]');
      if (target) {
        let warned = false;
        const check = () => {
          if (revision !== generation) return;
          const overflow = target.scrollHeight > target.clientHeight || target.scrollWidth > target.clientWidth;
          status.textContent = overflow ? "Widget content extends beyond its canvas and will be clipped." : "";
          if (overflow && !warned) console.warn(status.textContent);
          warned = overflow;
        };
        const resized = new ResizeObserver(check);
        const observeChildren = () => {
          resized.disconnect();
          resized.observe(target);
          for (const child of target.children) resized.observe(child);
          check();
        };
        const changed = new MutationObserver(observeChildren);
        changed.observe(target, {subtree: true, childList: true, attributes: true, characterData: true});
        observeChildren();
        stopOverflow = () => { changed.disconnect(); resized.disconnect(); };
      }
    } catch (error) { status.textContent = String(error); }
  })();
  await updating;
}
if (selector) {
  selector.addEventListener("change", () => { requested = selector.value; void updatePreview(); });
  window.addEventListener("message", event => {
    if (event.origin !== location.origin || event.source !== frame.contentWindow) return;
    if (event.data?.type === "slop:preview-ready") {
      selector.closest("label").hidden = !event.data.widget;
      if (!event.data.widget) { requested = "editor"; selector.value = "editor"; }
    } else if (event.data !== "slop:preview-ready") return;
    void updatePreview();
  });
}
if (stage && surface && handle) {
  const ratio = Number(stage.dataset.aspect);
  const size = (width, height, vertical = false) => {
    if (ratio) {
      width = vertical ? height * ratio : width;
      width = Math.max(${B.minWidth}, ${B.minHeight} * ratio, Math.min(${B.max}, ${B.max} * ratio, width));
      height = width / ratio;
    } else {
      width = Math.max(${B.minWidth}, Math.min(${B.max}, width));
      height = Math.max(${B.minHeight}, Math.min(${B.max}, height));
    }
    surface.style.width = width + "px";
    surface.style.height = height + "px";
  };
  let drag;
  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || requested !== "editor") return;
    event.preventDefault();
    const rect = surface.getBoundingClientRect();
    drag = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      width: rect.width,
      height: rect.height,
    };
    handle.setPointerCapture(event.pointerId);
  });
  handle.addEventListener("pointermove", (event) => {
    if (drag?.id !== event.pointerId) return;
    size(drag.width + event.clientX - drag.x, drag.height + event.clientY - drag.y);
  });
  for (const event of ["pointerup", "pointercancel", "lostpointercapture"]) {
    handle.addEventListener(event, () => {
      drag = undefined;
    });
  }
  handle.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    const rect = surface.getBoundingClientRect(),
      step = event.shiftKey ? 50 : 10;
    size(
      rect.width + (event.key === "ArrowRight" ? step : event.key === "ArrowLeft" ? -step : 0),
      rect.height + (event.key === "ArrowDown" ? step : event.key === "ArrowUp" ? -step : 0),
      event.key === "ArrowDown" || event.key === "ArrowUp",
    );
  });
}
`;
