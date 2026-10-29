import type { ViewCaptureKind } from "../../wire/page.generated";

/** The icon canvas's side in CSS pixels. */
export const iconCanvas = 512;

/**
 * The platform stylesheet a static capture renders with, after the app's own CSS and theme:
 * the page reset, each role's canvas and the hide rule for `data-tiny-export="hide"`
 * (editing controls a capture must not draw). `tiny dev` applies the same rules to its
 * browser layouts of Preview and Icon, and the widget reset to its Widget.
 */
export const viewCaptureCSS = (kind: ViewCaptureKind) =>
  `html,body{margin:0;padding:0;width:100%;height:${kind === "icon" ? `${iconCanvas}px` : "auto"};min-height:0;overflow:visible;background:transparent}*{box-sizing:border-box}[data-tiny-capture-target="icon"]{width:${iconCanvas}px;height:${iconCanvas}px;display:grid;place-items:center}[data-tiny-capture-target="preview"],[data-tiny-capture-target="export"]{width:100%;display:flow-root}[data-tiny-export="hide"]{display:none!important}`;
export const widgetCaptureCSS =
  "html,body{margin:0;width:100%;height:100%;overflow:hidden}*{box-sizing:border-box}";
