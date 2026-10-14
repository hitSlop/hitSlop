import { readFileSync } from "node:fs";
import type { Capability, WindowDefinition } from "../wire/app.generated";
import { silhouetteClip } from "../schema/presentation";
import { capabilityFeatures } from "../schema/policy";
import { WidgetViewports, WindowBounds } from "../schema/constants";

const template = readFileSync(new URL("preview.html", import.meta.url), "utf8");

/** Stands in for the native window: manifest size, shape mask, and backing or skin chrome,
 * around the app frame and its view selector (preview-frame.ts). */
export function previewFrame(app: {
  title: string;
  window: WindowDefinition;
  capabilities: readonly Capability[];
  widget?: boolean;
}) {
  const { width, height, frame } = app.window;
  const escape = (value: string) => value.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
  const surface: string[] = [];
  let clip = "",
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
      clip = silhouetteClip(shape);
      surface.push("clip-path:url(#window-shape)");
    }
  }
  // The app frame gets only the devices it declared; the response headers say the same.
  const features = capabilityFeatures(app.capabilities);
  const values: Record<string, string> = {
    title: escape(app.title),
    surface: surface.join(";"),
    shadow: shadow ? '.stage[data-view="editor"]{filter:drop-shadow(0 18px 25px #0004)}' : "",
    widgets: app.widget ? "" : "hidden disabled",
    families: WidgetViewports.map(
      ({ family }) =>
        `<option value="${family}">${family[0]!.toUpperCase()}${family.slice(1)} widget</option>`,
    ).join(""),
    clip,
    aspect: String(ratio),
    width: String(width),
    height: String(height),
    allow: features.length ? `allow="${features.join("; ")}"` : "",
    resize: resize
      ? `<button class="resize-preview" aria-label="Resize preview" title="Drag to resize; arrow keys adjust by 10px" data-bounds="${escape(JSON.stringify(WindowBounds))}">↘</button>`
      : "",
  };
  return template.replace(/\{\{(\w+)\}\}/g, (_, key: string) => values[key]!);
}
