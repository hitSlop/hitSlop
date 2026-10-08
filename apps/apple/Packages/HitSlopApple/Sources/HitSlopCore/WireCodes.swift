import HitSlopCoreBinding

public typealias CoreErrorCode = WireCoreErrorCode

extension WireCoreErrorCode {
  public init?(rawValue: String) {
    guard let value = parseCoreCode(name: rawValue) else { return nil }
    self = value
  }
  public var rawValue: String { coreCodeName(code: self) }
}
