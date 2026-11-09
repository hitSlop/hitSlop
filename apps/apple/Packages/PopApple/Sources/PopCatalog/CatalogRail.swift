import AppKit
import PopCore
import PopFeatures
import SwiftUI

struct CatalogRail: View {
  let title: String
  let entries: [CatalogEntry]
  let guestDocumentIDs: Set<String>
  @Binding var selectedID: String?
  @Binding var query: String
  var searchFocused: FocusState<Bool>.Binding
  let localIssues: [String]
  var recentsIssue: String? = nil
  let onRefresh: () -> Void
  var onDelete: (CatalogEntry) -> Void = { _ in }
  var onRemoveDownload: (CatalogEntry) -> Void = { _ in }
  var onLocalCopy: (CatalogEntry) -> Void = { _ in }
  var onEnableSync: (CatalogEntry) -> Void = { _ in }
  var onRemoveRecent: (CatalogEntry) -> Void = { _ in }

  var body: some View {
    VStack(spacing: 0) {
      CatalogSearchBar(query: $query, focused: searchFocused)
        .padding(.horizontal, 14).padding(.top, 14)

      HStack(alignment: .center) {
        Text(title).font(PopBrand.heading(20, relativeTo: .title3))
        Spacer()
        QuietIconButton(systemName: "arrow.clockwise", help: "Refresh", action: onRefresh)
          .accessibilityLabel("Refresh catalog")
      }
      .padding(.leading, 16).padding(.trailing, 12).padding(.top, 16).padding(.bottom, 10)
      .overlay(alignment: .bottom) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1).padding(.horizontal, 14) }

      if let recentsIssue {
        VStack(alignment: .leading, spacing: 6) {
          Label(recentsIssue, systemImage: "exclamationmark.triangle")
          Button("Retry", action: onRefresh)
        }
        .font(.caption).foregroundStyle(.secondary)
        .padding(12).frame(maxWidth: .infinity, alignment: .leading)
        .background(PopBrand.notice, in: RoundedRectangle(cornerRadius: 12))
        .padding(.horizontal, 14).padding(.top, 10)
      }

