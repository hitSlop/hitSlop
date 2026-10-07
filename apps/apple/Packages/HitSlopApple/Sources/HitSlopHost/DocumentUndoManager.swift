import AppKit
import HitSlopDocument

/// A document window's undo manager. Edit ▸ Undo and Redo, and ⌘Z in a text field, undo
/// the person's changes to the document. WebKit's own text-editing undo knows nothing of
/// the document: it still registers here, as AppKit's grouping requires, but its actions
/// are never performed, and only the latest is kept.
@MainActor
final class DocumentUndoManager: UndoManager {
  private weak var session: DocumentSession?

  init(session: DocumentSession) {
    self.session = session
    super.init()
    levelsOfUndo = 1
  }

  // AppKit calls this window-owned manager on the main actor. Older SDKs expose
  // UndoManager's overrides as nonisolated, so make that synchronous boundary explicit.
  override var canUndo: Bool {
    MainActor.assumeIsolated { session?.undoAvailability.canUndo ?? false }
  }
  override var canRedo: Bool {
    MainActor.assumeIsolated { session?.undoAvailability.canRedo ?? false }
  }
  override var undoMenuItemTitle: String { NSLocalizedString("Undo", comment: "Edit menu") }
  override var redoMenuItemTitle: String { NSLocalizedString("Redo", comment: "Edit menu") }
  override func undo() { MainActor.assumeIsolated { perform(redo: false) } }
  override func redo() { MainActor.assumeIsolated { perform(redo: true) } }

  private func perform(redo: Bool) {
    guard let session else { return }
    Task { @MainActor in
      do { try await session.undo(redo: redo) } catch { NSSound.beep() }
    }
  }
}
