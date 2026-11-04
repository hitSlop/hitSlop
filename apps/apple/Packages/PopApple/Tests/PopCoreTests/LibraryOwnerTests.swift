@preconcurrency import CloudKit
import Foundation
import PopCoreBinding
import PopTestSupport
import Synchronization
import Testing

@testable import PopCore
@testable import PopDocument

@Test func dotpopSyncLeavesLegacyCloudRecordsAndCachedDocumentsAlone() async throws {
  let base = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: base) }
  let template = try Fixtures.template(stage: Fixtures.stage())
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let account = "fixture"
  let legacy = try NativeLibrary.open(
    root: base.appendingPathComponent(account, isDirectory: true).path, account: account)
  let old = try legacy.create(template: template.path, title: "Legacy cached document")
  let original = try Data(contentsOf: URL(fileURLWithPath: old.path))
  let staged = try legacy.stage(id: old.id)
  try legacy.acknowledge(
    id: old.id, version: staged.entry.version, assetRevision: staged.entry.assetRevision,
    fields: Data(), deleted: false, stamp: nil)
  if let asset = staged.assetPath { try legacy.releaseUpload(path: asset) }

  let legacyZone = CKRecordZone.ID(zoneName: "Library")
  let legacyRecord = CKRecord(
    recordType: "SlopDocument",
    recordID: CKRecord.ID(recordName: UUID().uuidString.lowercased(), zoneID: legacyZone))
  legacyRecord["format"] = 0
  legacyRecord["title"] = "Legacy cloud document"
  let location = try LibraryLocation(base: base, account: account, development: false)
  let current = try NativeLibrary.open(root: location.root.path, account: account)
  let uploaded = Mutex<[CKRecord.ID]>([])
  let sync = LibrarySync(library: current, zone: location.zone) { operation in
    if let operation = operation as? CKModifyRecordZonesOperation {
      operation.modifyRecordZonesResultBlock?(.success(()))
    } else if let operation = operation as? CKModifySubscriptionsOperation {
      operation.modifySubscriptionsResultBlock?(.success(()))
    } else if let operation = operation as? CKFetchRecordZoneChangesOperation {
      if operation.recordZoneIDs?.contains(legacyZone) == true {
        operation.recordWasChangedBlock?(legacyRecord.recordID, .success(legacyRecord))
      }
      // A deterministic transport failure avoids inventing an opaque server token.
      // Per-record failures and accepted uploads are still observable independently.
      operation.fetchRecordZoneChangesResultBlock?(.failure(CKError(.networkUnavailable)))
    } else if let operation = operation as? CKModifyRecordsOperation {
      for record in operation.recordsToSave ?? [] {
        uploaded.withLock { $0.append(record.recordID) }
        operation.perRecordSaveBlock?(record.recordID, .success(CommittedCloudRecord(committing: record)))
      }
      operation.modifyRecordsResultBlock?(.success(()))
    }
  }
  let first = await sync.sync(.full)
  #expect(first.issues.isEmpty)
  try #require(try current.entries().isEmpty)

  let fresh = try current.create(template: template.path, title: "New pop")
  let saved = await sync.sync(.full, explicit: true)
  #expect(saved.issues.isEmpty)
  #expect(saved.pending == 0)
  #expect(try current.entries().map(\.id) == [fresh.id])
  let uploads = uploaded.withLock { $0 }
  #expect(uploads.count == 1)
  #expect(uploads.first?.recordName == fresh.id)
  #expect(uploads.first?.zoneID != legacyZone)
  #expect(try legacy.entries().map(\.id) == [old.id])
  #expect(try Data(contentsOf: URL(fileURLWithPath: old.path)) == original)
  #expect(legacyRecord["title"] as? String == "Legacy cloud document")
  await sync.stop()
}

