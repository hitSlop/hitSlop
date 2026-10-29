import Foundation
import TinysCoreBinding

/// A document operation: the app runs one at a time per document, so none runs beside a
/// close, an export or a save recovery. What only changes the window (pin, the theme panel,
/// Reveal) is the window's own.
public enum TinyDocumentCommand: Equatable, Sendable {
  case exportPNG, exportPDF, duplicate, retry, close, moveToCloud
  /// A shared document's way to keep going without its shared history: a copy that opens in
  /// its own window, or, when the shared document is full and cannot save, the owner's
  /// hand-off to the copy followed by closing the original.
  case saveUnsharedCopy
  /// Opens the share chooser. Preparing a selected format is a separate operation.
  case share
  case sharePDF, sharePNG, shareTiny
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
public enum TinyDocumentFailure: Error, Equatable, Sendable {
  case save
  case cancelled
  case other(String)

  /// Keeps a classified failure; any other error is a plain failure with its message.
  public init(_ error: Error) {
    if let failure = error as? TinyDocumentFailure {
      self = failure
    } else if error is SaveFailure {
      self = .save
    } else if let failure = error as? OwnerFailure,
      [.busy, .full, .moved, .invalidated, .saveFailed].contains(failure.kind)
    {
      self = .save
    } else if TinyFailureContext.isCancellation(error) {
      self = .cancelled
    } else {
      self = .other(error.localizedDescription)
    }
  }
}
