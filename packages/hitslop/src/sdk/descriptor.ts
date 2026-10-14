// Internal descriptor helpers shared by the page shell, the app-side adapter and the capture
// renderers. Not a package export: authors use `defineDocument` and `s` from the package root.
import type { Definition, Descriptor, Node, ObjectNode, OptionalNode, Scalar } from "./schema";

export const unwrap = (node: Node): Exclude<Node, OptionalNode> =>
  node.kind === "optional" ? node.inner : node;
export const isScalar = (node: Node): node is Scalar =>
  ["string", "number", "integer", "boolean", "enum"].includes(node.kind);
export function fromDescriptor(input: Descriptor): Definition<ObjectNode> {
  const descriptor = JSON.parse(JSON.stringify(input)) as Descriptor;
  if (descriptor.kind !== "object") throw new Error("Document root must be an object");
  deepFreeze(descriptor);
  return Object.freeze({ descriptor: descriptor as ObjectNode });
}
/** Freezes `value` and everything it holds, in place, so identities never change. A frozen
 * object is taken as already deeply frozen and not walked again. */
export function deepFreeze<T>(value: T): T {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}
