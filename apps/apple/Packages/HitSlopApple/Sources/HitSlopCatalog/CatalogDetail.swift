import AppKit
import HitSlopCore
import HitSlopFeatures
import SwiftUI

struct CatalogDetail: View {
  let section: String
  let entry: CatalogEntry?
  let isCreating: Bool
  let isCopying: Bool
  let isQuitting: Bool
  let primaryAction: (CatalogEntry) -> Void

  var body: some View {
    VStack(spacing: 0) {
      HStack(spacing: 8) {
        Text(section).foregroundStyle(.secondary)
        if let entry {
          Text("/").foregroundStyle(.tertiary)
          Text(entry.displayTitle).lineLimit(1).truncationMode(.middle)
        }
        Spacer()
      }
      .font(.callout)
      .padding(.horizontal, 28)
      .frame(height: CatalogStyle.titlebar)
      .overlay(alignment: .bottom) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1).padding(.horizontal, 28) }

      if let entry {
        ScrollView {
          VStack(alignment: .leading, spacing: 18) {
            hero(entry)

            if !entry.description.isEmpty {
              Text(entry.description)
                .font(.body)
                .fixedSize(horizontal: false, vertical: true)
            }

            if !entry.categories.isEmpty {
              HStack(spacing: 7) {
                ForEach(entry.categories, id: \.self) { category in
                  Text(category.label).font(.callout)
                    .foregroundStyle(CatalogStyle.brand)
                    .padding(.horizontal, 11).padding(.vertical, 4)
                    .background(CatalogStyle.selection, in: Capsule())
                }
              }
            }

            PreviewCard(artwork: entry.previews)

            details(entry)

            if let authorName = entry.authorName {
              footer(authorName: authorName, url: entry.authorURL)
            }
          }
          .padding(.horizontal, 28)
          .padding(.top, 22)
          .padding(.bottom, 88)
          .frame(maxWidth: 760, alignment: .leading)
          .frame(maxWidth: .infinity, alignment: .leading)
        }
        .overlay(alignment: .bottomTrailing) {
          PrimaryButton(
            title: entry.isRecent ? "Open" : "Create",
            icon: entry.isRecent ? "arrow.up.forward.square" : "plus",
            isBusy: isCreating, isWorking: isCopying
          ) { primaryAction(entry) }
          .disabled(isQuitting)
          .padding(24)
        }
      } else {
        ContentUnavailableView(
          "Select a slop", systemImage: "square.stack.3d.up",
          description: Text("Choose an item from the list to see its preview and details.")
        )
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      }
    }
    .background(CatalogStyle.content.ignoresSafeArea())
  }

  private func hero(_ entry: CatalogEntry) -> some View {
    HStack(alignment: .top, spacing: 16) {
      IconTile(artwork: entry.icons, size: 64, radius: 14)
      VStack(alignment: .leading, spacing: 3) {
        Text(entry.displayTitle)
          .font(SlopBrand.heading(28, relativeTo: .title))
          .lineLimit(nil)
          .fixedSize(horizontal: false, vertical: true)
        if let identity = entry.documentIdentity {
          Text(entry.title).font(.body).foregroundStyle(.secondary)
          Label(folderName(identity.folderPath), systemImage: "folder")
            .font(.callout).foregroundStyle(.secondary)
            .labelStyle(.titleAndIcon)
            .help(identity.path)
        } else {
          Text(entry.isBundled ? "Included with hitSlop" : "Installed template")
            .font(.body).foregroundStyle(.secondary)
        }
      }
      .padding(.top, 2)
      .frame(maxWidth: .infinity, alignment: .leading)
    }
  }

  private func details(_ entry: CatalogEntry) -> some View {
    Grid(alignment: .leading, horizontalSpacing: 24, verticalSpacing: 8) {
      ForEach(facts(for: entry)) { fact in
        GridRow {
          Text(fact.title).foregroundStyle(.secondary)
          Text(fact.value)
        }
      }
    }
    .font(.callout)
    .frame(maxWidth: .infinity, alignment: .leading)
    .padding(.top, 16)
    .overlay(alignment: .top) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1) }
  }

  private func footer(authorName: String, url: URL?) -> some View {
    HStack(spacing: 14) {
      (Text("Made by ").foregroundStyle(.secondary) + Text(authorName))
      if let url {
        Link(destination: url) {
          HStack(spacing: 4) {
            Text(url.absoluteString).lineLimit(1).truncationMode(.middle)
            Image(systemName: "arrow.up.right.square")
          }
        }
        .foregroundStyle(CatalogStyle.brand)
      }
      Spacer()
    }
    .font(.callout)
    .padding(.top, 16)
    .overlay(alignment: .top) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1) }
  }

  private func facts(for entry: CatalogEntry) -> [CatalogFact] {
    var result = [
      CatalogFact(
        title: "Source",
        value: entry.isRecent ? "Local document" : entry.isBundled ? "Included with hitSlop" : "Installed locally")
    ]
    if entry.isRecent, let date = entry.createdAt {
      result.append(CatalogFact(title: "Created", value: date.formatted(date: .abbreviated, time: .omitted)))
    }
    if let size = entry.initialSize { result.append(CatalogFact(title: "Initial size", value: size)) }
    if entry.fileBytes > 0 {
      result.append(
        CatalogFact(
          title: "File size", value: ByteCountFormatter.string(fromByteCount: entry.fileBytes, countStyle: .file)))
    }
    if let date = entry.updatedAt {
      result.append(CatalogFact(title: "Updated", value: date.formatted(date: .abbreviated, time: .shortened)))
    }
    return result
  }
}

