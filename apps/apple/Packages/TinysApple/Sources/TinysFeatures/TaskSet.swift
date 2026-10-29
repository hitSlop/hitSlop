/// The work a model started. Nothing in the app awaits it; tests wait for all of it with
/// `settled()`.
@MainActor final class TaskSet {
  private var tasks: [Int: Task<Void, Never>] = [:]
  private var next = 0

  var isEmpty: Bool { tasks.isEmpty }

  @discardableResult func run(_ body: @escaping @MainActor () async -> Void) -> Task<Void, Never> {
    next += 1
    let key = next
    // The task can only start once this returns to the main actor, after it is recorded.
    let task = Task { @MainActor in
      await body()
      self.tasks[key] = nil
    }
    tasks[key] = task
    return task
  }

  /// Returns once every task has finished, including tasks they started.
  func settled() async {
    while let task = tasks.values.first { await task.value }
  }
}
