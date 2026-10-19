#if os(macOS)
  import AppKit

  /// Save and open pickers used by document operations and catalog creation.
  @MainActor public enum SlopPanels {
    public static func select(_ panel: NSSavePanel, in window: NSWindow? = nil) async -> URL? {
      let response = if let window { await panel.beginSheetModal(for: window) } else { await panel.begin() }
      return response == .OK ? panel.url : nil
    }
  }

#endif
