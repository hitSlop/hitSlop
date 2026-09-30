import type { Static, TSchema } from "typebox";
import { Check } from "typebox/value";
export function validate<S extends TSchema>(schema:S,value:unknown,message = "Invalid platform contract"):Static<S> {
  if (!Check(schema,value)) throw new Error(message);
  return value as Static<S>;
}
