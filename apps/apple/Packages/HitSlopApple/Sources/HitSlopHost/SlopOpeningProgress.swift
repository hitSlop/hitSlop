import AppKit
import HitSlopDocument

/// Native feedback for a slow open. Never renders document content.
@MainActor public final class SlopOpeningProgress: NSObject {
  private(set) var panel: NSPanel?
  private(set) var wasShown = false
  private var timer: Task<Void, Never>?
  private var finished = false
  var onCancel: (() -> Void)?

  public init(started: ContinuousClock.Instant = .now, wait: @escaping @Sendable (ContinuousClock.Instant) async throws -> Void = { deadline in
    try await Task.sleep(until: deadline, clock: .continuous)
  }) {
    super.init()
    let deadline = started.advanced(by: Timeouts.progressDelay)
    timer = Task { @MainActor [weak self] in
      do { try await wait(deadline) } catch { return }
      self?.show()
    }
  }

  deinit { timer?.cancel() }

  private func show() {
    guard !finished else { return }
    let panel = NSPanel(contentRect: NSRect(x: 0, y: 0, width: 280, height: 100),
      styleMask: [.titled], backing: .buffered, defer: false)
    panel.title = "Opening document"
    panel.isReleasedWhenClosed = false
    panel.animationBehavior = .none
    let spinner = NSProgressIndicator(frame: NSRect(x: 24, y: 48, width: 20, height: 20))
    spinner.style = .spinning
    spinner.startAnimation(nil)
    let label = NSTextField(labelWithString: "Opening…")
    label.frame = NSRect(x: 56, y: 48, width: 200, height: 20)
    let cancel = NSButton(title: "Cancel", target: self, action: #selector(cancelOpening))
    cancel.frame = NSRect(x: 174, y: 10, width: 86, height: 28)
    cancel.keyEquivalent = "\u{1b}"
    for view in [spinner, label, cancel] { panel.contentView?.addSubview(view) }
    panel.center()
    wasShown = true
    self.panel = panel
    panel.makeKeyAndOrderFront(nil)
  }

  @objc func cancelOpening() {
    let action = onCancel
    finish()
    action?()
  }

  func waitForFeedback() async { await timer?.value }

  public func focus() { panel?.makeKeyAndOrderFront(nil) }

  func finish() {
    finished = true
    timer?.cancel()
    timer = nil
    panel?.close()
    panel = nil
    onCancel = nil
  }
}
