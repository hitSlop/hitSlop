import AppKit
import HitSlopCore
import HitSlopCoreBinding
import SQLite3

/// Test files built the way `slop build` builds them: a stage directory (manifest with its
/// markers, descriptor, initial values, assets, optional artwork) packed by the core into a
/// template, and documents created from templates.
public enum Fixtures {
  public static let repository = URL(fileURLWithPath: String(#filePath.components(separatedBy: "/apps/apple/")[0]))

  public static var engine: URL {
    let environment = ProcessInfo.processInfo.environment
    if let override = environment["HITSLOP_ENGINE"] { return URL(fileURLWithPath: override) }
    let profile = environment["HITSLOP_CARGO_PROFILE"] ?? "release"
    return repository.appendingPathComponent("target/\(profile)/slop-engine")
  }

  /// JSON text (a document's state, a reply) as an object.
  public static func object(_ json: String) throws -> [String: Any] {
    try JSONSerialization.jsonObject(with: Data(json.utf8)) as! [String: Any]
  }
  /// A value as JSON text, its keys sorted.
  public static func json(_ value: Any) throws -> String {
    String(decoding: try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys]), as: UTF8.self)
  }

  /// A fresh, empty temporary folder.
  public static func folder() throws -> URL {
    let folder = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    return folder
  }

