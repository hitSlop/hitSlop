import Foundation

/// Only the active authentication session consumes these callbacks. Firebase validates the JWT.
public struct AppleWebCallback {
  public let idToken: String
  public let appleUser: String
  public let code: String?

  public enum Failure: Error, Equatable {
    case invalidCallback
    case cancelled
  }

  public static func parse(_ url: URL, state: String) throws -> Self {
    guard url.scheme == "hitslop", url.host == "auth", url.path == "/apple",
      url.query == nil, let fragment = URLComponents(url: url, resolvingAgainstBaseURL: false)?.percentEncodedFragment
    else { throw Failure.invalidCallback }
    guard let components = URLComponents(string: "hitslop://callback?" + fragment), components.fragment == nil
    else { throw Failure.invalidCallback }
    let items = components.queryItems ?? []
    // Reject ambiguous credentials rather than choosing one of several state/token values.
    for name in ["state", "id_token", "code", "error"] {
      guard items.filter({ $0.name == name }).count <= 1 else { throw Failure.invalidCallback }
    }
    func value(_ name: String) -> String? { items.first(where: { $0.name == name })?.value }
    guard !state.isEmpty, value("state") == state else { throw Failure.invalidCallback }
    if let error = value("error") {
      throw error == "user_cancelled_authorize" ? Failure.cancelled : Failure.invalidCallback
    }
    guard let token = value("id_token"), !token.isEmpty else { throw Failure.invalidCallback }
    let parts = token.split(separator: ".", omittingEmptySubsequences: false)
    guard parts.count == 3 else { throw Failure.invalidCallback }
    var payload = String(parts[1]).replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
    payload += String(repeating: "=", count: (4 - payload.count % 4) % 4)
    guard let bytes = Data(base64Encoded: payload),
      let claims = try? JSONSerialization.jsonObject(with: bytes) as? [String: Any],
      let subject = claims["sub"] as? String, !subject.isEmpty
    else { throw Failure.invalidCallback }
    return Self(idToken: token, appleUser: subject, code: value("code"))
  }
}
