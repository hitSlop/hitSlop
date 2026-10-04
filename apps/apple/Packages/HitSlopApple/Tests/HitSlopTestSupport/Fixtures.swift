import AppKit
import HitSlopCoreBinding

/// Test files built the way `slop build` builds them: a stage directory (manifest with its
/// markers, descriptor, initial values, assets, optional artwork) packed by the core into a
/// template, and documents created from templates.
public enum Fixtures {
  public static let repository = URL(fileURLWithPath: String(#filePath.components(separatedBy: "/apps/apple/")[0]))

  /// A fresh, empty temporary folder.
  public static func folder() throws -> URL {
    let folder = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-\(UUID().uuidString)")
    try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
    return folder
  }

  /// A PNG of the given size, transparent unless `draw` paints it (in a context the size
  /// of the image).
  public static func png(width: Int = 8, height: Int = 8, draw: (NSRect) -> Void = { _ in }) throws -> Data {
    guard let bitmap = NSBitmapImageRep(
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

  /// A temporary copy of a stage in the repository, for a test to change before packing.
  public static func stage(_ source: String = "tests/fixtures/checklist/document") throws -> URL {
    let stage = try folder().appendingPathComponent("stage")
    try FileManager.default.copyItem(at: repository.appendingPathComponent(source), to: stage)
    return stage
  }

  /// A minimal app's stage: an app that mounts nothing and an empty descriptor. `manifest`
  /// replaces fields of the default manifest; `theme` is written as given, so its colors
  /// keep their order.
  public static func minimalStage(
    slug: String = "fixture", manifest overrides: [String: Any] = [:],
    theme: String = ##"{"accent":"#335577"}"##
  ) throws -> URL {
    let stage = try folder().appendingPathComponent("stage")
    try FileManager.default.createDirectory(at: stage.appendingPathComponent("assets"), withIntermediateDirectories: true)
    try Data("export default { mount() { return {}; } };".utf8).write(to: stage.appendingPathComponent("assets/app.js"))
    var manifest: [String: Any] = [
      "author": ["name": "Fixture Author", "url": "https://example.com"], "slug": slug, "title": "Fixture",
      "description": "A test app.", "categories": ["utilities"], "presentation": ["width": 320, "height": 240],
    ]
    manifest.merge(overrides) { $1 }
    let manifestJSON = String(decoding: try JSONSerialization.data(withJSONObject: manifest), as: UTF8.self)
    let app = #"{"packageFormat":1,"runtimeABI":1,"manifest":\#(manifestJSON),"descriptor":{"kind":"object","properties":{}},"initial":{},"theme":\#(theme)}"#
    try Data(app.utf8).write(to: stage.appendingPathComponent("app.json"))
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

  /// `stage` packed into a template file beside it.
  public static func template(stage: URL, named name: String = "fixture") throws -> URL {
    let template = stage.deletingLastPathComponent().appendingPathComponent(name + ".slop")
    try packTemplate(stage: stage.path, destination: template.path)
    return template
  }

  /// A new document from `stage`, a single temporary file unless `destination` names one;
  /// the stage and its template are removed.
  public static func document(stage: URL, at destination: URL? = nil) throws -> URL {
    defer { try? FileManager.default.removeItem(at: stage.deletingLastPathComponent()) }
    return try document(from: template(stage: stage, named: "template"), at: destination)
  }

  /// A new document from the repository stage `source`, unchanged.
  public static func document(_ source: String = "tests/fixtures/checklist/document", at destination: URL? = nil) throws -> URL {
    try document(stage: stage(source), at: destination)
  }

  /// A new document from a built template, such as `generated/native-fixtures/<slug>.slop`.
  public static func document(from template: URL, at destination: URL? = nil) throws -> URL {
    let document = destination ?? FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try createDocument(template: template.path, destination: document.path)
    return document
  }

  /// A copy of a stage the native build prepared (`generated/native-fixtures/<slug>`), the
  /// build of an example app before packing.
  public static func nativeStage(_ slug: String = "quick-checklist") throws -> URL {
    try stage("generated/native-fixtures/\(slug)")
  }

  /// A new document from a template the native build prepared (`generated/native-fixtures`).
  public static func native(_ slug: String = "quick-checklist") throws -> URL {
    try document(from: repository.appendingPathComponent("generated/native-fixtures/\(slug).slop"))
  }
}
