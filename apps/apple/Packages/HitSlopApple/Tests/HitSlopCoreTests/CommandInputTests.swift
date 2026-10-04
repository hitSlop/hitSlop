import Foundation
import Darwin
import Testing
import HitSlopTestSupport
@testable import HitSlopCore

@Test func commandFileReadsRejectLinksSpecialFilesAndOversize() throws {
    let root = try Fixtures.folder()
    defer { try? FileManager.default.removeItem(at: root) }
    let target = root.appendingPathComponent("test.txt")
    try Data("safe".utf8).write(to: target)
    #expect(try CommandInput.read(target, maximumBytes: 4) == Data("safe".utf8))
    #expect(throws: SlopError.self) { _ = try CommandInput.read(target, maximumBytes: 3) }
    let link = root.appendingPathComponent("link")
    try FileManager.default.createSymbolicLink(at: link, withDestinationURL: target)
    #expect(throws: SlopError.self) { _ = try CommandInput.read(link, maximumBytes: 4) }
    let fifo = root.appendingPathComponent("pipe")
    #expect(mkfifo(fifo.path, 0o600) == 0)
    #expect(throws: SlopError.self) { _ = try CommandInput.read(fifo, maximumBytes: 4) }
    #expect(throws: SlopError.self) { _ = try CommandInput.read(root, maximumBytes: 4) }
}
