import HitSlopDocument
import Testing

@testable import HitSlopHost

// The save-failure alert's rules, without a window: `SaveAttention` decides what the window
// presents. Data safety under a failed save is the Rust owner's and store's; that the app
// coordinator adds no second alert is `AppModelTests`.

private let busy = SaveAttention.alert(for: .busy)

@Test func aFailureShowsOneAlertUntilItEnds() {
  var attention = SaveAttention()
  let effect1 = attention.status(.failed(.busy), otherSheet: false, hidden: false)
  #expect(effect1 == .present(busy))
  // Its own alert holds the window now: a repeated failure neither shows nor waits.
  let effect2 = attention.status(.failed(.busy), otherSheet: true, hidden: false)
  #expect(effect2 == .none)
  let effect3 = attention.otherSheetEnded()
  #expect(!effect3)
  attention.alertEnded()
  // Keep Open (or any button) is final until another failure arrives.
  let effect4 = attention.reveal(otherSheet: false, hidden: false)
  #expect(effect4 == .present(busy))
}

// Failure: a save failure that arrived behind an unrelated sheet was never shown.
@Test func anUnrelatedSheetDefersTheAlertUntilItEnds() {
  var attention = SaveAttention()
  let effect5 = attention.status(.failed(.busy), otherSheet: true, hidden: false)
  #expect(effect5 == .none)
  let effect6 = attention.otherSheetEnded()
  #expect(effect6)
  let effect7 = attention.reveal(otherSheet: false, hidden: false)
  #expect(effect7 == .present(busy))
  // Only a deferred failure tries again when a sheet ends.
  let effect8 = attention.otherSheetEnded()
  #expect(!effect8)
}

@Test func aSaveWhileDeferredOrShownLeavesNoAlert() {
  var deferred = SaveAttention()
  _ = deferred.status(.failed(.busy), otherSheet: true, hidden: false)
  let effect9 = deferred.status(.saved, otherSheet: true, hidden: false)
  #expect(effect9 == .none)
  let effect10 = deferred.otherSheetEnded()
  #expect(!effect10)
  let effect11 = deferred.reveal(otherSheet: false, hidden: false)
  #expect(effect11 == .none)

  var shown = SaveAttention()
  _ = shown.status(.failed(.busy), otherSheet: false, hidden: false)
  let effect12 = shown.status(.saving, otherSheet: true, hidden: false)
  #expect(effect12 == .none)
  let effect13 = shown.status(.saved, otherSheet: true, hidden: false)
  #expect(effect13 == .dismiss)
  shown.alertEnded()
  #expect(shown.failure == nil)
}

@Test func aWindowHiddenForCloseShowsTheFailureOnlyOnceItReturns() {
  var attention = SaveAttention()
  let effect14 = attention.status(.failed(.busy), otherSheet: false, hidden: true)
  #expect(effect14 == .none)
  let effect15 = attention.reveal(otherSheet: false, hidden: false)
  #expect(effect15 == .present(busy))
}

@Test func aRendererDeathReplacesTheAlertWithItsOverlay() {
  var attention = SaveAttention()
  _ = attention.status(.failed(.busy), otherSheet: false, hidden: false)
  let effect16 = attention.clear()
  #expect(effect16 == .dismiss)
  attention.alertEnded()
  let effect17 = attention.reveal(otherSheet: false, hidden: false)
  #expect(effect17 == .none)
}

@Test func eachFailureOffersItsWayBack() {
  let actions = { (failure: SaveFailure) in SaveAttention.alert(for: failure).buttons.map(\.action) }
  #expect(actions(.busy) == [.retrySave, .keepOpen])
  #expect(actions(.moved) == [.retrySave, .keepOpen])
  #expect(actions(.full) == [.retrySave, .discardAndRetry, .keepOpen])
  #expect(actions(.invalidated) == [.discardAndRetry, .keepOpen])
  #expect(SaveAttention.alert(for: .invalidated).title == "The document engine needs recovery")
  #expect(busy.title == "Changes could not be saved")
  #expect(busy.message == SaveFailure.busy.localizedDescription)
}
