import Foundation

struct Snapshot: Decodable, Equatable {
    var deviceName: String
    var status: String
    var listenPort: UInt16
    var discoveryPort: UInt16
    var peers: [Peer]
    var pairing: Pairing?
    var displays: [Display]
    var mirror: MirrorState
    var shares: [Share]
    var transfers: [Transfer]
    var errorId: UInt64
    var error: String?

    static let empty = Snapshot(
        deviceName: "", status: "starting", listenPort: 0, discoveryPort: 0,
        peers: [], pairing: nil, displays: [], mirror: .empty, shares: [], transfers: [], errorId: 0, error: nil
    )
}

struct SnapshotErrorCursor {
    private var lastId: UInt64 = 0

    mutating func consume(_ snapshot: Snapshot) -> String? {
        guard snapshot.errorId != lastId else { return nil }
        lastId = snapshot.errorId
        return snapshot.error
    }
}

struct Peer: Decodable, Equatable, Identifiable {
    var id: String
    var address: String
    var name: String
    var platform: String
    var status: String
    var trusted: Bool
}

struct Pairing: Decodable, Equatable {
    var peerName: String
    var code: String?
    var expiresIn: UInt64
    var mode: String
    var windowId: String
    var attempt: UInt64
    var stage: String
    var error: String?
    var errorRevision: UInt64
    var retryAfterMs: UInt32

    var canEnter: Bool { mode == "enter" && stage == "ready" && expiresIn > 0 }
    var terminal: Bool { ["expired", "limited", "disconnected"].contains(stage) || expiresIn == 0 }
    var messageKey: String {
        switch error {
        case "incorrect": return "pairing_incorrect"
        case "expired": return "pairing_expired"
        case "rate_limited": return "error_pairing_rate"
        case "version": return "error_version_incompatible"
        default: return "error_connection"
        }
    }
}

/// Logical-window input survives TCP retries; each ready attempt submits once.
struct PairingInput: Equatable {
    private(set) var text = ""
    private(set) var feedback = false
    private(set) var errorVisible = false
    private(set) var rejectionCount = 0
    private(set) var windowId: String?
    private var submittedAttempt: UInt64?
    private var rejectedRevision: UInt64 = 0

    mutating func apply(_ pairing: Pairing?) -> Bool {
        guard let pairing else { self = PairingInput(); return false }
        if windowId != pairing.windowId { self = PairingInput(); windowId = pairing.windowId }
        if pairing.mode == "enter" && pairing.error == "incorrect" && pairing.errorRevision != 0 && pairing.errorRevision != rejectedRevision {
            rejectedRevision = pairing.errorRevision; rejectionCount += 1; feedback = true; errorVisible = true
            return true
        }
        return false
    }
    mutating func edit(_ value: String, pairing: Pairing?) -> String? {
        guard let pairing, pairing.canEnter, !feedback, submittedAttempt != pairing.attempt else { return nil }
        text = String(value.filter { $0.asciiValue.map { (48...57).contains($0) } == true }.prefix(4)); errorVisible = false
        guard text.count == 4 else { return nil }
        submittedAttempt = pairing.attempt
        return text
    }
    mutating func clearRejected(window: String, revision: Int) {
        guard windowId == window && rejectionCount == revision else { return }
        text = ""; feedback = false
    }
}

struct Display: Decodable, Equatable, Identifiable {
    var id: UInt32
    var name: String
    var width: UInt32
    var height: UInt32
}

struct MirrorState: Decodable, Equatable {
    var status: String
    var permission: String
    var displayId: UInt32?
    var audio: Bool
    var quality: String
    var fps: Double?
    var bitrateMbps: Double?
    var latencyMs: Double?
    var error: String?

    var isMirroring: Bool { ["starting", "running"].contains(status) }
    var isActive: Bool { isMirroring || status == "paused" }

    static let empty = MirrorState(
        status: "stopped", permission: "unknown", displayId: nil, audio: false,
        quality: "balanced", fps: nil, bitrateMbps: nil, latencyMs: nil, error: nil
    )
}

struct Share: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
    var path: String
}

struct Transfer: Decodable, Equatable, Identifiable {
    var id: String
    var name: String
    var transferred: UInt64
    var total: UInt64
    var status: String

    var fraction: Double {
        total == 0 ? 0 : min(1, Double(transferred) / Double(total))
    }
}

struct CommandResult: Decodable {
    var ok: Bool
    var error: String?
}

private final class LocalizationBundleAnchor: NSObject {}

enum Localizer {
    static let resourceBundleName = "YM_Link_YM_Link.bundle"
    private static let resources = resourceBundle(relativeTo: .main)
        ?? resourceBundle(relativeTo: Bundle(for: LocalizationBundleAnchor.self))
    private static let languages = resources.map { languageBundles(in: $0) } ?? [:]

    // Do not use SwiftPM's Bundle.module: its generated accessor embeds the
    // build directory as a runtime fallback. Both app and test resources travel
    // next to their host bundle, with no dependency on the original checkout.
    static func resourceBundle(relativeTo host: Bundle) -> Bundle? {
        let roots = [host.resourceURL, host.bundleURL, host.bundleURL.deletingLastPathComponent()]
        for root in roots.compactMap({ $0 }) {
            if let bundle = Bundle(url: root.appendingPathComponent(resourceBundleName)) {
                return bundle
            }
        }
        return nil
    }

