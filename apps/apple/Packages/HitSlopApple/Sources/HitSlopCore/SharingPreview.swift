import Foundation
import HitSlopCoreBinding
import Security

extension Notification.Name {
  public static let hitSlopSharingAccountChanged = Notification.Name("hitSlop.sharingAccountChanged")
  public static let hitSlopSharingForeground = Notification.Name("hitSlop.sharingForeground")
  public static let hitSlopJoinSharedSlop = Notification.Name("hitSlop.joinSharedSlop")
  public static let hitSlopNewSyncedChecklist = Notification.Name("hitSlop.newSyncedChecklist")
}

/// Preview builds (the `HITSLOP_SHARING_PREVIEW` compilation condition, e.g. TestFlight) enable it;
/// Debug needs a launch flag; ordinary releases never include it.
public enum SharingPreview {
  /// Preserve the full URL, including its capability fragment, through the Rust parser.
  /// A non-sharing URL is left to the caller's existing routing.
  public static func incomingLink(_ url: URL) throws -> String? {
    guard url.scheme == "https", url.host == "share.hitslop.com", url.path == "/join" else { return nil }
    guard let fragment = url.fragment, !fragment.isEmpty else {
      throw SlopFailure("Open the link in hitSlop again")
    }
    let link = url.absoluteString
    _ = try sharingJoinCredential(link: link)
    return link
  }

  public static let enabled: Bool = {
    #if HITSLOP_SHARING_PREVIEW
      true
    #elseif DEBUG
      ProcessInfo.processInfo.arguments.contains("--live-sharing-preview")
        || ProcessInfo.processInfo.environment["HITSLOP_LIVE_SHARING_PREVIEW"] == "1"
    #else
      false
    #endif
  }()
}

public enum SyncedLibraryPreview {
  public static let enabled: Bool = {
    #if DEBUG || HITSLOP_SYNCED_LIBRARY
      true
    #else
      false
    #endif
  }()
}

/// Rust link credentials stay on this device. Verified owners recover through the account index.
public enum SharingCredentials {
  private static func query(_ document: String) throws -> [String: Any] {
    guard let group = Bundle.main.object(forInfoDictionaryKey: "SharingKeychainGroup") as? String,
      !group.isEmpty, !group.contains("$(")
    else { throw SlopFailure("Sharing preview requires the signed hitSlop app.") }
    return [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "com.hitslop.sharing-accounts-v1",
      kSecAttrAccount as String: document,
      kSecAttrAccessGroup as String: group,
      kSecUseDataProtectionKeychain as String: true,
    ]
  }
  public static func read(_ document: String) throws -> String? {
    var query = try query(document)
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
    guard status == errSecSuccess, let data = result as? Data, let value = String(data: data, encoding: .utf8)
    else { throw SlopFailure("Sharing details are unavailable in Keychain. Unlock this device and retry.") }
    return value
  }
  /// Enumerate this app's local link credentials; no tokens are stored here.
  public static func all() throws -> [String: String] {
    var query = try query("")
    query.removeValue(forKey: kSecAttrAccount as String)
    query[kSecReturnAttributes as String] = true
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitAll
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return [:] }
    guard status == errSecSuccess, let items = result as? [[String: Any]] else {
      throw SlopFailure("Could not read sharing details. Unlock this device and retry.")
    }
    return Dictionary(
      uniqueKeysWithValues: items.compactMap { item in
        guard let document = item[kSecAttrAccount as String] as? String,
          let data = item[kSecValueData as String] as? Data,
          let value = String(data: data, encoding: .utf8)
        else { return nil }
        return (document, value)
      })
  }
  public static func remove(_ document: String) throws {
    let status = SecItemDelete(try query(document) as CFDictionary)
    guard status == errSecSuccess || status == errSecItemNotFound else {
      throw SlopFailure("Could not clear sharing details. Unlock this device and retry.")
    }
  }
  public static func write(_ value: String, document: String) throws {
    let query = try query(document)
    let values: [String: Any] = [kSecValueData as String: Data(value.utf8)]
    var status = SecItemUpdate(query as CFDictionary, values as CFDictionary)
    if status == errSecItemNotFound {
      var item = query.merging(values) { _, new in new }
      item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
      status = SecItemAdd(item as CFDictionary, nil)
      if status == errSecDuplicateItem { status = SecItemUpdate(query as CFDictionary, values as CFDictionary) }
    }
    guard status == errSecSuccess else {
      throw SlopFailure("Could not save sharing details in Keychain. Unlock this device and retry.")
    }
  }
}
