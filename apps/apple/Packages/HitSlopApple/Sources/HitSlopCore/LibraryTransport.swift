@preconcurrency import CloudKit
import CryptoKit
import Foundation
import HitSlopCoreBinding

struct CloudFailure: Error, CustomStringConvertible {
  var description: String
  init(_ text: String) { description = text }
}
struct LibraryTransportState: Codable, Sendable {
  let sent: Int, fetched: Int, conflicts: Int, explicitFetches: Int
  let failure: String
  let paused: Bool
}

/// CloudKit transports complete files; Rust owns replication and the live owner.
actor LibraryTransport: CKSyncEngineDelegate {
  let library: NativeLibrary
  let database: CKDatabase
  let zone: CKRecordZone.ID
  let account: String
  private var engine: CKSyncEngine?
  private var failure: String?
  private var issues: [String: String] = [:]
  private var uploads: [CKRecord.ID: String] = [:]
  private var checkpointSafe = true
  private var pendingCheckpoint: Data?
  private var syncing = false
  private var explicitCheckpoint = false
  private var observed: [String: (String, Date)] = [:]
  private var incoming: Set<String> = []
  private var retryAt: [String: Date] = [:]
  private var blockedVersions: [String: String] = [:]
  private var sent = 0, fetched = 0, conflicts = 0, explicitFetches = 0
  init(library: NativeLibrary, database: CKDatabase, zone: CKRecordZone.ID, account: String) {
    self.library = library
    self.database = database
    self.zone = zone
    self.account = account
  }
  func start(automatic: Bool = true) async throws {
    failure = nil
    _ = try await database.save(CKRecordZone(zoneID: zone))
    let saved = try library.engineState().map {
      try JSONDecoder().decode(CKSyncEngine.State.Serialization.self, from: $0)
    }
    var config = CKSyncEngine.Configuration(database: database, stateSerialization: saved, delegate: self)
    config.automaticallySync = automatic
    // The development container is also used by the preserved feasibility apps.
    // Never discover and reuse another app's database subscription.
    config.subscriptionID = (Bundle.main.bundleIdentifier ?? "com.hitslop") + ".Library"
    engine = CKSyncEngine(config)
    for entry in try library.entries() where entry.problem != nil {
      incoming.insert(entry.id)
      issues[entry.id] = entry.problem
    }
    try enqueue()
  }
  func stop() async {
    let old = engine
    engine = nil
    if let old { await old.cancelOperations() }
  }
  func enqueue() throws {
    guard let engine else { return }
    let entries = try library.entries()
    let eligible = try entries.filter { entry in
      guard entry.dirty, issues[entry.id] == nil else { return false }
      return try library.stageAllowed(id: entry.id, explicit: explicitCheckpoint)
    }
    let excluded = Set(entries.map(\.id)).subtracting(eligible.map(\.id))
    engine.state.remove(
      pendingRecordZoneChanges: excluded.map { .saveRecord(CKRecord.ID(recordName: $0, zoneID: zone)) })
    let dirty = eligible.map {
      CKSyncEngine.PendingRecordZoneChange.saveRecord(CKRecord.ID(recordName: $0.id, zoneID: zone))
    }
    engine.state.add(pendingRecordZoneChanges: dirty)
  }
  func sync(explicit: Bool = true) async throws {
    guard !syncing else { return }
    guard let engine else { throw CloudFailure("Not syncing") }
    syncing = true
    defer { syncing = false }
    explicitFetches += 1
    for id in Array(incoming) {
      do { try await accept(try await database.record(for: CKRecord.ID(recordName: id, zoneID: zone))) } catch {
        issues[id] = libraryMessage(error)
      }
    }
    try await engine.fetchChanges()
    // Discover zones first, then force a refresh of this zone even when no
    // notification has marked it dirty in the engine's saved state.
    try await engine.fetchChanges(.init(scope: .zoneIDs([zone])))
    try await sendOnly(explicit: explicit)
  }
  private func hasEligibleChanges() throws -> Bool {
    for entry in try library.entries() where entry.dirty && issues[entry.id] == nil {
      if try library.stageAllowed(id: entry.id, explicit: explicitCheckpoint) { return true }
    }
    return false
  }
  func sendOnly(explicit: Bool = false) async throws {
    explicitCheckpoint = explicit
    defer { explicitCheckpoint = false }
    guard let engine else { throw CloudFailure("Sync paused") }
    for _ in 0..<4 {
      try check()
      try enqueue()
      guard try hasEligibleChanges() else { return }
      do { try await engine.sendChanges() } catch {
        try check()
        // A handled conflict still surfaces as an aggregate partial failure.
        guard let ck = error as? CKError, ck.code == .partialFailure,
          let failures = ck.partialErrorsByItemID, !failures.isEmpty,
          failures.values.allSatisfy({ ($0 as? CKError)?.code == .serverRecordChanged })
        else { throw error }
      }
    }
    try check()
    guard try !hasEligibleChanges() else {
      throw CloudFailure("Still waiting to upload")
    }
  }
  func stats() -> LibraryTransportState {
    LibraryTransportState(
      sent: sent, fetched: fetched, conflicts: conflicts,
      explicitFetches: explicitFetches, failure: failure ?? "", paused: engine == nil)
  }
  func snapshot() throws -> ([LibraryEntry], [String: String]) {
    (try library.entries(), issues)
  }
  func create(template: String, title: String) throws -> LibraryEntry {
    try library.create(template: template, title: title)
  }
  func delete(id: String) throws { try library.delete(id: id) }
  func tick() async throws {
    let entries = try library.entries()
    for entry in entries where issues[entry.id] != nil {
      if (retryAt[entry.id].map { $0 <= Date() } ?? false)
        || (blockedVersions[entry.id].map { $0 != entry.assetRevision } ?? false)
      {
        if incoming.contains(entry.id) {
          do { try await accept(try await database.record(for: CKRecord.ID(recordName: entry.id, zoneID: zone))) } catch
          { retryAt[entry.id] = Date().addingTimeInterval(5) }
        } else {
          issues.removeValue(forKey: entry.id)
          retryAt.removeValue(forKey: entry.id)
        }
      }
    }
    var ready = false
    for entry in entries where entry.dirty && issues[entry.id] == nil {
      let version = entry.deleted ? "deleted" : entry.assetRevision
      if observed[entry.id]?.0 != version {
        observed[entry.id] = (version, Date())
      } else if let at = observed[entry.id]?.1, Date().timeIntervalSince(at) >= 1 {
        ready = true
      }
    }
    if ready && !syncing {
      syncing = true
      defer { syncing = false }
      try await sendOnly()
    }
    try library.cleanup()
  }
  private func check() throws { if let failure { throw CloudFailure(failure) } }
  private func fields(_ record: CKRecord) -> Data {
    let coder = NSKeyedArchiver(requiringSecureCoding: true)
    record.encodeSystemFields(with: coder)
    coder.finishEncoding()
    return coder.encodedData
  }
  private func accept(_ input: CKRecord) async throws {
    guard input.recordID.zoneID == zone, input.recordType == "SlopDocument" else { return }
    var record = input
    if (record["deleted"] as? Int) != 1 && (record["payload"] as? CKAsset)?.fileURL == nil {
      record = try await database.record(for: input.recordID)
    }
    let deleted = (record["deleted"] as? Int) == 1
    guard let version = record["snapshotVersion"] as? String,
      let title = record["title"] as? String,
      let template = record["template"] as? String,
      (record["format"] as? Int) == 1
    else { throw CloudFailure("This synced slop needs a newer hitSlop.") }
    let path = (record["payload"] as? CKAsset)?.fileURL?.path
    try library.receive(
      id: record.recordID.recordName, title: title, template: template,
      asset: path, version: version, fields: fields(record), deleted: deleted,
      artworkVersion: record["artworkVersion"] as? String, assetRevision: record["assetRevision"] as? String)
    issues.removeValue(forKey: record.recordID.recordName)
    incoming.remove(record.recordID.recordName)
    retryAt.removeValue(forKey: record.recordID.recordName)
  }

  func handleEvent(_ event: CKSyncEngine.Event, syncEngine: CKSyncEngine) async {
    guard failure == nil, syncEngine === engine else { return }
    do {
      switch event {
      case .stateUpdate(let update):
        let bytes = try JSONEncoder().encode(update.stateSerialization)
        if checkpointSafe { try library.saveEngineState(bytes: bytes) } else { pendingCheckpoint = bytes }
      case .fetchedRecordZoneChanges(let changes):
        // An asset fetch can suspend this actor. Keep a concurrent engine callback
        // from advancing its durable cursor past work Rust has not accepted yet.
        let wasSafe = checkpointSafe
        checkpointSafe = false
        var acceptedAll = true
        for item in changes.modifications where item.record.recordID.zoneID == zone {
          do {
            try await accept(item.record)
            fetched += 1
          } catch {
            acceptedAll = false
            let id = item.record.recordID.recordName
            issues[id] = libraryMessage(error)
            incoming.insert(id)
            checkpointSafe = false
            if case LibraryError.Failed(.busy, _) = error {
              retryAt[id] = Date().addingTimeInterval(5)
            } else {
              try library.block(
                id: id, title: item.record["title"] as? String ?? "Slop",
                template: item.record["template"] as? String ?? "", message: libraryMessage(error))
            }
          }
        }
        if changes.deletions.contains(where: { $0.recordID.zoneID == zone }) {
          throw CloudFailure("This slop was removed from iCloud unexpectedly. Your local copy is retained.")
        }
        if acceptedAll && wasSafe {
          checkpointSafe = true
          if let pendingCheckpoint {
            try library.saveEngineState(bytes: pendingCheckpoint)
            self.pendingCheckpoint = nil
          }
        }
        try enqueue()
      case .sentRecordZoneChanges(let changes):
        for record in changes.savedRecords where record.recordID.zoneID == zone {
          guard let version = record["snapshotVersion"] as? String else {
            throw CloudFailure("Missing uploaded version")
          }
          try library.acknowledge(
            id: record.recordID.recordName, version: version, assetRevision: record["assetRevision"] as? String,
            fields: fields(record),
            deleted: (record["deleted"] as? Int) == 1)
          if let path = uploads.removeValue(forKey: record.recordID) { try? library.releaseUpload(path: path) }
          sent += 1
        }
        for item in changes.failedRecordSaves where item.record.recordID.zoneID == zone {
          switch item.error.code {
          case .serverRecordChanged:
            // Preserve the server change tag, merge its asset, then retry the union.
            do {
              try await accept(try await database.record(for: item.record.recordID))
              conflicts += 1
              syncEngine.state.add(pendingRecordZoneChanges: [.saveRecord(item.record.recordID)])
            } catch {
              issues[item.record.recordID.recordName] = libraryMessage(error)
              checkpointSafe = false
              syncEngine.state.remove(pendingRecordZoneChanges: [.saveRecord(item.record.recordID)])
            }
          case .networkFailure, .networkUnavailable, .serviceUnavailable, .requestRateLimited, .zoneBusy:
            break
          default:
            issues[item.record.recordID.recordName] = item.error.localizedDescription
            syncEngine.state.remove(pendingRecordZoneChanges: [.saveRecord(item.record.recordID)])
          }
        }
        try enqueue()
      case .accountChange(let change):
        if case .signIn(let user) = change.changeType {
          let fingerprint = SHA256.hash(data: Data(user.recordName.utf8)).map { String(format: "%02x", $0) }.joined()
          guard fingerprint == account else { throw CloudFailure("Different iCloud account; local files retained") }
        } else {
          throw CloudFailure("iCloud account changed; this account's local files are retained")
        }
      case .didFetchRecordZoneChanges(let result):
        if let error = result.error, !transient(error) { throw error }
      default: break
      }
    } catch { if !transient(error) { failure = String(describing: error) } }
  }
  private func transient(_ error: Error) -> Bool {
    guard let error = error as? CKError else { return false }
    return [
      .networkFailure, .networkUnavailable, .serviceUnavailable, .requestRateLimited, .zoneBusy, .operationCancelled,
    ].contains(error.code)
  }
  func nextRecordZoneChangeBatch(_ context: CKSyncEngine.SendChangesContext, syncEngine: CKSyncEngine) async
    -> CKSyncEngine.RecordZoneChangeBatch?
  {
    guard failure == nil, syncEngine === engine else { return nil }
    do {
      let entries = try library.entries().filter { $0.dirty && issues[$0.id] == nil }
      var records: [CKRecord.ID: CKRecord] = [:]
      for entry in entries {
        let id = CKRecord.ID(recordName: entry.id, zoneID: zone)
        let change = CKSyncEngine.PendingRecordZoneChange.saveRecord(id)
        guard context.options.scope.contains(change) else { continue }
        let upload: LibraryUpload
        do {
          guard let staged = try library.stageSync(id: entry.id, explicit: explicitCheckpoint) else {
            syncEngine.state.remove(pendingRecordZoneChanges: [.saveRecord(id)])
            continue
          }
          upload = staged
        } catch {
          issues[entry.id] = libraryMessage(error)
          blockedVersions[entry.id] = entry.assetRevision
          if case LibraryError.Failed(.busy, _) = error { retryAt[entry.id] = Date().addingTimeInterval(5) }
          syncEngine.state.remove(pendingRecordZoneChanges: [.saveRecord(id)])
          continue
        }
        let record: CKRecord
        if let fields = upload.fields {
          let decoder = try NSKeyedUnarchiver(forReadingFrom: fields)
          decoder.requiresSecureCoding = true
          guard let restored = CKRecord(coder: decoder) else { throw CloudFailure("Invalid server record metadata") }
          decoder.finishDecoding()
          record = restored
        } else {
          record = CKRecord(recordType: "SlopDocument", recordID: id)
        }
        record["payload"] = upload.assetPath.map { CKAsset(fileURL: URL(fileURLWithPath: $0)) }
        record["deleted"] = upload.entry.deleted ? 1 : 0
        if let path = upload.assetPath { uploads[id] = path }
        record["title"] = upload.entry.title
        record["template"] = upload.entry.template
        record["snapshotVersion"] = upload.entry.version
        record["assetRevision"] = upload.entry.assetRevision
        record["artworkVersion"] = upload.entry.artworkVersion
        record["format"] = 1
        records[id] = record
      }
      let changes = records.keys.map { CKSyncEngine.PendingRecordZoneChange.saveRecord($0) }
      let immutable = records
      return await CKSyncEngine.RecordZoneChangeBatch(pendingChanges: changes) { immutable[$0] }
    } catch {
      failure = String(describing: error)
      return nil
    }
  }
}
