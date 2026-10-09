import type { ViteDevServer, WebSocketClient } from "vite";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { execute, findEngine, negotiate } from "./engine";
import type { PreviewRequest } from "../wire/preview.generated";

const limit = 64 * 1024 * 1024;
type Session = {
  pid: number;
  path: string;
  token: string;
  resource(id: string, offset: number, length: number): Promise<{info: {size: number; mimeType: string} | null; /** Base64. */ bytes: string | null; error?: string}>;
  send(frame: PreviewRequest): void;
  close(): Promise<void>;
};

/** Internal development harness hook. The caller owns the existing file and supplies
 * the native process; ordinary previews keep creating disposable local documents. */
export type PreviewOwnerFactory = (template: string) => Promise<{
  path: string;
  command: string[];
  startup?: unknown;
  dispose(): Promise<void>;
}>;

/** Vite already owns the loopback WebSocket. This adapter only frames requests;
 * the native owner holds all state, attachments, validation and save scheduling. */
export class PreviewOwners {
  readonly token = crypto.randomUUID();
  private sessions = new Map<WebSocketClient, Promise<Session>>();
  private generation = 0;
  private stopped = false;
  private available = true;
  private closing = new Set<Promise<void>>();
  constructor(private root: string, private template: () => string, private factory?: PreviewOwnerFactory) {}

  attach(server: ViteDevServer) {
    const admitted = new WeakSet<WebSocketClient["socket"]>();
    server.ws.on("connection", (socket, request) => {
      // Loopback under either name; never another site that can reach the port.
      const origins = (server.resolvedUrls?.local ?? []).flatMap(url => {
        const loopback = new URL(url);
        return ["localhost", "127.0.0.1", "[::1]"].map(host => { loopback.hostname = host; return loopback.origin; });
      });
      if (origins.includes(request.headers.origin ?? "")) admitted.add(socket);
    });
    server.ws.on("slop:open", (data, client) => {
      if (!admitted.has(client.socket) || data?.token !== this.token || this.sessions.has(client)) return;
      if (this.stopped || !this.available) {
        client.send("slop:fatal", { error: this.stopped ? "The preview server stopped" : "The app failed to build; fix the error and save to reload" });
        return;
      }
      const generation = this.generation;
      const opening = this.open(client, generation);
      this.sessions.set(client, opening);
      client.socket.once("close", () => this.remove(client));
      opening.catch(error => {
        if (generation === this.generation) client.send("slop:fatal", { error: String(error) });
        this.remove(client);
      });
    });
    server.ws.on("slop:request", (data, client) => {
      if (!admitted.has(client.socket) || data?.token !== this.token || this.stopped || !this.available) return;
      const session = this.sessions.get(client);
      if (!session) return;
      const generation = this.generation;
      try {
        const frame = { type: "page", id: data.id, request: data.request } satisfies PreviewRequest;
        if (Buffer.byteLength(JSON.stringify(frame)) > limit) throw new Error("Oversized preview frame");
        void session.then(session => {
          if (generation !== this.generation || !this.sessions.has(client)) return;
          session.send(frame);
        }).catch(error => { client.send("slop:fatal", { error: String(error) }); this.remove(client); });
      } catch (error) { client.send("slop:fatal", { error: String(error) }); this.remove(client); }
    });
  }

  private remove(client: WebSocketClient) {
    const session = this.sessions.get(client);
    if (!session) return;
    this.sessions.delete(client);
    const closing = session.then(session => session.close(), () => {}).finally(() => this.closing.delete(closing));
    this.closing.add(closing);
  }

  /** Invalidate before rebuilding, so failures never leave an editable stale preview. */
  async invalidate() {
    this.available = false;
    this.generation++;
    for (const client of this.sessions.keys()) {
      client.send("slop:fatal", { error: "Preview definition changed; rebuilding" });
      this.remove(client);
    }
    await Promise.all(this.closing);
  }
  activate() { this.available = true; }
  async close() { this.stopped = true; await this.invalidate(); }
  async diagnostics() {
    return Promise.all([...this.sessions.values()].map(async session => {
      const { pid, path } = await session; return { pid, path };
    }));
  }

  async resource(token: string, id: string, offset: number, length: number) {
    for (const pending of this.sessions.values()) {
      const session = await pending.catch(() => undefined);
      if (session?.token === token) return session.resource(id, offset, length);
    }
    throw new Error("Preview resource session was replaced");
  }

