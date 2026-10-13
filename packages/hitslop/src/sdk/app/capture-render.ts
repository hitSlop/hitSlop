import { render } from "svelte/server";
import type { Component } from "svelte";
import type { ViewRenderRequest } from "../../wire/page.generated";
import { withReadOnlyDocument } from "./widget-render";
import { captureHTML } from "./capture-html";

/** The authored role renders saved state once in a disposable synchronous realm. */
export function viewRenderer(definition: object, component: Component, css: string) {
  return (json: string) => {
    const request: ViewRenderRequest = JSON.parse(json);
    return withReadOnlyDocument(definition, request.snapshot, request.resources, () => {
      const theme = Object.entries(request.theme)
        .map(([key, value]) => `--slop-${key}:${value}`)
        .join(";");
      const icon = request.kind === "icon";
      const mode = request.kind === "preview" ? "preview" : "export";
      const html = captureHTML(
        render(component, { props: { mode: icon ? "icon" : mode } }).body,
        request.theme,
      );
      return JSON.stringify([
        {
          width: request.width,
          height: request.height,
          kind: request.kind,
          html: `<html data-slop-renderer="true" data-slop-capture="${icon ? "icon" : "static"}"><body><div data-slop-capture-target="${request.kind}">${html}</div></body></html>`,
          css: `${css}\n:root{${theme}}\nhtml,body{margin:0;padding:0;width:100%;height:${icon ? "512px" : "auto"};min-height:0;overflow:visible;background:transparent}*{box-sizing:border-box}[data-slop-capture-target="icon"]{width:512px;height:512px;display:grid;place-items:center}[data-slop-capture-target="preview"],[data-slop-capture-target="export"]{width:100%;display:flow-root}`,
        },
      ]);
    });
  };
}
