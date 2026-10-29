import type { ViteDevServer, WebSocketClient } from "vite";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { execute, findEngine, negotiate } from "./engine";
import type { ResponsePlan } from "../wire/resources.generated";
import type { PreviewRequest } from "../wire/preview.generated";

const limit = 64 * 1024 * 1024;
type Session = {
  pid: number;
  path: string;
  token: string;
  resource(
    id: string,
    offset: number,
    length: number,
    range?: string | null,
  ): Promise<{
    plan?: ResponsePlan | null;
    info: { size: number; mimeType: string } | null;
    /** Base64. */ bytes: string | null;
    error?: string;
  }>;
  connect(client: WebSocketClient): void;
  disconnect(client: WebSocketClient): void;
  send(frame: PreviewRequest): void;
  close(): Promise<void>;
};

/** Vite already owns the loopback WebSocket. This adapter only frames requests;
 * the native owner holds all state, attachments, validation and save scheduling. */
export class PreviewOwners {
  readonly token = crypto.randomUUID();
  // A tab can replace its socket during UI reloads. Retain its owner until definition reset/close.
  private sessions = new Map<string, Promise<Session>>();
  private clients = new Map<WebSocketClient, string>();
  private generation = 0;
  private stopped = false;
  private available = true;
  private closing = new Set<Promise<void>>();
  constructor(
    private root: string,
    private template: () => string,
  ) {}

  attach(server: ViteDevServer) {
    const admitted = new WeakSet<WebSocketClient["socket"]>();
    server.ws.on("connection", (socket, request) => {
      // Loopback under either name; never another site that can reach the port.
      const origins = (server.resolvedUrls?.local ?? []).flatMap((url) => {
        const loopback = new URL(url);
        return ["localhost", "127.0.0.1", "[::1]"].map((host) => {
          loopback.hostname = host;
          return loopback.origin;
        });
      });
      if (origins.includes(request.headers.origin ?? "")) admitted.add(socket);
    });
    server.ws.on("tiny:open", (data, client) => {
      if (
        !admitted.has(client.socket) ||
        data?.token !== this.token ||
        this.clients.has(client) ||
        typeof data?.session !== "string" ||
        !/^[0-9a-f-]{36}$/.test(data.session)
      )
        return;
      if (this.stopped || !this.available) {
        client.send("tiny:fatal", {
          error: this.stopped
            ? "The preview server stopped"
            : "The app failed to build; fix the error and save to reload",
        });
        return;
      }
      const generation = this.generation;
      const id = data.session as string;
      this.clients.set(client, id);
      let opening = this.sessions.get(id);
      if (!opening) {
        opening = this.open(id, generation);
        this.sessions.set(id, opening);
      }
      void opening
        .then((session) => {
          if (this.clients.get(client) === id && generation === this.generation)
            session.connect(client);
        })
        .catch(() => {});
      client.socket.once("close", () => {
        this.clients.delete(client);
        void opening.then((session) => session.disconnect(client)).catch(() => {});
      });
      opening.catch((error) => {
        if (generation === this.generation) client.send("tiny:fatal", { error: String(error) });
        this.remove(id);
      });
    });
    server.ws.on("tiny:request", (data, client) => {
      if (
        !admitted.has(client.socket) ||
        data?.token !== this.token ||
        this.stopped ||
        !this.available
      )
        return;
      const id = this.clients.get(client);
      if (!id) return;
      const session = this.sessions.get(id);
      if (!session) return;
      const generation = this.generation;
      try {
        const frame = { type: "page", id: data.id, request: data.request } satisfies PreviewRequest;
        if (Buffer.byteLength(JSON.stringify(frame)) > limit)
          throw new Error("Oversized preview frame");
        void session
          .then((session) => {
            if (generation !== this.generation || !this.clients.has(client)) return;
            session.send(frame);
          })
          .catch((error) => {
            client.send("tiny:fatal", { error: String(error) });
            this.remove(id);
          });
      } catch (error) {
        client.send("tiny:fatal", { error: String(error) });
        this.remove(id);
      }
    });
  }

  private remove(id: string) {
    const session = this.sessions.get(id);
    if (!session) return;
    this.sessions.delete(id);
    for (const [client, key] of this.clients) if (key === id) this.clients.delete(client);
    // Only a rebuild or the server's close waits for this, and a rejection nobody observes ends
    // the CLI. A close that fails (its directory could not be removed) is reported where it starts.
    const closing = session
      .then(
        (session) =>
          session
            .close()
            .catch((error) =>
              console.error(`Could not close the preview ${session.path}: ${error}`),
            ),
        () => {},
      )
      .finally(() => this.closing.delete(closing));
    this.closing.add(closing);
  }

  /** Invalidate before rebuilding, so failures never leave an editable stale preview. */
  async invalidate() {
    this.available = false;
    this.generation++;
    for (const client of this.clients.keys())
      client.send("tiny:fatal", { error: "Preview definition changed; rebuilding" });
    for (const id of this.sessions.keys()) this.remove(id);
    await Promise.all(this.closing);
  }
  activate() {
    this.available = true;
  }
  async close() {
    this.stopped = true;
    await this.invalidate();
  }
  async diagnostics() {
    return Promise.all(
      [...this.sessions.values()].map(async (session) => {
        const { pid, path } = await session;
        return { pid, path };
      }),
    );
  }

  async resource(token: string, id: string, offset: number, length: number, range?: string | null) {
    for (const pending of this.sessions.values()) {
      const session = await pending.catch(() => undefined);
      if (session?.token === token) return session.resource(id, offset, length, range);
    }
    throw new Error("Preview resource session was replaced");
  }

