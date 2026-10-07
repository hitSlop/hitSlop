import HitSlopCoreBinding

public typealias CoreErrorCode = WireCoreErrorCode
public typealias OutcomeCode = WireOutcomeCode

extension WireCoreErrorCode {
  public init?(rawValue: String) {
    guard let value = parseCoreCode(name: rawValue) else { return nil }
    self = value
  }
  public var rawValue: String { coreCodeName(code: self) }
}
extension WireOutcomeCode {
  public init?(rawValue: String) {
    guard let value = parseOutcomeCode(name: rawValue) else { return nil }
    self = value
  }
  public var rawValue: String { outcomeCodeName(code: self) }
}
