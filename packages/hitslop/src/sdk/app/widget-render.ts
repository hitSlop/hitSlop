import { render } from "svelte/server";
import type { Component } from "svelte";
import type { WidgetRenderRequest, WidgetViewport } from "../../wire/page.generated";
import type { Definition, ObjectNode } from "../schema";
import { captureProps, type CaptureProps } from "./capture-props";
import { captureHTML } from "./capture-html";
import { widgetCaptureCSS } from "./capture-css";

/** A private, synchronous entry point. Each batch runs in a fresh restricted realm. */
export function widgetRenderer(
  component: Component<CaptureProps<Definition<ObjectNode>> & WidgetViewport>,
  css: string,
) {
  return (json: string) => {
    const request: WidgetRenderRequest = JSON.parse(json);
    const props = captureProps(request.snapshot, request.resources);
    const theme = Object.entries(request.theme)
      .map(([key, value]) => `--slop-${key}:${value}`)
      .join(";");
    return JSON.stringify(
      request.viewports.map((viewport) => {
        const result = render(component, { props: { ...props, ...viewport } });
        return {
          width: viewport.width,
          height: viewport.height,
          html: `<html data-slop-renderer="true" data-slop-capture="widget"><body>${captureHTML(result.body, request.theme)}</body></html>`,
          css: `${css}\n:root{${theme}}\n${widgetCaptureCSS}`,
        };
      }),
    );
  };
}
