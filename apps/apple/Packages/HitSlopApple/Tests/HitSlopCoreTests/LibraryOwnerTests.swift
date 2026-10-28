@preconcurrency import CloudKit
import Foundation
import HitSlopCoreBinding
import HitSlopTestSupport
import Synchronization
import Testing

@testable import HitSlopCore
@testable import HitSlopDocument

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
      fields: Data(), deleted: false, artworkVersion: nil, assetRevision: nil)
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
    fields: Data(), deleted: false)
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
        op.perRecordSaveBlock?(record.recordID, .success(record))
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
  record["title"] = "Future slop"
  record["template"] = "fixture"
  record["snapshotVersion"] = "future"
  // A tombstone reaches download's final metadata guard without fetching an asset.
  record["deleted"] = path == "download" ? 1 : 0
  if path == "retry" {
    try library.retry(
      retry: LibraryRetry(
        id: id.recordName, title: "Future slop", template: "fixture", message: "Previous issue",
        retryAt: 0, incoming: true))
  }
  let sync = LibrarySync(library: library, zone: zone) { operation in
    if let op = operation as? CKModifyRecordZonesOperation {
      op.modifyRecordZonesResultBlock?(.success(()))
    } else if let op = operation as? CKModifySubscriptionsOperation {
      op.modifySubscriptionsResultBlock?(.success(()))
    } else if let op = operation as? CKFetchRecordZoneChangesOperation {
      if path == "fetch" { op.recordWasChangedBlock?(id, .success(record)) }
      op.fetchRecordZoneChangesResultBlock?(.failure(CKError(.networkUnavailable)))
    } else if let op = operation as? CKFetchRecordsOperation {
      op.perRecordResultBlock?(id, .success(record))
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
    #expect(result.message == "Update hitSlop to sync this slop.")
    #expect(result.issues[id.recordName] == "Update hitSlop to sync this slop.")
    let reopened = try NativeLibrary.open(root: folder.path, account: "fixture")
    #expect(try reopened.retries().first?.message == "Update hitSlop to sync this slop.")
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
