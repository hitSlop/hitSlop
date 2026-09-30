import Foundation
import Testing
@testable import HitSlopDocument

struct PublicationDeliveryTests {
  @Test func overflowRequiresOneSnapshotInsteadOfAnUnboundedBacklog() {
    let queue = PushQueue()
    queue.configure(view: "first")
    for i in 0..<257 { _ = queue.append(#"{"type":"saved","sequence":\#(i)}"#) }
    let batch = queue.peek()
    #expect(batch?.items.count == 1)
    #expect(batch?.items.first?.contains("resync") == true)
  }
}


extension PublicationDeliveryTests {
  @Test func oldDeliveryCannotAcknowledgeAnOverflowResync() throws {
    let queue = PushQueue()
    queue.configure(view: "first")
    _ = queue.append(#"{"type":"saved","sequence":1}"#)
    let inFlight = try #require(queue.peek())
    for i in 2...257 { _ = queue.append(#"{"type":"saved","sequence":\#(i)}"#) }
    queue.acknowledge(inFlight)
    #expect(queue.peek()?.items.first?.contains("resync") == true)
  }
  @Test func retiredViewDeliveryCannotConsumeNewViewPushes() throws {
    let queue = PushQueue()
    queue.configure(view: "first")
    _ = queue.append(#"{"type":"saved","sequence":1}"#)
    let inFlight = try #require(queue.peek())
    queue.configure(view: "second")
    _ = queue.append(#"{"type":"saved","sequence":2}"#)
    queue.acknowledge(inFlight)
    queue.failed(inFlight)
    let next = try #require(queue.peek())
    #expect(next.view == "second")
    #expect(next.items.count == 1)
    #expect(next.items[0].contains(#""sequence":2"#))
  }
}
