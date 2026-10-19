import AuthenticationServices
import CryptoKit
@preconcurrency import FirebaseAuth
import FirebaseCore
import HitSlopCore
import HitSlopCoreBinding
import Security
import SwiftUI

/// Explicit developer auth proof. Named Firebase initialization never enables the
/// default app's Analytics/Crashlytics collection in Debug or on the phone.
@MainActor private final class SharingAccount: ObservableObject {
  @Published var fingerprint: String?
  @Published var message = "Sign in to test your sharing account. Existing document sharing is unchanged."
  @Published var busy = false
  private var auth: Auth?
  private var nonce: String?
  func configure() {
    guard auth == nil, SharingPreview.enabled else { return }
    let name = "sharing-auth"
    if FirebaseApp.app(name: name) == nil {
      guard let path = Bundle.main.path(forResource: "GoogleService-Info", ofType: "plist"),
        let options = FirebaseOptions(contentsOfFile: path)
      else {
        message = "The signed app is missing its Firebase configuration."
        return
      }
      FirebaseApp.configure(name: name, options: options)
    }
    guard let app = FirebaseApp.app(name: name) else { return }
    auth = Auth.auth(app: app)
    updateIdentity()
  }
  private func updateIdentity() {
    fingerprint = auth?.currentUser.map { user in
      SHA256.hash(data: Data(user.uid.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
    }
  }
  func prepare(_ request: ASAuthorizationAppleIDRequest) {
    var bytes = [UInt8](repeating: 0, count: 32)
    guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
      message = "Could not prepare sign-in. Try again."
      return
    }
    let raw = bytes.map { String(format: "%02x", $0) }.joined()
    nonce = raw
    request.nonce = SHA256.hash(data: Data(raw.utf8)).map { String(format: "%02x", $0) }.joined()
    request.requestedScopes = []
  }
  func complete(_ result: Result<ASAuthorization, Error>) async {
    guard !busy else { return }
    busy = true
    defer {
      busy = false
      nonce = nil
    }
    do {
      guard let auth, let nonce,
        let credential = try result.get().credential as? ASAuthorizationAppleIDCredential,
        let bytes = credential.identityToken,
        let token = String(data: bytes, encoding: .utf8)
      else { throw SlopFailure("Apple did not return a sign-in credential. Try again.") }
      let apple = OAuthProvider.appleCredential(withIDToken: token, rawNonce: nonce, fullName: nil)
      _ = try await auth.signIn(with: apple)
      updateIdentity()
      try await verify()
    } catch {
      // Auth errors can carry provider response data. Keep the UI/logs generic.
      message = "Sign-in or service verification failed. Check Apple/Firebase configuration and try again."
    }
  }
  func checkService() async {
    guard !busy else { return }
    busy = true
    defer { busy = false }
    do { try await verify() } catch {
      message = "The sharing service could not verify this session. Try signing in again."
    }
  }
  private func verify() async throws {
    guard let user = auth?.currentUser else { throw SlopFailure("Sign in required") }
    let expected = user.uid
    let token = try await user.getIDToken(forcingRefresh: true)
    let verified = try await Task.detached(priority: .utility) {
      try sharingVerifyAccount(token: token)
    }.value
    guard verified.uid == expected else { throw SlopFailure("Identity mismatch") }
    message = "Sharing service verified. Compare the account code on your other device."
    updateIdentity()
  }
  func signOut() {
    do {
      try auth?.signOut()
      updateIdentity()
      message = "Signed out. Documents already joined on this device can continue syncing."
    } catch { message = "Could not sign out. Try again." }
  }
}

public struct SharingAccountView: View {
  @StateObject private var account = SharingAccount()
  public init() {}
  public var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("Sharing Account").font(.title2.bold())
      if SharingPreview.enabled {
        if let fingerprint = account.fingerprint {
          Text("Account code: \(fingerprint)").font(.system(.body, design: .monospaced)).textSelection(.enabled)
          Button("Verify Sharing Service") { Task { await account.checkService() } }
          Button("Sign Out") { account.signOut() }
        } else {
          SignInWithAppleButton(.signIn, onRequest: account.prepare) { result in
            Task { await account.complete(result) }
          }.frame(height: 44)
        }
        Text(account.message).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        if account.busy { ProgressView() }
      } else {
        Text("Sharing accounts are available in the developer preview.")
      }
    }
    .padding(24)
    .disabled(account.busy)
    .task { account.configure() }
  }
}
