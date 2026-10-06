import { Type, type TSchema } from "typebox";
import { Strict } from "./strict";

export const CommandAssets = { metadata: "__commands/metadata.json", bundle: "__commands/run.js" } as const;
export const CommandName = Type.String({ minLength: 1, maxLength: 80, pattern: "^[a-z][a-zA-Z0-9]{0,79}$" });
export const CommandMetadata = Type.Record(CommandName, Strict({
  description: Type.String({ minLength: 1, maxLength: 500 }),
  args: Type.Object({}, { additionalProperties: true }),
}), { additionalProperties: false, maxProperties: 64 });
export const CommandCall = Strict({ documentPath: Type.String({ minLength: 1, maxLength: 4096 }), command: CommandName, args: Type.Unknown() });
export const CommandSuccess = Strict({ ok: Type.Literal(true), method: Type.Literal("call"), result: Type.Unknown(), ids: Type.Array(Type.String()) });

/** Version-one arguments use portable JSON Schema. Reject extensions and executable
 * transforms at authoring time so the page and QuickJS validate identically. */
export function commandArgs(schema: TSchema): void {
  const visit = (node: any, depth: number) => {
    if (!node || typeof node !== "object" || Array.isArray(node) || depth > 16) throw new Error("Invalid command argument schema");
    const allowed = new Set(["type", "properties", "required", "additionalProperties", "items", "anyOf", "const", "enum", "minimum", "maximum", "minLength", "maxLength", "minItems", "maxItems", "pattern", "description", "title", "~optional", "~kind"]);
    if (Object.getOwnPropertySymbols(node).length || Object.keys(node).some(key => !allowed.has(key)))
      throw new Error("Unsupported command argument schema; use plain TypeBox JSON types without refs, transforms or formats");
    if (node.anyOf) { if (!Array.isArray(node.anyOf) || !node.anyOf.length) throw new Error("Invalid command union"); node.anyOf.forEach((n: unknown) => visit(n, depth + 1)); }
    else if (!["object", "array", "string", "number", "integer", "boolean", "null"].includes(node.type)) throw new Error("Unsupported command argument type");
    if (node.type === "object") {
      if (node.additionalProperties !== false || !node.properties) throw new Error("Command argument objects require additionalProperties: false");
      Object.values(node.properties).forEach(n => visit(n, depth + 1));
    }
    if (node.type === "array") visit(node.items, depth + 1);
  };
  if ((schema as any).type !== "object") throw new Error("Command arguments must be a TypeBox object");
  visit(schema, 0);
  if (JSON.stringify(schema).length > 32_768) throw new Error("Command argument schema is too large");
}
