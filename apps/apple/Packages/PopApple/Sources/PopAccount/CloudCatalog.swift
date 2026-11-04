@preconcurrency import FirebaseAuth
@preconcurrency import FirebaseFirestore
import Foundation
import PopCore
import PopCoreBinding

/// Native Firebase IO only. Rust validates entries and owns files, membership and sync.
@MainActor final class CloudCatalog {
  static let shared = CloudCatalog()
  private var listener: ListenerRegistration?
  private var task: Task<Void, Never>?
  private var generation: UInt64 = 0

  func connect(auth: Auth, user: User?) {
    generation += 1
    let current = generation
    listener?.remove()
    listener = nil
    task?.cancel()
    PopLibrary.shared.catalogPublisher = nil
    task = Task {
      do {
        let account = user?.uid ?? ""
        try await PopLibrary.shared.setAccount(account, external: true)
        guard current == generation, !Task.isCancelled else { return }
        guard user != nil else { return }
        guard let app = auth.app else { throw PopFailure("The account service is unavailable.") }
        let db = Firestore.firestore(app: app)
        PopLibrary.shared.catalogPublisher = { entry, targetAccount in
          guard current == self.generation, targetAccount == account, auth.currentUser?.uid == account else {
            throw PopFailure("The account changed. Catalog publication will retry after sign-in.")
          }
          try await db.collection("users").document(account).collection("documents")
            .document(entry.documentId).setData(Self.fields(entry))
        }
        listener = db.collection("users").document(account).collection("documents")
          .order(by: "modifiedAt", descending: true)
          .addSnapshotListener { snapshot, error in
            Task { @MainActor in
              guard current == self.generation else { return }
              do {
                if let error { throw error }
                guard let snapshot else { return }
                var entries: [DocumentEntry] = []
                var unreadable = 0
                for record in snapshot.documents {
                  do { entries.append(try Self.entry(record)) } catch { unreadable += 1 }
                }
                try await PopLibrary.shared.receiveCatalog(entries, account: account)
                if unreadable > 0 {
                  PopLibrary.shared.reportCatalogError(
                    PopFailure("Some cloud references could not be read. Other documents are still available."),
                    account: account)
                }
              } catch { PopLibrary.shared.reportCatalogError(error, account: account) }
            }
          }
        await PopLibrary.shared.syncNow()
      } catch { PopLibrary.shared.reportCatalogError(error, account: user?.uid ?? "") }
    }
  }

  private static func fields(_ entry: DocumentEntry) -> [String: Any] {
    [
      "documentId": entry.documentId, "title": entry.title, "templateId": entry.templateId,
      "appRevision": entry.appRevision as Any? ?? NSNull(),
      "previewRef": entry.previewRef as Any? ?? NSNull(),
      "modifiedAt": Timestamp(date: Date(timeIntervalSince1970: Double(entry.modifiedAt) / 1000)),
      "role": entry.role == .owner ? "owner" : "editor",
      "accessGeneration": NSNumber(value: entry.accessGeneration),
      "catalogRevision": NSNumber(value: entry.catalogRevision), "deleted": entry.deleted,
    ]
  }

  private static func entry(_ record: QueryDocumentSnapshot) throws -> DocumentEntry {
    let data = record.data()
    guard let document = data["documentId"] as? String, document == record.documentID,
      let title = data["title"] as? String, let template = data["templateId"] as? String,
      let rawRole = data["role"] as? String,
      let access = data["accessGeneration"] as? NSNumber,
      let revision = data["catalogRevision"] as? NSNumber,
      access.int64Value >= 0, revision.int64Value > 0,
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
      appRevision: data["appRevision"] as? String, previewRef: data["previewRef"] as? String,
      modifiedAt: UInt64(milliseconds), role: role, accessGeneration: access.uint64Value,
      catalogRevision: revision.uint64Value, deleted: deleted)
  }
}