// A cold file-open can beat the asynchronous iCloud connection. A checkpoint must
// still merge through that live writer, retaining its local edit and writer lease.
@Test func aDocumentOpenedBeforeTheLibraryAcceptsCloudCheckpoints() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let template = try Fixtures.template(stage: Fixtures.stage())
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let entry = try library.create(template: template.path, title: "Cold open")
  let checkpoint = try library.stage(id: entry.id)
  defer { if let asset = checkpoint.assetPath { try? library.releaseUpload(path: asset) } }
  let owner = try DocumentOwner(url: URL(fileURLWithPath: entry.path))
  let owners = LibraryOwners()
  try owners.register(owner.native)
  _ = try await owner.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#)
  try owners.activate(library)
  do {
    try library.receive(
      id: entry.id, title: entry.title, template: entry.template,
      asset: checkpoint.assetPath, version: checkpoint.entry.version,
      fields: Data(), deleted: false, artworkVersion: nil, assetRevision: nil, stamp: nil)
    let state = try Fixtures.object(await owner.state())["value"] as? [String: Any]
    #expect(state?["hits"] as? Int == 3)
    let upload = try library.stage(id: entry.id)
    if let asset = upload.assetPath { try library.releaseUpload(path: asset) }
    try await owner.close()
  } catch {
    try? await owner.close()
    throw error
  }
}

/// Counts finished callers across tasks.
private final class FinishedCount: Sendable { let value = Mutex(0) }

@Test func removedDownloadsStayEvictedDuringSyncAndDownloadOnRequest() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let template = try Fixtures.template(stage: Fixtures.stage())
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let entry = try library.create(template: template.path, title: "Downloadable")
  let checkpoint = try library.stage(id: entry.id)
  let asset = try #require(checkpoint.assetPath)
  try library.acknowledge(
    id: entry.id, version: checkpoint.entry.version, assetRevision: checkpoint.entry.assetRevision,
    fields: Data(), deleted: false, stamp: nil)
  try library.removeDownload(id: entry.id)
  let zone = CKRecordZone.ID(zoneName: "Library")
  let id = CKRecord.ID(recordName: entry.id, zoneID: zone)
  let metadata = CKRecord(recordType: "SlopDocument", recordID: id)
  metadata["format"] = 1
  metadata["title"] = "Renamed in iCloud"
  metadata["template"] = entry.template
  metadata["snapshotVersion"] = checkpoint.entry.version
  metadata["assetRevision"] = checkpoint.entry.assetRevision
  metadata["artworkVersion"] = checkpoint.entry.artworkVersion
  let downloads = FinishedCount()
  let sync = LibrarySync(library: library, zone: zone) { operation in
    if let operation = operation as? CKModifyRecordZonesOperation {
      operation.modifyRecordZonesResultBlock?(.success(()))
    } else if let operation = operation as? CKModifySubscriptionsOperation {
      operation.modifySubscriptionsResultBlock?(.success(()))
    } else if let operation = operation as? CKFetchRecordZoneChangesOperation {
      operation.recordWasChangedBlock?(id, .success(metadata))
      operation.fetchRecordZoneChangesResultBlock?(.failure(CKError(.networkUnavailable)))
    } else if let operation = operation as? CKFetchRecordsOperation {
      downloads.value.withLock { $0 += 1 }
      metadata["payload"] = CKAsset(fileURL: URL(fileURLWithPath: asset))
      operation.perRecordResultBlock?(id, .success(metadata))
      operation.fetchRecordsResultBlock?(.success(()))
    }
  }
  _ = await sync.sync(.changes)
  #expect(downloads.value.withLock { $0 } == 0)
  #expect(try library.entries().first?.title == "Renamed in iCloud")
  #expect(try library.entries().first?.downloaded == false)
  #expect(!FileManager.default.fileExists(atPath: entry.path))
  try await sync.download(id: entry.id)
  #expect(downloads.value.withLock { $0 } == 1)
  #expect(try library.entries().first?.downloaded == true)
  #expect(FileManager.default.fileExists(atPath: entry.path))
  await sync.stop()
}

