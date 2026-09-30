import Foundation
import HitSlopCoreBinding

/// Platform envelopes are checked by the core against the generated contracts. Swift
/// only serializes them for that check and maps the accepted values.
public enum Envelope {
  public static func valid(_ kind: EnvelopeKind, _ json: Data) -> Bool {
    envelopeIsValid(kind: kind, json: json)
  }

  public static func valid(_ kind: EnvelopeKind, object: Any) -> Bool {
    guard JSONSerialization.isValidJSONObject(object),
      let json = try? JSONSerialization.data(withJSONObject: object, options: .withoutEscapingSlashes)
    else { return false }
    return valid(kind, json)
  }
}
