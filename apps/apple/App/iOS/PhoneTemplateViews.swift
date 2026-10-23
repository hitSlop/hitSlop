import HitSlopCore
import HitSlopDocument
import SwiftUI
import UIKit

struct PhoneTemplate: Identifiable, Hashable {
  let url: URL, title: String, description: String, categories: [String]
  var id: URL { url }
}
struct PhoneArtworkView: View {
  let url: URL
  var preview = false
  var revision = ""
  @State private var image: UIImage?
  var body: some View {
    Group {
      if let image {
        Image(uiImage: image).resizable().scaledToFit()
      } else {
        Image(systemName: "doc.text").resizable().scaledToFit().foregroundStyle(PhoneStyle.accent.opacity(0.6)).padding(
          12)
      }
    }.task(id: url.path + "|" + revision) {
      let preview = preview
      let data = await Task.detached(priority: .utility) {
        try? SlopArtwork.first(url, preview ? [.preview] : [.icon, .preview])?.png
      }.value
      image = data.flatMap(UIImage.init(data:))
    }
  }
}
struct PhoneSlopRow: View {
  let url: URL, title: String, subtitle: String
  var revision = ""
  /// A synced row shows a small iCloud mark instead of a status line.
  var synced = false
  var body: some View {
    HStack(spacing: 12) {
      PhoneArtworkView(url: url, revision: revision).padding(3).frame(width: 44, height: 46)
        .background(Color(.systemBackground), in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.primary.opacity(0.08), lineWidth: 1))
      VStack(alignment: .leading, spacing: 2) {
        Text(title).font(SlopBrand.heading(16)).foregroundStyle(.primary).lineLimit(1)
        if !synced { Text(subtitle).font(.caption).foregroundStyle(.secondary).lineLimit(2) }
      }.frame(maxWidth: .infinity, alignment: .leading)
      if synced {
        Image(systemName: "checkmark.icloud").font(.footnote).foregroundStyle(.tertiary)
          .accessibilityLabel(subtitle)
      }
      Image(systemName: "chevron.right").font(.caption.weight(.semibold)).foregroundStyle(.tertiary)
    }.padding(.vertical, 6).padding(.horizontal, 8).contentShape(Rectangle())
      .overlay(alignment: .bottom) {
        Rectangle().fill(Color.primary.opacity(0.065)).frame(height: 0.5).padding(.leading, 64)
      }
  }
}
struct PhoneTemplateDetail: View {
  let template: PhoneTemplate
  let creating: Bool, canCreate: Bool
  let create: (String) -> Void
  @State private var showName = false
  /// The name chosen in the sheet, created once the sheet has closed: navigation and any
  /// error alert cannot present from behind a sheet.
  @State private var chosenName: String?
  @State private var taps = 0
  @State private var name = ""
  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 22) {
        HStack(spacing: 18) {
          PhoneArtworkView(url: template.url).padding(6).frame(width: 92, height: 100)
            .background(Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 18))
          VStack(alignment: .leading, spacing: 6) {
            Text(template.title).font(SlopBrand.heading(26, relativeTo: .title2)).tracking(-0.4)
            Text("Included with hitSlop").foregroundStyle(.secondary)
          }
        }
        Text(template.description).font(.body).lineSpacing(4)
        HStack {
          ForEach(template.categories, id: \.self) { name in
            Text(name).font(.subheadline.weight(.medium)).foregroundStyle(PhoneStyle.accent)
              .padding(.horizontal, 17).padding(.vertical, 9).background(PhoneStyle.accent.opacity(0.09), in: Capsule())
          }
        }
        VStack(alignment: .leading, spacing: 16) {
          Text("PREVIEW").font(.caption.weight(.semibold)).tracking(0.8).foregroundStyle(.secondary)
          PhoneArtworkView(url: template.url, preview: true).frame(maxWidth: .infinity)
            .clipShape(RoundedRectangle(cornerRadius: 8))
        }.padding(20).background(Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 20))
      }.padding(24)
    }
    .appHaptic(.impact(weight: .light), trigger: taps)
    .sheet(
      isPresented: $showName,
      onDismiss: {
        guard let chosen = chosenName else { return }
        chosenName = nil
        create(chosen)
      }
    ) {
      PhoneNameSheet(template: template, creating: creating, name: $name) { chosen in
        chosenName = chosen
        showName = false
      }
    }
    .toolbar(.visible, for: .navigationBar)
    .navigationBarTitleDisplayMode(.inline)
    .safeAreaInset(edge: .bottom) {
      VStack(spacing: 8) {
        Button {
          taps += 1
          name = template.title
          showName = true
        } label: {
          HStack(spacing: 12) {
            Image(systemName: "plus")
            Text(creating ? "Creating…" : "Create slop").fontWeight(.semibold)
          }
          .font(.title3).frame(maxWidth: .infinity).padding(.vertical, 18)
        }.buttonStyle(.plain).foregroundStyle(.white).background(
          PhoneStyle.accent, in: RoundedRectangle(cornerRadius: 18)
        )
        .accessibilityIdentifier("create-slop")
        .disabled(creating || !canCreate).opacity(canCreate ? 1 : 0.55)
        Text(
          canCreate
            ? "Saved on this iPhone. Synced with your Mac." : "Connect to iCloud in Settings to create a synced slop."
        )
        .font(.caption).foregroundStyle(.secondary)
      }.padding(.horizontal, 20).padding(.top, 10).padding(.bottom, 8).background(.regularMaterial)
    }
  }
}

/// Names a new slop before it is made: the template's icon, the name ready to type over,
/// and one Create that rides above the keyboard.
private struct PhoneNameSheet: View {
  let template: PhoneTemplate
  let creating: Bool
  @Binding var name: String
  let create: (String) -> Void
  @Environment(\.dismiss) private var dismiss
  @FocusState private var focused: Bool
  @State private var selection: TextSelection?
  private var trimmed: String { name.trimmingCharacters(in: .whitespacesAndNewlines) }

  var body: some View {
    NavigationStack {
      VStack(spacing: 20) {
        PhoneArtworkView(url: template.url).padding(6).frame(width: 84, height: 92)
          .background(Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 18))
        TextField("Name", text: $name, selection: $selection)
          .font(.title3).focused($focused).submitLabel(.done)
          .onSubmit { if !trimmed.isEmpty { create(trimmed) } }
          .padding(.horizontal, 16).padding(.vertical, 14)
          .background(Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 14))
          .accessibilityLabel("Slop name").accessibilityIdentifier("slop-name")
        Text("Saved on this iPhone. Synced with your Mac.").font(.caption).foregroundStyle(.secondary)
        Spacer(minLength: 0)
      }
      .padding(24)
      .safeAreaInset(edge: .bottom) {
        Button {
          create(trimmed)
        } label: {
          Text(creating ? "Creating…" : "Create slop").fontWeight(.semibold)
            .font(.title3).frame(maxWidth: .infinity).padding(.vertical, 8)
        }
        .buttonStyle(.borderedProminent).buttonBorderShape(.roundedRectangle(radius: 18))
        .tint(PhoneStyle.accent).controlSize(.large)
        .disabled(creating || trimmed.isEmpty)
        .accessibilityIdentifier("create-named-slop")
        .padding(.horizontal, 20).padding(.bottom, 12)
      }
      .navigationTitle("New Slop").navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) { Button("Cancel") { dismiss() } }
      }
      .onAppear {
        // The title is ready to type over, as when naming a new list.
        selection = TextSelection(range: name.startIndex..<name.endIndex)
        focused = true
      }
    }
    .presentationDetents([.large])
  }
}
