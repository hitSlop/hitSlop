import type { SlopPresentation } from "@hitslop/schema";
import { DefaultWindowRadius } from "@hitslop/schema/constants";

/** Host-supplied initial geometry, not the current viewport or a live resize API. */
type PresentationStage = {
  mode: "standard" | "transparent" | "glass" | "skin";
  width: number;
  height: number;
  resizable: boolean;
  radius?: string;
};

/** Maps a manifest presentation to the stage the native host would configure. */
export function presentationStage(presentation: SlopPresentation): PresentationStage {
  if ("skin" in presentation)
    return {
      mode: "skin",
      width: presentation.width,
      height: presentation.height,
      resizable: false,
    };
  return {
    mode: presentation.background ?? "standard",
    width: presentation.width,
    height: presentation.height,
    resizable: presentation.resizable ?? true,
    radius:
      typeof presentation.shape === "object" ? "0" : (presentation.shape ?? DefaultWindowRadius),
  };
}

const hostScrollbarCSS =
  "*{scrollbar-width:none!important}*::-webkit-scrollbar{width:0!important;height:0!important;display:none!important}";

/**
 * The window is the stage in every mode: html, body and the Slop root fill it, and
 * the app lays out inside. Sizing and selection defaults have zero specificity so
 * authors can override them. Editor UI is unselectable except for text fields and
 * editable content; a body or region rule can opt back into selection.
 * Transparent and skin windows show native geometry or chrome behind the page,
 * overriding ordinary authored page backgrounds. A glass window keeps them: a
 * translucent page background tints the frosted material behind it. Capture keeps
 * the page reset and lays out in normal flow.
 */
function presentationStageCSS(stage: PresentationStage): string {
  const page = `html[data-slop-presentation="${stage.mode}"]`;
  const root = `${page}:not([data-slop-capture])`;
  return (
    hostScrollbarCSS +
    `:where(${page},${page} body){margin:0;padding:0}` +
    `:where(${root},${root} body){width:100%;height:100%}` +
    `:where(${root} body){-webkit-user-select:none;user-select:none}` +
    `:where(${root} input,${root} textarea,${root} [contenteditable=""],${root} [contenteditable="true"],${root} [contenteditable="plaintext-only"]){-webkit-user-select:text;user-select:text}` +
    `:where(${root} [data-hitslop-root],${root} body *:has([data-hitslop-root])){height:100%;min-height:0}` +
    (stage.mode === "transparent" || stage.mode === "skin" ? `${root},${root} body{background:transparent}` : "") +
    (stage.mode === "skin" ? `:where(${root},${root} body){overflow:hidden}` : "")
  );
}

/** Installed by the page shell for native and disposable preview hosts. */
export function installPresentationStage(stage: PresentationStage): void {
  const install = () => {
    const root = document.documentElement;
    root.dataset.slopPresentation = stage.mode;
    if (stage.radius !== undefined) root.style.setProperty("--slop-window-radius", stage.radius);
    else root.style.removeProperty("--slop-window-radius");
    root.toggleAttribute("data-slop-resizable", stage.resizable);
    root.style.setProperty("--slop-window-width", `${stage.width}px`);
    root.style.setProperty("--slop-window-height", `${stage.height}px`);
    let style = document.querySelector<HTMLStyleElement>("style[data-hitslop-host]");
    if (!style) {
      style = document.createElement("style");
      style.dataset.hitslopHost = "";
      (document.head || root).appendChild(style);
    }
    style.textContent = presentationStageCSS(stage);
  };
  if (document.documentElement) install();
  else document.addEventListener("DOMContentLoaded", install, { once: true });
}
