import { readFile, writeFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { out, root, command } from "./harness";
const name = process.argv[2]!;
const workspace = join(out, name);
const native = "apps/apple/Packages/HitSlopApple";
const owner = join(workspace, native, "Sources/HitSlopDocument/DocumentOwner.swift");
let source = await readFile(owner, "utf8");
if (!source.includes("func spikeVersion")) {
source = source.replace("  public func state()", `  // Experiment-only access: never included in the production recommendation.
  func spikeVersion() async throws -> String { try await enqueue { try self.core.version() } }
  func spikeCheckpoint() async throws -> Data { try await enqueue { try self.core.checkpoint() } }
  func spikeExport(_ base: String) async throws -> Data { try await enqueue { try self.core.exportSince(version: base) } }
  func spikeImport(_ bytes: Data) async throws {
    try await enqueue {
      try self.requireEditable()
      let publication = try self.core.spikeImport(bytes: bytes)
      self.didEdit(publication, sequence: Int(try self.core.sequence()))
    }
  }

  public func state()`);
await writeFile(owner, source);
}
const ffi = join(workspace, "crates/hitslop-core-ffi/src/lib.rs");
source = await readFile(ffi, "utf8");
if (!source.includes("pub fn spike_import")) source = source.replace("    pub fn sequence(&self)", `    /// Spike-only adapter, absent from production.
    pub fn spike_import(&self, bytes: Vec<u8>) -> Result<String, CoreError> {
        self.call(|d| d.import(&bytes))
    }
    pub fn sequence(&self)`);
await writeFile(ffi, source);
for (const [file, target] of [["BoundarySpike.swift", "HitSlopDocumentTests"], ["PackageRegression.swift", "HitSlopCoreTests"]]) {
  let contents = await readFile(join(root, "spikes/boundary-simplification", file!), "utf8");
  contents = contents.replaceAll("SPIKE_PHASE", ["C", "D"].includes(name) ? "checkpoint" : "append");
  await writeFile(join(workspace, native, "Tests", target, file!), contents);
}
await mkdir(join(workspace, ".hitslop"), { recursive: true });
if (["B", "C", "D"].includes(name)) {
  const r = await command(["/bin/cp", "-cR", join(out, "A", native, "Generated") + "/.", join(workspace, native, "Generated")]);
  if (r.code) throw new Error(r.stderr);
}
const storage = join(workspace, native, "Sources/HitSlopDocument/Storage.swift");
source = await readFile(storage, "utf8");
if (!source.includes("spikeWrittenBytes")) {
source = source.replace("  static let maximumBytes: Int64 = 32 * 1024 * 1024", '  static let maximumBytes: Int64 = ProcessInfo.processInfo.environment["SPIKE_STORAGE_BYTES"].flatMap(Int64.init) ?? 32 * 1024 * 1024\n  private(set) var spikeWrittenBytes: Int64 = 0\n  private(set) var spikeWriteMS: [Double] = []');
source = source.replace('      let method: String', '      let method: String\n      let writtenBytes: Int');
source = source.replace('        method = "append"', '        method = "append"\n        writtenBytes = bytes.count').replace('        method = "checkpoint"', '        method = "checkpoint"\n        writtenBytes = bytes.count');
source = source.replace('    try exec("BEGIN IMMEDIATE")', '    let started = DispatchTime.now().uptimeNanoseconds\n    defer { spikeWriteMS.append(Double(DispatchTime.now().uptimeNanoseconds - started) / 1e6) }\n    try exec("BEGIN IMMEDIATE")');
source = source.replace('      try exec("COMMIT")\n      #if DEBUG\n      try testingPhase?(method', '      try exec("COMMIT")\n      spikeWrittenBytes += Int64(writtenBytes)\n      #if DEBUG\n      try testingPhase?(method');
}
if (!source.includes("let started = DispatchTime.now()")) {
  source = source.replace('    try exec("BEGIN IMMEDIATE")', '    let started = DispatchTime.now().uptimeNanoseconds\n    defer { spikeWriteMS.append(Double(DispatchTime.now().uptimeNanoseconds - started) / 1e6) }\n    try exec("BEGIN IMMEDIATE")');
}
await writeFile(storage, source);

