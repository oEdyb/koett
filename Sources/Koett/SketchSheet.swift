import CoreGraphics
import CoreText
import Foundation
import ImageIO
import UniformTypeIdentifiers

/// Core Graphics port of Linux `sketch/sheet.rs`: ink on a screen still, a
/// crop around it, and one numbered contact sheet with the same constants.
enum SketchSheet {
    static let longEdge: Double = 1600
    static let maxPixels: Double = 1_560_000
    static let gutter: Double = 6
    static let chip: Double = 24
    static let inset: Double = 5
    static let inkWidth: CGFloat = 5
    static let ink = CGColor(srgbRed: 1, green: 59 / 255, blue: 48 / 255, alpha: 1)

    /// Logical crop around the ink: bounding box plus 40 pt, at least 45% of
    /// the screen each way for context, clamped to the screen.
    static func inkCrop(screen: CGSize, strokes: [[CGPoint]]) -> CGRect {
        let (width, height) = (Double(screen.width), Double(screen.height))
        var bounds = (width, height, 0.0, 0.0)
        for point in strokes.joined() {
            bounds = (
                min(bounds.0, point.x), min(bounds.1, point.y),
                max(bounds.2, point.x), max(bounds.3, point.y)
            )
        }
        guard bounds.0 <= bounds.2 else { return CGRect(x: 0, y: 0, width: width, height: height) }
        let cropWidth = min(max(bounds.2 - bounds.0 + 80, width * 0.45), width)
        let cropHeight = min(max(bounds.3 - bounds.1 + 80, height * 0.45), height)
        return CGRect(
            x: min(max((bounds.0 + bounds.2 - cropWidth) / 2, 0), width - cropWidth),
            y: min(max((bounds.1 + bounds.3 - cropHeight) / 2, 0), height - cropHeight),
            width: cropWidth,
            height: cropHeight
        )
    }

    /// Draws the sketch's own ink onto a screen still (any pixel scale) and
    /// crops it around the ink. Only this sketch's strokes are ever drawn.
    static func keyframe(screen image: CGImage, logical: CGSize, strokes: [[CGPoint]]) -> CGImage? {
        guard logical.width > 0, logical.height > 0 else { return image }
        let (width, height) = (image.width, image.height)
        guard let context = canvas(width: width, height: height) else { return nil }
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        let scaleX = CGFloat(width) / logical.width
        let scaleY = CGFloat(height) / logical.height
        context.translateBy(x: 0, y: CGFloat(height))
        context.scaleBy(x: scaleX, y: -scaleY)
        strokeInk(strokes, in: context, alpha: 1)
        guard let inked = context.makeImage() else { return nil }
        let crop = inkCrop(screen: logical, strokes: strokes)
        let left = Int((crop.minX * scaleX).rounded(.down))
        let top = Int((crop.minY * scaleY).rounded(.down))
        let right = min(Int((crop.maxX * scaleX).rounded(.up)), width)
        let bottom = min(Int((crop.maxY * scaleY).rounded(.up)), height)
        return inked.cropping(to: CGRect(x: left, y: top, width: right - left, height: bottom - top))
    }

    /// Strokes in a top-left coordinate space, as the overlay and stills use.
    static func strokeInk(_ strokes: [[CGPoint]], in context: CGContext, alpha: CGFloat) {
        context.setLineWidth(inkWidth)
        context.setLineCap(.round)
        context.setLineJoin(.round)
        context.setStrokeColor(ink.copy(alpha: alpha) ?? ink)
        for stroke in strokes {
            guard let first = stroke.first else { continue }
            context.move(to: first)
            if stroke.count == 1 { context.addLine(to: CGPoint(x: first.x + 0.1, y: first.y)) }
            for point in stroke.dropFirst() { context.addLine(to: point) }
            context.strokePath()
        }
    }

    typealias Row = (indices: [Int], height: Double)

