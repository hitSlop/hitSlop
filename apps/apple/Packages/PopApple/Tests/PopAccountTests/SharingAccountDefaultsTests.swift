import Foundation
import Testing

@testable import PopAccount

private func scratchDefaults() throws -> (UserDefaults, cleanup: () -> Void) {
  let suite = "click.dotpop.tests.account.\(UUID().uuidString)"
  let defaults = try #require(UserDefaults(suiteName: suite))
  return (defaults, { defaults.removePersistentDomain(forName: suite) })
}

private struct Refused: Error {}

/// Records the deletion steps that ran, in the order they ran.
@MainActor private final class DeletionSteps {
  enum Step: Equatable {
    case cloud
    case identity
  }
  var ran: [Step] = []
  var cloudFails = false
  var identityFails = false
  func cloud() throws {
    ran.append(.cloud)
    if cloudFails { throw Refused() }
  }
  func identity(_ authorization: Result<String, Error>) throws {
    ran.append(.identity)
    if identityFails { throw Refused() }
  }
  func delete(
    _ account: SharingAccountDefaults, uid: String,
    consent: AccountDeletionConsent<String> = .answered(.success("apple"))
  ) async -> AccountDeletionOutcome {
    await account.deleteAccount(
      uid: uid, consent: { consent }, deleteCloud: { try self.cloud() }, deleteIdentity: { try self.identity($0) })
  }
}

/// A retained identity that Firebase lost is refused, also after relaunch, rather than replaced
/// by a different anonymous UID; only an explicit sign-out or reset clears it.
@Test func aLostRetainedIdentityIsRefusedUntilAnExplicitReset() throws {
  let (defaults, cleanup) = try scratchDefaults()
  defer { cleanup() }
  let account = SharingAccountDefaults(defaults: defaults)
  #expect(!account.identityRetained)
  #expect(!account.identityMissing(hasUser: false), "a first launch has nothing to miss")

  account.identityRetained = true
  #expect(!account.identityMissing(hasUser: true))
  #expect(account.identityMissing(hasUser: false))
  #expect(SharingAccountDefaults(defaults: defaults).identityMissing(hasUser: false), "relaunch keeps refusing")

  account.identityRetained = false
  #expect(!account.identityMissing(hasUser: false))
}

/// Apple is asked first, so dismissing its confirmation deletes nothing: no deletion step runs
/// and nothing is recorded, and the account is exactly as it was.
@Test @MainActor func cancellingAppleConfirmationDeletesNothing() async throws {
  let (defaults, cleanup) = try scratchDefaults()
  defer { cleanup() }
  let account = SharingAccountDefaults(defaults: defaults)
  account.identityRetained = true
  let steps = DeletionSteps()
  #expect(await steps.delete(account, uid: "person", consent: .cancelled) == .cancelled)
  #expect(steps.ran.isEmpty)
  #expect(!defaults.bool(forKey: account.cloudDeletedKey("person")))
  #expect(account.identityRetained)
}

/// The identity owns the cloud documents, so it is never deleted while they remain.
@Test @MainActor func aFailedCloudDeletionKeepsTheIdentity() async throws {
  let (defaults, cleanup) = try scratchDefaults()
  defer { cleanup() }
  let account = SharingAccountDefaults(defaults: defaults)
  account.identityRetained = true
  let steps = DeletionSteps()
  steps.cloudFails = true
  #expect(await steps.delete(account, uid: "person") == .failed(cloudDeleted: false))
  #expect(steps.ran == [.cloud], "the identity is kept for the retry")
  #expect(!defaults.bool(forKey: account.cloudDeletedKey("person")))
  #expect(account.identityRetained)
}

/// The relay fences a deleted UID for good, so a retry that asked it to delete the account
/// again could never finish. A failure after the cloud deletion keeps that UID's checkpoint
/// across a relaunch; the retry resumes at the identity, and the finished deletion clears it.
/// Another account's checkpoint is its own.
@Test @MainActor func aRetryAfterTheCloudDeletionResumesAtTheIdentity() async throws {
  let (defaults, cleanup) = try scratchDefaults()
  defer { cleanup() }
  let account = SharingAccountDefaults(defaults: defaults)
  account.identityRetained = true
  // Another account on this device stopped at the same point earlier.
  let other = DeletionSteps()
  other.identityFails = true
  #expect(await other.delete(account, uid: "other") == .failed(cloudDeleted: true))

  let first = DeletionSteps()
  first.identityFails = true
  #expect(await first.delete(account, uid: "person") == .failed(cloudDeleted: true))
  #expect(first.ran == [.cloud, .identity], "its own cloud documents are deleted despite the other checkpoint")
  #expect(defaults.bool(forKey: account.cloudDeletedKey("person")))
  #expect(account.identityRetained)

  let relaunched = SharingAccountDefaults(defaults: defaults)
  let retry = DeletionSteps()
  #expect(await retry.delete(relaunched, uid: "person") == .deleted)
  #expect(retry.ran == [.identity], "the cloud deletion is not asked again")
  #expect(!defaults.bool(forKey: account.cloudDeletedKey("person")))
  #expect(!relaunched.identityRetained)
  #expect(defaults.bool(forKey: account.cloudDeletedKey("other")), "another account's checkpoint is unaffected")
}
