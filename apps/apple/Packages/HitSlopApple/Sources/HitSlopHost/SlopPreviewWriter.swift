import AppKit
import Foundation

public extension Notification.Name {
    static let hitSlopPreviewDidChange = Notification.Name("com.hitslop.preview-did-change")
}

/// A document's artwork lives in its file (`artwork`): the build's preview and icon, and
/// what its window rendered as it last closed. Finder and Quick Look show it through the
/// app's Quick Look extensions, and the catalog reads the rows.
@MainActor public enum SlopPreviewWriter {
    /// Tells Finder and the catalog that the artwork of the document at `url` changed.
    public static func announce(_ url: URL) {
        NSWorkspace.shared.noteFileSystemChanged(url.path)
        NotificationCenter.default.post(name: .hitSlopPreviewDidChange, object: url.standardizedFileURL)
    }
}
