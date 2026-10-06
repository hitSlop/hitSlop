import ComposableArchitecture
import Foundation
import HitSlopCore

@DependencyClient
public struct DocumentClient: Sendable {
    public var open: @Sendable (UUID, URL) async throws -> String
    public var focus: @Sendable (UUID) async -> Void
    /// Returns the new URL for duplication. Close returns only after native teardown.
    /// Failures arrive classified as `SlopDocumentFailure`.
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

/// One document's lifecycle: one command at a time, a close or save recovery requested
/// meanwhile runs afterwards, and failures that need an alert. How the window looks
/// (pin level, page failure, the save-failure sheet) belongs to the window.
@Reducer public struct DocumentFeature: Sendable {
    @ObservableState public struct State: Equatable, Identifiable {
        public let id: UUID
        public let url: URL
        public var title: String
        public var isOpening = true
        public var operation: SlopDocumentCommand?
        public var closeRequested = false
        /// A save recovery chosen while another command ran; it runs next.
        public var pendingRecovery: SlopDocumentCommand?
        public var isQuitting = false
        @Presents public var alert: AlertState<ErrorAlertAction>?
        public init(id: UUID, url: URL) { self.id = id; self.url = url; self.title = url.deletingPathExtension().lastPathComponent }
        public var acceptsCommands: Bool { !isOpening && !isQuitting && operation == nil && !closeRequested }
    }
    public enum Action {
        case command(SlopDocumentCommand)
        case operationFinished(SlopDocumentCommand, URL?)
        case operationFailed(SlopDocumentCommand, SlopDocumentFailure)
        case alert(PresentationAction<ErrorAlertAction>)
    }
    @Dependency(\.documentClient) var client
    public init() {}
    public var body: some ReducerOf<Self> {
        Reduce { state, action in
            switch action {
            case .command(let command):
                // Quit takes no new commands, but an answer to the save-failure sheet still
                // runs: quit waits for it, as it waits for any operation.
                guard !state.isOpening, !state.isQuitting || command.isSaveRecovery else { return .none }
                if state.operation != nil {
                    if command == .close { state.closeRequested = true }
                    if command.isSaveRecovery { state.pendingRecovery = command }
                    return .none
                }
                return run(command, &state)
            case .operationFinished(let command, _):
                guard state.operation == command else { return .none }
                state.operation = nil
                // A closed document has nothing left to recover.
                if command == .close { state.pendingRecovery = nil; return .none }
                if let recovery = state.pendingRecovery {
                    state.pendingRecovery = nil
                    return run(recovery, &state)
                }
                if state.closeRequested {
                    state.closeRequested = false
                    // A queued close must finish even if quit began during the operation.
                    return run(.close, &state)
                }
                return .none
            case .operationFailed(let command, let failure):
                guard state.operation == command else { return .none }
                state.operation = nil; state.closeRequested = false
                // The window's save-failure sheet presents a save failure.
                if case .other(let message) = failure { state.alert = .operationFailure(message) }
                if let recovery = state.pendingRecovery {
                    state.pendingRecovery = nil
                    return run(recovery, &state)
                }
                return .none
            case .alert: return .none
            }
        }
        .ifLet(\.$alert, action: \.alert)
    }
    private func run(_ command: SlopDocumentCommand, _ state: inout State) -> Effect<Action> {
        state.operation = command
        let id = state.id
        return .run { send in
            do { await send(.operationFinished(command, try await client.perform(id, command))) }
            catch { await send(.operationFailed(command, SlopDocumentFailure(error))) }
        }
    }
}
