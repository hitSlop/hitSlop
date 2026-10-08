import AppKit
import SwiftUI

/// Marks real controls without depending on SwiftUI's private view hierarchy.
struct SlopToolbarControlRegion: NSViewRepresentable {
  final class Marker: NSView {
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
  }
  func makeNSView(context: Context) -> Marker { Marker() }
  func updateNSView(_ view: Marker, context: Context) {}
}

func slopToolbarDragStarted(from start: NSPoint, to point: NSPoint) -> Bool {
  hypot(point.x - start.x, point.y - start.y) >= 4
}

struct SlopToolbarVisibility {
  private(set) var outsideSince: TimeInterval?
  mutating func shouldShow(inside: Bool, interacting: Bool, visible: Bool, now: TimeInterval) -> Bool {
    if inside || interacting {
      outsideSince = nil
      return true
    }
    guard visible else {
      outsideSince = nil
      return false
    }
    if outsideSince == nil { outsideSince = now }
    return now - (outsideSince ?? now) < 0.8
  }
}

@MainActor final class SlopToolbarPanel: NSPanel {
  var drag: (NSEvent) -> Void = { _ in }
  var interactionChanged: (Bool) -> Void = { _ in }

  private func descendants(of view: NSView) -> [NSView] {
    [view] + view.subviews.flatMap { descendants(of: $0) }
  }

  override func sendEvent(_ event: NSEvent) {
    guard event.type == .leftMouseDown, let contentView else {
      super.sendEvent(event)
      return
    }
    interactionChanged(true)
    defer { interactionChanged(false) }
    let views = descendants(of: contentView)
    let contains: (NSView, NSPoint) -> Bool = { view, point in
      !view.isHiddenOrHasHiddenAncestor && view.bounds.contains(view.convert(point, from: nil))
    }
    if views.contains(where: { $0 is SlopToolbarControlRegion.Marker && contains($0, event.locationInWindow) }) {
      super.sendEvent(event)
      return
    }
    let file = views.compactMap { $0 as? SlopToolbarFileButton }
      .first { contains($0, event.locationInWindow) }
    while let next = nextEvent(
      matching: [.leftMouseDragged, .leftMouseUp], until: .distantFuture,
      inMode: .eventTracking, dequeue: true)
    {
      if next.type == .leftMouseUp {
        if let file, file.isEnabled, contains(file, next.locationInWindow) { file.performClick(nil) }
        return
      }
      if slopToolbarDragStarted(from: event.locationInWindow, to: next.locationInWindow) {
        drag(event)
        return
      }
    }
  }
}

/// A single sampler recovers missed tracking events, including in inactive pinned windows.
/// Pointer movement in any app, a window that may have appeared under the pointer, or
/// entering a document starts it; it stops once no window shows its toolbar (including
/// the grace period before it hides), so a still pointer costs nothing.
@MainActor final class SlopToolbarPointerSampler {
  static let shared = SlopToolbarPointerSampler()
  private struct Entry {
    weak var owner: AnyObject?
    /// Refreshes one window's hover state; true while its toolbar shows.
    let update: (NSPoint, Int) -> Bool
  }
  private var entries: [ObjectIdentifier: Entry] = [:]
  private var timer: Timer?
  /// Sees pointer events delivered to other apps, where an inactive app's tracking areas
  /// miss entries. Mouse events need no Accessibility permission.
  private var pointerMonitor: Any?

  private var observers: [NSObjectProtocol] = []

  private init() {
    for name in [
      NSWindow.didChangeOcclusionStateNotification, NSWindow.didDeminiaturizeNotification,
      NSApplication.didBecomeActiveNotification, NSApplication.didUnhideNotification,
    ] {
      observers.append(
        NotificationCenter.default.addObserver(forName: name, object: nil, queue: .main) { [weak self] _ in
          MainActor.assumeIsolated { self?.start() }
        })
    }
    observers.append(
      NSWorkspace.shared.notificationCenter.addObserver(
        forName: NSWorkspace.activeSpaceDidChangeNotification, object: nil, queue: .main
      ) { [weak self] _ in
        MainActor.assumeIsolated { self?.start() }
      })
  }

  func add(_ owner: AnyObject, update: @escaping (NSPoint, Int) -> Bool) {
    entries[ObjectIdentifier(owner)] = Entry(owner: owner, update: update)
    if pointerMonitor == nil {
      pointerMonitor = NSEvent.addGlobalMonitorForEvents(
        matching: [.mouseMoved, .leftMouseDragged, .rightMouseDragged, .otherMouseDragged]
      ) { [weak self] _ in
        MainActor.assumeIsolated { self?.start() }
      }
    }
    start()
  }

  func start() {
    guard timer == nil, !entries.isEmpty else { return }
    let timer = Timer(timeInterval: 0.1, repeats: true) { [weak self] _ in
      MainActor.assumeIsolated { self?.sample() }
    }
    self.timer = timer
    RunLoop.main.add(timer, forMode: .common)
  }

  func remove(_ owner: AnyObject) {
    entries.removeValue(forKey: ObjectIdentifier(owner))
    guard entries.isEmpty else { return }
    stop()
    if let pointerMonitor { NSEvent.removeMonitor(pointerMonitor) }
    pointerMonitor = nil
  }

  private func stop() {
    timer?.invalidate()
    timer = nil
  }

  private func sample() {
    entries = entries.filter { $0.value.owner != nil }
    let point = NSEvent.mouseLocation
    let front = NSWindow.windowNumber(at: point, belowWindowWithWindowNumber: 0)
    // Every window refreshes, so one leaving hides while another shows.
    let showing = entries.values.reduce(false) { showing, entry in entry.update(point, front) || showing }
    if !showing { stop() }
  }
}
