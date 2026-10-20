import Foundation
import Security

extension Notification.Name {
  public static let hitSlopSharingAccountChanged = Notification.Name("hitSlop.sharingAccountChanged")
  public static let hitSlopSharingForeground = Notification.Name("hitSlop.sharingForeground")
  public static let hitSlopJoinSharedSlop = Notification.Name("hitSlop.joinSharedSlop")
  public static let hitSlopNewSyncedChecklist = Notification.Name("hitSlop.newSyncedChecklist")
}

/// This preview requires an explicit developer launch flag; it is absent from ordinary releases.
public enum SharingPreview {
  public static let enabled: Bool = {
    #if DEBUG || HITSLOP_SHARING_PREVIEW
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
