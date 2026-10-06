import type { SlopManifest } from "@hitslop/schema";
import { DefaultWindowRadius } from "@hitslop/schema/constants";

/** Stands in for the native window: manifest size, shape mask, and backing or skin chrome. */
export function previewFrame(manifest: Pick<SlopManifest, "title" | "presentation">) {
  const presentation = manifest.presentation;
  const { width, height } = presentation;
  const escape = (value: string) => value.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
  const surface = [`width:${width}px`, `height:${height}px`];
  let definitions = "",
    resize = false,
    ratio = 0,
    shadow = false;
  if ("skin" in presentation) {
    surface.push(
      ...["background", "-webkit-mask", "mask"].map(
        (property) => `${property}:url("/${presentation.skin}") 0 0/100% 100% no-repeat`,
      ),
    );
  } else {
    const { shape = DefaultWindowRadius, background, resizable = true, lockAspect = false } = presentation;
    resize = resizable;
    ratio = lockAspect ? width / height : 0;
    // The stage's drop shadow is a filter, which would hide the page from a backdrop filter,
    // so glass frosts the checkerboard without it.
    shadow = background === undefined;
    if (shadow) surface.push("background:Canvas");
    if (background === "glass")
      surface.push(
        "background:color-mix(in srgb,Canvas 30%,transparent)",
        "-webkit-backdrop-filter:blur(24px) saturate(1.8)",
        "backdrop-filter:blur(24px) saturate(1.8)",
      );
    if (typeof shape === "string")
      surface.push(`border-radius:${shape}`, `clip-path:inset(0 round ${shape})`);
    else {
      const [w, h] = shape.viewBox ?? [width, height];
      const rule = shape.fillRule ?? "nonzero";
      definitions = `<svg width="0" height="0" aria-hidden="true" style="position:absolute"><defs><clipPath id="window-shape" clipPathUnits="objectBoundingBox"><path d="${escape(shape.path)}" transform="scale(${1 / w},${1 / h})" clip-rule="${rule}" fill-rule="${rule}"/></clipPath></defs></svg>`;
      surface.push("clip-path:url(#window-shape)");
    }
  }
  const title = escape(manifest.title);
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${title} preview</title><style>html,body{margin:0;min-height:100%;height:100%}body{display:grid;place-items:center;background:#e4e4e4 repeating-conic-gradient(#d2d2d2 0 25%,#e4e4e4 0 50%) 0 0/24px 24px}.stage{position:relative;pointer-events:none;${shadow ? "filter:drop-shadow(0 18px 25px #0004)" : ""}}.window{pointer-events:auto;overflow:hidden;${surface.join(";")}}iframe{display:block;width:100%;height:100%;border:0}.resize-preview{position:absolute;right:-28px;bottom:-28px;width:28px;height:28px;pointer-events:auto;cursor:nwse-resize;touch-action:none;border:1px solid #777;background:white;border-radius:5px}.resize-preview:focus-visible{outline:3px solid #1957bd}</style></head><body>${definitions}<div class="stage" data-aspect="${ratio}"><div class="window"><iframe src="/app.html" title="${title}"></iframe></div>${resize ? '<button class="resize-preview" aria-label="Resize preview" title="Drag to resize; arrow keys adjust by 10px">↘</button>' : ""}</div><script src="/__preview__/resize.js"></script></body></html>`;
}
