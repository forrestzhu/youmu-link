using System.Globalization;
using System.Reflection;
using System.Text.Json;

namespace YM_Link;

internal sealed class Localizer
{
    private readonly Dictionary<string, string> strings;
    private static readonly Dictionary<string, string> English = Load("en");
    public IEnumerable<string> Keys => strings.Keys;
    public string this[string key] => strings.GetValueOrDefault(key) ?? English.GetValueOrDefault(key) ?? key;

    public Localizer(string language, CultureInfo? systemCulture = null)
    {
        var selected = language == "system" ? (systemCulture ?? CultureInfo.CurrentUICulture).Name : language;
        strings = selected.StartsWith("zh", StringComparison.OrdinalIgnoreCase) ? Load("zh-Hans") : English;
    }

    private static Dictionary<string, string> Load(string language)
    {
        var assembly = Assembly.GetExecutingAssembly();
        var name = assembly.GetManifestResourceNames().Single(value => value.EndsWith($"Resources.{language}.json", StringComparison.Ordinal));
        using var stream = assembly.GetManifestResourceStream(name)!;
        return JsonSerializer.Deserialize<Dictionary<string, string>>(stream)!;
    }

    public string Status(string status) => this[English.ContainsKey("status_" + status) ? "status_" + status : "status_unknown"];
    public string DisplayName(Display display) => display.Name == "Built-in display" ? this["built_in_display"]
        : display.Name == $"Display {display.Id}" ? $"{this["display"]} {display.Id}" : display.Name;

    public string BackendMessage(string message)
    {
        var source = message.ToLowerInvariant();
        var text = source.StartsWith("protocol error: ", StringComparison.Ordinal) ? source[16..] : source;
        var key = text switch
        {
            "paired identity key changed" => "error_identity_changed",
            "pairing code rejected" => "error_pairing_rejected",
            "pairing code expired" or "pairing expired" => "pairing_expired",
            "explicit pairing required" => "error_pairing_required",
            "enter the four digit pairing code" => "error_pairing_digits",
            "no pairing input is pending" => "error_pairing_not_pending",
            "session authentication failed" => "error_authentication",
            "connection timed out" => "error_connection_timeout",
            "disconnect the current device first" => "error_disconnect_first",
            "select a device or enter its address" => "error_choose_device",
            "enter an ipv4 address and port" => "error_address_invalid",
            "only lan device addresses are allowed" => "error_lan_only",
            "backend busy or unavailable" => "error_backend_busy",
            "choose a local folder" or "folder path must be valid utf-8" => "error_choose_folder",
            "the selected folder is unavailable" => "error_folder_unavailable",
            "folder is already shared" => "error_folder_shared",
            "too many shared folders" => "error_share_limit",
            "shared folder was not found" => "error_share_not_found",
            "file access denied" => "error_file_denied",
            "file unavailable" or "not a regular file" or "not a directory" => "error_file_unavailable",
            "file changed; restart this read" => "error_file_changed",
            "reconnect the device to control this transfer" => "error_reconnect_transfer",
            "connect and pair a device before mirroring" => "error_connect_before_mirror",
            "mirror is not active" => "error_mirror_inactive",
            "first mirror start requires local confirmation" => "error_mirror_local_start",
            "start mirroring on the mac before remote resume" or "start mirroring on the mac before remote display selection" => "error_mirror_local_start",
            "mirror command timed out" => "error_mirror_timeout",
            "connection no longer active" => "error_connection",
            "incompatible application version; upgrade ym_link on both devices" => "error_version_incompatible",
            "connection busy; try starting mirror again" => "error_mirror_busy",
            "connection too slow for screen mirroring; turn off system audio or reconnect" => "error_mirror_connection_slow",
            "selected display is unavailable" or "the selected display is no longer available" or "unable to list displays" => "error_display_unavailable",
            "display size changed; start screen mirroring again" => "error_display_changed",
            "windows h.264 encoding is unavailable; install media feature pack on windows n" or "media foundation is unavailable" => "error_media_feature",
            _ when text.Contains("pairing rate limit") => "error_pairing_rate",
            _ when text.Contains("keychain") || text.Contains("credential") || text.Contains("dpapi") => "error_keychain",
            _ when text.StartsWith("identity storage error:") || text.Contains("shared folder configuration") || text.Contains("save shared folders") || text.Contains("sync shared folders") || text.Contains("application data directory") || text.Contains("secure trust directory") => "error_storage",
            _ when text.Contains("screen recording permission") || text.Contains("screen capture permission") => "error_screen_permission",
            _ when text.Contains("h.264") || text.Contains("encoder") || text.Contains("video format") || text.Contains("video size") || text.Contains("video frame") || text.Contains("video sample") || text.Contains("video buffer") || text.Contains("video timestamp") || text.Contains("video duration") || text.Contains("video color") || text.Contains("video nominal range") || text.Contains("progressive video") || text.Contains("pixel aspect") || text.Contains("mirror quality") || text.Contains("mirror size limit") || text.Contains("encode the captured screen frame") => "error_encoder",
            _ when text.Contains("system audio") => "error_audio",
            _ when text.Contains("screen") || text.Contains("capture") || text.Contains("direct3d") || text.Contains("media clock") || text.Contains("media worker") || text.Contains("mirror output queue") || text.Contains("display dimensions") || text.Contains("selected display") || text.Contains("windows display callback") => "error_capture",
            _ when text.StartsWith("connection error:") || text == "connection closed" || text.Contains("lan listener") || text.Contains("lan connections") || text.StartsWith("cannot bind discovery socket:") || text.StartsWith("socket operation failed:") => "error_connection",
            _ when source.StartsWith("protocol error:") => "error_protocol",
            _ => "command_failed"
        };
        // Unknown native errors may contain paths. Only localized, fixed text reaches the interface.
        return this[key];
    }
}
