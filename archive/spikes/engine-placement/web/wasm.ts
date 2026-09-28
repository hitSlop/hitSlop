import init from "loro-crdt/web/loro_wasm.js";
import { Document } from "../../../packages/document/src/document";
import { fromDescriptor } from "../../../packages/document/src/schema";
import { base64 } from "../../../packages/document/src/bridge";
import type { ByteStore } from "../../../packages/document/src/storage";
import type { Operation } from "../contract";
import { call } from "./wire";
// This is a harness adapter, not an SDK exported to authored apps.
export async function open(schema: any) {
  await init();
  const store: ByteStore = {
    async load() {
      const r = await call("load");
      return {
        ...r,
        checkpoint: r.checkpoint ? base64.decode(r.checkpoint) : null,
        updates: r.updates.map(base64.decode),
      };
    },
    metadata: () => call("metadata"),
    async append(generation, updates) {
      return (await call("append", { generation, updates: updates.map(base64.encode) })).generation;
    },
    async checkpoint(generation, bytes, schemaKey) {
      return (await call("checkpoint", { generation, bytes: base64.encode(bytes), schemaKey }))
        .generation;
    },
    async close() {},
  };
  const doc = await Document.open(fromDescriptor(schema), store, {});
  return {
    current: () => doc.current,
    async submit(ops: Operation[]) {
      const started = performance.now();
      doc.change((tx) => {
        for (const op of ops) {
          let h: any = tx.fields;
          for (const p of op.path)
            h =
              typeof p === "string"
                ? h[p]
                : "id" in p
                  ? h.item(p.id)
                  : "key" in p
                    ? h.entry(p.key)
                    : h.item(p.index);
          switch (op.type) {
            case "set":
            case "assign":
              h.set(op.value);
              break;
            case "clear":
              h.clear();
              break;
            case "splice":
              h.splice(op.index, op.deleteCount, op.text);
              break;
            case "mark":
              h.mark({ start: op.start, end: op.end }, op.key, op.value);
              break;
            case "increment":
              h.increment(op.amount);
              break;
            case "insert": {
              const rows = (doc.current as any)[op.path[0] as string];
              h.insert(op.value, rows[op.index] ? { before: rows[op.index].$id } : undefined);
              break;
            }
            case "remove": {
              const rows = (doc.current as any)[op.path[0] as string];
              h.remove(rows[op.index].$id);
              break;
            }
            case "move": {
              const rows = (doc.current as any)[op.path[0] as string];
              h.move(
                rows[op.from].$id,
                op.to > op.from ? { after: rows[op.to].$id } : { before: rows[op.to].$id },
              );
              break;
            }
          }
        }
      });
      return { engineMS: performance.now() - started };
    },
    flush: () => doc.flush(),
    async close() {
      await doc.close();
    },
    snapshot: () => base64.encode(doc.exportSnapshot()),
    async receive(bytes: string) {
      doc.importUpdates(base64.decode(bytes));
    },
  };
}
