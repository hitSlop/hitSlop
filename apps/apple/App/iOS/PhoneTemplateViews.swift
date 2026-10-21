import HitSlopCore
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
    .alert("Name your slop", isPresented: $showName) {
      TextField("Name", text: $name).accessibilityIdentifier("slop-name")
      Button("Cancel", role: .cancel) {}
      Button("Create") { create(name.trimmingCharacters(in: .whitespacesAndNewlines)) }
        .disabled(creating || name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }
    .toolbar(.visible, for: .navigationBar)
    .navigationBarTitleDisplayMode(.inline)
    .safeAreaInset(edge: .bottom) {
      VStack(spacing: 8) {
        Button {
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
