import { readFile, writeFile, copyFile, mkdir } from "node:fs/promises";
import { resolve, join, dirname } from "node:path";

const root = resolve(import.meta.dir, "../..");
const out = join(root, "generated/native-manifest-integration");
const workspace = join(out, "workspace");
const apple = "apps/apple/Packages/HitSlopApple";
const changed: string[] = [];
try {
  await readFile(join(out, "candidate-files.json"));
  throw new Error("Candidate already applied; reuse this experiment.");
} catch (error: any) {
  if (error.code !== "ENOENT") throw error;
}
async function edit(path: string, change: (source: string) => string) {
  const original = await readFile(join(workspace, path), "utf8");
  const result = change(original);
  if (result === original) throw new Error(`No change: ${path}`);
  await mkdir(dirname(join(out, "before", path)), { recursive: true });
  // Test harness regressions belong in the candidate patch, but not the baseline.
  const before = path.endsWith("SlopPackageTests.swift")
    ? await readFile(join(root, path), "utf8")
    : original;
  await writeFile(join(out, "before", path), before);
  await writeFile(join(workspace, path), result);
  changed.push(path);
}
function replace(source: string, before: string, after: string) {
  if (!source.includes(before)) throw new Error(`Missing source: ${before}`);
  return source.replace(before, after);
}
async function add(path: string, local: string) {
  await copyFile(join(import.meta.dir, local), join(workspace, path));
  changed.push(path);
}

