import type { CaptureMode } from "../sdk/abi";
import type { CaptureTarget as Target } from "../sdk/abi";
export type { CaptureMode } from "../sdk/abi";
type CaptureState = {
  style: HTMLStyleElement;
  target?: Target | undefined;
  controller: AbortController;
  restoring?: Promise<void>;
  inputs: { input: HTMLElement; replacement: HTMLElement; display: string; priority: string }[];
};
const timeoutMS = 10_000;
type Preparation = (mode: CaptureMode, signal: AbortSignal) => void | Promise<void>;

/** Captures run in disposable read-only pages, never in the interactive editor. */
export function createCaptureController() {
  const targets = new Map<"icon" | "export", Target>();
  const preparations = new Set<Preparation>();
  /** The capture in progress; there is at most one. */
  let session: { token: string; state: CaptureState } | undefined;
  const active = (token: string) => (session?.token === token ? session.state : undefined);
  const bounded = async <T>(work: Promise<T>, signal: AbortSignal): Promise<T> => {
    let timer: ReturnType<typeof setTimeout>;
    let abort: () => void;
    const cancelled = new Promise<never>((_, reject) => {
      abort = () => reject(signal.reason ?? new DOMException("Capture cancelled", "AbortError"));
      signal.addEventListener("abort", abort, { once: true });
    });
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
      const result = await Promise.race([work, timeout, cancelled]);
      signal.throwIfAborted();
      return result;
    } finally {
      clearTimeout(timer!);
      signal.removeEventListener("abort", abort!);
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
    // One restoration even when cancellation and a failed begin race each other.
    state.restoring ??= Promise.resolve().then(async () => {
      state.controller.abort();
      try {
        await bounded(Promise.resolve(state.target?.restore()), new AbortController().signal);
      } finally {
        for (const { input, replacement, display, priority } of state.inputs) {
          replacement.remove();
          input.style.setProperty("display", display, priority);
        }
        state.target?.element.removeAttribute("data-slop-active-target");
        document.documentElement.removeAttribute("data-slop-capture");
        state.style.remove();
        if (active(token) === state) session = undefined;
      }
    });
    return state.restoring;
  };
  return {
    registerTarget(kind: "icon" | "export", target: Target) {
      if (targets.has(kind)) throw new Error(`Expected one ${kind} capture target`);
      targets.set(kind, target);
      return () => {
        if (targets.get(kind) === target) targets.delete(kind);
      };
    },
    onPrepare(handler: Preparation) {
      preparations.add(handler);
      return () => {
        preparations.delete(handler);
      };
    },
    async begin(token: string, mode: CaptureMode) {
      if (session) throw new Error("Another capture is already in progress");
      const style = document.createElement("style");
      style.textContent =
        '*{animation:none!important;transition:none!important;caret-color:transparent!important;scroll-behavior:auto!important}html[data-slop-capture="static"] [data-slop-export="hide"]{display:none!important}';
      const state: CaptureState = {
        style,
        controller: new AbortController(),
        inputs: [],
      };
      session = { token, state };
      try {
        document.head.append(style);
        document.documentElement.setAttribute(
          "data-slop-capture",
          mode === "icon" ? "icon" : "static",
        );
        state.target = targets.get(mode === "icon" ? "icon" : "export");
        if (state.target) {
          state.target.element.setAttribute("data-slop-active-target", "");
          style.textContent +=
            "html,body{margin:0!important;padding:0!important;width:100%!important;background:transparent!important}body>:not([data-slop-active-target]){display:none!important}";
        }
        if (mode === "icon") style.textContent += "html,body{background:transparent!important}";
        await bounded(
          (async () => {
            const prepared = new Set<Preparation>();
            const prepareViews = async () => {
              for (const prepare of preparations) {
                if (prepared.has(prepare)) continue;
                prepared.add(prepare);
                await prepare(mode, state.controller.signal);
                state.controller.signal.throwIfAborted();
              }
            };
            await prepareViews();
            await state.target?.prepare();
            state.controller.signal.throwIfAborted();
            // Mounting a dedicated target installs its own motion/layout hooks.
            await prepareViews();
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
            input.after(replacement);
            state.inputs.push({ input, replacement, display: input.style.getPropertyValue("display"), priority: input.style.getPropertyPriority("display") });
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