// A second request must await the queued run, including its failure, rather than
// reporting success while the first CloudKit operation is still outstanding.
@Test(.timeLimit(.minutes(1))) func overlappingLibrarySyncRequestsAwaitTheRerun() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let operations = AsyncStream<CKDatabaseOperation>.makeStream()
  let sync = LibrarySync(library: library, zone: CKRecordZone.ID(zoneName: "Library")) {
    operations.continuation.yield($0)
  }
  let finished = FinishedCount()
  let first = Task { @Sendable [sync, finished] in
    let result = await sync.sync(.changes)
    finished.value.withLock { $0 += 1 }
    return result
  }
  var iterator = operations.stream.makeAsyncIterator()
  let initial = try #require(await iterator.next() as? CKModifyRecordZonesOperation)
  let second = Task { @Sendable [sync, finished] in
    let result = await sync.sync(.full, explicit: true)
    finished.value.withLock { $0 += 1 }
    return result
  }
  // Release CloudKit only once the second caller is queued behind the first run.
  while await !sync.hasQueuedRequest { await Task.yield() }
  #expect(finished.value.withLock { $0 } == 0)
  initial.modifyRecordZonesResultBlock?(.failure(CKError(.networkUnavailable)))
  let rerun = try #require(await iterator.next() as? CKModifyRecordZonesOperation)
  #expect(finished.value.withLock { $0 } == 0)
  rerun.modifyRecordZonesResultBlock?(.failure(CKError(.networkFailure)))
  let a = await first.value
  let b = await second.value
  #expect(a.lastSuccess == nil)
  #expect(a.lastError == b.lastError)
  #expect(a.lastError != nil)
  #expect(a.message == "Couldn't reach iCloud · will retry")
  #expect(finished.value.withLock { $0 } == 2)
  await sync.stop()
}

@Test func aFailedCloudRecordKeepsTheTokenAndItsRetryAcrossRelaunch() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let previous = Data([1, 2, 3])
  try library.saveChangeToken(bytes: previous)
  let zone = CKRecordZone.ID(zoneName: "Library")
  let id = CKRecord.ID(recordName: UUID().uuidString.lowercased(), zoneID: zone)
  let sync = LibrarySync(library: library, zone: zone) { operation in
    if let operation = operation as? CKModifyRecordZonesOperation {
      operation.modifyRecordZonesResultBlock?(.success(()))
    } else if let operation = operation as? CKModifySubscriptionsOperation {
      operation.modifySubscriptionsResultBlock?(.success(()))
    } else if let operation = operation as? CKFetchRecordZoneChangesOperation {
      operation.recordWasChangedBlock?(id, .failure(CKError(.assetFileNotFound)))
      operation.fetchRecordZoneChangesResultBlock?(.failure(CKError(.networkUnavailable)))
    }
  }
  let result = await sync.sync(.full)
  #expect(result.lastSuccess == nil)
  #expect(result.lastError != nil)
  #expect(result.issues[id.recordName] != nil)
  #expect(try library.changeToken() == previous)
  await sync.stop()
  let reopened = try NativeLibrary.open(root: folder.path, account: "fixture")
  let retry = try #require(reopened.retries().first)
  #expect(retry.id == id.recordName)
  #expect(retry.incoming)
  #expect(retry.retryAt > Date().timeIntervalSince1970)
  #expect(try reopened.changeToken() == previous)
}

@Test func automaticLibrarySyncWaitsForTheGlobalRetryWhileSyncNowOverridesIt() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let submitted = FinishedCount()
  let sync = LibrarySync(library: library, zone: CKRecordZone.ID(zoneName: "Library")) { operation in
    submitted.value.withLock { $0 += 1 }
    (operation as? CKModifyRecordZonesOperation)?.modifyRecordZonesResultBlock?(
      .failure(CKError(.requestRateLimited, userInfo: [CKErrorRetryAfterKey: 60.0])))
  }
  _ = await sync.sync(.changes)
  #expect(submitted.value.withLock { $0 } == 1)
  // Even a due record retry cannot bypass the run's global rate limit.
  try library.retry(
    retry: LibraryRetry(
      id: UUID().uuidString.lowercased(), title: "Retry", template: "fixture", message: "Offline",
      retryAt: 0, incoming: true))
  #expect(try await !sync.changesDue())
  _ = await sync.sync(.changes)
  _ = await sync.sync(.full, background: true)
  #expect(submitted.value.withLock { $0 } == 1)
  _ = await sync.sync(.full, explicit: true)
  #expect(submitted.value.withLock { $0 } == 2)
  await sync.stop()
}

