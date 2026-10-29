import Foundation

/// Waits until `condition` holds, checking every 10 ms, for at most `timeout`; returns
/// whether it held. The assertions after it say what was expected.
@discardableResult
public func eventually(
  timeout: Duration = .seconds(2), isolation: isolated (any Actor)? = #isolation, _ condition: () async throws -> Bool
) async rethrows -> Bool {
  let end = ContinuousClock.now + timeout
  while try await !condition() {
    guard ContinuousClock.now < end else { return false }
    try? await Task.sleep(for: .milliseconds(10))
  }
  return true
}
