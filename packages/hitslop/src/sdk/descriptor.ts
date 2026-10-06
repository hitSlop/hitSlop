// Internal descriptor helpers shared by the page shell and the app-side adapter. Not a
// package export: authors use `defineDocument` and `s` from the package root.
import type { Definition, Descriptor, Node, ObjectNode, OptionalNode, Scalar } from "./schema";

export const unwrap = (node: Node): Exclude<Node, OptionalNode> =>
  node.kind === "optional" ? node.inner : node;
export const isScalar = (node: Node): node is Scalar =>
  ["string", "number", "integer", "boolean", "enum"].includes(node.kind);
export function fromDescriptor(input: Descriptor): Definition<ObjectNode> {
  const descriptor = JSON.parse(JSON.stringify(input)) as Descriptor;
  if (descriptor.kind !== "object") throw new Error("Document root must be an object");
  freeze(descriptor);
  return Object.freeze({ descriptor: descriptor as ObjectNode });
}
function freeze(value: any) {
  if (value && typeof value === "object") {
    Object.values(value).forEach(freeze);
    Object.freeze(value);
  }
}
