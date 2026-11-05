import PopCore
import PopDocument
import SwiftUI

struct PhoneCatalogView: View {
  @ObservedObject var model: PhoneDocuments
  @ObservedObject var library: PopLibrary
  @Binding var tab: PhoneTab
  @Binding var showSettings: Bool
  @Binding var query: String
  @Binding var category: String

  @Environment(\.dynamicTypeSize) private var typeSize

  var body: some View {
    TabView(selection: $tab) {
      libraryList.tabItem { Label("Recents", systemImage: "clock") }.tag(PhoneTab.recents)
      templateList.tabItem { Label("Templates", systemImage: "square.grid.2x2.fill") }.tag(PhoneTab.templates)
    }
    .tint(PopBrand.accent)
    .appHaptic(.selection, trigger: tab) { _, _ in model.session == nil }
    .appHaptic(.selection, trigger: category)
    .navigationTitle(tab == .templates ? "Templates" : "Recents")
  }
  private var header: some View {
    HStack(spacing: 12) {
      PopBrandMark(size: 42)
      Spacer()
      Button {
        model.tapped()
        showSettings = true
      } label: {
        Image(systemName: "gearshape").font(.title3).foregroundStyle(PopBrand.accent).frame(width: 44, height: 44)
      }
      .accessibilityLabel("Settings")
    }.padding(.horizontal, 22).padding(.top, 4).padding(.bottom, 6)
  }
  private var allDocuments: [PhoneRecent] { model.recents(in: library.visibleEntries) }
  private var matchingDocuments: [PhoneRecent] {
    allDocuments.filter {
      query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || $0.title.localizedCaseInsensitiveContains(query)
    }
  }
  private var libraryList: some View {
    VStack(spacing: 0) {
      header
      search
      GeometryReader { geometry in
        ScrollView {
          VStack(alignment: .leading, spacing: 12) {
            let inbox = matchingDocuments.filter { $0.entry?.inbox == true }
            let recents = matchingDocuments.filter { $0.entry?.inbox != true }
            if !inbox.isEmpty {
              sectionHeading("Inbox", count: inbox.count)
              inboxStrip(inbox)
            }
            sectionHeading("Recents", refresh: true)
            if allDocuments.isEmpty {
              firstUse
            } else if matchingDocuments.isEmpty {
              ContentUnavailableView.search(text: query)
            } else if !recents.isEmpty {
              documentGrid(recents, width: geometry.size.width)
            } else {
              Text("Opened documents will appear here.").font(.subheadline).foregroundStyle(.secondary)
                .frame(maxWidth: .infinity).padding(.vertical, 28)
            }
          }.padding(.horizontal, 20).padding(.top, 14).padding(.bottom, 20)
        }.refreshable { await model.syncNow() }
      }
    }
    .safeAreaInset(edge: .bottom, spacing: 0) {
      LibrarySyncStatusView(library: library).padding(.vertical, 6).background(PopBrand.cream)
    }
    .background(PopBrand.cream)
  }
  private func columns(width: CGFloat) -> [GridItem] {
    let count = typeSize.isAccessibilitySize ? (width < 500 ? 1 : 2) : (width < 350 || typeSize >= .xxxLarge ? 2 : 3)
    return Array(repeating: GridItem(.flexible(), spacing: 14, alignment: .top), count: count)
  }
  private func artworkSize(width: CGFloat) -> CGFloat {
    min(90, (width - 40 - CGFloat(columns(width: width).count - 1) * 14) / CGFloat(columns(width: width).count))
  }
  private func inboxStrip(_ documents: [PhoneRecent]) -> some View {
    ScrollView(.horizontal, showsIndicators: false) {
      LazyHStack(alignment: .top, spacing: 12) {
        ForEach(documents) { recent in
          recentTile(recent, artworkSize: 70).frame(width: typeSize.isAccessibilitySize ? 150 : 96)
        }
      }.padding(.horizontal, 4).padding(.vertical, 6)
    }
    .padding(.bottom, 6)
  }
  private func documentGrid(_ documents: [PhoneRecent], width: CGFloat) -> some View {
    LazyVGrid(columns: columns(width: width), alignment: .center, spacing: 18) {
      ForEach(documents) { recent in recentTile(recent, artworkSize: artworkSize(width: width)) }
    }
  }
  private func sectionHeading(_ title: String, count: Int? = nil, refresh: Bool = false) -> some View {
    HStack(spacing: 8) {
      Text(title).font(PopBrand.heading(24, relativeTo: .title2))
      if let count {
        Text("\(count)").font(.caption.weight(.semibold)).monospacedDigit()
          .padding(.horizontal, 8).padding(.vertical, 3)
          .background(PopBrand.accent.opacity(0.1), in: Capsule()).foregroundStyle(PopBrand.accent)
      }
      Spacer()
      if refresh {
        Button {
          model.tapped()
          Task { await model.syncNow() }
        } label: {
          Group {
            if library.isSyncing {
              ProgressView()
            } else {
              Image(systemName: "arrow.triangle.2.circlepath").font(.title2)
            }
          }.frame(width: 44, height: 44).foregroundStyle(PopBrand.accent)
        }.accessibilityLabel("Sync Now").disabled(library.isSyncing)
      }
    }.accessibilityElement(children: .contain)
  }
  private var firstUse: some View {
    VStack(spacing: 14) {
      Image(systemName: "sparkles").font(.system(size: 34)).foregroundStyle(PopBrand.accent)
      Text("A little app for your next thing.").font(.title3.bold())
      Text("Start with a template. Keep it here or sync across devices.").foregroundStyle(.secondary)
      Button("Explore templates") { tab = .templates }.buttonStyle(.borderedProminent).tint(PopBrand.accent)
    }.multilineTextAlignment(.center).frame(maxWidth: .infinity).padding(.vertical, 50)
  }
  private func recentTile(_ recent: PhoneRecent, artworkSize: CGFloat) -> some View {
    let presentation: CatalogDocumentPresentation =
      recent.entry.map { CatalogDocumentPresentation(entry: $0, needsAttention: recent.isUnavailable) }
      ?? CatalogDocumentPresentation(
        needsAttention: recent.isUnavailable, downloaded: true, localOnly: true,
        sharing: recent.documentID.map { library.guestDocumentIDs.contains($0) } == true ? .sharedWithYou : .none)
    return Button {
      model.open(recent.url)
    } label: {
      PhonePopTile(
        url: recent.url, title: recent.title,
        revision: (recent.entry?.assetRevision ?? "") + (recent.entry?.downloaded == false ? "|cloud" : "|local"),
        presentation: presentation, artworkSize: artworkSize)
    }
    .buttonStyle(.plain)
    .contextMenu { removalActions(recent) }
    .accessibilityActions { removalActions(recent) }
    .accessibilityLabel(recent.title).accessibilityValue(presentation.accessibilityValue)
    .disabled(model.busy)
  }
  @ViewBuilder private func removalActions(_ recent: PhoneRecent) -> some View {
    if let entry = recent.entry, !entry.localOnly {
      if entry.downloaded {
        Button("Remove Download", systemImage: "icloud.and.arrow.down") { model.removeDownload(entry) }
          .tint(.gray).disabled(entry.dirty || entry.problem != nil)
      }
      if entry.downloaded {
        Button("Save Local Copy", systemImage: "doc.on.doc") { model.saveLocalCopy(entry) }
      }
      Button(
        entry.role == .editor ? "Leave Shared Document" : "Delete Everywhere",
        systemImage: "trash", role: .destructive
      ) { model.delete(recent) }
    } else {
      if let entry = recent.entry {
        Button("Enable Sync Across Devices", systemImage: "arrow.triangle.2.circlepath") { model.enableSync(entry) }
          .disabled(library.accountIdentity.isEmpty)
      }
      Button("Delete from This iPhone", systemImage: "trash", role: .destructive) { model.delete(recent) }
    }
  }
  private var search: some View {
    HStack(spacing: 10) {
      Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
      TextField("Search pop", text: $query).autocorrectionDisabled()
    }.padding(.horizontal, 13).padding(.vertical, 14).background(
      Color(.secondarySystemBackground), in: RoundedRectangle(cornerRadius: 17)
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
                .foregroundStyle(category == name ? PopBrand.accent : .primary)
                .padding(.horizontal, 16).padding(.vertical, 11)
                .background(
                  category == name ? PopBrand.accent.opacity(0.1) : PopBrand.creamDeep, in: Capsule())
            }.buttonStyle(.plain)
          }
        }.padding(.horizontal, 22)
      }.padding(.top, 18)
      HStack {
        Text(category).font(PopBrand.heading(24, relativeTo: .title2))
        Spacer()
      }
      .padding(.horizontal, 24).padding(.top, 25).padding(.bottom, 6)
      GeometryReader { geometry in
        ScrollView {
          let templates = model.templates.filter {
            $0.categories.contains(category) && (query.isEmpty || $0.title.localizedCaseInsensitiveContains(query))
          }
          if templates.isEmpty {
            if query.isEmpty {
              ContentUnavailableView("No templates here yet", systemImage: "square.grid.2x2")
            } else {
              ContentUnavailableView.search(text: query)
            }
          } else {
            LazyVGrid(columns: columns(width: geometry.size.width), spacing: 18) {
              ForEach(templates) { template in
                NavigationLink(value: PhoneRoute.template(template)) {
                  PhonePopTile(
                    url: template.url, title: template.title, artworkSize: artworkSize(width: geometry.size.width))
                }.buttonStyle(.plain).accessibilityLabel(template.title)
              }
            }.padding(.horizontal, 20).padding(.bottom, 28)
          }
        }
      }
    }
    .safeAreaInset(edge: .bottom, spacing: 0) {
      LibrarySyncStatusView(library: library).padding(.vertical, 6).background(PopBrand.cream)
    }
    .background(PopBrand.cream)
  }
}
