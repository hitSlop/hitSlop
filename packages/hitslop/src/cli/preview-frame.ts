// The `slop dev` page around the app frame: the view selector and the editor's resize handle.
// Vite compiles it for /__preview__/resize.js, the one script the preview host policy admits,
// so it imports nothing at run time. No authored code or native bridge runs here.
import type { WindowBounds } from "../schema/constants";
import type { PreviewSurface, PreviewView } from "./preview-views.svelte";

const stage = document.querySelector<HTMLElement>(".stage")!;
const frame = document.querySelector("iframe")!;
const select = document.querySelector("select")!;
const widgets = select.querySelector("optgroup")!;
const status = document.querySelector("output")!;
const handle = document.querySelector<HTMLButtonElement>(".resize-preview");

const choose = () =>
  frame.contentWindow?.postMessage(
    { type: "slop:view", view: select.value as PreviewView },
    location.origin,
  );
select.addEventListener("change", choose);
// The frame says when it is ready, after each load and update, and which surface each view
// needs. A reload keeps the selection: it is sent again.
addEventListener("message", ({ source, origin, data }) => {
  if (origin !== location.origin || source !== frame.contentWindow) return;
  if (data?.type === "slop:preview-ready") {
    widgets.hidden = widgets.disabled = !data.widget;
    if (!data.widget && select.selectedOptions[0]?.parentElement === widgets)
      select.value = "editor";
    choose();
  } else if (data?.type === "slop:surface") {
    const { width, height, clipped } = data as PreviewSurface;
    stage.dataset.view = width === undefined ? "editor" : "static";
    if (width !== undefined) {
      stage.style.setProperty("--view-width", `${width}px`);
      stage.style.setProperty("--view-height", `${height}px`);
    }
    status.textContent = clipped ? "Content extends beyond this canvas and will be clipped." : "";
  }
});

if (handle) {
  const bounds: typeof WindowBounds = JSON.parse(handle.dataset.bounds!);
  const ratio = Number(stage.dataset.aspect);
  const size = (width: number, height: number, vertical = false) => {
    if (ratio) {
      width = vertical ? height * ratio : width;
      width = Math.max(
        bounds.minWidth,
        bounds.minHeight * ratio,
        Math.min(bounds.max, bounds.max * ratio, width),
      );
      height = width / ratio;
    } else {
      width = Math.max(bounds.minWidth, Math.min(bounds.max, width));
      height = Math.max(bounds.minHeight, Math.min(bounds.max, height));
    }
    stage.style.setProperty("--editor-width", `${width}px`);
    stage.style.setProperty("--editor-height", `${height}px`);
  };
  const current = () => frame.getBoundingClientRect();
  let drag: { id: number; x: number; y: number; width: number; height: number } | undefined;
  handle.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || stage.dataset.view !== "editor") return;
    event.preventDefault();
    const { width, height } = current();
    drag = { id: event.pointerId, x: event.clientX, y: event.clientY, width, height };
    handle.setPointerCapture(event.pointerId);
  });
  handle.addEventListener("pointermove", (event) => {
    if (drag?.id !== event.pointerId) return;
    size(drag.width + event.clientX - drag.x, drag.height + event.clientY - drag.y);
  });
  for (const type of ["pointerup", "pointercancel", "lostpointercapture"])
    handle.addEventListener(type, () => (drag = undefined));
  handle.addEventListener("keydown", (event) => {
    if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
    event.preventDefault();
    const { width, height } = current();
    const step = event.shiftKey ? 50 : 10;
    size(
      width + (event.key === "ArrowRight" ? step : event.key === "ArrowLeft" ? -step : 0),
      height + (event.key === "ArrowDown" ? step : event.key === "ArrowUp" ? -step : 0),
      event.key === "ArrowDown" || event.key === "ArrowUp",
    );
  });
}
