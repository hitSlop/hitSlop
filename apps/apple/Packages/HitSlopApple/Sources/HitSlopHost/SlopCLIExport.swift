import Darwin
import Foundation
import HitSlopCore
import HitSlopDocument

extension SlopRenderer {
    public static func exportDocument(session: DocumentSession, format: ExportFormat, output: URL,
                                      deadline: NativeCommandDeadline = NativeCommandDeadline()) async throws {
        try validateExportOutput(output, source: session.file.url)
        try deadline.check()
        let data = try await exportData(session: session, format: format)
        try publishExport(data, to: output, source: session.file.url, deadline: deadline)
    }

    /// `slop export` of a closed document (`DocumentCommand.run`'s `closed`): its saved
    /// state, rendered from a snapshot that takes no lock. An open document exports its
    /// live view through its owner instead.
    public static func exportClosed(_ root: URL, format: ExportFormat, output: URL) async throws -> SocketReply {
        let output = output.standardizedFileURL
        try validateExportOutput(output, source: root)
        let deadline = NativeCommandDeadline()
        let data = try await withRenderSession(url: root) { try await exportData(session: $0, format: format) }
        try publishExport(data, to: output, source: root, deadline: deadline)
        return SocketReply(ok: true, output: output.path)
    }

    private static func exportData(session: DocumentSession, format: ExportFormat) async throws -> Data {
        switch format {
        case .png: try await exportPNGData(session: session)
        case .pdf: try await exportPDFData(session: session)
        }
    }

    private static func validateExportOutput(_ output: URL, source: URL) throws {
        guard !SlopPath.same(source, output) else {
            throw SlopDiagnosticError(SlopError.invalid("Export destination must not be the document"), diagnostic: .init(.rejection, reason: .operationRejected))
        }
        if FileManager.default.fileExists(atPath: output.path) {
            let values = try output.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
            guard values.isRegularFile == true, values.isSymbolicLink != true else {
                throw SlopDiagnosticError(SlopError.invalid("Export destination must be a regular file"), diagnostic: .init(.rejection, reason: .operationRejected))
            }
        }
    }

    /// Callers check the destination before rendering; it is checked again just before the
    /// rename that publishes it.
    static func publishExport(_ data: Data, to output: URL, source: URL, deadline: NativeCommandDeadline) throws {
        try deadline.check()
        let staged = output.deletingLastPathComponent().appendingPathComponent(".hitslop-export-" + UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: staged) }
        try data.write(to: staged, options: .withoutOverwriting)
        // Only the final rename publishes output. A late renderer result is discarded.
        try deadline.check()
        try validateExportOutput(output, source: source)
        guard Darwin.rename(staged.path, output.path) == 0 else {
            throw CocoaError(.fileWriteUnknown, userInfo: [NSFilePathErrorKey: output.path])
        }
    }
}
