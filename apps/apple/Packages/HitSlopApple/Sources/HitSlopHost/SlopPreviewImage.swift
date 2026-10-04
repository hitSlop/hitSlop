import AppKit
import Foundation
import HitSlopCore

@MainActor enum SlopPreviewImage {
    /// A PNG of `draw`, rendered into a bitmap of exactly `size` pixels.
    static func png(size: NSSize, draw: (NSRect) -> Void) throws -> Data {
        let width = max(1, Int(size.width.rounded()))
        let height = max(1, Int(size.height.rounded()))
        guard let bitmap = NSBitmapImageRep(
            bitmapDataPlanes: nil,
            pixelsWide: width,
            pixelsHigh: height,
            bitsPerSample: 8,
            samplesPerPixel: 4,
            hasAlpha: true,
            isPlanar: false,
            colorSpaceName: .deviceRGB,
            bytesPerRow: 0,
            bitsPerPixel: 0
        ), let context = NSGraphicsContext(bitmapImageRep: bitmap) else {
            throw SlopError.invalid("could not encode rendered image")
        }
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = context
        draw(NSRect(x: 0, y: 0, width: width, height: height))
        context.flushGraphics()
        NSGraphicsContext.restoreGraphicsState()
        guard let png = bitmap.representation(using: .png, properties: [:]) else {
            throw SlopError.invalid("could not encode rendered image")
        }
        return png
    }

    /// A snapshot's pixels as PNG; its size is already in pixels.
    static func png(from image: NSImage) throws -> Data {
        try png(size: image.size) { image.draw(in: $0) }
    }

    static func png(from image: NSImage, file: SlopFile, scale: CGFloat = 1) throws -> Data {
        try SlopWindowMask(file: file).png(from: image, scale: scale)
    }
}
