import Foundation

public enum SlopLocalDocument {
    /// Check existing ancestors as destinations may not exist yet. Resolve aliases
    /// before comparing components; a sibling prefix is not an iCloud directory.
    public static func requireLocal(_ url: URL, iCloudRoot: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Mobile Documents")) throws {
        let resolved = resolvingExistingAncestors(url)
        let cloud = resolvingExistingAncestors(iCloudRoot).pathComponents
        let path = resolved.pathComponents
        let inside = path.count >= cloud.count && zip(cloud, path).allSatisfy { $0.caseInsensitiveCompare($1) == .orderedSame }
        var current = resolved
        var ubiquitous = false
        while current.path != "/" {
            if (try? current.resourceValues(forKeys: [.isUbiquitousItemKey]).isUbiquitousItem) == true { ubiquitous = true; break }
            current.deleteLastPathComponent()
        }
        guard !inside && !ubiquitous else {
            throw SlopFailure("iCloud document locations are not supported. Move the document to a local folder.")
        }
    }

    private static func resolvingExistingAncestors(_ url: URL) -> URL {
        var ancestor = url.standardizedFileURL
        var missing: [String] = []
        while !FileManager.default.fileExists(atPath: ancestor.path), ancestor.path != "/" {
            missing.append(ancestor.lastPathComponent)
            ancestor.deleteLastPathComponent()
        }
        return missing.reversed().reduce(ancestor.resolvingSymlinksInPath()) { $0.appendingPathComponent($1) }
    }
}
