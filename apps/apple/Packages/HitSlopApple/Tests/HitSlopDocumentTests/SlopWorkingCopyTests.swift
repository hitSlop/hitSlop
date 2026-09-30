import Foundation
import HitSlopCore
import Testing

@Test func templateLocationRecognizesOnlyManagedTemplatePackages() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent("hitslop-template-boundary-\(UUID().uuidString)", isDirectory: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let templates = root.appendingPathComponent("templates", isDirectory: true)
    let managed = { SlopTemplateLocation.isManagedTemplatePackage($0, templatesRoot: templates) }

    #expect(managed(templates.appendingPathComponent("soma-amp.slop")))
    #expect(managed(templates.appendingPathComponent("cache/publisher/soma-amp/1.slop")))
    #expect(managed(templates.deletingLastPathComponent().appendingPathComponent("TEMPLATES/soma-amp.slop")))
    #expect(!managed(templates))
    #expect(!managed(root.appendingPathComponent("templates-backup/soma-amp.slop")))
    #expect(!managed(root.appendingPathComponent("documents/soma-amp.slop")))
}
