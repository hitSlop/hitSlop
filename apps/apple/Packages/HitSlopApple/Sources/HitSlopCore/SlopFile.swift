import Foundation
import Darwin

/// Reads an untrusted package entry without following links or reopening a checked path.
public enum SlopFile {
    public static let maximumBytes = Limits.packageFile

    public static func read(_ url: URL, within root: URL, maximumBytes: Int = maximumBytes) throws -> Data {
        let rootPath = root.standardizedFileURL.path
        let path = url.standardizedFileURL.path
        guard path.hasPrefix(rootPath + "/") else { throw SlopPackageError.invalid("file escapes its package") }
        let parts = String(path.dropFirst(rootPath.count + 1)).split(separator: "/").map(String.init)
        guard !parts.isEmpty else { throw SlopPackageError.invalid("expected a file") }
        var directory = Darwin.open(rootPath, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard directory >= 0 else { throw SlopPackageError.invalid("cannot open package directory") }
        defer { Darwin.close(directory) }
        for part in parts.dropLast() {
            let next = openat(directory, part, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
            guard next >= 0 else { throw SlopPackageError.invalid("unsafe resource directory") }
            Darwin.close(directory); directory = next
        }
        // NONBLOCK prevents a FIFO from hanging before fstat can reject it.
        let descriptor = openat(directory, parts.last!, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        guard descriptor >= 0 else { throw SlopPackageError.invalid("cannot open resource") }
        defer { Darwin.close(descriptor) }
        var info = stat()
        guard fstat(descriptor, &info) == 0, info.st_mode & S_IFMT == S_IFREG,
              info.st_size >= 0, info.st_size <= maximumBytes else {
            throw SlopPackageError.invalid("resource must be a regular file within its size limit")
        }
        var result = Data(), buffer = [UInt8](repeating: 0, count: 64 * 1024)
        while true {
            let count = Darwin.read(descriptor, &buffer, min(buffer.count, maximumBytes - result.count + 1))
            if count < 0, errno == EINTR { continue }
            guard count >= 0 else { throw SlopPackageError.invalid("cannot read resource") }
            if count == 0 { return result }
            guard result.count + count <= maximumBytes else { throw SlopPackageError.invalid("resource exceeds its size limit") }
            result.append(contentsOf: buffer.prefix(count))
        }
    }
}
