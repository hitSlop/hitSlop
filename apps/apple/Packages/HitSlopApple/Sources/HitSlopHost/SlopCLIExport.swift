import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument

private final class ExportActivity: NativeExportActivity {
  let deadline: NativeCommandDeadline
  init(_ deadline: NativeCommandDeadline) { self.deadline = deadline }
  func isActive() -> Bool { (try? deadline.check()) != nil }
}

extension SlopRenderer {
  /// `slop export` of an open document: its saved state, rendered after its owner drains
  /// pending edits. A closed document is exported by the engine.
  public static func exportDocument(
    session: DocumentSession, format: ExportFormat, output: URL,
    deadline: NativeCommandDeadline = NativeCommandDeadline()
  ) async throws {
    try refuseExisting(output)
    try deadline.check()
    let data = try await exportData(session: session, format: format)
    try publishExport(data, to: output, deadline: deadline)
  }

  private static func exportData(session: DocumentSession, format: ExportFormat) async throws -> Data {
    switch format {
    case .png: try await exportPNGData(session: session)
    case .pdf: try await exportPDFData(session: session)
    }
  }

  /// Exports never replace a file: not the document, another document or an earlier
  /// export, under any spelling of its path. Checked before rendering, and again by the
  /// write that publishes.
  private static func refuseExisting(_ output: URL) throws {
    var info = stat()
    guard lstat(output.path, &info) != 0 else { throw exists(output) }
  }
  private static func exists(_ output: URL) -> Error {
    SlopDiagnosticError(
      OwnerFailure(
        kind: .rejected,
        message:
          "A file already exists at “\(output.lastPathComponent)”. Export cannot replace existing files. Choose a different name or location.",
        reason: .exists, opIndex: nil),
      diagnostic: .init(.rejection, reason: .operationRejected))
  }

  /// Rust stages and syncs the bytes, checks activity immediately before publishing,
  /// then publishes exclusively and syncs the folder.
  static func publishExport(_ data: Data, to output: URL, deadline: NativeCommandDeadline) throws {
    try deadline.check()
    do {
      try HitSlopCoreBinding.publishExport(bytes: data, output: output.path, activity: ExportActivity(deadline))
    } catch CoreError.Failure(let failure) {
      if failure.reason == .exists { throw exists(output) }
      throw SlopDiagnosticError(failure, diagnostic: .classify(failure))
    }
  }
}
