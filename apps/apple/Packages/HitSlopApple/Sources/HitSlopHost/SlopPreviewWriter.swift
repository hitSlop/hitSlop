import AppKit
import Foundation
import HitSlopCore
import HitSlopDocument

public extension Notification.Name {
    static let hitSlopPreviewDidChange = Notification.Name("com.hitslop.preview-did-change")
}

@MainActor public enum SlopPreviewWriter {
    public static func write(_ png: Data, to packageURL: URL) throws {
        let directory = packageURL.appendingPathComponent("QuickLook", isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try png.write(to: directory.appendingPathComponent("Preview.png"), options: .atomic)
        announce(packageURL)
    }

    /// Finder list rows use a document custom icon rather than the package's Quick Look
    /// preview. Seed a missing Finder icon from the latest saved icon PNG (initially
    /// the author-supplied artwork); never from the live full-document preview.
    public static func installAuthoredIcon(for package: SlopPackage, telemetry: SlopTelemetry = .disabled) {
        // Announce only an installed icon: every announcement refreshes the catalog entry.
        guard !FileManager.default.fileExists(atPath: package.rootURL.appendingPathComponent("Icon\r").path),
              let image = NSImage(contentsOf: package.iconURL) else { return }
        guard NSWorkspace.shared.setIcon(image, forFile: package.rootURL.path, options: []) else {
            telemetry.send(.failed(.artwork, .init(reason: .icon)))
            return
        }
        announce(package.rootURL)
    }

    /// Saves a rendered icon for the catalog, then installs the same image for Finder.
    public static func installFinderIcon(_ png: Data, for packageURL: URL, telemetry: SlopTelemetry = .disabled) {
        guard FileManager.default.fileExists(atPath: packageURL.appendingPathComponent("manifest.json").path) else { return }
        guard let image = NSImage(data: png) else {
            telemetry.send(.failed(.artwork, .init(reason: .icon)))
            return
        }
        do {
            let directory = packageURL.appendingPathComponent("QuickLook", isDirectory: true)
            try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
            try png.write(to: directory.appendingPathComponent("Icon.png"), options: .atomic)
        } catch {
            telemetry.failure(.artwork, error: error)
            return
        }
        if !NSWorkspace.shared.setIcon(image, forFile: packageURL.path, options: []) {
            telemetry.send(.failed(.artwork, .init(reason: .icon)))
        }
        announce(packageURL)
    }

    private static func announce(_ packageURL: URL) {
        NSWorkspace.shared.noteFileSystemChanged(packageURL.path)
        NotificationCenter.default.post(name: .hitSlopPreviewDidChange, object: packageURL.standardizedFileURL)
    }
}
