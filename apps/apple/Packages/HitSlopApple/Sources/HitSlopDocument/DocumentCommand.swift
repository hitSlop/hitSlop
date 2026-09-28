import Darwin
import Foundation
import HitSlopCore

/// Native CLI transport. The document interpreter is the shared native Rust owner.
@MainActor public enum DocumentCommand {
  public static func run(
    method: String, url: URL, operation: Data? = nil, operations: Data? = nil,
    themeValues: Data? = nil, themeToken: String? = nil,
    attachmentBytes: Data? = nil, attachmentID: String? = nil
  )
    async throws -> Data
  {
    let deadline = ContinuousClock.now + .seconds(2)
    while true {
      do {
        return try await runAttempt(method: method, url: url, operation: operation, operations: operations,
          themeValues: themeValues, themeToken: themeToken, attachmentBytes: attachmentBytes,
          attachmentID: attachmentID, admissionDeadline: deadline)
      } catch let retry as AdmissionRetry {
        guard ContinuousClock.now < deadline else {
          throw failure((retry.reply.error ?? "Document is closing") + retryHint(retry.reply))
        }
        try await Task.sleep(for: .milliseconds(50))
      }
    }
  }
  private struct AdmissionRetry: Error { let reply: SocketReply }

  private static func runAttempt(
    method: String, url: URL, operation: Data? = nil, operations: Data? = nil,
    themeValues: Data? = nil, themeToken: String? = nil,
    attachmentBytes: Data? = nil, attachmentID: String? = nil, admissionDeadline: ContinuousClock.Instant
  )
    async throws -> Data
  {
    try SlopLocalDocument.requireLocal(url)
    let package = try SlopPackage(rootURL: url)
    let root = package.rootURL
    guard let command = SocketRequest.Method(rawValue: method) else { throw failure("Invalid document command") }
    if command == .schema {
      return try SlopFile.read(package.dataSchemaURL, within: root, maximumBytes: 1_048_576)
    }
    let templatesRoot = ProcessInfo.processInfo.environment["HITSLOP_TEMPLATES_ROOT"]
      .map { URL(fileURLWithPath: $0) } ?? SlopTemplateLocation.defaultTemplatesRoot
    guard !SlopTemplateLocation.isManagedTemplatePackage(root),
      !SlopTemplateLocation.isManagedTemplatePackage(root, templatesRoot: templatesRoot) else {
      throw failure("Create a writable copy of this template first")
    }
    var input: [String: Any] = [
      "id": UUID().uuidString, "method": method, "documentPath": root.path,
    ]
    if let themeValues { input["values"] = try JSONSerialization.jsonObject(with: themeValues) }
    if let themeToken { input["token"] = themeToken }
    if let operation { input["op"] = try JSONSerialization.jsonObject(with: operation) }
    if let operations { input["ops"] = try JSONSerialization.jsonObject(with: operations) }
    if let attachmentBytes { input["bytes"] = attachmentBytes.base64EncodedString() }
    if let attachmentID { input["attachmentID"] = attachmentID }
    // Validate before acquiring ownership or creating any document state.
    // hello supplies the real epoch before any epoch-requiring request is dispatched.
    if command.requiresEpoch { input["epoch"] = String(repeating: "x", count: 128) }
    guard PlatformContract.valid(input, against: socketRequestSchema) else {
      throw failure("Invalid document command")
    }
    guard try JSONSerialization.data(withJSONObject: input).count <= requestLimit(command) else {
      throw failure("Document command exceeds the request size limit")
    }
    var request = try SocketRequest(json: input)
    var engine: DocumentOwner?
    var socket: String?
    let readyDeadline = admissionDeadline
    while true {
      do { engine = try DocumentOwner(package: package); break }
      catch {
        guard DocumentWriterLock.isBusy(error) else { throw error }
        do { socket = try liveSocket(for: root); break }
        catch {
          guard ContinuousClock.now < readyDeadline else { throw error }
          try await Task.sleep(for: .milliseconds(50))
        }
      }
    }
    do {
      if request.requiresEpoch {
        do {
          let hello = SocketRequest.hello(.init(id: UUID().uuidString, documentPath: root.path))
          let opening = try await send(hello, engine: engine, socket: socket)
          if opening.code == .closing { throw AdmissionRetry(reply: opening) }
          guard opening.ok, let current = opening.epoch else {
            throw failure(opening.error ?? "Cannot open session")
          }
          request = request.with(epoch: current)
        }
      }
      let reply: SocketReply
      do { reply = try await send(request, engine: engine, socket: socket) } catch {
        throw failure(error.localizedDescription + (request.requiresEpoch ? retryHint(nil) : ""))
      }
      if reply.code == .closing { throw AdmissionRetry(reply: reply) }
      guard reply.ok else {
        throw failure(
          (reply.error ?? "Document operation failed")
            + (request.requiresEpoch ? retryHint(reply) : ""))
      }
      guard let state = reply.state else { throw failure("Missing document state in response") }
      let data = try JSONSerialization.data(
        withJSONObject: state, options: [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes])
      try await engine?.close()
      return data
    } catch {
      try? await engine?.close()
      throw error
    }
  }

