import PopCore
import PopCoreBinding

private let host = hostLimits()
extension Limits {
  public static let socketAttachment = Int(host.socketAttachment)
}
public enum OutcomeCode: String, Sendable {
  case rejected
  case ownerReplaced = "owner_replaced"
  case closing
  case saveFailed = "save_failed"
  case ownerInvalidated = "owner_invalidated"
  case unknownOutcome = "unknown_outcome"
}
