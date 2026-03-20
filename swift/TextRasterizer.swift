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
        let strokeWidth = fontSize * 0.08
        let shadowOffset: CGFloat = 2.0
        let shadowBlur: CGFloat = 4.0
        let padding = Int(ceil(strokeWidth + shadowBlur + shadowOffset)) + 4

        let lines = text.components(separatedBy: "\n")
        let isMultiline = lines.count > 1

        let measureAttrs: [NSAttributedString.Key: Any] = [.font: font]

        // Measure each line to find max width and total height
        var maxWidth: CGFloat = 0
        var lineHeight: CGFloat = 0
        for line in lines {
            let str = NSAttributedString(string: line, attributes: measureAttrs)
            let ctLine = CTLineCreateWithAttributedString(str)
            var ascent: CGFloat = 0, descent: CGFloat = 0, leading: CGFloat = 0
            let w = CGFloat(CTLineGetTypographicBounds(ctLine, &ascent, &descent, &leading))
            maxWidth = max(maxWidth, w)
            lineHeight = max(lineHeight, ascent + descent + leading)
        }

        let width = Int(ceil(maxWidth)) + padding * 2
        let height = Int(ceil(lineHeight * CGFloat(lines.count))) + padding * 2

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

        // Use monospaced font for multiline (poll) to align columns
        let drawFont = isMultiline ? (NSFont.monospacedSystemFont(ofSize: fontSize, weight: .bold)) : font

        // Draw each line bottom-up (CGContext y=0 is bottom)
        for (i, line) in lines.enumerated() {
            let drawX = CGFloat(padding)
            let drawY = CGFloat(padding) + CGFloat(lines.count - 1 - i) * lineHeight + lineHeight * 0.2

            // Shadow
            context.saveGState()
            context.setShadow(
                offset: CGSize(width: shadowOffset, height: -shadowOffset),
                blur: shadowBlur,
                color: NSColor.black.withAlphaComponent(0.6).cgColor
            )
            let shadowStr = NSAttributedString(string: line, attributes: [.font: drawFont, .foregroundColor: NSColor.white])
            let shadowLine = CTLineCreateWithAttributedString(shadowStr)
            context.textMatrix = .identity
            context.textPosition = CGPoint(x: drawX, y: drawY)
            CTLineDraw(shadowLine, context)
            context.restoreGState()

            // Outline
            let outlineStr = NSAttributedString(string: line, attributes: [
                .font: drawFont,
                .foregroundColor: NSColor.clear,
                .strokeColor: NSColor.black,
                .strokeWidth: NSNumber(value: Double(fontSize * 0.15)),
            ])
            let outlineLine = CTLineCreateWithAttributedString(outlineStr)
            context.textMatrix = .identity
            context.textPosition = CGPoint(x: drawX, y: drawY)
            CTLineDraw(outlineLine, context)

            // Fill
            let fillStr = NSAttributedString(string: line, attributes: [.font: drawFont, .foregroundColor: NSColor.white])
            let fillLine = CTLineCreateWithAttributedString(fillStr)
            context.textMatrix = .identity
            context.textPosition = CGPoint(x: drawX, y: drawY)
            CTLineDraw(fillLine, context)
        }

        return RasterizedText(rgba: pixels, width: width, height: height)
    }

    struct PollChoice {
        let key: String
        let label: String
        let count: Int
    }

    static func rasterizePollGraph(question: String, choices: [PollChoice], scale: CGFloat) -> RasterizedText {
        let padding: CGFloat = 20 * scale
        let fontSize: CGFloat = 18 * scale
        let questionFontSize: CGFloat = 22 * scale
        let barHeight: CGFloat = 28 * scale
        let rowSpacing: CGFloat = 8 * scale
        let barMaxWidth: CGFloat = 300 * scale
        let labelWidth: CGFloat = 120 * scale
        let keyWidth: CGFloat = 30 * scale
        let countWidth: CGFloat = 70 * scale

        let totalVotes = choices.reduce(0) { $0 + $1.count }
        let rowCount = CGFloat(choices.count)
        let contentWidth = keyWidth + labelWidth + barMaxWidth + countWidth + 30 * scale
        let contentHeight = questionFontSize + 16 * scale + rowCount * (barHeight + rowSpacing)

        let width = Int(ceil(contentWidth + padding * 2))
        let height = Int(ceil(contentHeight + padding * 2))

        guard width > 0 && height > 0 else {
            return RasterizedText(rgba: [], width: 0, height: 0)
        }

        let bytesPerRow = width * 4
        var pixels = [UInt8](repeating: 0, count: bytesPerRow * height)

        let colorSpace = CGColorSpaceCreateDeviceRGB()
        guard let ctx = CGContext(
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

        // Background (semi-transparent dark)
        ctx.setFillColor(NSColor(red: 0.05, green: 0.05, blue: 0.15, alpha: 0.85).cgColor)
        ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))

        // Rounded corners
        let bgRect = CGRect(x: 0, y: 0, width: width, height: height)
        let bgPath = CGPath(roundedRect: bgRect, cornerWidth: 12 * scale, cornerHeight: 12 * scale, transform: nil)
        ctx.clear(bgRect)
        ctx.addPath(bgPath)
        ctx.setFillColor(NSColor(red: 0.05, green: 0.05, blue: 0.15, alpha: 0.85).cgColor)
        ctx.fillPath()

        // Question text (top, white)
        let questionFont = NSFont.boldSystemFont(ofSize: questionFontSize)
        let questionAttrs: [NSAttributedString.Key: Any] = [
            .font: questionFont,
            .foregroundColor: NSColor.white,
        ]
        let questionStr = NSAttributedString(string: question, attributes: questionAttrs)
        let questionLine = CTLineCreateWithAttributedString(questionStr)
        ctx.textMatrix = .identity
        // CGContext y is bottom-up
        let questionY = CGFloat(height) - padding - questionFontSize
        ctx.textPosition = CGPoint(x: padding, y: questionY)
        CTLineDraw(questionLine, ctx)

        // Bar colors
        let barColors: [NSColor] = [
            NSColor(red: 0.91, green: 0.27, blue: 0.38, alpha: 1.0), // red
            NSColor(red: 0.20, green: 0.60, blue: 0.86, alpha: 1.0), // blue
            NSColor(red: 0.30, green: 0.78, blue: 0.47, alpha: 1.0), // green
            NSColor(red: 0.96, green: 0.65, blue: 0.14, alpha: 1.0), // orange
            NSColor(red: 0.61, green: 0.35, blue: 0.86, alpha: 1.0), // purple
            NSColor(red: 0.0, green: 0.80, blue: 0.78, alpha: 1.0),  // teal
        ]

        let font = NSFont.boldSystemFont(ofSize: fontSize)
        let startY = questionY - 20 * scale

        for (i, choice) in choices.enumerated() {
            let rowY = startY - CGFloat(i) * (barHeight + rowSpacing)

            // Key (e.g. "A")
            let keyAttrs: [NSAttributedString.Key: Any] = [
                .font: font,
                .foregroundColor: NSColor(red: 0.96, green: 0.65, blue: 0.14, alpha: 1.0),
            ]
            let keyStr = NSAttributedString(string: choice.key, attributes: keyAttrs)
            let keyLine = CTLineCreateWithAttributedString(keyStr)
            ctx.textMatrix = .identity
            ctx.textPosition = CGPoint(x: padding, y: rowY)
            CTLineDraw(keyLine, ctx)

            // Label
            let labelAttrs: [NSAttributedString.Key: Any] = [
                .font: font,
                .foregroundColor: NSColor.white,
            ]
            let labelStr = NSAttributedString(string: choice.label, attributes: labelAttrs)
            let labelLine = CTLineCreateWithAttributedString(labelStr)
            ctx.textPosition = CGPoint(x: padding + keyWidth, y: rowY)
            CTLineDraw(labelLine, ctx)

            // Bar background
            let barX = padding + keyWidth + labelWidth
            let barBgRect = CGRect(x: barX, y: rowY - 2 * scale, width: barMaxWidth, height: barHeight - 4 * scale)
            ctx.setFillColor(NSColor(red: 0.1, green: 0.1, blue: 0.2, alpha: 1.0).cgColor)
            ctx.fill(barBgRect)

            // Bar fill
            let pct = totalVotes > 0 ? CGFloat(choice.count) / CGFloat(totalVotes) : 0
            let barFillWidth = max(barMaxWidth * pct, 2)
            let barFillRect = CGRect(x: barX, y: rowY - 2 * scale, width: barFillWidth, height: barHeight - 4 * scale)
            let colorIdx = i % barColors.count
            ctx.setFillColor(barColors[colorIdx].cgColor)
            ctx.fill(barFillRect)

            // Count text
            let countText = totalVotes > 0 ? "\(choice.count) (\(Int(pct * 100))%)" : "0"
            let countAttrs: [NSAttributedString.Key: Any] = [
                .font: NSFont.monospacedDigitSystemFont(ofSize: fontSize * 0.85, weight: .regular),
                .foregroundColor: NSColor(red: 0.7, green: 0.7, blue: 0.7, alpha: 1.0),
            ]
            let countStr = NSAttributedString(string: countText, attributes: countAttrs)
            let countLine = CTLineCreateWithAttributedString(countStr)
            ctx.textPosition = CGPoint(x: barX + barMaxWidth + 8 * scale, y: rowY)
            CTLineDraw(countLine, ctx)
        }

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
