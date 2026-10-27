import Foundation

struct Snapshot: Decodable {
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
    var error: String?

    static let empty = Snapshot(
        deviceName: "", status: "starting", listenPort: 0, discoveryPort: 0,
        peers: [], pairing: nil, displays: [], mirror: .empty, shares: [], transfers: [], error: nil
    )
}

struct Peer: Decodable, Identifiable {
    var id: String
    var address: String
    var name: String
    var platform: String
    var status: String
    var trusted: Bool
}

struct Pairing: Decodable {
    var peerName: String
    var code: String?
    var expiresIn: UInt64
    var mode: String
}

struct Display: Decodable, Identifiable {
    var id: UInt32
    var name: String
    var width: UInt32
    var height: UInt32
}

struct MirrorState: Decodable {
    var status: String
    var permission: String
    var displayId: UInt32?
    var audio: Bool
    var quality: String
    var fps: Double?
    var bitrateMbps: Double?
    var latencyMs: Double?
    var error: String?

    static let empty = MirrorState(
        status: "stopped", permission: "unknown", displayId: nil, audio: false,
        quality: "balanced", fps: nil, bitrateMbps: nil, latencyMs: nil, error: nil
    )
}

struct Share: Decodable, Identifiable {
    var id: String
    var name: String
    var path: String
}

struct Transfer: Decodable, Identifiable {
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

enum Localizer {
    static func backendMessage(_ message: String, language: String) -> String {
        let text = message.lowercased()
        let key: String
        switch text {
        case "paired identity key changed": key = "error_identity_changed"
        case "pairing code rejected": key = "error_pairing_rejected"
        case "pairing code expired", "pairing expired": key = "pairing_expired"
        case "explicit pairing required": key = "error_pairing_required"
        case "enter the six digit pairing code": key = "error_pairing_digits"
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
        case _ where text.hasPrefix("protocol error:"): key = "error_protocol"
        default: key = "command_failed"
        }
        // Unknown backend errors are never echoed: they may contain local paths or transport details.
        return string(key, language: language)
    }

    static func string(_ key: String, language: String) -> String {
        if language != "system", let path = Bundle.module.path(forResource: language, ofType: "lproj"),
           let bundle = Bundle(path: path) {
            return bundle.localizedString(forKey: key, value: nil, table: nil)
        }
        return Bundle.module.localizedString(forKey: key, value: nil, table: nil)
    }
}
