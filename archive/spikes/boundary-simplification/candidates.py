"""Apply independent candidates to a frozen experiment copy, never the source tree."""
from pathlib import Path
import sys, re

repo = Path(__file__).resolve().parents[3]
name = sys.argv[1]
target = sys.argv[2] if len(sys.argv) > 2 else name
root = repo / 'generated/boundary-simplification' / target
assert root.is_dir() and name != 'baseline'
native = 'apps/apple/Packages/HitSlopApple/'
doc = native + 'Sources/HitSlopDocument/'
core = native + 'Sources/HitSlopCore/'
def edit(path, fn):
    p = root / path
    p.write_text(fn(p.read_text()))
def replace(s, a, b):
    assert a in s, a[:100]
    return s.replace(a, b)
def between(s, a, b, content):
    start = s.index(a); end = s.index(b, start)
    return s[:start] + content + s[end:]

if name in ['B', 'C', 'D']:
    def storage(s):
        s = s.replace('generation INTEGER NOT NULL, ', '').replace(', last_attempt TEXT', '').replace('NULL,NULL,0,lower(hex(randomblob(16))),NULL', 'NULL,NULL,lower(hex(randomblob(16)))')
        s = s.replace('schema_key=?,generation=?,doc_id=?', 'schema_key=?,doc_id=?')
        s = s.replace('    sqlite3_bind_int64(row, 3, saved.generation)\n', '').replace('sqlite3_bind_text(row, 4, saved.docID', 'sqlite3_bind_text(row, 3, saved.docID')
        s = s.replace('    var generation: Int64\n', '').replace('    var lastAttempt: String?\n', '')
        s = s.replace('schemaKey: saved.schemaKey, generation: saved.generation,', 'schemaKey: saved.schemaKey,')
        s = s.replace('schemaKey: String?, generation: Int64, docID:', 'schemaKey: String?, docID:')
        s = s.replace('SELECT checkpoint,schema_key,generation,doc_id', 'SELECT checkpoint,schema_key,doc_id').replace('sqlite3_column_text(row, 3)', 'sqlite3_column_text(row, 2)')
        s = s.replace('generation: sqlite3_column_int64(row, 2), docID:', 'docID:')
        s = s.replace('SELECT generation,COALESCE(length(checkpoint),0),(SELECT COALESCE(sum(length(bytes)),0) FROM updates),(SELECT count(*) FROM updates),last_attempt', 'SELECT COALESCE(length(checkpoint),0),(SELECT COALESCE(sum(length(bytes)),0) FROM updates),(SELECT count(*) FROM updates)')
        s = between(s, '    return Metadata(', '\n  }\n\n  /// Commits', '''    return Metadata(rows: sqlite3_column_int64(row, 2), updateBytes: sqlite3_column_int64(row, 1), checkpointBytes: sqlite3_column_int64(row, 0))''')
        s = between(s, '  /// Commits one write', '    try exec("BEGIN IMMEDIATE")', '''  /// An error may follow a commit. The owner conservatively retries encoded state.
  func write(_ write: Write) throws {
    do { try checkLocation() } catch { throw SaveFailure.moved }
''')
        s = between(s, '      guard try scalar("SELECT generation', '      let method:', '')
        s = s.replace('        try exec("UPDATE document SET generation=generation+1 WHERE id=1")\n', '').replace('schema_key=?,generation=generation+1', 'schema_key=?')
        s = between(s, '      let marker = try statement(', '      #if DEBUG', '')
        s = s.replace('      return try scalar("SELECT generation FROM document WHERE id=1")\n', '')
        if 'spikeWriteMS' in s and 'let started = DispatchTime.now()' not in s:
            s = s.replace('    try exec("BEGIN IMMEDIATE")', '    let started = DispatchTime.now().uptimeNanoseconds\n    defer { spikeWriteMS.append(Double(DispatchTime.now().uptimeNanoseconds - started) / 1e6) }\n    try exec("BEGIN IMMEDIATE")')
        return s
    edit(doc+'Storage.swift', storage)
    def owner(s):
        s = s.replace('  private var generation: Int64\n', '').replace('    let generation: Int64\n', '').replace('    let attempt: String\n', '')
        s = s.replace('  /// A write whose reply was lost and whose outcome is not yet known.\n  private var uncertainWrite: WriteJob?\n', '  private var refreshNeeded = false\n')
        s = s.replace('case committed(Int64)', 'case committed').replace('    /// The reply was lost and the attempt token could not be read.\n    case uncertain(SaveFailure)\n', '')
        s = s.replace('        generation = restored.generation\n', '').replace('generation = try storage.write', 'try storage.write').replace(', generation: loaded.generation, attempt: UUID().uuidString', '')
        s = s.replace('core: core, generation: loaded.generation,', 'core: core,')
        s = s.replace('    if let pending = uncertainWrite { resolve(pending); return }', '    if refreshNeeded { refreshStorage(); return }')
        s = s.replace('    let attempt = UUID().uuidString\n', '').replace('epoch: epoch, attempt: attempt,', 'epoch: epoch,').replace('generation: generation, write:', 'write:')
        s = between(s, '  /// A successful SQLite commit may lose its reply.', '  private func finish(', '''  private func perform(_ job: WriteJob) -> Outcome {
    do { try storage.write(job.write); return .committed }
    catch { return .failed(error as? SaveFailure ?? .io(error.localizedDescription)) }
  }
''')
        s = between(s, '    switch outcome {', '    // After a failure', '''    switch outcome {
    case .committed: commit(job)
    case .failed(let failure):
      refreshNeeded = true
      fail(failure, upTo: job.target)
    }
''')
        s = s.replace('private func commit(_ job: WriteJob, generation observed: Int64)', 'private func commit(_ job: WriteJob)').replace('    generation = observed\n', '')
        s = between(s, '  /// Settles a lost reply', '  /// Resumes waiters', '''  /// Refresh size accounting after an uncertain save before choosing append/checkpoint.
  private func refreshStorage() {
    writing = true
    let epoch = self.epoch
    storage.queue.async {
      let result = Result { try self.storage.metadata() }
      self.queue.async {
        guard self.epoch == epoch else { return }
        self.writing = false
        switch result {
        case .success(let meta):
          self.stored = (meta.rows, meta.updateBytes, meta.checkpointBytes)
          self.refreshNeeded = false
          self.pump()
        case .failure(let error): self.fail(error as? SaveFailure ?? .io(error.localizedDescription), upTo: .max)
        }
      }
    }
  }
''')
        s = s.replace('    let fenced = try await enqueue(allowInvalidated: true) {', '    try await enqueue(allowInvalidated: true) {')
        s = s.replace('      let fenced = self.inFlight ?? self.uncertainWrite\n', '').replace('      return fenced\n', '')
        s = s.replace('        self.generation = restored.generation\n', '').replace('        self.uncertainWrite = nil', '        self.refreshNeeded = false')
        s = between(s, '        if var pending = fenced {', '        self.pump()', '        self.refreshNeeded = true\n')
        return s
    edit(doc+'DocumentOwner.swift', owner)
    edit(doc+'StorageProbe.swift', lambda s: s.replace(', generation: disk.generation, attempt: UUID().uuidString', ''))
    def tests(s):
        s = between(s, '  // Lost-reply recovery depends', '  @Test @MainActor', '')
        s = re.sub(r', generation: [^,\n]+, attempt: UUID\(\)\.uuidString', '', s)
        s = re.sub(r'    #expect\(loaded.generation == saved\)\n', '', s)
        s = re.sub(r'let (?:saved|appended) = try (\w+)\.write', r'try \1.write', s)
        return s
    edit(native+'Tests/HitSlopDocumentTests/DocumentSessionTests.swift', tests)

