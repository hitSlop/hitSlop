// CLI subprocesses are non-TTY. These cases cover the distinct interactive
// boundary: answers reach slop.ts, and cancellation never creates a project.
import { afterAll, beforeAll, expect, test } from "bun:test";
import { createPromptIO } from "@crustjs/prompts/testing";
import { PassThrough } from "node:stream";
import { mkdtemp, lstat, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { initProject } from "../../src/cli/init";
import { stageProject } from "../../src/cli/build";
import cli from "../../package.json";

// Simulate a person's terminal even when the enclosing test runner is in CI.
// CLI subprocess tests separately prove that CI selects unattended defaults.
const runnerCI = process.env.CI;
beforeAll(() => {
  delete process.env.CI;
});
afterAll(() => {
  if (runnerCI === undefined) delete process.env.CI;
  else process.env.CI = runnerCI;
});

function terminal() {
  const tty = createPromptIO();
  const output = Object.assign(new PassThrough(), { isTTY: true });
  output.on("data", (chunk: Buffer) => tty.io.output.write(chunk.toString()));
  function shown(label: string) {
    return new Promise<void>((resolve) => {
      const onData = (chunk: Buffer) => {
        if (chunk.toString().includes(label)) {
          output.off("data", onData);
          resolve();
        }
      };
      output.on("data", onData);
    });
  }
  return { ...tty, io: { input: tty.io.input, output }, shown };
}

// Only the brief and author are asked; the building agent replaces the
// placeholder title, categories, and description to match the brief.
test("interactive init asks for a brief and author and leaves other metadata to the agent", async () => {
  // Inside the checkout, so the created slop.ts resolves hitslop.
  const root = await mkdtemp(join(process.cwd(), ".build-test-"));
  const tty = terminal();
  try {
    const brief = tty.shown("What should your slop do?");
    const result = initProject(join(root, "app"), {}, tty.io);
    await brief;
    tty.type("A trip planner with a packing checklist and budget.");
    const author = tty.shown("Author");
    tty.keys("return");
    await author;
    tty.type("Jordan");
    tty.keys("return");
    const destination = await result;
    const manifest = (await stageProject(destination, join(root, "stage"))).declaration.metadata;
    expect([
      manifest.title,
      manifest.categories,
      manifest.author.name,
      manifest.description,
    ]).toEqual(["app", ["productivity"], "Jordan", "A hitSlop mini app."]);
    expect(await readFile(join(destination, "BRIEF.md"), "utf8")).toContain(
      "A trip planner with a packing checklist and budget.",
    );
  } finally {
    tty.io.output.destroy();
    await rm(root, { recursive: true, force: true });
  }
}, 30_000);

test("cancelling init leaves no destination; --yes bypasses TTY prompts", async () => {
  const root = await mkdtemp(join(tmpdir(), "hsl-init-cancel-"));
  const tty = terminal();
  try {
    const target = join(root, "app");
    const author = tty.shown("Author");
    const result = initProject(target, { brief: "A timer" }, tty.io);
    const rejected = result.catch((error: unknown) => error);
    await author;
    tty.keys("ctrl+c");
    expect(await rejected).toMatchObject({ name: "AbortError" });
    expect(await lstat(target).catch(() => undefined)).toBeUndefined();
    expect(await initProject(target, { yes: true }, tty.io)).toBe(target);
    const project = JSON.parse(await readFile(join(target, "package.json"), "utf8"));
    expect(project.devDependencies["hitslop"]).toBe(cli.version);
  } finally {
    tty.io.output.destroy();
    await rm(root, { recursive: true, force: true });
  }
}, 30_000);
