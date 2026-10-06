import Foundation
@testable import HitSlopCore
import HitSlopTestSupport
import Testing

@Test func templateLocationRecognizesOnlyManagedTemplateFiles() throws {
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    let templates = root.appendingPathComponent("templates", isDirectory: true)
    let managed = { SlopTemplateLocation.isInTemplates($0, templatesRoot: templates) }

    #expect(managed(templates.appendingPathComponent("soma-amp.slop")))
    #expect(managed(templates.appendingPathComponent("cache/publisher/soma-amp/1.slop")))
    #expect(managed(templates.deletingLastPathComponent().appendingPathComponent("TEMPLATES/soma-amp.slop")))
    #expect(!managed(templates))
    #expect(!managed(root.appendingPathComponent("templates-backup/soma-amp.slop")))
    #expect(!managed(root.appendingPathComponent("documents/soma-amp.slop")))
}