if name in ['C', 'D']:
    def snapshots(s):
        s = s.replace(' CREATE TABLE updates(seq INTEGER PRIMARY KEY, bytes BLOB NOT NULL);', '')
        s = between(s, '    for (seq, bytes) in saved.updates {', '\n  private func readTheme()', '  }\n')
        s = s.replace('let rows = try scalar("SELECT count(*) FROM updates")', 'let rows: Int64 = 0')
        s = s.replace('      try scalar("SELECT COALESCE(sum(length(bytes)),0) FROM updates")\n      + scalar', '      try scalar')
        s = between(s, '    case append(Data)', '    case checkpoint', '')
        s = between(s, '    let updates = try statement("SELECT seq,bytes', '    return Saved(', '')
        s = s.replace('docID: docID, updates: records', 'docID: docID, updates: []')
        s = s.replace('(SELECT COALESCE(sum(length(bytes)),0) FROM updates),(SELECT count(*) FROM updates)', '0,0')
        s = between(s, '      case .append(let bytes):', '      case .checkpoint', '')
        s = s.replace('        try exec("DELETE FROM updates")\n', '')
        return s
    edit(doc+'Storage.swift', snapshots)
    def owner_snap(s):
        s = s.replace('  private var savedVersion: String\n', '').replace('    savedVersion = try core.version()\n', '')
        s = s.replace('  private var refreshNeeded = false\n', '')
        s = s.replace('    if refreshNeeded { refreshStorage(); return }\n', '')
        s = between(s, '      let version = try core.version()\n      if version', '      writing = true', '      let job = try makeJob()\n')
        s = between(s, '  private func makeJob(', '  private func perform(', '''  private func makeJob() throws -> WriteJob {
    let checkpoint = try core.checkpoint()
    guard Int64(checkpoint.count + schemaKey.utf8.count + 512) <= Storage.maximumBytes else { throw SaveFailure.full }
    return WriteJob(epoch: epoch, target: sequence, checkpoint: true, bytes: Int64(checkpoint.count), write: .checkpoint(checkpoint, schemaKey: schemaKey))
  }
''')
        s = s.replace('    let version: String\n', '').replace('      refreshNeeded = true\n', '').replace('    savedVersion = job.version\n', '')
        s = between(s, '  /// Refresh size accounting', '  /// Resumes waiters', '')
        s = s.replace('        let version = try restored.core.version()\n', '').replace('        self.savedVersion = version\n', '').replace('        self.refreshNeeded = false\n', '').replace('        self.refreshNeeded = true\n', '')
        return s
    edit(doc+'DocumentOwner.swift', owner_snap)
    edit(doc+'StorageProbe.swift', lambda s: between(s, '      let write: Storage.Write', '      _ = try storage.write', '      let write: Storage.Write = .checkpoint(try core.checkpoint(), schemaKey: schemaKey)\n'))
    edit('scripts/crash-matrix.ts', lambda s: s.replace('  "append:uncommitted",\n', '').replace('  "append:committed",\n', ''))
    def snapshot_tests(s):
        s = s.replace('.append(Data([1, 2, 3]))', '.checkpoint(Data([1, 2, 3]), schemaKey: "key")').replace('.append(Data([4]))', '.checkpoint(Data([4]), schemaKey: "key")').replace('.append(Data([1]))', '.checkpoint(Data([1]), schemaKey: "key")')
        s = s.replace('.append(Data(repeating: 7, count: 8 * 1024 * 1024))', '.checkpoint(Data(repeating: 7, count: 8 * 1024 * 1024), schemaKey: "key")')
        s = s.replace('#expect(loaded.updates == [Data([1, 2, 3])])', '#expect(loaded.checkpoint == Data([1, 2, 3]))')
        s = between(s, '  @Test(arguments: ["rows", "bytes"])', '\n  @Test', '') if '  @Test(arguments: ["rows", "bytes"])' in s and '\n  @Test' in s[s.index('  @Test(arguments: ["rows", "bytes"])')+10:] else s
        return s
    edit(native+'Tests/HitSlopDocumentTests/DocumentSessionTests.swift', snapshot_tests)
    # Phases are the real snapshot writes for these variants.
    for p in (root/(native+'Tests/HitSlopDocumentTests')).glob('*.swift'):
        s=p.read_text().replace('append:uncommitted', 'checkpoint:uncommitted').replace('append:committed', 'checkpoint:committed')
        p.write_text(s)

