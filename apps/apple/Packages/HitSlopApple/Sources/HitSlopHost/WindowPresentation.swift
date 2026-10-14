import AppKit

/// The window's loading/reveal state, independent of the document's close/save state.
struct WindowPresentation {
  let started: ContinuousClock.Instant
  var progress: SlopOpeningProgress?
  var loading = false
  var requested = false
  var ready = false
  var task: Task<Void, Never>?
  weak var webView: NSView?
}
