using System.ComponentModel;
using System.Drawing;
using Forms = System.Windows.Forms;

namespace YM_Link;

internal sealed class TrayIcon : IDisposable
{
    private readonly Container components = new();
    private readonly Icon image;
    private readonly Forms.NotifyIcon notification;
    private readonly Forms.ToolStripMenuItem open;
    private readonly Forms.ToolStripMenuItem exit;
    private bool disposed;

    internal TrayIcon(Localizer localizer, Action restore, Action quit)
    {
        using var stream = typeof(TrayIcon).Assembly.GetManifestResourceStream("YM_Link.TrayIcon.ico")
            ?? throw new InvalidOperationException("Application tray icon is unavailable.");
        using var source = new Icon(stream);
        image = (Icon)source.Clone();
        try
        {
            var menu = new Forms.ContextMenuStrip(components);
            open = new Forms.ToolStripMenuItem();
            exit = new Forms.ToolStripMenuItem();
            menu.Items.AddRange([open, new Forms.ToolStripSeparator(), exit]);
            open.Click += (_, _) => restore();
            exit.Click += (_, _) => quit();
            notification = new Forms.NotifyIcon(components) { Icon = image, Text = "YM_Link", ContextMenuStrip = menu };
            notification.MouseDoubleClick += (_, e) => { if (e.Button == Forms.MouseButtons.Left) restore(); };
            UpdateLanguage(localizer);
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

    internal void UpdateLanguage(Localizer localizer)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        open.Text = localizer["tray_open"];
        exit.Text = localizer["tray_exit"];
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        // Remove the notification before releasing its menu and owned icon.
        notification.Visible = false;
        components.Dispose();
        image.Dispose();
    }
}
