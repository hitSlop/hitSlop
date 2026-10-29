import AppKit
import SwiftUI
import TinysDocument

/// The badge's frame: just past the hover toolbar's trailing end, so it sits outside the
/// tiny's own content and shape, kept on screen.
func tinyIssueBadgeFrame(toolbar: NSRect, visible: NSRect?) -> NSRect {
  let size: CGFloat = 22
  var frame = NSRect(x: toolbar.maxX + 4, y: toolbar.midY - size / 2, width: size, height: size)
  if let visible { frame.origin.x = min(frame.minX, visible.maxX - size - 4) }
  return frame
}

/// A page issue that leaves the tiny running (a refused edit, an authored error) shows as
/// a red dot beside the toolbar, without blocking the window. A shared document that is
/// nearly or completely full shows the same way as an orange dot, while no page issue is
/// shown. The dot exists exactly while either is shown; it is a child of the document
/// window, so it moves, hides and minimizes with it. Opening failures keep the overlay
/// and save failures the sheet.
extension TinyDocumentWindowController {
  func refreshIssueBadge() {
    guard let window, let message = guestIssue?.message ?? session.storageNotice else {
      if let badge = issueBadge {
        badge.parent?.removeChildWindow(badge)
        badge.close()
        issueBadge = nil
      }
      return
    }
    let badge = issueBadge ?? makeIssueBadge()
    issueBadge = badge
    badge.contentView?.toolTip = message
    (badge.contentView as? TinyIssueDotButton)?.storage = guestIssue == nil
    badge.setFrame(
      tinyIssueBadgeFrame(
        toolbar: tinyToolbarFrame(document: window.frame, visible: window.screen?.visibleFrame),
        visible: window.screen?.visibleFrame), display: true)
    if window.isVisible, badge.parent == nil { window.addChildWindow(badge, ordered: .above) }
  }

  private func makeIssueBadge() -> TinyIssueBadgePanel {
    let panel = TinyIssueBadgePanel(
      contentRect: NSRect(x: 0, y: 0, width: 22, height: 22),
      styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = false
    panel.hidesOnDeactivate = false
    panel.isReleasedWhenClosed = false
    panel.isExcludedFromWindowsMenu = true
    let button = TinyIssueDotButton()
    button.target = self
    button.action = #selector(showIssueDetails(_:))
    panel.contentView = button
    return panel
  }

  @objc func showIssueDetails(_ sender: NSView) {
    let popover = NSPopover()
    popover.behavior = .transient
    if let issue = guestIssue {
      popover.contentViewController = NSHostingController(
        rootView: TinyIssueDetails(
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
    } else if let message = session.storageNotice {
      popover.contentViewController = NSHostingController(
        rootView: TinyStorageNotice(
          message: message,
          saveCopy: { [weak self, weak popover] in
            popover?.close()
            self?.request(.saveUnsharedCopy)
          },
          dismiss: { [weak popover] in popover?.close() }))
    } else {
      return
    }
    popover.show(relativeTo: sender.bounds, of: sender, preferredEdge: .maxY)
  }

  func dismissIssue() {
    guestIssue = nil
    refreshIssueBadge()
  }

  /// Reloading is a command: the app runs it after any command in progress.
  private func reloadForIssue() { request(.retry) }
}

final class TinyIssueBadgePanel: NSPanel {
  override var canBecomeKey: Bool { false }
  override var canBecomeMain: Bool { false }
}

@MainActor final class TinyIssueDotButton: NSButton {
  /// The dot reports the document's storage, not a page issue.
  var storage = false {
    didSet {
      needsDisplay = true
      setAccessibilityLabel(storage ? "Show the document's storage notice" : "Show the tiny's problem")
    }
  }
  init() {
    super.init(frame: NSRect(x: 0, y: 0, width: 22, height: 22))
    isBordered = false
    title = ""
    setAccessibilityLabel("Show the tiny's problem")
  }
  @available(*, unavailable) required init?(coder: NSCoder) { fatalError() }
  override func draw(_ dirtyRect: NSRect) {
    let dot = NSBezierPath(ovalIn: bounds.insetBy(dx: 5, dy: 5))
    (storage ? NSColor.systemOrange : NSColor.systemRed).setFill()
    dot.fill()
    NSColor.white.withAlphaComponent(0.9).setStroke()
    dot.lineWidth = 1.5
    dot.stroke()
  }
}

struct TinyIssueDetails: View {
  let issue: TinyPageIssue
  let reload: () -> Void, copy: () -> Void, dismiss: () -> Void
  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Text(issue.isOperation ? "An edit wasn't applied" : "This tiny hit a problem").font(.headline)
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

/// A shared document that is nearly or completely full: what it means and the way out.
struct TinyStorageNotice: View {
  let message: String
  let saveCopy: () -> Void, dismiss: () -> Void
  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Text("Shared document is filling up").font(.headline)
      Text(message).font(.caption).fixedSize(horizontal: false, vertical: true)
      HStack {
        Button("Save Unshared Copy…", action: saveCopy)
        Spacer()
        Button("Dismiss", action: dismiss).keyboardShortcut(.defaultAction)
      }
    }.padding(14).frame(width: 340)
  }
}
