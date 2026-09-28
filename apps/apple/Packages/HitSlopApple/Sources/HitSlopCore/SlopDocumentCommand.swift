import Foundation

/// Commands shared by native document windows and the application's reducers.
public enum SlopDocumentCommand: Equatable, Sendable {
    case pin(Bool), exportPNG, exportPDF, duplicate, reveal, copyPath, openEditor(URL), retry, close
}