@Test(arguments: [false, true]) func missingCloudDeletionPublishesAFreshTombstone(persistentFailure: Bool) async throws
{
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let template = try Fixtures.template(stage: Fixtures.stage())
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let entry = try library.create(template: template.path, title: "Deletion")
  try library.delete(id: entry.id)
  let saves = FinishedCount()
  let sync = LibrarySync(library: library, zone: CKRecordZone.ID(zoneName: "Library")) { operation in
    if let op = operation as? CKModifyRecordZonesOperation {
      op.modifyRecordZonesResultBlock?(.success(()))
    } else if let op = operation as? CKModifySubscriptionsOperation {
      op.modifySubscriptionsResultBlock?(.success(()))
    } else if let op = operation as? CKFetchRecordZoneChangesOperation {
      op.fetchRecordZoneChangesResultBlock?(.success(()))
    } else if let op = operation as? CKModifyRecordsOperation, let record = op.recordsToSave?.first {
      let attempt = saves.value.withLock {
        $0 += 1
        return $0
      }
      #expect(record["deleted"] as? Int == 1)
      #expect(record["payload"] == nil)
      if attempt == 1 || persistentFailure {
        op.perRecordSaveBlock?(record.recordID, .failure(CKError(.unknownItem)))
      } else {
        op.perRecordSaveBlock?(record.recordID, .success(CommittedCloudRecord(committing: record)))
      }
      op.modifyRecordsResultBlock?(.success(()))
    }
  }
  _ = await sync.sync(.full, explicit: true)
  #expect(saves.value.withLock { $0 } == (persistentFailure ? 3 : 2))
  #expect(try library.entries().allSatisfy { $0.deleted && $0.dirty == persistentFailure })
  #expect(try library.retries().isEmpty == !persistentFailure)
  await sync.stop()
}

@Test(arguments: ["fetch", "retry", "download"])
func unsupportedCloudRecordsPreserveUpdateGuidance(path: String) async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let zone = CKRecordZone.ID(zoneName: "Library")
  let id = CKRecord.ID(recordName: UUID().uuidString.lowercased(), zoneID: zone)
  let record = CKRecord(recordType: "SlopDocument", recordID: id)
  record["format"] = 2
  record["title"] = "Future pop"
  record["template"] = "fixture"
  record["snapshotVersion"] = "future"
  // A tombstone reaches download's final metadata guard without fetching an asset.
  record["deleted"] = path == "download" ? 1 : 0
  if path == "retry" {
    try library.retry(
      retry: LibraryRetry(
        id: id.recordName, title: "Future pop", template: "fixture", message: "Previous issue",
        retryAt: 0, incoming: true))
  }
  let sync = LibrarySync(library: library, zone: zone) { operation in
    if let op = operation as? CKModifyRecordZonesOperation {
      op.modifyRecordZonesResultBlock?(.success(()))
    } else if let op = operation as? CKModifySubscriptionsOperation {
      op.modifySubscriptionsResultBlock?(.success(()))
    } else if let op = operation as? CKFetchRecordZoneChangesOperation {
      if path == "fetch" { op.recordWasChangedBlock?(id, .success(CommittedCloudRecord(committing: record))) }
      op.fetchRecordZoneChangesResultBlock?(.failure(CKError(.networkUnavailable)))
    } else if let op = operation as? CKFetchRecordsOperation {
      op.perRecordResultBlock?(id, .success(CommittedCloudRecord(committing: record)))
      op.fetchRecordsResultBlock?(.success(()))
    }
  }
  if path == "download" {
    do {
      try await sync.download(id: id.recordName)
      Issue.record("An unsupported cloud record must require an update")
    } catch LibraryError.Failed(let issue, _) {
      #expect(issue == .requiresUpdate)
    } catch {
      Issue.record("Expected a typed update refusal, received: \(error)")
    }
    #expect(try library.entries().isEmpty)
  } else {
    let result = await sync.sync(.full, explicit: true)
    #expect(result.message == "Update pop to sync this pop.")
    #expect(result.issues[id.recordName] == "Update pop to sync this pop.")
    let reopened = try NativeLibrary.open(root: folder.path, account: "fixture")
    #expect(try reopened.retries().first?.message == "Update pop to sync this pop.")
  }
  await sync.stop()
}

@Test func unknownCloudFailuresKeepTheirDetailsOutOfGuidance() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let sync = LibrarySync(library: library, zone: CKRecordZone.ID(zoneName: "Library")) { operation in
    (operation as? CKModifyRecordZonesOperation)?.modifyRecordZonesResultBlock?(
      .failure(CloudFailure("Private transport details")))
  }
  let result = await sync.sync(.full)
  #expect(result.message == "Some changes couldn't sync. Please retry.")
  await sync.stop()
}

