import Darwin
import Foundation
import HitSlopCore
import HitSlopRuntime
import HitSlopDocument

extension SlopRenderer {
    /// Attach at the host boundary; the engine never imports the renderer.
    public static func installCLIExport(
        on session: SlopRuntimeSession, telemetry: SlopTelemetry = .disabled,
        onFailure: ((Error, SlopTelemetryEvent.ExportFormat?) -> Void)? = nil
    ) {
        session.engine.onExport = { [weak session] format, output, deadline in
            guard let session else { throw SlopPackageError.invalid("Document closed") }
            telemetry.send(.breadcrumb(.export, .started))
            do {
                try await exportDocument(session: session, format: format, output: output, deadline: deadline)
                telemetry.send(.breadcrumb(.export, .completed))
                if let format = SlopTelemetryEvent.ExportFormat(rawValue: format) { telemetry.send(.exported(format)) }
            } catch {
                if let onFailure { onFailure(error, SlopTelemetryEvent.ExportFormat(rawValue: format)) }
                else { telemetry.failure(.export, error: error,
                                         format: SlopTelemetryEvent.ExportFormat(rawValue: format)) }
                throw error
            }
        }
    }

    public static func exportDocument(session: SlopRuntimeSession, format: String, output: URL,
                                      deadline: NativeCommandDeadline = NativeCommandDeadline()) async throws {
        try validateExportOutput(output, source: session.package.rootURL)
        try deadline.check()
        let data = try await exportData(session: session, format: format)
        try publishExport(data, to: output, source: session.package.rootURL, deadline: deadline)
    }

    public static func exportDocument(packageURL: URL, format: String, output: URL) async throws {
        guard ["png", "pdf"].contains(format) else { throw SlopPackageError.invalid("Expected png or pdf") }
        try SlopLocalDocument.requireLocal(packageURL)
        let package = try SlopPackage(rootURL: packageURL)
        let output = output.standardizedFileURL
        try validateExportOutput(output, source: package.rootURL)
        var ownership: DocumentWriterLock?
        // Managed/read-only masters cannot have a live writable session.
        if DocumentFactory.isManagedTemplatePackage(package.rootURL) ||
           !FileManager.default.isWritableFile(atPath: package.rootURL.path) {
            try package.validateAsTemplate()
        } else {
            do { ownership = try DocumentWriterLock(root: package.rootURL) }
            catch {
                guard DocumentWriterLock.isBusy(error) else { throw error }
                let socket = try DocumentCommand.liveSocket(for: package.rootURL)
                try await DocumentCommand.exportLive(root: package.rootURL, socket: socket, format: format, output: output)
                return
            }
        }
        // Ownership covers taking the in-memory snapshot, so no writer can intervene;
        // rendering from that snapshot needs none.
        let deadline = NativeCommandDeadline()
        let data = try await withRenderSession(packageURL: package.rootURL, inputReady: { ownership?.close() }) { session in
            try await exportData(session: session, format: format)
        }
        try publishExport(data, to: output, source: package.rootURL, deadline: deadline)
    }

    private static func exportData(session: SlopRuntimeSession, format: String) async throws -> Data {
        switch format {
        case "png": return try await exportPNGData(session: session)
        case "pdf": return try await exportPDFData(session: session)
        default: throw SlopPackageError.invalid("Expected png or pdf")
        }
    }

    private static func validateExportOutput(_ output: URL, source: URL) throws {
        let destination = output.standardizedFileURL.resolvingSymlinksInPath().path
        let root = source.standardizedFileURL.resolvingSymlinksInPath().path
        guard destination != root, !destination.hasPrefix(root + "/") else {
            throw SlopDiagnosticError(SlopPackageError.invalid("Export destination must be outside the source package"), diagnostic: .init(.rejection, reason: .operationRejected))
        }
        if FileManager.default.fileExists(atPath: output.path) {
            let values = try output.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey])
            guard values.isRegularFile == true, values.isSymbolicLink != true else {
                throw SlopDiagnosticError(SlopPackageError.invalid("Export destination must be a regular file"), diagnostic: .init(.rejection, reason: .operationRejected))
            }
        }
    }

    static func publishExport(_ data: Data, to output: URL, source: URL, deadline: NativeCommandDeadline) throws {
        try validateExportOutput(output, source: source)
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
