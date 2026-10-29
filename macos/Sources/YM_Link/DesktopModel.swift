import AppKit
import CYMLink
import Foundation

// All C calls are serialized off the main thread; only decoded snapshots reach SwiftUI.
private final class Backend: @unchecked Sendable {
    private var handle: UnsafeMutableRawPointer?

    func start(directory: String, name: String) -> Bool {
        handle = directory.withCString { path in
            name.withCString { ymlink_create(path, $0) }
        }
        return handle != nil
    }

    func snapshot() -> String? {
        guard let handle, let pointer = ymlink_snapshot(handle) else { return nil }
        defer { ymlink_string_free(pointer) }
        return String(cString: pointer)
    }

    func command(_ json: String) -> String? {
        guard let handle else { return nil }
        return json.withCString {
            guard let pointer = ymlink_command(handle, $0) else { return nil }
            defer { ymlink_string_free(pointer) }
            return String(cString: pointer)
        }
    }

    func stop() {
        if let handle { ymlink_destroy(handle) }
        handle = nil
    }
}

@MainActor
final class DesktopModel: ObservableObject {
    @Published private(set) var snapshot = Snapshot.empty
    @Published var error: String?
    @Published var language: String = UserDefaults.standard.string(forKey: "interfaceLanguage") ?? "system" {
        didSet { UserDefaults.standard.set(language, forKey: "interfaceLanguage") }
    }
    @Published var showAddress = false
    @Published var addressDraft = ""
    @Published private(set) var pairingInput = PairingInput()
    @Published private(set) var pendingActions: Set<String> = []

    private let queue = DispatchQueue(label: "app.youmu.link.backend", qos: .userInitiated)
    private nonisolated let backend = Backend()
    private var timer: DispatchSourceTimer?
    private var errorCursor = SnapshotErrorCursor()
    private var didReportReadError = false
    private var clearPairingTask: Task<Void, Never>?

    init() {
        let directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("YM_Link", isDirectory: true).path
        let name = Host.current().localizedName ?? "Mac"
        let backend = self.backend
        queue.async { [weak self] in
            guard backend.start(directory: directory, name: name) else {
                DispatchQueue.main.async {
                    self?.snapshot.status = "error"
                    self?.error = self?.l("backend_start_failed")
                }
                return
            }
            self?.readSnapshot()
        }
        let timer = DispatchSource.makeTimerSource(queue: queue)
        timer.schedule(deadline: .now() + .milliseconds(500), repeating: .milliseconds(500))
        timer.setEventHandler { [weak self] in self?.readSnapshot() }
        timer.resume()
        self.timer = timer
    }

    func l(_ key: String) -> String { Localizer.string(key, language: language) }
    func displayName(_ display: Display) -> String {
        if display.name == "Built-in display" { return l("built_in_display") }
        if display.name == "Display \(display.id)" { return "\(l("display")) \(display.id)" }
        return display.name
    }

    func backendMessage(_ message: String) -> String { Localizer.backendMessage(message, language: language) }

    func status(_ value: String) -> String {
        let key = "status_" + value
        let localized = l(key)
        return localized == key ? l("status_unknown") : localized
    }

    var isConnected: Bool { snapshot.status == "connected" }
    var isMirroring: Bool { snapshot.mirror.isMirroring }
    var isMirrorActive: Bool { snapshot.mirror.isActive }

    func editPairing(_ value: String) {
        if let pairing = snapshot.pairing, let code = pairingInput.edit(value, pairing: pairing) { command("pair", ["code": code, "window_id": pairing.windowId, "attempt": pairing.attempt]) }
    }

    func command(_ action: String, _ arguments: [String: Any] = [:]) {
        guard !pendingActions.contains(action) else { return }
        var value = arguments
        value["action"] = action
        guard let data = try? JSONSerialization.data(withJSONObject: value),
              let json = String(data: data, encoding: .utf8) else { return }
        pendingActions.insert(action)
        let backend = self.backend
        queue.async { [weak self] in
            defer { DispatchQueue.main.async { self?.pendingActions.remove(action) } }
            guard let response = backend.command(json), let data = response.data(using: .utf8) else {
                DispatchQueue.main.async { self?.error = self?.l("backend_unavailable") }
                return
            }
            do {
                let result = try JSONDecoder().decode(CommandResult.self, from: data)
                if !result.ok {
                    DispatchQueue.main.async {
                        guard let self else { return }
                        self.error = result.error.map { self.backendMessage($0) } ?? self.l("command_failed")
                    }
                }
            } catch {
                DispatchQueue.main.async { self?.error = self?.l("backend_response_failed") }
            }
            self?.readSnapshot()
        }
    }

    private nonisolated func readSnapshot() {
        // Called only from the serial backend queue.
        guard let json = backend.snapshot(), let data = json.data(using: .utf8) else { return }
        do {
            let decoder = JSONDecoder()
            decoder.keyDecodingStrategy = .convertFromSnakeCase
            let value = try decoder.decode(Snapshot.self, from: data)
            DispatchQueue.main.async { [weak self] in
                guard let self else { return }
                if pairingInput.apply(value.pairing), let window = pairingInput.windowId {
                    clearPairingTask?.cancel()
                    let revision = pairingInput.rejectionCount
                    clearPairingTask = Task { [weak self] in
                        do { try await Task.sleep(for: .milliseconds(520)) } catch { return }
                        self?.pairingInput.clearRejected(window: window, revision: revision)
                    }
                } else if value.pairing == nil { clearPairingTask?.cancel() }
                snapshot = value
                if let message = errorCursor.consume(value) {
                    error = backendMessage(message)
                }
                didReportReadError = false
            }
        } catch {
            DispatchQueue.main.async { [weak self] in
                guard let self, !didReportReadError else { return }
                self.error = l("backend_response_failed")
                didReportReadError = true
            }
        }
    }

    func chooseShare() {
        let panel = NSOpenPanel()
        panel.title = l("add_share")
        panel.prompt = l("authorize_folder")
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.allowsMultipleSelection = false
        panel.canCreateDirectories = false
        panel.begin { [weak self] response in
            guard response == .OK, let url = panel.url else { return }
            self?.command("add_share", ["path": url.path])
        }
    }

    func connectAddress() {
        let address = addressDraft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !address.isEmpty else { return }
        command("connect", ["address": address])
        showAddress = false
    }

    func openScreenSettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") else { return }
        NSWorkspace.shared.open(url)
    }

    func shutdown() {
        timer?.cancel()
        timer = nil
        queue.sync { backend.stop() }
    }
}
