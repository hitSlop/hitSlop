import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopDocument

/// Catalog file work runs on its own queue, off the main actor and away from document
/// opening. Installed templates are validated once per version; recent documents are
/// listed from their manifests, and opening validates them.
actor CatalogScanner {
    private static let queue = DispatchQueue(label: "hitslop.catalog", qos: .utility)
    private static func run<T: Sendable>(_ work: @escaping @Sendable () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { continuation in
            queue.async { continuation.resume(with: Result { try work() }) }
        }
    }

    /// A package directory as last validated. Installs replace the whole directory, so its
    /// identity and dates change with every new version.
    private struct Version: Equatable, Sendable {
        let identifier: UInt64?
        let modified: Date?
        let manifestModified: Date?
        init(_ url: URL) {
            let root = try? url.resourceValues(forKeys: [.fileIdentifierKey, .contentModificationDateKey])
            identifier = root?.fileIdentifier
            modified = root?.contentModificationDate
            manifestModified = try? url.appendingPathComponent("manifest.json")
                .resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate
        }
    }
    private enum Outcome: Sendable {
        case template(LocalTemplate)
        case issue(String, SlopFailureContext)
    }
    /// Per templates folder, each package's last validation.
    private var validated: [URL: [URL: (version: Version, outcome: Outcome)]] = [:]

    func local(at root: URL, makeImmutable: Bool = true) async throws -> LocalTemplateSnapshot {
        try Task.checkCancellation()
        let children = try await Self.run {
            if makeImmutable { try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true) }
            return try FileManager.default.contentsOfDirectory(
                at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]
            ).filter { $0.pathExtension.lowercased() == "slop" }
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
                outcome = try await validate(child, makeImmutable: makeImmutable)
            }
            seen[child] = (version, outcome)
            switch outcome {
            case .template(let template): result.templates.append(template)
            case .issue(let issue, let diagnostic):
                result.issues.append(issue)
                if !result.diagnostics.contains(diagnostic) { result.diagnostics.append(diagnostic) }
            }
        }
        // Removed packages leave the cache.
        validated[root] = seen
        return result
    }

    private func validate(_ child: URL, makeImmutable: Bool) async throws -> Outcome {
        do {
            let template = try await Self.run {
                let package = try SlopPackage(rootURL: child)
                try package.validateAsTemplate()
                guard child.deletingPathExtension().lastPathComponent == package.manifest.slug else {
                    throw SlopPackageError.invalid("installed filename must match manifest slug")
                }
                if makeImmutable { try? SlopPermissions.makeImmutable(child) }
                let values = try? child.resourceValues(forKeys: [.creationDateKey, .contentModificationDateKey])
                return LocalTemplate(
                    packageURL: child, icon: Self.artwork(at: package.iconURL), preview: Self.artwork(at: package.previewURL),
                    manifest: package.manifest, packageBytes: package.byteCount,
                    createdAt: values?.creationDate, updatedAt: values?.contentModificationDate
                )
            }
            return .template(template)
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            let diagnostic = error is SlopPackageError || error is DecodingError
                ? SlopFailureContext(.rejection, reason: .invalidPackage) : .classify(error)
            return .issue("\(child.lastPathComponent): \(error.localizedDescription)", diagnostic)
        }
    }

    func recents(_ urls: [URL], templatesRoot: URL) async throws -> [CatalogEntry] {
        try Task.checkCancellation()
        var seen = Set<URL>()
        var entries: [CatalogEntry] = []
        for original in urls {
            try Task.checkCancellation()
            guard (try? original.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) != true else { continue }
            let url = original.standardizedFileURL.resolvingSymlinksInPath()
            guard url.pathExtension.lowercased() == "slop", seen.insert(url).inserted,
                  !SlopTemplateLocation.isManagedTemplatePackage(url, templatesRoot: templatesRoot),
                  let entry = try await recent(url) else { continue }
            entries.append(entry)
        }
        return entries
    }

    /// A recent document's entry, or nil once it no longer exists.
    func recent(_ url: URL) async throws -> CatalogEntry? {
        try await Self.run {
            guard FileManager.default.fileExists(atPath: url.path) else { return nil }
            let summary = try? SlopPackageSummary(rootURL: url)
            var entry = CatalogEntry(
                id: "recent:\(url.path)", source: .recent(url),
                title: summary?.manifest.title ?? url.deletingPathExtension().lastPathComponent
            )
            if let summary {
                CatalogServices.apply(summary.manifest, to: &entry)
                let icon = Self.artwork(at: summary.iconURL), preview = Self.artwork(at: summary.previewURL)
                entry.icons = [icon, preview]
                entry.previews = [preview, icon]
                entry.packageBytes = summary.byteCount
            }
            let values = try? url.resourceValues(forKeys: [.creationDateKey, .contentModificationDateKey])
            entry.createdAt = values?.creationDate
            entry.updatedAt = values?.contentModificationDate
            return entry
        }
    }

    private static func artwork(at url: URL) -> CatalogArtwork {
        let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
        return CatalogArtwork(url: url, modifiedAt: values?.contentModificationDate, byteCount: values?.fileSize)
    }
}
