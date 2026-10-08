// Renderer-only immutable projection. No Loro dependency.
export function applyOps(value: any, ops: any[]): any {
  function update(value: any, path: any[], f: (v: any) => any): any {
    if (!path.length) return f(value);
    const [head, ...tail] = path;
    if (typeof head === "string") return { ...value, [head]: update(value[head], tail, f) };
    const i = value.findIndex((v: any) => v.$id === head.id);
    if (i < 0) throw new Error("Missing publication row");
    const copy = value.slice(); copy[i] = update(value[i], tail, f); return copy;
  }
  for (const op of ops) {
    if (op.type === "remove") {
      const key = op.path.at(-1);
      value = update(value, op.path.slice(0,-1), v => { const copy = {...v}; delete copy[key]; return copy; });
    } else value = update(value, op.path, v => {
      if (op.type === "set") return op.value;
      const copy = v.slice();
      if (op.type === "insertRow") copy.splice(op.index, 0, op.value);
      else {
        const i = copy.findIndex((row: any) => row.$id === op.id);
        if (i < 0) throw new Error("Missing publication row");
        const [row] = copy.splice(i, 1);
        if (op.type === "moveRow") copy.splice(op.index, 0, row);
        else if (op.type !== "deleteRow") throw new Error("Unknown patch");
      }
      return copy;
    });
  }
  return value;
}
export function projection(initial: any, resync: () => Promise<any>) {
  let frame = initial;
  async function refresh() {
    const observed = frame;
    const fresh = await resync();
    if (frame.session !== observed.session && fresh.session !== frame.session) return;
    if (fresh.session === frame.session && fresh.sequence < frame.sequence) return;
    frame = fresh;
  }
  return {
    get: () => frame,
    async accept(reply: any) {
      const p = reply.patch;
      if (p.session !== frame.session) { await refresh(); return; }
      if (p.sequence <= frame.sequence) return;
      if (p.previous !== frame.sequence) { await refresh(); return; }
      frame = {session:p.session, sequence:p.sequence, version:reply.version, value:applyOps(frame.value,p.ops), issues:p.issues};
    },
  };
}