    static func layout(_ aspects: [Double], width: Double) -> [Row] {
        let columns = max(Int(Double(aspects.count).squareRoot().rounded(.up)), 1)
        return stride(from: 0, to: aspects.count, by: columns).map { start in
            let row = Array(start..<min(start + columns, aspects.count))
            let sum = row.reduce(0) { $0 + aspects[$1] }
            let free = width - gutter * (Double(row.count) + 1)
            return (row, free / sum)
        }
    }

    static func canvasHeight(_ rows: [Row]) -> Double {
        rows.reduce(0) { $0 + $1.height } + gutter * (Double(rows.count) + 1)
    }

    /// Canvas width and rows fitting the long-edge and pixel caps.
    static func fit(_ aspects: [Double]) -> (width: Double, rows: [Row]) {
        var width = longEdge
        for _ in 0..<4 {
            let rows = layout(aspects, width: width)
            let height = canvasHeight(rows)
            let shrink = min(longEdge / height, (maxPixels / (width * height)).squareRoot())
            if shrink >= 0.999 { return (width, rows) }
            width = (width * shrink).rounded(.down)
        }
        return (width, layout(aspects, width: width))
    }

    /// One numbered contact sheet from frames in creation order.
    static func contactSheet(_ images: [CGImage]) -> CGImage? {
        guard !images.isEmpty else { return nil }
        let aspects = images.map { Double($0.width) / Double(max($0.height, 1)) }
        let (width, rows) = fit(aspects)
        let height = canvasHeight(rows)
        guard let context = canvas(width: Int(width), height: Int(height.rounded(.up))) else { return nil }
        let canvasHeight = Double(context.height)
        context.setFillColor(CGColor(srgbRed: 0.09, green: 0.09, blue: 0.1, alpha: 1))
        context.fill(CGRect(x: 0, y: 0, width: context.width, height: context.height))
        context.interpolationQuality = .high
        var y = gutter
        for row in rows {
            var x = gutter
            for index in row.indices {
                let tileWidth = aspects[index] * row.height
                let flippedY = canvasHeight - y - row.height
                context.draw(images[index], in: CGRect(x: x, y: flippedY, width: tileWidth, height: row.height))
                drawChip(in: context, x: x + inset, top: y + inset, canvasHeight: canvasHeight, number: index + 1)
                x += tileWidth + gutter
            }
            y += row.height + gutter
        }
        return context.makeImage()
    }

    private static func drawChip(in context: CGContext, x: Double, top: Double, canvasHeight: Double, number: Int) {
        let font = CTFontCreateWithName("Helvetica-Bold" as CFString, 15, nil)
        let label = NSAttributedString(string: "\(number)", attributes: [
            NSAttributedString.Key(kCTFontAttributeName as String): font,
            NSAttributedString.Key(kCTForegroundColorAttributeName as String): CGColor(gray: 0, alpha: 1),
        ])
        let line = CTLineCreateWithAttributedString(label)
        context.textPosition = .zero
        let bounds = CTLineGetImageBounds(line, context)
        let advance = CTLineGetTypographicBounds(line, nil, nil, nil)
        let width = max(chip, advance + 10)
        let rect = CGRect(x: x, y: canvasHeight - top - chip, width: width, height: chip)
        let path = CGPath(roundedRect: rect, cornerWidth: 5, cornerHeight: 5, transform: nil)
        context.addPath(path)
        context.setFillColor(CGColor(gray: 1, alpha: 1))
        context.fillPath()
        context.addPath(path)
        context.setStrokeColor(CGColor(gray: 0, alpha: 0.7))
        context.setLineWidth(1.5)
        context.strokePath()
        context.textPosition = CGPoint(
            x: x + (width - advance) / 2,
            y: rect.midY - bounds.height / 2 - bounds.minY
        )
        CTLineDraw(line, context)
    }

    static func canvas(width: Int, height: Int) -> CGContext? {
        CGContext(
            data: nil,
            width: max(width, 1),
            height: max(height, 1),
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        )
    }

    static func png(_ image: CGImage) -> Data? {
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.png.identifier as CFString, 1, nil) else {
            return nil
        }
        CGImageDestinationAddImage(destination, image, nil)
        return CGImageDestinationFinalize(destination) ? data as Data : nil
    }
}
