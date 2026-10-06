import Foundation
import HitSlopCore
import Testing

@testable import HitSlopFeatures

// MARK: Opening

@Test @MainActor func openingADocumentThatNeedsANewerAppOffersTheUpdate() async {
  let native = Native()
  let model = await app(native, ids: [documentID, otherID])
  native.open = { _, _ in throw SlopRequiresUpdate() }
  model.open(documentURL)
  await model.settled()
  #expect(model.documents.isEmpty)
  #expect(native.alerts == [.alert(.requiresUpdate(SlopRequiresUpdate().localizedDescription), nil)])

  // Other failures only acknowledge.
  native.open = { _, _ in throw Failure() }
  model.open(documentURL)
  await model.settled()
  #expect(native.alerts.last == .alert(.failure("Save failed"), nil))
}

@Test @MainActor func repeatedOpenDuringPreparationUsesOneOperation() async {
  let native = Native()
  let catalog = Catalog()
  let gate = Gate()
  let model = await app(native, catalog: catalog, ids: [documentID])
  native.open = { _, _ in await gate.wait() }
  model.open(documentURL)
  #expect(model.documents.map(\.id) == [documentID] && model[id: documentID]?.isOpening == true)
  model.open(documentURL)
  gate.open()
  await model.settled()
  #expect(model[id: documentID]?.isOpening == false)
  #expect(native.calls.filter { $0 == .open(documentID) }.count == 1)
  #expect(native.calls.filter { $0 == .focus(documentID) }.count == 1)
  #expect(native.enabled[documentID] == true)
  #expect(catalog.calls == [.recents], "an opened document joins Recents")
}

@Test @MainActor func openingAnExistingDocumentFocusesIt() async {
  let native = Native()
  let model = await app(native, open: [documentID])
  model.open(documentURL)
  await model.settled()
  #expect(native.calls == [.focus(documentID)])
}

@Test @MainActor func cancelledDocumentOpenRemovesPendingDocumentWithoutError() async {
  let native = Native()
  let model = await app(native, ids: [documentID])
  native.open = { _, _ in throw CancellationError() }
  model.open(documentURL)
  #expect(model.documents.map(\.id) == [documentID])
  await model.settled()
  #expect(model.documents.isEmpty)
  #expect(native.alerts.isEmpty)
}

// MARK: Operations

@Test @MainActor func closeWaitsForExportAndRunsOnlyOnce() async {
  let native = Native()
  let gate = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { _, command in
    if command == .exportPNG { await gate.wait() }
    return nil
  }
  model.send(.exportPNG, to: documentID)
  #expect(model[id: documentID]?.queue.running == .exportPNG)
  #expect(native.enabled[documentID] == false)
  model.send(.close, to: documentID)
  #expect(model[id: documentID]?.queue.closeRequested == true)
  model.send(.close, to: documentID)
  gate.open()
  await model.settled()
  #expect(native.performed == [.exportPNG, .close])
  #expect(model.documents.isEmpty)
  #expect(native.calls.last == .noDocuments, "the catalog returns once nothing is open")
}

@Test @MainActor func saveFailureRetainsDocumentForRetry() async {
  let native = Native()
  let model = await app(native, open: [documentID])
  native.perform = { _, _ in throw Failure() }
  model.send(.close, to: documentID)
  await model.settled()
  #expect(model.acceptsCommands(documentID))
  #expect(native.alerts == [.alert(.failure("Save failed"), documentID)])
  model.send(.close, to: documentID)
  await model.settled()
  #expect(native.performed == [.close, .close])
  #expect(native.alerts.count == 2, "a repeated failure alerts again")
}

@Test @MainActor func closingOneDocumentLeavesTheOtherDocumentAndItsStateAlone() async {
  let native = Native()
  let model = await app(native, open: [documentID, otherID])
  native.perform = { _, _ in nil }
  let other = model[id: otherID]
  model.send(.close, to: documentID)
  await model.settled()
  #expect(model.documents == [other].compactMap { $0 })
  #expect(native.calls == [.perform(documentID, .close)])
}

@Test @MainActor func aFailedOperationAlertsOnItsWindowAndTheDocumentTakesARetry() async {
  let native = Native()
  let model = await app(native, open: [documentID, otherID])
  native.perform = { _, command in
    if command == .exportPNG { throw Failure() }
    return nil
  }
  model.send(.exportPNG, to: documentID)
  model.send(.exportPNG, to: otherID)
  await model.settled()
  #expect(native.alerts == [.alert(.failure("Save failed"), documentID), .alert(.failure("Save failed"), otherID)])
  model.send(.retry, to: documentID)
  await model.settled()
  #expect(native.calls.last == .perform(documentID, .retry))
}

