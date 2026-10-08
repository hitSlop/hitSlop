import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { parseManifest } from "../../packages/schema/src/manifest";
import { platformValidatorSource } from "../../scripts/platform-validator";

const root = resolve(import.meta.dir, "../..");
const out = join(root, "generated/manifest-validation-spike");
const workspace = join(out, "workspace");
await mkdir(workspace, { recursive: true });
await cp(join(root, "Cargo.toml"), join(workspace, "Cargo.toml"));
await cp(join(import.meta.dir, "Cargo.lock"), join(workspace, "Cargo.lock"));
await cp(join(root, "crates"), join(workspace, "crates"), { recursive: true });
await cp(join(root, "packages/schema/generated/manifest.schema.json"), join(workspace, "manifest.schema.json"));
const core = join(workspace, "crates/hitslop-core");
await cp(join(import.meta.dir, "validator.rs"), join(core, "src/manifest_spike.rs"));
await mkdir(join(core, "src/bin"), { recursive: true });
await cp(join(import.meta.dir, "bench.rs"), join(core, "src/bin/manifest-bench.rs"));
await writeFile(join(core, "Cargo.toml"), (await readFile(join(core, "Cargo.toml"), "utf8")) + `
jsonschema = { version = "=0.58.3", default-features = false, optional = true }

[features]
manifest-runtime = ["dep:jsonschema"]
manifest-compiled = ["dep:jsonschema", "jsonschema/macros"]
`);
await writeFile(join(core, "src/lib.rs"), (await readFile(join(core, "src/lib.rs"), "utf8")) + "\npub mod manifest_spike;\n");
for (const adapter of ["wasm", "ffi"]) {
  const path = join(workspace, `crates/hitslop-core-${adapter}/src/lib.rs`);
  const exportCode = adapter === "wasm"
    ? '\n#[wasm_bindgen(js_name = manifestProbe)]\npub fn manifest_probe(input: &str) -> String { hitslop_core::manifest_spike::probe(input) }\n'
    : '\n#[uniffi::export]\npub fn manifest_probe(input: String) -> String { hitslop_core::manifest_spike::probe(&input) }\n';
  await writeFile(path, (await readFile(path, "utf8")) + exportCode);
}

const base = JSON.parse(await readFile(join(root, "examples/slops/quick-checklist/manifest.json"), "utf8"));
const cases: { name: string; input: string; typebox: boolean }[] = [];
function add(name: string, patch: (value: any) => void = () => {}) {
  const value = structuredClone(base); patch(value);
  let typebox = true;
  try { parseManifest(value); } catch { typebox = false; }
  cases.push({ name, input: JSON.stringify(value), typebox });
}
add("baseline");
for (const n of [40, 41, 80, 81]) add(`title-${n}-emoji`, v => v.title = "😀".repeat(n));
for (const n of [0, 80, 81]) add(`title-${n}-ascii`, v => v.title = "a".repeat(n));
add("combining-title", v => v.title = "e\u0301".repeat(41));
for (const name of [" ", "\t\n", "\u00a0", "\u0085", "\ufeff", "Alice"]) add(`author-${JSON.stringify(name)}`, v => v.author.name = name);
for (const url of ["https://example.com", "http://example.com/a?q=b#c", "https://example.com/%20", "ftp://example.com", "mailto:a@example.com", "example.com", "https://", "https://example.com/a b", "https://example.com/%zz", "https://[bad]", "https://example.com/😀", "https://例え.jp", "https://example.com\n"]) {
  add(`url-${JSON.stringify(url)}`, v => v.author.url = url);
}
add("unknown-root", v => v.future = true);
add("unknown-author", v => v.author.future = true);
add("missing-author", v => delete v.author);
add("duplicate-category", v => v.categories = ["utilities", "utilities"]);
add("unknown-category", v => v.categories = ["future"]);
add("three-categories", v => v.categories = ["utilities", "games", "media"]);
add("empty-categories", v => v.categories = []);
add("slug-newline", v => v.slug = "test\n");
add("slug-invalid", v => v.slug = "UPPER");
add("wrong-schema-url", v => v.$schema = "https://example.com/schema");
for (const width of [239, 240, 4096, 4097, 240.5, "400", null, true]) add(`width-${JSON.stringify(width)}`, v => v.presentation.width = width);
for (const height of [179, 180, 4096, 4097]) add(`height-${height}`, v => v.presentation.height = height);
add("unknown-presentation", v => v.presentation.future = true);
add("default-shape", v => delete v.presentation.shape);
for (const shape of ["0", "5px", "5.0px", ".5%", "0 / 20% 30%", "5.px", "5.%", "calc(5px)", "-1px", "1e309px", "1px 2px 3px 4px 5px", null,
  { path: "M0 0 L100 0 L100 100 Z", viewBox: [100, 100], fillRule: "evenodd" },
  { path: "M0 0X" }, { path: "M,0 0" }, { path: "M0 0Z 1 1" }, { path: "M1e309 0" },
  ...[[100], [100, 100, 100], [0, 100], [1, 16385], [1.5, 100], ["100", 100], [100, null]].map(viewBox => ({ path: "M0 0L1 1", viewBox })),
  { path: "M0 0L1 1", fillRule: "future" }, { path: "M0 0L1 1", future: true },
]) add(`shape-${JSON.stringify(shape)}`, v => v.presentation.shape = shape);
for (const skin of ["assets/skin.png", "assets/nested/skin.PNG", "assets/../skin.png", "assets/a/../../skin.png", "/assets/skin.png", "assets/skin.jpg", "assets/a//skin.png", "assets/a/./skin.png", "assets/skin.png\n"]) {
  add(`skin-${JSON.stringify(skin)}`, v => v.presentation = { width: 400, height: 300, skin });
}
add("mixed-presentation", v => v.presentation.skin = "assets/skin.png");
for (const file of new Bun.Glob("examples/slops/*/manifest.json").scanSync(root)) {
  const value = JSON.parse(await readFile(join(root, file), "utf8"));
  let typebox = true; try { parseManifest(value); } catch { typebox = false; }
  cases.push({ name: file, input: JSON.stringify(value), typebox });
}
await writeFile(join(out, "cases.json"), JSON.stringify(cases, null, 2) + "\n");
await writeFile(join(out, "PlatformContract.swift"), platformValidatorSource);
await cp(join(import.meta.dir, "swift-probe.swift"), join(out, "main.swift"));
console.log(`Prepared ${cases.length} cases and disposable workspace at ${workspace}`);
