import { checkContainer } from "./copies";
import type { BrowserRequest, BrowserEvent } from "../wire/browser.generated";

/** The part of the WASM module this worker calls. wasm-bindgen declares the module in
 * `generated/browser/core/hitslop_core_wasm.d.ts`, but that is a build output, absent wherever
 * types are checked without building WASM, so the surface is restated here. The browser tests
 * run the real module through this worker. */
export interface BrowserCopy {
  events(): string;
  export(write: (bytes: Uint8Array) => void): void;
  importBegin(size: number): void;
  importFinish(): void;
  importWrite(bytes: Uint8Array): void;
  open(): void;
  /** A negative delay means there is no pending timer. */
  poll(monotonicMs: number, unixMs: number): number;
  recoverStorage(): Promise<void>;
  request(json: string): Uint8Array | undefined;
}
export interface BrowserRuntime<C extends BrowserCopy = BrowserCopy> {
  default(): Promise<unknown>;
  decodeBrowserRequest(json: string): string;
  installCopy(copy: string): Promise<C>;
}

export interface WorkerOptions<C extends BrowserCopy> {
  importCopy(copy: C, request: Extract<BrowserRequest, { type: "open" }>): Promise<void>;
  openCopy(copy: C): void | Promise<void>;
  afterOpen?(request: Extract<BrowserRequest, { type: "open" }>): void | Promise<void>;
  afterPoll?(): void;
  control?(input: unknown, runtime: BrowserRuntime<C>): boolean | Promise<boolean>;
}

/** One serialized owner path for local imports and hosted joins. */
export class WorkerOwner<C extends BrowserCopy = BrowserCopy> {
  constructor(private readonly options: WorkerOptions<C>) {}
  // WASM stays in the trusted storage origin’s dedicated worker.
  readonly initialized = (async () => {
    const runtime = (await import(
      /* @vite-ignore */ new URL("./core/hitslop_core_wasm.js", location.href).href
    )) as BrowserRuntime<C>;
    await runtime.default();
    return runtime;
  })();
  copy: C | undefined;
  opened = false;
  copyID = "";
  private timer: ReturnType<typeof setTimeout> | undefined;
  send(event: BrowserEvent, bytes?: Uint8Array) {
    postMessage(bytes ? { ...event, bytes } : event, bytes ? [bytes.buffer] : []);
  }
  poll = (bytes?: Uint8Array) => {
    clearTimeout(this.timer);
    if (!this.copy) return;
    const delay = this.copy.poll(performance.now(), Date.now());
    for (const event of JSON.parse(this.copy.events()) as BrowserEvent[])
      this.send(event, event.type === "resource" ? bytes : undefined);
    this.options.afterPoll?.();
    if (delay >= 0) this.timer = setTimeout(this.poll, Math.max(1, delay));
  };
  private async open(request: Extract<BrowserRequest, { type: "open" }>) {
    if (this.copy) throw new Error("Worker already owns a copy");
    if (!("createSyncAccessHandle" in FileSystemFileHandle.prototype))
      throw new Error("This browser cannot save local copies");
    this.copyID = request.copy;
    const runtime = await this.initialized;
    await checkContainer(this.copyID, !!request.source);
    let installed: C;
    try {
      installed = await runtime.installCopy(this.copyID);
    } catch (error) {
      throw `Browser storage is busy: ${String(error)}`;
    }
    this.copy = installed;
    if (request.source) await this.options.importCopy(installed, request);
    await this.options.openCopy(installed);
    this.opened = true;
    this.poll();
    await this.options.afterOpen?.(request);
  }
  private async exportFile(id: string) {
    if (!this.copy) throw new Error("Copy is not open");
    const root = await navigator.storage.getDirectory();
    const exports = await root.getDirectoryHandle("exports", { create: true });
    const folder = await exports.getDirectoryHandle(this.copyID, { create: true });
    const name = `${crypto.randomUUID()}.slop`;
    const file = await folder.getFileHandle(name, { create: true });
    const handle = await (file as any).createSyncAccessHandle();
    try {
      handle.truncate(0);
      let at = 0;
      this.copy.export((bytes) => {
        const written = handle.write(bytes, { at });
        if (written !== bytes.byteLength) throw new Error("Incomplete export write");
        at += written;
      });
      handle.flush();
    } catch (error) {
      handle.close();
      await folder.removeEntry(name).catch(() => {});
      throw error;
    }
    handle.close();
    this.send({ type: "exported", id, file: name });
  }
  /** What escapes a request's own handling (a poll that throws, an event that cannot be posted)
   * leaves the owner in an unknown state. Raised as an uncaught error it reaches the host as
   * `Worker.onerror`, which fails every waiting request; swallowed by the queue it would leave
   * every later message unanswered, with no signal at all. */
  private crash = (error: unknown) =>
    queueMicrotask(() => {
      throw error;
    });
  // Serial async control operations; authored page work never interleaves an import/export.
  private queue = Promise.resolve();
  handle = ({ data: input }: MessageEvent<unknown>) => {
    this.queue = this.queue.then(async () => {
      let data: BrowserRequest | undefined;
      let canPoll = false;
      let bytes: Uint8Array | undefined;
      try {
        const runtime = await this.initialized;
        if (await this.options.control?.(input, runtime)) return;
        data = JSON.parse(runtime.decodeBrowserRequest(JSON.stringify(input))) as BrowserRequest;
        clearTimeout(this.timer);
        if (this.copy && data.type !== "open") await this.copy.recoverStorage();
        canPoll = this.opened;
        if (data.type === "open") {
          await this.open(data);
          return;
        }
        if (data.type === "export") await this.exportFile(data.id);
        else {
          if (!this.copy) throw new Error("Copy is not open");
          bytes = this.copy.request(JSON.stringify(data)) ?? undefined;
        }
      } catch (error) {
        const id =
          data && "id" in data
            ? data.id
            : input &&
                typeof input === "object" &&
                "id" in input &&
                typeof input.id === "string" &&
                input.id.length <= 128
              ? input.id
              : null;
        this.send({ type: "error", id, error: String(error) });
      }
      if (canPoll) this.poll(bytes);
    });
    this.queue = this.queue.catch(this.crash);
  };
}
