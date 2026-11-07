import AuthenticationServices
import CryptoKit
@preconcurrency import FirebaseAuth
import FirebaseCore
import PopCore
import PopCoreBinding
import Security
import SwiftUI

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
        let session = ASWebAuthenticationSession(url: url, callbackURLScheme: "dotpop") { @Sendable url, error in
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

/// The person's answer when Apple is asked to confirm an account deletion.
enum AccountDeletionConsent<Credential> {
  /// The person dismissed Apple's sheet: nothing is deleted.
  case cancelled
  /// Apple answered; a failure still deletes, without revoking Apple's access.
  case answered(Result<Credential, Error>)
}

/// How an account deletion attempt ended.
enum AccountDeletionOutcome: Equatable {
  case cancelled
  case deleted
  /// Nothing more was deleted; `cloudDeleted` says the cloud documents are already gone.
  case failed(cloudDeleted: Bool)
}

/// This device's account flags. Firebase keeps an identity in the Keychain; the retained flag
/// outlives its loss, so a missing identity is refused rather than replaced by a different
/// anonymous UID. Keys name the document lineage, so another lineage starts with its own.
struct SharingAccountDefaults {
  var defaults: UserDefaults = .standard
  private static let prefix = "click.dotpop.sharing.lineage\(documentLineage())."
  private static let retainedKey = prefix + "signedIn"
  /// This device has held an identity in this lineage's Auth.
  var identityRetained: Bool {
    get { defaults.bool(forKey: Self.retainedKey) }
    nonmutating set {
      if newValue {
        defaults.set(true, forKey: Self.retainedKey)
      } else {
        defaults.removeObject(forKey: Self.retainedKey)
      }
    }
  }
  /// A retained identity that Firebase no longer has.
  func identityMissing(hasUser: Bool) -> Bool { !hasUser && identityRetained }
  /// Only this UID's identity is left to delete. Sign-out keeps the mark: the relay fences a deleted UID for good.
  func cloudDeletedKey(_ uid: String) -> String {
    Self.prefix + "deletion." + SHA256.hash(data: Data(uid.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
  }
  /// Deletes `uid`'s account in order. Apple is asked first, so cancelling deletes nothing.
  /// The cloud documents go before the identity that owns them, once: a failure after they
  /// are gone keeps this UID's checkpoint, so a retry (or a relaunch) resumes at the identity.
  @MainActor func deleteAccount<Credential>(
    uid: String,
    consent: () async -> AccountDeletionConsent<Credential>,
    deleteCloud: () async throws -> Void,
    deleteIdentity: (Result<Credential, Error>) async throws -> Void
  ) async -> AccountDeletionOutcome {
    guard case .answered(let authorization) = await consent() else { return .cancelled }
    let checkpoint = cloudDeletedKey(uid)
    var cloudDeleted = defaults.bool(forKey: checkpoint)
    do {
      if !cloudDeleted {
        try await deleteCloud()
        cloudDeleted = true
        defaults.set(true, forKey: checkpoint)
      }
      try await deleteIdentity(authorization)
      defaults.removeObject(forKey: checkpoint)
      identityRetained = false
      return .deleted
    } catch {
      return .failed(cloudDeleted: cloudDeleted)
    }
  }
}

/// Apple's answer for this account: Firebase's credential, Apple's user, and the code that
/// revokes Apple's access.
private typealias AppleAuthorization = (credential: AuthCredential, appleUser: String, code: String?)

/// Retained anonymous identities accept invitations; Apple links the same UID for cloud creation.
/// Auth runs on the one default Firebase app, which the host configures at launch (`PopFirebase.configure()`).
@MainActor public final class SharingAccount: ObservableObject {
  public static let shared = SharingAccount()
  private init() {}
  @Published public private(set) var signedIn = false
  @Published public private(set) var hasIdentity = false
  public var identityID: String? { auth?.currentUser?.uid }
  private var observedUID: String?
  private var identityTask: Task<User, Error>?
  fileprivate var retainedIdentityMissing = false
  private let stored = SharingAccountDefaults()
  /// Creates a guest identity only when none is retained; credential failures preserve the current UID.
  public func token() async throws -> String {
    configure()
    guard !deleting else { throw PopFailure("Account deletion is in progress.") }
    let user = try await identity()
    guard let auth else { throw PopFailure("Sign-in is unavailable in this app.") }
    await CloudCatalog.shared.connect(auth: auth, user: user)
    guard auth.currentUser?.uid == user.uid, PopLibrary.shared.isReady,
      PopLibrary.shared.accountIdentity == user.uid, !deleting
    else {
      throw PopFailure("The account changed. Try again.")
    }
    let token = try await user.getIDToken()
    guard auth.currentUser?.uid == user.uid, PopLibrary.shared.accountIdentity == user.uid,
      PopLibrary.shared.isReady, !deleting
    else {
      throw PopFailure("The account changed. Try again.")
    }
    return token
  }
  @Published var fingerprint: String?
  @Published public private(set) var message = ""
  @Published var busy = false
  private var auth: Auth?
  private var tokenListener: NSObjectProtocol?
  private var nonce: String?
  public func configure() {
    guard auth == nil else { return }
    guard let app = FirebaseApp.app() else {
      message = "Sign-in is unavailable in this app."
      return
    }
    let auth = Auth.auth(app: app)
    self.auth = auth
    retainedIdentityMissing = stored.identityMissing(hasUser: auth.currentUser != nil)
    PopLibrary.shared.tokenProvider = { try await SharingAccount.shared.token() }
    tokenListener = auth.addIDTokenDidChangeListener { @Sendable _, user in
      let uid = user?.uid
      Task { @MainActor in
        guard let auth = self.auth, !self.deleting, auth.currentUser?.uid == uid else { return }
        let previous = self.observedUID
        self.updateIdentity()
        if previous != self.observedUID {
          NotificationCenter.default.post(name: .popSharingAccountChanged, object: nil)
        }
        await CloudCatalog.shared.connect(auth: auth, user: auth.currentUser)
        // Firebase refreshes the retained identity; push its latest token into
        // existing Rust connections even when no document sharing panel is open.
        await PopLibrary.shared.syncNow()
        NotificationCenter.default.post(name: .popSharingAccountChanged, object: nil)
      }
    }
    updateIdentity()
    Task {
      do { _ = try await token() } catch {
        message =
          retainedIdentityMissing
          ? "Your saved cloud identity could not be restored. Local documents remain available. Reset the identity explicitly before switching accounts."
          : "Cloud connection is unavailable. Your local documents remain available. Try again when online."
      }
    }
  }
  /// A retained credential failure is never replaced with a different anonymous UID.
  private func identity() async throws -> User {
    guard let auth else { throw PopFailure("Sign-in is unavailable in this app.") }
    if let user = auth.currentUser { return user }
    guard !retainedIdentityMissing, !stored.identityRetained else {
      throw PopFailure(
        "This device's cloud identity could not be restored. Your local documents are retained. Sign out explicitly to reset the identity."
      )
    }
    if let identityTask { return try await identityTask.value }
    let task = Task { try await auth.signInAnonymously().user }
    identityTask = task
    defer { identityTask = nil }
    return try await task.value
  }
  private func updateIdentity() {
    hasIdentity = auth?.currentUser != nil
    signedIn = auth?.currentUser?.isAnonymous == false
    if hasIdentity { stored.identityRetained = true }
    retainedIdentityMissing = stored.identityMissing(hasUser: hasIdentity)
    observedUID = auth?.currentUser?.uid
    fingerprint = auth?.currentUser.map { user in
      SHA256.hash(data: Data(user.uid.utf8)).prefix(8).map { String(format: "%02x", $0) }.joined()
    }
  }
  private func randomValue() throws -> String {
    var bytes = [UInt8](repeating: 0, count: 32)
    guard SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes) == errSecSuccess else {
      throw PopFailure("Could not prepare sign-in. Try again.")
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
  private func appleCredential(_ result: Result<ASAuthorization, Error>? = nil) async throws -> AppleAuthorization {
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
      else { throw PopFailure("Apple did not return a sign-in credential. Try again.") }
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
      configure()
      guard let auth else { throw PopFailure("Sign-in is unavailable in this app.") }
      guard !retainedIdentityMissing else {
        throw PopFailure("Sign out explicitly before replacing an identity that could not be restored.")
      }
      _ = try await identityTask?.value
      let apple = try await appleCredential(result)
      guard auth.currentUser != nil || !stored.identityRetained else {
        throw PopFailure("Sign out explicitly before replacing an identity that could not be restored.")
      }
      if let user = auth.currentUser, user.isAnonymous {
        // Linking preserves the UID that owns this device's accepted invitations.
        _ = try await user.link(with: apple.credential)
      } else {
        _ = try await auth.signIn(with: apple.credential)
      }
      updateIdentity()
      NotificationCenter.default.post(name: .popSharingAccountChanged, object: nil)
      try await verify()
      await CloudCatalog.shared.connect(auth: auth, user: auth.currentUser)
      return true
    } catch AppleWebCallback.Failure.cancelled {
      return false
    } catch {
      if (error as NSError).code == AuthErrorCode.credentialAlreadyInUse.rawValue {
        message =
          "This Apple account is already linked elsewhere. Your guest identity and local documents are retained. Sign out explicitly before switching accounts."
        return false
      }
      // Auth errors can carry provider response data. Keep the UI/logs generic.
      message = "Could not finish signing in. Try again."
      return false
    }
  }
  private func verify() async throws {
    guard let user = auth?.currentUser else { throw PopFailure("Sign in required") }
    let token = try await user.getIDToken(forcingRefresh: true)
    try await Task.detached(priority: .utility) {
      try sharingVerifyAccount(token: token)
    }.value
    message = ""
    updateIdentity()
  }
  private var deleting = false

  public func deleteAccount() async {
    guard !busy, let auth, let user = auth.currentUser else { return }
    busy = true
    deleting = true
    message = ""
    defer {
      deleting = false
      busy = false
      nonce = nil
      updateIdentity()
    }
    var note = ""
    let outcome = await stored.deleteAccount(
      uid: user.uid,
      consent: { () async -> AccountDeletionConsent<AppleAuthorization> in
        // Cancelling Apple's sheet must not stop links or delete anything.
        do { return .answered(.success(try await appleCredential())) } catch AppleWebCallback.Failure.cancelled {
          return .cancelled
        } catch ASAuthorizationError.canceled {
          return .cancelled
        } catch {
          return .answered(.failure(error))
        }
      },
      deleteCloud: {
        let token = try await user.getIDToken(forcingRefresh: true)
        try await PopLibrary.shared.setAccount("")
        try await Task.detached(priority: .utility) { try sharingDeleteAccount(token: token) }.value
      },
      deleteIdentity: { authorization in
        var apple: AuthCredential?
        do {
          let authorization = try authorization.get()
          // Never revoke an Apple account different from the one being deleted.
          guard
            user.providerData.contains(where: { $0.providerID == "apple.com" && $0.uid == authorization.appleUser })
          else { throw PopFailure("Apple sign-in did not match this account.") }
          apple = authorization.credential
          if let code = authorization.code {
            do { try await auth.revokeToken(withAuthorizationCode: code) } catch {
              note = " Apple access could not be revoked. Remove pop in your Apple Account settings."
            }
          } else {
            note = " Apple access could not be revoked. Remove pop in your Apple Account settings."
          }
        } catch {
          note = " Apple access was not revoked. Remove pop in your Apple Account settings."
        }
        do { try await user.delete() } catch {
          guard (error as NSError).code == AuthErrorCode.requiresRecentLogin.rawValue, let apple else {
            throw error
          }
          _ = try await user.reauthenticate(with: apple)
          try await user.delete()
        }
      })
    switch outcome {
    case .cancelled:
      return
    case .deleted:
      message = "Account deleted." + note
    case .failed(let cloudDeleted):
      message =
        cloudDeleted
        ? "Your cloud documents were deleted. Could not finish deleting the account. Try again." + note
        : "Could not delete the account. Try again."
    }
  }
  public func signOut() {
    guard !busy else { return }
    busy = true
    Task {
      defer { busy = false }
      do {
        // Finish an in-flight first identity before resetting it, so its late
        // completion cannot sign this device back in after an explicit sign-out.
        _ = try? await identityTask?.value
        // Flush and fence Rust transport before changing identity. Failure keeps ownership.
        try await PopLibrary.shared.setAccount("")
        try auth?.signOut()
        stored.identityRetained = false
        retainedIdentityMissing = false
        updateIdentity()
        message = ""
      } catch { message = "Could not save and sign out. Your documents remain open. Retry." }
    }
  }
}

public struct SharingAccountView: View {
  @ObservedObject private var account = SharingAccount.shared
  @State private var confirmingDeletion = false
  public init() {}
  public var body: some View {
    VStack(alignment: .leading, spacing: 16) {
      Text("Account").font(.title2.bold())
      if account.signedIn, let fingerprint = account.fingerprint {
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
        if account.hasIdentity || account.retainedIdentityMissing {
          Button(account.retainedIdentityMissing ? "Reset Missing Cloud Identity" : "Sign Out of Guest Identity") {
            account.signOut()
          }
        }
        Text("Sign in to start live links. Shared links use a guest identity on this device.")
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
        "Your cloud documents and shared links will be deleted. Downloaded files stay on your devices."
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
