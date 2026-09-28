import Foundation

/// Used only on the harness document queue. Renderer lifetime is independent.
public final class Session {
  public let store: Store
  public let replica: Replica
  private var savedVersion: Data
  private var generation: String
  private var dirty = false
  private let key: String
  public init(store: Store, schema: JSONValue, indexed: Bool = true) throws {
    self.store = store
    let state = try store.load()
    guard let encoded = state.checkpoint, let bytes = Data(base64Encoded: encoded) else {
      throw SpikeError("No seed")
    }
    key = state.schemaKey ?? "spike"
    generation = state.generation
    replica = try Replica(schema: schema, snapshot: bytes, indexed: indexed)
    for update in state.updates { try replica.receive(Data(base64Encoded: update)!) }
    replica.saved()
    savedVersion = replica.version()
  }
  public func apply(_ operations: [Operation]) throws -> Publication {
    let r = try replica.apply(operations)
    dirty = true
    return r
  }
  public func receive(_ bytes: Data) throws {
    try replica.receive(bytes)
    dirty = true
  }
  public func flush() throws {
    guard dirty else { return }
    let version = replica.version()
    let meta = try store.metadata()
    let checkpoint =
      replica.counterDirty || meta.updateRows >= 256 || meta.updateBytes >= 4 * 1024 * 1024
    let bytes = try checkpoint ? replica.snapshot() : replica.updates(since: savedVersion)
    do {
      generation = try store.write(
        generation: generation, checkpoint: checkpoint ? bytes : nil,
        updates: checkpoint ? [] : [bytes], schemaKey: key)
    } catch {
      let observed = try store.metadata()
      guard observed.generation == String((Int(generation) ?? 0) + 1) else { throw error }
      generation = observed.generation
    }
    savedVersion = version
    dirty = false
    replica.saved()
  }
  public func close() throws {
    try flush()
    store.close()
  }
}
