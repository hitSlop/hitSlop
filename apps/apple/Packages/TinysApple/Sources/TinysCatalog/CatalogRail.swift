import AppKit
import SwiftUI
import TinysCore
import TinysFeatures

struct CatalogRail: View {
  let title: String
  let entries: [CatalogEntry]
  @Binding var selectedID: String?
  @Binding var query: String
  var searchFocused: FocusState<Bool>.Binding
  let localIssues: [String]
  let onRefresh: () -> Void
  var onDelete: (CatalogEntry) -> Void = { _ in }

  var body: some View {
    VStack(spacing: 0) {
      CatalogSearchBar(query: $query, focused: searchFocused)
        .padding(.horizontal, 14).padding(.top, 14)

      HStack(alignment: .center) {
        Text(title).font(TinyBrand.heading(20, relativeTo: .title3))
        Spacer()
        QuietIconButton(systemName: "arrow.clockwise", help: "Refresh", action: onRefresh)
          .accessibilityLabel("Refresh catalog")
      }
      .padding(.leading, 16).padding(.trailing, 12).padding(.top, 16).padding(.bottom, 10)
      .overlay(alignment: .bottom) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1).padding(.horizontal, 14) }

      if !localIssues.isEmpty {
        Label(
          "\(localIssues.count) local template \(localIssues.count == 1 ? "needs" : "need") attention",
          systemImage: "exclamationmark.triangle.fill"
        )
        .font(.caption).foregroundStyle(Color.orange)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 10).padding(.vertical, 7)
        .background(Color.orange.opacity(0.09), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
        .padding(.horizontal, 14).padding(.top, 10)
        .help(localIssues.joined(separator: "\n"))
      }

      if entries.isEmpty {
        ContentUnavailableView(
          "Nothing here", systemImage: "sparkles.rectangle.stack", description: Text("Try another search or category.")
        )
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else {
        ScrollViewReader { proxy in
          ScrollView {
            LazyVStack(spacing: 2) {
              ForEach(entries) { entry in
                CatalogRow(entry: entry, selected: entry.id == selectedID) { selectedID = entry.id }
                  .id(entry.id)
                  .contextMenu {
                    if entry.isSynced,
                      TinyLibrary.shared.visibleEntries.contains(where: { "library:" + $0.id == entry.id })
                    {
                      Button("Delete Tiny", role: .destructive) {
                        onDelete(entry)
                      }
                    }
                  }
              }
            }
            .padding(.horizontal, 8).padding(.vertical, 8)
          }
          .focusable()
          .focusEffectDisabled()
          .onMoveCommand { direction in
            guard let next = neighbor(direction) else { return }
            selectedID = next
            withAnimation(.easeOut(duration: 0.12)) { proxy.scrollTo(next) }
          }
        }
      }

      Text(entries.count == 1 ? "1 tiny" : "\(entries.count) tinys")
        .font(.caption).foregroundStyle(.tertiary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16).padding(.vertical, 12)
    }
    .background(CatalogStyle.content)
  }

  private func neighbor(_ direction: MoveCommandDirection) -> String? {
    let index = entries.firstIndex { $0.id == selectedID }
    switch direction {
    case .up: return index.map { entries[max($0 - 1, 0)].id } ?? entries.last?.id
    case .down: return index.map { entries[min($0 + 1, entries.count - 1)].id } ?? entries.first?.id
    default: return nil
    }
  }
}

private struct QuietIconButton: View {
  let systemName: String
  let help: String
  let action: () -> Void
  @State private var isHovering = false

  var body: some View {
    Button(action: action) {
      Image(systemName: systemName)
        .font(.system(size: 12, weight: .semibold))
        .foregroundStyle(isHovering ? .primary : .secondary)
        .frame(width: 26, height: 26)
        .background(isHovering ? CatalogStyle.hover : .clear, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .onHover { isHovering = $0 }
    .help(help)
  }
}

private struct CatalogSearchBar: View {
  @Binding var query: String
  var focused: FocusState<Bool>.Binding

  var body: some View {
    HStack(spacing: 8) {
      Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
      TextField("Search tinys", text: $query)
        .textFieldStyle(.plain).focused(focused)
      if query.isEmpty {
        KeyCap("⌘K")
      } else {
        Button {
          query = ""
        } label: {
          Image(systemName: "xmark.circle.fill").foregroundStyle(.tertiary)
        }
        .buttonStyle(.plain).help("Clear search")
      }
    }
    .padding(.horizontal, 10).frame(height: 34)
    .background(
      focused.wrappedValue ? Color.white : CatalogStyle.fill, in: RoundedRectangle(cornerRadius: 9, style: .continuous)
    )
    .overlay(
      RoundedRectangle(cornerRadius: 9, style: .continuous)
        .strokeBorder(
          focused.wrappedValue ? CatalogStyle.brand.opacity(0.6) : CatalogStyle.hairline,
          lineWidth: focused.wrappedValue ? 1.5 : 1)
    )
    .animation(.easeOut(duration: 0.12), value: focused.wrappedValue)
  }
}

private struct CatalogRow: View {
  let entry: CatalogEntry
  let selected: Bool
  let select: () -> Void
  @State private var isHovering = false

  var body: some View {
    HStack(spacing: 11) {
      IconTile(artwork: entry.icons, size: 40, radius: 9)
      VStack(alignment: .leading, spacing: 2) {
        Text(entry.displayTitle).font(TinyBrand.heading(15, relativeTo: .body)).lineLimit(1)
          .truncationMode(entry.isRecent ? .middle : .tail)
        Text(subtitle)
          .font(.callout).foregroundStyle(.secondary).lineLimit(1)
      }
      Spacer(minLength: 0)
    }
    .padding(.horizontal, 8).padding(.vertical, 7)
    .background(
      selected ? CatalogStyle.selection : isHovering ? CatalogStyle.hover : .clear,
      in: RoundedRectangle(cornerRadius: 10, style: .continuous)
    )
    .contentShape(Rectangle())
    .onTapGesture(perform: select)
    .onHover { isHovering = $0 }
    .help(entry.documentIdentity?.path ?? entry.title)
    .accessibilityElement(children: .combine)
    .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    .accessibilityAction(.default, select)
  }

  private var subtitle: String {
    if let identity = entry.documentIdentity {
      let folder = entry.inCloud ? "iCloud" : folderName(identity.folderPath)
      guard let date = entry.updatedAt else { return folder }
      return "\(folder) · \(relativeDate(date))"
    }
    var parts = entry.categories.prefix(2).map(\.label)
    if !entry.isBundled { parts.append("Installed") }
    return parts.isEmpty ? entry.title : parts.joined(separator: " · ")
  }
}

private func relativeDate(_ date: Date, now: Date = .now) -> String {
  now.timeIntervalSince(date) < 60 ? "Just now" : date.formatted(.relative(presentation: .named))
}
