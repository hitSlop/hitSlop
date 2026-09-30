import { builtTemplates } from "./templates";
import { digest, shellDestinations, shellFiles } from "./runtime-artifacts";
import { strict as assert } from "node:assert";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { join, resolve, dirname } from "node:path";
import { tmpdir } from "node:os";
import { validateTemplate } from "./template-cache";
const app = resolve(process.argv[2] ?? "generated/app/hitSlop.app");
const helper = join(app, "Contents/Helpers/hitslop-native");
// Host and helper each bundle the page shell, byte-identical to the build; no engine WASM.
const shells = [...new Bun.Glob("**/shell/boot.js").scanSync({ cwd: app, onlyFiles: true })].map((p) =>
  dirname(join(app, p)),
);
assert.ok(shells.some((root) => root.startsWith(join(app, "Contents/Resources") + "/")), "Missing app page shell");
assert.ok(shells.some((root) => root.startsWith(join(app, "Contents/Helpers") + "/")), "Missing helper page shell");
const expected = await digest(shellDestinations.app, shellFiles);
for (const root of shells) assert.equal(await digest(root, shellFiles), expected, `Page shell differs: ${root}`);
assert.deepEqual([...new Bun.Glob("**/*.wasm").scanSync({ cwd: app, onlyFiles: true })], [], "The app must not bundle WASM");
const folder = await mkdtemp(join(tmpdir(), "hitslop-release-verify-"));
try {
  const run = async (args: string[], executable = helper) => {
    const child = Bun.spawn([executable, ...args], {
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
    assert.equal(code, 0, error);
    return out;
  };
  const appCore = (await run(["--core-build"], join(app, "Contents/MacOS/hitSlop"))).trim();
  const helperCore = (await run(["--core-build"])).trim();
  assert.ok(appCore.length > 0, "App did not identify its document core");
  assert.equal(appCore, helperCore, "App and helper embed different document cores");
  const cliCore = (await Bun.file("packages/cli/shell/core/BUILD_ID").text()).trim();
  assert.equal(cliCore, helperCore, "CLI and helper embed different document cores");
  const selected = (await builtTemplates()).templates.filter((t) => t.bundled).map((t) => t.slug);
  const starters = join(app, "Contents/Resources/StarterTemplates");
  assert.deepEqual((await readdir(starters)).sort(), selected.map((slug) => slug + ".slop").sort());
  const exhaustive = process.env.HITSLOP_TEMPLATE_EXHAUSTIVE === "1";
  const fixtures = ["quick-checklist"];
  for (const fixture of fixtures)
    assert.ok(selected.includes(fixture), `Missing release fixture: ${fixture}`);
  for (const slug of selected) {
    console.log(`Validating packaged template: ${slug}`);
    const source = join(app, "Contents/Resources/StarterTemplates", slug + ".slop");
    assert.equal(
      await validateTemplate(source, slug),
      await digest(resolve("generated/templates", slug + ".slop")),
      `Packaged template differs from build: ${slug}`,
    );
    if (!exhaustive && !fixtures.includes(slug)) continue;
    console.log(`Exercising installed create/reopen/export: ${slug}`);
    const document = join(folder, slug + ".slop");
    await run(["create", "--from", source, "--output", document]);
    const initial = JSON.parse(await run(["get", document]));
    assert.ok(initial && typeof initial === "object");
    assert.ok(JSON.parse(await run(["schema", document])));
    assert.deepEqual(JSON.parse(await run(["get", document])), initial);
    for (const format of ["png", "pdf"]) {
      const output = join(folder, slug + "." + format);
      await run(["export", document, "--format", format, "--output", output]);
      assert.ok((await readFile(output)).length > 100);
    }
  }
  // Mutation semantics use a deliberate fixture, independent of bundled selection
  // and of the fields provided by any newly authored template.
  const mutation = join(folder, "mutation.slop");
  await run([
    "create",
    "--from",
    resolve("generated/templates/quick-checklist.slop"),
    "--output",
    mutation,
  ]);
  await run([
    "apply",
    mutation,
    "--op",
    JSON.stringify({ type: "set", path: ["title"], value: "Installed helper verified" }),
  ]);
  assert.ok(JSON.parse(await run(["get", mutation])).title.startsWith("Installed helper verified"));
  console.log(
    "PASS packaged starters, matching page shells, installed editing and export without Bun/Node",
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}
