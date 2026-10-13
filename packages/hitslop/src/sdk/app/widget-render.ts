import { render } from "svelte/server";
import type { Component } from "svelte";
import type { SlopContext } from "../abi";
import type { WidgetRenderRequest } from "../../wire/page.generated";
import { activate, deactivate } from "./context";
import { captureHTML } from "./capture-html";

function freeze(value: unknown): unknown {
  if (value && typeof value === "object") {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
  return value;
}
const refuse = (): never => {
  throw new Error(
    "Native capture views are read-only; read doc.current instead of document handles or writes",
  );
};

/** A private, synchronous entry point. Each batch runs in a fresh restricted realm. */
export function widgetRenderer(definition: object, component: Component, css: string) {
  return (json: string) => {
    const request: WidgetRenderRequest = JSON.parse(json);
    return withReadOnlyDocument(definition, request.snapshot, request.resources, () => {
      const theme = Object.entries(request.theme)
        .map(([key, value]) => `--slop-${key}:${value}`)
        .join(";");
      return JSON.stringify(
        request.viewports.map((viewport) => {
          const result = render(component, { props: viewport });
          return {
            width: viewport.width,
            height: viewport.height,
            html: `<html data-slop-renderer="true" data-slop-capture="widget"><body>${captureHTML(result.body, request.theme)}</body></html>`,
            css: `${css}\n:root{${theme}}\nhtml,body{margin:0;width:100%;height:100%;overflow:hidden}*{box-sizing:border-box}`,
          };
        }),
      );
    });
  };
}

export function withReadOnlyDocument<T>(
  definition: object,
  snapshot: unknown,
  resources: Record<string, string>,
  body: () => T,
): T {
  const document = {
    current: freeze(snapshot),
    change: refuse,
    flush: refuse,
    undo: refuse,
    redo: refuse,
    at: refuse,
    get fields() {
      return refuse();
    },
  };
  const images = Object.freeze({ ...resources });
  const context = {
    document,
    attachments: {
      url(id: string) {
        if (
          typeof id !== "string" ||
          !Object.hasOwn(images, id) ||
          typeof images[id] !== "string" ||
          !images[id]
        )
          throw new Error(`Capture attachment ${id} is missing or is not a supported image`);
        return `slop-attachment:${id}`;
      },
      import: refuse,
      read: refuse,
    },
  } as unknown as SlopContext;
  activate(context, definition, document);
  try {
    return body();
  } finally {
    deactivate(context);
  }
}
