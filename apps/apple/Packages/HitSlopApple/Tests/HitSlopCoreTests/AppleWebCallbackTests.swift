import Foundation
import HitSlopCore
import Testing

/// An unsigned JWT-shaped fixture, built at runtime so it never reads as a stored credential.
private func fixtureToken(_ claims: String) -> String {
  let payload = Data(claims.utf8).base64EncodedString().replacingOccurrences(of: "=", with: "")
  return ["header", payload, "signature"].joined(separator: ".")
}
private let appleToken = fixtureToken(#"{"sub":"apple-user"}"#)
private let claimlessToken = fixtureToken("{}")

@Test func appleWebCallbackPreservesCredentialsAndReadsTheSubject() throws {
  let url = try #require(URL(string: "hitslop://auth/apple#state=expected&id_token=\(appleToken)&code=a%2Bb%26c%3D"))
  let callback = try AppleWebCallback.parse(url, state: "expected")
  #expect(callback.idToken == appleToken)
  #expect(callback.appleUser == "apple-user")
  #expect(callback.code == "a+b&c=")
  let withoutCode = try #require(URL(string: "hitslop://auth/apple#state=expected&id_token=\(appleToken)"))
  #expect(try AppleWebCallback.parse(withoutCode, state: "expected").code == nil)
}

@Test(arguments: [
  "hitslop://auth/apple#state=wrong&id_token=\(appleToken)",
  "hitslop://auth/apple#state=expected&code=code",
  "hitslop://auth/apple#state=expected&id_token=",
  "hitslop://auth/apple#state=expected&id_token=invalid",
  "hitslop://auth/apple#state=expected&id_token=\(claimlessToken)",
  "hitslop://auth/apple#state=expected&id_token=header.____.signature",
  "hitslop://auth/apple#state=expected&state=wrong&id_token=\(appleToken)",
  "hitslop://auth/apple#state=expected&id_token=\(appleToken)&id_token=other",
  "hitslop://auth/apple#state=expected&error=invalid_request",
  "hitslop://auth/apple#state=wrong&error=user_cancelled_authorize",
  "hitslop://widgets/apple#state=expected&id_token=\(appleToken)",
  "hitslop://auth/other#state=expected&id_token=\(appleToken)",
  "https://auth/apple#state=expected&id_token=\(appleToken)",
  "hitslop://auth/apple?state=expected&id_token=\(appleToken)",
]) func appleWebCallbackRejectsInvalidResponses(value: String) throws {
  let url = try #require(URL(string: value))
  #expect(throws: AppleWebCallback.Failure.invalidCallback) {
    try AppleWebCallback.parse(url, state: "expected")
  }
}

@Test func appleWebCallbackReportsCancellationAfterCheckingState() throws {
  let url = try #require(URL(string: "hitslop://auth/apple#state=expected&error=user_cancelled_authorize"))
  #expect(throws: AppleWebCallback.Failure.cancelled) {
    try AppleWebCallback.parse(url, state: "expected")
  }
}
