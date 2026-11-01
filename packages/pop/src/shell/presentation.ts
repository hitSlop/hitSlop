import type { WindowDefinition } from "../wire/app.generated";
import { preview } from "./preview";

/** Host-supplied initial geometry, not the current viewport or a live resize API. */
type PresentationStage = {
  mode: "standard" | "transparent" | "glass" | "skin";
  width: number;
  height: number;
  resizable: boolean;
  radius?: string;
};

/** Presents the frame resolved by Rust, shared with the native host. */
export function presentationStage({ width, height, frame }: WindowDefinition): PresentationStage {
  if (frame.kind === "skin") return { mode: "skin", width, height, resizable: false };
  return {
    mode: frame.frame.background ?? "standard",
    width,
    height,
    resizable: frame.frame.resizable,
    radius: frame.frame.radius,
  };
}

const hostScrollbarCSS =
  "*{scrollbar-width:none!important}*::-webkit-scrollbar{width:0!important;height:0!important;display:none!important}";

/**
 * The window is the stage in every mode: html, body and the Pop root fill it, and
 * the app lays out inside. Sizing and selection defaults have zero specificity so
 * authors can override them. Editor UI is unselectable except for text fields and
 * editable content; a body or region rule can opt back into selection.
 * Transparent and skin windows show native geometry or chrome behind the page,
 * overriding ordinary authored page backgrounds. A glass window keeps them: a
 * translucent page background tints the frosted material behind it. Capture keeps
 * the page reset and lays out in normal flow.
 */
function presentationStageCSS(stage: PresentationStage): string {
  const page = `html[data-pop-presentation="${stage.mode}"]`;
  const root = `${page}:not([data-pop-capture])`;
  // The iPhone editor runs edge to edge. The native host publishes the real safe areas
  // (`__pop.safeArea`); the iPhone preset stands in a notched phone's.
  const simulatedSafeArea = preview.host ? ";--pop-safe-top:47px;--pop-safe-bottom:34px" : "";
  return (
    hostScrollbarCSS +
    `${page}{--pop-host-actions-width:0px;--pop-host-actions-height:0px;--pop-keyboard-inset:0px;--pop-safe-top:0px;--pop-safe-bottom:0px}` +
    `${root}[data-pop-platform="ios"][data-pop-device="phone"]{--pop-host-actions-width:64px;--pop-host-actions-height:44px${simulatedSafeArea}}` +
    `:where(${page},${page} body){margin:0;padding:0}` +
    `:where(${root},${root} body){width:100%;height:100%}` +
    `:where(${root} body){-webkit-user-select:none;user-select:none}` +
    `:where(${root}[data-pop-platform="ios"] body){-webkit-tap-highlight-color:transparent;-webkit-touch-callout:none}` +
    `:where(${root}[data-pop-platform="ios"] input,${root}[data-pop-platform="ios"] textarea,${root}[data-pop-platform="ios"] [contenteditable=""],${root}[data-pop-platform="ios"] [contenteditable="true"],${root}[data-pop-platform="ios"] [contenteditable="plaintext-only"]){-webkit-touch-callout:default}` +
    `${root}[data-pop-platform="ios"][data-pop-device="phone"] [data-pop-notice]{bottom:calc(16px + max(var(--pop-keyboard-inset),var(--pop-safe-bottom)))}` +
    `:where(${root}[data-pop-platform="ios"] button,${root}[data-pop-platform="ios"] a,${root}[data-pop-platform="ios"] [role="button"]){touch-action:manipulation}` +
    `:where(${root} input,${root} textarea,${root} [contenteditable=""],${root} [contenteditable="true"],${root} [contenteditable="plaintext-only"]){-webkit-user-select:text;user-select:text}` +
    `:where(${root} [data-pop-root],${root} body *:has([data-pop-root])){height:100%;min-height:0}` +
    (stage.mode === "transparent" || stage.mode === "skin"
      ? `${root},${root} body{background:transparent}`
      : "") +
    (stage.mode === "skin" ? `:where(${root},${root} body){overflow:hidden}` : "")
  );
}

/** Installed by the page shell for native and disposable preview hosts. */
export function installPresentationStage(stage: PresentationStage): void {
  const install = () => {
    const root = document.documentElement;
    root.dataset.popPresentation = stage.mode;
    if (stage.radius !== undefined) root.style.setProperty("--pop-window-radius", stage.radius);
    else root.style.removeProperty("--pop-window-radius");
    root.toggleAttribute("data-pop-resizable", stage.resizable);
    root.style.setProperty("--pop-window-width", `${stage.width}px`);
    root.style.setProperty("--pop-window-height", `${stage.height}px`);
    let style = document.querySelector<HTMLStyleElement>("style[data-pop-host]");
    if (!style) {
      style = document.createElement("style");
      style.dataset.popHost = "";
      (document.head || root).appendChild(style);
    }
    style.textContent = presentationStageCSS(stage);
  };
  if (document.documentElement) install();
  else document.addEventListener("DOMContentLoaded", install, { once: true });
}
