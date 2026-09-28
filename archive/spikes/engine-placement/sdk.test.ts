import { expect, test } from "bun:test";
import { defineDocument, s } from "../../packages/document/src/schema";
import { NativeDocumentClient } from "./sdk";
// Gap: a new async authoring boundary must not send a partially constructed batch.
// Independent oracle: the host receives no edit after an author callback throws.
test("callback failure submits nothing; flush waits for accepted edits", async () => {
  const schema = defineDocument({ title: s.text(), count: s.counter() });
  let accepted = false,
    release!: () => void,
    requests = 0,
    saved = false;
  const client = new NativeDocumentClient(
    async () => {
      requests++;
      await new Promise<void>((r) => (release = r));
      accepted = true;
    },
    async () => {
      expect(accepted).toBe(true);
      saved = true;
    },
  );
  await expect(
    client.edit((tx) => {
      tx.increment(schema.fields.count);
      throw new Error("author failure");
    }),
  ).rejects.toThrow("author failure");
  expect(requests).toBe(0);
  const edit = client.edit((tx) => tx.splice(schema.fields.title, 0, 0, "hello"));
  const flush = client.flush();
  await Promise.resolve();
  expect(saved).toBe(false);
  release();
  await edit;
  await flush;
  expect(requests).toBe(1);
  expect(saved).toBe(true);
});
