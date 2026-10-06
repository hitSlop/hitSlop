import * as Type from "typebox";
import { validate } from "./validation";

/** Public discovery response. New metadata is allowed independently of protocol 1. */
export const HelperProtocolSchema = Type.Object({
  version: Type.Integer({ minimum: 1, maximum: Number.MAX_SAFE_INTEGER }),
  minimum: Type.Integer({ minimum: 1, maximum: Number.MAX_SAFE_INTEGER }),
});
export function parseHelperProtocol(value: unknown) {
  const result = validate(HelperProtocolSchema, value, "hitSlop.app did not report its command protocol; update hitSlop");
  if (result.minimum > result.version) throw new Error("hitSlop.app reported an invalid command protocol range");
  return result;
}
