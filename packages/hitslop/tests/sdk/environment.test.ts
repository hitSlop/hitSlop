import { afterEach, expect, test } from "bun:test";
import { createEnvironment } from "../../src/sdk/environment";

const before = globalThis.matchMedia;
afterEach(() => { globalThis.matchMedia = before; });

test("identity and viewport layout are independent; snapshots and subscription cleanup are stable", () => {
  const events = new EventTarget();
  let matches = false;
  globalThis.matchMedia = (() => {
    return {
      get matches() { return matches; },
      addEventListener(type: string, listener: EventListener) { events.addEventListener(type, listener); },
      removeEventListener(type: string, listener: EventListener) { events.removeEventListener(type, listener); },
    } as MediaQueryList;
  }) as typeof matchMedia;
  const state = createEnvironment({ platform: "macos", device: "desktop" });
  let changes = 0;
  const unsubscribe = state.environment.subscribe(() => changes++);
  const first = state.environment.current;
  expect(first).toEqual({ platform: "macos", device: "desktop", layout: "compact" });
  expect(Object.isFrozen(first)).toBe(true);
  events.dispatchEvent(new Event("change"));
  expect(state.environment.current).toBe(first);
  matches = true; events.dispatchEvent(new Event("change"));
  expect(state.environment.current).toEqual({ platform: "macos", device: "desktop", layout: "regular" });
  expect(first.layout).toBe("compact");
  state.setHost({ platform: "ios", device: "tablet" });
  expect(changes).toBe(2);
  unsubscribe(); unsubscribe();
  state.setHost({ platform: "ios", device: "phone" });
  expect(changes).toBe(2);
  state.dispose(); state.dispose();
  matches = false; events.dispatchEvent(new Event("change"));
  expect(state.environment.current.layout).toBe("regular");
  const fallback = createEnvironment();
  expect(fallback.environment.current).toEqual({ platform: "unknown", device: "unknown", layout: "compact" });
  fallback.dispose();
});
