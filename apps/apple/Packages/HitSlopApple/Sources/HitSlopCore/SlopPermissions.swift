import Foundation

public enum SlopPermissions {
  public static func makeWritable(_ root: URL) throws {
    try setWriteBits(root, adding: true)
  }

  public static func makeImmutable(_ root: URL) throws {
    try setWriteBits(root, adding: false)
  }

  private static func setWriteBits(_ root: URL, adding: Bool) throws {
    var urls = [root]
    if let enumerator = FileManager.default.enumerator(
      at: root, includingPropertiesForKeys: [.isDirectoryKey, .isSymbolicLinkKey])
    {
      urls += enumerator.compactMap { $0 as? URL }
    }
    for url in adding ? urls : urls.reversed() {
      let values = try url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
      guard values.isSymbolicLink != true else {
        throw SlopPackageError.invalid("symlinks are not allowed")
      }
      let attributes = try FileManager.default.attributesOfItem(atPath: url.path)
      guard let value = attributes[.posixPermissions] as? NSNumber else { continue }
      let next =
        adding
        ? value.intValue | (values.isDirectory == true ? 0o700 : 0o600)
        : value.intValue & ~0o222
      if next != value.intValue {
        try FileManager.default.setAttributes([.posixPermissions: next], ofItemAtPath: url.path)
      }
    }
  }
}
