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
@MainActor public final class SharingAccount: ObservableObject {
  public static let shared = SharingAccount()
  private init() {}
  @Published public private(set) var signedIn = false
  public func token() async throws -> String {
    configure()
    guard let user = auth?.currentUser else { throw SlopFailure("Sign in with Apple first.") }
    return try await user.getIDToken()
  }
  @Published var fingerprint: String?
  @Published public private(set) var message = ""
  @Published var busy = false
  private var auth: Auth?
  private var nonce: String?
  public func configure() {
    guard auth == nil, SharingPreview.enabled else { return }
    let name = "sharing-auth"
    if FirebaseApp.app(name: name) == nil {
      guard let path = Bundle.main.path(forResource: "GoogleService-Info", ofType: "plist"),
        let options = FirebaseOptions(contentsOfFile: path)
      else {
        message = "Sign-in is unavailable in this app."
        return
      }
      FirebaseApp.configure(name: name, options: options)
    }
    guard let app = FirebaseApp.app(name: name) else { return }
    auth = Auth.auth(app: app)
    updateIdentity()
  }
  private func updateIdentity() {
    signedIn = auth?.currentUser != nil
    fingerprint = auth?.currentUser.map { user in
      SHA256.hash(data: Data(user.uid.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
    }
  }
  public func prepare(_ request: ASAuthorizationAppleIDRequest) {
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
  public func complete(_ result: Result<ASAuthorization, Error>) async -> Bool {
    guard !busy else { return false }
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
      NotificationCenter.default.post(name: .hitSlopSharingAccountChanged, object: nil)
      try await verify()
      return true
    } catch {
      // Auth errors can carry provider response data. Keep the UI/logs generic.
      message = "Could not finish signing in. Try again."
      return false
    }
  }
  private func verify() async throws {
    guard let user = auth?.currentUser else { throw SlopFailure("Sign in required") }
    let token = try await user.getIDToken(forcingRefresh: true)
    try await Task.detached(priority: .utility) {
      try sharingVerifyAccount(token: token)
    }.value
    message = ""
    updateIdentity()
  }
  public func signOut() {
    do {
      try auth?.signOut()
      updateIdentity()
      NotificationCenter.default.post(name: .hitSlopSharingAccountChanged, object: nil)
      message = ""
    } catch { message = "Could not sign out. Try again." }
  }
}

public struct SharingAccountView: View {
  @ObservedObject private var account = SharingAccount.shared
  public init() {}
  public var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("Account").font(.title2.bold())
      if SharingPreview.enabled {
        if let fingerprint = account.fingerprint {
          Text("Signed in with Apple")
          Text("Account code: \(fingerprint.prefix(8))").font(.footnote).textSelection(.enabled)
          Button("Sign Out") { account.signOut() }
        } else {
          SignInWithAppleButton(.signIn, onRequest: account.prepare) { result in
            Task { _ = await account.complete(result) }
          }.frame(height: 44)
          Text("Sign in to start live links. Opening a shared link doesn't need an account.")
            .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
        }
        if !account.message.isEmpty {
          Text(account.message).foregroundStyle(.red).fixedSize(horizontal: false, vertical: true)
        }
        if account.busy { ProgressView() }
      } else {
        Text("Live collaboration is unavailable in this app.")
      }
    }
    .padding(24)
    .disabled(account.busy)
    .task { account.configure() }
  }
}
