import Foundation

/// Commands shared by native document windows and the application's reducers.
public enum SlopDocumentCommand: Equatable, Sendable {
    case pin(Bool), exportPNG, exportPDF, duplicate, reveal, copyPath, openEditor(URL), retry, close
    /// Shows or hides the theme panel; imports or exports a theme file.
    case theme(Bool), importTheme, exportTheme
}
