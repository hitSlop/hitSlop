import Foundation
import PopCoreBinding

extension Notification.Name {
  public static let popDocumentSaveStatusChanged = Notification.Name("click.dotpop.document-save-status-changed")
  public static let popSharingAccountChanged = Notification.Name("click.dotpop.sharing-account-changed")
  public static let popSharingForeground = Notification.Name("click.dotpop.sharing-foreground")
  public static let popJoinSharedPop = Notification.Name("click.dotpop.join-shared-pop")
}

/// Parses live collaboration invitations on Mac and iPhone.
public enum SharingLinks {
  public static let appleSignIn = (
    servicesID: "com.hitslop.app.signin",
    returnURL: "https://share.dotpop.click/auth/apple"
  )

  /// Preserve the full URL, including its capability fragment, through the Rust parser.
  /// A non-sharing URL is left to the caller's existing routing.
  public static func incomingLink(_ url: URL) throws -> String? {
    let custom =
      url.scheme == "dotpop" && url.host == "join" && url.path.isEmpty
      && url.query == nil && url.user == nil && url.password == nil && url.port == nil
    let universal = url.scheme == "https" && url.host == "share.dotpop.click" && url.path == "/join"
    guard custom || universal else { return nil }
    guard let fragment = url.fragment, !fragment.isEmpty else {
      throw PopFailure("Open the link in pop again")
    }
    let link = custom ? "https://share.dotpop.click/join#" + fragment : url.absoluteString
    _ = try sharingDocument(link: link)
    return link
  }
}