// Failure: a close that stopped on a failed save showed a generic alert on top of the
// window's save-failure sheet. Oracle: a save failure leaves no alert and commands return.
@Test @MainActor func aSaveFailureIsLeftToTheSaveFailureSheet() async {
  let native = Native()
  let model = await app(native, open: [documentID])
  native.perform = { _, _ in throw SlopDocumentFailure.save }
  model.send(.close, to: documentID)
  await model.settled()
  #expect(native.alerts.isEmpty)
  #expect(model.acceptsCommands(documentID))
}

// Failure: the save-failure sheet ran its own recovery beside the command in progress.
// Oracle: a recovery chosen during an export runs after it, before a queued close.
@Test @MainActor func aSaveRecoveryChosenDuringAnotherCommandRunsNext() async {
  let native = Native()
  let gate = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { _, command in
    if command == .exportPNG { await gate.wait() }
    return nil
  }
  model.send(.exportPNG, to: documentID)
  model.send(.discardUnsaved, to: documentID)
  #expect(model[id: documentID]?.queue.pendingRecovery == .discardUnsaved)
  model.send(.close, to: documentID)
  gate.open()
  await model.settled()
  #expect(native.performed == [.exportPNG, .discardUnsaved, .close])
}

@Test @MainActor func aDuplicateOpensTheCopy() async {
  let native = Native()
  let duplicateURL = URL(fileURLWithPath: "/tmp/duplicate.slop")
  let model = await app(native, open: [documentID], ids: [otherID])
  native.perform = { _, _ in duplicateURL }
  native.open = { _, _ in }
  model.send(.duplicate, to: documentID)
  await model.settled()
  #expect(model.documents.map(\.url) == [documentURL, duplicateURL])
  #expect(model.acceptsCommands(otherID))
}

// MARK: Quitting

// A late quit failure must not resurrect sessions whose native teardown already completed.
@Test @MainActor func partialQuitKeepsCompletedDocumentsClosedAndRetriesOnlyRemainingDocuments() async {
  let ids = [documentID, otherID, UUID()]
  let native = Native()
  let model = await app(native, open: ids)
  native.prepareToQuit = { _ in }
  native.finishQuit = { id in
    if id == ids[1], native.calls.filter({ $0 == .finish(ids[1]) }).count == 1 { throw Failure() }
  }
  model.requestQuit()
  await model.settled()
  #expect(native.calls.filter { if case .finish = $0 { true } else { false } } == [.finish(ids[0]), .finish(ids[1])])
  #expect(model.documents.map(\.id) == Array(ids.dropFirst()))
  #expect(ids.dropFirst().allSatisfy(model.acceptsCommands))
  #expect(native.calls.filter { if case .cancel = $0 { true } else { false } } == ids.dropFirst().map { .cancel($0) })
  #expect(model.quitPhase == .running)
  #expect(native.alerts == [.alert(.failure("Save failed"), nil)])
  #expect(native.replies == [false])

  model.requestQuit()
  await model.settled()
  #expect(model.documents.isEmpty)
  #expect(native.calls.filter { if case .finish = $0 { true } else { false } }.count == 4)
  #expect(native.replies == [false, true])
}

@Test @MainActor func quitFailureRestoresCommandsAndRepliesFalse() async {
  let native = Native()
  let model = await app(native, open: [documentID])
  native.prepareToQuit = { _ in throw Failure() }
  model.requestQuit()
  #expect(model.quitPhase == .preparing && model.catalog.isQuitting)
  #expect(native.enabled[documentID] == false)
  await model.settled()
  #expect(model.quitPhase == .running && !model.catalog.isQuitting)
  #expect(native.enabled[documentID] == true)
  #expect(
    native.calls == [.prepare(documentID), .cancel(documentID), .alert(.failure("Save failed"), nil), .reply(false)])
  #expect(model.documents.count == 1)
}

@Test @MainActor func quitWaitsForCreationAndOpeningBeforePreparingTheNewDocument() async {
  let native = Native()
  let catalog = Catalog()
  let creation = Gate()
  let opening = Gate()
  let preparation = Gate()
  let template = entry("a")
  catalog.chooseDestination = { _ in documentURL }
  catalog.create = { _, url in
    await creation.wait()
    return url
  }
  let model = await app(native, catalog: catalog, ids: [documentID])
  native.open = { _, _ in await opening.wait() }
  native.prepareToQuit = { _ in await preparation.wait() }
  native.finishQuit = { _ in }

  model.catalog.primaryAction(template)
  #expect(model.catalog.creating == template)
  await until { model.catalog.isCopying }
  model.requestQuit()
  #expect(model.quitPhase == .waiting && model.catalog.isQuitting)
  model.open(URL(fileURLWithPath: "/tmp/rejected.slop"))
  #expect(model.documents.isEmpty, "nothing new opens once quit began")

  creation.open()
  await until { model.catalog.creating == nil }
  #expect(model.documents.map(\.id) == [documentID] && model[id: documentID]?.isOpening == true)
  #expect(model.quitPhase == .waiting, "quit waits for the new document to open")
  opening.open()
  await until { model.quitPhase == .preparing }
  #expect(model[id: documentID]?.isOpening == false)
  #expect(native.replies.isEmpty)
  preparation.open()
  await model.settled()
  #expect(model.documents.isEmpty && model.quitPhase == .finished)
  #expect(native.calls == [.open(documentID), .prepare(documentID), .finish(documentID), .reply(true)])
}

