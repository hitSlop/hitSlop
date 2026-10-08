// One edit path: whoever holds writer.lock runs the Rust owner. A live owner serves
// a Unix socket; everyone else forwards to it. A closed document is owned in-process
// by the CLI itself. No WebKit is involved anywhere in this module.
import Darwin
import Foundation
import NativeOwner

public struct Discovery: Codable {
  public let socket: String
  public let documentPath: String
  public static func url(_ root: URL) -> URL { root.appendingPathComponent("host.json") }
  /// Consulted only after observing a busy lock; a stale file is harmless because a
  /// free lock is always taken first.
  public static func read(_ root: URL) -> Discovery? {
    guard let data = try? Data(contentsOf: url(root)),
      let d = try? JSONDecoder().decode(Discovery.self, from: data),
      d.documentPath == root.path
    else { return nil }
    return d
  }
}

public struct Request: Codable {
  public let id: String
  public let method: String  // "get" | "apply"
  public let batch: String?
  public init(id: String, method: String, batch: String?) {
    self.id = id
    self.method = method
    self.batch = batch
  }
}
public struct Reply: Codable {
  public let id: String
  public let status: String  // "ok" | "applied" | "rejected" | "closing" | "failed"
  public let body: String
  public init(id: String, status: String, body: String) {
    self.id = id
    self.status = status
    self.body = body
  }
}

/// Executes one request against an owner. Applied edits are flushed before replying,
/// so "applied" always means durable.
public func execute(_ request: Request, on owner: RustOwner) -> Reply {
  do {
    switch request.method {
    case "get":
      return Reply(id: request.id, status: "ok", body: try owner.core.snapshot())
    case "apply":
      let body: String
      do { body = try owner.command(request.batch ?? "") } catch {
        return Reply(id: request.id, status: "rejected", body: "\(error)")
      }
      try owner.flush()
      return Reply(id: request.id, status: "applied", body: body)
    default:
      return Reply(id: request.id, status: "rejected", body: "unknown method")
    }
  } catch {
    // The edit was accepted in memory but not saved: the owner keeps it and retries.
    return Reply(id: request.id, status: "failed", body: "\(error)")
  }
}

// MARK: Unix socket transport: one newline-terminated JSON request per connection.

func readLine(_ fd: Int32, limit: Int = 8 << 20) -> Data? {
  var data = Data()
  var byte: UInt8 = 0
  while data.count < limit {
    let n = Darwin.read(fd, &byte, 1)
    if n <= 0 { return nil }
    if byte == 10 { return data }
    data.append(byte)
  }
  return nil
}
func writeAll(_ fd: Int32, _ data: Data) -> Bool {
  data.withUnsafeBytes { raw in
    var offset = 0
    while offset < raw.count {
      let n = Darwin.write(fd, raw.baseAddress! + offset, raw.count - offset)
      if n <= 0 { return false }
      offset += n
    }
    return true
  }
}
func address(_ path: String) -> sockaddr_un {
  var addr = sockaddr_un()
  addr.sun_family = sa_family_t(AF_UNIX)
  withUnsafeMutableBytes(of: &addr.sun_path) { buffer in
    let bytes = Array(path.utf8.prefix(buffer.count - 1))
    buffer.copyBytes(from: bytes + [0])
  }
  return addr
}

public final class OwnerServer {
  public let owner: RustOwner
  public let store: Store
  public let root: URL
  public let queue = DispatchQueue(label: "owner")
  private var listener: Int32 = -1
  private let socketPath: String
  /// Set on the owner queue at the start of close; later requests are refused
  /// unapplied ("closing"), so none can land after the final flush.
  private var closing = false
  /// Test hook: called on the owner queue after applying, before replying.
  public var beforeReply: ((Request) -> Void)?

