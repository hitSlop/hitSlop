import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

/// The same deliberate help flow in both catalogs. Background failures never present it.
public struct LibrarySyncStatusView: View {
  @ObservedObject private var library: TinyLibrary
  @State private var showing = false
  @State private var copied = false
  @Environment(\.openURL) private var openURL

  public init(library: TinyLibrary) { self.library = library }

  public var body: some View {
    Group {
      if library.attention != nil {
        Button {
          showing = true
        } label: {
          Label("iCloud needs attention", systemImage: "exclamationmark.icloud")
            .padding(.horizontal, 12).frame(minHeight: 44)
        }
        .accessibilityHint("Shows sync issues, retry, and help")
      } else {
        Text(library.status).foregroundStyle(.secondary)
      }
    }
    .font(.footnote).padding(.vertical, 6)
    .sheet(isPresented: $showing) {
      NavigationStack {
        ScrollView {
          VStack(alignment: .leading, spacing: 18) {
            Label(
              library.attention != nil ? "iCloud needs attention" : "iCloud is up to date",
              systemImage: library.attention != nil ? "exclamationmark.icloud" : "checkmark.icloud"
            )
            .font(.title2.bold())
            Text(library.attention != nil ? library.syncExplanation : "Your changes have synced successfully.")
            ForEach(library.entries.filter { library.issues[$0.id] != nil || $0.problem != nil }) { entry in
              Label(entry.title + (entry.deleted ? " · deletion pending" : ""), systemImage: "doc")
            }
            Button(library.isSyncing ? "Retrying…" : "Retry") {
              Task { await library.syncNow() }
            }.disabled(library.isSyncing || library.attention == nil)
            Text("If this keeps happening, copy the diagnostics and share them with us on Discord.")
              .foregroundStyle(.secondary)
            Button(copied ? "Copied" : "Copy diagnostics") {
              #if os(macOS)
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(library.syncReport, forType: .string)
              #else
                UIPasteboard.general.string = library.syncReport
              #endif
              copied = true
            }
            Button("Get help on Discord") { openURL(TinyLinks.discord) }
            DisclosureGroup("Technical details") {
              Text(library.syncReport).font(.caption.monospaced()).textSelection(.enabled)
            }
          }.padding(24)
        }
        .toolbar { Button("Done") { showing = false } }
      }
      .frame(minWidth: 300, idealWidth: 440, minHeight: 360)
    }
  }
}
