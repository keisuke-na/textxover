import AppKit

class DisplaySelector {
    static func availableScreens() -> [NSScreen] {
        return NSScreen.screens
    }

    static func screenNames() -> [String] {
        return NSScreen.screens.enumerated().map { index, screen in
            let name = screen.localizedName
            let size = screen.frame.size
            return "\(index): \(name) (\(Int(size.width))x\(Int(size.height)))"
        }
    }

    static func screen(at index: Int) -> NSScreen? {
        let screens = NSScreen.screens
        guard index >= 0 && index < screens.count else { return nil }
        return screens[index]
    }
}