    private static func languageBundles(in bundle: Bundle) -> [String: Bundle] {
        guard let root = bundle.resourceURL,
              let folders = try? FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil) else { return [:] }
        var result: [String: Bundle] = [:]
        for folder in folders where folder.pathExtension.lowercased() == "lproj" {
            if let localized = Bundle(url: folder) {
                // SwiftPM lowercases locale identifiers when copying resources.
                result[folder.deletingPathExtension().lastPathComponent.lowercased()] = localized
            }
        }
        return result
    }

    static func backendMessage(_ message: String, language: String) -> String {
        let source = message.lowercased()
        let protocolPrefix = "protocol error: "
        let text = source.hasPrefix(protocolPrefix) ? String(source.dropFirst(protocolPrefix.count)) : source
        let key: String
        switch text {
        case "paired identity key changed": key = "error_identity_changed"
        case "pairing code rejected": key = "error_pairing_rejected"
        case "pairing code expired", "pairing expired": key = "pairing_expired"
        case "explicit pairing required": key = "error_pairing_required"
        case "enter the four digit pairing code": key = "error_pairing_digits"
        case "no pairing input is pending": key = "error_pairing_not_pending"
        case "session authentication failed": key = "error_authentication"
        case "connection timed out": key = "error_connection_timeout"
        case "disconnect the current device first": key = "error_disconnect_first"
        case "select a device or enter its address": key = "error_choose_device"
        case "enter an ipv4 address and port": key = "error_address_invalid"
        case "only lan device addresses are allowed": key = "error_lan_only"
        case "backend busy or unavailable": key = "error_backend_busy"
        case "choose a local folder", "folder path must be valid utf-8": key = "error_choose_folder"
        case "the selected folder is unavailable": key = "error_folder_unavailable"
        case "folder is already shared": key = "error_folder_shared"
        case "too many shared folders": key = "error_share_limit"
        case "shared folder was not found": key = "error_share_not_found"
        case "file access denied": key = "error_file_denied"
        case "file unavailable", "not a regular file", "not a directory": key = "error_file_unavailable"
        case "file changed; restart this read": key = "error_file_changed"
        case "reconnect the device to control this transfer": key = "error_reconnect_transfer"
        case "connect and pair a device before mirroring": key = "error_connect_before_mirror"
        case "mirror is not active": key = "error_mirror_inactive"
        case "start mirroring on the mac before remote resume", "start mirroring on the mac before remote display selection", "first mirror start requires local confirmation": key = "error_mirror_local_start"
        case "mirror command timed out": key = "error_mirror_timeout"
        case "connection no longer active": key = "error_connection"
        case "incompatible application version; upgrade ym_link on both devices": key = "error_version_incompatible"
        case "connection busy; try starting mirror again": key = "error_mirror_busy"
        case "connection too slow for screen mirroring; turn off system audio or reconnect": key = "error_mirror_connection_slow"
        case "selected display is unavailable", "the selected display is no longer available", "unable to list displays": key = "error_display_unavailable"
        case "hardware h.264 encoding is unavailable on this mac": key = "error_hardware_unavailable"
        case "screen capture stopped; check the display and screen recording permission": key = "error_capture_stopped"
        case _ where text.contains("pairing rate limit"): key = "error_pairing_rate"
        case _ where text.contains("keychain"): key = "error_keychain"
        case _ where text.hasPrefix("identity storage error:") || text.contains("shared folder configuration") || text.contains("save shared folders") || text.contains("sync shared folders") || text.contains("application data directory"):
            key = "error_storage"
        case _ where text.contains("screen recording permission"): key = "error_screen_permission"
        case _ where text.contains("hardware encoder") || text.contains("hardware video encoder") || text.contains("hardware encoded frame") || text.contains("encode the captured screen frame"):
            key = "error_encoder"
        case _ where text.contains("system audio"): key = "error_audio"
        case _ where text.contains("screen capture"): key = "error_capture"
        case _ where text.hasPrefix("connection error:") || text == "connection closed" || text == "protocol error: connection closed" || text.contains("lan listener") || text.contains("lan connections") || text.hasPrefix("cannot bind discovery socket:") || text.hasPrefix("socket operation failed:"):
            key = "error_connection"
        case _ where source.hasPrefix("protocol error:"): key = "error_protocol"
        default: key = "command_failed"
        }
        // Unknown backend errors are never echoed: they may contain local paths or transport details.
        return string(key, language: language)
    }

    static func string(_ key: String, language: String) -> String {
        string(key, language: language, bundles: languages, preferredLanguages: Locale.preferredLanguages)
    }

    static func string(_ key: String, language: String, in bundle: Bundle, preferredLanguages: [String] = Locale.preferredLanguages) -> String {
        string(key, language: language, bundles: languageBundles(in: bundle), preferredLanguages: preferredLanguages)
    }

    private static func string(_ key: String, language: String, bundles: [String: Bundle], preferredLanguages: [String]) -> String {
        let selected = language == "system"
            ? Bundle.preferredLocalizations(from: bundles.keys.sorted(), forPreferences: preferredLanguages).first ?? "en"
            : language
        return (bundles[selected.lowercased()] ?? bundles["en"])?.localizedString(forKey: key, value: nil, table: nil) ?? key
    }
}
