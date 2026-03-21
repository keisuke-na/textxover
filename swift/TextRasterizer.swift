import AppKit
import CoreText
import WebKit

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

    private static var pollWebView: WKWebView?

    static func rasterizePollGraph(question: String, choices: [PollChoice], scale: CGFloat, completion: @escaping (RasterizedText) -> Void) {
        let fixedWidth = 900
        let rowHeight = 68
        let fixedHeight = 100 + choices.count * rowHeight + 30

        let totalVotes = choices.reduce(0) { $0 + $1.count }
        let barColors = ["#e84560", "#3399dd", "#4cc77a", "#f5a623", "#9c59d9", "#00ccca"]

        var rowsHTML = ""
        for (i, c) in choices.enumerated() {
            let pct = totalVotes > 0 ? Double(c.count) / Double(totalVotes) * 100 : 0
            let color = barColors[i % barColors.count]
            rowsHTML += """
            <div class="row">
              <span class="key">\(escapeHTML(c.key))</span>
              <span class="label">\(escapeHTML(c.label))</span>
              <div class="bar-bg"><div class="bar" style="width:\(pct)%;background:\(color)"></div></div>
              <span class="count">\(c.count) (\(Int(pct))%)</span>
            </div>
            """
        }

        let html = """
        <!DOCTYPE html>
        <html><head><style>
          * { margin:0; padding:0; box-sizing:border-box; }
          body {
            width: \(fixedWidth)px;
            height: \(fixedHeight)px;
            background: rgba(10,10,30,0.88);
            border-radius: 14px;
            padding: 30px 36px;
            font-family: -apple-system, sans-serif;
            color: #fff;
            overflow: hidden;
          }
          .question {
            font-size: 34px;
            font-weight: bold;
            margin-bottom: 22px;
          }
          .row {
            display: flex;
            align-items: center;
            height: \(rowHeight - 12)px;
            margin-bottom: 12px;
            gap: 14px;
          }
          .key {
            width: 44px;
            font-weight: bold;
            color: #f5a623;
            font-size: 28px;
            flex-shrink: 0;
          }
          .label {
            width: 180px;
            font-size: 26px;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
            flex-shrink: 0;
          }
          .bar-bg {
            flex: 1;
            height: 34px;
            background: rgba(255,255,255,0.08);
            border-radius: 6px;
            overflow: hidden;
          }
          .bar {
            height: 100%;
            border-radius: 6px;
            min-width: 2px;
            transition: width 0.3s;
          }
          .count {
            width: 140px;
            text-align: right;
            font-size: 22px;
            color: #aaa;
            font-variant-numeric: tabular-nums;
            flex-shrink: 0;
          }
        </style></head>
        <body>
          <div class="question">\(escapeHTML(question))</div>
          \(rowsHTML)
        </body></html>
        """

        DispatchQueue.main.async {
            let pxWidth = fixedWidth * Int(scale)
            let pxHeight = fixedHeight * Int(scale)

            if pollWebView == nil {
                let config = WKWebViewConfiguration()
                let wv = WKWebView(frame: NSRect(x: 0, y: 0, width: fixedWidth, height: fixedHeight), configuration: config)
                wv.setValue(false, forKey: "drawsBackground")
                pollWebView = wv
            }

            guard let wv = pollWebView else {
                completion(RasterizedText(rgba: [], width: 0, height: 0))
                return
            }
            wv.frame = NSRect(x: 0, y: 0, width: fixedWidth, height: fixedHeight)

            wv.loadHTMLString(html, baseURL: nil)

            // Wait for render
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.15) {
                let snapshotConfig = WKSnapshotConfiguration()
                snapshotConfig.snapshotWidth = NSNumber(value: fixedWidth)

                wv.takeSnapshot(with: snapshotConfig) { image, error in
                    guard let image = image,
                          let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil) else {
                        completion(RasterizedText(rgba: [], width: 0, height: 0))
                        return
                    }

                    // Render to RGBA bitmap at screen scale
                    let w = pxWidth
                    let h = pxHeight
                    let bytesPerRow = w * 4
                    var pixels = [UInt8](repeating: 0, count: bytesPerRow * h)

                    let colorSpace = CGColorSpaceCreateDeviceRGB()
                    guard let ctx = CGContext(
                        data: &pixels,
                        width: w,
                        height: h,
                        bitsPerComponent: 8,
                        bytesPerRow: bytesPerRow,
                        space: colorSpace,
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
                    ) else {
                        completion(RasterizedText(rgba: [], width: 0, height: 0))
                        return
                    }

                    ctx.draw(cgImage, in: CGRect(x: 0, y: 0, width: w, height: h))
                    completion(RasterizedText(rgba: pixels, width: w, height: h))
                }
            }
        }
    }

    private static func escapeHTML(_ s: String) -> String {
        s.replacingOccurrences(of: "&", with: "&amp;")
         .replacingOccurrences(of: "<", with: "&lt;")
         .replacingOccurrences(of: ">", with: "&gt;")
         .replacingOccurrences(of: "\"", with: "&quot;")
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
