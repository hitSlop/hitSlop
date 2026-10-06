import AppKit
import HitSlopDocument
import SwiftUI

/// The badge's frame: just past the hover toolbar's trailing end, so it sits outside the
/// slop's own content and shape, kept on screen.
func slopIssueBadgeFrame(toolbar: NSRect, visible: NSRect?) -> NSRect {
  let size: CGFloat = 22
  var frame = NSRect(x: toolbar.maxX + 4, y: toolbar.midY - size / 2, width: size, height: size)
  if let visible { frame.origin.x = min(frame.minX, visible.maxX - size - 4) }
  return frame
}

/// A page issue that leaves the slop running (a refused edit, an authored error) shows as
/// a red dot beside the toolbar, without blocking the window. The dot exists exactly while
/// an issue is shown; it is a child of the document window, so it moves, hides and
/// minimizes with it. Opening failures keep the overlay and save failures the sheet.
extension SlopDocumentWindowController {
  func refreshIssueBadge() {
    guard let window, let issue = guestIssue else {
      if let badge = issueBadge {
        badge.parent?.removeChildWindow(badge)
        badge.close()
        issueBadge = nil
      }
      return
    }
    let badge = issueBadge ?? makeIssueBadge()
    issueBadge = badge
    badge.contentView?.toolTip = issue.message
    badge.setFrame(slopIssueBadgeFrame(
      toolbar: slopToolbarFrame(document: window.frame, visible: window.screen?.visibleFrame),
      visible: window.screen?.visibleFrame), display: true)
    if window.isVisible, badge.parent == nil { window.addChildWindow(badge, ordered: .above) }
  }

  private func makeIssueBadge() -> SlopIssueBadgePanel {
    let panel = SlopIssueBadgePanel(
      contentRect: NSRect(x: 0, y: 0, width: 22, height: 22),
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = false
    panel.hidesOnDeactivate = false
    panel.isReleasedWhenClosed = false
    panel.isExcludedFromWindowsMenu = true
    let button = SlopIssueDotButton()
    button.target = self
    button.action = #selector(showIssueDetails(_:))
    panel.contentView = button
    return panel
  }

  @objc func showIssueDetails(_ sender: NSView) {
    guard let issue = guestIssue else { return }
    let popover = NSPopover()
    popover.behavior = .transient
    popover.contentViewController = NSHostingController(rootView: SlopIssueDetails(
      issue: issue,
      reload: { [weak self, weak popover] in
        popover?.close()
        self?.reloadForIssue()
      },
      copy: { NSPasteboard.general.copy(issue.message) },
      dismiss: { [weak self, weak popover] in
        popover?.close()
        self?.dismissIssue()
      }))
    popover.show(relativeTo: sender.bounds, of: sender, preferredEdge: .maxY)
  }

  func dismissIssue() {
    guestIssue = nil
    refreshIssueBadge()
  }

  /// Reloading is a command: the app runs it after any command in progress.
  private func reloadForIssue() { request(.retry) }
}

final class SlopIssueBadgePanel: NSPanel {
  override var canBecomeKey: Bool { false }
  override var canBecomeMain: Bool { false }
}

@MainActor final class SlopIssueDotButton: NSButton {
  init() {
    super.init(frame: NSRect(x: 0, y: 0, width: 22, height: 22))
    isBordered = false
    title = ""
    setAccessibilityLabel("Show the slop's problem")
  }
  @available(*, unavailable) required init?(coder: NSCoder) { fatalError() }
  override func draw(_ dirtyRect: NSRect) {
    let dot = NSBezierPath(ovalIn: bounds.insetBy(dx: 5, dy: 5))
    NSColor.systemRed.setFill()
    dot.fill()
    NSColor.white.withAlphaComponent(0.9).setStroke()
    dot.lineWidth = 1.5
    dot.stroke()
  }
}

struct SlopIssueDetails: View {
  let issue: SlopPageIssue
  let reload: () -> Void, copy: () -> Void, dismiss: () -> Void
  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Text(issue.isOperation ? "An edit wasn't applied" : "This slop hit a problem").font(.headline)
      ScrollView {
        Text(issue.message).font(.caption).textSelection(.enabled)
          .frame(maxWidth: .infinity, alignment: .leading)
      }.frame(maxHeight: 120)
      HStack {
        Button("Reload Interface", action: reload)
        Button("Copy Details", action: copy)
        Spacer()
        Button("Dismiss", action: dismiss).keyboardShortcut(.defaultAction)
      }
    }.padding(14).frame(width: 340)
  }
}
