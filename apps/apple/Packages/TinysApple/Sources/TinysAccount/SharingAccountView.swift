import AuthenticationServices
import CryptoKit
@preconcurrency import FirebaseAuth
import FirebaseCore
import Security
import SwiftUI
import TinysCore
import TinysCoreBinding

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

#if !os(macOS)
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
      UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        .flatMap(\.windows).first(where: \.isKeyWindow) ?? ASPresentationAnchor()
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

#else
  @MainActor
  private final class WebAppleAuthorization: NSObject, ASWebAuthenticationPresentationContextProviding {
    private var session: ASWebAuthenticationSession?
    private var continuation: CheckedContinuation<URL, Error>?

    func request(_ url: URL) async throws -> URL {
      try await withCheckedThrowingContinuation { continuation in
        self.continuation = continuation
        let session = ASWebAuthenticationSession(url: url, callbackURLScheme: "tinys") { url, error in
          Task { @MainActor in
            if let error = error as? ASWebAuthenticationSessionError, error.code == .canceledLogin {
              self.finish(.failure(AppleWebCallback.Failure.cancelled))
            } else if let error {
              self.finish(.failure(error))
            } else if let url {
              self.finish(.success(url))
            } else {
              self.finish(.failure(AppleWebCallback.Failure.invalidCallback))
            }
          }
        }
        self.session = session
        session.presentationContextProvider = self
        session.prefersEphemeralWebBrowserSession = false
        if !session.start() { finish(.failure(AppleWebCallback.Failure.invalidCallback)) }
      }
    }

    private func finish(_ result: Result<URL, Error>) {
      continuation?.resume(with: result)
      continuation = nil
      session = nil
    }

    func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
      NSApplication.shared.keyWindow ?? NSApplication.shared.windows.first ?? ASPresentationAnchor()
    }
  }
#endif

