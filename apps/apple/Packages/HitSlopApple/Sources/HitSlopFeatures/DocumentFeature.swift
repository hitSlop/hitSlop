import ComposableArchitecture
import Foundation
import HitSlopCore

@DependencyClient
public struct DocumentClient: Sendable {
    public var open: @Sendable (UUID, URL) async throws -> String
    public var focus: @Sendable (UUID) async -> Void
    /// Returns the new URL for duplication. Close returns only after native teardown.
    public var perform: @Sendable (UUID, SlopDocumentCommand) async throws -> URL?
    public var prepareToQuit: @Sendable (UUID) async throws -> Void
    public var finishQuit: @Sendable (UUID) async throws -> Void
    public var cancelQuit: @Sendable (UUID) async -> Void
    public var finishAssetRefreshes: @Sendable () async -> Void
    public var replyToQuit: @Sendable (Bool) async -> Void

}
extension DocumentClient: DependencyKey {
    public static let liveValue = Self(
        open: { _, _ in preconditionFailure("Install DocumentClient at the application root") },
        focus: { _ in preconditionFailure("Install DocumentClient at the application root") },
        perform: { _, _ in preconditionFailure("Install DocumentClient at the application root") },
        prepareToQuit: { _ in preconditionFailure("Install DocumentClient at the application root") },
        finishQuit: { _ in preconditionFailure("Install DocumentClient at the application root") },
        cancelQuit: { _ in preconditionFailure("Install DocumentClient at the application root") },
        finishAssetRefreshes: { preconditionFailure("Install DocumentClient at the application root") },
        replyToQuit: { _ in preconditionFailure("Install DocumentClient at the application root") }
    )
    public static let testValue = Self()
}
public extension DependencyValues {
    var documentClient: DocumentClient { get { self[DocumentClient.self] } set { self[DocumentClient.self] = newValue } }
}

@Reducer public struct DocumentFeature: Sendable {
    @ObservableState public struct State: Equatable, Identifiable {
        public let id: UUID
        public let url: URL
        public var title: String
        public var isOpening = true
        public var isPinned = false
        public var operation: SlopDocumentCommand?
        public var closeRequested = false
        public var isQuitting = false
        @Presents public var alert: AlertState<ErrorAlertAction>?
        public var runtimeError: String?
        public init(id: UUID, url: URL) { self.id = id; self.url = url; self.title = url.deletingPathExtension().lastPathComponent }
        public var acceptsCommands: Bool { !isOpening && !isQuitting && operation == nil && !closeRequested }
    }
    public enum Action {
        case command(SlopDocumentCommand)
        case operationFinished(SlopDocumentCommand, URL?)
        case operationFailed(SlopDocumentCommand, String)
        case runtimeFailed(String), runtimeReady
        case alert(PresentationAction<ErrorAlertAction>)
    }
    @Dependency(\.documentClient) var client
    public init() {}
    public var body: some ReducerOf<Self> {
        Reduce { state, action in
            switch action {
            case .command(let command):
                guard !state.isQuitting, !state.isOpening else { return .none }
                if state.operation != nil {
                    if command == .close { state.closeRequested = true }
                    return .none
                }
                state.operation = command
                if command == .retry { state.runtimeError = nil }
                let id = state.id
                return .run { send in
                    do { await send(.operationFinished(command, try await client.perform(id, command))) }
                    catch { await send(.operationFailed(command, error.localizedDescription)) }
                }
            case .operationFinished(let command, _):
                guard state.operation == command else { return .none }
                state.operation = nil
                if case .pin(let pinned) = command { state.isPinned = pinned }
                if state.closeRequested && command != .close {
                    state.closeRequested = false
                    // A queued close must finish even if quit began during the operation.
                    state.operation = .close
                    let id = state.id
                    return .run { send in
                        do { await send(.operationFinished(.close, try await client.perform(id, .close))) }
                        catch { await send(.operationFailed(.close, error.localizedDescription)) }
                    }
                }
                return .none
            case .operationFailed(let command, let message):
                guard state.operation == command else { return .none }
                state.operation = nil; state.closeRequested = false; state.alert = .operationFailure(message)
                return .none
            case .runtimeFailed(let message): state.runtimeError = message; return .none
            case .runtimeReady: state.runtimeError = nil; return .none
            case .alert: return .none
            }
        }
        .ifLet(\.$alert, action: \.alert)
    }
}
