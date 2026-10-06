import type { Static, TSchema } from "typebox";
import { Check, Errors } from "typebox/value";

/** The first reason `value` fails `schema`, located by its JSON pointer: an unknown field by
 * name, a union as a whole rather than each of its branches. */
function reason(schema: TSchema, value: unknown): string | undefined {
  // `false` schemas repeat each unknown field that `additionalProperties` names.
  const errors = [...Errors(schema, value)].filter((error) => error.keyword !== "boolean");
  const unions = errors.filter((error) => error.keyword === "anyOf").map((error) => error.instancePath);
  const first = errors.find(
    (error) => error.keyword === "anyOf" || !unions.some((path) => error.instancePath.startsWith(path + "/") || error.instancePath === path),
  );
  if (!first) return undefined;
  const at = first.instancePath || "/";
  if (first.keyword === "anyOf") return `${at}: matches none of its allowed forms`;
  if (first.keyword === "additionalProperties") {
    const names = (first.params as { additionalProperties?: string[] }).additionalProperties ?? [];
    return `${at}: unknown ${names.length === 1 ? "field" : "fields"} ${names.map((name) => JSON.stringify(name)).join(", ")}`;
  }
  return `${at}: ${first.message}`;
}

export function validate<S extends TSchema>(schema: S, value: unknown, message = "Invalid platform contract"): Static<S> {
  if (!Check(schema, value)) {
    const why = reason(schema, value);
    throw new Error(why ? `${message} at ${why}` : message);
  }
  return value as Static<S>;
}
