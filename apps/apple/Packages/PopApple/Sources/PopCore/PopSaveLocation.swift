#if os(macOS)
  import Foundation

  /// Native save-panel policy. Document acceptance and relocation remain in the core.
  public enum PopSaveLocation {
    private static let key = "click.dotpop.lastLocalDocumentFolder"

    public static func folder(
      home: URL = FileManager.default.homeDirectoryForCurrentUser,
      defaults: UserDefaults = .standard
    ) throws -> URL {
      let manager = FileManager.default
      let remembered = defaults.string(forKey: key).map { URL(fileURLWithPath: $0, isDirectory: true) }
      for candidate in [remembered, home.appendingPathComponent("Desktop", isDirectory: true)].compactMap({ $0 }) {
        var directory: ObjCBool = false
        if manager.fileExists(atPath: candidate.path, isDirectory: &directory), directory.boolValue,
          manager.isWritableFile(atPath: candidate.path), (try? PopLocalDocument.requireLocal(candidate)) != nil
        {
          return candidate
        }
      }
      return try localFolder(home: home)
    }

    public static func localFolder(home: URL = FileManager.default.homeDirectoryForCurrentUser) throws -> URL {
      let folder = home.appendingPathComponent("pop", isDirectory: true)
      try PopLocalDocument.requireLocal(folder)
      try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
      guard FileManager.default.isWritableFile(atPath: folder.path) else {
        throw PopFailure("The local pop folder is not writable. Choose another local folder.")
      }
      return folder
    }

    public static func remember(_ document: URL, defaults: UserDefaults = .standard) {
      let folder = document.deletingLastPathComponent()
      guard (try? PopLocalDocument.requireLocal(folder)) != nil else { return }
      defaults.set(folder.path, forKey: key)
    }
  }

#endif
