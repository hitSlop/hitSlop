@preconcurrency import FirebaseAuth
import PopCore

/// Firebase supplies identity; Rust owns the account library connection.
@MainActor enum CloudAccount {
  static func connect(auth: Auth, user: User?) async {
    guard auth.currentUser?.uid == user?.uid else { return }
    do {
      try await PopLibrary.shared.connectAccount(user?.uid ?? "", anonymous: user?.isAnonymous ?? true)
    } catch {
      guard auth.currentUser?.uid == user?.uid else { return }
      PopLibrary.shared.reportAccountError(error, account: user?.uid ?? "")
    }
  }
}
