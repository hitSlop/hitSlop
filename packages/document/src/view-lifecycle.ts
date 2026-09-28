import type { ObjectNode } from "./schema";
import type { OwnerSession as Session } from "./owner/session";
import type { Request } from "./session-types";
import type { createCaptureController } from "./capture";

/** A mounted view; rendered must wait for pending framework updates. */
export interface DocumentView {
  rendered(): void | Promise<void>;
  unmount(): void | Promise<void>;
}

/** Shared visible-session lifecycle. Headless sessions never mount a view. */
export async function mountViewLifecycle<N extends ObjectNode>(options: {
  mount(): DocumentView | Promise<DocumentView>;
  document: unknown;
  target: HTMLElement;
  session: Pick<
    Session,
    "flush" | "handle" | "prepareClose" | "cancelClose" | "close" | "discardPending"
  >;
  capture: Pick<ReturnType<typeof createCaptureController>, "begin" | "restore">;
  recovered?: () => Promise<unknown>;
}) {
  const { mount, target, session, capture, recovered } = options;
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
    request: (request: Request) => session.handle(request),
    flush: () => session.flush(),
    prepareClose: async () => {
      await session.prepareClose();
      target.inert = true;
    },
    cancelClose: () => {
      session.cancelClose();
      target.inert = false;
    },
    discardPending: async () => {
      target.inert = true;
      try {
        await session.discardPending();
        await view.unmount();
        view = await mount();
        await view.rendered();
      } finally {
        target.inert = false;
      }
    },
    retrySave: async () => {
      try {
        await session.flush();
        return true;
      } catch {
        return false;
      }
    },
    close: async () => {
      await session.close();
      try {
        await view.unmount();
      } catch (error) {
        console.error(error);
      }
    },
    captureBegin: async (token: string) => {
      await session.prepareClose();
      target.inert = true;
      try {
        await view.rendered();
        return await capture.begin(token, "export");
      } catch (error) {
        session.cancelClose();
        target.inert = false;
        throw error;
      }
    },
    captureRestore: async (token: string) => {
      try {
        return await capture.restore(token);
      } finally {
        session.cancelClose();
        target.inert = false;
      }
    },
  } satisfies import("./runtime-handle").SlopRuntimeHandle;
}
