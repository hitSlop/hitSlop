import Darwin
import Foundation
import HitSlopCore
import HitSlopDocument

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
      CocoaError(.fileWriteFileExists, userInfo: [NSFilePathErrorKey: output.path]),
      diagnostic: .init(.rejection, reason: .operationRejected))
  }

  /// Writes the export after the last deadline check, so a late renderer result is
  /// discarded, creating the file exclusively.
  static func publishExport(_ data: Data, to output: URL, deadline: NativeCommandDeadline) throws {
    try deadline.check()
    do {
      try data.write(to: output, options: .withoutOverwriting)
    } catch CocoaError.fileWriteFileExists {
      throw exists(output)
    }
  }
}
