import CryptoKit
import Foundation
import Testing

@testable import PopAccount

/// 1.x signed in through a named Firebase app whose identity this lineage's Auth never holds.
/// Its leftover flags must not read as this lineage's identity having gone missing, which would
/// refuse guests and sign-in until the person found the reset; a 2.0 identity that goes missing
/// is still refused rather than replaced by a different anonymous UID.
@Test func anEarlierLineagesAccountFlagsAreNotThisLineagesIdentity() throws {
  let suite = "pop.tests.account.\(UUID().uuidString)"
  let defaults = try #require(UserDefaults(suiteName: suite))
  defer { defaults.removePersistentDomain(forName: suite) }
  // What a signed-in 1.x device leaves: its retained flag, and the mark of a deletion whose
  // 1.x cloud step finished. The same Firebase project gives an Apple account the same UID.
  let uid = "apple-account-uid"
  let hash = SHA256.hash(data: Data(uid.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
  defaults.set(true, forKey: "sharing.signedIn")
  defaults.set(true, forKey: "sharing.deletion." + hash)
  let account = SharingAccountDefaults(defaults: defaults)

  // First 2.0 launch: no Firebase user, and nothing to report missing or already deleted.
  #expect(!account.identityRetained)
  #expect(!account.identityMissing(hasUser: false))
  #expect(!defaults.bool(forKey: account.cloudDeletedKey(uid)), "2.0 still deletes its own cloud documents")

  // 2.0 held an identity; losing it afterwards is refused, including after relaunch.
  account.identityRetained = true
  #expect(!account.identityMissing(hasUser: true))
  #expect(account.identityMissing(hasUser: false))
  #expect(SharingAccountDefaults(defaults: defaults).identityMissing(hasUser: false))

  // An explicit sign-out or reset clears only this lineage's flag.
  account.identityRetained = false
  #expect(!account.identityMissing(hasUser: false))
  #expect(defaults.bool(forKey: "sharing.signedIn"), "1.x flags are left as they are")
}