if name == 'D':
    # One executor; storage initialization completes before the owner is published.
    edit(doc+'DocumentOwner.swift', lambda s: s.replace('  let queue = DispatchQueue(label: "hitslop.owner")', '  var queue: DispatchQueue { storage.queue }').replace('''      storage.queue.async {
        let outcome = self.perform(job)
        self.queue.async { self.finish(job, outcome) }
      }''', '''      let outcome = perform(job)
      finish(job, outcome)'''))

if name == 'sqlite-module':
    p=root/(core+'SlopDuplicator.swift')
    s=p.read_text()
    permissions=s[s.index('  public static func makeWritable'):]
    (root/(doc+'SlopDuplicator.swift')).write_text(s[:s.index('  public static func makeWritable')]+'}\n')
    p.write_text('import Foundation\n\npublic enum SlopPermissions {\n'+permissions)
    for p in (root/native).rglob('*.swift'):
        if '.build' in p.parts or 'Generated' in p.parts: continue
        s=p.read_text().replace('SlopDuplicator.makeWritable', 'SlopPermissions.makeWritable').replace('SlopDuplicator.makeImmutable', 'SlopPermissions.makeImmutable')
        if p.name=='SlopDuplicator.swift' and 'HitSlopDocument' in p.parts:
            s=s.replace('import Foundation', 'import Foundation\nimport HitSlopCore').replace('try makeWritable(', 'try SlopPermissions.makeWritable(').replace('try? makeWritable(', 'try? SlopPermissions.makeWritable(')
        if p.name=='SlopPackageTests.swift': s=s.replace('@testable import HitSlopCore', '@testable import HitSlopCore\n@testable import HitSlopDocument')
        p.write_text(s)
    edit(native+'Package.swift', lambda s: s.replace('.linkedFramework("ImageIO"), .linkedLibrary("sqlite3")', '.linkedFramework("ImageIO")').replace('.testTarget(name: "HitSlopCoreTests", dependencies: ["HitSlopCore"])', '.testTarget(name: "HitSlopCoreTests", dependencies: ["HitSlopCore", "HitSlopDocument"])'))

