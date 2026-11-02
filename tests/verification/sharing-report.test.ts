import { expect, test } from "bun:test";
import { sharingFailureMessage } from "../../scripts/sharing/report";

test.each(["https://share.dotpop.click", "https://share.hitslop.com", "http://127.0.0.1:8787"])(
  "sharing failure reports hide private invitations from %s", origin => {
    const room = "a".repeat(32);
    const capability = "b".repeat(64);
    const message = sharingFailureMessage(new Error(`Fetch failed: ${origin}/join#${room}.${capability}`));
    expect(message).toBe("Error: Fetch failed: [private invitation]");
    expect(message).not.toContain(room);
    expect(message).not.toContain(capability);
  },
);

test("sharing failure reports preserve useful context and ordinary links", () => {
  const message = "Server returned 503; see https://dotpop.click/docs/sharing";
  expect(sharingFailureMessage(message)).toBe(message);
});