  /// Coded refusals were never applied. Only transport loss or "failed" leaves the outcome unknown.
  private static func retryHint(_ reply: SocketReply?) -> String {
    switch reply?.code {
    case .rejected, .unavailable: return "\nNot applied."
    case .sessionChanged, .closing: return "\nNot applied. Run slop get before issuing another edit."
    case .failed, nil: return "\nOutcome unknown. Run slop get before issuing another edit."
    }
  }

  /// Discovery is consulted only after the caller observes a busy OS writer lock.
  public static func liveSocket(for root: URL) throws -> String {
    let url = root.appendingPathComponent("state/host.lock")
    do {
      let bytes = try SlopFile.read(url, within: root, maximumBytes: 16384)
      guard bytes.count <= 16384,
        let value = try JSONSerialization.jsonObject(with: bytes) as? [String: Any],
        PlatformContract.valid(value, against: socketDiscoverySchema)
      else { throw failure("Invalid live session discovery") }
      let discovery = try SocketDiscovery(json: value)
      guard discovery.documentPath == root.path else { throw failure("Invalid live session discovery") }
      return discovery.socket
    } catch {
      throw failure(
        "Writer is busy without a ready session; retry later. \(error.localizedDescription)")
    }
  }

  public static func exportLive(root: URL, socket: String, format: String, output: URL) async throws
  {
    let base = SocketRequest.hello(.init(id: UUID().uuidString, documentPath: root.path))
    let hello = try await send(base, engine: nil, socket: socket)
    guard hello.ok, let epoch = hello.epoch else {
      throw failure(hello.error ?? "Cannot open session")
    }
    guard let format = SocketExportRequestFormat(rawValue: format) else { throw failure("Invalid export format") }
    let request = SocketRequest.export(.init(
      id: UUID().uuidString, documentPath: root.path, epoch: epoch, format: format, output: output.path))
    let reply: SocketReply
    do { reply = try await send(request, engine: nil, socket: socket) } catch {
      throw failure(
        "Export outcome may be unknown; inspect the destination before retrying. \(error.localizedDescription)"
      )
    }
    guard reply.ok, reply.output == output.path else {
      throw failure(reply.error ?? "Invalid export response")
    }
  }

  private static func send(_ request: SocketRequest, engine: DocumentOwner?, socket: String?)
    async throws -> SocketReply
  {
    guard PlatformContract.valid(request.json, against: socketRequestSchema) else {
      throw failure("Invalid socket request")
    }
    let encoded = try JSONSerialization.data(withJSONObject: request.json)
    guard encoded.count <= requestLimit(request.method) else {
      throw failure("Document command exceeds the request size limit")
    }
    let object: [String: Any]
    if let engine {
      object = await engine.request(request).json
    } else {
      guard let socket else { throw failure("Missing live session") }
      let bytes = encoded
      let result: Data = try await withCheckedThrowingContinuation { continuation in
        DispatchQueue.global(qos: .userInitiated).async {
          continuation.resume(with: Result { try SocketClient.call(path: socket, request: bytes) })
        }
      }
      guard let value = try JSONSerialization.jsonObject(with: result) as? [String: Any] else {
        throw failure("Invalid socket response")
      }
      object = value
    }
    guard PlatformContract.valid(object, against: socketReplySchema) else {
      throw failure("Invalid socket response")
    }
    return try SocketReply(json: object)
  }

  private static func requestLimit(_ method: SocketRequest.Method) -> Int {
    method == .attachmentsPut ? 16 * 1024 * 1024 : 1_048_576
  }
}
enum SocketClient {
  static func call(path: String, request: Data) throws -> Data {
    guard request.count <= 16 * 1024 * 1024 else { throw failure("Oversized socket request") }
    if request.count > 1_048_576 {
      guard let value = try? JSONSerialization.jsonObject(with: request) as? [String: Any],
        ["attachments.put"].contains(value["method"] as? String ?? "") else { throw failure("Oversized socket request") }
    }
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
    var payload = request
    payload.append(10)
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
    while result.count <= 16 * 1024 * 1024 {
      let count = read(fd, &buffer, buffer.count)
      guard count > 0 else {
        throw failure("Host disconnected or timed out; outcome may be unknown")
      }
      let start = result.count
      result.append(contentsOf: buffer.prefix(count))
      // Prior chunks contain no delimiter; keep large replies linear to read.
      if let delimiter = buffer.prefix(count).firstIndex(of: 10) {
        let end = start + delimiter
        guard end <= 16 * 1024 * 1024 else { throw failure("Oversized socket response") }
        return result.prefix(upTo: end)
      }
    }
    throw failure("Oversized socket response")
  }
}
