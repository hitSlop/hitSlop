// A window reveals once its declared fonts settle; a page without web fonts never waits.
import { expect, jest, test } from "bun:test";
import { fontsSettled } from "../src/view-lifecycle";

test("every declared face is loaded, readiness ends the wait, and fonts that never settle fail at the limit", async () => {
  const page = (faces: { load(): Promise<unknown> }[], ready: Promise<unknown>) =>
    ({ fonts: { size: faces.length, ready, forEach: (visit: (face: unknown) => void) => faces.forEach(visit) } }) as unknown as Document;
  const loads: string[] = [];
  const face = (name: string, load: () => Promise<unknown>) => ({ load: () => (loads.push(name), load()) });
  // No declared fonts: nothing to wait for, even if `ready` never settles.
  await fontsSettled(page([], new Promise(() => {})));
  // A face that fails to load still settles `ready`; the browser falls back.
  await fontsSettled(page([face("present", () => Promise.resolve()), face("missing", () => Promise.reject(new Error("missing font file")))], Promise.resolve()));
  expect(loads).toEqual(["present", "missing"]);
  jest.useFakeTimers();
  try {
    const waiting = fontsSettled(page([face("stuck", () => new Promise(() => {}))], new Promise(() => {})));
    jest.advanceTimersByTime(12_000);
    await expect(waiting).rejects.toThrow("Document fonts did not become ready");
  } finally {
    jest.useRealTimers();
  }
});
