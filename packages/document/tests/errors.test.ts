import { expect, test } from "bun:test";

// Authors and the shell bundle different entry points and separate SDK copies.
test("an author recognizes a refusal from a separately bundled shell", async () => {
  const load = async (entry: string) => {
    const build = await Bun.build({
      entrypoints: [new URL(entry, import.meta.url).pathname],
      target: "browser",
    });
    expect(build.success).toBe(true);
    const source = await build.outputs[0]!.text();
    return import("data:text/javascript;base64," + Buffer.from(source).toString("base64"));
  };
  const author = await load("../src/schema.ts"),
    shell = await load("../src/internal.ts");
  const refusal = new shell.DocumentError("rejected", "Invalid value", "out_of_range", 2);
  expect(author.isDocumentError(refusal)).toBe(true);
  expect(author.isRejected(refusal)).toBe(true);
  expect(author.isRejected(new Error("application failure"))).toBe(false);
});
