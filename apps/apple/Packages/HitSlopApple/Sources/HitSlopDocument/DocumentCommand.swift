import Darwin
import Foundation
import HitSlopCore

/// Native CLI transport. The document interpreter is the shared native Rust owner.
@MainActor public enum DocumentCommand {
  /// One command for the document at `url`: forwarded to its live owner, or run by an
  /// owner opened here under the writer lock. `make` builds the request for the package's
  /// path; `hello` supplies the epoch a mutation needs. Reads return the projected value
  /// unless `snapshot` requests the complete schema and owner frame. Returns JSON.
  public static func run(url: URL, snapshot: Bool = false, _ make: @escaping @Sendable (_ documentPath: String) -> SocketRequest) async throws -> Data {
    let deadline = ContinuousClock.now + .seconds(2)
    // The package is validated once per command, off MainActor; retries reuse it.
    let package = try await SlopPreparation.run {
      try SlopLocalDocument.requireLocal(url)
      return try SlopPackage(rootURL: url)
    }
    guard !SlopTemplateLocation.isMaster(package.rootURL) else {
      throw failure(SlopTemplateLocation.writableCopyRequired)
    }
    let request = make(package.rootURL.path)
    // Refuse a malformed command before acquiring ownership or creating document state.
    try validate(request)
    while true {
      do {
        return try await attempt(request, package: package, admissionDeadline: deadline, snapshot: snapshot)
      } catch let retry as AdmissionRetry {
        guard ContinuousClock.now < deadline else {
          throw failure(retry.message)
        }
        try await Task.sleep(for: .milliseconds(50))
      }
    }
  }
  private struct AdmissionRetry: Error {
    let message: String
    init(reply: SocketReply) {
      message = (reply.error ?? "Document is closing") + DocumentCommand.retryHint(reply)
    }
  }
  private static func validate(_ request: SocketRequest) throws {
    let probe = request.requiresEpoch ? request.with(epoch: String(repeating: "x", count: 128)) : request
    guard JSONSerialization.isValidJSONObject(probe.json),
      let encoded = try? JSONSerialization.data(withJSONObject: probe.json, options: .withoutEscapingSlashes),
      encoded.count <= requestLimit(request.method), Envelope.valid(.socketRequest, encoded)
    else { throw failure("Invalid document command") }
    // Operations are JSON text the core parses; only their outer shape is checked here.
    let shaped: Bool
    switch request {
    case .batch(let r): shaped = (try? JSONSerialization.jsonObject(with: Data(r.ops.utf8))) is [Any]
    default: shaped = true
    }
    guard shaped else { throw failure("Invalid document command") }
  }

  private static func attempt(
    _ request: SocketRequest, package: SlopPackage, admissionDeadline: ContinuousClock.Instant, snapshot: Bool
  ) async throws -> Data {
    let root = package.rootURL
    let connection: Connection
    switch try await connect(root: root, until: admissionDeadline, own: { try DocumentOwner(package: package) }) {
    case .owned(let owner): connection = .owner(owner)
    case .live(let socket): connection = .socket(socket)
    }
    do {
      let hello = SocketRequest.hello(.init(documentPath: root.path))
      let opening = try await send(hello, over: connection)
      if opening.code == .closing { throw AdmissionRetry(reply: opening) }
      let current = try checkedEpoch(opening)
      let request = request.requiresEpoch ? request.with(epoch: current) : request
      let reply: SocketReply
      do { reply = try await send(request, over: connection) } catch {
        throw failure(error.localizedDescription + (request.requiresEpoch ? retryHint(nil) : ""))
      }
      if reply.code == .closing { throw AdmissionRetry(reply: reply) }
      guard reply.ok else {
        throw failure(
          (reply.error ?? "Document operation failed")
            + (request.requiresEpoch ? retryHint(reply) : ""))
      }
      guard let state = reply.state else { throw failure("Missing document state in response") }
      // Edits also report the inserted row IDs (minted IDs are new on every run) and the
      // owner sequence, so an agent can address new rows without another read.
      let output: Any
      if request.method == .batch {
        output = ["ids": reply.ids ?? [], "sequence": reply.sequence ?? 0, "value": state]
      } else if request.method == .get, !snapshot {
        guard let snapshot = state as? [String: Any], let frame = snapshot["state"] as? [String: Any], let value = frame["value"]
        else { throw failure("Missing document value in response") }
        output = value
      } else { output = state }
      let data = try formatted(output)
      try await connection.close()
      return data
    } catch {
      try? await connection.close()
      throw error
    }
  }

  /// Coded refusals were never applied. Only transport loss or "failed" leaves the outcome unknown.
  nonisolated private static func retryHint(_ reply: SocketReply?) -> String {
    switch reply?.code {
    case .rejected, .unavailable: return "\nNot applied."
    case .sessionChanged, .closing: return "\nNot applied. Run slop get before issuing another edit."
    case .failed, nil: return "\nOutcome unknown. Run slop get before issuing another edit."
    }
  }

  /// Discovery is consulted only after the caller observes a busy OS writer lock.
  public enum Access<Owned: Sendable>: Sendable { case owned(Owned), live(socket: String) }
  /// Takes a closed document's writer lock through `own`, or finds its live owner's
  /// socket. A busy lock with no socket yet (an owner still opening) retries until
  /// `deadline`. Never bypasses a busy lock.
  public static func connect<Owned: Sendable>(
    root: URL, until deadline: ContinuousClock.Instant, own: @escaping @Sendable () throws -> Owned
  ) async throws -> Access<Owned> {
    while true {
      do { return .owned(try await SlopPreparation.run(own)) }
      catch {
        guard error is DocumentLocked else { throw error }
        do { return .live(socket: try liveSocket(for: root)) }
        catch {
          guard ContinuousClock.now < deadline else { throw error }
          try await Task.sleep(for: .milliseconds(50))
        }
      }
    }
  }

