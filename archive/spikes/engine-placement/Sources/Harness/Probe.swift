import Foundation
import SpikeCore

struct ProbeRequest: Decodable {
  let schema: String
  let seed: String
  let operations: [SpikeCore.Operation]
  let imports: [String]?
  let storage: String?
  let fromVersion: String?
}
func probe(_ input: URL, _ output: URL) throws {
  let request = try JSONDecoder().decode(ProbeRequest.self, from: Data(contentsOf: input))
  let schema = try JSONValue.decode(Data(contentsOf: URL(fileURLWithPath: request.schema)))["root"]
  let replica: Replica
  let session: Session?
  if let directory = request.storage {
    let store = try Store(root: URL(fileURLWithPath: directory))
    if try store.metadata().checkpointBytes == 0 {
      _ = try store.write(
        generation: "0", checkpoint: Data(contentsOf: URL(fileURLWithPath: request.seed)),
        schemaKey: "probe")
    }
    let owner = try Session(store: store, schema: schema)
    session = owner
    replica = owner.replica
  } else {
    session = nil
    replica = try Replica(
      schema: schema, snapshot: Data(contentsOf: URL(fileURLWithPath: request.seed)))
  }
  for value in request.imports ?? [] {
    guard let bytes = Data(base64Encoded: value) else { throw SpikeError("Invalid import") }
    if let session { try session.receive(bytes) } else { try replica.receive(bytes) }
  }
  let before = replica.version()
  var rejected: JSONValue = .null
  do {
    if !request.operations.isEmpty {
      if let session {
        _ = try session.apply(request.operations)
      } else {
        _ = try replica.apply(request.operations)
      }
    }
  } catch { rejected = .string(String(describing: error)) }
  try session?.flush()
  let delta: JSONValue
  if let encoded = request.fromVersion, let vv = Data(base64Encoded: encoded) {
    delta = .string(try replica.updates(since: vv).base64EncodedString())
  } else {
    delta = .null
  }
  try JSONValue.object([
    "delta": delta, "version": .string(replica.version().base64EncodedString()),
    "data": try replica.read([]), "rejected": rejected,
    "versionUnchanged": .bool(before == replica.version()),
    "snapshot": .string(try replica.snapshot().base64EncodedString()),
  ]).encoded().write(to: output)
  try session?.close()
}
