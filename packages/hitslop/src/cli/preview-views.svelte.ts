// `slop dev` only, never part of a slop: the app frame mounts one view at a time, as the
// preview page around it selects. The editor is the app's own entry; Preview, Icon and Widget
// render the live document through the props their captures render from.
import { mount, tick, unmount, type Component } from "svelte";
import type { SlopApp } from "../sdk/abi";
import type { CaptureMode, CaptureProps } from "../sdk/app/capture-props";
import { iconCanvas, viewCaptureCSS, widgetCaptureCSS } from "../sdk/app/capture-css";
import type { Definition, ObjectNode } from "../sdk/schema";
import { WidgetViewports } from "../schema/constants";
import type { WidgetFamily, WidgetViewport } from "../wire/page.generated";

type Props = CaptureProps<Definition<ObjectNode>>;
/** What the preview page selects, with `{ type: "slop:view", view }`. */
export type PreviewView = "editor" | "preview" | "icon" | WidgetFamily;
/** The frame's answer: the canvas a static view renders on, which the page sizes its plain
 * surface to, or no size for the editor, which keeps the window. */
export type PreviewSurface = {
  type: "slop:surface";
  width?: number;
  height?: number;
  /** Content extends beyond a fixed canvas. */
  clipped?: boolean;
};

export function previewViews(
  editor: SlopApp,
  views: {
    preview: Component<Props & { mode: CaptureMode }>;
    icon: Component<Props & { mode: CaptureMode }>;
    widget?: Component<Props & WidgetViewport>;
  },
): SlopApp {
  return {
    descriptor: editor.descriptor,
    async mount(ctx, target) {
      // Static views read the editor's snapshot, frozen like a capture's, and its URLs.
      let snapshot = $state.raw(ctx.document.current);
      // Lays the shown static view out again once its content may have changed.
      let refresh = () => {};
      const unsubscribe = ctx.document.subscribe(() => {
        snapshot = ctx.document.current;
        void tick().then(() => refresh());
      });
      const propsFor = (input: { mode: CaptureMode } | WidgetViewport) => ({
        ...input,
        get snapshot() {
          return snapshot as Props["snapshot"];
        },
        attachmentURL: (id: string) => ctx.attachments.url(id),
      });
      const root = document.documentElement;
      let requested: PreviewView = "editor";
      let shown: PreviewView | undefined;
      let sent = "";
      const post = (surface: Omit<PreviewSurface, "type">) => {
        const message: PreviewSurface = { type: "slop:surface", ...surface };
        if (JSON.stringify(message) === sent) return;
        sent = JSON.stringify(message);
        parent.postMessage(message, location.origin);
      };
      const cleanups: (() => unknown)[] = [];
      const leave = async () => {
        for (const cleanup of cleanups.splice(0).reverse()) await cleanup();
      };
      const show = async () => {
        const view = requested;
        // The page selects again after each update.
        if (view === shown) return tick().then(() => refresh());
        shown = view;
        await leave();
        if (view === "editor") {
          const mounted = await editor.mount(ctx, target);
          cleanups.push(() => mounted.unmount?.());
          post({});
          return;
        }
        // The attributes and stylesheet of the capture's static HTML (capture-render.ts,
        // widget-render.ts): the app's CSS keys its capture layouts to them.
        const viewport = WidgetViewports.find((size) => size.family === view);
        root.dataset.slopCapture = viewport ? "widget" : view === "icon" ? "icon" : "static";
        cleanups.push(() => delete root.dataset.slopCapture);
        const style = document.head.appendChild(document.createElement("style"));
        style.textContent = viewport ? widgetCaptureCSS : viewCaptureCSS(view as CaptureMode);
        cleanups.push(() => style.remove());
        // A widget fills the page; a preview or icon renders in its capture target.
        const canvas = viewport ? target : target.appendChild(document.createElement("div"));
        if (canvas !== target) {
          canvas.dataset.slopCaptureTarget = view;
          cleanups.push(() => canvas.remove());
        }
        try {
          const component = viewport ? views.widget : views[view as "preview" | "icon"];
          if (!component) throw new Error("This slop has no Widget.svelte");
          const props = propsFor(viewport ?? { mode: view as CaptureMode });
          const instance = mount(component as Component<typeof props>, { target: canvas, props });
          cleanups.push(() => unmount(instance));
        } catch (error) {
          // The page selects again after each update, which retries the fixed view.
          shown = undefined;
          ctx.reportError(error);
          const alert = canvas.appendChild(document.createElement("p"));
          alert.role = "alert";
          alert.textContent = `Could not render this view: ${error instanceof Error ? error.message : error}`;
          cleanups.push(() => alert.remove());
        }
        // Production lays a preview out at the window's size, where vh resolves, and renders
        // it at the window's width and its content's height, at most three widths or the
        // window's height. A measurement waits for the page to give this frame that size.
        const host = (name: string) => parseFloat(root.style.getPropertyValue(name));
        const [width, height] = [host("--slop-window-width"), host("--slop-window-height")];
        const content = () => Math.ceil(canvas.getBoundingClientRect().height);
        let measuring = true;
        let steady = -1;
        const layout = () => {
          if (viewport || view === "icon")
            post({
              width: viewport?.width ?? iconCanvas,
              height: viewport?.height ?? iconCanvas,
              clipped:
                canvas.scrollWidth > canvas.clientWidth ||
                canvas.scrollHeight > canvas.clientHeight,
            });
          else if (!measuring) steady = content();
          else if (innerWidth !== width || innerHeight !== height) post({ width, height });
          else {
            measuring = false;
            post({ width, height: Math.min(content(), Math.max(height, width * 3)) });
          }
        };
        const again = () => {
          if (!measuring && content() === steady) return;
          measuring = true;
          layout();
        };
        // A resize is the page answering. Loaded images, snapshots and updates measure again
        // when the content moved.
        refresh = again;
        addEventListener("resize", layout);
        document.addEventListener("load", again, true);
        cleanups.push(() => {
          refresh = () => {};
          removeEventListener("resize", layout);
          document.removeEventListener("load", again, true);
        });
        await tick();
        again();
      };
      let switching = Promise.resolve();
      const select = (view: PreviewView) => {
        requested = view;
        return (switching = switching.then(show).catch((error) => {
          shown = undefined;
          ctx.reportError(error);
        }));
      };
      // Only the preview page around this frame selects, and only views this app has.
      const receive = ({ source, origin, data }: MessageEvent) => {
        if (
          source === parent &&
          origin === location.origin &&
          data?.type === "slop:view" &&
          (["editor", "preview", "icon"].includes(data.view) ||
            (views.widget && WidgetViewports.some((size) => size.family === data.view)))
        )
          void select(data.view);
      };
      addEventListener("message", receive);
      await select("editor");
      return {
        rendered: tick,
        async unmount() {
          removeEventListener("message", receive);
          unsubscribe();
          await switching;
          await leave();
        },
      };
    },
  };
}
