import { WindowBounds as B } from "@hitslop/schema/constants";

export const previewResizeScript = String.raw`
// Preview chrome stays outside the clipped surface. No authored code or native bridge.
const stage = document.querySelector(".stage");
const surface = document.querySelector(".window");
const handle = document.querySelector(".resize-preview");
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
    if (event.button !== 0) return;
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
