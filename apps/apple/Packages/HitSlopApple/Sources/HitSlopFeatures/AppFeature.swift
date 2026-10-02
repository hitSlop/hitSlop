import ComposableArchitecture
import Foundation
import HitSlopCore

@Reducer public struct AppFeature: Sendable {
    public enum QuitPhase: Equatable, Sendable { case running, waiting, preparing, finished }
    @ObservableState public struct State: Equatable {
        public var catalog = CatalogFeature.State()
        public var documents: IdentifiedArrayOf<DocumentFeature.State> = []
        public var quitPhase: QuitPhase = .running
        @Presents public var alert: AlertState<ErrorAlertAction>?
        public init() {}
    }
    public enum Action {
        case catalog(CatalogFeature.Action)
        case documents(IdentifiedActionOf<DocumentFeature>)
        /// Caller resolves symlinks before dispatching; the reducer performs no filesystem access.
        case openDocument(URL)
        case openFinished(UUID, String), openFailed(UUID, String, requiresUpdate: Bool = false), openCancelled(UUID)
        case quitDocumentClosed(UUID)
        case quitRequested, quitFinished, quitFailed(SlopDocumentFailure), externalFailure(String)
        case alert(PresentationAction<ErrorAlertAction>)
    }
    @Dependency(\.documentClient) var client
    @Dependency(\.uuid) var uuid
    public init() {}
    public var body: some ReducerOf<Self> {
        Scope(state: \.catalog, action: \.catalog) { CatalogFeature() }
        Reduce { state, action in
            var effect: Effect<Action> = .none
            switch action {
            case .openDocument(let url), .catalog(.openDocument(let url)):
                guard state.quitPhase == .running else { return .none }
                effect = open(url, state: &state)
            case .catalog(.creationFinished(let url)):
                // Insert the document before advancing quit so a completed creation cannot be missed.
                if let url { effect = open(url, state: &state) }
            case .openFinished(let id, let title):
                guard state.documents[id: id] != nil else { return .none }
                state.documents[id: id]?.isOpening = false
                state.documents[id: id]?.title = title
                effect = .send(.catalog(.refreshRecents))
            case .openCancelled(let id):
                state.documents.remove(id: id)
            case .openFailed(let id, let message, let requiresUpdate):
                guard state.documents.remove(id: id) != nil else { return .none }
                state.alert = requiresUpdate ? .requiresUpdate(message) : .operationFailure(message)
            case .quitDocumentClosed(let id):
                state.documents.remove(id: id)
            case .documents(.element(let id, .operationFinished(.close, _))):
                state.documents.remove(id: id)
            case .documents(.element(_, .operationFinished(.duplicate, let url))):
                if let url { effect = open(url, state: &state) }
            case .documents(.element(let id, .operationFailed(.close, let failure))):
                if state.quitPhase == .waiting {
                    state.documents[id: id]?.alert = nil
                    return cancelQuit(&state, failure: failure)
                }
            case .quitRequested:
                guard state.quitPhase == .running else { return .none }
                state.quitPhase = .waiting
                state.catalog.isQuitting = true
                for id in state.documents.ids { state.documents[id: id]?.isQuitting = true }
            case .quitFinished:
                state.quitPhase = .finished
                return .run { _ in await client.replyToQuit(true) }
            case .quitFailed(let failure): return cancelQuit(&state, failure: failure)
            case .externalFailure(let message): state.alert = .operationFailure(message)
            case .alert: break
            case .catalog, .documents: break
            }
            return .merge(effect, advanceQuit(&state))
        }
        .ifLet(\.$alert, action: \.alert)
        .forEach(\.documents, action: \.documents) { DocumentFeature() }
    }
    private func open(_ url: URL, state: inout State) -> Effect<Action> {
        if let existing = state.documents.first(where: { $0.url == url }) {
            let id = existing.id
            return .run { _ in await client.focus(id) }
        }
        let id = uuid()
        var document = DocumentFeature.State(id: id, url: url)
        document.isQuitting = state.quitPhase != .running
        state.documents.append(document)
        return .run { send in
            do { await send(.openFinished(id, try await client.open(id, url))) }
            catch is CancellationError { await send(.openCancelled(id)) }
            catch {
                let requiresUpdate = SlopFailureContext.classify(error).reason == .requiresUpdate
                await send(.openFailed(id, error.localizedDescription, requiresUpdate: requiresUpdate))
            }
        }
    }
    private func advanceQuit(_ state: inout State) -> Effect<Action> {
        guard state.quitPhase == .waiting, state.catalog.creating == nil,
              state.documents.allSatisfy({ !$0.isOpening && $0.operation == nil && !$0.closeRequested }) else { return .none }
        state.quitPhase = .preparing
        let ids = Array(state.documents.ids)
        return .run { send in
            var remaining = ids[...]
            do {
                for id in ids { try await client.prepareToQuit(id) }
                await client.finishAssetRefreshes()
                for id in ids {
                    try await client.finishQuit(id)
                    remaining = remaining.dropFirst()
                    await send(.quitDocumentClosed(id))
                }
                await send(.quitFinished)
            } catch {
                // A completed close has destroyed its renderer and cannot be rolled back.
                for id in remaining { await client.cancelQuit(id) }
                await send(.quitFailed(SlopDocumentFailure(error)))
            }
        }
    }
    /// A save failure is already on its document's save-failure sheet; other failures alert.
    private func cancelQuit(_ state: inout State, failure: SlopDocumentFailure) -> Effect<Action> {
        state.quitPhase = .running; state.catalog.isQuitting = false
        if case .other(let message) = failure { state.alert = .operationFailure(message) }
        for id in state.documents.ids { state.documents[id: id]?.isQuitting = false }
        return .run { _ in await client.replyToQuit(false) }
    }
}
