// A window reveals once its declared fonts settle; a page without web fonts never waits.
import { test, expect } from "bun:test";
import { fontsSettled } from "../src/view-lifecycle";

function page(faces: { load(): Promise<unknown> }[], ready: Promise<unknown>) {
  return { fonts: { size: faces.length, ready, forEach: (visit: (face: unknown) => void) => faces.forEach(visit) } } as unknown as Document;
}
const face = (load: () => Promise<unknown>) => {
  const loaded = { calls: 0, load: () => (loaded.calls++, load()) };
  return loaded;
};

test("a page that declares no fonts does not wait", async () => {
  await fontsSettled(page([], new Promise(() => {})), 10);
  await fontsSettled({} as Document, 10);
});

test("every declared face is loaded and readiness ends the wait", async () => {
  const faces = [face(() => Promise.resolve()), face(() => Promise.resolve())];
  let settle!: () => void;
  const ready = new Promise<void>((resolve) => { settle = resolve; });
  let done = false;
  const waiting = fontsSettled(page(faces, ready), 1_000).then(() => { done = true; });
  expect(faces.map((f) => f.calls)).toEqual([1, 1]);
  await Promise.resolve();
  expect(done).toBe(false);
  settle();
  await waiting;
  expect(done).toBe(true);
});

test("a face that fails to load does not fail the wait", async () => {
  const failing = face(() => Promise.reject(new Error("missing font file")));
  await fontsSettled(page([failing], Promise.resolve()), 1_000);
  expect(failing.calls).toBe(1);
});

test("fonts that never settle fail at the limit", async () => {
  const started = performance.now();
  await expect(fontsSettled(page([face(() => new Promise(() => {}))], new Promise(() => {})), 20))
    .rejects.toThrow("Document fonts did not become ready");
  expect(performance.now() - started).toBeLessThan(1_000);
});
