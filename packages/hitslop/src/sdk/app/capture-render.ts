import { render } from "svelte/server";
import type { Component } from "svelte";
import type { ViewRenderRequest } from "../../wire/page.generated";
import type { Definition, ObjectNode } from "../schema";
import { captureProps, type CaptureMode, type CaptureProps } from "./capture-props";
import { captureHTML } from "./capture-html";
import { viewCaptureCSS } from "./capture-css";

/** The authored role renders saved state once in a disposable synchronous realm. */
export function viewRenderer(
  component: Component<CaptureProps<Definition<ObjectNode>> & { mode: CaptureMode }>,
  css: string,
) {
  return (json: string) => {
    const request: ViewRenderRequest = JSON.parse(json);
    const theme = Object.entries(request.theme)
      .map(([key, value]) => `--slop-${key}:${value}`)
      .join(";");
    const icon = request.kind === "icon";
    const props = { ...captureProps(request.snapshot, request.resources), mode: request.kind };
    const html = captureHTML(render(component, { props }).body, request.theme);
    return JSON.stringify([
      {
        width: request.width,
        height: request.height,
        kind: request.kind,
        html: `<html data-slop-capture="${icon ? "icon" : "static"}"><body><div data-slop-capture-target="${request.kind}">${html}</div></body></html>`,
        css: `${css}\n:root{${theme}}\n${viewCaptureCSS(request.kind)}`,
      },
    ]);
  };
}
