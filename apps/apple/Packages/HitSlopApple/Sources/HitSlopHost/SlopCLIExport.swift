import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument

extension SlopRenderer {
    /// Attach at the host boundary; the engine never imports the renderer.
    public static func installCLIExport(
        on session: DocumentSession, telemetry: SlopTelemetry,
        onFailure: @escaping (Error, ExportFormat?) -> Void
    ) {
        session.onExport = { [weak session] format, output, deadline in
            guard let session else { throw SlopPackageError.invalid("Document closed") }
            telemetry.send(.breadcrumb(.export, .started))
            do {
                try await exportDocument(session: session, format: format, output: output, deadline: deadline)
                telemetry.send(.breadcrumb(.export, .completed))
                telemetry.send(.exported(format))
            } catch {
                onFailure(error, format)
                throw error
            }
        }
    }

    public static func exportDocument(session: DocumentSession, format: ExportFormat, output: URL,
                                      deadline: NativeCommandDeadline = NativeCommandDeadline()) async throws {
        try validateExportOutput(output, source: session.package.rootURL)
        try deadline.check()
        let data = try await exportData(session: session, format: format)
        try publishExport(data, to: output, source: session.package.rootURL, deadline: deadline)
    }

    public static func exportDocument(packageURL: URL, format: ExportFormat, output: URL) async throws {
        try SlopLocalDocument.requireLocal(packageURL)
        let package = try SlopPackage(rootURL: packageURL)
        let output = output.standardizedFileURL
        try validateExportOutput(output, source: package.rootURL)
        var ownership: WriterLock?
        // Managed/read-only masters cannot have a live writable session.
        if SlopTemplateLocation.isMaster(package.rootURL) ||
           !FileManager.default.isWritableFile(atPath: package.rootURL.path) {
            try package.validateAsTemplate()
        } else {
            let root = package.rootURL
            switch try await DocumentCommand.connect(root: root, until: .now + .seconds(2), own: { try WriterLock.acquire(root) }) {
            case .owned(let lock): ownership = lock
            case .live(let socket):
                try await DocumentCommand.exportLive(root: root, socket: socket, format: format, output: output)
                return
            }
        }
        // Ownership covers taking the in-memory snapshot, so no writer can intervene;
        // rendering from that snapshot needs none.
        let deadline = NativeCommandDeadline()
        let data = try await withRenderSession(packageURL: package.rootURL, inputReady: { ownership?.release() }) { session in
            try await exportData(session: session, format: format)
        }
        try publishExport(data, to: output, source: package.rootURL, deadline: deadline)
    }

    private static func exportData(session: DocumentSession, format: ExportFormat) async throws -> Data {
        switch format {
        case .png: try await exportPNGData(session: session)
        case .pdf: try await exportPDFData(session: session)
        }
    }

    private static func validateExportOutput(_ output: URL, source: URL) throws {
        guard !SlopPath.contains(source, output) else {
            throw SlopDiagnosticError(SlopPackageError.invalid("Export destination must be outside the source package"), diagnostic: .init(.rejection, reason: .operationRejected))
        }
        if FileManager.default.fileExists(atPath: output.path) {
            let values = try output.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
            guard values.isRegularFile == true, values.isSymbolicLink != true else {
                throw SlopDiagnosticError(SlopPackageError.invalid("Export destination must be a regular file"), diagnostic: .init(.rejection, reason: .operationRejected))
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
