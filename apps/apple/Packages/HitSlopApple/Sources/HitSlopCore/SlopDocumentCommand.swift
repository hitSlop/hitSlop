import Foundation

/// A document operation: the app runs one at a time per document, so none runs beside a
/// close, an export or a save recovery. What only changes the window (pin, the theme panel,
/// Reveal) is the window's own.
public enum SlopDocumentCommand: Equatable, Sendable {
    case exportPNG, exportPDF, duplicate, retry, close
    /// Shares a consistent copy of the document as a new logical document.
    case share
    /// Imports or exports a theme file.
    case importTheme, exportTheme
    /// The save-failure sheet's choices: save again, or discard unsaved edits and reload
    /// the saved document.
    case retrySave, discardUnsaved

    /// Whether this answers the save-failure sheet. A recovery chosen while another
    /// command runs waits for it instead of being dropped.
    public var isSaveRecovery: Bool { self == .retrySave || self == .discardUnsaved }
}

/// Why a document command failed, as coordination needs it. A save failure is presented
/// by the window's save-failure sheet, never by a second alert.
public enum SlopDocumentFailure: Error, Equatable, Sendable {
    case save
    case cancelled
    case other(String)

    /// Keeps a classified failure; any other error is a plain failure with its message.
    public init(_ error: Error) {
        if let failure = error as? SlopDocumentFailure { self = failure }
        else if error is CancellationError { self = .cancelled }
        else { self = .other(error.localizedDescription) }
    }
}
