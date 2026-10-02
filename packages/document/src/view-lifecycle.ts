import type { OwnerDocument } from "./owner/document";
import type { ThemeController } from "./theme-runtime";
import type { SlopView } from "./abi";

/** A mounted view; rendered must wait for pending framework updates. */
type DocumentView = Required<SlopView>;

/**
 * Waits until the page's web fonts have loaded, so a window reveals with its final text.
 * Packages carry their fonts as local files, so every declared face is loaded directly:
 * cheaper than forcing layout of a large hidden page to find the faces it uses. A page
 * that declares none uses system fonts and has nothing to wait for.
 */
export async function fontsSettled(page: Document, limit = 12_000) {
  if (!page.fonts?.size) return;
  // A face that fails to load still settles `ready`; the browser falls back.
  page.fonts.forEach((face) => void face.load().catch(() => {}));
  let timer: ReturnType<typeof setTimeout> | undefined;
  const deadline = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error("Document fonts did not become ready")), limit);
  });
  try {
    await Promise.race([page.fonts.ready, deadline]);
  } finally {
    clearTimeout(timer);
  }
}

/** The visible page's lifecycle, called by the native host through `globalThis.__slop`. */
export async function mountViewLifecycle(options: {
  mount(): DocumentView | Promise<DocumentView>;
  document: Pick<OwnerDocument<any>, "flush" | "undo" | "redo" | "prepareClose" | "cancelClose">;
  theme: Pick<ThemeController, "load">;
  target: HTMLElement;
  recovered?: () => Promise<unknown>;
}) {
  const { mount, target, document: doc, theme, recovered } = options;
  let view = await mount();
  await view.rendered();
  await fontsSettled(target.ownerDocument);
  return {
    reloadInterface: async () => {
      await doc.flush();
      let failure: { error: unknown } | undefined;
      const failed = (event: Event) => {
        failure = { error: (event as CustomEvent).detail };
      };
      target.ownerDocument.addEventListener("hitslop:render-error", failed);
      try {
        await view.unmount();
        view = await mount();
        await view.rendered();
        if (failure) throw failure.error;
        await fontsSettled(target.ownerDocument);
        await recovered?.();
      } finally {
        target.ownerDocument.removeEventListener("hitslop:render-error", failed);
      }
    },
    /** Theme writes are validated and saved by the native owner; the page only applies them. */
    applyTheme: (overrides: Record<string, string>) => theme.load(overrides),
    flush: () => doc.flush(),
    undo: () => doc.undo(),
    redo: () => doc.redo(),
    /** Drains pending page work behind a barrier; the host then saves and closes. */
    prepareClose: async () => {
      await doc.prepareClose();
      target.inert = true;
    },
    cancelClose: () => {
      doc.cancelClose();
      target.inert = false;
    },
    /** Called once the native owner has saved and released the document: only unmounts. */
    close: async () => {
      try {
        await view.unmount();
      } catch (error) {
        console.error(error);
      }
    },
  } satisfies import("./page-handle").SlopPageHandle;
}
