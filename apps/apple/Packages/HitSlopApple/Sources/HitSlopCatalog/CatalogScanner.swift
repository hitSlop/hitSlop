import Foundation
import HitSlopCore
import HitSlopFeatures
import HitSlopRuntime

/// Filesystem work is isolated from the main actor and yields during large enumerations.
actor CatalogScanner {
    func local(at root: URL, makeImmutable: Bool = true) async throws -> LocalTemplateSnapshot {
        try Task.checkCancellation()
        if makeImmutable { try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true) }
        let children = try FileManager.default.contentsOfDirectory(
            at: root, includingPropertiesForKeys: [.isDirectoryKey], options: [.skipsHiddenFiles]
        ).sorted { $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending }
        var result = LocalTemplateSnapshot()
        for child in children where child.pathExtension.lowercased() == "slop" {
            try Task.checkCancellation()
            do {
                let package = try await SlopPreparation.run {
                    let package = try SlopPackage(rootURL: child)
                    try package.validateAsTemplate()
                    return package
                }
                guard child.deletingPathExtension().lastPathComponent == package.manifest.slug else {
                    throw SlopPackageError.invalid("installed filename must match manifest slug")
                }
                let values = try? child.resourceValues(forKeys: [.creationDateKey, .contentModificationDateKey])
                let bytes = try await byteCount(child)
                try Task.checkCancellation()
                if makeImmutable { try? SlopDuplicator.makeImmutable(child) }
                result.templates.append(LocalTemplate(
                    packageURL: child, icon: artwork(at: package.iconURL), preview: artwork(at: package.previewURL),
                    manifest: package.manifest, packageBytes: bytes,
                    createdAt: values?.creationDate, updatedAt: values?.contentModificationDate
                ))
            } catch is CancellationError { throw CancellationError() }
            catch {
                result.issues.append("\(child.lastPathComponent): \(error.localizedDescription)")
                let diagnostic = error is SlopPackageError || error is DecodingError
                  ? SlopFailureContext(.rejection, reason: .invalidPackage) : .classify(error)
                if !result.diagnostics.contains(diagnostic) { result.diagnostics.append(diagnostic) }
            }
            await Task.yield()
        }
        return result
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
                  FileManager.default.fileExists(atPath: url.path),
                  !DocumentFactory.isManagedTemplatePackage(url, templatesRoot: templatesRoot) else { continue }
            let package = try? await SlopPreparation.run { try SlopPackage(rootURL: url) }
            var entry = CatalogEntry(
                id: "recent:\(url.path)", source: .recent(url),
                title: package?.manifest.title ?? url.deletingPathExtension().lastPathComponent
            )
            if let package {
                CatalogServices.apply(package.manifest, to: &entry)
                let icon = artwork(at: package.iconURL), preview = artwork(at: package.previewURL)
                entry.icons = [icon, preview]
                entry.previews = [preview, icon]
            }
            entry.packageBytes = try await byteCount(url)
            let values = try? url.resourceValues(forKeys: [.creationDateKey, .contentModificationDateKey])
            entry.createdAt = values?.creationDate
            entry.updatedAt = values?.contentModificationDate
            entries.append(entry)
            await Task.yield()
        }
        return entries
    }

    private func artwork(at url: URL) -> CatalogArtwork {
        let values = try? url.resourceValues(forKeys: [.contentModificationDateKey, .fileSizeKey])
        return CatalogArtwork(url: url, modifiedAt: values?.contentModificationDate, byteCount: values?.fileSize)
    }

    private func byteCount(_ root: URL) async throws -> Int64 {
        guard let enumerator = FileManager.default.enumerator(
            at: root, includingPropertiesForKeys: [.fileSizeKey], options: [.skipsHiddenFiles]
        ) else { return 0 }
        var total: Int64 = 0
        var count = 0
        while let url = enumerator.nextObject() as? URL {
            total += Int64((try? url.resourceValues(forKeys: [.fileSizeKey]).fileSize) ?? 0)
            count += 1
            if count.isMultiple(of: 128) {
                try Task.checkCancellation()
                await Task.yield()
            }
        }
        try Task.checkCancellation()
        return total
    }
}
