import { call } from "../../engine-placement/web/wire";
import type { Operation } from "../../engine-placement/contract";
import { revision, type State, type Op, type Path } from "../core";
function share(old: any, value: any): any {
  if (old === value || value === null || typeof value !== "object") return value;
  if (Array.isArray(value)) {
    const byID = new Map((Array.isArray(old) ? old : []).map((row: any) => [row.$id, row]));
    const next = value.map(row => share(byID.get(row.$id), row));
    return Array.isArray(old) && old.length === next.length && next.every((row, i) => row === old[i]) ? old : next;
  }
  const next = Object.fromEntries(Object.entries(value).map(([key, v]) => [key, share(old?.[key], v)]));
  return old && Object.keys(old).length === Object.keys(next).length && Object.keys(next).every(k => old[k] === next[k]) ? old : next;
}
export async function openJSC() {
  let state: State = await call("current");
  const sharingMS: number[] = [];
  return {
    current: () => state.value,
    async submit(operations: Operation[]) {
      const ops: Op[] = operations.map(op => {
        // The old benchmark has positional workload descriptions. Resolve those NOW,
        // at the submitting snapshot, then send only ID-addressed, versioned intents.
        const path = op.path as Path;
        const expected = revision(state, path);
        if (op.type === "set") return { type: "set", path, value: op.value as string | boolean, expected };
        if (op.type === "splice") {
          if (path.length !== 1 || path[0] !== "title") throw new Error("Benchmark splice only targets title");
          return { type: "set", path, value: state.value.title.slice(0, op.index) + op.text + state.value.title.slice(op.index + op.deleteCount), expected };
        }
        if (op.type === "move") {
          const rows = state.value.rows;
          return { type: "move", path, id: rows[op.from].$id,
            to: op.to > op.from ? { after: rows[op.to].$id } : { before: rows[op.to].$id }, expected };
        }
        throw new Error(`Unsupported benchmark operation: ${op.type}`);
      });
      const response = await call("apply", { operations: ops });
      if (response.state.rev <= state.rev) throw new Error("Out of order snapshot");
      const start = performance.now();
      response.state.value = share(state.value, response.state.value);
      sharingMS.push(performance.now() - start);
      state = response.state;
      return response;
    },
    flush: () => call("flush"), close: () => call("close"),
    snapshot: () => call("current"), sharingMS,
  };
}