  private async open(client: WebSocketClient, generation: number): Promise<Session> {
    const supplied = await this.factory?.(this.template());
    const directory = supplied ? undefined : await mkdtemp(join(this.root, "session-"));
    const path = supplied?.path ?? join(directory!, "preview.slop");
    let disposed = false;
    const dispose = async () => {
      if (disposed) return;
      disposed = true;
      if (supplied) await supplied.dispose();
      else await rm(directory!, { recursive: true, force: true });
    };
    try {
      if (!supplied) await execute({ method: "create", from: this.template(), output: path });
      if (this.stopped || generation !== this.generation) throw new Error("Preview session replaced");
      const child = Bun.spawn(supplied?.command ?? [...negotiate(await findEngine()), "--preview-owner", path], {
        stdin: "pipe", stdout: "pipe", stderr: "pipe",
      });
      const resourceToken = crypto.randomUUID();
      let resourceID = 0;
      const reads = new Map<number, {resolve: (value: Awaited<ReturnType<Session["resource"]>>) => void; reject: (error:unknown) => void; timer:ReturnType<typeof setTimeout>}>();
      let closing: Promise<void> | undefined;
      let ended = false;
      const pending = new Set<number>();
      const ready = Promise.withResolvers<void>();
      const timer = setTimeout(() => ready.reject(new Error("Preview owner startup timed out")), 15000);
      const active = () => !ended && generation === this.generation && this.sessions.has(client);
      const read = (async () => {
        const reader = child.stdout.getReader();
        const decoder = new TextDecoder();
        let text = "";
        try {
          for (;;) {
            const { value, done } = await reader.read();
            if (done) break;
            text += decoder.decode(value, { stream: true });
            if (text.length > limit) throw new Error("Oversized preview response");
            let end: number;
            while ((end = text.indexOf("\n")) !== -1) {
              const frame = JSON.parse(text.slice(0, end)); text = text.slice(end + 1);
              if (frame.type === "ready") {
                ready.resolve();
                if (active()) client.send("slop:ready", {resourceToken});
              } else if (frame.type === "resource") {
                const read = reads.get(frame.id);
                if (read) { reads.delete(frame.id); clearTimeout(read.timer); read.resolve(frame); }
              } else if (frame.type === "reply") {
                pending.delete(frame.id);
                if (active()) client.send("slop:reply", frame);
              } else if (frame.type === "push") {
                if (active()) client.send("slop:push", frame.pushes);
              } else if (frame.type === "fatal") {
                throw new Error(frame.error?.error ?? "Preview owner failed");
              } else if (frame.type === "save" && frame.error) {
                // Keep ownership until an explicit reset/close discards this preview.
                if (active()) client.send("slop:fatal", { error: `Preview save failed: ${frame.error}. Reload to reset the disposable preview.` });
              }
            }
          }
          if (!ended) throw new Error("Preview owner disconnected; outcome unknown. Reload before editing.");
        } catch (error) {
          ready.reject(error);
          if (active()) client.send("slop:fatal", { error: String(error) });
          this.remove(client);
        } finally { reader.releaseLock(); }
      })();
      const stderr = new Response(child.stderr).text().then(output => {
        if (supplied && output.trim()) console.error(output.trim());
        return output;
      });
      const disconnected = (error: unknown) => {
        if (active()) client.send("slop:fatal", { error: String(error) });
        this.remove(client);
      };
      const session: Session = {
        pid: child.pid, path, token: resourceToken,
        resource(attachmentId, offset, length) {
          if (ended || reads.size >= 16) return Promise.reject(new Error("Resource reader unavailable"));
          const id = ++resourceID;
          return new Promise((resolve, reject) => {
            const timer = setTimeout(() => { reads.delete(id); reject(new Error("Resource read timed out")); }, 15000);
            reads.set(id, {resolve, reject, timer});
            const frame: PreviewRequest = {type:"resource",id,attachmentId,offset,length};
            try { child.stdin.write(JSON.stringify(frame) + "\n"); void Promise.resolve(child.stdin.flush()).catch(reject); }
            catch (error) { reads.delete(id); clearTimeout(timer); reject(error); }
          });
        },
        send(frame: PreviewRequest) {
          if (ended || pending.size >= 64 || pending.has(frame.id)) throw new Error("Preview disconnected or request queue full");
          pending.add(frame.id);
          child.stdin.write(JSON.stringify(frame) + "\n");
          void Promise.resolve(child.stdin.flush()).catch(disconnected);
        },
        close() {
          return closing ??= (async () => {
            ended = true;
            for (const read of reads.values()) { clearTimeout(read.timer); read.reject(new Error("Preview closed")); }
            reads.clear();
            child.stdin.end();
            const kill = setTimeout(() => child.kill(), 12000);
            try {
              const code = await child.exited;
              await read; const errors = await stderr;
              if (supplied && code) throw new Error(`Replica owner close failed (${code}): ${errors}`);
            }
            finally { clearTimeout(kill); await dispose(); }
          })();
        },
      };
      try {
        if (supplied?.startup !== undefined) {
          child.stdin.write(JSON.stringify(supplied.startup) + "\n");
          await child.stdin.flush();
        }
        await ready.promise; return session;
      }
      catch (error) { await session.close(); throw error; }
      finally { clearTimeout(timer); }
    } catch (error) {
      await dispose();
      throw error;
    }
  }
}
