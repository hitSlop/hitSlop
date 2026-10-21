// The preview adapter writes frames to its owner's stdin without waiting. A frame larger than a
// pipe holds, written after the owner stopped reading, is a write that rejects with EPIPE: left
// unobserved it ends the CLI (bun test fails the running test) and the page is never told. Closing
// a preview nobody waits for is the same: a failure there is reported, not left unobserved.
import { afterEach, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { ViteDevServer, WebSocketClient } from "vite";
import { PreviewOwners } from "../../src/cli/preview-owner";

const origin = "http://localhost:5173";
const engine = process.env.HITSLOP_ENGINE;
const roots: string[] = [];
afterEach(async () => {
  if (engine === undefined) delete process.env.HITSLOP_ENGINE;
  else process.env.HITSLOP_ENGINE = engine;
  for (const root of roots.splice(0)) await rm(root, { recursive: true, force: true });
});

/** A page connected to a preview owner that closes its input, reports ready and stays alive. */
async function connect() {
  const root = await mkdtemp(join(tmpdir(), "hitslop-preview-owner-"));
  roots.push(root);
  const stub = join(root, "engine");
  await writeFile(
    stub,
    `#!${process.execPath}
import { closeSync } from "node:fs";
if (process.argv.includes("--preview-owner")) {
  // Stops reading but stays alive, so the only sign is the broken pipe.
  closeSync(0);
  console.log(JSON.stringify({ type: "ready" }));
  await Bun.sleep(5000);
} else {
  await Bun.stdin.text();
  console.log(JSON.stringify({ ok: true, method: "create", documentPath: "preview.slop" }));
}
`,
    { mode: 0o755 },
  );
  process.env.HITSLOP_ENGINE = stub;
  const owners = new PreviewOwners(root, () => "template.slop");
  const handlers = new Map<string, (...args: any[]) => void>();
  owners.attach({
    resolvedUrls: { local: [origin] },
    ws: { on: (event: string, handler: (...args: any[]) => void) => handlers.set(event, handler) },
  } as unknown as ViteDevServer);
  const ready = Promise.withResolvers<string>();
  const fatal = Promise.withResolvers<string>();
  const socket = { once() {} };
  const client = {
    socket,
    send(event: string, payload: any) {
      if (event === "slop:ready") ready.resolve(payload.resourceToken);
      if (event === "slop:fatal") fatal.resolve(payload.error);
    },
  } as unknown as WebSocketClient;
  handlers.get("connection")!(socket, { headers: { origin } });
  handlers.get("slop:open")!({ token: owners.token, session: crypto.randomUUID() }, client);
  const resourceToken = await ready.promise;
  const { pid } = (await owners.diagnostics())[0]!;
  return {
    owners,
    root,
    pid,
    resourceToken,
    fatal: fatal.promise,
    request: (request: unknown) =>
      handlers.get("slop:request")!({ token: owners.token, id: 1, request }, client),
    async close() {
      try {
        process.kill(pid, "SIGKILL");
      } catch {
        // Already gone.
      }
      await owners.close();
    },
  };
}

const large = "x".repeat(2 ** 21);

test("a page request the owner cannot receive is reported as a disconnect", async () => {
  const page = await connect();
  try {
    page.request({ padding: large });
    expect(await page.fatal).toContain("EPIPE");
  } finally {
    await page.close();
  }
});

test("an attachment read the owner cannot receive is rejected, not left to time out", async () => {
  const page = await connect();
  try {
    // Only an oversized id makes this request large enough to be a pending write.
    const error = await page.owners.resource(page.resourceToken, large, 0, 0).catch((e) => e);
    expect(String(error)).toContain("EPIPE");
  } finally {
    await page.close();
  }
});

test.skipIf(process.getuid?.() === 0)("a preview that cannot be closed is reported, not left unobserved", async () => {
  const page = await connect();
  const reported = Promise.withResolvers<string>();
  const report = console.error;
  console.error = (...args: unknown[]) => reported.resolve(args.join(" "));
  try {
    // Nobody waits for this close, and without write access to its parent it cannot remove the
    // session's directory. The owner ending is what starts it.
    await chmod(page.root, 0o500);
    process.kill(page.pid, "SIGKILL");
    expect(await reported.promise).toContain("Could not close the preview");
  } finally {
    console.error = report;
    await chmod(page.root, 0o700);
    await page.close();
  }
});
