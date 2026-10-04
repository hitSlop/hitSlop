import AppKit
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import HitSlopDocument

public extension Notification.Name {
    static let hitSlopPreviewDidChange = Notification.Name("com.hitslop.preview-did-change")
}

/// A document's artwork lives in its file (`artwork`): the build's preview and icon, and
/// what a closed document's last render refreshed. Finder and Quick Look show it through
/// the app's Quick Look extensions, and the catalog reads the rows.
@MainActor public enum SlopPreviewWriter {
    /// Writes artwork rendered from a closed document's saved state, while the document still
    /// holds that state (`marker`), then tells Finder and the catalog. Skips, writing
    /// nothing, when the document is open again or has changed. Never fails a save.
    public static func writeRendered(
        preview: Data?, icon: Data?, marker: String, to url: URL, telemetry: SlopTelemetry = .disabled
    ) {
        guard preview != nil || icon != nil else { return }
        do {
            _ = SlopRegistry.prepared
            guard try writeArtwork(path: url.path, marker: marker, preview: preview, icon: icon) else { return }
        } catch {
            telemetry.failure(.artwork, error: error)
            return
        }
        announce(url)
    }

    private static func announce(_ url: URL) {
        NSWorkspace.shared.noteFileSystemChanged(url.path)
        NotificationCenter.default.post(name: .hitSlopPreviewDidChange, object: url.standardizedFileURL)
    }
}
