import { apply, create, fixture, revision, seed, type Op, type State, type Value } from "./core";
const documents = new Map<string, State>();
function state(id: string): State {
  const value = documents.get(id);
  if (!value) throw new Error("Document unavailable");
  return value;
}
/** Explicit JSON boundary; core state never crosses as a live JSValue graph. */
export function invoke(method: string, id: string, input: string): string {
  if (method === "fixture") return JSON.stringify(fixture(JSON.parse(input)));
  if (method === "create") { documents.set(id, create(JSON.parse(input) as Value)); return "null"; }
  if (method === "seed") { documents.set(id, create(seed(Number(input)))); return "null"; }
  if (method === "drop") { documents.delete(id); return "null"; }
  if (method === "current") return JSON.stringify(state(id));
  if (method === "apply") { documents.set(id, apply(state(id), JSON.parse(input))); return String(state(id).rev); }
  if (method === "work") {
    const s = state(id), spec = JSON.parse(input), row = s.value.rows.at(-1)!;
    let ops: Op[];
    if (spec.kind === "move") ops = [{ type: "move", path: ["rows"], id: s.value.rows[0].$id, to: { end: true }, expected: revision(s, ["rows"]) }];
    else if (spec.kind === "text") ops = [{ type: "set", path: ["title"], value: s.value.title + "x", expected: revision(s, ["title"]) }];
    else {
      ops = Array.from({ length: spec.count ?? 1 }, (_, i) => {
        const r = spec.count ? s.value.rows[i % s.value.rows.length] : row;
        const path = ["rows", { id: r.$id }, "done"];
        return { type: "set" as const, path, value: !r.done, expected: revision(s, path) };
      });
    }
    documents.set(id, apply(s, ops));
    return String(state(id).rev);
  }
  throw new Error("Unknown method");
}
