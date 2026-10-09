import AppKit

extension NSWindow {
  /// Places a test window beyond every screen before it is shown. AppKit and WebKit treat it
  /// as an ordinary visible window, and its sheets and child panels follow it, but nothing
  /// appears on the desktop of whoever runs the tests.
  @MainActor public func moveOffScreen() {
    setFrameOrigin(NSPoint(x: -30_000, y: -30_000))
  }
}
