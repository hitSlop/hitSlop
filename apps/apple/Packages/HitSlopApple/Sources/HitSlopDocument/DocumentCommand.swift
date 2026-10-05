import Darwin
import Foundation
import HitSlopCore
import HitSlopCoreBinding

/// Native CLI transport. The document interpreter is the shared native Rust owner.
@MainActor public enum DocumentCommand {
  /// One request (`SocketRequest` JSON) for the document it names: forwarded to its live
  /// owner, or run by an owner opened here under the writer lock. `closed` runs it for a
  /// closed document without the lock instead (an export renders saved state). A mutation
  /// needs no epoch: `hello` supplies the owner's. Returns the reply line; a request that
  /// fails before it is sent was not applied (`rejected`).
  public static func run(json: Data, closed: (@MainActor (URL, SocketRequest) async throws -> SocketReply)? = nil) async -> Data {
    let deadline = ContinuousClock.now + Timeouts.admission
    let request: SocketRequest, root: URL
    do {
      (request, root) = try await parse(json)
    } catch { return refused(error) }
    while true {
      do {
        return try await attempt(request, root: root, admissionDeadline: deadline, closed: closed)
      } catch let retry as AdmissionRetry {
        guard ContinuousClock.now < deadline else { return retry.reply }
        try? await Task.sleep(for: Timeouts.admissionRetry)
      } catch { return refused(error) }
    }
  }
  private struct AdmissionRetry: Error { let reply: Data }
  /// A refusal: nothing was sent, so nothing was applied.
  private static func refused(_ error: Error) -> Data {
    let reply = RequestOutcome.socket(error)
    return SocketReply(ok: false, error: reply.error, code: .rejected, reason: reply.reason, opIndex: reply.opIndex).encoded()
  }

  /// The request for the document's canonical path, checked before anything acquires
  /// ownership or creates document state.
  private static func parse(_ json: Data) async throws -> (SocketRequest, URL) {
    guard json.count <= Limits.socketAttachment, var object = try? JSONSerialization.jsonObject(with: json) as? [String: Any],
      let path = object["documentPath"] as? String, let raw = object["method"] as? String,
      let method = SocketRequest.Method(rawValue: raw)
    else { throw SlopFailure("Invalid document command") }
    let root = try await SlopPreparation.run {
      return try SlopFile.documentRoot(URL(fileURLWithPath: path))
    }
    object["documentPath"] = root.path
    // The longest epoch an owner mints, so the size checked is the size sent.
    if method.requiresEpoch { object["epoch"] = String(repeating: "x", count: 128) }
    guard JSONSerialization.isValidJSONObject(object),
      let encoded = try? JSONSerialization.data(withJSONObject: object, options: .withoutEscapingSlashes),
      encoded.count <= (method == .attachmentsPut ? Limits.socketAttachment : Limits.socketRequest),
      Envelope.valid(.socketRequest, encoded), let request = try? SocketRequest(json: object)
    else { throw SlopFailure("Invalid document command") }
    // Operations are JSON text the core parses; only their outer shape is checked here.
    if case .batch(let r) = request, !((try? JSONSerialization.jsonObject(with: Data(r.ops.utf8))) is [Any]) {
      throw SlopFailure("Invalid document command")
    }
    return (request, root)
  }

  private static func attempt(
    _ request: SocketRequest, root: URL, admissionDeadline: ContinuousClock.Instant,
    closed: (@MainActor (URL, SocketRequest) async throws -> SocketReply)?
  ) async throws -> Data {
    let connection: Connection
    if let closed {
      // A live owner answers; otherwise the closed document's saved state does.
      guard let socket = try? liveSocket(for: root) else { return try await closed(root, request).encoded() }
      connection = .socket(socket)
    } else {
      connection = try await connect(root: root, until: admissionDeadline)
    }
    let answer: Data
    do { answer = try await exchange(request, root: root, over: connection) } catch {
      try? await connection.close()
      throw error
    }
    // An owner opened here saved what it applied; its close releases the lock.
    do { try await connection.close() } catch {
      return SocketReply(ok: false, error: error.localizedDescription, code: .unknownOutcome).encoded()
    }
    return answer
  }
  /// `hello` for the owner's epoch, then the request.
  private static func exchange(_ request: SocketRequest, root: URL, over connection: Connection) async throws -> Data {
    let opening = try await send(SocketRequest.hello(.init(documentPath: root.path)), over: connection)
    if opening.reply.code == .closing { throw AdmissionRetry(reply: opening.bytes) }
    // Every request checks the owner is this build's; only a mutation carries its epoch.
    let epoch = try checkedEpoch(opening.reply)
    let request = request.requiresEpoch ? request.with(epoch: epoch) : request
    let answer: (reply: SocketReply, bytes: Data)
    do { answer = try await send(request, over: connection) } catch {
      // Lost after sending: a mutation may have been applied.
      return SocketReply(ok: false, error: error.localizedDescription, code: request.requiresEpoch ? .unknownOutcome : .rejected).encoded()
    }
    if answer.reply.code == .closing { throw AdmissionRetry(reply: answer.bytes) }
    return answer.bytes
  }

