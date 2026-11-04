@preconcurrency import CloudKit
import Foundation
import PopCoreBinding
import Synchronization
import os

struct CloudFailure: Error, CustomStringConvertible {
  var description: String
  init(_ text: String) { description = text }
}

/// CloudKit owns the catalog and snapshot authority; a live relay exclusively owns document bytes.
actor LibrarySync {
  enum Request: String, Sendable { case full, changes }
  struct Outcome: Sendable {
    var lastSuccess: Date?
    var lastError: String?
    var message: String?
    var diagnosticCodes: [String] = []
    var pending = 0
    var issues: [String: String] = [:]
  }
  private struct Options {
    var request: Request
    var explicit: Bool
    var background: Bool
    mutating func include(_ other: Options) {
      if other.request == .full { request = .full }
      explicit = explicit || other.explicit
      background = background && other.background
    }
  }
  private struct Page {
    var records: [CKRecord.ID: Result<CKRecord, Error>] = [:]
    var deleted: [CKRecord.ID] = []
    var token: CKServerChangeToken?
    var more = false
    var error: Error?
  }
  let library: NativeLibrary
  private let submit: @Sendable (CKDatabaseOperation) -> Void
  private let zone: CKRecordZone.ID
  private let logger = Logger(subsystem: "click.dotpop.library", category: "sync")
  private var running: Task<Outcome, Never>?
  private var queued: Options?
  private var operation: CKOperation?
  private var stopped = false
  private var zoneSaved = false
  private var subscriptionSaved = false
  private var outcome = Outcome()
  private var observed: [String: String] = [:]
  private var changedAt: Date?
  private var retryRunAt: Date?
  /// Consecutive failures per record this session; retries back off from 5 s to 15 min.
  private var failures: [String: Int] = [:]
  private var transitions: Set<String> = []
  /// Consecutive failed runs; the next automatic run backs off the same way.
  private var runFailures = 0
  /// Whether a caller is waiting for a rerun.
  var hasQueuedRequest: Bool { queued != nil }

  init(library: NativeLibrary, database: CKDatabase, zone: CKRecordZone.ID) {
    self.library = library
    self.submit = { database.add($0) }
    self.zone = zone
  }
  init(library: NativeLibrary, zone: CKRecordZone.ID, submit: @escaping @Sendable (CKDatabaseOperation) -> Void) {
    self.library = library
    self.zone = zone
    self.submit = submit
  }

  func sync(_ request: Request, explicit: Bool = false, background: Bool = false) async -> Outcome {
    guard explicit || retryRunAt.map({ $0 <= Date() }) != false else { return outcome }
    let options = Options(request: request, explicit: explicit, background: background)
    if let running {
      if queued == nil { queued = options } else { queued?.include(options) }
      return await running.value
    }
    let work = Task { await self.drain(options) }
    running = work
    return await work.value
  }

  private func drain(_ first: Options) async -> Outcome {
    var options = first
    repeat {
      if options.explicit || retryRunAt.map({ $0 <= Date() }) != false {
        await run(options)
      }
      guard let next = queued, !stopped else { break }
      queued = nil
      options = next
    } while true
    queued = nil
    running = nil
    return outcome
  }

  func stop() async {
    stopped = true
    running?.cancel()
    operation?.cancel()
    _ = await running?.value
  }

  func create(template: String, title: String) throws -> LibraryEntry {
    try library.create(template: template, title: title)
  }
  func adopt(source: String, title: String) throws -> LibraryEntry {
    try library.adopt(source: source, title: title)
  }
  func delete(id: String) throws { try library.delete(id: id) }
  func removeDownload(id: String) async throws {
    if let running { _ = await running.value }
    try check()
    try library.removeDownload(id: id)
  }
  func refreshRecord(id: String) async throws -> CatalogAuthority {
    if let running { _ = await running.value }
    let options = Options(request: .full, explicit: true, background: false)
    try await prepare(options)
    let record = try await fetchRecord(CKRecord.ID(recordName: id, zoneID: zone), options)
    try await accept(record, options)
    return try library.authority(id: id)
  }
  func download(id: String) async throws {
    if let running { _ = await running.value }
    try check()
    let options = Options(request: .full, explicit: true, background: false)
    let record = try await fetchRecord(CKRecord.ID(recordName: id, zoneID: zone), options)
    try await accept(record, options, downloading: true)
    guard let entry = try library.entries().first(where: { $0.id == id }), !entry.deleted, entry.downloaded else {
      throw CloudFailure("This pop is no longer available in iCloud.")
    }
  }

  func snapshot() throws -> ([LibraryEntry], Outcome, Bool) {
    var state = outcome
    let entries = try library.entries()
    state.pending = entries.filter(\.dirty).count
    state.issues = Dictionary(uniqueKeysWithValues: try library.retries().map { ($0.id, $0.message) })
    for entry in entries { if let problem = entry.problem { state.issues[entry.id] = problem } }
    return (entries, state, running != nil)
  }

  /// Polling only observes disk; network work never holds up catalog refreshes.
  func changesDue() throws -> Bool {
    let entries = try library.entries()
    let dirty = Dictionary(
      uniqueKeysWithValues: entries.filter(\.dirty).map {
        ($0.id, $0.deleted ? "deleted" : $0.assetRevision)
      })
    if dirty != observed {
      observed = dirty
      changedAt = Date()
    }
    guard running == nil, !stopped else { return false }
    if let retryRunAt { return retryRunAt <= Date() }
    if try library.retries().contains(where: { $0.retryAt <= Date().timeIntervalSince1970 }) { return true }
    let blocked = Set(
      try library.retries().filter { $0.incoming || $0.retryAt > Date().timeIntervalSince1970 }.map(\.id))
    guard changedAt.map({ Date().timeIntervalSince($0) >= 1 }) == true else { return false }
    for entry in entries where entry.dirty && !blocked.contains(entry.id) {
      do {
        if try library.stageAllowed(id: entry.id, explicit: false) { return true }
      } catch {
        block(CKRecord.ID(recordName: entry.id, zoneID: zone), error, incoming: false)
      }
    }
    return false
  }

  private func run(_ options: Options) async {
    logger.info("run start request=\(options.request.rawValue, privacy: .public) background=\(options.background)")
    outcome.diagnosticCodes = []
    outcome.lastError = nil
    outcome.message = nil
    retryRunAt = nil
    do {
      try check()
      try await prepare(options)
      try await retryIncoming(options)
      do { try await fetch(options) } catch { failed(error) }
      try check()
      do { try await send(options) } catch { failed(error) }
      try library.cleanup()
      let (_, state, _) = try snapshot()
      outcome.pending = state.pending
      outcome.issues = state.issues
      if let issue = state.issues.values.sorted().first { outcome.lastError = outcome.lastError ?? issue }
      if outcome.lastError == nil { outcome.lastSuccess = Date() }
      if retryRunAt == nil { runFailures = 0 }
    } catch {
      failed(error)
    }
    if let (_, state, _) = try? snapshot() {
      outcome.pending = state.pending
      outcome.issues = state.issues
    }
    logger.info(
      "run end pending=\(self.outcome.pending) issues=\(self.outcome.issues.count) success=\(self.outcome.lastError == nil)"
    )
  }

  private func failed(_ error: Error) {
    recordDiagnostic(error)
    outcome.lastError = outcome.lastError.map { $0 + "\n" + String(describing: error) } ?? String(describing: error)
    outcome.message = outcome.message ?? message(error)
    if retryRunAt == nil { runFailures += 1 }
    let backoff = min(5 * pow(2, Double(min(runFailures, 10) - 1)), 900)
    retryRunAt = Date().addingTimeInterval(max(backoff, (error as? CKError)?.retryAfterSeconds ?? 0))
    let underlying = (error as NSError).userInfo[NSUnderlyingErrorKey] as? NSError
    logger.error(
      "run error code=\((error as NSError).code) timeout=\(underlying?.code == NSURLErrorTimedOut) retry scheduled")
  }

  private func check() throws {
    try Task.checkCancellation()
    if stopped { throw CloudFailure("iCloud account changed; local files retained") }
  }

  /// Operations that carry a document file get time to move up to 16 MiB on a slow network.
  private func configure(_ operation: CKOperation, _ options: Options, payload: Bool = false) {
    operation.configuration.timeoutIntervalForRequest = 30
    operation.configuration.timeoutIntervalForResource = payload ? 600 : 60
    operation.qualityOfService = options.background ? .utility : .userInitiated
    self.operation = operation
    logger.debug("operation start timeout=30 background=\(options.background)")
  }

  private func prepare(_ options: Options) async throws {
    try check()
    if !zoneSaved {
      let op = CKModifyRecordZonesOperation(recordZonesToSave: [CKRecordZone(zoneID: zone)], recordZoneIDsToDelete: nil)
      configure(op, options)
      try await withCheckedThrowingContinuation { (reply: CheckedContinuation<Void, Error>) in
        op.modifyRecordZonesResultBlock = { reply.resume(with: $0) }
        submit(op)
      }
      zoneSaved = true
    }
    try check()
    if !subscriptionSaved {
      let subscription = CKRecordZoneSubscription(zoneID: zone, subscriptionID: "com.hitslop.library." + zone.zoneName)
      let notification = CKSubscription.NotificationInfo()
      notification.shouldSendContentAvailable = true
      subscription.notificationInfo = notification
      let op = CKModifySubscriptionsOperation(subscriptionsToSave: [subscription], subscriptionIDsToDelete: nil)
      configure(op, options)
      try await withCheckedThrowingContinuation { (reply: CheckedContinuation<Void, Error>) in
        op.modifySubscriptionsResultBlock = { reply.resume(with: $0) }
        submit(op)
      }
      subscriptionSaved = true
    }
  }

  private func page(_ token: CKServerChangeToken?, _ options: Options) async throws -> Page {
    try check()
    let config = CKFetchRecordZoneChangesOperation.ZoneConfiguration()
    config.previousServerChangeToken = token
    // Fetch metadata first so an evicted document stays off this device during sync.
    config.desiredKeys = [
      "deleted", "title", "template", "snapshotVersion", "assetRevision", "artworkVersion", "format",
      "authority", "catalogRevision",
    ]
    let op = CKFetchRecordZoneChangesOperation(recordZoneIDs: [zone], configurationsByRecordZoneID: [zone: config])
    op.fetchAllChanges = false
    configure(op, options, payload: options.request == .changes)
    let result = Mutex(Page())
    return await withCheckedContinuation { reply in
      op.recordWasChangedBlock = { id, record in result.withLock { $0.records[id] = record } }
      op.recordWithIDWasDeletedBlock = { id, _ in result.withLock { $0.deleted.append(id) } }
      op.recordZoneFetchResultBlock = { _, zoneResult in
        result.withLock { page in
          switch zoneResult {
          case .success(let value):
            page.token = value.serverChangeToken
            page.more = value.moreComing
          case .failure(let error): page.error = error
          }
        }
      }
      op.fetchRecordZoneChangesResultBlock = { completion in
        reply.resume(
          returning: result.withLock { page in
            if case .failure(let error) = completion { page.error = page.error ?? error }
            return page
          })
      }
      submit(op)
    }
  }

  private func fetchRecord(_ id: CKRecord.ID, _ options: Options) async throws -> CKRecord {
    try check()
    let op = CKFetchRecordsOperation(recordIDs: [id])
    configure(op, options, payload: true)
    let result = Mutex<Result<CKRecord, Error>?>(nil)
    return try await withCheckedThrowingContinuation { reply in
      op.perRecordResultBlock = { _, record in result.withLock { $0 = record } }
      op.fetchRecordsResultBlock = { completion in
        let record = result.withLock { $0 }
        switch completion {
        case .success: reply.resume(with: record ?? .failure(CloudFailure("Missing iCloud record")))
        case .failure(let error): reply.resume(with: record ?? .failure(error))
        }
      }
      submit(op)
    }
  }

  private func fetch(_ options: Options) async throws {
    var token: CKServerChangeToken?
    if options.request == .changes, let bytes = try library.changeToken() {
      token = try? NSKeyedUnarchiver.unarchivedObject(ofClass: CKServerChangeToken.self, from: bytes)
    }
    var safe = true
    var count = 0
    var payloads = 0
    while true {
      let batch = try await page(token, options)
      for (id, result) in batch.records {
        do {
          let record = try result.get()
          if record.recordType != "SlopDocument" { continue }
          let confirmed = try library.confirmedRevision(id: id.recordName)
          let authority = try decodeAuthority(record)
          let stamp = try catalogStamp(record)
          guard record["title"] is String, record["template"] is String,
            !authority.isSnapshot || record["snapshotVersion"] is String
          else { throw LibraryError.Failed(issue: .requiresUpdate, message: "This synced pop needs a newer pop.") }
          let sameAuthority = try library.authority(id: id.recordName) == authority
          let sameStamp = try library.catalogStamp(id: id.recordName) == stamp
          let retrying = try library.retries().contains { $0.id == id.recordName && $0.incoming }
          let unchanged =
            options.request == .full && confirmed?.count == 2
            && sameAuthority && sameStamp
            && confirmed?[0] == record["snapshotVersion"] as? String
            && confirmed?[1] == record["assetRevision"] as? String
            && (record["deleted"] as? Int) != 1
            && !retrying
          if !unchanged {
            try await accept(record, options)
            payloads += 1
          }
          if unchanged, try !library.entries().contains(where: { $0.id == id.recordName && $0.dirty }) {
            try library.clearRetry(id: id.recordName)
          }
          count += 1
        } catch {
          if !block(id, error, record: try? result.get(), incoming: true) { safe = false }
        }
      }
      for id in batch.deleted {
        if try library.prepareDeletionReupload(id: id.recordName) { continue }
        if !block(id, CloudFailure("Removed from iCloud unexpectedly; your local copy is kept"), incoming: true) {
          safe = false
        }
      }
      if let error = batch.error {
        logger.info("token kept fetch error")
        if (error as? CKError)?.code == .changeTokenExpired, token != nil {
          var full = options
          full.request = .full
          try await fetch(full)
          return
        }
        throw error
      }
      token = batch.token
      if !batch.more { break }
    }
    try check()
    if safe, let token {
      try library.saveChangeToken(
        bytes: NSKeyedArchiver.archivedData(withRootObject: token, requiringSecureCoding: true))
      logger.info("token saved records=\(count) payloads=\(payloads)")
    } else {
      logger.info("token kept records=\(count) payloads=\(payloads)")
      throw CloudFailure("Some iCloud records could not be saved; will retry")
    }
  }

  private func recordDiagnostic(_ error: Error) {
    let value = error as NSError
    let code = "\(value.domain) code=\(value.code)"
    if !outcome.diagnosticCodes.contains(code), outcome.diagnosticCodes.count < 20 {
      outcome.diagnosticCodes.append(code)
    }
  }

  private func message(_ error: Error) -> String {
    if let error = error as? CKError,
      [.networkFailure, .networkUnavailable, .serviceUnavailable, .requestRateLimited, .zoneBusy, .operationCancelled]
        .contains(error.code)
    {
      return "Couldn't reach iCloud · will retry"
    }
    if let error = error as? CKError {
      switch error.code {
      case .notAuthenticated: return "Sign in to iCloud to sync your pop."
      case .quotaExceeded: return "Your iCloud storage is full. Free up space, then retry."
      case .unknownItem: return "An iCloud record is missing. Sync needs attention."
      default: return "iCloud couldn't sync some changes. Please retry."
      }
    }
    if case LibraryError.Failed(let issue, _) = error {
      switch issue {
      case .requiresUpdate: return "Update pop to sync this pop."
      case .appMismatch: return "These copies use different data schemas or theme palettes."
      case .busy: return "This pop is busy. Please retry shortly."
      default: break
      }
    }
    return "Some changes couldn't sync. Please retry."
  }

  @discardableResult private func block(
    _ id: CKRecord.ID, _ error: Error, record: CKRecord? = nil, incoming: Bool
  ) -> Bool {
    // A superseded callback cannot rewrite catalog metadata or schedule a retry.
    if case LibraryError.Failed(let issue, _) = error, issue == .staleCatalog { return false }
    recordDiagnostic(error)
    let message = message(error)
    outcome.message = outcome.message ?? message
    outcome.lastError = outcome.lastError ?? String(describing: error)
    let attempt = (failures[id.recordName] ?? 0) + 1
    failures[id.recordName] = attempt
    let backoff = min(5 * pow(2, Double(min(attempt, 10) - 1)), 900)
    let delay = max(backoff, (error as? CKError)?.retryAfterSeconds ?? 0)
    logger.error("record error id=\(id.recordName, privacy: .public) code=\((error as NSError).code) retry=\(delay)")
    do {
      try library.retry(
        retry: LibraryRetry(
          id: id.recordName, title: record?["title"] as? String ?? "Pop",
          template: record?["template"] as? String ?? "", message: message,
          retryAt: Date().addingTimeInterval(delay).timeIntervalSince1970, incoming: incoming))
      if incoming {
        try? library.block(
          id: id.recordName, title: record?["title"] as? String ?? "Pop",
          template: record?["template"] as? String ?? "", message: message)
      }
      return true
    } catch {
      logger.error("retry could not be saved id=\(id.recordName, privacy: .public)")
      return false
    }
  }

  private func retryIncoming(_ options: Options) async throws {
    for retry in try library.retries()
    where retry.incoming && (options.explicit || retry.retryAt <= Date().timeIntervalSince1970) {
      let id = CKRecord.ID(recordName: retry.id, zoneID: zone)
      logger.info("retry incoming id=\(retry.id, privacy: .public)")
      do { try await accept(try await fetchRecord(id, options), options) } catch {
        if (error as? CKError)?.code == .unknownItem,
          try library.prepareDeletionReupload(id: id.recordName)
        {
          continue
        }
        block(id, error, incoming: true)
      }
    }
    // Problems written by earlier preview builds also need a durable retry path.
    let retries = Set(try library.retries().map(\.id))
    for entry in try library.entries() where entry.problem != nil && !retries.contains(entry.id) {
      let id = CKRecord.ID(recordName: entry.id, zoneID: zone)
      do { try await accept(try await fetchRecord(id, options), options) } catch {
        if (error as? CKError)?.code == .unknownItem,
          try library.prepareDeletionReupload(id: id.recordName)
        {
          continue
        }
        block(id, error, incoming: true)
      }
    }
  }

  private func fields(_ record: CKRecord) -> Data {
    let coder = NSKeyedArchiver(requiringSecureCoding: true)
    record.encodeSystemFields(with: coder)
    if let revision = record["catalogRevision"] as? Int64 { coder.encode(revision, forKey: "popCatalogRevision") }
    coder.finishEncoding()
    return coder.encodedData
  }

  private func accept(_ input: CKRecord, _ options: Options, downloading: Bool = false, transitioning: Bool = false)
    async throws
  {
    guard input.recordID.zoneID == zone, input.recordType == "SlopDocument" else { return }
    try check()
    if transitions.contains(input.recordID.recordName) && !transitioning { return }
    let authority = try decodeAuthority(input)
    try library.admitAuthority(id: input.recordID.recordName, authority: authority, stamp: try catalogStamp(input))
    guard let title = input["title"] as? String, let template = input["template"] as? String else {
      throw CloudFailure("Missing catalog metadata")
    }
    if !authority.isSnapshot {
      try library.acceptAuthority(
        id: input.recordID.recordName, title: title, template: template, authority: authority, fields: fields(input),
        stamp: try catalogStamp(input))
      failures[input.recordID.recordName] = nil
      try library.clearRetry(id: input.recordID.recordName)
      return
    }
    if !downloading, (input["deleted"] as? Int) != 1, let version = input["snapshotVersion"] as? String {
      if try library.receiveRemovedMetadata(
        id: input.recordID.recordName, title: title, template: template, version: version, fields: fields(input),
        artworkVersion: input["artworkVersion"] as? String, assetRevision: input["assetRevision"] as? String,
        stamp: try catalogStamp(input))
      {
        try library.acceptAuthority(
          id: input.recordID.recordName, title: title, template: template, authority: authority, fields: fields(input),
          stamp: try catalogStamp(input))
        failures[input.recordID.recordName] = nil
        try library.clearRetry(id: input.recordID.recordName)
        return
      }
    }
    var record = input
    if (record["deleted"] as? Int) != 1 && (record["payload"] as? CKAsset)?.fileURL == nil {
      record = try await fetchRecord(input.recordID, options)
    }
    try check()
    if !(try decodeAuthority(record)).isSnapshot {
      try await accept(record, options, downloading: downloading, transitioning: transitioning)
      return
    }
    guard let version = record["snapshotVersion"] as? String,
      let title = record["title"] as? String,
      let template = record["template"] as? String,
      try decodeAuthority(record).isSnapshot
    else { throw LibraryError.Failed(issue: .requiresUpdate, message: "This synced pop needs a newer pop.") }
    try library.receiveCatalogSnapshot(
      id: record.recordID.recordName, title: title, template: template,
      asset: (record["payload"] as? CKAsset)?.fileURL?.path, version: version, fields: fields(record),
      deleted: (record["deleted"] as? Int) == 1,
      artworkVersion: record["artworkVersion"] as? String, assetRevision: record["assetRevision"] as? String,
      authority: try decodeAuthority(record), stamp: try catalogStamp(record))
    try library.acceptAuthority(
      id: record.recordID.recordName, title: title, template: template, authority: try decodeAuthority(record),
      fields: fields(record), stamp: try catalogStamp(record))
    failures[record.recordID.recordName] = nil
    let dirty = try library.entries().contains { $0.id == record.recordID.recordName && $0.dirty }
    if try library.retries().contains(where: { $0.id == record.recordID.recordName && ($0.incoming || !dirty) }) {
      try library.clearRetry(id: record.recordID.recordName)
    }
  }

  /// The room is reserved once by the creator before this change-tag fence.
  func beginRelay(id: String, room: String, transition: String, seedHash: String?) async throws -> CatalogAuthority {
    guard transitions.insert(id).inserted else { throw CloudFailure("A sharing transition is already running") }
    defer { transitions.remove(id) }
    if let running { _ = await running.value }
    let options = Options(request: .full, explicit: true, background: false)
    try await prepare(options)
    for attempt in 0..<3 {
      let record = try await authorityRecord(id, options)
      if record.recordChangeTag != nil {
        let current = try decodeAuthority(record)
        if current.roomID == room {
          switch current {
          case .preparing:
            return try await resumePreparing(record, room: room, options)
          case .relay:
            try await accept(record, options, transitioning: true)
            return current
          default: break
          }
        }
        guard current.isSnapshot else {
          throw CloudFailure("This catalog record already belongs to another live transition")
        }
        // Reconcile the old CloudKit snapshot through the existing owner before the fence.
        try await accept(record, options, transitioning: true)
      }
      var preparing = CatalogAuthority.preparing(
        transition: transition, room: room, seedHash: seedHash,
        priorCheckpoint: try library.authority(id: id).checkpointProof)
      try library.prepareAuthority(id: id, authority: preparing)
      let upload = try library.stagePreparing(id: id, transition: transition)
      defer { if let path = upload.assetPath { try? library.releaseUpload(path: path) } }
      if var proof = try library.authority(id: id).checkpointProof {
        proof.version = upload.entry.version
        preparing = .preparing(transition: transition, room: room, seedHash: seedHash, priorCheckpoint: proof)
        try library.prepareAuthority(id: id, authority: preparing)
      }
      apply(upload, to: record)
      record["authority"] = try libraryEncodeAuthority(authority: preparing)
      do {
        let saved = try await save(record, options)
        try rememberAuthority(saved, preparing)
        return preparing
      } catch let error as CKError where error.code == .serverRecordChanged && attempt < 2 { continue }
    }
    throw CloudFailure("The catalog changed while preparing sharing. Retry the same room.")
  }

  // Preparing retains private CloudKit bytes until activation. Only this explicit
  // start/resume path reconciles them; background discovery stays metadata-only.
  private func resumePreparing(_ input: CKRecord, room: String, _ options: Options) async throws -> CatalogAuthority {
    var record = input
    if (record["payload"] as? CKAsset)?.fileURL == nil {
      record = try await fetchRecord(record.recordID, options)
    }
    let authority = try decodeAuthority(record)
    guard authority.roomID == room else { throw CloudFailure("The reserved sharing room changed") }
    if case .relay = authority {
      try await accept(record, options, transitioning: true)
      return authority
    }
    guard case .preparing = authority,
      (record["deleted"] as? Int) != 1,
      let path = (record["payload"] as? CKAsset)?.fileURL?.path,
      let version = record["snapshotVersion"] as? String,
      let title = record["title"] as? String, let template = record["template"] as? String
    else { throw CloudFailure("The preparing relay is missing its retained private snapshot") }
    try library.receiveCatalogSnapshot(
      id: record.recordID.recordName, title: title, template: template,
      asset: path, version: version, fields: fields(record), deleted: false,
      artworkVersion: record["artworkVersion"] as? String, assetRevision: record["assetRevision"] as? String,
      authority: authority, stamp: try catalogStamp(record))
    return authority
  }

  func confirmRelay(id: String, room: String, transition: String, seedHash: String) async throws {
    let relay = CatalogAuthority.relay(transition: transition, room: room, seedHash: seedHash)
    try await commitCatalog(id: id, authority: relay)
  }

  func handoffSnapshot(id: String, room: String, transition: String, freeze: String) async throws -> CatalogReceipt {
    guard transitions.insert(id).inserted else { throw CloudFailure("A sharing transition is already running") }
    defer { transitions.remove(id) }
    if let running { _ = await running.value }
    let options = Options(request: .full, explicit: true, background: false)
    try await prepare(options)
    let frozen = CatalogAuthority.handoff(transition: transition, room: room, freeze: freeze)
    for attempt in 0..<3 {
      let record = try await authorityRecord(id, options)
      let current = try decodeAuthority(record)
      let restaging: Bool
      if case .snapshot(let receipt) = current, let receipt, receipt.transition == transition,
        receipt.room == room, receipt.freeze == freeze
      {
        restaging = true
      } else {
        restaging = false
      }
      try await accept(record, options, transitioning: true)
      if current != frozen && !restaging {
        try library.prepareAuthority(id: id, authority: frozen)
        record["format"] = 2
        record["authority"] = try libraryEncodeAuthority(authority: frozen)
        record["payload"] = nil
        do {
          let saved = try await save(record, options)
          try rememberAuthority(saved, frozen)
        } catch let error as CKError where error.code == .serverRecordChanged && attempt < 2 { continue }
      }
      let expected = restaging ? current : frozen
      // Rust accepts the staged file and verifies it covers the frozen relay frontier.
      let upload = try library.stageHandoff(id: id, transition: transition)
      defer { if let path = upload.assetPath { try? library.releaseUpload(path: path) } }
      let receipt = CatalogReceipt(
        transition: transition, room: room, freeze: freeze, snapshotVersion: upload.entry.version)
      let snapshot = CatalogAuthority.snapshot(receipt: receipt)
      try library.prepareAuthority(id: id, authority: snapshot)
      let latest = try await fetchRecord(record.recordID, options)
      guard try decodeAuthority(latest) == expected else {
        try await accept(latest, options, transitioning: true)
        continue
      }
      apply(upload, to: latest)
      latest["authority"] = try libraryEncodeAuthority(authority: snapshot)
      do {
        let saved = try await save(latest, options)
        try library.acknowledge(
          id: id, version: upload.entry.version, assetRevision: upload.entry.assetRevision, fields: fields(saved),
          deleted: false, stamp: try catalogStamp(saved))
        try rememberAuthority(saved, snapshot)
        return receipt
      } catch let error as CKError where error.code == .serverRecordChanged && attempt < 2 { continue }
    }
    throw CloudFailure("The final snapshot is retained locally. Retry stopping sharing.")
  }

  func cancelPreparing(id: String, room: String, transition: String) async throws {
    guard transitions.insert(id).inserted else { throw CloudFailure("A sharing transition is already running") }
    defer { transitions.remove(id) }
    if let running { _ = await running.value }
    let options = Options(request: .full, explicit: true, background: false)
    let record = try await fetchRecord(CKRecord.ID(recordName: id, zoneID: zone), options)
    let current = try decodeAuthority(record)
    guard case .preparing(let actual, let actualRoom, _, let priorCheckpoint) = current, actual == transition,
      actualRoom == room
    else {
      let local = try library.authority(id: id)
      let pending = try library.pendingAuthority(id: id)
      var restored: CatalogAuthority?
      if case .preparing(let nonce, let reserved, _, let prior) = local,
        nonce == transition, reserved == room
      {
        restored = prior.map { .checkpoint(proof: $0) } ?? .snapshot(receipt: nil)
      } else if let pending, pending.isSnapshot {
        restored = pending
      }
      if current == restored {
        try rememberAuthority(record, current)
        return
      }
      throw CloudFailure("The catalog no longer has the canceled preparing room")
    }
    try rememberAuthority(record, current)
    let snapshot = priorCheckpoint.map { CatalogAuthority.checkpoint(proof: $0) } ?? .snapshot(receipt: nil)
    try library.prepareAuthority(id: id, authority: snapshot)
    record["format"] = 2
    record["authority"] = try libraryEncodeAuthority(authority: snapshot)
    let saved = try await save(record, options)
    try rememberAuthority(saved, snapshot)
  }

  func finishHandoff(id: String, receipt: CatalogReceipt) async throws {
    guard transitions.insert(id).inserted else { throw CloudFailure("A sharing transition is already running") }
    defer { transitions.remove(id) }
    if let running { _ = await running.value }
    let options = Options(request: .full, explicit: true, background: false)
    try await prepare(options)
    for attempt in 0..<3 {
      let record = try await fetchRecord(CKRecord.ID(recordName: id, zoneID: zone), options)
      let current = try decodeAuthority(record)
      if let proof = current.checkpointProof, proof.transition == receipt.transition,
        proof.sourceVersion == receipt.snapshotVersion
      {
        try await accept(record, options, transitioning: true)
        return
      }
      guard current == .snapshot(receipt: receipt),
        let version = record["snapshotVersion"] as? String, version == receipt.snapshotVersion
      else {
        throw CloudFailure("The handoff receipt changed")
      }
      try rememberAuthority(record, current)
      let retired = CatalogAuthority.checkpoint(
        proof: CatalogCheckpointProof(
          transition: receipt.transition, sourceVersion: receipt.snapshotVersion, version: version))
      try library.prepareAuthority(id: id, authority: retired)
      record["authority"] = try libraryEncodeAuthority(authority: retired)
      do {
        let saved = try await save(record, options)
        try rememberAuthority(saved, retired)
        return
      } catch let error as CKError where error.code == .serverRecordChanged && attempt < 2 { continue }
    }
    throw CloudFailure("The catalog changed while retiring the handoff receipt")
  }

  private func commitCatalog(id: String, authority: CatalogAuthority) async throws {
    guard transitions.insert(id).inserted else { throw CloudFailure("A sharing transition is already running") }
    defer { transitions.remove(id) }
    if let running { _ = await running.value }
    let options = Options(request: .full, explicit: true, background: false)
    try await prepare(options)
    for attempt in 0..<3 {
      let record = try await authorityRecord(id, options)
      let current = try decodeAuthority(record)
      try await accept(record, options, transitioning: true)
      if current == authority { return }
      try library.prepareAuthority(id: id, authority: authority)
      record["format"] = 2
      record["authority"] = try libraryEncodeAuthority(authority: authority)
      if !authority.isSnapshot { record["payload"] = nil }
      do {
        let saved = try await save(record, options)
        try rememberAuthority(saved, authority)
        return
      } catch let error as CKError where error.code == .serverRecordChanged && attempt < 2 { continue }
    }
    throw CloudFailure("The catalog changed. Retry the same sharing transition.")
  }

  private func authorityRecord(_ id: String, _ options: Options) async throws -> CKRecord {
    let recordID = CKRecord.ID(recordName: id, zoneID: zone)
    do { return try await fetchRecord(recordID, options) } catch let error as CKError where error.code == .unknownItem {
      guard let entry = try library.entries().first(where: { $0.id == id }), !entry.deleted else {
        throw CloudFailure("The library document is unavailable")
      }
      let record = CKRecord(recordType: "SlopDocument", recordID: recordID)
      record["title"] = entry.title
      record["template"] = entry.template
      record["snapshotVersion"] = entry.version
      return record
    }
  }
  private func apply(_ upload: LibraryUpload, to record: CKRecord) {
    record["payload"] = upload.assetPath.map { CKAsset(fileURL: URL(fileURLWithPath: $0)) }
    record["deleted"] = upload.entry.deleted ? 1 : 0
    record["title"] = upload.entry.title
    record["template"] = upload.entry.template
    record["snapshotVersion"] = upload.entry.version
    record["assetRevision"] = upload.entry.assetRevision
    record["artworkVersion"] = upload.entry.artworkVersion
    record["format"] = 2
  }
  private func rememberAuthority(_ record: CKRecord, _ authority: CatalogAuthority) throws {
    guard let title = record["title"] as? String, let template = record["template"] as? String else {
      throw CloudFailure("Missing catalog metadata")
    }
    try library.acceptAuthority(
      id: record.recordID.recordName, title: title, template: template, authority: authority, fields: fields(record),
      stamp: try catalogStamp(record))
  }

  private func decodeAuthority(_ record: CKRecord) throws -> CatalogAuthority {
    let format = record["format"] as? Int ?? 0
    guard format >= 0, format <= Int(UInt32.max) else { throw CloudFailure("Invalid catalog format") }
    return try libraryDecodeAuthority(format: UInt32(format), json: record["authority"] as? String)
  }

  private func catalogStamp(_ record: CKRecord) throws -> CatalogStamp? {
    let format = record["format"] as? Int ?? 0
    guard format >= 2 else { return nil }
    guard let revision = record["catalogRevision"] as? Int64, revision > 0,
      let tag = record.recordChangeTag, !tag.isEmpty
    else { throw CloudFailure("Missing catalog revision receipt") }
    return CatalogStamp(revision: UInt64(revision), changeTag: tag)
  }

  private func save(_ record: CKRecord, _ options: Options) async throws -> CKRecord {
    try check()
    var revision = record["catalogRevision"] as? Int64 ?? 0
    if record.recordChangeTag == nil, let prior = try library.catalogStamp(id: record.recordID.recordName) {
      revision = max(revision, Int64(prior.revision))
    }
    guard revision >= 0, revision < Int64.max else { throw CloudFailure("Catalog revision exhausted") }
    record["catalogRevision"] = revision + 1
    let op = CKModifyRecordsOperation(recordsToSave: [record], recordIDsToDelete: nil)
    op.savePolicy = .ifServerRecordUnchanged
    op.isAtomic = false
    configure(op, options, payload: true)
    let result = Mutex<Result<CKRecord, Error>?>(nil)
    return try await withCheckedThrowingContinuation { reply in
      op.perRecordSaveBlock = { _, saved in result.withLock { $0 = saved } }
      op.modifyRecordsResultBlock = { completion in
        let saved = result.withLock { $0 }
        switch completion {
        case .success: reply.resume(with: saved ?? .failure(CloudFailure("Missing upload receipt")))
        case .failure(let error): reply.resume(with: saved ?? .failure(error))
        }
      }
      submit(op)
    }
  }

  private func send(_ options: Options) async throws {
    let retries = Dictionary(uniqueKeysWithValues: try library.retries().map { ($0.id, $0) })
    var sent = 0
    var conflicts = 0
    for entry in try library.entries() where !transitions.contains(entry.id) {
      guard try (entry.dirty || library.hasPendingCheckpoint(id: entry.id)) else { continue }
      if let retry = retries[entry.id],
        retry.incoming || (!options.explicit && retry.retryAt > Date().timeIntervalSince1970)
      {
        continue
      }
      let id = CKRecord.ID(recordName: entry.id, zoneID: zone)
      for round in 0..<3 {
        var path: String?
        defer { if let path { try? library.releaseUpload(path: path) } }
        do {
          try check()
          if round == 0, try library.hasPendingCheckpoint(id: entry.id) {
            do {
              let latest = try await fetchRecord(id, options)
              try await accept(latest, options)
            } catch let error as CKError where error.code == .unknownItem {
              guard try library.pendingCheckpointNewRecord(id: entry.id) else { throw error }
            }
          }
          let upload: LibraryUpload
          if round == 0 {
            guard let staged = try library.stageSync(id: entry.id, explicit: options.explicit) else { break }
            upload = staged
          } else {
            // Retrying the admitted checkpoint must not wait for the relay throttle again.
            upload = try library.stage(id: entry.id)
          }
          path = upload.assetPath
          let record: CKRecord
          if let bytes = upload.fields {
            let decoder = try NSKeyedUnarchiver(forReadingFrom: bytes)
            decoder.requiresSecureCoding = true
            guard let restored = CKRecord(coder: decoder) else { throw CloudFailure("Invalid server record metadata") }
            if decoder.containsValue(forKey: "popCatalogRevision") {
              restored["catalogRevision"] = decoder.decodeInt64(forKey: "popCatalogRevision")
            }
            decoder.finishDecoding()
            record = restored
          } else {
            record = CKRecord(recordType: "SlopDocument", recordID: id)
          }
          record["payload"] = path.map { CKAsset(fileURL: URL(fileURLWithPath: $0)) }
          record["deleted"] = upload.entry.deleted ? 1 : 0
          record["title"] = upload.entry.title
          record["template"] = upload.entry.template
          record["snapshotVersion"] = upload.entry.version
          record["assetRevision"] = upload.entry.assetRevision
          record["artworkVersion"] = upload.entry.artworkVersion
          record["format"] = 2
          record["authority"] = try libraryEncodeAuthority(authority: upload.authority)
          let saved = try await save(record, options)
          try check()
          guard let version = saved["snapshotVersion"] as? String else {
            throw CloudFailure("Missing uploaded version")
          }
          if case .checkpoint = upload.authority {
            try library.receiveCatalogSnapshot(
              id: entry.id, title: upload.entry.title, template: upload.entry.template, asset: path, version: version,
              fields: fields(saved), deleted: upload.entry.deleted, artworkVersion: upload.entry.artworkVersion,
              assetRevision: upload.entry.assetRevision, authority: upload.authority, stamp: try catalogStamp(saved))
          } else {
            try library.acknowledge(
              id: entry.id, version: version, assetRevision: saved["assetRevision"] as? String,
              fields: fields(saved), deleted: (saved["deleted"] as? Int) == 1, stamp: try catalogStamp(saved))
          }
          try library.clearRetry(id: entry.id)
          failures[entry.id] = nil
          sent += 1
          break
        } catch let error as CKError where error.code == .unknownItem {
          if try library.prepareDeletionReupload(id: entry.id), round < 2 { continue }
          block(id, error, incoming: false)
          break
        } catch let error as CKError where error.code == .serverRecordChanged {
          conflicts += 1
          logger.info("conflict id=\(entry.id, privacy: .public) round=\(round + 1)")
          do {
            let server: CKRecord
            if let record = error.serverRecord { server = record } else { server = try await fetchRecord(id, options) }
            try await accept(server, options)
            if round == 2, try library.entries().contains(where: { $0.id == entry.id && $0.dirty }) {
              block(id, CloudFailure("Still waiting to upload; will retry"), incoming: false)
            }
          } catch {
            block(id, error, incoming: true)
            break
          }
        } catch {
          block(id, error, incoming: false)
          break
        }
      }
    }
    logger.info("send saved=\(sent) conflicts=\(conflicts)")
  }
}