      if !localIssues.isEmpty {
        Label(
          "\(localIssues.count) local template \(localIssues.count == 1 ? "needs" : "need") attention",
          systemImage: "exclamationmark.triangle.fill"
        )
        .font(.caption).foregroundStyle(Color.orange)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 12).padding(.vertical, 8)
        .background(PopBrand.notice, in: RoundedRectangle(cornerRadius: 12, style: .continuous))
        .padding(.horizontal, 14).padding(.top, 10)
        .help(localIssues.joined(separator: "\n"))
      }

      if entries.isEmpty {
        ContentUnavailableView {
          Label(
            query.isEmpty ? "Nothing here yet" : "No results",
            systemImage: query.isEmpty ? "sparkles.rectangle.stack" : "magnifyingglass"
          )
          .symbolRenderingMode(.hierarchical).foregroundStyle(PopBrand.accent)
        } description: {
          Text(
            query.isEmpty
              ? "Choose a template to create a pop, or open a document." : "Try another name or clear the search.")
        }
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
      } else {
        ScrollViewReader { proxy in
          ScrollView {
            LazyVStack(spacing: 2) {
              ForEach(entries) { entry in
                CatalogRow(
                  entry: entry, showFolder: duplicateTitles.contains(entry.displayTitle),
                  isGuest: entry.savedDocumentID.map { guestDocumentIDs.contains($0) } == true,
                  selected: entry.id == selectedID
                ) { selectedID = entry.id }
                .id(entry.id)
                .contextMenu {
                  if let document = PopLibrary.shared.visibleEntries.first(where: { "library:" + $0.id == entry.id }) {
                    if document.localOnly {
                      Button("Enable Sync Across Devices") { onEnableSync(entry) }
                        .disabled(PopLibrary.shared.accountIdentity.isEmpty)
                    } else {
                      if document.downloaded {
                        Button("Save Local Copy") { onLocalCopy(entry) }
                        Button("Remove Download") { onRemoveDownload(entry) }
                          .disabled(document.dirty || document.problem != nil)
                      }
                    }
                    Button(
                      document.localOnly
                        ? "Delete from This Device"
                        : document.role == .editor ? "Leave Shared Document" : "Delete Everywhere", role: .destructive
                    ) { onDelete(entry) }
                  }
                  if entry.recentHistoryID != nil {
                    Button("Remove from Recents") { onRemoveRecent(entry) }
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

      Text(entries.count == 1 ? "1 pop" : "\(entries.count) pop")
        .font(.caption).foregroundStyle(.tertiary)
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, 16).padding(.vertical, 12)
    }
    .background(CatalogStyle.content)
  }

  private var duplicateTitles: Set<String> {
    Set(Dictionary(grouping: entries.filter(\.isRecent), by: \.displayTitle).filter { $0.value.count > 1 }.keys)
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
      TextField("Search pop", text: $query)
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
      focused.wrappedValue ? CatalogStyle.card : CatalogStyle.fill,
      in: RoundedRectangle(cornerRadius: 12, style: .continuous)
    )
    .overlay(
      RoundedRectangle(cornerRadius: 12, style: .continuous)
        .strokeBorder(
          focused.wrappedValue ? PopBrand.accent.opacity(0.6) : CatalogStyle.hairline,
          lineWidth: focused.wrappedValue ? 1.5 : 1)
    )
    .animation(.easeOut(duration: 0.12), value: focused.wrappedValue)
  }
}

private struct CatalogRow: View {
  let entry: CatalogEntry
  let showFolder: Bool
  let isGuest: Bool
  let selected: Bool
  let select: () -> Void
  @State private var isHovering = false

  var body: some View {
    HStack(spacing: 11) {
      IconTile(artwork: entry.icons, size: 50)
        .overlay(alignment: .bottomTrailing) {
          if let badge = presentation?.badge {
            CatalogDocumentBadgeView(badge: badge).offset(x: 3, y: 3)
          }
        }
      VStack(alignment: .leading, spacing: 2) {
        Text(entry.displayTitle).font(PopBrand.heading(15, relativeTo: .body)).lineLimit(1)
          .truncationMode(entry.isRecent ? .middle : .tail)
        if showFolder, let identity = entry.documentIdentity {
          Text(folderName(identity.folderPath)).font(.caption).foregroundStyle(.secondary).lineLimit(1)
        }
      }
      Spacer(minLength: 0)
    }
    .padding(.horizontal, 8).padding(.vertical, 7)
    .background(
      selected ? CatalogStyle.selection : isHovering ? CatalogStyle.hover : .clear,
      in: RoundedRectangle(cornerRadius: 12, style: .continuous)
    )
    .overlay(
      RoundedRectangle(cornerRadius: 12, style: .continuous)
        .strokeBorder(selected ? CatalogStyle.selectionStroke : .clear)
    )
    .contentShape(Rectangle())
    .onTapGesture(perform: select)
    .onHover { isHovering = $0 }
    .help(entry.documentIdentity?.path ?? entry.title)
    .accessibilityElement(children: .ignore)
    .accessibilityLabel(entry.displayTitle)
    .accessibilityValue(
      [
        presentation?.accessibilityValue,
        showFolder ? entry.documentIdentity?.folderPath : nil,
      ].compactMap { $0 }
        .joined(separator: ", ")
    )
    .accessibilityAddTraits(selected ? [.isButton, .isSelected] : .isButton)
    .accessibilityAction(.default, select)
  }

  private var presentation: CatalogDocumentPresentation? {
    guard entry.isRecent else { return nil }
    if let presentation = entry.documentPresentation {
      return entry.needsAttention ? presentation.requiringAttention() : presentation
    }
    return CatalogDocumentPresentation(
      needsAttention: entry.needsAttention, downloaded: true, localOnly: true,
      sharing: isGuest ? .sharedWithYou : .none)
  }
}
