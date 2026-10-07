import { builtTemplates } from "../templates/discover";
import { digest, fileDigest, shellDestinations, shellFiles } from "../lib/artifacts";
import { strict as assert } from "node:assert";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { join, resolve, dirname } from "node:path";
import { tmpdir } from "node:os";
import { run } from "../../packages/hitslop/src/cli/process";
import { createDocument, engineRequest } from "../lib/native";
const app = resolve(process.argv[2] ?? "generated/app/hitSlop.app");
const helper = join(app, "Contents/Helpers/hitslop-native");
const evaluator = join(app, "Contents/Helpers/hitslop-evaluator");
await run(["/usr/bin/codesign", "--verify", "--strict", evaluator]);
const documentEngine = await (await import("../../packages/hitslop/src/cli/engine")).findEngine();
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
  // Installed, without Bun or Node on the path.
  const placement = { engine: documentEngine, cwd: folder, env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin", HITSLOP_NATIVE_CLI: helper } };
  const installed = (args: string[], executable = helper) => run([executable, ...args], placement);
  const value = async (document: string) => (await engineRequest({ method: "get", documentPath: document }, placement)).state.value;
  const appCore = (await installed(["--core-build"], join(app, "Contents/MacOS/hitSlop"))).trim();
  const helperCore = (await installed(["--core-build"])).trim();
  assert.ok(appCore.length > 0, "App did not identify its document core");
  assert.equal(appCore, helperCore, "App and helper embed different document cores");
  assert.equal(await installed(["--build-id"], documentEngine).then(s => s.trim()), helperCore, "Document engine and helper embed different cores");
  assert.deepEqual(JSON.parse(await installed(["--protocol"], documentEngine)), JSON.parse(await installed(["--protocol"])), "Native tools serve different command protocols");
  assert.deepEqual([...new Bun.Glob("**/slop-engine").scanSync({ cwd: app })], [], "The Mac app must not bundle the npm engine");
  // A release is built from one tree, so the CLI's authoring core is the app's core.
  // Finder shows a .slop through the app's type declaration and its Quick Look extensions,
  // which read the file with the same core.
  const plist = async (path: string) => JSON.parse(await installed(["-convert", "json", "-o", "-", path], "/usr/bin/plutil"));
  const info = await plist(join(app, "Contents/Info.plist"));
  const type = info.UTExportedTypeDeclarations?.find((t: { UTTypeIdentifier: string }) => t.UTTypeIdentifier === "com.hitslop.slop");
  assert.ok(type?.UTTypeConformsTo?.includes("public.data"), "The .slop type must be a plain data file");
  const badge = type?.UTTypeIcons?.UTTypeIconBadgeName;
  assert.ok(badge, "The .slop type has no document icon");
  const assets = await installed(["assetutil", "--info", join(app, "Contents/Resources/Assets.car")], "/usr/bin/xcrun");
  assert.ok(assets.includes(`"Name" : "${badge}"`), `The asset catalog has no ${badge} image`);
  for (const kind of ["Preview", "Thumbnail"]) {
    const appex = join(app, `Contents/PlugIns/hitSlop-QuickLook${kind}.appex`);
    const extension = await plist(join(appex, "Contents/Info.plist"));
    assert.ok(
      extension.NSExtension?.NSExtensionAttributes?.QLSupportedContentTypes?.includes("com.hitslop.slop"),
      `Quick Look ${kind} does not handle .slop files`,
    );
    let embedded = false;
    for (const binary of await readdir(join(appex, "Contents/MacOS")))
      embedded ||= (await readFile(join(appex, "Contents/MacOS", binary))).includes(helperCore);
    assert.ok(embedded, `Quick Look ${kind} embeds a different document core`);
  }
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
    // The build validated each template; the packaged one must be the same bytes.
    assert.equal(
      await fileDigest(source),
      await fileDigest(resolve("generated/templates", slug + ".slop")),
      `Packaged template differs from build: ${slug}`,
    );
    if (!exhaustive && !fixtures.includes(slug)) continue;
    console.log(`Exercising installed create/reopen/export: ${slug}`);
    const document = join(folder, slug + ".slop");
    await createDocument(source, document, placement);
    const initial = await value(document);
    assert.ok(initial && typeof initial === "object");
    assert.ok((await engineRequest({ method: "schema", file: document }, placement)).schema);
    assert.deepEqual(await value(document), initial);
    for (const format of ["png", "pdf"] as const) {
      const output = join(folder, slug + "." + format);
      await engineRequest({ method: "export", documentPath: document, format, output }, placement);
      assert.ok((await readFile(output)).length > 100);
    }
  }
  // Mutation semantics use a deliberate fixture, independent of bundled selection
  // and of the fields provided by any newly authored template.
  const mutation = join(folder, "mutation.slop");
  await createDocument(resolve("generated/templates/quick-checklist.slop"), mutation, placement);
  await engineRequest({ method: "batch", documentPath: mutation, batch: { intents: [{ type: "set", path: ["title"], value: "Installed engine verified" }] } }, placement);
  assert.ok(String((await value(mutation) as { title: unknown }).title).startsWith("Installed engine verified"));
  console.log(
    "PASS packaged starters, matching page shells, installed editing and export without Bun/Node",
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}
