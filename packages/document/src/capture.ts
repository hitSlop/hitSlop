import type { CaptureMode } from "./abi";
import type { CaptureTarget as Target } from "./abi";
export type { CaptureMode } from "./abi";
type CaptureState = {
  attribute: string | null;
  active: HTMLElement | null;
  selection: Range[];
  inputSelection?: [number | null, number | null, "forward" | "backward" | "none" | null];
  scroll: [Element, number, number][];
  windowScroll: [number, number];
  style: HTMLStyleElement;
  target?: Target | undefined;
  block: (event: Event) => void;
  controller: AbortController;
  imageLoading: Map<HTMLImageElement, string | null>;
  inputs: { element: HTMLElement; display: string; replacement: HTMLElement }[];
};
const interactionEvents = ["pointerdown", "click", "keydown", "wheel", "touchstart"];
const timeoutMS = 10_000;

function createCaptureController() {
  const targets = new Map<"icon" | "export", Target>();
  const preparations = new Set<(mode: CaptureMode, signal: AbortSignal) => void | Promise<void>>();
  /** The capture in progress; there is at most one. */
  let session: { token: string; state: CaptureState } | undefined;
  const active = (token: string) => (session?.token === token ? session.state : undefined);
  const bounded = async <T>(work: Promise<T>, signal: AbortSignal): Promise<T> => {
    let timer: ReturnType<typeof setTimeout>;
    const timeout = new Promise<never>((_, reject) => {
      timer = setTimeout(
        () =>
          reject(
            new Error("Capture timed out waiting for content, fonts, images, or stable layout"),
          ),
        timeoutMS,
      );
    });
    try {
      signal.throwIfAborted();
      const result = await Promise.race([work, timeout]);
      signal.throwIfAborted();
      return result;
    } finally {
      clearTimeout(timer!);
    }
  };
  const measure = (state: CaptureState) => {
    const target = state.target?.element;
    if (target) {
      const rect = target.getBoundingClientRect();
      return {
        x: rect.x,
        y: rect.y,
        width: Math.ceil(Math.max(rect.width, target.scrollWidth)),
        height: Math.ceil(Math.max(rect.height, target.scrollHeight)),
        dedicated: true,
      };
    }
    const root = document.documentElement;
    const body = document.body;
    return {
      x: 0,
      y: 0,
      width: window.innerWidth,
      height: Math.ceil(
        Math.max(
          root.scrollHeight,
          root.offsetHeight,
          root.clientHeight,
          body?.scrollHeight ?? 0,
          body?.offsetHeight ?? 0,
        ),
      ),
      dedicated: false,
    };
  };
  /** Waits for fonts, images and a stable layout; returns the settled measurement. */
  const settle = async (token: string) => {
    const state = active(token);
    if (!state) throw new Error("Capture session is no longer active");
    return bounded(
      (async () => {
        await document.fonts.ready;
        state.controller.signal.throwIfAborted();
        const root = state.target?.element ?? document;
        const images = [...root.querySelectorAll("img")].filter(
          (image) => image.getClientRects().length > 0,
        );
        for (const image of images) {
          if (!state.imageLoading.has(image))
            state.imageLoading.set(image, image.getAttribute("loading"));
          image.loading = "eager";
        }
        await Promise.all(images.map((image) => image.decode()));
        // Stable once three ticks in a row measure what the previous one did.
        let current = measure(state);
        let previous = JSON.stringify(current);
        for (let stable = 0; stable < 3; ) {
          await new Promise((resolve) => setTimeout(resolve, 40));
          state.controller.signal.throwIfAborted();
          current = measure(state);
          const next = JSON.stringify(current);
          stable = next === previous ? stable + 1 : 0;
          previous = next;
        }
        return current;
      })(),
      state.controller.signal,
    );
  };
  const restore = async (token: string) => {
    const state = active(token);
    if (!state) return;
    state.controller.abort();
    // Restore host-owned state even when an author's teardown fails.
    try {
      await bounded(Promise.resolve(state.target?.restore()), new AbortController().signal);
    } finally {
      state.target?.element.removeAttribute("data-hitslop-active-target");
      if (state.attribute === null) document.documentElement.removeAttribute("data-slop-capture");
      else document.documentElement.setAttribute("data-slop-capture", state.attribute);
      state.style.remove();
      for (const { element, display, replacement } of state.inputs) {
        replacement.remove();
        element.style.display = display;
      }
      for (const [image, loading] of state.imageLoading) {
        if (loading === null) image.removeAttribute("loading");
        else image.setAttribute("loading", loading);
      }
      for (const event of interactionEvents) window.removeEventListener(event, state.block, true);
      state.active?.focus({ preventScroll: true });
      if (
        state.inputSelection &&
        (state.active instanceof HTMLInputElement || state.active instanceof HTMLTextAreaElement)
      ) {
        const [start, end, direction] = state.inputSelection;
        try {
          state.active.setSelectionRange(start, end, direction ?? undefined);
        } catch {
          /* Non-text inputs. */
        }
      } else {
        const selection = window.getSelection();
        selection?.removeAllRanges();
        for (const range of state.selection) {
          try {
            selection?.addRange(range);
          } catch {
            /* Detached selection. */
          }
        }
      }
      for (const [element, left, top] of state.scroll) {
        element.scrollLeft = left;
        element.scrollTop = top;
      }
      window.scrollTo(...state.windowScroll);
      session = undefined;
    }
  };
  return {
    registerTarget(kind: "icon" | "export", target: Target) {
      if (targets.has(kind)) throw new Error(`Expected one ${kind} capture target`);
      targets.set(kind, target);
      return () => {
        if (targets.get(kind) === target) targets.delete(kind);
      };
    },
    onPrepare(handler: (mode: CaptureMode, signal: AbortSignal) => void | Promise<void>) {
      preparations.add(handler);
      return () => {
        preparations.delete(handler);
      };
    },
    async begin(token: string, mode: CaptureMode) {
      if (session) throw new Error("Another capture is already in progress");
      const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      const selection = window.getSelection();
      const style = document.createElement("style");
      style.textContent =
        '*{animation:none!important;transition:none!important;caret-color:transparent!important;scroll-behavior:auto!important}html[data-slop-capture="static"] [data-slop-export="hide"]{display:none!important}';
      const state: CaptureState = {
        attribute: document.documentElement.getAttribute("data-slop-capture"),
        active,
        selection: selection
          ? Array.from({ length: selection.rangeCount }, (_, i) =>
              selection.getRangeAt(i).cloneRange(),
            )
          : [],
        scroll: [...document.querySelectorAll("*")]
          .filter((e) => e.scrollTop || e.scrollLeft)
          .map((e) => [e, e.scrollLeft, e.scrollTop]),
        windowScroll: [window.scrollX, window.scrollY],
        style,
        block: (event) => {
          if (event.isTrusted) {
            event.preventDefault();
            event.stopImmediatePropagation();
          }
        },
        controller: new AbortController(),
        imageLoading: new Map(),
        inputs: [],
      };
      if (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement)
        state.inputSelection = [
          active.selectionStart,
          active.selectionEnd,
          active.selectionDirection,
        ];
      session = { token, state };
      try {
        for (const event of interactionEvents)
          window.addEventListener(event, state.block, { capture: true, passive: false });
        document.head.append(style);
        active?.blur();
        document.documentElement.setAttribute(
          "data-slop-capture",
          mode === "icon" ? "icon" : "static",
        );
        state.target = targets.get(mode === "icon" ? "icon" : "export");
        if (state.target) {
          state.target.element.setAttribute("data-hitslop-active-target", "");
          style.textContent +=
            "html,body{margin:0!important;padding:0!important;width:100%!important;background:transparent!important}body>:not([data-hitslop-active-target]){display:none!important}";
        }
        if (mode === "icon") style.textContent += "html,body{background:transparent!important}";
        await bounded(
          (async () => {
            for (const prepare of preparations) {
              await prepare(mode, state.controller.signal);
              state.controller.signal.throwIfAborted();
            }
            await state.target?.prepare();
            state.controller.signal.throwIfAborted();
          })(),
          state.controller.signal,
        );
        window.scrollTo(0, 0);
        if (!state.target)
          for (const input of document.querySelectorAll<HTMLInputElement | HTMLTextAreaElement>(
            'input:not([type="checkbox"]):not([type="radio"]),textarea',
          )) {
            if (!input.getClientRects().length) continue;
            const replacement = document.createElement("span");
            replacement.className = input.className;
            replacement.textContent = input.value;
            replacement.style.whiteSpace = "pre-wrap";
            replacement.style.overflowWrap = "anywhere";
            replacement.style.display = "block";
            replacement.style.height = "auto";
            state.inputs.push({ element: input, display: input.style.display, replacement });
            input.after(replacement);
            input.style.display = "none";
          }
        return await settle(token);
      } catch (error) {
        await restore(token);
        throw error;
      }
    },
    settle,
    restore,
  };
}

declare global {
  interface Window {
    __hitslopCapture?: ReturnType<typeof createCaptureController>;
  }
}
export function captureController() {
  return (window.__hitslopCapture ??= createCaptureController());
}
if (typeof window !== "undefined") captureController();
