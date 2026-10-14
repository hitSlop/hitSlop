import HitSlopCore
import HitSlopCoreBinding

private let host = hostLimits()
extension Limits {
  public static let socketAttachment = Int(host.socketAttachment)
}
public typealias OutcomeCode = WireOutcomeCode

extension WireOutcomeCode {
  public init?(rawValue: String) {
    guard let value = parseOutcomeCode(name: rawValue) else { return nil }
    self = value
  }
  public var rawValue: String { outcomeCodeName(code: self) }
}