@Test func catalogAuthorityRoundTripsThroughRustAndKeepsFormatOneAcceptance() throws {
  let relay = CatalogAuthority.relay(
    transition: String(repeating: "a", count: 32), room: "creator-room", seedHash: String(repeating: "b", count: 64))
  let encoded = try libraryEncodeAuthority(authority: relay)
  #expect(try libraryDecodeAuthority(format: 2, json: encoded) == relay)
  #expect(try libraryDecodeAuthority(format: 1, json: nil) == .snapshot(receipt: nil))
  #expect(throws: LibraryError.self) { try libraryDecodeAuthority(format: 3, json: nil) }
}

@Test func aLiveRelayCatalogEntryFetchesNoCloudKitDocumentAsset() async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let template = try Fixtures.template(stage: Fixtures.stage())
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let original = try library.create(template: template.path, title: "Private copy")
  let privateUpload = try library.stage(id: original.id)
  try library.acknowledge(
    id: original.id, version: privateUpload.entry.version,
    assetRevision: privateUpload.entry.assetRevision, fields: Data(), deleted: false,
    stamp: CatalogStamp(revision: 1, changeTag: "private-record"))
  if let path = privateUpload.assetPath { try library.releaseUpload(path: path) }
  try library.removeDownload(id: original.id)
  let zone = CKRecordZone.ID(zoneName: "CatalogAuthority")
  let id = CKRecord.ID(recordName: original.id, zoneID: zone)
  let relay = CatalogAuthority.relay(
    transition: String(repeating: "a", count: 32), room: "creator-room", seedHash: String(repeating: "b", count: 64))
  let record = CKRecord(recordType: "SlopDocument", recordID: id)
  record["format"] = 2
  record["catalogRevision"] = Int64(2)
  record["snapshotVersion"] = privateUpload.entry.version
  record["assetRevision"] = privateUpload.entry.assetRevision
  record["authority"] = try libraryEncodeAuthority(authority: relay)
  record["title"] = "Live on another device"
  record["template"] = "fixture"
  let committed = CommittedCloudRecord(committing: record)
  let assets = FinishedCount()
  let uploads = FinishedCount()
  let sync = LibrarySync(library: library, zone: zone) { operation in
    if let op = operation as? CKModifyRecordZonesOperation {
      op.modifyRecordZonesResultBlock?(.success(()))
    } else if let op = operation as? CKModifySubscriptionsOperation {
      op.modifySubscriptionsResultBlock?(.success(()))
    } else if let op = operation as? CKFetchRecordZoneChangesOperation {
      let projected = CommittedCloudRecord(
        committing: committed, desiredKeys: op.configurationsByRecordZoneID?[zone]?.desiredKeys)
      op.recordWasChangedBlock?(id, .success(projected))
      op.fetchRecordZoneChangesResultBlock?(.failure(CKError(.networkUnavailable)))
    } else if let op = operation as? CKFetchRecordsOperation {
      assets.value.withLock { $0 += 1 }
      op.fetchRecordsResultBlock?(.failure(CKError(.networkUnavailable)))
    } else if let op = operation as? CKModifyRecordsOperation {
      uploads.value.withLock { $0 += 1 }
      op.modifyRecordsResultBlock?(.failure(CKError(.networkUnavailable)))
    }
  }
  _ = await sync.sync(.full, explicit: true)
  let entry = try #require(library.entries().first)
  #expect(entry.id == id.recordName)
  #expect(!entry.downloaded && !entry.dirty && entry.problem == nil)
  #expect(!FileManager.default.fileExists(atPath: entry.path))
  #expect(try library.authority(id: entry.id) == relay)
  #expect(try library.catalogStamp(id: entry.id)?.revision == 2)
  #expect(assets.value.withLock { $0 } == 0)
  #expect(uploads.value.withLock { $0 } == 0)
  await sync.stop()
}

