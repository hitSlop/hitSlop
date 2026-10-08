import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding
import ImageIO
import UniformTypeIdentifiers

/// Screenshot output may replace a PNG, but never a document or a destination that
/// changed while WebKit rendered. Exports have their separate, exclusive-write contract.
public struct SlopScreenshotDestination {
  private struct Version: Equatable {
    let device: Int32
    let inode: UInt64
    let size: Int64
    let modified: Int
    let modifiedNanos: Int
    let changed: Int
    let changedNanos: Int
  }
  private let url: URL
  private let source: URL
  private let version: Version?

  public init(output: URL, source: URL) throws {
    // Resolve the parent, but keep the final component so lstat can refuse a symlink.
    url = SlopPath.canonical(output.deletingLastPathComponent()).appendingPathComponent(output.lastPathComponent)
    self.source = SlopPath.canonical(source)
    version = try Self.inspect(url, source: self.source)
  }

  public func publish(_ png: Data) throws {
    guard try Self.inspect(url, source: source) == version else {
      throw Self.refusal("Screenshot destination changed during rendering")
    }
    if version == nil {
      do { try png.write(to: url, options: .withoutOverwriting) } catch CocoaError.fileWriteFileExists {
        throw Self.refusal("Screenshot destination now exists")
      }
    } else {
      try png.write(to: url, options: .atomic)
    }
  }

  private static func inspect(_ url: URL, source: URL) throws -> Version? {
    guard url.pathExtension.lowercased() != "slop", SlopPath.canonical(url) != source else {
      throw refusal("A screenshot cannot replace a hitSlop document")
    }
    var info = stat()
    guard lstat(url.path, &info) == 0 else {
      if errno == ENOENT { return nil }
      throw POSIXError(POSIXErrorCode(rawValue: errno) ?? .EIO)
    }
    var original = stat()
    guard info.st_mode & mode_t(S_IFMT) == mode_t(S_IFREG), info.st_nlink == 1,
      !(stat(source.path, &original) == 0 && info.st_dev == original.st_dev && info.st_ino == original.st_ino)
    else { throw refusal("Screenshot destination must be a regular PNG file, never a link or its source") }
    guard let image = CGImageSourceCreateWithURL(url as CFURL, nil),
      CGImageSourceGetType(image) as String? == UTType.png.identifier,
      CGImageSourceCopyPropertiesAtIndex(image, 0, nil) != nil
    else { throw refusal("Screenshots may replace only existing PNG images") }
    return Version(
      device: info.st_dev, inode: info.st_ino, size: info.st_size,
      modified: info.st_mtimespec.tv_sec, modifiedNanos: info.st_mtimespec.tv_nsec,
      changed: info.st_ctimespec.tv_sec, changedNanos: info.st_ctimespec.tv_nsec)
  }

  private static func refusal(_ message: String) -> OwnerFailure {
    OwnerFailure(kind: .rejected, message: message, reason: CoreErrorCode.invalidRequest.rawValue, opIndex: nil)
  }
}
