using System.Windows;

namespace YM_Link;

internal enum TrayDot
{
    Muted,
    Green,
    GreenRipple,
    Red,
}

/// <summary>What the tray menu status header shows; identical semantics on macOS.</summary>
internal sealed record TrayMenuStatus(TrayDot Dot, string Title, string Detail);

/// <summary>Pure mapping from the polled Snapshot to tray menu content, plus the
/// popup placement contract. No WPF state so regressions can drive it directly.</summary>
internal static class TrayMenuLogic
{
    // Same rule as PeerItem.ShowsPairedStatus: a trusted idle peer reads as paired.
    public static bool ShowsPaired(Peer peer) =>
        peer.Status == "paired" || (peer.Trusted && peer.Status is "disconnected" or "discovered");

    /// <summary>The peer whose name the status header names: connected first, then paired, then any.</summary>
    public static string? DeviceName(Snapshot snapshot)
    {
        var peer = snapshot.Peers.FirstOrDefault(value => value.Status == "connected")
            ?? snapshot.Peers.FirstOrDefault(ShowsPaired)
            ?? snapshot.Peers.FirstOrDefault();
        return peer is null || peer.Name.Length == 0 ? null : peer.Name;
    }

    public static TrayMenuStatus Describe(Snapshot snapshot, Localizer localizer)
    {
        if (snapshot.Status == "error")
            return new(TrayDot.Red, localizer["tray_unavailable"], localizer["tray_unavailable_hint"]);
        var mirror = snapshot.Mirror;
        if (mirror.IsActive)
        {
            var device = DeviceName(snapshot);
            if (mirror.Status == "paused")
                return new(TrayDot.Green, localizer["tray_mirror_paused"], device ?? "");
            var detail = string.Join(" · ", new[] { device, MirrorStats(mirror) }.Where(part => !string.IsNullOrEmpty(part)));
            return new(TrayDot.Green, localizer["tray_mirror_running"], detail);
        }
        if (snapshot.Status == "connected" || snapshot.Peers.Any(value => value.Status == "connected"))
            return new(TrayDot.Green, Named(localizer, "tray_connected_format", "connected", snapshot), "");
        if (snapshot.Peers.Any(ShowsPaired))
            return new(TrayDot.GreenRipple, Named(localizer, "tray_paired_format", "paired", snapshot), "");
        if (snapshot.Status == "starting") return new(TrayDot.Muted, localizer.Status("starting"), "");
        return new(TrayDot.Muted, localizer["tray_searching"], "");
    }

    /// <summary>Backend still booting has no snapshot yet; the menu can still be opened.</summary>
    public static TrayMenuStatus Starting(Localizer localizer) => new(TrayDot.Muted, localizer.Status("starting"), "");

    /// <summary>Matches the main window footer: whole fps and one bitrate decimal.</summary>
    public static string MirrorStats(Mirror mirror)
    {
        var parts = new List<string>();
        if (mirror.Fps is { } fps) parts.Add($"{fps:0} fps");
        if (mirror.BitrateMbps is { } bitrate) parts.Add($"{bitrate:0.0} Mbps");
        return string.Join(" · ", parts);
    }

    private static string Named(Localizer localizer, string formatKey, string status, Snapshot snapshot)
    {
        var device = DeviceName(snapshot);
        return device is null ? localizer.Status(status) : string.Format(localizer[formatKey], device);
    }

    /// <summary>Popup placement in DIP: the visible card's right edge lands 12 DIP left of the
    /// anchor, its bottom at most 6 DIP above the anchor and never below 8 DIP above the work
    /// area; the card keeps 4 DIP inside the left edge even on narrower-than-card work areas.</summary>
    public static (double Left, double Top) Place(Point anchor, Rect workArea, double width, double height)
    {
        var left = Math.Max(workArea.Left + 4, Math.Min(anchor.X - 12 - width, workArea.Right - width - 4));
        var top = Math.Max(workArea.Top + 4, Math.Min(anchor.Y - 6, workArea.Bottom - 8) - height);
        return (left, top);
    }
}
