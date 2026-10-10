import type { WindowDefinition } from "../wire/app.generated";
import { silhouettePath } from "../schema/presentation";

/** Stands in for the native window: manifest size, shape mask, and backing or skin chrome. */
export function previewFrame(app: { title: string; window: WindowDefinition }) {
  const presentation = app.window;
  const { width, height, frame } = presentation;
  const escape = (value: string) => value.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
  const surface = [`width:${width}px`, `height:${height}px`];
  let definitions = "",
    resize = false,
    ratio = 0,
    shadow = false;
  if (frame.kind === "skin") {
    surface.push(
      ...["background", "-webkit-mask", "mask"].map(
        (property) => `${property}:url("/assets/${frame.skin}") 0 0/100% 100% no-repeat`,
      ),
    );
  } else {
    const { shape, radius, background, resizable, lockAspect } = frame.frame;
    resize = resizable;
    ratio = lockAspect ? width / height : 0;
    // The stage's drop shadow is a filter, which would hide the page from a backdrop filter,
    // so glass frosts the checkerboard without it.
    shadow = background === null;
    if (shadow) surface.push("background:Canvas");
    if (background === "glass")
      surface.push(
        "background:color-mix(in srgb,Canvas 30%,transparent)",
        "-webkit-backdrop-filter:blur(24px) saturate(1.8)",
        "backdrop-filter:blur(24px) saturate(1.8)",
      );
    if (shape.kind === "radii")
      surface.push(`border-radius:${radius}`, `clip-path:inset(0 round ${radius})`);
    else {
      const { viewBoxWidth: w, viewBoxHeight: h } = shape;
      const rule = shape.evenOdd ? "evenodd" : "nonzero";
      definitions = `<svg width="0" height="0" aria-hidden="true" style="position:absolute"><defs><clipPath id="window-shape" clipPathUnits="objectBoundingBox"><path d="${escape(silhouettePath(shape.segments))}" transform="scale(${1 / w},${1 / h})" clip-rule="${rule}" fill-rule="${rule}"/></clipPath></defs></svg>`;
      surface.push("clip-path:url(#window-shape)");
    }
  }
  const title = escape(app.title);
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title} preview</title><style>html,body{margin:0;min-height:100%;height:100%}body{display:grid;place-items:center;background:#e4e4e4 repeating-conic-gradient(#d2d2d2 0 25%,#e4e4e4 0 50%) 0 0/24px 24px}.stage{position:relative;pointer-events:none;${shadow ? "filter:drop-shadow(0 18px 25px #0004)" : ""}}.window{pointer-events:auto;overflow:hidden;${surface.join(";")}}iframe{display:block;width:100%;height:100%;border:0}.resize-preview{position:absolute;right:-28px;bottom:-28px;width:28px;height:28px;pointer-events:auto;cursor:nwse-resize;touch-action:none;border:1px solid #777;background:white;border-radius:5px}.resize-preview:focus-visible{outline:3px solid #1957bd}</style></head><body>${definitions}<div class="stage" data-aspect="${ratio}"><div class="window"><iframe src="/app.html" title="${title}"></iframe></div>${resize ? '<button class="resize-preview" aria-label="Resize preview" title="Drag to resize; arrow keys adjust by 10px">↘</button>' : ""}</div><script src="/__preview__/resize.js"></script></body></html>`;
}
