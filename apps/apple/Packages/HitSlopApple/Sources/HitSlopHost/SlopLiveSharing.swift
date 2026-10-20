import AppKit
import HitSlopDocument
import SwiftUI

extension SlopDocumentWindowController {
  public var canShowLiveSharing: Bool { session.sharing != nil && isAvailable(.share) }
  /// Attach to the document window, so the transient hover panel can hide normally.
  public func showLiveSharing() {
    guard canShowLiveSharing, let model = session.sharing, let window, window.attachedSheet == nil else { return }
    let panel = NSPanel(contentRect: .zero, styleMask: [.titled], backing: .buffered, defer: false)
    panel.title = "Live Sharing Preview"
    panel.isReleasedWhenClosed = false
    let content = NSHostingView(
      rootView: DocumentSharingView(model: model) { [weak self] in
        guard let self, let panel = self.liveSharingPanel else { return }
        self.window?.endSheet(panel)
        panel.close()
        self.liveSharingPanel = nil
      }.frame(width: 420, height: 420, alignment: .topLeading))
    content.sizingOptions = []
    panel.contentView = content
    panel.setContentSize(NSSize(width: 420, height: 420))
    liveSharingPanel = panel
    window.beginSheet(panel)
  }
}
