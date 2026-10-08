import Foundation
import HitSlopCore
import Synchronization

/// Delivery retains its batch until acknowledgement. Overflow and view replacement
/// invalidate old acknowledgements, so they can never consume a new resync marker.
final class PushQueue: Sendable {
  struct Batch {
    let generation: UInt64
    let view: String
    let items: [String]
  }
  private struct State {
    var items: [String] = []
    var bytes = 0
    var generation: UInt64 = 0
    var view = ""
    var draining = false

    mutating func markResync() {
      generation &+= 1
      items = [#"{"type":"resync"}"#]
      bytes = items[0].utf8.count
    }
  }
  private let state = Mutex(State())

  func configure(view: String) {
    state.withLock { state in
      state.generation &+= 1
      state.view = view
      state.items.removeAll()
      state.bytes = 0
    }
  }
  /// Queues `json`; returns true when the caller should start a drain.
  func append(_ json: String) -> Bool {
    state.withLock { state in
      if state.items.count >= Limits.pushItems || state.bytes + json.utf8.count > Limits.pushBytes {
        state.markResync()
      } else {
        state.items.append(json)
        state.bytes += json.utf8.count
      }
      if state.draining { return false }
      state.draining = true
      return true
    }
  }
  func peek() -> Batch? {
    state.withLock { state in
      if state.items.isEmpty {
        state.draining = false
        return nil
      }
      return Batch(generation: state.generation, view: state.view, items: state.items)
    }
  }
  func acknowledge(_ batch: Batch) {
    state.withLock { state in
      guard batch.generation == state.generation else { return }
      state.items.removeFirst(batch.items.count)
      state.bytes -= batch.items.reduce(0) { $0 + $1.utf8.count }
    }
  }
  func failed(_ batch: Batch) {
    state.withLock { state in
      guard batch.generation == state.generation else { return }
      state.markResync()
    }
  }
}
