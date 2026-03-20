import AppKit
import QuartzCore

class AppDelegate: NSObject, NSApplicationDelegate {
    var statusItem: NSStatusItem!
    var overlayWindow: OverlayWindow!
    var rustHandle: UnsafeMutableRawPointer?
    var displayLink: CVDisplayLink?
    var serverPort: UInt16 = 8080
    var screenScale: CGFloat = 2.0
    var currentSpeed: Float = 1.0
    var currentOpacity: Float = 1.0
    var speedLabel: NSTextField?
    var opacityLabel: NSTextField?

    func applicationDidFinishLaunching(_ notification: Notification) {
        setupMenuBar()

        guard let screen = NSScreen.main else {
            NSLog("No main screen found")
            NSApplication.shared.terminate(nil)
            return
        }

        screenScale = screen.backingScaleFactor
        overlayWindow = OverlayWindow(screen: screen)
        overlayWindow.orderFrontRegardless()

        let width = UInt32(screen.frame.width * screenScale)
        let height = UInt32(screen.frame.height * screenScale)

        let layerPtr = Unmanaged.passUnretained(overlayWindow.metalLayer).toOpaque()
        rustHandle = txo_init(layerPtr, width, height)

        guard rustHandle != nil else {
            NSLog("Failed to initialize Rust renderer")
            NSApplication.shared.terminate(nil)
            return
        }

        txo_start_server(rustHandle, serverPort)
        startDisplayLink()

        NSLog("textxover started on port \(serverPort)")
    }

    func applicationWillTerminate(_ notification: Notification) {
        stopDisplayLink()
        if let handle = rustHandle {
            txo_destroy(handle)
            rustHandle = nil
        }
    }

    // MARK: - Comment Processing

    func processComments() {
        guard let handle = rustHandle else { return }

        var pending = TxoPendingComment()

        while txo_poll_comment(handle, &pending) != 0 {
            // Read text
            let text: String
            if let ptr = pending.text, pending.text_len > 0 {
                text = String(cString: ptr)
            } else {
                continue
            }

            // Determine font size
            let config = getConfig()
            let fontSize: CGFloat
            switch pending.size {
            case 1: fontSize = CGFloat(config.fontSizeBig) * screenScale
            case 2: fontSize = CGFloat(config.fontSizeSmall) * screenScale
            default: fontSize = CGFloat(config.fontSizeMedium) * screenScale
            }

            // Parse color
            let colorHex = String(format: "#%06X", pending.color)
            let color = TextRasterizer.parseColor(colorHex)

            // Rasterize text
            let rasterized = TextRasterizer.rasterize(text: text, color: color, fontSize: fontSize)

            guard rasterized.width > 0 && rasterized.height > 0 else { continue }

            // Submit texture to Rust
            rasterized.rgba.withUnsafeBufferPointer { buffer in
                guard let baseAddress = buffer.baseAddress else { return }
                txo_submit_texture(
                    handle,
                    pending.comment_id,
                    UInt32(rasterized.width),
                    UInt32(rasterized.height),
                    baseAddress,
                    UInt32(rasterized.rgba.count)
                )
            }

            // Calculate Y position using comment type
            // For now, pass -1 to let Rust assign the lane
            txo_start_comment(handle, pending.comment_id, pending.comment_type, -1.0)
        }
    }

    private struct SimpleConfig {
        let fontSizeBig: Int
        let fontSizeMedium: Int
        let fontSizeSmall: Int
    }

    private func getConfig() -> SimpleConfig {
        // Default sizes; will be updated when config API is connected
        return SimpleConfig(fontSizeBig: 48, fontSizeMedium: 36, fontSizeSmall: 24)
    }

    // MARK: - Menu Bar

