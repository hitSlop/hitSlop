import { afterEach, expect, test } from "bun:test";
import { ownerHaptics } from "../../src/shell/haptics";
import { haptics } from "../../src/sdk/app/haptics";
import { activate, deactivate } from "../../src/sdk/app/context";
import type { SlopContext } from "../../src/sdk/abi";

const root = globalThis as any;
const before = root.webkit;
afterEach(() => { root.webkit = before; });

test("Svelte feedback is immediate while the typed host request is still pending", async () => {
  const requests: unknown[] = [];
  let finish!: (reply: string) => void;
  root.webkit = { messageHandlers: { hitslop: { postMessage(body: string) {
    requests.push(JSON.parse(body));
    return new Promise<string>(resolve => { finish = resolve; });
  } } } };
  const ctx = { haptics: ownerHaptics() } as SlopContext;
  activate(ctx);
  try {
    expect(haptics.selection()).toBeUndefined();
    expect(requests).toEqual([{ method: "haptics.play", feedback: { kind: "selection" } }]);
    finish(JSON.stringify({ ok: true, method: "haptics.play" }));
    await Promise.resolve();
  } finally { deactivate(ctx); }
});

test("standard feedback reaches the bridge and a failed bridge stays silent", async () => {
  const requests: unknown[] = [];
  root.webkit = { messageHandlers: { hitslop: { async postMessage(body: string) {
    requests.push(JSON.parse(body));
    throw new Error("The page closed");
  } } } };
  const ctx = { haptics: ownerHaptics() } as SlopContext;
  activate(ctx);
  try {
    haptics.impact();
    haptics.impact("rigid");
    haptics.notification("success");
    await Bun.sleep(0);
    expect(requests).toEqual([
      { method: "haptics.play", feedback: { kind: "impact", style: "light" } },
      { method: "haptics.play", feedback: { kind: "impact", style: "rigid" } },
      { method: "haptics.play", feedback: { kind: "notification", notification: "success" } },
    ]);
  } finally { deactivate(ctx); }
});

test("an older host without feedback still runs the new Svelte helper", () => {
  const ctx = {} as SlopContext;
  activate(ctx);
  try {
    expect(haptics.selection()).toBeUndefined();
    expect(haptics.impact("soft")).toBeUndefined();
    expect(haptics.notification("error")).toBeUndefined();
  } finally { deactivate(ctx); }
});
