import AppKit
import HitSlopCore
import HitSlopHost
import ComposableArchitecture
import HitSlopFeatures
import HitSlopDocument
import SwiftUI

public struct CatalogView: View {
    @Bindable var store: StoreOf<CatalogFeature>
    @FocusState private var searchFocused: Bool
    @Environment(\.scenePhase) private var scenePhase

    public init(store: StoreOf<CatalogFeature>) {
        self.store = store
    }

    public var body: some View {
        NavigationSplitView {
            CatalogSidebarFeatureView(store: store)
            .navigationSplitViewColumnWidth(min: 180, ideal: 200, max: 220)
        } content: {
            CatalogResultsFeatureView(store: store, searchFocused: $searchFocused)
            .navigationSplitViewColumnWidth(min: 250, ideal: 280, max: 320)
            .ignoresSafeArea(.container, edges: .top)
        } detail: {
            CatalogDetailFeatureView(store: store)
            .frame(minWidth: 460)
            .ignoresSafeArea(.container, edges: .top)
        }
        .navigationSplitViewStyle(.balanced)
        .tint(CatalogStyle.brand)
        .background {
            Button("Focus template search") { searchFocused = true }
                .keyboardShortcut("k", modifiers: .command)
                .frame(width: 1, height: 1).opacity(0)
        }
        .onAppear { store.send(.start) }
        .onChange(of: scenePhase) { _, phase in if phase == .active { store.send(.activated) } }
        .onReceive(NotificationCenter.default.publisher(for: .hitSlopPreviewDidChange)) { notification in
            if let url = notification.object as? URL { store.send(.packageChanged(url)) }
        }
        .alert($store.scope(state: \.$alert, action: \.alert))
        .preferredColorScheme(.light)
    }
}

private struct CatalogSidebarFeatureView: View {
    let store: StoreOf<CatalogFeature>
    var body: some View {
        CatalogSidebar(filter: store.filter, categories: store.categories, recentCount: store.recents.count) {
            store.send(.filterChanged($0))
        }
    }
}

private struct CatalogResultsFeatureView: View {
    @Bindable var store: StoreOf<CatalogFeature>
    var searchFocused: FocusState<Bool>.Binding
    var body: some View {
        CatalogRail(
            title: store.filter.title, entries: store.visibleEntries,
            selectedID: $store.selectedID.sending(\.selected),
            query: $store.query.sending(\.queryChanged), searchFocused: searchFocused,
            localIssues: store.filter != .recents ? store.localIssues : [],
            onRefresh: { store.send(.refreshSources) }
        )
    }
}

private struct CatalogDetailFeatureView: View {
    let store: StoreOf<CatalogFeature>
    var body: some View {
        CatalogDetail(
            section: store.filter.title, entry: store.selectedEntry,
            isCreating: store.creating != nil, isCopying: store.isCopying, isQuitting: store.isQuitting
        ) {
            store.send(.primaryAction($0))
        }
    }
}

// MARK: - Style

private enum CatalogStyle {
    static let brand = Color(red: 0.45, green: 0.16, blue: 0.96)
    static let brandHover = Color(red: 0.53, green: 0.24, blue: 0.98)
    static let selection = brand.opacity(0.10)
    static let hover = Color.primary.opacity(0.04)
    static let hairline = Color.primary.opacity(0.07)
    static let fill = Color.primary.opacity(0.045)
    static let sidebar = Color(nsColor: .windowBackgroundColor)
    static let content = Color(nsColor: .textBackgroundColor)
    /// Height of the titlebar strip the traffic lights sit in.
    static let titlebar: CGFloat = 52
}

/// Rounded white tile holding a template or document icon.
private struct IconTile: View {
    let artwork: [CatalogArtwork]
    let size: CGFloat
    let radius: CGFloat

    var body: some View {
        CatalogImageView(artwork: artwork, fallback: .applicationIcon)
            .padding(size * 0.1)
            .frame(width: size, height: size)
            .background(.white, in: RoundedRectangle(cornerRadius: radius, style: .continuous))
            .overlay(RoundedRectangle(cornerRadius: radius, style: .continuous).strokeBorder(CatalogStyle.hairline))
            .shadow(color: .black.opacity(0.06), radius: 1.5, y: 1)
    }
}

// MARK: - Sidebar

