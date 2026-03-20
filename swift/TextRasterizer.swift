import AppKit
import CoreText

struct RasterizedText {
    let rgba: [UInt8]
    let width: Int
    let height: Int
}

class TextRasterizer {
    static func rasterize(text: String, color: NSColor, fontSize: CGFloat) -> RasterizedText {
        let font = NSFont.boldSystemFont(ofSize: fontSize)
        let strokeWidth = fontSize * 0.08 // outline thickness
        let shadowOffset: CGFloat = 2.0
        let shadowBlur: CGFloat = 4.0
        let padding = Int(ceil(strokeWidth + shadowBlur + shadowOffset)) + 4

        // Measure text size
        let measureAttrs: [NSAttributedString.Key: Any] = [
            .font: font,
        ]
        let measureString = NSAttributedString(string: text, attributes: measureAttrs)
        let measureLine = CTLineCreateWithAttributedString(measureString)

        var ascent: CGFloat = 0
        var descent: CGFloat = 0
        var leading: CGFloat = 0
        let lineWidth = CGFloat(CTLineGetTypographicBounds(measureLine, &ascent, &descent, &leading))

        let width = Int(ceil(lineWidth)) + padding * 2
        let height = Int(ceil(ascent + descent + leading)) + padding * 2

        guard width > 0 && height > 0 else {
            return RasterizedText(rgba: [], width: 0, height: 0)
        }

        let bytesPerRow = width * 4
        var pixels = [UInt8](repeating: 0, count: bytesPerRow * height)

        let colorSpace = CGColorSpaceCreateDeviceRGB()
        guard let context = CGContext(
            data: &pixels,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: bytesPerRow,
            space: colorSpace,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return RasterizedText(rgba: [], width: 0, height: 0)
        }

        let drawX = CGFloat(padding)
        let drawY = descent + CGFloat(padding)

        // 1. Drop shadow (black, offset down-right)
        context.saveGState()
        context.setShadow(
            offset: CGSize(width: shadowOffset, height: -shadowOffset),
            blur: shadowBlur,
            color: NSColor.black.withAlphaComponent(0.6).cgColor
        )
        let shadowAttrs: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: NSColor.white,
        ]
        let shadowString = NSAttributedString(string: text, attributes: shadowAttrs)
        let shadowLine = CTLineCreateWithAttributedString(shadowString)
        context.textMatrix = .identity
        context.textPosition = CGPoint(x: drawX, y: drawY)
        CTLineDraw(shadowLine, context)
        context.restoreGState()

        // 2. Black outline (draw stroke only, multiple passes for thickness)
        let outlineAttrs: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: NSColor.clear,
            .strokeColor: NSColor.black,
            .strokeWidth: NSNumber(value: Double(fontSize * 0.15)), // positive = stroke only
        ]
        let outlineString = NSAttributedString(string: text, attributes: outlineAttrs)
        let outlineLine = CTLineCreateWithAttributedString(outlineString)
        context.textMatrix = .identity
        context.textPosition = CGPoint(x: drawX, y: drawY)
        CTLineDraw(outlineLine, context)

        // 3. White fill on top
        let fillAttrs: [NSAttributedString.Key: Any] = [
            .font: font,
            .foregroundColor: NSColor.white,
        ]
        let fillString = NSAttributedString(string: text, attributes: fillAttrs)
        let fillLine = CTLineCreateWithAttributedString(fillString)
        context.textMatrix = .identity
        context.textPosition = CGPoint(x: drawX, y: drawY)
        CTLineDraw(fillLine, context)

        return RasterizedText(rgba: pixels, width: width, height: height)
    }

    static func parseColor(_ hex: String) -> NSColor {
        var hexStr = hex
        if hexStr.hasPrefix("#") {
            hexStr = String(hexStr.dropFirst())
        }

        guard hexStr.count == 6, let rgb = UInt32(hexStr, radix: 16) else {
            return .white
        }

        let r = CGFloat((rgb >> 16) & 0xFF) / 255.0
        let g = CGFloat((rgb >> 8) & 0xFF) / 255.0
        let b = CGFloat(rgb & 0xFF) / 255.0

        return NSColor(red: r, green: g, blue: b, alpha: 1.0)
    }
}
