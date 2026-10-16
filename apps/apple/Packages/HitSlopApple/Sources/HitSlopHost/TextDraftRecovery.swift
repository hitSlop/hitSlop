import AppKit
import HitSlopCoreBinding

/// A decision about page-local text, separate from saving accepted document edits.
@MainActor struct TextDraftRecovery {
  enum Choice { case copyAndClose, discardAndClose, cancel }

  /// Like the file panels, integration tests can script the native presenter.
  var choose: (NSWindow, Int, Bool) async -> Choice = present

  func resolve(
    _ drafts: [UnresolvedTextDraft], document: String, in window: NSWindow,
    copy: (String) -> Bool
  ) async throws {
    guard !drafts.isEmpty else { return }
    let text = document + "\n\n" + drafts.map { "Field \($0.field)\n\($0.text)" }.joined(separator: "\n\n")
    var copyFailed = false
    while true {
      switch await choose(window, drafts.count, copyFailed) {
      case .copyAndClose:
        if copy(text) { return }
        copyFailed = true
      case .discardAndClose: return
      case .cancel: throw CancellationError()
      }
    }
  }

  static func copy(_ text: String) -> Bool {
    NSPasteboard.general.clearContents()
    return NSPasteboard.general.setString(text, forType: .string)
  }

  private static func present(window: NSWindow, count: Int, copyFailed: Bool) async -> Choice {
    let alert = NSAlert()
    alert.alertStyle = .warning
    alert.messageText = "Keep a copy before closing?"
    let explanation = count == 1 ? "A text edit could not be confirmed." : "\(count) text edits could not be confirmed."
    alert.informativeText =
      copyFailed
      ? "The text could not be copied. Try again, discard it, or cancel to keep this window open."
      : explanation + " Copy the full text before closing, or discard it. Your saved document will be kept."
    alert.addButton(withTitle: "Copy and Close")
    alert.addButton(withTitle: "Discard and Close")
    alert.addButton(withTitle: "Cancel").keyEquivalent = "\u{1b}"
    return await withCheckedContinuation { continuation in
      NSApp.activate()
      if window.isMiniaturized { window.deminiaturize(nil) }
      window.makeKeyAndOrderFront(nil)
      alert.beginSheetModal(for: window) { response in
        switch response {
        case .alertFirstButtonReturn: continuation.resume(returning: .copyAndClose)
        case .alertSecondButtonReturn: continuation.resume(returning: .discardAndClose)
        default: continuation.resume(returning: .cancel)
        }
      }
    }
  }
}
