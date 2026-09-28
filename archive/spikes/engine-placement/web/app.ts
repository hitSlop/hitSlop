import { verifyNative } from "../../hitslop-core/web-checks";
import { openRust } from "../../hitslop-core/renderer";
import { mount, tick } from "svelte";
import App from "./App.svelte";
import { call } from "./wire";
import type { Operation, Path } from "../contract";
const config = (globalThis as any).spikeConfig as {
  candidate: string;
  fixture: string;
  rows: number;
};
const start = performance.now();
let current: any;
function updateAt(value: any, path: Path, replacement: any): any {
  if (!path.length) return replacement;
  const [head, ...tail] = path;
  if (typeof head === "string")
    return { ...value, [head]: updateAt(value[head], tail, replacement) };
  if ("id" in head!) {
    const index = value.findIndex((r: any) => r.$id === head.id);
    if (index < 0) throw new Error("Missing publication row");
    const next = value.slice();
    next[index] = updateAt(value[index], tail, replacement);
    return next;
  }
  if ("key" in head!) return { ...value, [head.key]: updateAt(value[head.key], tail, replacement) };
  throw new Error("Unsupported publication path");
}
let adapter: any;
try {
  const schema = await fetch(`/${config.fixture}.schema.json`).then((r) => r.json());
  if (config.candidate === "rust-core") {
    adapter = await openRust(call); current = adapter.current();
  } else if (config.candidate.startsWith("native")) {
    current = await call("current");
    let revision = 0;
    adapter = {
      current: () => current,
      async submit(operations: Operation[]) {
        const p = await call("apply", { operations });
        if (p.revision <= revision) throw new Error("Out of order publication");
        revision = p.revision;
        for (const patch of p.patches) {
          let value = patch.value;
          if (patch.action === "move") {
            let list = current;
            for (const field of patch.path) list = list[field];
            list = list.slice();
            const [row] = list.splice(value.from, 1);
            list.splice(value.to, 0, row);
            value = list;
          }
          current = updateAt(current, patch.path, value);
        }
        return p;
      },
      flush: () => call("flush"),
      close: () => call("close"),
      snapshot: () => call("snapshot"),
    };
  } else {
    const module = await import(
      config.candidate === "baseline" ? "/wasm-baseline.js" : "/wasm-matched.js"
    );
    adapter = await module.open(schema);
    current = adapter.current();
  }
  const engineReadyMS = performance.now() - start;
  let ui: any;
  mount(App, {
    target: document.body,
    props: {
      adapter,
      initial: current,
      candidate: config.candidate,
      register: (api: any) => (ui = api),
    },
  });
  await tick();
  const frame = () =>
    new Promise<number>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve(performance.now()))),
    );
  await frame();
  const readyMS = performance.now() - start;
  const quantiles = (values: number[]) => {
    const a = values.toSorted((a, b) => a - b);
    return {
      median: a[Math.floor(a.length / 2)],
      p95: a[Math.min(a.length - 1, Math.ceil(a.length * 0.95) - 1)],
      max: a.at(-1),
      samples: values,
    };
  };
  const frames: number[] = [];
  let previous = performance.now(),
    sampling = false;
  function track(now: number) {
    if (sampling) frames.push(now - previous);
    previous = now;
    requestAnimationFrame(track);
  }
  requestAnimationFrame(track);
  (globalThis as any).spike = {
    adapter, ui, call,
    verifyNative:()=>verifyNative(adapter,call),
    async transport(base: string) {
      const transport: any[] = [];
      for (const rtt of [0, 50, 150]) {
        const url = base + "?rtt=" + rtt,
          ws = new WebSocket(url);
        await new Promise<void>((resolve, reject) => {
          ws.onopen = () => resolve();
          ws.onerror = () => reject(new Error("WebSocket failed"));
        });
        for (const size of [256, 4096, 65536])
          for (const route of ["direct", "host"]) {
            const samples: number[] = [];
            for (let i = 0; i < 21; i++) {
              const payload = JSON.stringify({ id: i, bytes: "x".repeat(size) }),
                t = performance.now();
              const response =
                route === "host"
                  ? await call("relay", { url, payload })
                  : await new Promise<string>((resolve, reject) => {
                      ws.onmessage = (e) => resolve(e.data);
                      ws.onerror = () => reject(new Error("Relay error"));
                      ws.send(payload);
                    });
              if (response !== payload) throw new Error("Transport corruption");
              if (i) samples.push(performance.now() - t);
            }
            transport.push({ rtt, size, route, ...quantiles(samples) });
          }
        ws.close();
      }
      return {
        transport,
        method:
          "Echo round trip of exact strings; isolates transport bridge overhead. No CRDT merge, server storage, auth, or remote paint is measured.",
      };
    },
    async run(samples = 30) {
      const results: any = {};
      sampling = true;
      previous = performance.now();
      for (const scenario of ["checkbox", "typing", "move", "paste"]) {
        const visible: number[] = [],
          accepted: number[] = [],
          engine: number[] = [], patchCost: number[] = [];
        for (let i = 0; i < samples; i++) {
          const state = ui.state(),
            id = state.rows.at(-1).$id;
          let ops: Operation[];
          if (scenario === "checkbox")
            ops = [{ type: "set", path: ["rows", { id }, "done"], value: !state.rows.at(-1).done }];
          else if (scenario === "move")
            ops = [{ type: "move", path: ["rows"], from: 0, to: state.rows.length - 1 }];
          else
            ops = [
              {
                type: "splice",
                path: ["title"],
                index: state.title.length,
                deleteCount: 0,
                text: scenario === "paste" ? "p".repeat(1000) : "x",
              },
            ];
          const t = performance.now();
          const result = await ui.submit(ops);
          accepted.push(performance.now() - t);
          engine.push(result.engineMS);
          if(result.patchUpperBoundMS !== undefined) patchCost.push(result.patchUpperBoundMS);
          if (scenario === "typing" || scenario === "paste") ui.setDraft(ui.state().title);
          await tick();
          visible.push((await frame()) - t);
          if (scenario === "typing")
            await new Promise((r) => setTimeout(r, Math.max(0, 50 - (performance.now() - t))));
        }
        results[scenario] = {
          accepted: quantiles(accepted),
          renderOpportunity: quantiles(visible),
          engine: quantiles(engine),
          ...(patchCost.length ? {patchUpperBound:quantiles(patchCost)} : {}),
        };
      }
      const savesBeforeTyping = await call("saveCount");
      let maxPending = 0;
      const draftFrames: number[] = [];
      const typingStart = performance.now(),
        expected = ui.state().title + "z".repeat(60);
      for (let i = 0; i < 60; i++) {
        const eventAt = performance.now();
        ui.type("z");
        maxPending = Math.max(maxPending, ui.pending());
        await tick();
        draftFrames.push((await frame()) - eventAt);
        await new Promise((r) =>
          setTimeout(r, Math.max(0, typingStart + (i + 1) * 50 - performance.now())),
        );
      }
      const drainStart = performance.now();
      await ui.drain();
      const drainMS = performance.now() - drainStart;
      if (ui.state().title !== expected)
        throw new Error("Sustained typing lost or duplicated characters");
      const savesDuringTyping = (await call("saveCount")) - savesBeforeTyping;
      if (savesDuringTyping < 8)
        throw new Error("Autosave did not run periodically during sustained typing");
      const sustainedTyping = {
        maxPending,
        drainMS,
        savesDuringTyping,
        renderOpportunity: quantiles(draftFrames),
        durationMS: performance.now() - typingStart,
      };
      const t = performance.now();
      await adapter.flush();
      const flushMS = performance.now() - t;
      sampling = false;
      return {
        engineReadyMS,
        readyMS,
        results,
        flushMS,
        sustainedTyping,
        frameIntervals: quantiles(frames),
        wasmRequests: performance
          .getEntriesByType("resource")
          .filter((e) => e.name.includes("loro") || e.name.endsWith(".wasm"))
          .map((e) => e.name),
        rows: ui.state().rows.length,
      };
    },
    async verify() {
      const before = JSON.stringify(ui.state());
      let rejected = false;
      try {
        await ui.submit([
          { type: "splice", path: ["title"], index: 0, deleteCount: 0, text: "SHOULD NOT EXIST" },
          { type: "set", path: ["rows", { id: ui.state().rows[0].$id }, "done"], value: "invalid" },
        ]);
      } catch {
        rejected = true;
      }
      if (!rejected || JSON.stringify(adapter.current()) !== before)
        throw new Error("Atomic rejection failed");
      const state = ui.state(),
        id = state.rows[0].$id;
      await ui.submit([{ type: "set", path: ["rows", { id }, "done"], value: true }]);
      await adapter.flush();
      if (!ui.state().rows[0].done) throw new Error("Accepted edit missing");
      return {
        atomicRejection: true,
        acceptedEdit: true,
        wasmResources: performance
          .getEntriesByType("resource")
          .filter((e) => e.name.includes("loro")).length,
      };
    },
    async close() {
      await ui.drain();
      await adapter.close();
    },
  };
  await call("ready", { engineReadyMS, readyMS });
} catch (error) {
  await call("failed", { error: String(error) });
}
