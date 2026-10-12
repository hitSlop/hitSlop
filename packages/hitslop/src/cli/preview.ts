import type { Capability, WindowDefinition } from "../wire/app.generated";
import { silhouettePath } from "../schema/presentation";
import { capabilityFeatures } from "../schema/policy";

/** Stands in for the native window: manifest size, shape mask, and backing or skin chrome. */
export function previewFrame(app: {
  title: string;
  window: WindowDefinition;
  capabilities: readonly Capability[];
  widget?: boolean;
}) {
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
  // The app frame gets only the devices it declared; the response headers say the same.
  const features = capabilityFeatures(app.capabilities);
  const allow = features.length ? ` allow="${features.join("; ")}"` : "";
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title} preview</title><style>html,body{margin:0;min-height:100%;height:100%}body{display:grid;place-items:center;background:#e4e4e4 repeating-conic-gradient(#d2d2d2 0 25%,#e4e4e4 0 50%) 0 0/24px 24px}.preview-modes{position:fixed;top:16px;left:16px;z-index:2;padding:10px;background:Canvas;color:CanvasText;border-radius:8px;font:14px system-ui}.preview-modes output{display:block;max-width:420px}.stage{position:relative;pointer-events:none;${shadow ? "filter:drop-shadow(0 18px 25px #0004)" : ""}}.window{pointer-events:auto;overflow:hidden;${surface.join(";")}}iframe{display:block;width:100%;height:100%;border:0}.resize-preview{position:absolute;right:-28px;bottom:-28px;width:28px;height:28px;pointer-events:auto;cursor:nwse-resize;touch-action:none;border:1px solid #777;background:white;border-radius:5px}.resize-preview:focus-visible{outline:3px solid #1957bd}</style></head><body><label class="preview-modes" ${app.widget ? "" : "hidden"}>Preview <select aria-label="Preview composition"><option value="editor">Editor</option><option value="small">Small widget</option><option value="medium">Medium widget</option><option value="large">Large widget</option></select><output role="status"></output></label>${definitions}<div class="stage" data-aspect="${ratio}"><div class="window"><iframe src="/app.html" title="${title}"${allow}></iframe></div>${resize ? '<button class="resize-preview" aria-label="Resize preview" title="Drag to resize; arrow keys adjust by 10px">↘</button>' : ""}</div><script src="/__preview__/resize.js"></script></body></html>`;
}
