import Foundation
import Loro

public struct Patch: Codable {
  public let path: [JSONValue]
  public let value: JSONValue
  public var action: String? = nil
}
public struct Publication: Codable {
  public let revision: Int
  public let patches: [Patch]
  public let engineMS: Double
}
/// Single executor ownership; no await inside a batch. The schema is descriptor data.
public final class Replica {
  public let doc: LoroDoc
  public let schema: JSONValue
  public private(set) var revision = 0
  public private(set) var counterDirty = false
  private let indexed: Bool
  private var rowIndexes: [ContainerId: [String: ContainerId]] = [:]
  public init(schema: JSONValue, snapshot: Data, indexed: Bool = true) throws {
    self.indexed = indexed
    self.schema = schema
    doc = LoroDoc()
    let styles = StyleConfigMap()
    styles.insert(key: "bold", value: StyleConfig(expand: .after))
    doc.configTextStyle(textStyle: styles)
    let status = try doc.import(bytes: snapshot)
    guard status.pending?.isEmpty != false else { throw SpikeError("missing_dependencies") }
    if indexed {
      for key in schema["properties"].object.keys {
        if let list = doc.getMap(id: "data").get(key: key)?.asLoroMovableList() { index(list) }
      }
    }
  }
  public func snapshot() throws -> Data { try doc.exportSnapshot() }
  public func version() -> Data { doc.oplogVv().encode() }
  public func updates(since version: Data) throws -> Data {
    try doc.exportUpdates(vv: VersionVector.decode(bytes: version))
  }
  private func index(_ list: LoroMovableList) {
    guard case .list(let containers) = list.getValue(), case .list(let rows) = list.getDeepValue()
    else { return }
    var ids: [String: ContainerId] = [:]
    for (value, row) in zip(containers, rows) {
      if case .container(let cid) = value, case .map(let fields) = row,
        case .string(let id) = fields["$id"]
      {
        ids[id] = cid
      }
    }
    rowIndexes[list.id()] = ids
  }
  private struct Location {
    let node: JSONValue
    let value: ValueOrContainer?
    let parent: LoroMap?
    let key: String
  }
  private func unwrap(_ n: JSONValue) -> JSONValue {
    n["kind"].string == "optional" ? n["inner"] : n
  }
  private func resolve(_ engine: LoroDoc, _ path: [JSONValue]) throws -> Location {
    var node = schema
    var map = engine.getMap(id: "data")
    var current: ValueOrContainer?
    var parent: LoroMap?
    var key = ""
    if path.isEmpty { throw SpikeError("Root is not writable") }
    for (i, part) in path.enumerated() {
      node = unwrap(node)
      if let field = part.string {
        guard node["kind"].string == "object", let next = node["properties"].object[field] else {
          throw SpikeError("Unknown field")
        }
        if i > 0 {
          guard let m = current?.asLoroMap() else { throw SpikeError("Expected LoroMap") }
          map = m
        }
        node = next
        parent = map
        key = field
        current = map.get(key: field)
      } else if let id = part["id"].string {
        guard node["kind"].string == "list", let list = current?.asLoroMovableList() else {
          throw SpikeError("Expected LoroMovableList")
        }
        var found: ValueOrContainer?
        if indexed {
          if rowIndexes[list.id()]?[id] == nil { index(list) }
          if let cid = rowIndexes[list.id()]?[id] { found = engine.getContainer(id: cid) }
        } else {
          for index in 0..<list.len() {
            if list.get(index: index)?.asLoroMap()?.get(key: "$id")?.asValue()
              == LoroValue.string(value: id)
            {
              found = list.get(index: index)
              break
            }
          }
        }
        guard let found else { throw SpikeError("Missing row") }
        current = found
        node = node["item"]
        parent = nil
      } else if let entry = part["key"].string {
        guard node["kind"].string == "record", let m = current?.asLoroMap() else {
          throw SpikeError("Expected record")
        }
        node = node["value"]
        parent = m
        key = entry
        current = m.get(key: entry)
      } else {
        throw SpikeError("Unsupported path component")
      }
    }
    return Location(node: node, value: current, parent: parent, key: key)
  }
  private func validate(_ input: JSONValue, _ n: JSONValue) throws {
    let n = unwrap(n)
    let kind = n["kind"].string
    var valid = false
    switch kind {
    case "string", "text", "richtext": valid = input.string != nil
    case "boolean": if case .bool = input { valid = true }
    case "number", "integer", "counter":
      if let v = input.number {
        valid =
          v.isFinite && (kind != "integer" || (v.rounded() == v && abs(v) <= 9_007_199_254_740_991))
          && (n["min"].number.map { v >= $0 } ?? true) && (n["max"].number.map { v <= $0 } ?? true)
      }
    case "enum": valid = n["values"].array.contains(input)
    case "object":
      guard case .object(let obj) = input else { throw SpikeError("Expected object") }
      guard Set(obj.keys).subtracting(Set(n["properties"].object.keys).union(["$id"])).isEmpty
      else { throw SpikeError("Unknown property") }
      for (key, field) in n["properties"].object {
        if let v = obj[key] {
          try validate(v, field)
        } else if field["kind"].string != "optional" {
          throw SpikeError("Missing field")
        }
      }
      valid = true
    case "list":
      if case .array(let values) = input {
        for v in values { try validate(v, n["item"]) }
        valid = true
      }
    case "record":
      if case .object(let values) = input {
        for (_, v) in values { try validate(v, n["value"]) }
        valid = true
      }
    default: throw SpikeError("Unsupported descriptor: \(kind ?? "missing")")
    }
    if let max = n["maxLength"].number, let value = input.string, value.utf16.count > Int(max) {
      valid = false
    }
    guard valid else { throw SpikeError("Invalid \(kind ?? "value")") }
  }
  private func fill(_ map: LoroMap, _ key: String, _ n: JSONValue, _ v: JSONValue) throws {
    let n = unwrap(n)
    switch n["kind"].string {
    case "text", "richtext":
      let t = try map.insertContainer(key: key, child: LoroText())
      try t.insertUtf16(pos: 0, s: v.string!)
    case "counter":
      let c = try map.insertContainer(key: key, child: LoroCounter())
      try c.increment(value: v.number!)
    case "object", "record":
      let child = try map.insertContainer(key: key, child: LoroMap())
      for (k, value) in v.object {
        try fill(child, k, n["kind"].string == "record" ? n["value"] : n["properties"][k], value)
      }
    case "list":
      let list = try map.insertContainer(key: key, child: LoroMovableList())
      for (i, value) in v.array.enumerated() {
        try insert(list, n["item"], i, UUID().uuidString, value)
      }
    default: try map.insert(key: key, v: v.loro)
    }
  }
  private func insert(
    _ list: LoroMovableList, _ item: JSONValue, _ index: Int, _ id: String, _ value: JSONValue
  ) throws {
    guard index >= 0, index <= Int(list.len()) else { throw SpikeError("List index out of bounds") }
    try validate(value, item)
    if item["kind"].string == "object" {
      let row = try list.insertContainer(pos: UInt32(index), child: LoroMap())
      try row.insert(key: "$id", v: id)
      for (key, v) in value.object where key != "$id" {
        try fill(row, key, item["properties"][key], v)
      }
    } else {
      try list.insert(pos: UInt32(index), v: value.loro)
    }
  }
  private func apply(_ op: Operation, _ engine: LoroDoc) throws {
    let l = try resolve(engine, op.path)
    let n = unwrap(l.node)
    switch op {
    case .set(let o):
      guard ["string", "boolean", "number", "integer", "enum"].contains(n["kind"].string),
        let map = l.parent
      else { throw SpikeError("Expected scalar") }
      try validate(o.value, n)
      try map.insert(key: l.key, v: o.value.loro)
    case .clear:
      guard l.node["kind"].string == "optional" || op.path.last?["key"].string != nil,
        let map = l.parent
      else { throw SpikeError("Cannot clear required field") }
      try map.delete(key: l.key)
    case .assign(let o):
      guard l.value == nil, let map = l.parent else {
        throw SpikeError("Cannot replace identity-bearing field")
      }
      try validate(o.value, n)
      try fill(map, l.key, n, o.value)
    case .splice(let o):
      guard ["text", "richtext"].contains(n["kind"].string), let t = l.value?.asLoroText(),
        o.index >= 0, o.deleteCount >= 0, o.index <= Int(t.lenUtf16()),
        o.deleteCount <= Int(t.lenUtf16()) - o.index
      else { throw SpikeError("Invalid text range") }
      if o.deleteCount > 0 { try t.deleteUtf16(pos: UInt32(o.index), len: UInt32(o.deleteCount)) }
      if !o.text.isEmpty { try t.insertUtf16(pos: UInt32(o.index), s: o.text) }
    case .mark(let o):
      guard n["kind"].string == "richtext", n["marks"].object[o.key] != nil,
        let t = l.value?.asLoroText(), o.start >= 0, o.end > o.start, o.end <= Int(t.lenUtf16())
      else { throw SpikeError("Invalid mark") }
      try t.mark(from: UInt32(o.start), to: UInt32(o.end), key: o.key, value: o.value.loro)
    case .increment(let o):
      guard n["kind"].string == "counter", let c = l.value?.asLoroCounter(), o.amount.isFinite,
        (c.getValue() + o.amount).isFinite
      else { throw SpikeError("Invalid counter") }
      try c.increment(value: o.amount)
    case .insert(let o):
      guard n["kind"].string == "list", let list = l.value?.asLoroMovableList() else {
        throw SpikeError("Expected list")
      }
      try insert(list, n["item"], o.index, o.id, o.value)
    case .remove(let o):
      guard let list = l.value?.asLoroMovableList(), o.index >= 0, o.index < Int(list.len()) else {
        throw SpikeError("Invalid list index")
      }
      try list.delete(pos: UInt32(o.index), len: 1)
    case .move(let o):
      guard let list = l.value?.asLoroMovableList(), o.from >= 0, o.to >= 0,
        o.from < Int(list.len()), o.to < Int(list.len())
      else { throw SpikeError("Invalid list index") }
      try list.mov(from: UInt32(o.from), to: UInt32(o.to))
    }
  }
  public func apply(_ operations: [Operation]) throws -> Publication {
    let start = Date()
    let staged = doc.fork()
    try staged.setPeerId(peer: doc.peerId())
    do { for op in operations { try apply(op, staged) } } catch {
      rowIndexes.removeAll()
      throw error
    }
    staged.commit()
    let delta = try staged.exportUpdates(vv: doc.oplogVv())
    _ = try doc.import(bytes: delta)
    counterDirty =
      counterDirty || operations.contains { if case .increment = $0 { true } else { false } }
    revision += 1
    var paths: [[JSONValue]] = []
    for op in operations where !paths.contains(op.path) { paths.append(op.path) }
    let patches: [Patch]
    if indexed, operations.count == 1, case .move(let move) = operations[0] {
      patches = [
        Patch(
          path: move.path,
          value: .object(["from": .number(Double(move.from)), "to": .number(Double(move.to))]),
          action: "move")
      ]
    } else {
      if operations.contains(where: {
        if case .insert = $0 { true } else if case .remove = $0 { true } else { false }
      }) {
        rowIndexes.removeAll()
      }
      patches = try paths.map { Patch(path: $0, value: try read($0)) }
    }
    return Publication(
      revision: revision, patches: patches, engineMS: Date().timeIntervalSince(start) * 1000)
  }
  public func receive(_ bytes: Data) throws {
    // Import pending dependencies into the one live replica; Loro buffers them.
    _ = try doc.import(bytes: bytes)
    revision += 1
    rowIndexes.removeAll()
    // Conservative for the spike: incoming counter effects may require checkpointing.
    counterDirty = true
  }
  public func saved() { counterDirty = false }
  public func read(_ path: [JSONValue]) throws -> JSONValue {
    if path.isEmpty {
      var value = project(schema, doc.getMap(id: "data").getDeepValue()).object
      for (key, node) in schema["properties"].object
      where ["text", "richtext", "counter"].contains(unwrap(node)["kind"].string) {
        if node["kind"].string != "optional" || doc.getMap(id: "data").get(key: key) != nil {
          value[key] = try read([.string(key)])
        }
      }
      return .object(value)
    }
    let l = try resolve(doc, path)
    let n = unwrap(l.node)
    if let t = l.value?.asLoroText() {
      if n["kind"].string == "richtext" {
        let delta = t.toDelta().compactMap { d -> JSONValue? in
          if case .insert(let s, let attrs) = d {
            var v: [String: JSONValue] = ["insert": .string(s)]
            if let attrs { v["attributes"] = .object(attrs.mapValues(JSONValue.init)) }
            return .object(v)
          }
          return nil
        }
        return .object(["text": .string(t.toString()), "delta": .array(delta)])
      }
      return .string(t.toString())
    }
    if let m = l.value?.asLoroMap() { return project(n, m.getDeepValue()) }
    if let list = l.value?.asLoroMovableList() { return project(n, list.getDeepValue()) }
    if let c = l.value?.asLoroCounter() {
      return c.getValue().isFinite ? .number(c.getValue()) : .null
    }
    if ["text", "richtext", "object", "list", "record", "tree", "counter"].contains(
      n["kind"].string)
    {
      return project(n, .null)
    }
    return project(n, l.value?.asValue() ?? .null)
  }
  private func project(_ n: JSONValue, _ v: LoroValue) -> JSONValue {
    let n = unwrap(n)
    let raw = JSONValue(v)
    switch n["kind"].string {
    case "object":
      var result: [String: JSONValue] = [:]
      for (key, field) in n["properties"].object {
        if raw.object[key] != nil || field["kind"].string != "optional" {
          result[key] = project(field, raw[key].loro)
        }
      }
      if let id = raw.object["$id"] { result["$id"] = id }
      return .object(result)
    case "record": return .object(raw.object.mapValues { project(n["value"], $0.loro) })
    case "list":
      return .array(
        raw.array.filter { n["item"]["kind"].string != "object" || !$0.object.isEmpty }.map {
          project(n["item"], $0.loro)
        })
    case "text", "string", "enum": return raw.string == nil ? .string("") : raw
    case "richtext":
      return .object(["text": raw.string == nil ? .string("") : raw, "delta": .array([])])
    case "number", "integer": return raw.number == nil ? .number(0) : raw
    case "counter": return raw.number == nil ? .null : raw
    case "boolean":
      if case .bool = raw { return raw }
      return .bool(false)
    default: return raw
    }
  }
}
