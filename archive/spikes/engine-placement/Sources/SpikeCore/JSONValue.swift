@_exported import PlacementSupport
import Loro

extension JSONValue {
  public var loro: LoroValue {
    switch self {
    case .null: .null
    case .bool(let v): .bool(value: v)
    case .number(let v): .double(value: v)
    case .string(let v): .string(value: v)
    case .array(let v): .list(value: v.map(\.loro))
    case .object(let v): .map(value: v.mapValues(\.loro))
    }
  }
  public init(_ v: LoroValue) {
    switch v {
    case .null: self = .null
    case .bool(let v): self = .bool(v)
    case .double(let v): self = v.isFinite ? .number(v) : .null
    case .i64(let v): self = .number(Double(v))
    case .string(let v): self = .string(v)
    case .list(let v): self = .array(v.map(Self.init))
    case .map(let v): self = .object(v.mapValues(Self.init))
    case .binary, .container: self = .null
    }
  }
}
