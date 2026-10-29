import { buildTemplate } from "../../packages/tinys/src/cli/template";
import { builtTemplates } from "../templates/discover";
import { digest, fileDigest, shellDestinations, shellFiles } from "../lib/artifacts";
import { strict as assert } from "node:assert";
import { mkdtemp, readdir, readFile, rm } from "node:fs/promises";
import { join, resolve, dirname } from "node:path";
import { tmpdir } from "node:os";
import { run } from "../../packages/tinys/src/cli/process";
import { createDocument, engineRequest } from "../lib/native";
import { verifyAppCommands, verifyAppCorpus } from "./app-commands";
import { verifyArtifactCapabilities } from "./artifact-capabilities";
const args = process.argv.slice(2);
const unsigned = args.includes("--unsigned");
const app = resolve(args.find((arg) => arg !== "--unsigned") ?? "generated/app/tinys.app");
// Both bundles compile the intent directly. A package reference to a statically
// linked module extracts JSON but cannot be resolved by macOS's intent registry.
for (const bundle of [app, join(app, "Contents/PlugIns/tinys-Widgets.appex")]) {
  const metadata = join(bundle, "Contents/Resources/Metadata.appintents");
  const actions = JSON.parse(await readFile(join(metadata, "extract.actionsdata"), "utf8"));
  assert.ok(actions.actions?.TinyWidgetIntent?.parameters?.some((p: { name: string }) => p.name === "tiny"), "Missing widget document selection metadata");
  assert.ok(actions.entities?.TinyWidgetEntity && actions.queries?.TinyWidgetQuery, "Missing widget document query metadata");
  const packages = await readFile(join(metadata, "extract.packagedata"), "utf8").catch((error) => {
    if (error.code === "ENOENT") return "{}";
    throw error;
  });
  assert.deepEqual(JSON.parse(packages).includes ?? [], [], "Widget intent metadata must not reference an unbundled static package");
}
const evaluator = join(app, "Contents/Helpers/tinys-evaluator");
await run(["/usr/bin/codesign", "--verify", "--strict", evaluator]);
await verifyAppCommands(app);
await verifyAppCorpus(app, { allowUnreleased: unsigned });
const documentEngine = await (await import("../../packages/tinys/src/cli/engine")).findEngine();
// The host bundles the page shell, byte-identical to the build; no engine WASM.
const shells = [...new Bun.Glob("**/shell/boot.js").scanSync({ cwd: app, onlyFiles: true })].map((p) =>
  dirname(join(app, p)),
);
assert.ok(shells.some((root) => root.startsWith(join(app, "Contents/Resources") + "/")), "Missing app page shell");
const expected = await digest(shellDestinations.app, shellFiles);
for (const root of shells) assert.equal(await digest(root, shellFiles), expected, `Page shell differs: ${root}`);
assert.deepEqual([...new Bun.Glob("**/*.wasm").scanSync({ cwd: app, onlyFiles: true })], [], "The app must not bundle WASM");
const folder = await mkdtemp(join(tmpdir(), "tinys-release-verify-"));
try {
  // Installed, without Bun or Node on the path.
  const placement = { engine: documentEngine, cwd: folder, env: { HOME: process.env.HOME, TMPDIR: process.env.TMPDIR, PATH: "/usr/bin:/bin" } };
  const installed = (args: string[], executable: string) => run([executable, ...args], placement);
  const value = async (document: string) => (await engineRequest({ method: "get", documentPath: document }, placement)).state.value;
  const appCore = (await installed(["--build-id"], join(app, "Contents/MacOS/tinys"))).trim();
  assert.ok(appCore.length > 0, "App did not identify its document core");
  assert.equal(await installed(["--build-id"], documentEngine).then(s => s.trim()), appCore, "App and document engine embed different cores");
  assert.deepEqual([...new Bun.Glob("**/tiny-engine").scanSync({ cwd: app })], [], "The Mac app must not bundle the npm engine");
  // A release is built from one tree, so the CLI's authoring core is the app's core.
  // Finder shows a .tiny through the app's type declaration and its Quick Look extensions,
  // which read the file with the same core.
  const plist = async (path: string) => JSON.parse(await installed(["-convert", "json", "-o", "-", path], "/usr/bin/plutil"));
  const info = await plist(join(app, "Contents/Info.plist"));
  await verifyArtifactCapabilities(info, app, unsigned);
  const type = info.UTExportedTypeDeclarations?.find((t: { UTTypeIdentifier: string }) => t.UTTypeIdentifier === "click.tinys.document");
  assert.ok(type?.UTTypeConformsTo?.includes("public.data"), "The .tiny type must be a plain data file");
  const badge = type?.UTTypeIcons?.UTTypeIconBadgeName;
  assert.ok(badge, "The .tiny type has no document icon");
  const assets = await installed(["assetutil", "--info", join(app, "Contents/Resources/Assets.car")], "/usr/bin/xcrun");
  assert.ok(assets.includes(`"Name" : "${badge}"`), `The asset catalog has no ${badge} image`);
  for (const kind of ["Preview", "Thumbnail"]) {
    const appex = join(app, `Contents/PlugIns/tinys-QuickLook${kind}.appex`);
    const extension = await plist(join(appex, "Contents/Info.plist"));
    assert.ok(
      extension.NSExtension?.NSExtensionAttributes?.QLSupportedContentTypes?.includes("click.tinys.document"),
      `Quick Look ${kind} does not handle .tiny files`,
    );
    let embedded = false;
    for (const binary of await readdir(join(appex, "Contents/MacOS")))
      embedded ||= (await readFile(join(appex, "Contents/MacOS", binary))).includes(appCore);
    assert.ok(embedded, `Quick Look ${kind} embeds a different document core`);
  }
  const selected = (await builtTemplates()).templates.filter((t) => t.bundled).map((t) => t.slug);
  const widget = await plist(join(app, "Contents/PlugIns/tinys-Widgets.appex/Contents/Info.plist"));
  assert.equal(widget.NSExtension?.NSExtensionPointIdentifier, "com.apple.widgetkit-extension", "Missing image widget extension");
  assert.ok(info.CFBundleURLTypes?.some((entry: { CFBundleURLSchemes?: string[] }) => entry.CFBundleURLSchemes?.includes("tinys")), "Missing widget document-opening scheme");
  const starters = join(app, "Contents/Resources/StarterTemplates");
  assert.deepEqual((await readdir(starters)).sort(), selected.map((slug) => slug + ".tiny").sort());
  const exhaustive = process.env.TINYS_TEMPLATE_EXHAUSTIVE === "1";
  // The sampled starter is a packaging smoke, independent of its name or schema.
  const fixtures = selected.slice(0, 1);
  for (const slug of selected) {
    console.log(`Validating packaged template: ${slug}`);
    const source = join(app, "Contents/Resources/StarterTemplates", slug + ".tiny");
    // The build validated each template; the packaged one must be the same bytes.
    assert.equal(
      await fileDigest(source),
      await fileDigest(resolve("generated/templates", slug + ".tiny")),
      `Packaged template differs from build: ${slug}`,
    );
    if (!exhaustive && !fixtures.includes(slug)) continue;
    console.log(`Exercising installed create/reopen/export: ${slug}`);
    const document = join(folder, slug + ".tiny");
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
  const mutation = join(folder, "mutation.tiny");
  const fixture = await buildTemplate(resolve("tests/apps/document"), join(folder, "fixture.tiny"), { renderViews: false });
  await createDocument(fixture, mutation, placement);
  await engineRequest({ method: "batch", documentPath: mutation, batch: { intents: [{ type: "set", path: ["title"], value: "Installed engine verified" }] } }, placement);
  assert.ok(String((await value(mutation) as { title: unknown }).title).startsWith("Installed engine verified"));
  console.log(
    "PASS packaged starters, matching page shells, installed editing and export without Bun/Node",
  );
} finally {
  await rm(folder, { recursive: true, force: true });
}
