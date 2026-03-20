import AppKit
import QuartzCore

class OverlayWindow: NSWindow {
    var metalLayer: CAMetalLayer!

    init(screen: NSScreen) {
        super.init(
            contentRect: screen.frame,
            styleMask: .borderless,
            backing: .buffered,
            defer: false
        )

        // Transparency
        self.isOpaque = false
        self.backgroundColor = .clear
        self.hasShadow = false

        // Always on top (visible in screen sharing)
        self.level = .statusBar

        // Mouse events pass through
        self.ignoresMouseEvents = true

        // Show on all Spaces
        self.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]

        // Set up CAMetalLayer
        metalLayer = CAMetalLayer()
        metalLayer.isOpaque = false
        metalLayer.pixelFormat = .bgra8Unorm
        metalLayer.framebufferOnly = true
        metalLayer.contentsScale = screen.backingScaleFactor

        let contentView = NSView(frame: screen.frame)
        contentView.wantsLayer = true
        contentView.layer = metalLayer
        self.contentView = contentView

        // Size metal layer to screen
        metalLayer.frame = contentView.bounds
        metalLayer.drawableSize = CGSize(
            width: screen.frame.width * screen.backingScaleFactor,
            height: screen.frame.height * screen.backingScaleFactor
        )
    }
}