// A server response has an immutable change tag. Use the public property override;
// no private CloudKit archives or production acceptance exceptions are involved.
private final class CommittedCloudRecord: CKRecord, @unchecked Sendable {
  private var fixtureTag = UUID().uuidString
  override var recordChangeTag: String? { fixtureTag }
  convenience init(committing record: CKRecord, desiredKeys: [String]? = nil) {
    self.init(recordType: record.recordType, recordID: record.recordID)
    fixtureTag = record.recordChangeTag ?? fixtureTag
    for key in desiredKeys ?? record.allKeys() { self[key] = record[key] }
  }
}

@Test(arguments: [false, true])
func startingTheSameReservedRoomAdoptsAnotherDevicesCommittedNonce(alreadyLive: Bool) async throws {
  let folder = try Fixtures.folder()
  defer { try? FileManager.default.removeItem(at: folder) }
  let template = try Fixtures.template(stage: Fixtures.stage())
  defer { try? FileManager.default.removeItem(at: template.deletingLastPathComponent()) }
  let library = try NativeLibrary.open(root: folder.path, account: "fixture")
  let entry = try library.create(template: template.path, title: "Resume sharing")
  let retained = try library.stage(id: entry.id)
  let retainedPath = try #require(retained.assetPath)
  defer { try? library.releaseUpload(path: retainedPath) }
  let firstDevice = try DocumentOwner(url: URL(fileURLWithPath: retainedPath))
  _ = try await firstDevice.apply(
    batch: #"{"intents":[{"type":"set","path":["title"],"value":"Private first-device edit"}]}"#)
  let retainedVersion = try #require(try Fixtures.object(await firstDevice.state())["version"] as? String)
  try await firstDevice.close()
  let secondDevice = try DocumentOwner(url: URL(fileURLWithPath: entry.path))
  _ = try await secondDevice.apply(batch: #"{"intents":[{"type":"increment","path":["hits"],"by":3}]}"#)
  try await secondDevice.close()
  let zone = CKRecordZone.ID(zoneName: "Library")
  let record = CKRecord(recordType: "SlopDocument", recordID: CKRecord.ID(recordName: entry.id, zoneID: zone))
  let committed: CatalogAuthority =
    alreadyLive
    ? .relay(
      transition: String(repeating: "a", count: 32), room: "reserved-room", seedHash: String(repeating: "b", count: 64))
    : .preparing(
      transition: String(repeating: "a", count: 32), room: "reserved-room", seedHash: nil, priorCheckpoint: nil)
  record["format"] = 2
  record["catalogRevision"] = Int64(7)
  record["title"] = entry.title
  record["template"] = entry.template
  record["authority"] = try libraryEncodeAuthority(authority: committed)
  if !alreadyLive {
    record["payload"] = CKAsset(fileURL: URL(fileURLWithPath: retainedPath))
    record["snapshotVersion"] = retainedVersion
  }
  let response = CommittedCloudRecord(committing: record)
  let sync = LibrarySync(library: library, zone: zone) { operation in
    if let op = operation as? CKModifyRecordZonesOperation {
      op.modifyRecordZonesResultBlock?(.success(()))
    } else if let op = operation as? CKModifySubscriptionsOperation {
      op.modifySubscriptionsResultBlock?(.success(()))
    } else if let op = operation as? CKFetchRecordsOperation {
      op.perRecordResultBlock?(record.recordID, .success(response))
      op.fetchRecordsResultBlock?(.success(()))
    }
  }
  let accepted = try await sync.beginRelay(
    id: entry.id, room: "reserved-room",
    transition: String(repeating: "c", count: 32), seedHash: nil)
  #expect(accepted == committed)
  #expect(try library.authority(id: entry.id) == committed)
  #expect(try library.pendingAuthority(id: entry.id) == nil)
  let reopened = try DocumentOwner(url: URL(fileURLWithPath: entry.path))
  let value = try #require(try Fixtures.object(await reopened.state())["value"] as? [String: Any])
  #expect(value["hits"] as? Int == 3)
  if !alreadyLive { #expect(value["title"] as? String == "Private first-device edit") }
  try await reopened.close()
  do {
    _ = try await sync.beginRelay(
      id: entry.id, room: "different-room",
      transition: String(repeating: "d", count: 32), seedHash: nil)
    Issue.record("A different room must not take over the committed sharing transition")
  } catch {}
  #expect(try library.authority(id: entry.id) == committed)
  await sync.stop()
}
