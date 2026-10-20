import AuthenticationServices
import CryptoKit
@preconcurrency import FirebaseAuth
import FirebaseCore
import HitSlopCore
import HitSlopCoreBinding
import Security
import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

/// Retained for the duration of a fresh Apple request, including its presentation anchor.
@MainActor
private final class AccountAppleAuthorization: NSObject, ASAuthorizationControllerDelegate,
  ASAuthorizationControllerPresentationContextProviding
{
  private var continuation: CheckedContinuation<ASAuthorization, Error>?
  private var controller: ASAuthorizationController?
  func request(_ request: ASAuthorizationAppleIDRequest) async throws -> ASAuthorization {
    try await withCheckedThrowingContinuation { continuation in
      self.continuation = continuation
      let controller = ASAuthorizationController(authorizationRequests: [request])
      self.controller = controller
      controller.delegate = self
      controller.presentationContextProvider = self
      controller.performRequests()
    }
  }
  func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
    #if os(macOS)
      NSApplication.shared.keyWindow ?? NSApplication.shared.windows.first ?? ASPresentationAnchor()
    #else
      UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        .flatMap(\.windows).first(where: \.isKeyWindow) ?? ASPresentationAnchor()
    #endif
  }
  func authorizationController(
    controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization
  ) {
    continuation?.resume(returning: authorization)
    continuation = nil
    self.controller = nil
  }
  func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
    continuation?.resume(throwing: error)
    continuation = nil
    self.controller = nil
  }
}

/// Explicit developer auth proof. Named Firebase initialization never enables the
/// default app's Analytics/Crashlytics collection in Debug or on the phone.
@MainActor public final class SharingAccount: ObservableObject {
  public static let shared = SharingAccount()
  private init() {}
  @Published public private(set) var signedIn = false
  public func token() async throws -> String {
    configure()
    guard !deleting else { throw SlopFailure("Account deletion is in progress.") }
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
  private var deleting = false
  private var deletionStarted = false
  private var deletionDocuments: [String] = []

  public func deleteAccount() async {
    guard !busy, let auth, let user = auth.currentUser else { return }
    busy = true
    deleting = true
    message = ""
    var relayDeleted = deletionStarted
    defer {
      if relayDeleted {
        var failed = false
        for document in deletionDocuments {
          do { try SharingCredentials.remove(document) } catch { failed = true }
        }
        if failed { message += " Could not clear sharing details. Unlock this device and retry." }
      }
      deleting = false
      busy = false
      nonce = nil
      updateIdentity()
      if relayDeleted { NotificationCenter.default.post(name: .hitSlopSharingAccountChanged, object: nil) }
    }
    var note = ""
    do {
      if !deletionStarted {
        let token = try await user.getIDToken(forcingRefresh: true)
        let owned = try await Task.detached(priority: .utility) {
          var documents: [String] = []
          for (document, credential) in try SharingCredentials.all() {
            if try sharingAccountOwns(document: document, credential: credential, token: token) {
              documents.append(document)
            }
          }
          try sharingDeleteAccount(token: token)
          return documents
        }.value
        deletionDocuments = owned
        deletionStarted = true
        relayDeleted = true
      }
      var apple: AuthCredential?
      do {
        let request = ASAuthorizationAppleIDProvider().createRequest()
        prepare(request)
        let authorization = try await AccountAppleAuthorization().request(request)
        guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
          let bytes = credential.identityToken, let token = String(data: bytes, encoding: .utf8),
          let nonce,
          // Never revoke an Apple account different from the one being deleted.
          user.providerData.contains(where: { $0.providerID == "apple.com" && $0.uid == credential.user })
        else { throw SlopFailure("Apple sign-in did not match this account.") }
        apple = OAuthProvider.appleCredential(withIDToken: token, rawNonce: nonce, fullName: nil)
        if let bytes = credential.authorizationCode, let code = String(data: bytes, encoding: .utf8) {
          do { try await auth.revokeToken(withAuthorizationCode: code) } catch {
            note = " Apple access could not be revoked. Remove hitSlop in your Apple Account settings."
          }
        } else {
          note = " Apple access could not be revoked. Remove hitSlop in your Apple Account settings."
        }
      } catch {
        note = " Apple access was not revoked. Remove hitSlop in your Apple Account settings."
      }
      do { try await user.delete() } catch {
        guard (error as NSError).code == AuthErrorCode.requiresRecentLogin.rawValue, let apple else {
          throw error
        }
        _ = try await user.reauthenticate(with: apple)
        try await user.delete()
      }
      deletionStarted = false
      message = "Account deleted." + note
    } catch {
      message =
        deletionStarted
        ? "Your live links stopped. Could not finish deleting the account. Try again." + note
        : "Could not delete the account. Try again."
    }
  }
  public func signOut() {
    guard !busy else { return }
    do {
      try auth?.signOut()
      deletionStarted = false
      deletionDocuments = []
      updateIdentity()
      NotificationCenter.default.post(name: .hitSlopSharingAccountChanged, object: nil)
      message = ""
    } catch { message = "Could not sign out. Try again." }
  }
}

public struct SharingAccountView: View {
  @ObservedObject private var account = SharingAccount.shared
  @State private var confirmingDeletion = false
  public init() {}
  public var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("Account").font(.title2.bold())
      if SharingPreview.enabled {
        if let fingerprint = account.fingerprint {
          Text("Signed in with Apple")
          Text("Account code: \(fingerprint.prefix(8))").font(.footnote).textSelection(.enabled)
          Button("Sign Out") { account.signOut() }
          Button("Delete Account…", role: .destructive) { confirmingDeletion = true }
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
    .confirmationDialog("Delete Account?", isPresented: $confirmingDeletion, titleVisibility: .visible) {
      Button("Delete Account", role: .destructive) { Task { await account.deleteAccount() } }
      Button("Cancel", role: .cancel) {}
    } message: {
      Text(
        "Your live links stop working and the sharing account is deleted. Documents on your devices and in iCloud stay."
      )
    }
  }
}
