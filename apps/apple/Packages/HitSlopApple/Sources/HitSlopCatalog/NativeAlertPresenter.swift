import AppKit
import HitSlopFeatures

/// Shows the app's alerts: as a sheet on a window, which AppKit queues behind any sheet the
/// window already shows, or as a modal alert when there is no window.
@MainActor enum NativeAlertPresenter {
  static func present(_ alert: AppAlert, on window: NSWindow?, checkForUpdates: @escaping @MainActor () -> Void) {
    let panel = NSAlert()
    switch alert {
    case .failure(let message, let title):
      panel.messageText = title
      panel.informativeText = message
      panel.addButton(withTitle: "OK")
    case .requiresUpdate(let message):
      panel.messageText = "Could not open document"
      panel.informativeText = message
      panel.addButton(withTitle: "Update hitSlop…")
      panel.addButton(withTitle: "Cancel")
    }
    let finish: @MainActor (NSApplication.ModalResponse) -> Void = { response in
      if case .requiresUpdate = alert, response == .alertFirstButtonReturn { checkForUpdates() }
    }
    if let window {
      panel.beginSheetModal(for: window, completionHandler: finish)
    } else {
      finish(panel.runModal())
    }
  }
}
