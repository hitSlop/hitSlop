import Foundation
import HitSlopCore

/// Delivery retains its batch until acknowledgement. Overflow and view replacement
/// invalidate old acknowledgements, so they can never consume a new resync marker.
final class PushQueue: @unchecked Sendable {
  struct Batch {
    let generation: UInt64
    let view: String
    let items: [String]
  }
  private let lock = NSLock()
  private var items: [String] = []
  private var bytes = 0
  private var generation: UInt64 = 0
  private var view = ""
  private var draining = false

  func configure(view: String) {
    lock.withLock {
      generation &+= 1
      self.view = view
      items.removeAll()
      bytes = 0
    }
  }
  private func markResync() {
    generation &+= 1
    items = [#"{"view":"\#(view)","type":"resync"}"#]
    bytes = items[0].utf8.count
  }
  func append(_ json: String) -> Bool {
    lock.withLock {
      // Pushes are non-empty JSON objects built by the owner; publications are passed
      // through unparsed, so the view is spliced in as the first member.
      precondition(json.hasPrefix("{") && !json.hasPrefix("{}"), "push must be a non-empty JSON object")
      let tagged = #"{"view":"\#(view)","# + json.dropFirst()
      if items.count >= Limits.pushItems || bytes + tagged.utf8.count > Limits.pushBytes {
        markResync()
      } else {
        items.append(tagged)
        bytes += tagged.utf8.count
      }
      if draining { return false }
      draining = true
      return true
    }
  }
  func peek() -> Batch? {
    lock.withLock {
      if items.isEmpty { draining = false; return nil }
      return Batch(generation: generation, view: view, items: items)
    }
  }
  func acknowledge(_ batch: Batch) {
    lock.withLock {
      guard batch.generation == generation else { return }
      items.removeFirst(batch.items.count)
      bytes -= batch.items.reduce(0) { $0 + $1.utf8.count }
    }
  }
  func failed(_ batch: Batch) {
    lock.withLock {
      guard batch.generation == generation else { return }
      markResync()
    }
  }
}