private struct CatalogSidebar: View {
    let filter: CatalogFilter
    let categories: [SlopCategory]
    let recentCount: Int
    let select: (CatalogFilter) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Image(nsImage: NSApplication.shared.applicationIconImage)
                    .resizable().interpolation(.high).scaledToFit().frame(width: 30, height: 30)
                Text("hitSlop").font(.title3.weight(.bold))
            }
            .padding(.horizontal, 16).padding(.top, 6).padding(.bottom, 18)

            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    SidebarSection("Library")
                    SidebarButton(title: "Recents", icon: .symbol("clock"), count: recentCount, selected: filter == .recents) { select(.recents) }
                    SidebarButton(title: "Templates", icon: .symbol("square.grid.2x2"), selected: filter == .all) { select(.all) }

                    if !categories.isEmpty {
                        SidebarSection("Categories").padding(.top, 14)
                        ForEach(categories, id: \.self) { category in
                            SidebarButton(title: category.label, icon: .emoji(category.emoji), selected: filter == .category(category)) { select(.category(category)) }
                        }
                    }
                }
                .padding(.horizontal, 10).padding(.bottom, 18)
            }

            HStack(spacing: 2) {
                BrandLink(name: "github", label: "GitHub", destination: CatalogLinks.github)
                BrandLink(name: "discord", label: "Discord", destination: CatalogLinks.discord)
            }
            .padding(.horizontal, 12).padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .overlay(alignment: .top) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1).padding(.horizontal, 14) }
        }
        .background(CatalogStyle.sidebar)
    }
}

private struct SidebarSection: View {
    let title: String
    init(_ title: String) { self.title = title }
    var body: some View {
        Text(title)
            .font(.caption.weight(.medium)).foregroundStyle(.secondary)
            .padding(.horizontal, 10).padding(.bottom, 4)
    }
}

private struct BrandLink: View {
    let name: String
    let label: String
    let destination: URL?
    @State private var isHovering = false

    var body: some View {
        Group {
            if let destination {
                Link(destination: destination) { icon }
            } else {
                Button {} label: { icon }
                    .disabled(true)
            }
        }
        .buttonStyle(.plain)
        .onHover { isHovering = $0 }
        .help(destination == nil ? "\(label) — coming soon" : label)
        .accessibilityLabel(destination == nil ? "\(label) — coming soon" : label)
    }

    private var icon: some View {
        Group {
            if let image = brandImage(named: name) {
                Image(nsImage: image).resizable().scaledToFit()
            } else {
                Image(systemName: "link")
            }
        }
        .frame(width: 14, height: 14)
        .frame(width: 28, height: 28)
        .foregroundStyle(isHovering ? .primary : .secondary)
        .background(isHovering ? CatalogStyle.hover : .clear, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
        .contentShape(Rectangle())
    }
}

private enum SidebarIcon { case symbol(String), emoji(String) }

private struct SidebarButton: View {
    let title: String
    let icon: SidebarIcon
    var count: Int?
    let selected: Bool
    let action: () -> Void
    @State private var isHovering = false

    init(title: String, icon: SidebarIcon, count: Int? = nil, selected: Bool, action: @escaping () -> Void) {
        self.title = title; self.icon = icon; self.count = count; self.selected = selected; self.action = action
    }

    var body: some View {
        Button(action: action) {
            HStack(spacing: 9) {
                Group {
                    switch icon {
                    case .symbol(let name):
                        Image(systemName: name).font(.system(size: 14, weight: .medium))
                            .foregroundStyle(selected ? CatalogStyle.brand : .secondary)
                    case .emoji(let emoji):
                        Text(emoji).font(.system(size: 14))
                    }
                }
                .frame(width: 20).accessibilityHidden(true)
                Text(title).lineLimit(1)
                Spacer(minLength: 4)
                if let count, count > 0 {
                    Text("\(count)").font(.caption.monospacedDigit()).foregroundStyle(.tertiary)
                }
            }
            .font(.body.weight(selected ? .medium : .regular))
            .padding(.horizontal, 10).frame(height: 32)
            .background(
                selected ? CatalogStyle.selection : isHovering ? CatalogStyle.hover : .clear,
                in: RoundedRectangle(cornerRadius: 8, style: .continuous)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { isHovering = $0 }
    }
}

// MARK: - Rail

private struct CatalogRail: View {
    let title: String
    let entries: [CatalogEntry]
    @Binding var selectedID: String?
    @Binding var query: String
    var searchFocused: FocusState<Bool>.Binding
    let localIssues: [String]
    let onRefresh: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            CatalogSearchBar(query: $query, focused: searchFocused)
                .padding(.horizontal, 14).padding(.top, 14)

            HStack(alignment: .center) {
                Text(title).font(.title3.weight(.semibold))
                Spacer()
                QuietIconButton(systemName: "arrow.clockwise", help: "Refresh", action: onRefresh)
                    .accessibilityLabel("Refresh catalog")
            }
            .padding(.leading, 16).padding(.trailing, 12).padding(.top, 16).padding(.bottom, 10)
            .overlay(alignment: .bottom) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1).padding(.horizontal, 14) }

