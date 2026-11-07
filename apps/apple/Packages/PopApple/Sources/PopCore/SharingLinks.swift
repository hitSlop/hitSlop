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
    guard url.scheme == "https", url.host == "share.dotpop.click", url.path == "/join" else { return nil }
    guard let fragment = url.fragment, !fragment.isEmpty else {
      throw PopFailure("Open the link in pop again")
    }
    let link = url.absoluteString
    _ = try sharingDocument(link: link)
    return link
  }
}
