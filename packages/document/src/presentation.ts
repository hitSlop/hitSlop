import type { SlopPresentation } from "@hitslop/schema";
import { DefaultWindowRadius } from "@hitslop/schema/constants";

/** Host-supplied initial geometry, not the current viewport or a live resize API. */
type PresentationStage = {
  mode: "standard" | "transparent" | "skin";
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
    mode: presentation.background === "transparent" ? "transparent" : "standard",
    width: presentation.width,
    height: presentation.height,
    resizable: presentation.resizable ?? true,
    radius: typeof presentation.shape === "object" ? "0" : (presentation.shape ?? DefaultWindowRadius),
  };
}

const hostScrollbarCSS =
  "*{scrollbar-width:none!important}*::-webkit-scrollbar{width:0!important;height:0!important;display:none!important}";

/**
 * The window is the stage in every mode: html, body and the Slop root fill it, and
 * the app lays out inside. Sizing is zero-specificity so authors can override it.
 * Transparent and skin windows show native geometry or chrome behind the page,
 * overriding ordinary authored page backgrounds. Capture keeps the page reset
 * and lays out in normal flow.
 */
function presentationStageCSS(stage?: PresentationStage): string {
  if (!stage) return hostScrollbarCSS;
  const page = `html[data-slop-presentation="${stage.mode}"]`;
  const root = `${page}:not([data-slop-capture])`;
  return (
    hostScrollbarCSS +
    `:where(${page},${page} body){margin:0;padding:0}` +
    `:where(${root},${root} body){width:100%;height:100%}` +
    `:where(${root} [data-hitslop-root],${root} body *:has([data-hitslop-root])){height:100%;min-height:0}` +
    (stage.mode === "standard" ? "" : `${root},${root} body{background:transparent}`) +
    (stage.mode === "skin" ? `:where(${root},${root} body){overflow:hidden}` : "")
  );
}

/** Shared by WKWebView's document-start script and the disposable preview host. */
export function installPresentationStage(stage?: PresentationStage): void {
  const install = () => {
    const root = document.documentElement;
    if (stage) {
      root.dataset.slopPresentation = stage.mode;
      if (stage.radius !== undefined) root.style.setProperty("--slop-window-radius", stage.radius);
      else root.style.removeProperty("--slop-window-radius");
      root.toggleAttribute("data-slop-resizable", stage.resizable);
      root.style.setProperty("--slop-window-width", `${stage.width}px`);
      root.style.setProperty("--slop-window-height", `${stage.height}px`);
    }
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