/// Sign in with Apple through a named Firebase app, started only by an account action or for a
/// device that has signed in before. It never enables the default app's Analytics/Crashlytics
/// collection in Debug or on the phone.
@MainActor public final class SharingAccount: ObservableObject {
  public static let shared = SharingAccount()
  private init() {}
  @Published public private(set) var signedIn = false
  private static let signedInKey = "sharing.signedIn"
  /// For an explicit action such as creating or stopping a link: starts Firebase if needed.
  public func token() async throws -> String {
    configure()
    guard !deleting else { throw TinyFailure("Account deletion is in progress.") }
    guard let user = auth?.currentUser else { throw TinyFailure("Sign in with Apple first.") }
    return try await user.getIDToken()
  }
  /// For background recovery: nil, without starting Firebase, until this device has signed in.
  public func recoveryToken() async -> String? {
    guard auth != nil || UserDefaults.standard.bool(forKey: Self.signedInKey) else { return nil }
    return try? await token()
  }
  @Published var fingerprint: String?
  @Published public private(set) var message = ""
  @Published var busy = false
  private var auth: Auth?
  private var nonce: String?
  public func configure() {
    guard auth == nil else { return }
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
    UserDefaults.standard.set(signedIn, forKey: Self.signedInKey)
    fingerprint = auth?.currentUser.map { user in
      SHA256.hash(data: Data(user.uid.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
    }
  }
  private func randomValue() throws -> String {
    var bytes = [UInt8](repeating: 0, count: 32)
    guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
      throw TinyFailure("Could not prepare sign-in. Try again.")
    }
    return bytes.map { String(format: "%02x", $0) }.joined()
  }
  public func prepare(_ request: ASAuthorizationAppleIDRequest) {
    nonce = nil
    do {
      let raw = try randomValue()
      nonce = raw
      request.nonce = SHA256.hash(data: Data(raw.utf8)).map { String(format: "%02x", $0) }.joined()
      request.requestedScopes = []
    } catch { message = "Could not prepare sign-in. Try again." }
  }
  /// The native button supplies its result on iPhone; deletion requests a fresh authorization.
  private func appleCredential(_ result: Result<ASAuthorization, Error>? = nil) async throws -> (
    credential: AuthCredential, appleUser: String, code: String?
  ) {
    #if os(macOS)
      let raw = try randomValue()
      let state = try randomValue()
      var url = URLComponents(string: "https://appleid.apple.com/auth/authorize")!
      url.queryItems = [
        URLQueryItem(name: "client_id", value: SharingLinks.appleSignIn.servicesID),
        URLQueryItem(name: "redirect_uri", value: SharingLinks.appleSignIn.returnURL),
        URLQueryItem(name: "response_type", value: "code id_token"),
        URLQueryItem(name: "response_mode", value: "fragment"),
        URLQueryItem(
          name: "nonce", value: SHA256.hash(data: Data(raw.utf8)).map { String(format: "%02x", $0) }.joined()),
        URLQueryItem(name: "state", value: state),
      ]
      let callback = try await WebAppleAuthorization().request(url.url!)
      let response = try AppleWebCallback.parse(callback, state: state)
      return (
        OAuthProvider.appleCredential(withIDToken: response.idToken, rawNonce: raw, fullName: nil),
        response.appleUser, response.code
      )
    #else
      let authorization: ASAuthorization
      if let result {
        authorization = try result.get()
      } else {
        let request = ASAuthorizationAppleIDProvider().createRequest()
        prepare(request)
        authorization = try await AccountAppleAuthorization().request(request)
      }
      guard let credential = authorization.credential as? ASAuthorizationAppleIDCredential,
        let bytes = credential.identityToken, let token = String(data: bytes, encoding: .utf8), let nonce
      else { throw TinyFailure("Apple did not return a sign-in credential. Try again.") }
      return (
        OAuthProvider.appleCredential(withIDToken: token, rawNonce: nonce, fullName: nil), credential.user,
        credential.authorizationCode.flatMap { String(data: $0, encoding: .utf8) }
      )
    #endif
  }
  public func complete(_ result: Result<ASAuthorization, Error>? = nil) async -> Bool {
    guard !busy else { return false }
    busy = true
    #if os(macOS)
      message = ""
    #endif
    defer {
      busy = false
      nonce = nil
    }
    do {
      guard let auth else { throw TinyFailure("Sign-in is unavailable in this app.") }
      let apple = try await appleCredential(result)
      _ = try await auth.signIn(with: apple.credential)
      updateIdentity()
      NotificationCenter.default.post(name: .tinysSharingAccountChanged, object: nil)
      try await verify()
      return true
    } catch AppleWebCallback.Failure.cancelled {
      return false
    } catch {
      // Auth errors can carry provider response data. Keep the UI/logs generic.
      message = "Could not finish signing in. Try again."
      return false
    }
  }
  private func verify() async throws {
    guard let user = auth?.currentUser else { throw TinyFailure("Sign in required") }
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
  /// Owned document ids, read once before the relay deletion and kept per account across restarts:
  /// a relay that is mid-deletion refuses ownership lookups, so a retry must not repeat them.
  private static func deletionSnapshotKey(_ uid: String) -> String {
    "sharing.deletion." + SHA256.hash(data: Data(uid.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
  }

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
      if relayDeleted { NotificationCenter.default.post(name: .tinysSharingAccountChanged, object: nil) }
    }
    var note = ""
    do {
      #if os(macOS)
        // Cancelling the browser sheet must not stop links or delete the account.
        let webAuthorization: Result<(credential: AuthCredential, appleUser: String, code: String?), Error>
        do {
          webAuthorization = .success(try await appleCredential())
        } catch AppleWebCallback.Failure.cancelled {
          return
        } catch {
          webAuthorization = .failure(error)
        }
      #endif
      if !deletionStarted {
        let token = try await user.getIDToken(forcingRefresh: true)
        let snapshotKey = Self.deletionSnapshotKey(user.uid)
        if let saved = UserDefaults.standard.stringArray(forKey: snapshotKey) {
          deletionDocuments = saved
        } else {
          deletionDocuments = try await Task.detached(priority: .utility) {
            var documents: [String] = []
            for (document, credential) in try SharingCredentials.all()
            where try sharingAccountOwns(document: document, credential: credential, token: token) {
              documents.append(document)
            }
            return documents
          }.value
          UserDefaults.standard.set(deletionDocuments, forKey: snapshotKey)
        }
        try await Task.detached(priority: .utility) { try sharingDeleteAccount(token: token) }.value
        deletionStarted = true
        relayDeleted = true
      }
      var apple: AuthCredential?
      do {
        #if os(macOS)
          let authorization = try webAuthorization.get()
        #else
          let authorization = try await appleCredential()
        #endif
        // Never revoke an Apple account different from the one being deleted.
        guard user.providerData.contains(where: { $0.providerID == "apple.com" && $0.uid == authorization.appleUser })
        else { throw TinyFailure("Apple sign-in did not match this account.") }
        apple = authorization.credential
        if let code = authorization.code {
          do { try await auth.revokeToken(withAuthorizationCode: code) } catch {
            note = " Apple access could not be revoked. Remove tinys in your Apple Account settings."
          }
        } else {
          note = " Apple access could not be revoked. Remove tinys in your Apple Account settings."
        }
      } catch {
        note = " Apple access was not revoked. Remove tinys in your Apple Account settings."
      }
      do { try await user.delete() } catch {
        guard (error as NSError).code == AuthErrorCode.requiresRecentLogin.rawValue, let apple else {
          throw error
        }
        _ = try await user.reauthenticate(with: apple)
        try await user.delete()
      }
      deletionStarted = false
      UserDefaults.standard.removeObject(forKey: Self.deletionSnapshotKey(user.uid))
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
      NotificationCenter.default.post(name: .tinysSharingAccountChanged, object: nil)
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
      if let fingerprint = account.fingerprint {
        Text("Signed in with Apple")
        Text("Account code: \(fingerprint.prefix(8))").font(.footnote).textSelection(.enabled)
        Button("Sign Out") { account.signOut() }
        Button("Delete Account…", role: .destructive) { confirmingDeletion = true }
      } else {
        #if os(macOS)
          SharingAppleSignInButton { Task { _ = await account.complete() } }
        #else
          SignInWithAppleButton(.signIn, onRequest: account.prepare) { result in
            Task { _ = await account.complete(result) }
          }.frame(height: 44)
        #endif
        Text("Sign in to start live links. Opening a shared link doesn't need an account.")
          .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
      }
      if !account.message.isEmpty {
        Text(account.message).foregroundStyle(.red).fixedSize(horizontal: false, vertical: true)
      }
      if account.busy { ProgressView() }
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

#if os(macOS)
  /// Mac sign-in uses the browser session rather than the native Apple authorization button.
  public struct SharingAppleSignInButton: View {
    private let action: () -> Void
    public init(action: @escaping () -> Void) { self.action = action }
    public var body: some View {
      Button(action: action) {
        HStack(spacing: 6) {
          Image(systemName: "apple.logo")
          Text("Sign in with Apple")
        }
        .font(.system(size: 17, weight: .medium))
        .frame(maxWidth: .infinity, minHeight: 44)
        .foregroundStyle(.white)
        .background(.black, in: RoundedRectangle(cornerRadius: 6))
      }
      .buttonStyle(.plain)
    }
  }
#endif