  static func liveSocket(for root: URL) throws -> String {
    let url = root.appendingPathComponent("state/host.lock")
    do {
      let bytes = try SlopFile.read(url, within: root, maximumBytes: 16384)
      guard bytes.count <= 16384,
        Envelope.valid(.socketDiscovery, bytes),
        let value = try JSONSerialization.jsonObject(with: bytes) as? [String: Any]
      else { throw failure("Invalid live session discovery") }
      let discovery = try SocketDiscovery(json: value)
      guard discovery.documentPath == root.path else { throw failure("Invalid live session discovery") }
      return discovery.socket
    } catch {
      throw failure(
        "Writer is busy without a ready session; retry later. \(error.localizedDescription)")
    }
  }

  public static func exportLive(root: URL, socket: String, format: ExportFormat, output: URL) async throws
  {
    let base = SocketRequest.hello(.init(documentPath: root.path))
    let hello = try await send(base, over: .socket(socket))
    let epoch = try checkedEpoch(hello)
    let request = SocketRequest.export(.init(
      documentPath: root.path, epoch: epoch, format: format, output: output.path))
    let reply: SocketReply
    do { reply = try await send(request, over: .socket(socket)) } catch {
      throw failure(
        "Export outcome may be unknown; inspect the destination before retrying. \(error.localizedDescription)"
      )
    }
    guard reply.ok, reply.output == output.path else {
      throw failure(reply.error ?? "Invalid export response")
    }
  }

  private static func checkedEpoch(_ hello: SocketReply) throws -> String {
    guard hello.ok else { throw failure(hello.error ?? "Cannot open session") }
    guard hello.coreBuildId == DocumentOwner.coreBuildID else {
      throw failure("hitSlop.app and the native helper embed different document cores; install matching versions and reopen the app")
    }
    guard let epoch = hello.epoch else { throw failure("Cannot open session") }
    return epoch
  }

  private enum Connection {
    case owner(DocumentOwner)
    case socket(String)

    func close() async throws {
      if case .owner(let owner) = self { try await owner.close() }
    }
  }

  /// An owner opened here answers directly; a live owner's reply is checked as it arrives.
  private static func send(_ request: SocketRequest, over connection: Connection)
    async throws -> SocketReply
  {
    let replyBytes: Data
    switch connection {
    case .owner(let owner): replyBytes = await owner.request(request)
    case .socket(let socket):
      let encoded = try JSONSerialization.data(withJSONObject: request.json, options: .withoutEscapingSlashes)
      replyBytes = try await withCheckedThrowingContinuation { continuation in
        DispatchQueue.global(qos: .userInitiated).async {
          continuation.resume(with: Result { try SocketClient.call(path: socket, request: encoded) })
        }
      }
      guard Envelope.valid(.socketReply, replyBytes) else { throw failure("Invalid socket response") }
    }
    guard let object = try JSONSerialization.jsonObject(with: replyBytes) as? [String: Any]
    else { throw failure("Invalid socket response") }
    return try SocketReply(json: object)
  }

  /// Command output and theme files: pretty and key-sorted, so the same state is always
  /// the same bytes.
  nonisolated public static func formatted(_ object: Any) throws -> Data {
    try JSONSerialization.data(withJSONObject: object, options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes])
  }
  private static func requestLimit(_ method: SocketRequest.Method) -> Int {
    method == .attachmentsPut ? Limits.socketAttachment : Limits.socketRequest
  }
}
enum SocketClient {
  static func call(path: String, request: Data) throws -> Data {
    // Callers bound each method's request; this bounds every message.
    guard request.count <= Limits.socketAttachment else { throw failure("Oversized socket request") }
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    guard fd >= 0 else { throw failure("Cannot create client socket") }
    defer { Darwin.close(fd) }
    _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
    var timeout = timeval(tv_sec: 35, tv_usec: 0)
    var noPipe: Int32 = 1
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
    setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, socklen_t(MemoryLayout<timeval>.size))
    setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noPipe, socklen_t(MemoryLayout<Int32>.size))
    var address = sockaddr_un()
    address.sun_family = sa_family_t(AF_UNIX)
    let chars = Array(path.utf8CString)
    guard chars.count <= MemoryLayout.size(ofValue: address.sun_path) else {
      throw failure("Socket path too long")
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
      throw failure("Live document unavailable; writer lock remains authoritative")
    }
    let payload = request + [10]
    try payload.withUnsafeBytes { bytes in
      var offset = 0
      while offset < bytes.count {
        let n = Darwin.write(fd, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
        guard n > 0 else { throw failure("Socket write failed; outcome may be unknown") }
        offset += n
      }
    }
    var result = Data()
    var buffer = [UInt8](repeating: 0, count: 8192)
    while result.count <= Limits.socketAttachment {
      let count = read(fd, &buffer, buffer.count)
      guard count > 0 else {
        throw failure("Host disconnected or timed out; outcome may be unknown")
      }
      let start = result.count
      result.append(contentsOf: buffer.prefix(count))
      // Prior chunks contain no delimiter; keep large replies linear to read.
      if let delimiter = buffer.prefix(count).firstIndex(of: 10) {
        let end = start + delimiter
        guard end <= Limits.socketAttachment else { throw failure("Oversized socket response") }
        return result.prefix(upTo: end)
      }
    }
    throw failure("Oversized socket response")
  }
}