            if !localIssues.isEmpty {
                Label("\(localIssues.count) local template \(localIssues.count == 1 ? "needs" : "need") attention", systemImage: "exclamationmark.triangle.fill")
                    .font(.caption).foregroundStyle(Color.orange)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(.horizontal, 10).padding(.vertical, 7)
                    .background(Color.orange.opacity(0.09), in: RoundedRectangle(cornerRadius: 8, style: .continuous))
                    .padding(.horizontal, 14).padding(.top, 10)
                    .help(localIssues.joined(separator: "\n"))
            }

            if entries.isEmpty {
                ContentUnavailableView("Nothing here", systemImage: "sparkles.rectangle.stack", description: Text("Try another search or category."))
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollViewReader { proxy in
                    ScrollView {
                        LazyVStack(spacing: 2) {
                            ForEach(entries) { entry in
                                CatalogRow(entry: entry, selected: entry.id == selectedID) { selectedID = entry.id }
                                    .id(entry.id)
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

            Text(entries.count == 1 ? "1 slop" : "\(entries.count) slops")
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
            TextField("Search slops", text: $query)
                .textFieldStyle(.plain).focused(focused)
            if query.isEmpty {
                KeyCap("⌘K")
            } else {
                Button { query = "" } label: { Image(systemName: "xmark.circle.fill").foregroundStyle(.tertiary) }
                    .buttonStyle(.plain).help("Clear search")
            }
        }
        .padding(.horizontal, 10).frame(height: 34)
        .background(focused.wrappedValue ? Color.white : CatalogStyle.fill, in: RoundedRectangle(cornerRadius: 9, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 9, style: .continuous)
                .strokeBorder(focused.wrappedValue ? CatalogStyle.brand.opacity(0.6) : CatalogStyle.hairline, lineWidth: focused.wrappedValue ? 1.5 : 1)
        )
        .animation(.easeOut(duration: 0.12), value: focused.wrappedValue)
    }
}

private struct KeyCap: View {
    let label: String
    var foreground: Color = .secondary
    var border: Color = CatalogStyle.hairline
    init(_ label: String, foreground: Color = .secondary, border: Color = CatalogStyle.hairline) {
        self.label = label; self.foreground = foreground; self.border = border
    }
    var body: some View {
        Text(label)
            .font(.caption2.weight(.medium))
            .foregroundStyle(foreground)
            .padding(.horizontal, 5).padding(.vertical, 1.5)
            .overlay(RoundedRectangle(cornerRadius: 4, style: .continuous).strokeBorder(border))
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
                Text(entry.displayTitle).font(.body.weight(.medium)).lineLimit(1)
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
            let folder = folderName(identity.folderPath)
            guard let date = entry.updatedAt else { return folder }
            return "\(folder) · \(relativeDate(date))"
        }
        var parts = entry.categories.prefix(2).map(\.label)
        if !entry.isBundled { parts.append("Installed") }
        return parts.isEmpty ? entry.title : parts.joined(separator: " · ")
    }
}

// MARK: - Detail

private struct CatalogDetail: View {
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
                ContentUnavailableView("Select a slop", systemImage: "square.stack.3d.up", description: Text("Choose an item from the list to see its preview and details."))
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
                    .font(.title.weight(.semibold))
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
        var result = [CatalogFact(title: "Source", value: entry.isRecent ? "Local document" : entry.isBundled ? "Included with hitSlop" : "Installed locally")]
        if entry.isRecent, let date = entry.createdAt { result.append(CatalogFact(title: "Created", value: date.formatted(date: .abbreviated, time: .omitted))) }
        if let size = entry.initialSize { result.append(CatalogFact(title: "Initial size", value: size)) }
        if entry.packageBytes > 0 { result.append(CatalogFact(title: "Package size", value: ByteCountFormatter.string(fromByteCount: entry.packageBytes, countStyle: .file))) }
        if let date = entry.updatedAt { result.append(CatalogFact(title: "Updated", value: date.formatted(date: .abbreviated, time: .shortened))) }
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
            .background(isHovering ? CatalogStyle.brandHover : CatalogStyle.brand, in: RoundedRectangle(cornerRadius: 10, style: .continuous))
            .shadow(color: CatalogStyle.brand.opacity(isHovering ? 0.32 : 0.22), radius: isHovering ? 10 : 6, y: 3)
            .fixedSize()
        }
        .buttonStyle(.plain)
        .keyboardShortcut(.return, modifiers: .command)
        .disabled(isBusy || isWorking)
        .task(id: isWorking) {
            guard isWorking else { showsWork = false; return }
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

private enum CatalogImageFallback { case applicationIcon, preview }

private struct CatalogImageView: View {
    let artwork: [CatalogArtwork]
    let fallback: CatalogImageFallback
    /// The image loaded for `artwork`; a different artwork list shows its cached image or
    /// the placeholder, never the previous entry's image.
    @State private var loaded: (artwork: [CatalogArtwork], image: NSImage)?
    private let maxPreviewWidth: CGFloat = 560
    private let maxPreviewHeight: CGFloat = 420
    /// Decoded pixels: enough for a preview pane or an icon tile on a 2x display.
    private var maxPixelSize: Int { fallback == .preview ? 1200 : 256 }

    var body: some View {
        let shown = loaded?.artwork == artwork ? loaded?.image : cachedCatalogImage(artwork, maxPixelSize: maxPixelSize)
        Group {
            if let image = shown {
                let preview = Image(nsImage: image).resizable().interpolation(.high).scaledToFit()
                if fallback == .preview {
                    preview
                        .frame(maxWidth: min(maxPreviewWidth, image.size.width), maxHeight: min(maxPreviewHeight, image.size.height))
                        .shadow(color: .black.opacity(0.12), radius: 12, y: 6)
                } else {
                    preview
                }
            } else if fallback == .applicationIcon {
                Image(nsImage: NSApplication.shared.applicationIconImage).resizable().interpolation(.high).scaledToFit()
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "photo").font(.title2)
                    Text("Preview unavailable").font(.caption.weight(.medium))
                }
                .foregroundStyle(.tertiary)
                .frame(maxWidth: .infinity, minHeight: 180)
            }
        }
        .task(id: artwork) {
            for file in artwork {
                let image = await loadCatalogImage(file, maxPixelSize: maxPixelSize)
                guard !Task.isCancelled else { return }
                if let image {
                    loaded = (artwork, image)
                    return
                }
            }
            loaded = nil
        }
    }
}

/// Catalog artwork keyed by URL, modification date, byte count and decoded size, so
/// refreshed previews reload while reselecting reuses decoded images.
@MainActor private let catalogImages: NSCache<NSString, NSImage> = {
    let cache = NSCache<NSString, NSImage>()
    cache.totalCostLimit = 128 * 1024 * 1024
    return cache
}()

private func catalogImageKey(_ artwork: CatalogArtwork, maxPixelSize: Int) -> NSString? {
    guard artwork.url.isFileURL, let modified = artwork.modifiedAt, let bytes = artwork.byteCount else { return nil }
    return "\(artwork.url.absoluteString)|\(modified.timeIntervalSinceReferenceDate)|\(bytes)|\(maxPixelSize)" as NSString
}

@MainActor private func cachedCatalogImage(_ artwork: [CatalogArtwork], maxPixelSize: Int) -> NSImage? {
    for file in artwork {
        if let key = catalogImageKey(file, maxPixelSize: maxPixelSize), let image = catalogImages.object(forKey: key) { return image }
    }
    return nil
}

/// Decodes a downsampled image off the main thread.
@MainActor private func loadCatalogImage(_ artwork: CatalogArtwork, maxPixelSize: Int) async -> NSImage? {
    guard let key = catalogImageKey(artwork, maxPixelSize: maxPixelSize) else { return nil }
    if let cached = catalogImages.object(forKey: key) { return cached }
    let url = artwork.url
    let decoded = await Task.detached(priority: .userInitiated) { () -> CGImage? in
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        return CGImageSourceCreateThumbnailAtIndex(source, 0, [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxPixelSize,
        ] as CFDictionary)
    }.value
    guard !Task.isCancelled, let decoded else { return nil }
    let image = NSImage(cgImage: decoded, size: NSSize(width: decoded.width, height: decoded.height))
    catalogImages.setObject(image, forKey: key, cost: decoded.bytesPerRow * decoded.height)
    return image
}

private func brandImage(named name: String) -> NSImage? {
    guard let url = Bundle.module.url(forResource: name, withExtension: "svg"), let image = NSImage(contentsOf: url) else { return nil }
    image.isTemplate = true
    return image
}

/// Last component of an abbreviated folder path, e.g. "~/Desktop" → "Desktop".
private func folderName(_ folderPath: String) -> String {
    let name = (folderPath as NSString).lastPathComponent
    return name.isEmpty ? folderPath : name
}

private func relativeDate(_ date: Date, now: Date = .now) -> String {
    now.timeIntervalSince(date) < 60 ? "Just now" : date.formatted(.relative(presentation: .named))
}
