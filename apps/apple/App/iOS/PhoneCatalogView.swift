import HitSlopCore
import SwiftUI

struct PhoneCatalogView: View {
  @ObservedObject var model: PhoneDocuments
  @ObservedObject var library: SlopLibrary
  @Binding var tab: PhoneTab
  @Binding var showSettings: Bool
  @Binding var query: String
  @Binding var category: String

  var body: some View {
    TabView(selection: $tab) {
      libraryList.tabItem { Label("Recents", systemImage: "clock") }.tag(PhoneTab.recents)
      templateList.tabItem { Label("Templates", systemImage: "square.grid.2x2.fill") }.tag(PhoneTab.templates)
    }
    .navigationTitle(tab == .templates ? "Templates" : "Recents")
  }
  private var header: some View {
    HStack(spacing: 12) {
      SlopBrandMark(size: 38)
      Text("hitSlop").font(SlopBrand.heading(28, relativeTo: .title)).tracking(-0.8)
      Spacer()
      Button {
        showSettings = true
      } label: {
        Image(systemName: "gearshape").font(.title3).foregroundStyle(.primary).frame(width: 44, height: 44)
      }
      .accessibilityLabel("Settings")
    }.padding(.horizontal, 22).padding(.top, 10).padding(.bottom, 12)
  }
  private var libraryList: some View {
    VStack(spacing: 0) {
      header
      search
      HStack {
        Text("Recents").font(SlopBrand.heading(24, relativeTo: .title2))
        Spacer()
        Button {
          Task { await library.syncNow() }
        } label: {
          if library.isSyncing { ProgressView() } else { Image(systemName: "arrow.triangle.2.circlepath") }
        }.accessibilityLabel("Sync Now").disabled(library.isSyncing)
      }.padding(.horizontal, 24).padding(.top, 24).padding(.bottom, 12)
      ScrollView {
        LazyVStack(spacing: 0) {
          ForEach(library.visibleEntries.filter { query.isEmpty || $0.title.localizedCaseInsensitiveContains(query) }) {
            entry in
            Button {
              model.open(URL(fileURLWithPath: entry.path))
            } label: {
              PhoneSlopRow(
                url: URL(fileURLWithPath: entry.path), title: entry.title, subtitle: library.state(for: entry),
                revision: entry.assetRevision)
            }
            .contextMenu { Button("Delete Slop", role: .destructive) { model.delete(entry) } }
            .buttonStyle(.plain).accessibilityLabel(entry.title).disabled(
              entry.problem != nil && !FileManager.default.fileExists(atPath: entry.path))
          }
          ForEach(
            model.documents.filter { query.isEmpty || $0.lastPathComponent.localizedCaseInsensitiveContains(query) },
            id: \.self
          ) { url in
            Button {
              model.open(url)
            } label: {
              PhoneSlopRow(
                url: url, title: model.localTitles[url] ?? url.deletingPathExtension().lastPathComponent,
                subtitle: "On this iPhone")
            }.buttonStyle(.plain).accessibilityLabel(
              model.localTitles[url] ?? url.deletingPathExtension().lastPathComponent)
          }
        }.padding(.horizontal, 14)
        if library.visibleEntries.isEmpty && model.documents.isEmpty {
          VStack(spacing: 14) {
            Image(systemName: "sparkles").font(.system(size: 34)).foregroundStyle(PhoneStyle.accent)
            Text("A little app for your next thing.").font(.title3.bold())
            Text("Start a checklist here. Pick it up on your Mac.").foregroundStyle(.secondary).multilineTextAlignment(
              .center)
            Button("Explore templates") { tab = .templates }.buttonStyle(.borderedProminent).tint(PhoneStyle.accent)
          }.padding(.horizontal, 30).padding(.top, 66)
        }
      }.refreshable { await library.syncNow() }
      if !library.isReady { Text(library.status).font(.footnote).foregroundStyle(.secondary).padding(16) }
    }.background(Color(.systemBackground))
  }
  private var search: some View {
    HStack(spacing: 10) {
      Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
      TextField("Search slops", text: $query).autocorrectionDisabled()
    }.padding(.horizontal, 15).padding(.vertical, 12).background(
      Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 15)
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
        Text(category).font(SlopBrand.heading(24, relativeTo: .title2))
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
              PhoneSlopRow(
                url: template.url, title: template.title, subtitle: template.categories.joined(separator: " · "))
            }.buttonStyle(.plain).accessibilityLabel(template.title)
          }
        }.padding(.horizontal, 14)
        Text("More little apps are on their way.").font(.footnote).foregroundStyle(.secondary).padding(28)
      }
    }.background(Color(.systemBackground))
  }
}
