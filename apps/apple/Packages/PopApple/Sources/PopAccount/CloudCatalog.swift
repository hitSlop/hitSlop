@preconcurrency import FirebaseAuth
@preconcurrency import FirebaseFirestore
import Foundation
import PopCore
import PopCoreBinding

/// Which account's catalog listener is attached, and which callbacks still belong to it.
/// Each attachment is a generation: callbacks from an earlier one never count.
struct CatalogAttachment {
  private var generation: UInt64 = 0
  /// The account being attached, or attached.
  private(set) var account: String?
  private var readyAccount: String?

  /// Whether `account`'s listener is attached and still trusted.
  func isReady(_ account: String) -> Bool { readyAccount == account }
  func isCurrent(_ generation: UInt64) -> Bool { generation == self.generation }
  /// Starts attaching `account`; earlier generations stop counting.
  mutating func begin(_ account: String) -> UInt64 {
    self.account = account
    readyAccount = nil
    generation += 1
    return generation
  }
  mutating func attached(_ generation: UInt64, account: String) {
    guard isCurrent(generation) else { return }
    readyAccount = account
  }
  /// A listener from `generation` failed for good. Firestore removes a listener after a
  /// server rejection, so the attached one is forgotten (true: drop it) and the next
  /// connect re-attaches instead of trusting it.
  mutating func failed(_ generation: UInt64, account: String) -> Bool {
    guard isCurrent(generation), readyAccount == account else { return false }
    readyAccount = nil
    return true
  }
}

/// Native Firebase IO only. Rust validates entries and owns files, membership and sync.
@MainActor final class CloudCatalog {
  static let shared = CloudCatalog()
  private var listener: ListenerRegistration?
  private var task: Task<Void, Never>?
  private var delivery: Task<Void, Never>?
  private enum CatalogBatch: Sendable {
    case snapshot([DocumentEntry], unreadable: UInt32)
    case failed(String)
  }
  private var attachment = CatalogAttachment()

  func connect(auth: Auth, user: User?) async {
    guard auth.currentUser?.uid == user?.uid else { return }
    let target = user?.uid ?? ""
    if attachment.account == target {
      await task?.value
      guard auth.currentUser?.uid == user?.uid else { return }
      if attachment.isReady(target), PopLibrary.shared.isReady, PopLibrary.shared.accountIdentity == target {
        PopLibrary.shared.allowsCloudCreation = auth.currentUser?.isAnonymous == false
        return
      }
    }
    let current = attachment.begin(target)
    listener?.remove()
    listener = nil
    task?.cancel()
    delivery?.cancel()
    delivery = nil
    PopLibrary.shared.catalogPublisher = nil
    task = Task {
      do {
        let account = user?.uid ?? ""
        try await PopLibrary.shared.setAccount(account, external: true)
        guard attachment.isCurrent(current), !Task.isCancelled else { return }
        PopLibrary.shared.allowsCloudCreation = auth.currentUser?.isAnonymous == false
        guard user != nil else {
          attachment.attached(current, account: account)
          return
        }
        guard let app = auth.app else { throw PopFailure("The account service is unavailable.") }
        let db = Firestore.firestore(app: app)
        PopLibrary.shared.catalogPublisher = { entry, targetAccount in
          guard self.attachment.isCurrent(current), targetAccount == account, auth.currentUser?.uid == account else {
            throw PopFailure("The account changed. Catalog publication will retry after sign-in.")
          }
          // Submit the SDK's untyped payload synchronously on this actor. Only
          // the completion crosses the asynchronous boundary.
          try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            db.collection("users").document(account).collection("pops")
              .document(entry.documentId).setData(Self.fields(entry)) { @Sendable error in
                if let error { continuation.resume(throwing: error) } else { continuation.resume() }
              }
          }
        }
        // These are complete discovery snapshots, not page deltas. Only the newest
        // queued snapshot is needed; the consumer still finishes each application.
        let (snapshots, continuation) = AsyncStream<CatalogBatch>.makeStream(bufferingPolicy: .bufferingNewest(1))
        delivery = Task { @MainActor in
          for await batch in snapshots {
            guard self.attachment.isCurrent(current), !Task.isCancelled else { return }
            switch batch {
            case .failed(let message):
              // The next connect() (sync, foreground, token refresh) re-attaches.
              if self.attachment.failed(current, account: account) {
                self.listener?.remove()
                self.listener = nil
              }
              PopLibrary.shared.reportCatalogError(PopFailure(message), account: account)
            case .snapshot(let entries, let unreadable):
              do {
                let rejected = try await PopLibrary.shared.receiveCatalog(entries, account: account)
                guard self.attachment.isCurrent(current), !Task.isCancelled else { return }
                if rejected > 0 || unreadable > 0 {
                  PopLibrary.shared.reportCatalogError(
                    PopFailure("Some cloud references could not be read. Other documents are still available."),
                    account: account)
                }
              } catch { PopLibrary.shared.reportCatalogError(error, account: account) }
            }
          }
        }
        listener = db.collection("users").document(account).collection("pops")
          .order(by: "modifiedAt", descending: true)
          .addSnapshotListener { @Sendable snapshot, error in
            if let error {
              continuation.yield(.failed(error.localizedDescription))
              return
            }
            guard let snapshot else { return }
            var entries: [DocumentEntry] = []
            var unreadable: UInt32 = 0
            for record in snapshot.documents {
              do { entries.append(try Self.entry(record)) } catch { unreadable += 1 }
            }
            continuation.yield(.snapshot(entries, unreadable: unreadable))
          }
        attachment.attached(current, account: account)
        Task { await PopLibrary.shared.syncNow() }
      } catch { PopLibrary.shared.reportCatalogError(error, account: user?.uid ?? "") }
    }
    await task?.value
  }

  private static func fields(_ entry: DocumentEntry) -> [String: Any] {
    [
      "documentId": entry.documentId, "title": entry.title, "templateId": entry.templateId,
      "modifiedAt": Timestamp(date: Date(timeIntervalSince1970: Double(entry.modifiedAt) / 1000)),
      "role": entry.role == .owner ? "owner" : "editor",
      "catalogRevision": NSNumber(value: entry.catalogRevision), "deleted": entry.deleted,
    ]
  }

  private nonisolated static func entry(_ record: QueryDocumentSnapshot) throws -> DocumentEntry {
    let data = record.data()
    guard let document = data["documentId"] as? String, document == record.documentID,
      let title = data["title"] as? String, let template = data["templateId"] as? String,
      let rawRole = data["role"] as? String,
      let revision = data["catalogRevision"] as? NSNumber,
      revision.int64Value > 0,
      let modified = data["modifiedAt"] as? Timestamp,
      let deleted = data["deleted"] as? Bool
    else { throw PopFailure("The cloud catalog contains an invalid entry.") }
    let role: DocumentRole
    switch rawRole {
    case "owner": role = .owner
    case "editor": role = .editor
    default: throw PopFailure("This document requires a newer pop.")
    }
    let milliseconds = modified.dateValue().timeIntervalSince1970 * 1000
    guard milliseconds >= 0, milliseconds < Double(UInt64.max) else {
      throw PopFailure("The cloud catalog date is invalid.")
    }
    return DocumentEntry(
      documentId: document, title: title, templateId: template,
      modifiedAt: UInt64(milliseconds), role: role, catalogRevision: revision.uint64Value, deleted: deleted)
  }
}
