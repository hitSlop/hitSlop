import ComposableArchitecture
import Foundation
import HitSlopCore

extension SlopCategory {
    public var label: String { self == .developerTools ? "Developer Tools" : rawValue.capitalized }
    public var emoji: String {
        switch self {
        case .productivity: "⚡️"
        case .utilities: "🪄"
        case .finance: "🤑"
        case .media: "🎬"
        case .games: "🎮"
        case .developerTools: "👾"
        case .education: "🎓"
        case .business: "📊"
        case .personal: "💖"
        case .health: "🫀"
        case .creative: "🎨"
        case .music: "🎵"
        case .other: "🎲"
        }
    }
}

public enum CatalogFilter: Hashable, Sendable {
    case all, recents, category(SlopCategory)
    public var title: String {
        switch self {
        case .all: "Templates"
        case .recents: "Recents"
        case .category(let category): category.label
        }
    }
}

/// A slop's preview or icon artwork: the file holding it and which one. The file's
/// modification date and size participate in view reloads and decoded-image caching.
public struct CatalogArtwork: Hashable, Sendable {
    public enum Name: String, Sendable { case preview, icon }
    public let file: URL
    public let name: Name
    public let modifiedAt: Date?
    public let byteCount: Int?
    public init(file: URL, name: Name, modifiedAt: Date?, byteCount: Int?) {
        self.file = file; self.name = name; self.modifiedAt = modifiedAt; self.byteCount = byteCount
    }
}

/// Display and creation metadata only; no file reads occur when rendering a view.
public struct CatalogEntry: Equatable, Identifiable, Sendable {
    public enum Source: Equatable, Sendable { case local(URL), recent(URL) }
    public var id: String
    public var source: Source
    public var title: String
    public var slug = ""
    public var isBundled = false
    public var description = ""
    public var categories: [SlopCategory] = []
    public var authorName: String?
    public var authorURL: URL?
    public var icons: [CatalogArtwork] = []
    public var previews: [CatalogArtwork] = []
    public var fileBytes: Int64 = 0
    public var createdAt: Date?
    public var updatedAt: Date?
    public var initialSize: String?
    public init(id: String, source: Source, title: String) { self.id = id; self.source = source; self.title = title }
    public var isRecent: Bool { if case .recent = source { true } else { false } }
    public var documentIdentity: SlopDocumentIdentity? {
        guard case .recent(let url) = source else { return nil }
        return SlopDocumentIdentity(url: url)
    }
    public var displayTitle: String { documentIdentity?.filename ?? title }
    public var searchableText: String {
        ([title, description, authorName ?? "", documentIdentity?.path ?? "", documentIdentity?.folderPath ?? ""]
            + categories.flatMap { [$0.rawValue, $0.label] })
            .joined(separator: " ").localizedLowercase
    }
}

public struct CatalogSnapshot: Equatable, Sendable {
    public var entries: [CatalogEntry]
    public var issues: [String]
    public init(entries: [CatalogEntry] = [], issues: [String] = []) { self.entries = entries; self.issues = issues }
}

@DependencyClient
public struct CatalogClient: Sendable {
    public var local: @Sendable () async -> AsyncStream<CatalogSnapshot> = { .finished }
    /// Rescans installed templates; unless `force`, only when no folder watcher sees changes.
    public var refreshLocal: @Sendable (_ force: Bool) async -> Void
    public var recents: @Sendable () async -> [CatalogEntry] = { [] }
    /// The recent document at a URL, read again after its artwork changed.
    public var recent: @Sendable (URL) async -> CatalogEntry? = { _ in nil }
    /// Returns nil when the destination picker is cancelled.
    public var chooseDestination: @Sendable (CatalogEntry) async throws -> URL?
    /// Copies the template to the chosen destination and returns the new document.
    public var create: @Sendable (CatalogEntry, URL) async throws -> URL

}
extension CatalogClient: DependencyKey {
    public static let liveValue = Self(
        local: { preconditionFailure("Install CatalogClient at the application root") },
        refreshLocal: { _ in preconditionFailure("Install CatalogClient at the application root") },
        recents: { preconditionFailure("Install CatalogClient at the application root") },
        recent: { _ in preconditionFailure("Install CatalogClient at the application root") },
        chooseDestination: { _ in preconditionFailure("Install CatalogClient at the application root") },
        create: { _, _ in preconditionFailure("Install CatalogClient at the application root") }
    )
    public static let testValue = Self()
    /// Explicit fixture for native integration tests that do not display a catalog.
    public static let empty = Self(
        local: { .finished }, refreshLocal: { _ in },
        recents: { [] }, recent: { _ in nil }, chooseDestination: { _ in nil }, create: { _, url in url }
    )
}
public extension DependencyValues {
    var catalogClient: CatalogClient { get { self[CatalogClient.self] } set { self[CatalogClient.self] = newValue } }
}

