// The Swift contract generator emits each titled enumeration once; two definitions under
// one name must stop generation, never silently share the first one's cases.
import { expect, test } from "bun:test";
import { Type as T } from "typebox";
import { swiftContracts } from "../../../scripts/swift-contracts";
import { PageFailureSchema, PageRequestSchema, PageResults } from "../src/page";
import { SocketDiscoverySchema, SocketReplySchema, SocketRequestSchema } from "../src/socket";

test("one enumeration name with two definitions fails generation", () => {
  const generate = (reply: typeof SocketReplySchema | ReturnType<typeof T.Object>) =>
    swiftContracts(SocketRequestSchema, reply, SocketDiscoverySchema, PageRequestSchema, PageFailureSchema, PageResults);
  expect(() => generate(SocketReplySchema)).not.toThrow();
  // The page failure's `OutcomeCode` and this one differ.
  const conflicting = T.Object({ ok: T.Boolean(), code: T.Optional(T.Enum(["rejected"], { title: "OutcomeCode" })) }, { additionalProperties: false });
  expect(() => generate(conflicting)).toThrow("OutcomeCode");
});