@Test @MainActor func quitWaitsForAnExportAndPreparesBeforeReplying() async {
  let native = Native()
  let gate = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { _, _ in
    await gate.wait()
    return nil
  }
  native.prepareToQuit = { _ in }
  native.finishQuit = { _ in }
  model.send(.exportPNG, to: documentID)
  model.requestQuit()
  #expect(model.quitPhase == .waiting)
  model.requestQuit()
  gate.open()
  await model.settled()
  #expect(native.calls == [.perform(documentID, .exportPNG), .prepare(documentID), .finish(documentID), .reply(true)])
  #expect(model.documents.isEmpty && model.quitPhase == .finished)
}

@Test @MainActor func aFailedPendingCloseCancelsQuitWithOneError() async {
  let native = Native()
  let gate = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { _, _ in
    await gate.wait()
    throw SlopDocumentFailure.other("Save failed")
  }
  model.send(.close, to: documentID)
  model.requestQuit()
  #expect(model.quitPhase == .waiting)
  gate.open()
  await model.settled()
  #expect(native.alerts == [.alert(.failure("Save failed"), nil)], "one alert, over the app, none on the window")
  #expect(native.replies == [false])
  #expect(model.quitPhase == .running && model.acceptsCommands(documentID))
}

// Failure: quit dropped every command, so Retry or Discard chosen on a save-failure sheet
// while quit waited dismissed the sheet and did nothing. Oracle: the recovery runs after
// the operation quit waits for, and quit prepares only after it.
@Test @MainActor func aSaveRecoveryChosenDuringQuitRunsBeforeQuitPrepares() async {
  let native = Native()
  let gate = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { _, command in
    if command == .exportPNG { await gate.wait() }
    return nil
  }
  native.prepareToQuit = { _ in }
  native.finishQuit = { _ in }
  model.send(.exportPNG, to: documentID)
  model.requestQuit()
  model.send(.duplicate, to: documentID)
  model.send(.retrySave, to: documentID)
  #expect(model[id: documentID]?.queue.pendingRecovery == .retrySave)
  gate.open()
  await model.settled()
  #expect(
    native.calls == [
      .perform(documentID, .exportPNG), .perform(documentID, .retrySave), .prepare(documentID), .finish(documentID),
      .reply(true),
    ])
}

@Test @MainActor func aSaveFailureDuringQuitHasNoSecondAlert() async {
  let native = Native()
  let model = await app(native, open: [documentID])
  native.prepareToQuit = { _ in throw SlopDocumentFailure.save }
  model.requestQuit()
  await model.settled()
  #expect(native.alerts.isEmpty)
  #expect(native.replies == [false])
}

@Test @MainActor func quitIncludesADuplicateBeforePreparingDocuments() async {
  let duplicateURL = URL(fileURLWithPath: "/tmp/duplicate.slop")
  let native = Native()
  let duplicate = Gate()
  let opening = Gate()
  let preparation = Gate()
  let model = await app(native, open: [documentID], ids: [otherID])
  native.perform = { id, command in
    #expect(id == documentID && command == .duplicate)
    await duplicate.wait()
    return duplicateURL
  }
  native.open = { id, url in
    #expect(id == otherID && url == duplicateURL)
    await opening.wait()
  }
  native.prepareToQuit = { id in if id == documentID { await preparation.wait() } }
  native.finishQuit = { _ in }

  model.send(.duplicate, to: documentID)
  model.requestQuit()
  #expect(model.quitPhase == .waiting)
  duplicate.open()
  await until { model[id: otherID] != nil }
  #expect(model[id: documentID]?.queue.isIdle == true && model.quitPhase == .waiting)
  opening.open()
  await until { model.quitPhase == .preparing }
  #expect(native.replies.isEmpty)
  preparation.open()
  await model.settled()
  #expect(
    native.calls.filter { if case .prepare = $0 { true } else { false } } == [.prepare(documentID), .prepare(otherID)])
  #expect(native.replies == [true])
  #expect(model.documents.isEmpty)
}

@Test @MainActor func multiDocumentQuitFailureCancelsEveryPreparationWithoutFinishing() async {
  let native = Native()
  let model = await app(native, open: [documentID, otherID])
  native.prepareToQuit = { id in if id == otherID { throw Failure() } }
  model.requestQuit()
  await model.settled()
  #expect(
    native.calls == [
      .prepare(documentID), .prepare(otherID), .cancel(documentID), .cancel(otherID),
      .alert(.failure("Save failed"), nil), .reply(false),
    ])
}
