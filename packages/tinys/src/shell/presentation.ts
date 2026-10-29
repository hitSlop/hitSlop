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
 * The window is the stage in every mode: html, body and the Tiny root fill it, and
 * the app lays out inside. Sizing and selection defaults have zero specificity so
 * authors can override them. Editor UI is unselectable except for text fields and
 * editable content; a body or region rule can opt back into selection.
 * Transparent and skin windows show native geometry or chrome behind the page,
 * overriding ordinary authored page backgrounds. A glass window keeps them: a
 * translucent page background tints the frosted material behind it. Capture keeps
 * the page reset and lays out in normal flow.
 */
function presentationStageCSS(stage: PresentationStage): string {
  const page = `html[data-tiny-presentation="${stage.mode}"]`;
  const root = `${page}:not([data-tiny-capture])`;
  // The iPhone editor runs edge to edge. The native host publishes the real safe areas
  // (`__tiny.safeArea`); the iPhone preset stands in a notched phone's.
  const simulatedSafeArea = preview.host ? ";--tiny-safe-top:47px;--tiny-safe-bottom:34px" : "";
  return (
    hostScrollbarCSS +
    `${page}{--tiny-host-actions-width:0px;--tiny-host-actions-height:0px;--tiny-keyboard-inset:0px;--tiny-safe-top:0px;--tiny-safe-bottom:0px}` +
    `${root}[data-tiny-platform="ios"][data-tiny-device="phone"]{--tiny-host-actions-width:64px;--tiny-host-actions-height:44px${simulatedSafeArea}}` +
    `:where(${page},${page} body){margin:0;padding:0}` +
    `:where(${root},${root} body){width:100%;height:100%}` +
    `:where(${root} body){-webkit-user-select:none;user-select:none}` +
    `:where(${root}[data-tiny-platform="ios"] body){-webkit-tap-highlight-color:transparent;-webkit-touch-callout:none}` +
    `:where(${root}[data-tiny-platform="ios"] input,${root}[data-tiny-platform="ios"] textarea,${root}[data-tiny-platform="ios"] [contenteditable=""],${root}[data-tiny-platform="ios"] [contenteditable="true"],${root}[data-tiny-platform="ios"] [contenteditable="plaintext-only"]){-webkit-touch-callout:default}` +
    `${root}[data-tiny-platform="ios"][data-tiny-device="phone"] [data-tiny-notice]{bottom:calc(16px + max(var(--tiny-keyboard-inset),var(--tiny-safe-bottom)))}` +
    `:where(${root}[data-tiny-platform="ios"] button,${root}[data-tiny-platform="ios"] a,${root}[data-tiny-platform="ios"] [role="button"]){touch-action:manipulation}` +
    `:where(${root} input,${root} textarea,${root} [contenteditable=""],${root} [contenteditable="true"],${root} [contenteditable="plaintext-only"]){-webkit-user-select:text;user-select:text}` +
    `:where(${root} [data-tiny-root],${root} body *:has([data-tiny-root])){height:100%;min-height:0}` +
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
    root.dataset.tinyPresentation = stage.mode;
    if (stage.radius !== undefined) root.style.setProperty("--tiny-window-radius", stage.radius);
    else root.style.removeProperty("--tiny-window-radius");
    root.toggleAttribute("data-tiny-resizable", stage.resizable);
    root.style.setProperty("--tiny-window-width", `${stage.width}px`);
    root.style.setProperty("--tiny-window-height", `${stage.height}px`);
    let style = document.querySelector<HTMLStyleElement>("style[data-tiny-host]");
    if (!style) {
      style = document.createElement("style");
      style.dataset.tinyHost = "";
      (document.head || root).appendChild(style);
    }
    style.textContent = presentationStageCSS(stage);
  };
  if (document.documentElement) install();
  else document.addEventListener("DOMContentLoaded", install, { once: true });
}
