import { cp, mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { prepareNativeFixtures } from "./native-fixtures";
import { digest } from "./runtime-artifacts";
import { strict as assert } from "node:assert";

await prepareNativeFixtures();
const folder = await mkdtemp(join(tmpdir(), "hitslop-relocated-helper-"));
try {
  const helpers = join(folder, "hitSlop.app/Contents/Helpers");
  await mkdir(helpers, { recursive: true });
  const build = resolve("apps/apple/Packages/HitSlopApple/.build/debug");
  for (const name of ["hitslop-native", "HitSlopApple_HitSlopDocument.bundle"])
    await cp(join(build, name), join(helpers, name), { recursive: true });
  const document = join(folder, "List.slop");
  await cp("generated/native-fixtures/quick-checklist.slop", document, { recursive: true });
  const run = async (args: string[], expectedError?: string) => {
    const child = Bun.spawn([join(helpers, "hitslop-native"), ...args], {
      cwd: folder,
      env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin" },
      stdout: "pipe",
      stderr: "pipe",
    });
    const [out, error, code] = await Promise.all([
      new Response(child.stdout).text(),
      new Response(child.stderr).text(),
      child.exited,
    ]);
    if (!expectedError) assert.equal(code, 0, error);
    else {
      assert.notEqual(code, 0, "Expected refusal, but the helper succeeded");
      assert.ok(error.includes(expectedError), error);
    }
    return out;
  };
  // A helper inside Contents/Helpers must protect the enclosing app's bundled masters.
  const master = join(folder, "hitSlop.app/Contents/Resources/StarterTemplates/Checklist.slop");
  await cp(document, master, { recursive: true });
  const masterBytes = await digest(master);
  await run(
    [
      "apply",
      master,
      "--op",
      JSON.stringify({ type: "set", path: ["title"], value: "Must refuse" }),
    ],
    "writable copy",
  );
  assert.ok(!(await readdir(master)).includes("state"), "Bundled master acquired mutable state");
  assert.equal(await digest(master), masterBytes, "Bundled master bytes changed");
  const initial = JSON.parse(await run(["get", document]));
  await run([
    "apply",
    document,
    "--op",
    JSON.stringify({ type: "set", path: ["title"], value: "Relocated native edit" }),
  ]);
  assert.equal(JSON.parse(await run(["get", document])).title, "Relocated native edit");
  for (const format of ["png", "pdf"]) {
    const output = join(folder, "export." + format);
    await run(["export", document, "--format", format, "--output", output]);
    const bytes = await readFile(output);
    assert.ok(bytes.length > 100);
    assert.equal(
      bytes.subarray(0, format === "png" ? 8 : 4).toString("hex"),
      format === "png" ? "89504e470d0a1a0a" : "25504446",
    );
  }
  // A broken relocated resource must fail even while valid checkout resources exist.
  const bundle = join(helpers, "HitSlopApple_HitSlopDocument.bundle");
  const shell = [join(bundle, "shell/index.js"), join(bundle, "Contents/Resources/shell/index.js")];
  const index = (
    await Promise.all(shell.map(async (path) => ((await Bun.file(path).exists()) ? path : undefined)))
  ).find(Boolean);
  assert.ok(index, "Missing embedded page shell");
  await rm(index);
  // Closed state edits need no JS resources. Rendering must refuse the broken bundle.
  await run(["get", document]);
  await run(
    ["export", document, "--format", "pdf", "--output", join(folder, "broken.pdf")],
    "Incomplete page shell",
  );
  console.log(
    "PASS relocated helper: closed editing, PNG/PDF export, no Bun, no checkout page-shell fallback",
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}
