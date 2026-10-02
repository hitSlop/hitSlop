import Foundation
import Testing
@testable import HitSlopCore

@Suite struct PackageValidationTests {
  @Test(arguments: ["descriptor", "theme", "font", "utf8"])
  func refusesInvalidPackageInput(kind: String) throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    let (file, bytes): (String, Data) = switch kind {
    case "descriptor": ("state.schema.json", Data(#"{"kind":"object","properties":{"bad":{"kind":"not-a-kind"}}}"#.utf8))
    case "theme": ("assets/theme.json", Data(#"{"accent":"red;"}"#.utf8))
    // A theme is a palette: fonts belong in the slop's CSS.
    case "font": ("assets/theme.json", Data(#"{"font":"\"Avenir Next\", sans-serif"}"#.utf8))
    default: ("assets/theme.json", Data([0xff]))
    }
    try bytes.write(to: root.appendingPathComponent(file))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
  }

  // Failure: opening validated `initial.json`, which only creation reads, so a later,
  // stricter rule for it would refuse every saved document of that package. Oracle: the
  // package opens; it is still refused as a template, the source of new documents.
  @Test func initialDataIsCheckedOnlyWhereDocumentsAreCreated() throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    try SlopPackage(rootURL: root).validateAsTemplate()
    try Data(#"{"title":false,"rows":[],"hits":0}"#.utf8).write(to: root.appendingPathComponent("initial.json"))
    let package = try SlopPackage(rootURL: root)
    #expect(throws: SlopPackageError.self) { try package.validateAsTemplate() }
  }
}
