@_exported import PlacementSupport
@_exported import HitSlopCoreBinding
import Foundation

/// Only accessed on its host serial executor. The WebView never owns this object.
public final class RustOwner {
  public let store: Store
  public let core: NativeDocument
  private var savedVersion: String
  private var generation: String
  private let key: String
  public private(set) var dirty = false
  public init(store: Store) throws {
    self.store = store
    let state = try store.load()
    guard let encoded = state.checkpoint, let bytes = Data(base64Encoded: encoded), let schema = state.schemaKey else { throw SpikeError("No seed/schema") }
    key = schema; generation = state.generation
    core = try NativeDocument.open(schemaJson: schema, checkpoint: bytes)
    for update in state.updates { _ = try core.importUpdates(bytes: Data(base64Encoded: update)!) }
    savedVersion = try core.version()
  }
  public func command(_ batch: String) throws -> String { let r = try core.commandCurrent(batchJson:batch); dirty=true; return r }
  public func apply(_ batch: String) throws -> String { let r = try core.apply(batchJson: batch); dirty = true; return r }
  public func text(_ request: String) throws -> String { let r = try core.text(requestJson: request); dirty = true; return r }
  public func receive(_ bytes: Data) throws -> String { let r = try core.importUpdates(bytes: bytes); dirty = true; return r }
  public func flush() throws {
    guard dirty else { return }
    let version = try core.version(), meta = try store.metadata()
    let checkpoint = meta.updateRows >= 256 || meta.updateBytes >= 4 * 1024 * 1024
    let bytes = try checkpoint ? core.checkpoint() : core.exportSince(version: savedVersion)
    do {
      generation = try store.write(generation: generation, checkpoint: checkpoint ? bytes : nil, updates: checkpoint ? [] : [bytes], schemaKey: key)
    } catch {
      // With the OS lock and serial executor, exactly our commit can advance this generation.
      let observed = try store.metadata()
      guard observed.generation == String((Int(generation) ?? 0) + 1) else { throw error }
      generation = observed.generation
    }
    savedVersion = version; dirty = false
  }
  public func close() throws { try flush(); store.close() }
}
