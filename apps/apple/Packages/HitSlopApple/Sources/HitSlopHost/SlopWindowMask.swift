import AppKit
import HitSlopCore
import ImageIO

@MainActor final class SlopWindowMask {
    private enum Content {
        case vector(SlopSilhouette)
        case image(CGImage)
    }

    /// A skin's alpha at its own resolution, read only for hit-testing.
    private struct AlphaMap {
        private let context: CGContext

        init?(image: CGImage) {
            guard let context = CGContext(
                data: nil, width: image.width, height: image.height, bitsPerComponent: 8,
                bytesPerRow: image.width, space: CGColorSpaceCreateDeviceGray(),
                bitmapInfo: CGImageAlphaInfo.alphaOnly.rawValue
            ) else { return nil }
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
            self.context = context
        }

        func contains(_ point: CGPoint, in bounds: CGRect) -> Bool {
            guard bounds.width > 0, bounds.height > 0, bounds.contains(point), let data = context.data else { return false }
            let width = context.width, height = context.height
            let x = min(width - 1, max(0, Int((point.x / bounds.width) * CGFloat(width))))
            let appKitY = min(height - 1, max(0, Int((point.y / bounds.height) * CGFloat(height))))
            // Bitmap rows run from the top of the image.
            let y = height - 1 - appKitY
            // 10% alpha: soft edges look smooth without catching empty pixels.
            return data.load(fromByteOffset: y * context.bytesPerRow + x, as: UInt8.self) >= 26
        }
    }

    private let content: Content
    private let transparentBacking: Bool
    private lazy var alphaMap: AlphaMap? = {
        guard case .image(let image) = content else { return nil }
        return AlphaMap(image: image)
    }()
    private var cachedBounds: CGRect?
    private var cachedPath: CGPath?

    private func path(_ silhouette: SlopSilhouette, in bounds: CGRect) -> CGPath {
        if cachedBounds == bounds, let cachedPath { return cachedPath }
        let path = silhouette.path(in: bounds)
        cachedBounds = bounds; cachedPath = path
        return path
    }

    init(file: SlopFile) throws {
        transparentBacking = file.usesTransparentBackground
        if let skin = file.skin {
            content = .image(skin)
        } else {
            content = .vector(file.silhouette)
        }
    }

    private static func show(_ image: CGImage, on layer: CALayer) {
        layer.contents = image
        layer.contentsGravity = .resize
        layer.isGeometryFlipped = true
        layer.magnificationFilter = .linear
        layer.minificationFilter = .linear
    }

    func installBacking(on layer: CALayer?) {
        guard let layer else { return }
        switch content {
        case .vector:
            layer.backgroundColor = transparentBacking ? NSColor.clear.cgColor : NSColor.windowBackgroundColor.cgColor
        case .image(let image):
            Self.show(image, on: layer)
        }
    }

    func makeLayer() -> CALayer {
        switch content {
        case .vector:
            return CAShapeLayer()
        case .image(let image):
            let layer = CALayer()
            Self.show(image, on: layer)
            return layer
        }
    }

    func update(_ layer: CALayer, bounds: CGRect) {
        layer.frame = bounds
        guard case .vector(let silhouette) = content, let shapeLayer = layer as? CAShapeLayer else { return }
        shapeLayer.fillRule = silhouette.fillRule == .evenOdd ? .evenOdd : .nonZero
        shapeLayer.path = path(silhouette, in: bounds)
    }

    func contains(_ point: CGPoint, in bounds: CGRect) -> Bool {
        switch content {
        case .vector(let silhouette):
            return bounds.contains(point) && path(silhouette, in: bounds).contains(point, using: silhouette.fillRule)
        case .image:
            return alphaMap?.contains(point, in: bounds) ?? false
        }
    }

    /// A snapshot clipped to the window's shape, as PNG. `scale` is the snapshot's pixels
    /// per point.
    func png(from image: NSImage, scale: CGFloat = 1) throws -> Data {
        try SlopPreviewImage.png(size: image.size) { [content] rect in
            guard let context = NSGraphicsContext.current?.cgContext else { return }
            switch content {
            case .vector(let silhouette):
                let logical = CGRect(x: rect.minX / scale, y: rect.minY / scale, width: rect.width / scale, height: rect.height / scale)
                var transform = CGAffineTransform(scaleX: scale, y: scale)
                context.addPath(silhouette.path(in: logical).copy(using: &transform)!)
                context.clip(using: silhouette.fillRule)
                image.draw(in: rect)
            case .image(let mask):
                image.draw(in: rect)
                context.setBlendMode(.destinationIn)
                context.draw(mask, in: rect)
            }
        }
    }
}