if name == 'session':
    def session(s):
        s=s.replace('  public let package: SlopPackage\n', '  public let package: SlopPackage\n  public private(set) var purpose: SlopPagePurpose = .interactive\n', 1)
        return replace(s, '''    packageURL: URL, storage mode: StorageMode = .document
  ) async throws -> DocumentSession {
    let prepared = try await prepare(packageURL: packageURL, storage: mode)
    return try await finishOpening(prepared)''', '''    packageURL: URL, renderTargetsEnabled: Bool = false, purpose: SlopPagePurpose = .interactive
  ) async throws -> DocumentSession {
    let prepared = try await prepare(packageURL: packageURL, storage: purpose.storageMode)
    let session = try await finishOpening(prepared)
    session.purpose = purpose
    session.allowsFileSelection = purpose == .interactive
    session.renderTargetsEnabled = renderTargetsEnabled
    return session''')
    edit(doc+'DocumentSession.swift', session)
    (root/(doc+'SlopPageSession.swift')).unlink()
    for p in (root/native).rglob('*.swift'):
        if '.build' in p.parts or 'Generated' in p.parts: continue
        s=p.read_text().replace('SlopPageSession', 'DocumentSession').replace('session.engine', 'session').replace('session.finish()', 'session.close()')
        if p.name=='SlopWindow.swift':
            s=s.replace(',\n  DocumentSessionDelegate', '')
            s=replace(s, '    session.delegate = self', '''    session.onResize = { [weak self] size in
      guard let self else { throw SlopPackageError.invalid("No window") }
      return try self.pageSession(self.session, resizeContentTo: size)
    }
    session.onReady = { [weak self] in guard let self else { return }; self.pageSessionDidBecomeReady(self.session) }
    session.onRecovered = { [weak self] in guard let self else { return }; self.pageSessionRecovered(self.session) }
    session.onStatus = { [weak self] status in guard let self else { return }; self.pageSession(self.session, saveStatus: status) }
    session.onStorageFailure = { [weak self] failure in guard let self else { return }; self.pageSession(self.session, storageFailure: failure) }
    session.onIssue = { [weak self] message, operation in
      guard let self else { return }
      self.pageSession(self.session, didReport: SlopPageIssue(source: operation ? .document : .unhandled, message: message))
    }
    session.onError = { [weak self] message in
      guard let self else { return }
      self.pageSession(self.session, didFail: SlopDiagnosticError(SlopPackageError.invalid(message), diagnostic: .init(self.session.failureClassification, reason: self.session.failureReason ?? .startup)))
    }''')
        p.write_text(s)