await edit(
  "crates/hitslop-core/Cargo.toml",
  (s) =>
    s +
    `
[features]
manifest-validation = ["dep:jsonschema"]

[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
jsonschema = { version = "=0.58.3", default-features = false, features = ["macros"], optional = true }
`,
);
await edit("crates/hitslop-core-ffi/Cargo.toml", (s) =>
  replace(
    s,
    'hitslop-core = { path = "../hitslop-core" }',
    'hitslop-core = { path = "../hitslop-core", features = ["manifest-validation"] }',
  ),
);
await edit(
  "crates/hitslop-core/src/lib.rs",
  (s) =>
    s +
    '\n#[cfg(all(feature = "manifest-validation", not(target_arch = "wasm32")))]\npub mod manifest;\n',
);
await add("crates/hitslop-core/src/manifest.rs", "manifest.rs");
await add("crates/hitslop-core/tests/manifest.rs", "manifest-test.rs");
await edit("crates/hitslop-core/src/shape.rs", (s) =>
  replace(
    s,
    "    match shape {\n        Shape::Radius",
    `    normalize(shape, width, height)
}

#[cfg(all(feature = "manifest-validation", not(target_arch = "wasm32")))]
pub(crate) fn silhouette_value(shape: Option<&serde_json::Value>, width: f64, height: f64) -> Result<Silhouette> {
    let shape = match shape {
        None => Shape::Radius(DEFAULT_RADIUS.into()),
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| invalid())?,
    };
    normalize(shape, width, height)
}

fn normalize(shape: Shape, width: f64, height: f64) -> Result<Silhouette> {
    match shape {
        Shape::Radius`,
  ),
);
await edit("crates/hitslop-core-ffi/src/lib.rs", (s) =>
  replace(
    replace(
      s,
      "/// Parses a manifest `shape` (JSON, or none for the default) for a window of this size.\n#[uniffi::export]\npub fn window_silhouette(shape_json: Option<String>, width: f64, height: f64) -> Result<WindowSilhouette, CoreError> {",
      "/// Validates the manifest contract and returns its normalized window geometry.\n#[uniffi::export]\npub fn validate_manifest(manifest_json: String) -> Result<WindowSilhouette, CoreError> {",
    ),
    "shape::silhouette(shape_json.as_deref(), width, height)",
    "hitslop_core::manifest::validate(&manifest_json)",
  ),
);
await edit(`${apple}/Sources/HitSlopCore/SlopPackage.swift`, (s) => {
  s = replace(s, "import Foundation", "import Foundation\nimport HitSlopCoreBinding");
  s = replace(
    s,
    "manifest = try Self.decodeManifest(manifestData)",
    "let decoded = try Self.decodeManifest(manifestData)\n    manifest = decoded.manifest",
  );
  s = replace(
    s,
    "silhouette = try SlopSilhouette(shape: manifest.presentation.shape, width: manifest.presentation.width, height: manifest.presentation.height)",
    "silhouette = SlopSilhouette(parsed: decoded.silhouette)",
  );
  const start = s.indexOf("  private static func decodeManifest(");
  const end = s.indexOf("\n  private func validateState()", start);
  if (start < 0 || end < 0) throw new Error("decodeManifest not found");
  return (
    s.slice(0, start) +
    `  private static func decodeManifest(_ data: Data) throws -> (manifest: SlopManifest, silhouette: WindowSilhouette) {
    guard let json = String(data: data, encoding: .utf8) else {
      throw SlopPackageError.invalid("manifest.json must be UTF-8")
    }
    do {
      let silhouette = try validateManifest(manifestJson: json)
      return (try JSONDecoder().decode(SlopManifest.self, from: data), silhouette)
    } catch let CoreError.Rejected(_, message, _) {
      throw SlopPackageError.invalid(message)
    } catch {
      throw SlopPackageError.invalid(error.localizedDescription)
    }
  }
` +
    s.slice(end)
  );
});
await edit(`${apple}/Sources/HitSlopCore/SlopSilhouette.swift`, (s) => {
  s = replace(s, "(`window_silhouette`)", "during manifest validation");
  const start = s.indexOf("  public init(shape:");
  const end = s.indexOf("    switch parsed {", start);
  if (start < 0 || end < 0) throw new Error("silhouette init not found");
  return s.slice(0, start) + "  public init(parsed: WindowSilhouette) {\n" + s.slice(end);
});
await edit("scripts/generate.ts", (s) =>
  replace(
    s,
    `      '\\nnonisolated(unsafe) public let manifestSchema: [String: Any] = try! JSONSerialization.jsonObject(with: Data(#"' +
      JSON.stringify(SlopManifestSchema) +
      '"#.utf8)) as! [String: Any]\\nnonisolated(unsafe) public let bridgeValidationSchema: [String: Any] = try! JSONSerialization.jsonObject(with: Data(#"' +`,
    `      '\\nnonisolated(unsafe) public let bridgeValidationSchema: [String: Any] = try! JSONSerialization.jsonObject(with: Data(#"' +`,
  ),
);
// Save generated output before regenerating it through the normal entry point.
const generated = `${apple}/Sources/HitSlopCore/Generated/PlatformValidation.generated.swift`;
await mkdir(dirname(join(out, "before", generated)), { recursive: true });
await copyFile(join(workspace, generated), join(out, "before", generated));
changed.push(generated);
await edit(
  `${apple}/Tests/HitSlopCoreTests/SlopPackageTests.swift`,
  (s) =>
    s.replace(/^.*#expect\(!PlatformContract.valid\(.*against: manifestSchema\)\).*\n/gm, "") +
    `
@Test func invalidShapesAreRefusedAsInvalidPackages() throws {
    let root = try fixture(); defer { try? FileManager.default.removeItem(at: root.deletingLastPathComponent()) }
    let url = root.appendingPathComponent("manifest.json")
    var manifest = try #require(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
    for shape: Any in ["1em", ["path": "M0 0L1"]] {
        manifest["presentation"] = ["width": 320, "height": 240, "shape": shape]
        try JSONSerialization.data(withJSONObject: manifest).write(to: url)
        #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
    }
}
`,
);
await edit(`${apple}/Tests/HitSlopCoreTests/SlopSilhouetteTests.swift`, (s) => {
  s = replace(s, "import Testing", "import Testing\nimport HitSlopCoreBinding");
  const start = s.indexOf("// The grammar and its corpus");
  const end = s.indexOf("@Test func silhouettesPreserve", start);
  s =
    s.slice(0, start) +
    `// Rust owns parsing; these tests exercise native path construction from its output.
private func silhouette(shape: SlopShape, width: Int, height: Int) throws -> SlopSilhouette {
  let object: [String: Any] = [
    "author": ["name": "Lab"], "slug": "shape-lab", "title": "Lab", "description": "Geometry", "categories": ["developer-tools"],
    "presentation": ["width": max(240, width), "height": max(180, height),
      "shape": try JSONSerialization.jsonObject(with: JSONEncoder().encode(shape), options: .fragmentsAllowed)],
  ]
  let json = String(decoding: try JSONSerialization.data(withJSONObject: object), as: UTF8.self)
  return SlopSilhouette(parsed: try validateManifest(manifestJson: json))
}
` +
    s.slice(end);
  s = s.replaceAll("try SlopSilhouette(shape:", "try silhouette(shape:");
  s = replace(
    s,
    "@Test func nativeManifestValidatorChecksViewBoxTupleMembers() {",
    "@Test func nativeManifestValidatorChecksViewBoxTupleMembers() throws {",
  );
  return replace(
    s,
    "    #expect(!PlatformContract.valid(manifest, against: manifestSchema))",
    `    let json = String(decoding: try JSONSerialization.data(withJSONObject: manifest), as: UTF8.self)
    #expect(throws: CoreError.self) { _ = try validateManifest(manifestJson: json) }`,
  );
});
await edit("docs/architecture.md", (s) =>
  replace(
    s,
    "from one tree; nothing has shipped, so there is no version negotiation between them.",
    "from one tree; nothing has shipped, so there is no version negotiation between them.\n\nManifest acceptance is native-only Rust validation of the TypeBox-generated JSON\nSchema, followed by the shared shape parser. Swift decodes the validated manifest\ninto its generated model and owns filesystem, PNG and native path checks. Authoring\nkeeps TypeBox manifest validation; the manifest validator dependency is excluded from\nWASM. Swift's schema interpreter still validates socket and page envelopes.",
  ),
);
await edit("packages/document/tests/cli.native.test.ts", (s) =>
  replace(
    s,
    '    expect((await cli("get")).error).toContain("invalid manifest");',
    '    const refused = await cli("get");\n    expect(refused.code).not.toBe(0);\n    expect(refused.error).toContain("Invalid manifest.json at /");',
  ),
);
// Only the measurement executable gets this switch; production has one native path.
const packageFile = join(workspace, apple, "Package.swift");
await writeFile(
  packageFile,
  replace(
    await readFile(packageFile, "utf8"),
    '.executableTarget(name: "ManifestIntegrationProbe", dependencies: ["HitSlopCore", "HitSlopCoreBinding"]),',
    '.executableTarget(name: "ManifestIntegrationProbe", dependencies: ["HitSlopCore", "HitSlopCoreBinding"], swiftSettings: [.define("CANDIDATE_MANIFEST_SPIKE")]),',
  ),
);
await mkdir(join(out, "before"), { recursive: true });
await copyFile(join(workspace, "Cargo.lock"), join(out, "before/Cargo.lock"));
await copyFile(join(root, "spikes/manifest-validation/Cargo.lock"), join(workspace, "Cargo.lock"));
changed.push("Cargo.lock");
await writeFile(join(out, "candidate-files.json"), JSON.stringify(changed, null, 2));
console.log(`Applied candidate to ${workspace}. Run schema:generate and refresh Cargo.lock there.`);
