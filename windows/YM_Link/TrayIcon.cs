using System.ComponentModel;
using System.Drawing;
using Forms = System.Windows.Forms;

namespace YM_Link;

/// <summary>Latest polled state the tray menu reads when opened; UI-thread only.</summary>
internal sealed class TrayStatusSource
{
    public Snapshot Snapshot { get; private set; } = new();
    public bool BackendReady { get; private set; }

    public void Update(Snapshot snapshot, bool backendReady)
    {
        Snapshot = snapshot;
        BackendReady = backendReady;
    }
}

/// <summary>The Windows tray presence. It renders nothing itself: right-click requests the
/// owner-drawn menu at the cursor, left double-click restores the window, and all labels
/// resolve when App opens the menu.</summary>
internal sealed class TrayIcon : IDisposable
{
    private readonly Container components = new();
    private readonly Icon image;
    private readonly Forms.NotifyIcon notification;
    private bool disposed;

    /// <summary>Right-click anchor in physical screen pixels; test seam. NotifyIcon mouse
    /// events report their hidden message window's client point — (0,0) in practice — so
    /// the menu anchors to the cursor instead.</summary>
    internal static Func<Point> CursorPosition { get; set; } = () => Forms.Control.MousePosition;

    /// <summary>Right-click on the icon; the point is physical screen pixels.</summary>
    internal event Action<Point>? MenuRequested;

    internal TrayIcon(Action restore, Action quit)
    {
        using var stream = typeof(TrayIcon).Assembly.GetManifestResourceStream("YM_Link.TrayIcon.ico")
            ?? throw new InvalidOperationException("Application tray icon is unavailable.");
        using var source = new Icon(stream);
        image = (Icon)source.Clone();
        try
        {
            notification = new Forms.NotifyIcon(components) { Icon = image, Text = "YM_Link" };
            notification.MouseClick += (_, e) =>
            {
                if (e.Button == Forms.MouseButtons.Right) MenuRequested?.Invoke(CursorPosition());
            };
            notification.MouseDoubleClick += (_, e) => { if (e.Button == Forms.MouseButtons.Left) restore(); };
        }
        catch
        {
            components.Dispose();
            image.Dispose();
            throw;
        }
    }

    internal void Show()
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        notification.Visible = true;
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        // Remove the notification before releasing its owned icon.
        notification.Visible = false;
        components.Dispose();
        image.Dispose();
    }
}
