import Foundation
import Testing

@testable import HitSlopFeatures

@Test @MainActor func catalogReturnsWhileCloseStillOwnsTheDocument() async {
  let native = Native()
  let artwork = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { id, _ in
    model.documentHiddenForClose(id)
    await artwork.wait()
    return nil
  }
  model.send(.close, to: documentID)
  await until { native.calls.contains(.noDocuments) }
  #expect(model[id: documentID]?.isHiddenForClose == true)
  #expect(!model.acceptsCommands(documentID))
  #expect(model.documents.count == 1, "the close still owns the document")
  model.documentHiddenForClose(documentID)
  artwork.open()
  await model.settled()
  #expect(model.documents.isEmpty)
  #expect(native.calls.filter { $0 == .noDocuments }.count == 1)
}

@Test @MainActor func catalogWaitsUntilEveryDocumentHasHiddenForClose() async {
  let native = Native()
  let artwork = Gate()
  let model = await app(native, open: [documentID, otherID])
  native.perform = { id, _ in
    model.documentHiddenForClose(id)
    await artwork.wait()
    return nil
  }
  model.send(.close, to: documentID)
  await until { model[id: documentID]?.isHiddenForClose == true }
  #expect(!native.calls.contains(.noDocuments))
  model.send(.close, to: otherID)
  await until { native.calls.contains(.noDocuments) }
  #expect(model.documents.count == 2)
  artwork.open()
  await model.settled()
  #expect(native.calls.filter { $0 == .noDocuments }.count == 1)
}

@Test @MainActor func closingDoesNotInterruptAnotherDocumentOpening() async {
  let native = Native()
  let artwork = Gate()
  let opening = Gate()
  let model = await app(native, open: [documentID], ids: [otherID])
  native.open = { _, _ in await opening.wait() }
  native.perform = { id, _ in
    model.documentHiddenForClose(id)
    await artwork.wait()
    return nil
  }
  model.open(url(otherID))
  model.send(.close, to: documentID)
  await until { model[id: documentID]?.isHiddenForClose == true }
  #expect(!native.calls.contains(.noDocuments))
  artwork.open()
  opening.open()
  await model.settled()
  #expect(model.documents.map(\.id) == [otherID])
  #expect(!native.calls.contains(.noDocuments))
}

@Test @MainActor func finishingAnOldCloseDoesNotRefocusTheCatalog() async {
  let native = Native()
  let artwork = Gate()
  let model = await app(native, open: [documentID], ids: [otherID])
  native.open = { _, _ in }
  native.perform = { id, _ in
    model.documentHiddenForClose(id)
    await artwork.wait()
    return nil
  }
  model.send(.close, to: documentID)
  await until { native.calls.contains(.noDocuments) }
  model.open(url(otherID))
  await until { model[id: otherID]?.isOpening == false }
  artwork.open()
  await model.settled()
  #expect(native.calls.filter { $0 == .noDocuments }.count == 1)
  #expect(model.documents.map(\.id) == [otherID])
}

@Test @MainActor func failedHiddenCloseRestoresTheDocumentAndConsumesReopen() async {
  let native = Native()
  let artwork = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { id, _ in
    model.documentHiddenForClose(id)
    await artwork.wait()
    throw Failure()
  }
  model.send(.close, to: documentID)
  await until { native.calls.contains(.noDocuments) }
  model.open(documentURL)
  artwork.open()
  await model.settled()
  #expect(model[id: documentID]?.isHiddenForClose == false)
  #expect(model[id: documentID]?.reopenRequested == false)
  #expect(model.acceptsCommands(documentID))
  #expect(native.calls.contains(.focus(documentID)))
  native.perform = { id, _ in
    model.documentHiddenForClose(id)
    return nil
  }
  model.send(.close, to: documentID)
  await model.settled()
  #expect(model.documents.isEmpty)
  #expect(native.calls.filter { $0 == .noDocuments }.count == 2)
}

@Test @MainActor func openingAQueuedCloseSurvivesAnEarlierOperationFailure() async {
  let native = Native()
  let exporting = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { _, _ in
    await exporting.wait()
    throw Failure()
  }
  model.send(.exportPNG, to: documentID)
  model.send(.close, to: documentID)
  model.open(documentURL)
  #expect(!native.calls.contains(.focus(documentID)))
  exporting.open()
  await model.settled()
  #expect(model.acceptsCommands(documentID))
  #expect(model[id: documentID]?.reopenRequested == false)
  #expect(native.calls.contains(.focus(documentID)))
  #expect(native.performed == [.exportPNG])
}

@Test @MainActor func quitWaitsForHiddenCloseAndCancelsReopen() async {
  let native = Native()
  let artwork = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { id, _ in
    model.documentHiddenForClose(id)
    await artwork.wait()
    return nil
  }
  model.send(.close, to: documentID)
  await until { native.calls.contains(.noDocuments) }
  model.open(documentURL)
  model.requestQuit()
  #expect(model.quitPhase == .waiting)
  #expect(native.replies.isEmpty)
  artwork.open()
  await model.settled()
  #expect(model.documents.isEmpty)
  #expect(native.replies == [true])
  #expect(native.calls.filter { $0 == .noDocuments }.count == 1)
}

@Test @MainActor func quitBeforeTheHandoffNeverShowsTheCatalog() async {
  let native = Native()
  let barrier = Gate()
  let model = await app(native, open: [documentID])
  native.perform = { id, _ in
    await barrier.wait()
    model.documentHiddenForClose(id)
    return nil
  }
  model.send(.close, to: documentID)
  model.requestQuit()
  barrier.open()
  await model.settled()
  #expect(!native.calls.contains(.noDocuments))
  #expect(native.replies == [true])
}
