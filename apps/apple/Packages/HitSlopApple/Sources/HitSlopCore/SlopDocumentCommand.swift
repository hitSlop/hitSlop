import Foundation

/// Commands shared by native document windows and the application's reducers.
public enum SlopDocumentCommand: Equatable, Sendable {
    case pin(Bool), exportPNG, exportPDF, duplicate, reveal, copyPath, openEditor(URL), retry, close
    /// Shows or hides the theme panel; imports or exports a theme file.
    case theme(Bool), importTheme, exportTheme
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
