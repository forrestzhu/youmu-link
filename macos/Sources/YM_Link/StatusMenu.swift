import AppKit

/// What the menu-bar item shows. The mapping mirrors the Windows `TrayMenuLogic.Describe`
/// rules and the main window footer, so both platforms announce the same state from one
/// snapshot; only the presentation adapts (static native menu here, animated card on Windows).
struct TrayMenuStatus: Equatable {
    enum Dot: Equatable {
        case muted, green, red
    }

    var dot: Dot
    var title: String
    var detail: String

    static func showsPaired(_ peer: Peer) -> Bool {
        peer.status == "paired" || (peer.trusted && ["disconnected", "discovered"].contains(peer.status))
    }

    static func deviceName(_ snapshot: Snapshot) -> String? {
        let peer = snapshot.peers.first { $0.status == "connected" }
            ?? snapshot.peers.first(where: showsPaired)
            ?? snapshot.peers.first
        guard let peer, !peer.name.isEmpty else { return nil }
        return peer.name
    }

    static func describe(snapshot: Snapshot, language: String) -> TrayMenuStatus {
        let string = { Localizer.string($0, language: language) }
        let status = { (word: String) -> String in
            let key = "status_" + word
            let value = string(key)
            return value == key ? string("status_unknown") : value
        }
        if snapshot.status == "error" {
            return TrayMenuStatus(dot: .red, title: string("tray_unavailable"), detail: string("tray_unavailable_hint"))
        }
        let mirror = snapshot.mirror
        if mirror.isActive {
            let device = deviceName(snapshot)
            if mirror.status == "paused" {
                return TrayMenuStatus(dot: .green, title: string("tray_mirror_paused"), detail: device ?? "")
            }
            var parts = [String]()
            if let device { parts.append(device) }
            var stats = [String]()
            if let fps = mirror.fps { stats.append(String(format: "%.0f fps", fps)) }
            if let bitrate = mirror.bitrateMbps { stats.append(String(format: "%.1f Mbps", bitrate)) }
            if !stats.isEmpty { parts.append(stats.joined(separator: " · ")) }
            return TrayMenuStatus(dot: .green, title: string("tray_mirror_running"), detail: parts.joined(separator: " · "))
        }
        if snapshot.status == "connected" || snapshot.peers.contains(where: { $0.status == "connected" }) {
            return TrayMenuStatus(dot: .green, title: named(string, status, "tray_connected_format", "connected", snapshot), detail: "")
        }
        if snapshot.peers.contains(where: showsPaired) {
            return TrayMenuStatus(dot: .green, title: named(string, status, "tray_paired_format", "paired", snapshot), detail: "")
        }
        if snapshot.status == "starting" {
            return TrayMenuStatus(dot: .muted, title: status("starting"), detail: "")
        }
        return TrayMenuStatus(dot: .muted, title: string("tray_searching"), detail: "")
    }

    private static func named(
        _ string: (String) -> String,
        _ status: (String) -> String,
        _ formatKey: String,
        _ word: String,
        _ snapshot: Snapshot
    ) -> String {
        guard let device = deviceName(snapshot) else { return status(word) }
        return String(format: string(formatKey), device)
    }
}

/// The menu-bar presence that mirrors the Windows tray icon: a linked-ring template icon,
/// a status header that rebuilds from the latest snapshot on every open, then open and exit.
/// Closing the last window still terminates the app (unchanged platform lifecycle).
@MainActor
final class StatusMenuController: NSObject, NSMenuDelegate {
    private let item: NSStatusItem
    private let menu = NSMenu()
    private weak var model: DesktopModel?
    private let openWindow: () -> Void

    init(model: DesktopModel, openWindow: @escaping () -> Void) {
        item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        self.model = model
        self.openWindow = openWindow
        super.init()
        if let icon = NSImage(systemSymbolName: "link", accessibilityDescription: "YM_Link") {
            icon.isTemplate = true
            item.button?.image = icon
        } else {
            item.button?.title = "YM_Link"
        }
        menu.delegate = self
        menu.autoenablesItems = false
        item.menu = menu
        rebuild()
    }

    func close() { NSStatusBar.system.removeStatusItem(item) }

    func menuNeedsUpdate(_ menu: NSMenu) { rebuild() }

    private func rebuild() {
        menu.removeAllItems()
        guard let model else { return }
        let status = TrayMenuStatus.describe(snapshot: model.snapshot, language: model.language)
        let header = NSAttributedString(emojiIcon: color(status.dot), trailing: status.title, size: 13, color: .labelColor)
        let headerItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        headerItem.attributedTitle = header
        headerItem.isEnabled = false
        menu.addItem(headerItem)
        if !status.detail.isEmpty {
            let detailItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
            detailItem.attributedTitle = NSAttributedString(
                string: status.detail,
                attributes: [.foregroundColor: NSColor.secondaryLabelColor, .font: NSFont.systemFont(ofSize: 11)]
            )
            detailItem.isEnabled = false
            menu.addItem(detailItem)
        }
        menu.addItem(.separator())
        let open = NSMenuItem(title: model.l("tray_open"), action: #selector(openMain), keyEquivalent: "")
        open.target = self
        menu.addItem(open)
        let quit = NSMenuItem(title: model.l("tray_exit"), action: #selector(quit), keyEquivalent: "q")
        quit.target = self
        menu.addItem(quit)
    }

    @objc private func openMain() { openWindow() }

    @objc private func quit() { NSApp.terminate(nil) }

    private func color(_ dot: TrayMenuStatus.Dot) -> NSColor {
        switch dot {
        case .green: return .systemGreen
        case .red: return .systemRed
        case .muted: return .systemGray
        }
    }
}

private extension NSAttributedString {
    /// Status dot + label for disabled menu header rows.
    convenience init(emojiIcon dot: NSColor, trailing title: String, size: CGFloat, color: NSColor) {
        let attributed = NSMutableAttributedString(
            string: "● ",
            attributes: [.foregroundColor: dot, .font: NSFont.systemFont(ofSize: size - 2)]
        )
        attributed.append(NSAttributedString(
            string: title,
            attributes: [.foregroundColor: color, .font: NSFont.systemFont(ofSize: size)]
        ))
        self.init(attributed: attributed)
    }
}