private struct PreviewCard: View {
  let artwork: [CatalogArtwork]

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      Text("PREVIEW")
        .font(.caption2.weight(.semibold)).tracking(0.6)
        .foregroundStyle(.secondary)
      CatalogImageView(artwork: artwork, fallback: .preview)
        .frame(maxWidth: .infinity)
    }
    .padding(14).padding(.bottom, 6)
    .background(Color.primary.opacity(0.025), in: RoundedRectangle(cornerRadius: 14, style: .continuous))
    .overlay(RoundedRectangle(cornerRadius: 14, style: .continuous).strokeBorder(CatalogStyle.hairline))
  }
}

private struct PrimaryButton: View {
  let title: String
  let icon: String
  /// Disables the button, e.g. while the save panel is open.
  let isBusy: Bool
  /// Show copying feedback only if the copy lasts more than 300 ms.
  let isWorking: Bool
  let action: () -> Void
  @State private var isHovering = false
  @State private var showsWork = false

  var body: some View {
    Button(action: action) {
      HStack(spacing: 8) {
        if showsWork {
          ProgressView().controlSize(.small).tint(.white)
        } else {
          Image(systemName: icon)
        }
        Text(showsWork ? "Creating…" : title)
        if !showsWork {
          KeyCap("⌘↩", foreground: .white.opacity(0.75), border: .white.opacity(0.3))
        }
      }
      .font(.body.weight(.semibold))
      .foregroundStyle(.white)
      .padding(.horizontal, 16)
      .frame(height: 38)
      .background(
        isHovering ? CatalogStyle.brandHover : CatalogStyle.brand,
        in: RoundedRectangle(cornerRadius: 10, style: .continuous)
      )
      .shadow(color: CatalogStyle.brand.opacity(isHovering ? 0.32 : 0.22), radius: isHovering ? 10 : 6, y: 3)
      .fixedSize()
    }
    .buttonStyle(.plain)
    .keyboardShortcut(.return, modifiers: .command)
    .disabled(isBusy || isWorking)
    .task(id: isWorking) {
      guard isWorking else {
        showsWork = false
        return
      }
      try? await Task.sleep(for: .milliseconds(300))
      if !Task.isCancelled { showsWork = true }
    }
    .onHover { isHovering = $0 }
    .animation(.easeOut(duration: 0.12), value: isHovering)
  }
}

private struct CatalogFact: Identifiable {
  let title: String
  let value: String
  var id: String { title }
}