  public init(root: URL, store: Store, owner: RustOwner) {
    self.root = root
    self.store = store
    self.owner = owner
    socketPath = "/tmp/hs-\(getpid())-\(UInt32.random(in: 0...UInt32.max)).sock"
  }
  /// Listens, then publishes discovery. The CLI never sees a discovery file for a
  /// socket that is not yet accepting.
  public func start() throws {
    unlink(socketPath)
    listener = socket(AF_UNIX, SOCK_STREAM, 0)
    var addr = address(socketPath)
    let bound = withUnsafePointer(to: &addr) {
      $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
        bind(listener, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
      }
    }
    guard listener >= 0, bound == 0, listen(listener, 64) == 0 else {
      throw SpikeError("Cannot listen")
    }
    let discovery = try JSONEncoder().encode(Discovery(socket: socketPath, documentPath: root.path))
    let temporary = root.appendingPathComponent(".host.json.tmp")
    try discovery.write(to: temporary)
    guard rename(temporary.path, Discovery.url(root).path) == 0 else {
      throw SpikeError("Cannot publish discovery")
    }
    let fd = listener
    Thread.detachNewThread { [weak self] in
      while true {
        let client = accept(fd, nil, nil)
        if client < 0 { return }  // Listener closed.
        self?.serve(client)
      }
    }
  }
  private func serve(_ client: Int32) {
    defer { Darwin.close(client) }
    guard let line = readLine(client),
      let request = try? JSONDecoder().decode(Request.self, from: line)
    else { return }
    let reply = queue.sync { () -> Reply in
      if closing { return Reply(id: request.id, status: "closing", body: "") }
      let reply = execute(request, on: owner)
      if reply.status == "applied" { beforeReply?(request) }
      return reply
    }
    _ = writeAll(client, (try! JSONEncoder().encode(reply)) + Data([10]))
  }
  /// Stop accepting → drain queued work → flush → remove discovery → release lock.
  /// A CLI that arrives meanwhile sees a refused socket or no discovery, retries,
  /// and owns the document once the lock is free.
  public func close(beforeRelease: () -> Void = {}) throws {
    shutdown(listener, SHUT_RDWR)
    Darwin.close(listener)
    unlink(socketPath)
    try queue.sync {
      closing = true
      try owner.flush()
    }
    try? FileManager.default.removeItem(at: Discovery.url(root))
    beforeRelease()
    store.close()
  }
}

// MARK: Client routing

public enum Outcome {
  case reply(Reply)
  case busy(String)
  case unknown(String)
}
public struct Timings: Codable {
  public var routed = "owned"
  public var attempts = 0
  public var lockMS = 0.0, openMS = 0.0, applyMS = 0.0, flushMS = 0.0
  public init() {}
}
func ms(_ start: ContinuousClock.Instant) -> Double {
  let d = ContinuousClock.now - start
  return Double(d.components.seconds) * 1000 + Double(d.components.attoseconds) / 1e15
}
enum Forwarded {
  case reply(Reply)
  case retry
  case unknown(String)
}
func forward(_ socketPath: String, _ request: Request) -> Forwarded {
  let fd = socket(AF_UNIX, SOCK_STREAM, 0)
  guard fd >= 0 else { return .retry }
  defer { Darwin.close(fd) }
  var addr = address(socketPath)
  let connected = withUnsafePointer(to: &addr) {
    $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
      connect(fd, $0, socklen_t(MemoryLayout<sockaddr_un>.size))
    }
  }
  // Nothing was sent: safe to retry (owner starting, closing, or gone).
  guard connected == 0 else { return .retry }
  guard writeAll(fd, (try! JSONEncoder().encode(request)) + Data([10])) else { return .retry }
  // Sent: from here a missing reply means the outcome is unknown.
  guard let line = readLine(fd), let reply = try? JSONDecoder().decode(Reply.self, from: line)
  else {
    return .unknown("The owner accepted the request but did not reply; run get before another edit")
  }
  if reply.status == "closing" { return .retry }
  return .reply(reply)
}

/// Forward to the lock holder, or become the owner. Retries while an owner is
/// opening or closing, up to `deadline`.
public func route(
  root: URL, request: Request, deadline: Duration = .seconds(2), timings: inout Timings,
  prepare: (Store) -> Void = { _ in }
) -> Outcome {
  let end = ContinuousClock.now.advanced(by: deadline)
  var backoff = 2.0
  while true {
    timings.attempts += 1
    let started = ContinuousClock.now
    if let store = try? Store(root: root) {
      timings.lockMS = ms(started)
      defer { store.close() }
      prepare(store)
      do {
        let t = ContinuousClock.now
        let owner = try RustOwner(store: store)
        timings.openMS = ms(t)
        let a = ContinuousClock.now
        if request.method == "apply" {
          let body: String
          do { body = try owner.command(request.batch ?? "") } catch {
            return .reply(Reply(id: request.id, status: "rejected", body: "\(error)"))
          }
          timings.applyMS = ms(a)
          let f = ContinuousClock.now
          try owner.flush()
          timings.flushMS = ms(f)
          return .reply(Reply(id: request.id, status: "applied", body: body))
        }
        return .reply(Reply(id: request.id, status: "ok", body: try owner.core.snapshot()))
      } catch {
        return .reply(Reply(id: request.id, status: "failed", body: "\(error)"))
      }
    }
    if let discovery = Discovery.read(root) {
      switch forward(discovery.socket, request) {
      case .reply(let reply):
        timings.routed = "forwarded"
        return .reply(reply)
      case .unknown(let message):
        timings.routed = "forwarded"
        return .unknown(message)
      case .retry: break
      }
    }
    if ContinuousClock.now >= end {
      return .busy("Document has a live writer that is not accepting requests; retry later")
    }
    usleep(useconds_t(backoff * 1000))
    backoff = min(backoff * 2, 50)
  }
}
