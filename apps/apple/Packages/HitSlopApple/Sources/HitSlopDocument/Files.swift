import Foundation

func safeDirectory(_ url: URL) throws {
  if !FileManager.default.fileExists(atPath: url.path) {
    try FileManager.default.createDirectory(at: url, withIntermediateDirectories: false)
  }
  let info = try url.resourceValues(forKeys: [.isDirectoryKey, .isSymbolicLinkKey])
  guard info.isDirectory == true, info.isSymbolicLink != true else {
    throw failure("Unsafe directory: \(url.lastPathComponent)")
  }
}
func safeFile(_ url: URL, optional: Bool = false) throws {
  let info: URLResourceValues
  do { info = try url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey]) } catch {
    if optional && (error as NSError).code == NSFileReadNoSuchFileError { return }
    throw error
  }
  guard info.isRegularFile == true, info.isSymbolicLink != true else {
    throw failure("Unsafe file: \(url.lastPathComponent)")
  }
}
