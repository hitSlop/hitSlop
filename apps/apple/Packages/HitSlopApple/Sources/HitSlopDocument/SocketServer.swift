import Darwin
import Foundation
import HitSlopCore

/// Bounded nonblocking newline JSON. Socket state is confined to one I/O queue.
final class SocketServer: @unchecked Sendable {
  let path: String
  private let queue = DispatchQueue(label: "hitslop.socket")
  private let source: DispatchSourceRead
  private var clients: [Int32: Connection] = [:]
  private var stopped = false
  /// Answers one request with its reply line (without the newline).
  typealias Handler = @Sendable (SocketRequest, NativeCommandDeadline) async -> Data
  private let handle: Handler

  init(handle: @escaping Handler) throws {
    self.handle = handle
    let directory = "/tmp/hitslop-\(getuid())"
    if mkdir(directory, 0o700) != 0 && errno != EEXIST {
      throw SlopFailure("Cannot create socket directory")
    }
    var info = stat()
    guard lstat(directory, &info) == 0, info.st_uid == getuid(), info.st_mode & S_IFMT == S_IFDIR
    else {
      throw SlopFailure("Unsafe socket directory")
    }
    guard chmod(directory, 0o700) == 0 else { throw SlopFailure("Cannot protect socket directory") }
    path = directory + "/" + UUID().uuidString + ".sock"
    let fd = socket(AF_UNIX, SOCK_STREAM, 0)
    guard fd >= 0 else { throw SlopFailure("Cannot create socket") }
    _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
    var address = sockaddr_un()
    address.sun_family = sa_family_t(AF_UNIX)
    let bytes = Array(path.utf8CString)
    withUnsafeMutableBytes(of: &address.sun_path) {
      $0.copyBytes(from: bytes.map { UInt8(bitPattern: $0) })
    }
    let bound = withUnsafePointer(to: &address) { pointer in
      pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) {
        Darwin.bind(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
      }
    }
    guard bound == 0, listen(fd, 16) == 0, fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else {
      Darwin.close(fd)
      unlink(path)
      throw SlopFailure("Cannot listen on socket")
    }
    chmod(path, 0o600)
    source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
    source.setCancelHandler { Darwin.close(fd) }
    source.setEventHandler { [weak self] in self?.acceptClients(fd) }
    source.resume()
  }

  deinit {
    source.cancel()
    unlink(path)
  }

  func stop() {
    queue.async { [self] in
      guard !stopped else { return }
      stopped = true
      source.cancel()
      for client in Array(clients.values) { client.close() }
      clients.removeAll()
      unlink(path)
    }
  }

  private func acceptClients(_ listener: Int32) {
    guard !stopped else { return }
    while true {
      let fd = accept(listener, nil, nil)
      if fd < 0 {
        if errno == EINTR { continue }
        return
      }
      guard clients.count < 16, fcntl(fd, F_SETFL, O_NONBLOCK) == 0 else {
        Darwin.close(fd)
        continue
      }
      _ = fcntl(fd, F_SETFD, FD_CLOEXEC)
      var noPipe: Int32 = 1
      setsockopt(fd, SOL_SOCKET, SO_NOSIGPIPE, &noPipe, socklen_t(MemoryLayout<Int32>.size))
      let token = UUID()
      let client = Connection(
        token: token, fd: fd, queue: queue,
        request: { [weak self] bytes in
          self?.dispatch(bytes, fd: fd, token: token)
        }, finished: { [weak self] in self?.clients.removeValue(forKey: fd) })
      clients[fd] = client
      client.start()
    }
  }

