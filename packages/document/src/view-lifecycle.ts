import type { OwnerSession as Session } from "./owner/session";

/** A mounted view; rendered must wait for pending framework updates. */
export interface DocumentView {
  rendered(): void | Promise<void>;
  unmount(): void | Promise<void>;
}

/** The visible page's lifecycle, called by the native host through `globalThis.__slop`. */
export async function mountViewLifecycle(options: {
  mount(): DocumentView | Promise<DocumentView>;
  document: unknown;
  target: HTMLElement;
  session: Pick<Session, "flush" | "applyTheme" | "prepareClose" | "cancelClose">;
  recovered?: () => Promise<unknown>;
}) {
  const { mount, target, session, recovered } = options;
  let view = await mount();
  await view.rendered();
  return {
    reloadInterface: async () => {
      await session.flush();
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
        await recovered?.();
      } finally {
        target.ownerDocument.removeEventListener("hitslop:render-error", failed);
      }
    },
    applyTheme: (overrides: Record<string, string>) => session.applyTheme(overrides),
    flush: () => session.flush(),
    /** Drains pending page work behind a barrier; the host then saves and closes. */
    prepareClose: async () => {
      await session.prepareClose();
      target.inert = true;
    },
    cancelClose: () => {
      session.cancelClose();
      target.inert = false;
    },
    /** Called after the barrier: only unmounts. The native owner does the final save. */
    close: async () => {
      try {
        await view.unmount();
      } catch (error) {
        console.error(error);
      }
    },
  } satisfies import("./runtime-handle").SlopRuntimeHandle;
}