if name == 'shape':
    edit('crates/hitslop-core/Cargo.toml', lambda s: s.replace('[dependencies]', '[dependencies]\nsvgtypes = "=0.16.1"'))
    edit('crates/hitslop-core/src/shape.rs', lambda s: s[:s.index('/// Bounded SVG path data.')] + (repo/'archive/spikes/boundary-simplification/svg-parser.rs').read_text())

if name == 'validation':
    edit('crates/hitslop-core-ffi/src/lib.rs', lambda s: replace(s, '#[uniffi::export]\npub fn schema_key', '''#[uniffi::export]
pub fn validate_document(schema_json: String, initial_json: String) -> Result<String, CoreError> {
    hitslop_core::validate(&schema_json, &initial_json).map_err(rejected)?;
    hitslop_core::schema_key(&schema_json).map_err(rejected)
}
#[uniffi::export]
pub fn validate_theme_defaults(json: String) -> Result<(), CoreError> {
    hitslop_core::theme::validate_defaults(&json).map_err(rejected)
}
#[uniffi::export]
pub fn schema_key'''))
    def package(s):
        s=s.replace('  public let manifest: SlopManifest', '  public let schemaKey: String\n  public let manifest: SlopManifest')
        # Stored properties must all be initialized before accessing computed properties.
        s=s.replace('    silhouette = SlopSilhouette(parsed: decoded.silhouette)', '''    silhouette = SlopSilhouette(parsed: decoded.silhouette)
    do {
      func utf8(_ file: String, maximum: Int) throws -> String {
        let bytes = try SlopFile.read(root.appendingPathComponent(file), within: root, maximumBytes: maximum)
        guard let value = String(data: bytes, encoding: .utf8) else { throw SlopPackageError.invalid("\\(file) must be UTF-8") }
        return value
      }
      schemaKey = try validateDocument(schemaJson: utf8("state.schema.json", maximum: 1_048_576), initialJson: utf8("initial.json", maximum: SlopFile.maximumBytes))
      if fileManager.fileExists(atPath: root.appendingPathComponent("assets/theme.json").path) {
        try validateThemeDefaults(json: utf8("assets/theme.json", maximum: 65_536))
      }
    } catch let error as CoreError {
      switch error {
      case .Rejected(_, let message, _): throw SlopPackageError.invalid(message)
      case .Invalidated(let message): throw SlopPackageError.invalid(message)
      }
    }''')
        s=s.replace('    try validateSchemaMetadata()\n', '')
        return between(s, '  private func validateSchemaMetadata()', '  private func validateDocumentSkill', '')
    edit(core+'SlopPackage.swift', package)
    edit(doc+'DocumentOwner.swift', lambda s: between(s, '    let descriptor = try SlopFile.read(', '    storage = try Storage', '    schemaKey = package.schemaKey\n'))
    edit('packages/document/tests/cli.native.test.ts', lambda s: replace(s,
        '    await writeFile(join(root, "state.schema.json"), JSON.stringify(changed.descriptor));',
        '    await writeFile(join(root, "state.schema.json"), JSON.stringify(changed.descriptor));\n    await writeFile(join(root, "initial.json"), JSON.stringify({ title: "Initial", extra: false }));'))


phase = 'checkpoint' if name in ['C','D'] else 'append'
p=root/(native+'Tests/HitSlopDocumentTests/BoundarySpike.swift')
if p.exists(): p.write_text(p.read_text().replace('SPIKE_PHASE', phase))
if name in ['C', 'D']:
    import subprocess
    subprocess.run([sys.executable, str(repo/'archive/spikes/boundary-simplification/snapshot-cleanup.py'), target], check=True)
print('Applied', name)

if name == "sqlite-module":
    import subprocess
    subprocess.run([sys.executable, str(repo/"archive/spikes/boundary-simplification/sqlite-open.py"), target], check=True)

if name in ["B", "C", "D"]:
    import subprocess
    subprocess.run([sys.executable, str(repo/"archive/spikes/boundary-simplification/host-retry-tests.py"), target], check=True)
