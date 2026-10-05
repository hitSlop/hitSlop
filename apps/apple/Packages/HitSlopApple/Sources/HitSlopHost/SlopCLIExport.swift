import Darwin
import Foundation
import HitSlopCore
import HitSlopDocument

extension SlopRenderer {
    public static func exportDocument(session: DocumentSession, format: ExportFormat, output: URL,
                                      deadline: NativeCommandDeadline = NativeCommandDeadline()) async throws {
        try refuseExisting(output)
        try deadline.check()
        let data = try await exportData(session: session, format: format)
        try publishExport(data, to: output, deadline: deadline)
    }

    /// `slop export` of a closed document (`DocumentCommand.run`'s renderer callback): its saved
    /// state, rendered from a snapshot that takes no lock. An open document exports its
    /// saved state after its owner drains pending edits.
    public static func exportClosed(_ root: URL, format: ExportFormat, output: URL,
                                    deadline: NativeCommandDeadline = NativeCommandDeadline()) async throws -> SocketReply {
        let output = output.standardizedFileURL
        try refuseExisting(output)
        let data = try await withRenderSession(url: root) { try await exportData(session: $0, format: format) }
        try publishExport(data, to: output, deadline: deadline)
        return SocketReply.export(output: output.path)
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
        SlopDiagnosticError(CocoaError(.fileWriteFileExists, userInfo: [NSFilePathErrorKey: output.path]),
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
