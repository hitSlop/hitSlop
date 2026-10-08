import Foundation
import Testing
@testable import HitSlopCore

@Suite struct BoundaryPackageRegression {
  @Test(arguments: ["descriptor", "initial", "theme", "utf8"])
  func refusesInvalidPackageInput(kind: String) throws {
    let repository = #filePath.components(separatedBy: "/apps/apple/")[0]
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString + ".slop")
    try FileManager.default.copyItem(atPath: repository + "/tests/fixtures/checklist/document", toPath: root.path)
    defer { try? FileManager.default.removeItem(at: root) }
    let (file, bytes): (String, Data) = switch kind {
    case "descriptor": ("state.schema.json", Data(#"{"kind":"object","properties":{"bad":{"kind":"not-a-kind"}}}"#.utf8))
    case "initial": ("initial.json", Data(#"{"title":false,"rows":[],"hits":0}"#.utf8))
    case "theme": ("assets/theme.json", Data(#"{"accent":"red;"}"#.utf8))
    default: ("assets/theme.json", Data([0xff]))
    }
    try bytes.write(to: root.appendingPathComponent(file))
    #expect(throws: SlopPackageError.self) { _ = try SlopPackage(rootURL: root) }
  }
}
