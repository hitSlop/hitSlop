import { mkdir } from "node:fs/promises";
import { resolve, join } from "node:path";
// Loopback only. Delays an opaque echo by a complete simulated network RTT.
// This diagnostic intentionally does not stand in for a durable collaboration server.
const server = Bun.serve<{ delay: number }>({
  hostname: "127.0.0.1",
  port: 0,
  fetch(request, server) {
    const u = new URL(request.url),
      delay = Number(u.searchParams.get("rtt"));
    if (![0, 50, 150].includes(delay)) return new Response("Invalid delay", { status: 400 });
    if (server.upgrade(request, { data: { delay } })) return;
    return new Response("WebSocket required", { status: 400 });
  },
  websocket: {
    message(ws, message) {
      setTimeout(() => {
        if (ws.readyState === 1) ws.send(message);
      }, ws.data.delay);
    },
    maxPayloadLength: 256 * 1024,
  },
});
const here = import.meta.dir,
  root = resolve(here, "../../.hitslop/v1-evidence/engine-placement/transport");
await mkdir(root, { recursive: true });
try {
  for (let trial = 0; trial < 5; trial++) {
    const output = join(root, `${trial}.json`);
    const child = Bun.spawn(
      [
        join(here, ".build/out/Products/Release/engine-placement"),
        join(here, "dist"),
        output,
        "matched",
        "1000",
        "1",
        "checklist",
        "5",
        `ws://127.0.0.1:${server.port}/`,
      ],
      { stdout: "inherit", stderr: "inherit" },
    );
    if (await child.exited) throw new Error("Transport harness failed");
  }
} finally {
  server.stop(true);
}
