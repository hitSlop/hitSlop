import Foundation
import HitSlopCore
import HitSlopCoreBinding

extension DocumentOwner {
  /// A document request from the page `view`. Batches and text edits arrive as JSON text
  /// that only the core parses, and the opened state returns as the core's JSON text.
  @MainActor func admitPage(_ request: PageRequest, view: String, reply: @escaping @MainActor @Sendable ([String: Any]) -> Void) {
    enqueuePage(request, view: view) { outcome in
      DispatchQueue.main.async {
        switch outcome {
        case .failure(let error): reply(RequestOutcome.page(error))
        case .success(let result): reply(result.json)
        }
      }
    }
  }

}
