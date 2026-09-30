import CryptoKit
import Darwin
import Foundation

/// A refusal that retrying cannot fix. The bridge replies `{ rejected }`, never a save failure.
public protocol SlopRejection: LocalizedError {}

public enum SlopAttachmentError: SlopRejection, Equatable {
  case tooLarge, limitReached, invalidID, notFound
  public var errorDescription: String? {
    switch self {
    case .invalidID: "Invalid attachment ID"
    case .notFound: "Attachment not found"
    case .tooLarge: "Attachment exceeds \(AttachmentLimits.file >> 20) MiB"
    case .limitReached: "Document attachment limit reached (\(AttachmentLimits.total >> 20) MiB or \(AttachmentLimits.count) files)"
    }
  }
}

/// A stored attachment, as the page and the CLI see it.
public struct AttachmentRef: Codable, Sendable, Equatable {
  public let id: String
  public let byteLength: Int
  public var json: [String: Any] { ["id": id, "byteLength": byteLength] }
}

/// Opaque immutable blobs. Mutations must run under the package's writer lease.
public enum SlopAttachments {
  public static func validID(_ id: String) -> Bool {
    id.utf8.count == 64 && id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
  }
  private static func hash(_ bytes: Data) -> String {
    SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
  }
  public static func list(in root: URL) throws -> [AttachmentRef] {
    let directory = root.appendingPathComponent("state/attachments")
    var info = stat()
    let state = root.appendingPathComponent("state")
    if lstat(state.path, &info) != 0 {
      if errno == ENOENT { return [] }
      throw SlopPackageError.invalid("cannot inspect state directory")
    }
    guard info.st_mode & S_IFMT == S_IFDIR else { throw SlopPackageError.invalid("unsafe state directory") }
    if lstat(directory.path, &info) != 0 {
      if errno == ENOENT { return [] }
      throw SlopPackageError.invalid("cannot inspect attachments")
    }
    guard info.st_mode & S_IFMT == S_IFDIR else {
      throw SlopPackageError.invalid("attachments must be a directory without symlinks")
    }
    var files: [AttachmentRef] = []
    var total = 0
    let entries = try FileManager.default.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil)
    guard entries.count <= AttachmentLimits.count + 1 else { throw SlopPackageError.invalid("too many attachments") }
    for file in entries {
      let name = file.lastPathComponent
      // A crash may leave one staged write. Never expose it or treat it as a reference.
      let pending = name == ".pending"
      guard pending || validID(name), lstat(file.path, &info) == 0,
        info.st_mode & S_IFMT == S_IFREG, info.st_nlink == 1,
        info.st_size >= 0, info.st_size <= AttachmentLimits.file else {
        throw SlopPackageError.invalid("unsafe or oversized attachment")
      }
      total += Int(info.st_size)
      if !pending { files.append(AttachmentRef(id: name, byteLength: Int(info.st_size))) }
    }
    guard files.count <= AttachmentLimits.count, total <= AttachmentLimits.total + AttachmentLimits.file else {
      throw SlopPackageError.invalid("attachment storage limit exceeded")
    }
    let committed = files.reduce(0) { $0 + $1.byteLength }
    guard committed <= AttachmentLimits.total else { throw SlopPackageError.invalid("attachments exceed \(AttachmentLimits.total >> 20) MiB") }
    return files.sorted { $0.id < $1.id }
  }
  public static func read(_ id: String, in root: URL) throws -> Data {
    guard validID(id) else { throw SlopAttachmentError.invalidID }
    let url = root.appendingPathComponent("state/attachments/" + id)
    guard FileManager.default.fileExists(atPath: url.path) else { throw SlopAttachmentError.notFound }
    let data = try SlopFile.read(url, within: root, maximumBytes: AttachmentLimits.file)
    guard hash(data) == id else { throw SlopPackageError.invalid("attachment checksum mismatch") }
    return data
  }
  public static func put(_ data: Data, in root: URL) throws -> AttachmentRef {
    guard data.count <= AttachmentLimits.file else { throw SlopAttachmentError.tooLarge }
    let id = hash(data)
    let files = try list(in: root)
    if files.contains(where: { $0.id == id }) {
      _ = try read(id, in: root)
      return AttachmentRef(id: id, byteLength: data.count)
    }
    guard files.count < AttachmentLimits.count,
      files.reduce(0, { $0 + $1.byteLength }) + data.count <= AttachmentLimits.total
    else { throw SlopAttachmentError.limitReached }
    let rootFD = Darwin.open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
    guard rootFD >= 0 else { throw SlopPackageError.invalid("unsafe document directory") }
    defer { Darwin.close(rootFD) }
    let stateFD = openat(rootFD, "state", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
    guard stateFD >= 0 else { throw SlopPackageError.invalid("unsafe state directory") }
    defer { Darwin.close(stateFD) }
    if mkdirat(stateFD, "attachments", 0o700) != 0 && errno != EEXIST {
      throw SlopPackageError.invalid("cannot create attachments directory")
    }
    let directory = openat(stateFD, "attachments", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
    guard directory >= 0 else { throw SlopPackageError.invalid("unsafe attachments directory") }
    defer { Darwin.close(directory) }
    // Only the lease owner can stage a write. Remove a previous crash's partial file.
    _ = unlinkat(directory, ".pending", 0)
    let file = openat(directory, ".pending", O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
    guard file >= 0 else { throw SlopPackageError.invalid("cannot stage attachment") }
    defer { Darwin.close(file); _ = unlinkat(directory, ".pending", 0) }
    try data.withUnsafeBytes { bytes in
      var offset = 0
      while offset < bytes.count {
        let count = Darwin.write(file, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
        if count < 0 && errno == EINTR { continue }
        guard count > 0 else { throw SlopPackageError.invalid("cannot write attachment") }
        offset += count
      }
    }
    guard fsync(file) == 0, renameat(directory, ".pending", directory, id) == 0,
      fsync(directory) == 0, fsync(stateFD) == 0 else {
      throw SlopPackageError.invalid("cannot finalize attachment")
    }
    return AttachmentRef(id: id, byteLength: data.count)
  }
}