    private func setupMenuBar() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)

        if let button = statusItem.button {
            button.title = "TX"
        }

        let menu = NSMenu()

        let portItem = NSMenuItem(title: "Server: localhost:\(serverPort)", action: nil, keyEquivalent: "")
        portItem.isEnabled = false
        menu.addItem(portItem)

        menu.addItem(NSMenuItem.separator())

        let displayMenu = NSMenu()
        for name in DisplaySelector.screenNames() {
            displayMenu.addItem(NSMenuItem(title: name, action: #selector(selectDisplay(_:)), keyEquivalent: ""))
        }
        let displayItem = NSMenuItem(title: "Display", action: nil, keyEquivalent: "")
        displayItem.submenu = displayMenu
        menu.addItem(displayItem)

        menu.addItem(NSMenuItem.separator())

        // Speed slider
        let speedView = NSView(frame: NSRect(x: 0, y: 0, width: 250, height: 30))
        let speedTitleLabel = NSTextField(labelWithString: "Speed")
        speedTitleLabel.frame = NSRect(x: 14, y: 5, width: 45, height: 20)
        speedTitleLabel.font = NSFont.systemFont(ofSize: 12)
        speedView.addSubview(speedTitleLabel)

        let speedSlider = NSSlider(value: 1.0, minValue: 0.2, maxValue: 5.0, target: self, action: #selector(speedChanged(_:)))
        speedSlider.frame = NSRect(x: 60, y: 5, width: 140, height: 20)
        speedView.addSubview(speedSlider)

        let sLabel = NSTextField(labelWithString: "x1.0")
        sLabel.frame = NSRect(x: 205, y: 5, width: 40, height: 20)
        sLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        speedView.addSubview(sLabel)
        speedLabel = sLabel

        let speedMenuItem = NSMenuItem()
        speedMenuItem.view = speedView
        menu.addItem(speedMenuItem)

        // Opacity slider
        let opacityView = NSView(frame: NSRect(x: 0, y: 0, width: 250, height: 30))
        let opacityTitleLabel = NSTextField(labelWithString: "Opacity")
        opacityTitleLabel.frame = NSRect(x: 14, y: 5, width: 50, height: 20)
        opacityTitleLabel.font = NSFont.systemFont(ofSize: 12)
        opacityView.addSubview(opacityTitleLabel)

        let opacitySlider = NSSlider(value: 1.0, minValue: 0.1, maxValue: 1.0, target: self, action: #selector(opacityChanged(_:)))
        opacitySlider.frame = NSRect(x: 65, y: 5, width: 135, height: 20)
        opacityView.addSubview(opacitySlider)

        let oLabel = NSTextField(labelWithString: "100%")
        oLabel.frame = NSRect(x: 205, y: 5, width: 40, height: 20)
        oLabel.font = NSFont.monospacedDigitSystemFont(ofSize: 11, weight: .regular)
        opacityView.addSubview(oLabel)
        opacityLabel = oLabel

        let opacityMenuItem = NSMenuItem()
        opacityMenuItem.view = opacityView
        menu.addItem(opacityMenuItem)

        menu.addItem(NSMenuItem.separator())

        menu.addItem(NSMenuItem(title: "Quit", action: #selector(quit), keyEquivalent: "q"))

        statusItem.menu = menu
    }

    @objc private func selectDisplay(_ sender: NSMenuItem) {
        guard let menu = sender.menu else { return }
        let index = menu.index(of: sender)
        guard let screen = DisplaySelector.screen(at: index) else { return }

        overlayWindow.setFrame(screen.frame, display: true)
        overlayWindow.metalLayer.contentsScale = screen.backingScaleFactor
        screenScale = screen.backingScaleFactor

        let width = UInt32(screen.frame.width * screenScale)
        let height = UInt32(screen.frame.height * screenScale)

        overlayWindow.metalLayer.drawableSize = CGSize(width: CGFloat(width), height: CGFloat(height))

        if let handle = rustHandle {
            txo_resize(handle, width, height)
        }
    }

    @objc private func speedChanged(_ sender: NSSlider) {
        currentSpeed = Float(sender.doubleValue)
        speedLabel?.stringValue = String(format: "x%.1f", currentSpeed)
        if let handle = rustHandle {
            txo_update_config(handle, currentSpeed, currentOpacity)
        }
    }

    @objc private func opacityChanged(_ sender: NSSlider) {
        currentOpacity = Float(sender.doubleValue)
        opacityLabel?.stringValue = String(format: "%d%%", Int(currentOpacity * 100))
        if let handle = rustHandle {
            txo_update_config(handle, currentSpeed, currentOpacity)
        }
    }

    @objc private func quit() {
        NSApplication.shared.terminate(nil)
    }

    // MARK: - Display Link

    private func startDisplayLink() {
        CVDisplayLinkCreateWithActiveCGDisplays(&displayLink)

        guard let displayLink = displayLink else { return }

        let opaqueHandle = Unmanaged.passUnretained(self).toOpaque()

        CVDisplayLinkSetOutputCallback(displayLink, { (_, _, _, _, _, userInfo) -> CVReturn in
            guard let userInfo = userInfo else { return kCVReturnError }
            let appDelegate = Unmanaged<AppDelegate>.fromOpaque(userInfo).takeUnretainedValue()

            // Process pending comments (rasterize + submit)
            appDelegate.processComments()

            // Render frame
            if let handle = appDelegate.rustHandle {
                txo_render_frame(handle)
            }
            return kCVReturnSuccess
        }, opaqueHandle)

        CVDisplayLinkStart(displayLink)
    }

    private func stopDisplayLink() {
        if let displayLink = displayLink {
            CVDisplayLinkStop(displayLink)
        }
        displayLink = nil
    }
}
