import Foundation
import Security
import TinysCoreBinding

extension Notification.Name {
  public static let tinysSharingAccountChanged = Notification.Name("tinys.sharingAccountChanged")
  public static let tinysSharingForeground = Notification.Name("tinys.sharingForeground")
  public static let tinysJoinSharedTiny = Notification.Name("tinys.joinSharedTiny")
}

/// Parses live collaboration invitations on Mac and iPhone.
public enum SharingLinks {
  public static let appleSignIn = (
    servicesID: "com.hitslop.app.signin",
    returnURL: "https://share.tinys.click/auth/apple"
  )

  /// Preserve the full URL, including its capability fragment, through the Rust parser.
  /// A non-sharing URL is left to the caller's existing routing.
  public static func incomingLink(_ url: URL) throws -> String? {
    guard url.scheme == "https", url.host == "share.tinys.click", url.path == "/join" else { return nil }
    guard let fragment = url.fragment, !fragment.isEmpty else {
      throw TinyFailure("Open the link in tinys again")
    }
    let link = url.absoluteString
    _ = try sharingJoinCredential(link: link)
    return link
  }
}

/// Rust link credentials stay on this device. Verified owners recover through the account index.
public enum SharingCredentials {
  private static func query(_ document: String) throws -> [String: Any] {
    guard let group = Bundle.main.object(forInfoDictionaryKey: "SharingKeychainGroup") as? String,
      !group.isEmpty, !group.contains("$(")
    else { throw TinyFailure("Live collaboration needs the signed tinys app.") }
    return [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: "com.hitslop.sharing-accounts-v1",
      kSecAttrAccount as String: document,
      kSecAttrAccessGroup as String: group,
      kSecUseDataProtectionKeychain as String: true,
    ]
  }
  /// An unsigned build resolves the group but Keychain refuses it for lack of the entitlement.
  private static func failure(_ status: OSStatus, _ message: String) -> TinyFailure {
    TinyFailure(status == errSecMissingEntitlement ? "Live collaboration needs the signed tinys app." : message)
  }
  public static func read(_ document: String) throws -> String? {
    var query = try query(document)
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return nil }
    guard status == errSecSuccess, let data = result as? Data, let value = String(data: data, encoding: .utf8)
    else { throw failure(status, "Sharing details are unavailable in Keychain. Unlock this device and retry.") }
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
      throw failure(status, "Could not read sharing details. Unlock this device and retry.")
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
      throw failure(status, "Could not clear sharing details. Unlock this device and retry.")
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
      throw failure(status, "Could not save sharing details in Keychain. Unlock this device and retry.")
    }
  }
}
