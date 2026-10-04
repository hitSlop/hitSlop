import Foundation
import Darwin

/// Reads a file a command names (a batch, an attachment to import): a regular file, never a
/// link or a device, within a size limit checked before and during the read.
public enum CommandInput {
    public static func read(_ url: URL, maximumBytes: Int) throws -> Data {
        // NONBLOCK prevents a FIFO from hanging before fstat can reject it.
        let descriptor = Darwin.open(url.path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        guard descriptor >= 0 else { throw SlopError.invalid("cannot open \(url.lastPathComponent)") }
        defer { Darwin.close(descriptor) }
        var info = stat()
        guard fstat(descriptor, &info) == 0, info.st_mode & S_IFMT == S_IFREG,
              info.st_size >= 0, info.st_size <= maximumBytes else {
            throw SlopError.invalid("\(url.lastPathComponent) must be a regular file within its size limit")
        }
        var result = Data(), buffer = [UInt8](repeating: 0, count: 64 * 1024)
        while true {
            let count = Darwin.read(descriptor, &buffer, min(buffer.count, maximumBytes - result.count + 1))
            if count < 0, errno == EINTR { continue }
            guard count >= 0 else { throw SlopError.invalid("cannot read \(url.lastPathComponent)") }
            if count == 0 { return result }
            guard result.count + count <= maximumBytes else {
                throw SlopError.invalid("\(url.lastPathComponent) exceeds its size limit")
            }
            result.append(contentsOf: buffer.prefix(count))
        }
    }
}
