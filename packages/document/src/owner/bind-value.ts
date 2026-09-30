import { isOperationRejection } from "../errors";
import type { Node } from "../schema";
import { isScalar, unwrap } from "../descriptor";
import type { OwnerIntent, OwnerPath as Path } from "@hitslop/schema/owner";
interface ValueHost {
  locate(handle: object): { node: Node; path: Path } | undefined;
  read(path: Path): unknown;
  readOnly(): boolean;
  blocked(): boolean;
  write(intent: OwnerIntent): Promise<void>;
  preview(path: Path, value: unknown): void;
  report(error: unknown): void;
  track(work: Promise<void>): void;
  recover(path: Path): Promise<unknown>;
  subscribe(path: Path, listener: () => void): () => void;
  barrier(drain: () => Promise<void>): () => void;
}
/** DOM scalar binding with target-bound writes and explicit uncertain-outcome recovery. */
export function bindValue(host: ValueHost, element: HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement, initial: object) {
    let location!: { node: Node; path: Path };
    type Write = { clear: true } | { value: unknown };
    const queued: { path: Path; write: Write }[] = [];
    let running: Promise<void> | undefined;
    let outcomeFailure: unknown;
    let uncertainWrite: { path: Path; write: Write } | undefined;
    let destroyed = false;
    const input = element as HTMLInputElement;
    // `<select>` reports "select-one"; a textarea reports "textarea" and commits on input.
    const raw = String((element as { type?: string }).type ?? "text");
    const type = raw.startsWith("select") ? "select" : raw;
    const resolve = (next: object) => {
      const found = host.locate(next);
      if (!found || !isScalar(unwrap(found.node))) throw new Error("Expected this document's scalar handle");
      location = found;
    };
    const kind = () => unwrap(location.node) as import("../schema").Scalar;
    const optional = () => location.node.kind === "optional";
    const sync = (retarget = false) => {
      if ((running || outcomeFailure) && !retarget) return;
      const value = host.read(location.path);
      const expected = { boolean: "boolean", string: "string", enum: "string", number: "number", integer: "number" }[kind().kind];
      const parent = host.read(location.path.slice(0, -1));
      const valid = value === undefined ? optional() && parent != null : typeof value === expected;
      const disabled = host.readOnly() || !valid;
      if (element.disabled !== disabled) element.disabled = disabled;
      if (type === "checkbox") input.checked = value === true;
      else {
        const text = value === undefined ? "" : String(value);
        if (element.value !== text) element.value = text;
      }
    };
    /** The control's value as a write, or undefined when it cannot be written. */
    const parse = (): { clear: true } | { value: unknown } | undefined => {
      const k = kind();
      if (type === "checkbox") return k.kind === "boolean" ? { value: input.checked } : undefined;
      const raw = element.value;
      if (raw === "" && optional()) return { clear: true };
      switch (k.kind) {
        case "boolean":
          return raw === "true" || raw === "false" ? { value: raw === "true" } : undefined;
        case "number":
        case "integer": {
          const n = raw.trim() === "" ? NaN : Number(raw);
          if (!Number.isFinite(n) || (k.kind === "integer" && !Number.isSafeInteger(n))) return undefined;
          return { value: n };
        }
        default:
          return { value: raw };
      }
    };
    const intent = ({ path, write }: { path: Path; write: Write }): OwnerIntent =>
      "clear" in write ? { type: "clear", path } : { type: "set", path, value: write.value };
    const run = () => {
      if (running || outcomeFailure || !queued.length) return;
      running = (async () => {
        while (queued.length) {
          const next = queued.shift()!;
          try {
            await host.write(intent(next));
          } catch (error) {
            if (!isOperationRejection(error)) { outcomeFailure = error; uncertainWrite = next; }
            host.report(error);
            if (outcomeFailure) break;
          }
        }
      })().finally(() => {
        running = undefined;
        if (!destroyed) sync();
      });
      void host.track(running);
    };
    const commit = () => {
      if (destroyed) return;
      const write = parse();
      if (!write || host.blocked() || host.readOnly()) return void (running ? undefined : sync());
      // A new explicit control edit is a new operation, never an automatic replay.
      outcomeFailure = undefined;
      const last = queued.at(-1);
      if (last && JSON.stringify(last.path) === JSON.stringify(location.path)) last.write = write;
      else queued.push({ path: location.path, write });
      run();
    };
    const preview = () => {
      const write = parse();
      if (write && "value" in write && !host.blocked()) host.preview(location.path, write.value);
    };
    const events: [string, () => void][] =
      type === "range"
        ? [["input", preview], ["change", commit]]
        : type === "checkbox" || type === "select" || type === "number"
          ? [["change", commit]]
          : [["input", commit]];
    const drain = async () => {
      while (true) {
        while (running) await running;
        if (outcomeFailure && uncertainWrite) {
          const actual = await host.recover(uncertainWrite.path);
          const expected = "clear" in uncertainWrite.write ? undefined : uncertainWrite.write.value;
          if (!Object.is(actual, expected)) throw outcomeFailure;
          outcomeFailure = undefined;
          uncertainWrite = undefined;
        }
        if (!queued.length) { if (!destroyed) sync(); else stopBarrier(); return; }
        run();
      }
    };
    // Resolve first: a rejected handle must not leave a barrier behind.
    resolve(initial);
    const stopBarrier = host.barrier(drain);
    let stop = host.subscribe(location.path, sync);
    for (const [name, listener] of events) element.addEventListener(name, listener);
    sync();
    return {
      update: (next: object) => {
        resolve(next);
        stop();
        stop = host.subscribe(location.path, sync);
        sync(true);
      },
      destroy: () => {
        destroyed = true;
        stop();
        for (const [name, listener] of events) element.removeEventListener(name, listener);
        // No control remains to retry a detached draft: report it once, then stop
        // holding barriers so close can proceed.
        void drain().catch(error => host.report(error)).finally(stopBarrier);
      },
    };
  }