  /// A PNG of the given size, transparent unless `draw` paints it (in a context the size
  /// of the image).
  public static func png(width: Int = 8, height: Int = 8, draw: (NSRect) -> Void = { _ in }) throws -> Data {
    guard
      let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)
    else { throw CocoaError(.fileWriteUnknown) }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    draw(NSRect(x: 0, y: 0, width: width, height: height))
    NSGraphicsContext.restoreGraphicsState()
    guard let png = bitmap.representation(using: .png, properties: [:]) else { throw CocoaError(.fileWriteUnknown) }
    return png
  }

  /// A white RGBA PNG whose pixel `(x, y)` (from the top) has alpha `alpha(x, y)`, as a
  /// window skin or mask.
  public static func png(width: Int, height: Int, alpha: (Int, Int) -> UInt8) throws -> Data {
    var pixels = [UInt8](repeating: 255, count: width * height * 4)
    for y in 0..<height { for x in 0..<width { pixels[(y * width + x) * 4 + 3] = alpha(x, y) } }
    let output = NSMutableData()
    guard let provider = CGDataProvider(data: Data(pixels) as CFData),
      let image = CGImage(
        width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent),
      let destination = CGImageDestinationCreateWithData(output, "public.png" as CFString, 1, nil)
    else { throw SlopFailure("Cannot create a test PNG") }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else { throw SlopFailure("Cannot write a test PNG") }
    return output as Data
  }

  /// A PNG's pixels drawn into 8-bit RGBA, to compare images however they are encoded: the
  /// core stores artwork losslessly re-encoded.
  public static func pixels(_ png: Data?) -> Data? {
    guard let png, let source = CGImageSourceCreateWithData(png as CFData, nil),
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil),
      let context = CGContext(
        data: nil, width: image.width, height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
    else { return nil }
    context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
    guard let data = context.data else { return nil }
    return Data(bytes: data, count: image.width * image.height * 4)
  }

  /// A temporary copy of a stage in the repository, for a test to change before packing.
  public static func stage(_ source: String = "tests/fixtures/checklist/document") throws -> URL {
    let stage = try folder().appendingPathComponent("stage")
    try FileManager.default.copyItem(at: repository.appendingPathComponent(source), to: stage)
    return stage
  }

  /// Writes a raw platform probe against the stage's descriptor, as the generated Svelte
  /// entry does. An explicit descriptor stays explicit (including a deliberate mismatch).
  /// Throw-only probes remain unchanged so startup failures keep their intended cause.
  /// Call this after changing the stage descriptor; compiled apps keep their own declaration.
  public static func writeApp(_ source: String, to stage: URL) throws {
    var source = source
    if source.range(of: #"export\s+default\s*\{\s*descriptor\s*:"#, options: .regularExpression) == nil,
      let entry = source.range(of: #"export\s+default\s*\{"#, options: .regularExpression)
    {
      let row = try object(String(contentsOf: stage.appendingPathComponent("app.json"), encoding: .utf8))
      let descriptor = try json(row["descriptor"]!)
      source.replaceSubrange(entry, with: "export default { descriptor: \(descriptor),")
    }
    try Data(source.utf8).write(to: stage.appendingPathComponent("assets/app.js"))
  }

  /// A minimal app's stage: an app that mounts nothing and an empty descriptor. `manifest`
  /// replaces fields of the default manifest; `theme` is written as given, so its colors
  /// keep their order.
  public static func minimalStage(
    slug: String = "fixture", manifest overrides: [String: Any] = [:],
    theme: String = ##"{"accent":"#335577"}"##
  ) throws -> URL {
    let stage = try folder().appendingPathComponent("stage")
    try FileManager.default.createDirectory(
      at: stage.appendingPathComponent("assets"), withIntermediateDirectories: true)
    var manifest: [String: Any] = [
      "author": ["name": "Fixture Author", "url": "https://example.com"], "slug": slug, "title": "Fixture",
      "description": "A test app.", "categories": ["utilities"], "presentation": ["width": 320, "height": 240],
    ]
    manifest.merge(overrides) { $1 }
    let manifestJSON = String(decoding: try JSONSerialization.data(withJSONObject: manifest), as: UTF8.self)
    let app =
      #"{"packageFormat":\#(PackageFormat.level),"runtimeABI":\#(RuntimeABI.level),"manifest":\#(manifestJSON),"descriptor":{"kind":"object","properties":{}},"initial":{},"theme":\#(theme)}"#
    try Data(app.utf8).write(to: stage.appendingPathComponent("app.json"))
    try writeApp("export default { mount() { return {}; } };", to: stage)
    return stage
  }

  /// Changes a stage's `app.json`, the `app` row the build wrote: its manifest, descriptor,
  /// initial values or theme. (The theme's colors may be reordered.)
  public static func updateApp(_ stage: URL, _ change: (inout [String: Any]) throws -> Void) throws {
    let url = stage.appendingPathComponent("app.json")
    var app = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as! [String: Any]
    try change(&app)
    try JSONSerialization.data(withJSONObject: app).write(to: url)
  }

  /// Changes a stage's manifest.
  public static func updateManifest(_ stage: URL, _ change: (inout [String: Any]) throws -> Void) throws {
    try updateApp(stage) { app in
      var manifest = app["manifest"] as! [String: Any]
      try change(&manifest)
      app["manifest"] = manifest
    }
  }

  /// `stage` packed into a template file beside it by the file engine, as `slop build`
  /// packs one.
  public static func template(stage: URL, named name: String = "fixture") throws -> URL {
    let template = stage.deletingLastPathComponent().appendingPathComponent(name + ".slop")
    let (status, _, errors) = try run(
      engine, ["pack", stage.path, template.path])
    guard status == 0 else { throw SlopFailure(errors.trimmingCharacters(in: .whitespacesAndNewlines)) }
    return template
  }

  /// Runs `executable` with `arguments` to completion, writing `input` to its standard
  /// input: its status, standard output and standard error.
  public static func run(_ executable: URL, _ arguments: [String], input: Data? = nil) throws -> (Int32, String, String)
  {
    let process = Process()
    process.executableURL = executable
    process.arguments = arguments
    let (stdout, stderr, stdin) = (Pipe(), Pipe(), Pipe())
    process.standardOutput = stdout
    process.standardError = stderr
    process.standardInput = stdin
    try process.run()
    if let input { stdin.fileHandleForWriting.write(input) }
    try stdin.fileHandleForWriting.close()
    let output = stdout.fileHandleForReading.readDataToEndOfFile()
    let error = stderr.fileHandleForReading.readDataToEndOfFile()
    process.waitUntilExit()
    return (process.terminationStatus, String(decoding: output, as: UTF8.self), String(decoding: error, as: UTF8.self))
  }

  /// A new document from `stage`, a single temporary file unless `destination` names one;
  /// the stage and its template are removed.
  public static func document(stage: URL, at destination: URL? = nil) throws -> URL {
    defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
    return try document(from: template(stage: stage, named: "template"), at: destination)
  }

  /// A new document from the repository stage `source`, unchanged.
  public static func document(_ source: String = "tests/fixtures/checklist/document", at destination: URL? = nil) throws
    -> URL
  {
    try document(stage: stage(source), at: destination)
  }

  /// A new document from a built template, such as `generated/native-fixtures/<slug>.slop`.
  public static func document(from template: URL, at destination: URL? = nil) throws -> URL {
    let document =
      destination ?? FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try createDocument(template: template.path, destination: document.path)
    return document
  }

  /// A document of the core's checklist fixture (`crates/hitslop-core/fixtures`), whose app
  /// is `app` when given.
  public static func checklistDocument(app: String? = nil) throws -> URL {
    let stage = try stage()
    let spec =
      try JSONSerialization.jsonObject(
        with: Data(contentsOf: repository.appendingPathComponent("crates/hitslop-core/fixtures/checklist.json")))
      as! [String: Any]
    try updateApp(stage) { app in
      app["descriptor"] = spec["schema"]
      app["initial"] = spec["initial"]
    }
    let source =
      try app
      ?? String(
        contentsOf: repository.appendingPathComponent("tests/fixtures/checklist/document/assets/app.js"),
        encoding: .utf8)
    try writeApp(source, to: stage)
    return try document(stage: stage)
  }

  /// A copy of a stage the native build prepared (`generated/native-fixtures/<slug>`), the
  /// build of an example app before packing.
  public static func nativeStage(_ slug: String = "quick-checklist") throws -> URL {
    try stage("generated/native-fixtures/\(slug)")
  }

  /// Another connection holding the document's database, as a backup or another SQLite
  /// program would, until `release`. A save meanwhile waits, then fails as busy; with
  /// `readable`, the document can still be read.
  public final class DatabaseHold {
    private var connection: OpaquePointer?
    public init(_ document: URL, readable: Bool = false) throws {
      guard sqlite3_open_v2(document.path, &connection, SQLITE_OPEN_READWRITE, nil) == SQLITE_OK,
        sqlite3_exec(connection, readable ? "BEGIN IMMEDIATE" : "BEGIN EXCLUSIVE", nil, nil, nil) == SQLITE_OK
      else { throw SlopFailure("Cannot hold \(document.lastPathComponent)") }
    }
    public func release() {
      sqlite3_exec(connection, "COMMIT", nil, nil, nil)
      sqlite3_close(connection)
      connection = nil
    }
    deinit { if connection != nil { release() } }
  }

  /// Whether another writer could take the document now.
  public static func isLocked(_ document: URL) -> Bool {
    (try? writerLockHeld(path: document.path)) ?? false
  }

  /// What a document's saved state holds: checkpoint bytes, update bytes and update rows,
  /// read from the file.
  public struct Stored: Sendable {
    public let checkpointBytes: UInt64, updateBytes: UInt64, rows: UInt64
  }
  public static func stored(_ document: URL) throws -> Stored {
    var connection: OpaquePointer?
    var statement: OpaquePointer?
    defer {
      sqlite3_finalize(statement)
      sqlite3_close(connection)
    }
    let sql =
      "SELECT (SELECT coalesce(sum(length(bytes)),0) FROM checkpoint), (SELECT coalesce(sum(length(bytes)),0) FROM updates), (SELECT count(*) FROM updates)"
    guard sqlite3_open_v2(document.path, &connection, SQLITE_OPEN_READONLY, nil) == SQLITE_OK,
      sqlite3_prepare_v2(connection, sql, -1, &statement, nil) == SQLITE_OK, sqlite3_step(statement) == SQLITE_ROW
    else { throw SlopFailure("Cannot read \(document.lastPathComponent)") }
    return Stored(
      checkpointBytes: UInt64(sqlite3_column_int64(statement, 0)),
      updateBytes: UInt64(sqlite3_column_int64(statement, 1)),
      rows: UInt64(sqlite3_column_int64(statement, 2)))
  }

  /// Runs one statement on `file` outside the core, as a damaged or foreign writer would.
  public static func sql(_ file: URL, _ statement: String) throws {
    var db: OpaquePointer?
    defer { sqlite3_close(db) }
    guard sqlite3_open(file.path, &db) == SQLITE_OK, sqlite3_exec(db, statement, nil, nil, nil) == SQLITE_OK
    else { throw SlopFailure(String(cString: sqlite3_errmsg(db))) }
  }

  /// Whether Finder shows a custom icon for `file`: the flag in its Finder info.
  public static func hasCustomIcon(_ file: URL) -> Bool {
    var info = [UInt8](repeating: 0, count: 32)
    guard getxattr(file.path, "com.apple.FinderInfo", &info, info.count, 0, 0) == info.count else { return false }
    return (UInt16(info[8]) << 8 | UInt16(info[9])) & 0x0400 != 0
  }
  /// The document file's size in bytes.
  public static func size(_ document: URL) throws -> UInt64 {
    (try FileManager.default.attributesOfItem(atPath: document.path)[.size] as? NSNumber)?.uint64Value ?? 0
  }

  /// A new document from a template the native build prepared (`generated/native-fixtures`).
  public static func native(_ slug: String = "quick-checklist") throws -> URL {
    try document(from: repository.appendingPathComponent("generated/native-fixtures/\(slug).slop"))
  }
}
