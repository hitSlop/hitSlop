import HitSlopCoreBinding
import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

public struct DocumentSharingView: View {
  @ObservedObject var model: DocumentSharing
  private let onDone: (() -> Void)?
  @Environment(\.dismiss) private var dismiss
  public init(model: DocumentSharing, onDone: (() -> Void)? = nil) {
    self.model = model
    self.onDone = onDone
  }
  public var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      Text("Live Sharing Preview").font(.title2.bold())
      Text("Anyone with the link can edit this document and access its retained history and files.")
        .foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
      if let status = model.status {
        Text(status.phase == .ended ? "Sharing ended. Your local copy is kept." : status.detail)
          .accessibilityIdentifier("sharing-status")
        if status.phase != .ended, let link = status.link {
          Button("Copy Edit Link") {
            #if os(macOS)
              NSPasteboard.general.clearContents()
              NSPasteboard.general.setString(link, forType: .string)
            #else
              UIPasteboard.general.string = link
            #endif
          }.accessibilityIdentifier("sharing-link")
          ShareLink("Share Edit Link", item: link)
          if status.creator { Button("Stop Sharing", role: .destructive) { model.stop() } }
        } else if status.phase == .preparing {
          Button("Finish Creating Edit Link") { model.create() }
        }
      } else {
        Button("Create Edit Link") { model.create() }.accessibilityIdentifier("sharing-create")
      }
      if let error = model.error {
        Text(error).foregroundStyle(.red).textSelection(.enabled)
        Button("Retry Connection") { model.retry() }
      }
      if model.busy { ProgressView() }
      Text("This is a developer preview. Keep a backup of important work.").font(.footnote).foregroundStyle(.secondary)
      HStack {
        Spacer()
        Button("Done") { if let onDone { onDone() } else { dismiss() } }
      }
    }.padding(24).frame(idealWidth: 420).disabled(model.busy)
  }
}

public struct SharingJoinView: View {
  public let join: @MainActor (String) async throws -> Void
  @State private var link = ""
  @State private var busy = false
  @State private var error: String?
  @Environment(\.dismiss) private var dismiss
  public init(join: @escaping @MainActor (String) async throws -> Void) { self.join = join }
  public var body: some View {
    VStack(alignment: .leading, spacing: 18) {
      Text("Join a Shared Slop").font(.title2.bold())
      TextField("Paste an edit link", text: $link).textFieldStyle(.roundedBorder)
        .accessibilityIdentifier("sharing-join-link")
      if let error { Text(error).foregroundStyle(.red).textSelection(.enabled) }
      if busy { ProgressView() }
      HStack {
        Button("Cancel", role: .cancel) { dismiss() }
        Spacer()
        Button("Join") {
          busy = true
          error = nil
          Task {
            defer { busy = false }
            do {
              try await join(link.trimmingCharacters(in: .whitespacesAndNewlines))
              dismiss()
            } catch { self.error = error.localizedDescription }
          }
        }.disabled(link.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
      }
    }.padding(24).frame(idealWidth: 440).disabled(busy)
  }
}
