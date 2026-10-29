import SwiftUI
import TinysCore
import TinysDocument

struct PhoneCatalogView: View {
  @ObservedObject var model: PhoneDocuments
  @ObservedObject var library: TinyLibrary
  @Binding var tab: PhoneTab
  @Binding var showSettings: Bool
  @Binding var query: String
  @Binding var category: String

  var body: some View {
    TabView(selection: $tab) {
      libraryList.tabItem { Label("Recents", systemImage: "clock") }.tag(PhoneTab.recents)
      templateList.tabItem { Label("Templates", systemImage: "square.grid.2x2.fill") }.tag(PhoneTab.templates)
    }
    .appHaptic(.selection, trigger: tab) { _, _ in model.session == nil }
    .appHaptic(.selection, trigger: category)
    .navigationTitle(tab == .templates ? "Templates" : "Recents")
  }
  private var header: some View {
    HStack(spacing: 12) {
      TinyBrandMark(size: 30)
      Text("tinys").font(TinyBrand.heading(24, relativeTo: .title)).tracking(-0.6)
      Spacer()
      Button {
        model.tapped()
        showSettings = true
      } label: {
        Image(systemName: "gearshape").font(.title3).foregroundStyle(.primary).frame(width: 44, height: 44)
      }
      .accessibilityLabel("Settings")
    }.padding(.horizontal, 22).padding(.top, 4).padding(.bottom, 6)
  }
  private var libraryList: some View {
    VStack(spacing: 0) {
      header
      search
      HStack {
        Text("Recents").font(TinyBrand.heading(20, relativeTo: .title2))
        Spacer()
        Button {
          model.tapped()
          Task { await model.syncNow() }
        } label: {
          if library.isSyncing { ProgressView() } else { Image(systemName: "arrow.triangle.2.circlepath") }
        }.accessibilityLabel("Sync Now").disabled(library.isSyncing)
      }.padding(.horizontal, 24).padding(.top, 14).padding(.bottom, 4)
      List {
        ForEach(
          model.recents(in: library.visibleEntries).filter {
            query.isEmpty || $0.title.localizedCaseInsensitiveContains(query)
          }
        ) {
          recent in
          recentRow(recent)
            .listRowInsets(EdgeInsets(top: 0, leading: 14, bottom: 0, trailing: 14))
            .listRowSeparator(.hidden)
            .listRowBackground(Color.clear)
        }
        if library.visibleEntries.isEmpty && model.documents.isEmpty {
          VStack(spacing: 14) {
            Image(systemName: "sparkles").font(.system(size: 34)).foregroundStyle(PhoneStyle.accent)
            Text("A little app for your next thing.").font(.title3.bold())
            Text("Start a checklist here. Pick it up on your Mac.").foregroundStyle(.secondary).multilineTextAlignment(
              .center)
            Button("Explore templates") { tab = .templates }.buttonStyle(.borderedProminent).tint(PhoneStyle.accent)
          }.padding(.horizontal, 14).padding(.top, 66)
            .listRowSeparator(.hidden).listRowBackground(Color.clear)
        }
      }
      .listStyle(.plain).scrollContentBackground(.hidden)
      .contentMargins(.top, 0, for: .scrollContent)
      .refreshable { await model.syncNow() }
      LibrarySyncStatusView(library: library)
    }.background(Color(.systemBackground))
  }
  private func recentRow(_ recent: PhoneRecent) -> some View {
    let subtitle = recent.entry.map { library.state(for: $0) } ?? "On this iPhone"
    let cloud = recent.entry.map { !$0.localOnly } ?? false
    let canRemoveDownload = recent.entry.map { $0.downloaded && !$0.dirty && $0.problem == nil } ?? false
    let unavailable =
      recent.entry?.problem != nil && recent.entry?.downloaded != false
      && !FileManager.default.fileExists(atPath: recent.url.path)
    return Button {
      model.open(recent.url)
    } label: {
      PhoneTinyRow(
        url: recent.url, title: recent.title, subtitle: subtitle,
        revision: recent.entry?.assetRevision ?? "", synced: recent.entry.map { library.isSynced($0) } ?? false,
        downloaded: recent.entry?.downloaded ?? true)
    }
    .buttonStyle(.plain)
    .disabled(unavailable)
    .contextMenu { removalActions(recent) }
    .swipeActions(edge: .trailing, allowsFullSwipe: !cloud || canRemoveDownload) {
      removalActions(recent)
    }
    .accessibilityLabel(recent.title).accessibilityValue(subtitle)
    .disabled(model.busy)
  }
  @ViewBuilder private func removalActions(_ recent: PhoneRecent) -> some View {
    if let entry = recent.entry, !entry.localOnly {
      if entry.downloaded {
        Button("Remove Download", systemImage: "icloud.and.arrow.down") { model.removeDownload(entry) }
          .tint(.gray).disabled(entry.dirty || entry.problem != nil)
      }
      Button("Delete Everywhere", systemImage: "trash", role: .destructive) { model.delete(recent) }
    } else {
      Button("Delete from This iPhone", systemImage: "trash", role: .destructive) { model.delete(recent) }
    }
  }
  private var search: some View {
    HStack(spacing: 10) {
      Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
      TextField("Search tinys", text: $query).autocorrectionDisabled()
    }.padding(.horizontal, 13).padding(.vertical, 9).background(
      Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 12)
    )
    .padding(.horizontal, 22)
  }
  private var templateList: some View {
    VStack(spacing: 0) {
      header
      search
      ScrollView(.horizontal, showsIndicators: false) {
        HStack(spacing: 10) {
          ForEach(
            Array(Set(model.templates.flatMap(\.categories))).sorted { a, b in
              if a == b { return false }
              if a == "Popular" { return true }
              if b == "Popular" { return false }
              return a < b
            }, id: \.self
          ) { name in
            Button {
              category = name
            } label: {
              Label(name, systemImage: name == "Popular" ? "flame.fill" : "bolt.fill")
                .font(.subheadline.weight(category == name ? .semibold : .medium))
                .foregroundStyle(category == name ? PhoneStyle.accent : .primary)
                .padding(.horizontal, 16).padding(.vertical, 11)
                .background(
                  category == name ? PhoneStyle.accent.opacity(0.1) : Color(.secondarySystemBackground), in: Capsule())
            }.buttonStyle(.plain)
          }
        }.padding(.horizontal, 22)
      }.padding(.top, 18)
      HStack {
        Text(category).font(TinyBrand.heading(24, relativeTo: .title2))
        Spacer()
      }
      .padding(.horizontal, 24).padding(.top, 25).padding(.bottom, 12)
      ScrollView {
        LazyVStack(spacing: 0) {
          ForEach(
            model.templates.filter {
              $0.categories.contains(category) && (query.isEmpty || $0.title.localizedCaseInsensitiveContains(query))
            }
          ) {
            template in
            NavigationLink(value: PhoneRoute.template(template)) {
              PhoneTinyRow(
                url: template.url, title: template.title, subtitle: template.categories.joined(separator: " · "))
            }.buttonStyle(.plain).accessibilityLabel(template.title)
          }
        }.padding(.horizontal, 14)
        Text("More little apps are on their way.").font(.footnote).foregroundStyle(.secondary).padding(28)
      }
    }.background(Color(.systemBackground))
  }
}
