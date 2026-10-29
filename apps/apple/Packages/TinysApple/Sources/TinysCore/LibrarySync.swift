@preconcurrency import CloudKit
import Foundation
import Synchronization
import TinysCoreBinding
import os

struct CloudFailure: Error, CustomStringConvertible {
  var description: String
  init(_ text: String) { description = text }
}

/// CloudKit transports complete files; Rust owns replication and the live owner.
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
  private let logger = Logger(subsystem: "click.tinys.library", category: "sync")
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
  func download(id: String) async throws {
    if let running { _ = await running.value }
    try check()
    let options = Options(request: .full, explicit: true, background: false)
    let record = try await fetchRecord(CKRecord.ID(recordName: id, zoneID: zone), options)
    try await accept(record, options, downloading: true)
    guard let entry = try library.entries().first(where: { $0.id == id }), !entry.deleted, entry.downloaded else {
      throw CloudFailure("This tiny is no longer available in iCloud.")
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
          guard (record["format"] as? Int) == 1, record["title"] is String, record["template"] is String,
            record["snapshotVersion"] is String
          else { throw LibraryError.Failed(issue: .requiresUpdate, message: "This synced tiny needs a newer tinys.") }
          let retrying = try library.retries().contains { $0.id == id.recordName && $0.incoming }
          let unchanged =
            options.request == .full && confirmed?.count == 2
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
      case .notAuthenticated: return "Sign in to iCloud to sync your tinys."
      case .quotaExceeded: return "Your iCloud storage is full. Free up space, then retry."
      case .unknownItem: return "An iCloud record is missing. Sync needs attention."
      default: return "iCloud couldn't sync some changes. Please retry."
      }
    }
    if case LibraryError.Failed(let issue, _) = error {
      switch issue {
      case .requiresUpdate: return "Update tinys to sync this tiny."
      case .appMismatch: return "These copies use different data schemas or theme palettes."
      case .busy: return "This tiny is busy. Please retry shortly."
      default: break
      }
    }
    return "Some changes couldn't sync. Please retry."
  }

  @discardableResult private func block(
    _ id: CKRecord.ID, _ error: Error, record: CKRecord? = nil, incoming: Bool
  ) -> Bool {
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
          id: id.recordName, title: record?["title"] as? String ?? "Tiny",
          template: record?["template"] as? String ?? "", message: message,
          retryAt: Date().addingTimeInterval(delay).timeIntervalSince1970, incoming: incoming))
      if incoming {
        try? library.block(
          id: id.recordName, title: record?["title"] as? String ?? "Tiny",
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
    coder.finishEncoding()
    return coder.encodedData
  }

  private func accept(_ input: CKRecord, _ options: Options, downloading: Bool = false) async throws {
    guard input.recordID.zoneID == zone, input.recordType == "SlopDocument" else { return }
    try check()
    if !downloading, (input["deleted"] as? Int) != 1 {
      guard let version = input["snapshotVersion"] as? String,
        let title = input["title"] as? String, let template = input["template"] as? String,
        (input["format"] as? Int) == 1
      else { throw LibraryError.Failed(issue: .requiresUpdate, message: "This synced tiny needs a newer tinys.") }
      if try library.receiveRemovedMetadata(
        id: input.recordID.recordName, title: title, template: template, version: version, fields: fields(input),
        artworkVersion: input["artworkVersion"] as? String, assetRevision: input["assetRevision"] as? String)
      {
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
    guard let version = record["snapshotVersion"] as? String,
      let title = record["title"] as? String,
      let template = record["template"] as? String,
      (record["format"] as? Int) == 1
    else { throw LibraryError.Failed(issue: .requiresUpdate, message: "This synced tiny needs a newer tinys.") }
    try library.receive(
      id: record.recordID.recordName, title: title, template: template,
      asset: (record["payload"] as? CKAsset)?.fileURL?.path, version: version, fields: fields(record),
      deleted: (record["deleted"] as? Int) == 1,
      artworkVersion: record["artworkVersion"] as? String, assetRevision: record["assetRevision"] as? String)
    failures[record.recordID.recordName] = nil
    let dirty = try library.entries().contains { $0.id == record.recordID.recordName && $0.dirty }
    if try library.retries().contains(where: { $0.id == record.recordID.recordName && ($0.incoming || !dirty) }) {
      try library.clearRetry(id: record.recordID.recordName)
    }
  }

  private func save(_ record: CKRecord, _ options: Options) async throws -> CKRecord {
    try check()
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
    for entry in try library.entries() where entry.dirty {
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
          record["format"] = 1
          let saved = try await save(record, options)
          try check()
          guard let version = saved["snapshotVersion"] as? String else {
            throw CloudFailure("Missing uploaded version")
          }
          try library.acknowledge(
            id: entry.id, version: version, assetRevision: saved["assetRevision"] as? String,
            fields: fields(saved), deleted: (saved["deleted"] as? Int) == 1)
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