@Reducer public struct CatalogFeature: Sendable {
    @ObservableState public struct State: Equatable {
        public var query = ""
        public var filter: CatalogFilter = .all
        public var selectedID: String?
        public var local: [CatalogEntry] = []
        public var recents: [CatalogEntry] = []
        public var localIssues: [String] = []
        public var isStarted = false
        public var creating: CatalogEntry?
        /// True only while the template is copied, never while the save panel is open.
        public var isCopying = false
        @Presents public var alert: AlertState<ErrorAlertAction>?
        public var isQuitting = false
        public var recentsGeneration = 0
        public init() {}
        public var categories: [SlopCategory] {
            let present = Set(local.flatMap(\.categories))
            return SlopCategory.schemaOrder.filter { present.contains($0) }
        }
        public var visibleEntries: [CatalogEntry] {
            let items: [CatalogEntry]
            switch filter {
            case .all: items = local
            case .category(let category): items = local.filter { $0.categories.contains(category) }
            case .recents: items = recents
            }
            let search = query.trimmingCharacters(in: .whitespacesAndNewlines).localizedLowercase
            return search.isEmpty ? items : items.filter { $0.searchableText.contains(search) }
        }
        public var selectedEntry: CatalogEntry? { visibleEntries.first { $0.id == selectedID } }
        mutating func synchronizeSelection() {
            if let selectedID, visibleEntries.contains(where: { $0.id == selectedID }) { return }
            selectedID = visibleEntries.first?.id
        }
    }
    public enum Action {
        /// `refreshSources` rescans everything; `activated` skips templates a watcher covers.
        case start, refreshRecents, refreshSources, activated
        /// A file's artwork changed; only its entry is read again.
        case packageChanged(URL), recentReceived(URL, CatalogEntry?)
        case queryChanged(String), filterChanged(CatalogFilter), selected(String?)
        case localReceived(CatalogSnapshot), recentsReceived(Int, [CatalogEntry])
        case primaryAction(CatalogEntry), destinationChosen(URL?), creationFinished(URL?), creationFailed(String)
        case alert(PresentationAction<ErrorAlertAction>)
        case openDocument(URL)
    }
    private enum CancelID { case local, recents, refreshLocal }
    @Dependency(\.catalogClient) var client
    public init() {}
    public var body: some ReducerOf<Self> {
        Reduce { state, action in
            switch action {
            case .start:
                guard !state.isStarted else { return .none }
                state.isStarted = true
                return .merge(.run { send in
                    for await snapshot in await client.local() { await send(.localReceived(snapshot)) }
                }.cancellable(id: CancelID.local), recents(&state))
            case .refreshSources, .activated:
                guard state.isStarted else { return .none }
                let force = if case .refreshSources = action { true } else { false }
                return .merge(recents(&state), .run { _ in
                    await client.refreshLocal(force)
                }.cancellable(id: CancelID.refreshLocal, cancelInFlight: true))
            case .packageChanged(let url):
                guard state.isStarted else { return .none }
                guard state.recents.contains(where: { $0.source == .recent(url) }) else { return recents(&state) }
                return .run { send in await send(.recentReceived(url, await client.recent(url))) }
            case .recentReceived(let url, let entry):
                guard let index = state.recents.firstIndex(where: { $0.source == .recent(url) }) else { return .none }
                if let entry { state.recents[index] = entry } else { state.recents.remove(at: index) }
                state.synchronizeSelection()
                return .none
            case .refreshRecents: return recents(&state)
            case .queryChanged(let query):
                state.query = query
                state.synchronizeSelection()
                return .none
            case .filterChanged(let filter):
                guard state.filter != filter else { return .none }
                state.filter = filter
                state.selectedID = nil
                state.synchronizeSelection()
                return .none
            case .selected(let id): state.selectedID = id; return .none
            case .localReceived(let snapshot):
                state.local = snapshot.entries; state.localIssues = snapshot.issues
                if case .category(let category) = state.filter, !state.categories.contains(category) { state.filter = .all }
                state.synchronizeSelection(); return .none
            case .recentsReceived(let generation, let entries):
                guard generation == state.recentsGeneration else { return .none }
                state.recents = entries
                state.synchronizeSelection(); return .none
            case .primaryAction(let entry):
                guard !state.isQuitting, state.creating == nil else { return .none }
                if case .recent(let url) = entry.source { return .send(.openDocument(url)) }
                state.creating = entry
                return .run { send in
                    do { await send(.destinationChosen(try await client.chooseDestination(entry))) }
                    catch { await send(.creationFailed(error.localizedDescription)) }
                }
            case .destinationChosen(let url):
                guard let url, let entry = state.creating else { return .send(.creationFinished(nil)) }
                state.isCopying = true
                return .run { send in
                    do { await send(.creationFinished(try await client.create(entry, url))) }
                    catch { await send(.creationFailed(error.localizedDescription)) }
                }
            case .creationFinished:
                state.creating = nil; state.isCopying = false
                return recents(&state)
            case .creationFailed(let message): state.creating = nil; state.isCopying = false; state.alert = .operationFailure(message, title: "Could not create slop"); return .none
            case .alert: return .none
            case .openDocument: return .none
            }
        }
        .ifLet(\.$alert, action: \.alert)
    }
    private func recents(_ state: inout State) -> Effect<Action> {
        state.recentsGeneration += 1
        let generation = state.recentsGeneration
        return .run { send in await send(.recentsReceived(generation, await client.recents())) }.cancellable(id: CancelID.recents, cancelInFlight: true)
    }
}
