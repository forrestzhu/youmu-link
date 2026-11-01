using System.Reflection;
using YM_Link;
using Forms = System.Windows.Forms;

internal static class TrayIconTests
{
    internal static void Run()
    {
        var restores = 0;
        var exits = 0;
        System.Drawing.Point? requested = null;
        using var tray = new TrayIcon(() => restores++, () => exits++);
        tray.MenuRequested += point => requested = point;
        var notification = (Forms.NotifyIcon)(typeof(TrayIcon)
            .GetField("notification", BindingFlags.Instance | BindingFlags.NonPublic)?.GetValue(tray)
            ?? throw new InvalidOperationException("Missing production tray notification."));
        Require(!notification.Visible, "Tray construction must not publish an icon before the window is ready.");
        var icon = notification.Icon ?? throw new InvalidOperationException("The tray must own the YM_Link icon.");
        Require(icon.Width > 0 && icon.Height > 0, "The embedded tray icon must be usable.");
        Require(notification.ContextMenuStrip is null, "The tray must not build a WinForms menu; the popup is owner drawn.");

        // NotifyIcon event args report (0,0); the anchor must come from the cursor seam.
        TrayIcon.CursorPosition = () => new(55, 66);
        try
        {
            Click(notification, Forms.MouseButtons.Right, 0, 0);
            Require(requested is { } point && point.X == 55 && point.Y == 66, "Right-click must request the menu at the cursor point.");
            Click(notification, Forms.MouseButtons.Left, 0, 0);
            Require(requested?.X == 55, "Left click must not request the menu.");
        }
        finally { TrayIcon.CursorPosition = () => Forms.Control.MousePosition; }

        var doubleClick = typeof(Forms.NotifyIcon).GetMethod("OnMouseDoubleClick", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("Missing NotifyIcon mouse double-click event.");
        doubleClick.Invoke(notification, [new Forms.MouseEventArgs(Forms.MouseButtons.Right, 2, 0, 0, 0)]);
        Require(restores == 0 && exits == 0, "Right double-click must not restore or exit.");
        doubleClick.Invoke(notification, [new Forms.MouseEventArgs(Forms.MouseButtons.Left, 2, 0, 0, 0)]);
        Require(restores == 1 && exits == 0, "Left double-click must restore exactly once without exiting.");
        Require(!notification.Visible, "Configuration changes must keep an unpublished tray icon invisible.");

        tray.Dispose();
        tray.Dispose();
        Require(!notification.Visible && notification.Icon is null, "Disposal must hide and remove the notification icon.");
        var iconDisposed = false;
        try { _ = icon.Handle; }
        catch (ObjectDisposedException) { iconDisposed = true; }
        Require(iconDisposed, "Disposal must release the owned icon handle.");
        var showRejected = false;
        try { tray.Show(); }
        catch (ObjectDisposedException) { showRejected = true; }
        Require(showRejected && !notification.Visible, "A disposed tray must reject Show without publishing an icon.");
        Console.WriteLine("PASS: invisible production tray, right-click menu request at the cursor, left-only double-click restore, idempotent disposal and rejection after close.");
    }

    private static void Click(Forms.NotifyIcon notification, Forms.MouseButtons button, int x, int y)
    {
        var click = typeof(Forms.NotifyIcon).GetMethod("OnMouseClick", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("Missing NotifyIcon mouse click event.");
        click.Invoke(notification, [new Forms.MouseEventArgs(button, 1, x, y, 0)]);
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