  /// Takes a closed document's writer lock with an owner opened here, or finds its live
  /// owner's socket; discovery is consulted only after the lock is busy. A busy lock with
  /// no socket yet (an owner still opening) retries until `deadline`. Never bypasses a busy
  /// lock.
  private static func connect(root: URL, until deadline: ContinuousClock.Instant) async throws -> Connection {
    while true {
      do { return .owner(try await SlopPreparation.run { try DocumentOwner(url: root) }) }
      catch {
        guard error is DocumentLocked else { throw error }
        do { return .socket(try liveSocket(for: root)) }
        catch {
          guard ContinuousClock.now < deadline else { throw error }
          try await Task.sleep(for: Timeouts.admissionRetry)
        }
      }
    }
  }

  static func liveSocket(for root: URL) throws -> String {
    do {
      guard let json = try storeCall({ try liveDiscovery(path: root.path) }) else { throw SlopFailure("No live session yet") }
      let bytes = Data(json.utf8)
      guard Envelope.valid(.socketDiscovery, bytes),
        let value = try JSONSerialization.jsonObject(with: bytes) as? [String: Any]
      else { throw SlopFailure("Invalid live session discovery") }
      let discovery = try SocketDiscovery(json: value)
      guard discovery.documentPath == root.path else { throw SlopFailure("Invalid live session discovery") }
      return discovery.socket
    } catch {
      throw SlopFailure(
        "Writer is busy without a ready session; retry later. \(error.localizedDescription)")
    }
  }

  private static func checkedEpoch(_ hello: SocketReply) throws -> String {
    guard hello.ok else { throw SlopFailure(hello.error ?? "Cannot open session") }
    guard hello.coreBuildId == DocumentOwner.coreBuildID else {
      // The helper ships in the app bundle, so a mismatch means the running app predates an update.
      throw SlopFailure("hitSlop was updated while this document was open. Quit and reopen hitSlop, then try again")
    }
    guard let epoch = hello.epoch else { throw SlopFailure("Cannot open session") }
    return epoch
  }

  private enum Connection: Sendable {
    case owner(DocumentOwner)
    case socket(String)

    func close() async throws {
      if case .owner(let owner) = self { try await owner.close() }
    }
  }

  /// An owner opened here answers directly. A live owner's `hello` is checked against the
  /// contract before its core build is trusted; after that the owner is this build's, so
  /// both paths read a reply the same way, whatever its size, and the CLI validates it.
  private static func send(_ request: SocketRequest, over connection: Connection)
    async throws -> (reply: SocketReply, bytes: Data)
  {
    let replyBytes: Data
    switch connection {
    case .owner(let owner): replyBytes = await owner.request(request)
    case .socket(let socket):
      let encoded = try JSONSerialization.data(withJSONObject: request.json, options: .withoutEscapingSlashes)
      replyBytes = try await SlopPreparation.run { try SocketClient.call(path: socket, request: encoded) }
      if case .hello = request, !Envelope.valid(.socketReply, replyBytes) { throw SlopFailure("Invalid socket response") }
    }
    guard let object = try JSONSerialization.jsonObject(with: replyBytes) as? [String: Any]
    else { throw SlopFailure("Invalid socket response") }
    return (try SocketReply(json: object), replyBytes)
  }
}
enum SocketClient {
  static func call(path: String, request: Data) throws -> Data {
    // Callers bound each method's request; this bounds every message.
    guard request.count <= Limits.socketAttachment else { throw SlopFailure("Oversized socket request") }
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    guard fd >= 0 else { throw SlopFailure("Cannot create client socket") }
    defer { Darwin.close(fd) }
    _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
    var timeout = timeval(tv_sec: Int(Timeouts.client.components.seconds), tv_usec: 0)
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
