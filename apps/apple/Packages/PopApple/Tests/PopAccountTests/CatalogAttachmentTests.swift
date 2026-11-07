import Testing

@testable import PopAccount

/// Firestore removes a listener after a server rejection. A catalog that kept trusting it
/// would never re-attach, and the account's cloud documents would stop arriving until
/// relaunch. After a terminal failure the same account needs a new attachment, and callbacks
/// from an earlier attachment, of this account or another, never count.
@Test func aFailedListenerIsReattachedAndEarlierListenersNeverCount() {
  var attachment = CatalogAttachment()
  let first = attachment.begin("a")
  #expect(!attachment.isReady("a"), "attaching is not attached")
  attachment.attached(first, account: "a")
  #expect(attachment.isReady("a"))

  let dropped = attachment.failed(first, account: "a")
  #expect(dropped, "the failed listener is dropped")
  #expect(!attachment.isReady("a"), "the next connect re-attaches")
  #expect(attachment.account == "a")
  let second = attachment.begin("a")
  attachment.attached(second, account: "a")
  let lateFailure = attachment.failed(first, account: "a")
  #expect(!lateFailure, "a late failure of the dropped listener keeps the new one")
  #expect(attachment.isReady("a"))

  let other = attachment.begin("b")
  attachment.attached(second, account: "a")
  #expect(!attachment.isReady("a") && !attachment.isReady("b"), "a late attachment of the previous account")
  #expect(!attachment.isCurrent(second), "its snapshots are dropped")
  attachment.attached(other, account: "b")
  #expect(attachment.isReady("b"))
  let previousFailure = attachment.failed(second, account: "a")
  #expect(!previousFailure && attachment.isReady("b"), "the previous account's failure keeps this one")
}
