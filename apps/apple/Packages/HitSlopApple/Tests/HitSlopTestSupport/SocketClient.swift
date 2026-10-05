import Darwin
import Foundation
import HitSlopCore

// Raw wire access for native stale-epoch integration tests; routing lives in Rust.
public enum SocketClient {
  public static func call(path: String, request: Data) throws -> Data {
    // Callers bound each method's request; this bounds every message.
    guard request.count <= Limits.socketAttachment else { throw SlopFailure("Oversized socket request") }
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    guard fd >= 0 else { throw SlopFailure("Cannot create client socket") }
    defer { Darwin.close(fd) }
    _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
    var timeout = timeval(tv_sec: 5, tv_usec: 0)
    var noPipe: Int32 = 1
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
    setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noPipe, socklen_t(MemoryLayout<Int32>.size))
    var address = sockaddr_un()
    address.sun_family = sa_family_t(AF_UNIX)
    let chars = Array(path.utf8CString)
    guard chars.count <= MemoryLayout.size(ofValue: address.sun_path) else {
      throw SlopFailure("Socket path too long")
    }
    withUnsafeMutableBytes(of: &address.sun_path) {
      $0.copyBytes(from: chars.map { UInt8(bitPattern: $0) })
    }
    let connected = withUnsafePointer(to: &address) {
      $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
        connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
      }
    }
    guard connected == 0 else {
      throw SlopFailure("Live document unavailable; writer lock remains authoritative")
    }
    let payload = request + [10]
    try payload.withUnsafeBytes { bytes in
      var offset = 0
      while offset < bytes.count {
        let n = Darwin.write(fd, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
        guard n > 0 else { throw SlopFailure("Socket write failed; outcome may be unknown") }
        offset += n
      }
    }
    // A reply is as large as the document it carries; the owner is this build's own app.
    var result = Data()
    var buffer = [UInt8](repeating: 0, count: 64 * 1024)
    while true {
      let count = read(fd, &buffer, buffer.count)
      guard count > 0 else {
        throw SlopFailure("Host disconnected or timed out; outcome may be unknown")
      }
      let start = result.count
      result.append(contentsOf: buffer.prefix(count))
      // Prior chunks contain no delimiter; keep large replies linear to read.
      if let delimiter = buffer.prefix(count).firstIndex(of: 10) {
        return result.prefix(upTo: start + delimiter)
      }
    }
  }
}