  private async open(id: string, generation: number): Promise<Session> {
    const directory = await mkdtemp(join(this.root, "session-"));
    const path = join(directory, "preview.tiny");
    let disposed = false;
    const dispose = async () => {
      if (disposed) return;
      disposed = true;
      await rm(directory, { recursive: true, force: true });
    };
    try {
      await execute({ method: "create", from: this.template(), output: path });
      if (this.stopped || generation !== this.generation)
        throw new Error("Preview session replaced");
      const child = Bun.spawn([...negotiate(await findEngine()), "--preview-owner", path], {
        stdin: "pipe",
        stdout: "pipe",
        stderr: "pipe",
      });
      const resourceToken = crypto.randomUUID();
      let resourceID = 0;
      const reads = new Map<
        number,
        {
          resolve: (value: Awaited<ReturnType<Session["resource"]>>) => void;
          reject: (error: unknown) => void;
          timer: ReturnType<typeof setTimeout>;
        }
      >();
      let closing: Promise<void> | undefined;
      let ended = false;
      const pending = new Set<number>();
      const ready = Promise.withResolvers<void>();
      const timer = setTimeout(
        () => ready.reject(new Error("Preview owner startup timed out")),
        15000,
      );
      let client: WebSocketClient | undefined;
      // A failed save stays fatal for this session; a reload shows it again instead of resuming.
      let saveFailure: string | undefined;
      const active = () => !ended && generation === this.generation && client !== undefined;
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
              const frame = JSON.parse(text.slice(0, end));
              text = text.slice(end + 1);
              if (frame.type === "ready") {
                ready.resolve();
                if (active()) client?.send("tiny:ready", { resourceToken });
              } else if (frame.type === "resource") {
                const read = reads.get(frame.id);
                if (read) {
                  reads.delete(frame.id);
                  clearTimeout(read.timer);
                  read.resolve(frame);
                }
              } else if (frame.type === "reply") {
                pending.delete(frame.id);
                if (active()) client?.send("tiny:reply", frame);
              } else if (frame.type === "push") {
                if (active()) client?.send("tiny:push", frame.pushes);
              } else if (frame.type === "fatal") {
                throw new Error(frame.error?.error ?? "Preview owner failed");
              } else if (frame.type === "save" && frame.error) {
                // Keep ownership until an explicit reset/close discards this preview.
                saveFailure = `Preview save failed: ${frame.error}. Restart the preview to reset its disposable document.`;
                if (active()) client?.send("tiny:fatal", { error: saveFailure });
              }
            }
          }
          if (!ended)
            throw new Error("Preview owner disconnected; outcome unknown. Reload before editing.");
        } catch (error) {
          ready.reject(error);
          if (active()) client?.send("tiny:fatal", { error: String(error) });
          this.remove(id);
        } finally {
          reader.releaseLock();
        }
      })();
      const stderr = new Response(child.stderr).text();
      const disconnected = (error: unknown) => {
        if (active()) client?.send("tiny:fatal", { error: String(error) });
        this.remove(id);
      };
      /** One request line for the owner. A write it cannot take rejects after it started (a
       * broken pipe), so each caller says where that failure goes. */
      const post = (frame: PreviewRequest, failed: (error: unknown) => void) => {
        void Promise.resolve(child.stdin.write(JSON.stringify(frame) + "\n")).catch(failed);
        void Promise.resolve(child.stdin.flush()).catch(failed);
      };
      const session: Session = {
        connect(next) {
          client = next;
          if (saveFailure) client.send("tiny:fatal", { error: saveFailure });
          else client.send("tiny:ready", { resourceToken });
        },
        disconnect(previous) {
          if (client === previous) client = undefined;
        },
        pid: child.pid,
        path,
        token: resourceToken,
        resource(attachmentId, offset, length, range) {
          if (ended || reads.size >= 16)
            return Promise.reject(new Error("Resource reader unavailable"));
          const id = ++resourceID;
          return new Promise((resolve, reject) => {
            const timer = setTimeout(() => {
              reads.delete(id);
              reject(new Error("Resource read timed out"));
            }, 15000);
            reads.set(id, { resolve, reject, timer });
            const frame: PreviewRequest =
              range === undefined
                ? { type: "resource", id, attachmentId, offset, length }
                : { type: "resourcePlan", id, attachmentId, range };
            const fail = (error: unknown) => {
              reads.delete(id);
              clearTimeout(timer);
              reject(error);
            };
            try {
              post(frame, fail);
            } catch (error) {
              fail(error);
            }
          });
        },
        send(frame: PreviewRequest) {
          if (ended || pending.size >= 64 || pending.has(frame.id))
            throw new Error("Preview disconnected or request queue full");
          pending.add(frame.id);
          post(frame, disconnected);
        },
        close() {
          return (closing ??= (async () => {
            ended = true;
            for (const read of reads.values()) {
              clearTimeout(read.timer);
              read.reject(new Error("Preview closed"));
            }
            reads.clear();
            // The owner may already be gone, and then there is nobody to give EOF to.
            void Promise.resolve(child.stdin.end()).catch(() => {});
            const kill = setTimeout(() => child.kill(), 12000);
            try {
              await child.exited;
              await read;
              await stderr;
            } finally {
              clearTimeout(kill);
              await dispose();
            }
          })());
        },
      };
      try {
        await ready.promise;
        return session;
      } catch (error) {
        await session.close();
        throw error;
      } finally {
        clearTimeout(timer);
      }
    } catch (error) {
      await dispose();
      throw error;
    }
  }
}