  /// Runs on the socket queue: parsing and validation happen here, never on the main actor.
  private func dispatch(_ bytes: Data, fd: Int32, token: UUID) {
    guard !stopped, clients[fd]?.token == token else { return }
    let deadline = NativeCommandDeadline()
    guard let object = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any],
      bytes.count <= Limits.socketRequest || object["method"] as? String == SocketRequest.Method.attachmentsPut.rawValue,
      Envelope.valid(.socketRequest, bytes),
      let request = try? SocketRequest(json: object)
    else {
      return respond(RequestOutcome.socket(OwnerError.rejected("Invalid socket request")).encoded(), fd: fd, token: token)
    }
    let handle = handle
    Task { [weak self] in
      // A queued request may expire while the owner is busy; never start it late.
      let reply = (try? deadline.check()) == nil
        ? RequestOutcome.socket(OwnerError.closing).encoded()
        : await handle(request, deadline)
      guard let server = self else { return }
      server.queue.async { server.respond(reply, fd: fd, token: token) }
    }
  }

  /// The client validates the reply, which is as large as the document it carries.
  private func respond(_ reply: Data, fd: Int32, token: UUID) {
    guard !stopped, let client = clients[fd], client.token == token else { return }
    client.send(reply)
  }
}

private final class Connection: @unchecked Sendable {
  let token: UUID
  private let fd: Int32
  private let queue: DispatchQueue
  private let reader: DispatchSourceRead
  private var writer: DispatchSourceWrite?
  private var timer: DispatchWorkItem?
  private var input = Data()
  private var output = Data()
  private var offset = 0
  private var dispatched = false
  private var closed = false
  private let request: (Data) -> Void
  private let finished: () -> Void

  init(
    token: UUID, fd: Int32, queue: DispatchQueue, request: @escaping (Data) -> Void,
    finished: @escaping () -> Void
  ) {
    self.token = token
    self.fd = fd
    self.queue = queue
    self.request = request
    self.finished = finished
    reader = DispatchSource.makeReadSource(fileDescriptor: fd, queue: queue)
    reader.setCancelHandler { Darwin.close(fd) }
    reader.setEventHandler { [weak self] in self?.readRequest() }
  }

  deinit {
    timer?.cancel()
    writer?.cancel()
    reader.cancel()
  }

  func start() {
    reader.resume()
    expire(after: Timeouts.requestRead)
  }

  private func expire(after timeout: Duration) {
    timer?.cancel()
    let work = DispatchWorkItem { [weak self] in self?.close() }
    timer = work
    queue.asyncAfter(deadline: .now() + timeout.dispatch, execute: work)
  }

  private func readRequest() {
    guard !closed else { return }
    var buffer = [UInt8](repeating: 0, count: 8192)
    while true {
      let count = Darwin.read(fd, &buffer, buffer.count)
      if count < 0 {
        if errno == EINTR { continue }
        if errno == EAGAIN || errno == EWOULDBLOCK { return }
        close()
        return
      }
      if count == 0 {
        close()
        return
      }
      guard !dispatched else {
        close()
        return
      }
      let start = input.count
      input.append(contentsOf: buffer.prefix(count))
      // Scan each byte once, including for bulk imports and attachment payloads.
      if let delimiter = buffer.prefix(count).firstIndex(of: 10) {
        let end = start + delimiter
        guard end <= Limits.socketAttachment else {
          close()
          return
        }
        dispatched = true
        expire(after: Timeouts.connection)
        request(Data(input.prefix(upTo: end)))
        input.removeAll()
        return
      }
      if input.count > Limits.socketAttachment {
        close()
        return
      }
    }
  }

  func send(_ data: Data) {
    guard !closed, writer == nil else { return }
    output = data + [10]
    let source = DispatchSource.makeWriteSource(fileDescriptor: fd, queue: queue)
    writer = source
    source.setEventHandler { [weak self] in self?.writeReply() }
    source.resume()
  }

  private func writeReply() {
    guard !closed else { return }
    while offset < output.count {
      let count = output.withUnsafeBytes {
        Darwin.write(fd, $0.baseAddress!.advanced(by: offset), $0.count - offset)
      }
      if count < 0 {
        if errno == EINTR { continue }
        if errno == EAGAIN || errno == EWOULDBLOCK { return }
        close()
        return
      }
      guard count > 0 else {
        close()
        return
      }
      offset += count
    }
    close()
  }

  func close() {
    guard !closed else { return }
    closed = true
    timer?.cancel()
    writer?.cancel()
    reader.cancel()
    finished()
  }
}
