import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopDocument

/// Catalog file work runs on its own queue, off the main actor and away from document
/// opening. Installed templates are validated once per version; recent documents are
/// listed from their checked apps.
actor CatalogScanner {
    private static let queue = DispatchQueue(label: "hitslop.catalog", qos: .utility)
    private static func run<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { continuation.resume(with: Result { try work() }) }
        }
    }

    /// A template file as last validated. Installs replace the whole file, so its identity,
    /// date or size changes with every new version.
    private struct Version: Equatable, Sendable {
        let identifier: UInt64?
        let modified: Date?
        let size: Int?
        init(_ url: URL) {
            let values = try? url.resourceValues(forKeys: [.fileIdentifierKey, .contentModificationDateKey, .fileSizeKey])
            identifier = values?.fileIdentifier
            modified = values?.contentModificationDate
            size = values?.fileSize
        }
    }
    private enum Outcome: Sendable {
        case template(LocalTemplate)
        case issue(String, SlopFailureContext)
    }
    /// Per templates folder, each template file's last validation.
    private var validated: [URL: [URL: (version: Version, outcome: Outcome)]] = [:]

    func local(at root: URL) async throws -> LocalTemplateSnapshot {
        try Task.checkCancellation()
        let children = try await Self.run {
            // The installed templates folder may not exist yet.
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
            return try FileManager.default.contentsOfDirectory(
                at: root, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles]
            ).filter { $0.pathExtension == "slop" }
                .sorted { $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending }
        }
        var result = LocalTemplateSnapshot()
        var seen: [URL: (version: Version, outcome: Outcome)] = [:]
        for child in children {
            try Task.checkCancellation()
            let version = Version(child)
            let outcome: Outcome
            if let cached = validated[root]?[child], cached.version == version {
                outcome = cached.outcome
            } else {
                outcome = try await validate(child)
            }
            seen[child] = (version, outcome)
            switch outcome {
            case .template(let template): result.templates.append(template)
            case .issue(let issue, let diagnostic):
                result.issues.append(issue)
                if !result.diagnostics.contains(diagnostic) { result.diagnostics.append(diagnostic) }
            }
        }
        // Removed templates leave the cache.
        validated[root] = seen
        return result
    }

    private func validate(_ child: URL) async throws -> Outcome {
        do {
            let template = try await Self.run {
                let file = try SlopFile(url: child)
                guard file.kind == .template else { throw SlopError.invalid("an installed template holds no document") }
                guard child.deletingPathExtension().lastPathComponent == file.manifest.slug else {
                    throw SlopError.invalid("installed filename must match manifest slug")
                }
                let values = try? child.resourceValues(forKeys: [.creationDateKey, .contentModificationDateKey])
                return LocalTemplate(
                    url: child, icon: Self.artwork(child, .icon), preview: Self.artwork(child, .preview),
                    manifest: file.manifest, fileBytes: file.byteCount,
                    createdAt: values?.creationDate, updatedAt: values?.contentModificationDate
                )
            }
            return .template(template)
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            let diagnostic = error is SlopError
                ? SlopFailureContext(.rejection, reason: .invalidFile) : .classify(error)
            return .issue("\(child.lastPathComponent): \(error.localizedDescription)", diagnostic)
        }
    }

    func recents(_ urls: [URL]) async throws -> [CatalogEntry] {
        try Task.checkCancellation()
        var seen = Set<URL>()
        var entries: [CatalogEntry] = []
        for original in urls {
            try Task.checkCancellation()
            guard (try? original.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) != true else { continue }
            let url = SlopPath.canonical(original)
            guard url.pathExtension == "slop", seen.insert(url).inserted,
                  let entry = try await recent(url) else { continue }
            entries.append(entry)
        }
        return entries
    }

    /// A recent document's entry, or nil once it no longer exists.
    func recent(_ url: URL) async throws -> CatalogEntry? {
        try await Self.run {
            guard FileManager.default.fileExists(atPath: url.path) else { return nil }
            let summary = try? SlopFile(url: url)
            var entry = CatalogEntry(
                id: "recent:\(url.path)", source: .recent(url),
                title: summary?.manifest.title ?? url.deletingPathExtension().lastPathComponent
            )
            if let summary {
                CatalogServices.apply(summary.manifest, to: &entry)
                let icon = Self.artwork(url, .icon), preview = Self.artwork(url, .preview)
                entry.icons = [icon, preview]
                entry.previews = [preview, icon]
                entry.fileBytes = summary.byteCount
            }
            let values = try? url.resourceValues(forKeys: [.creationDateKey, .contentModificationDateKey])
            entry.createdAt = values?.creationDate
            entry.updatedAt = values?.contentModificationDate
            return entry
        }
    }

    private static func artwork(_ file: URL, _ name: CatalogArtwork.Name) -> CatalogArtwork {
        let values = try? file.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
        return CatalogArtwork(file: file, name: name, modifiedAt: values?.contentModificationDate, byteCount: values?.fileSize)
    }
}
