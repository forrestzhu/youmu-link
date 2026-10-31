using System.Reflection;
using YM_Link;
using Forms = System.Windows.Forms;

internal static class TrayIconTests
{
    internal static void Run()
    {
        var restores = 0;
        var exits = 0;
        using var tray = new TrayIcon(new Localizer("en"), () => restores++, () => exits++);
        var notification = (Forms.NotifyIcon)(typeof(TrayIcon)
            .GetField("notification", BindingFlags.Instance | BindingFlags.NonPublic)?.GetValue(tray)
            ?? throw new InvalidOperationException("Missing production tray notification."));
        Require(!notification.Visible, "Tray construction must not publish an icon before the window is ready.");
        var icon = notification.Icon ?? throw new InvalidOperationException("The tray must own the YM_Link icon.");
        Require(icon.Width > 0 && icon.Height > 0, "The embedded tray icon must be usable.");

        var menu = Menu(notification, "Open YM_Link", "Exit");
        menu.Items[0].PerformClick();
        Require(restores == 1 && exits == 0, "The open tray action must restore without exiting.");
        menu.Items[2].PerformClick();
        Require(restores == 1 && exits == 1, "The exit tray action must exit without restoring.");

        var doubleClick = typeof(Forms.NotifyIcon).GetMethod("OnMouseDoubleClick", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("Missing NotifyIcon mouse double-click event.");
        doubleClick.Invoke(notification, [new Forms.MouseEventArgs(Forms.MouseButtons.Right, 2, 0, 0, 0)]);
        Require(restores == 1 && exits == 1, "Right double-click must not restore or exit.");
        doubleClick.Invoke(notification, [new Forms.MouseEventArgs(Forms.MouseButtons.Left, 2, 0, 0, 0)]);
        Require(restores == 2 && exits == 1, "Left double-click must restore exactly once without exiting.");

        tray.UpdateLanguage(new Localizer("zh-Hans"));
        Menu(notification, "打开 YM_Link", "退出");
        tray.UpdateLanguage(new Localizer("en"));
        Menu(notification, "Open YM_Link", "Exit");
        tray.UpdateLanguage(new Localizer("zh-Hans"));
        menu = Menu(notification, "打开 YM_Link", "退出");
        menu.Items[0].PerformClick();
        menu.Items[2].PerformClick();
        Require(restores == 3 && exits == 2, "Language changes must preserve tray actions without duplicating callbacks.");
        Require(!notification.Visible, "Language changes must keep an unpublished tray icon invisible.");

        tray.Dispose();
        tray.Dispose();
        Require(!notification.Visible && notification.Icon is null, "Disposal must hide and remove the notification icon.");
        Require(menu.IsDisposed, "Disposal must release the owned tray menu.");
        var iconDisposed = false;
        try { _ = icon.Handle; }
        catch (ObjectDisposedException) { iconDisposed = true; }
        Require(iconDisposed, "Disposal must release the owned icon handle.");
        var showRejected = false;
        try { tray.Show(); }
        catch (ObjectDisposedException) { showRejected = true; }
        Require(showRejected && !notification.Visible, "A disposed tray must reject Show without publishing an icon.");
        Console.WriteLine("PASS: invisible production tray, bilingual menu actions, left-only double-click restore, idempotent disposal and rejection after close.");
    }

    private static Forms.ContextMenuStrip Menu(Forms.NotifyIcon notification, string open, string exit)
    {
        var menu = notification.ContextMenuStrip ?? throw new InvalidOperationException("Missing tray menu.");
        Require(menu.Items.Count == 3 && menu.Items[0] is Forms.ToolStripMenuItem
            && menu.Items[1] is Forms.ToolStripSeparator && menu.Items[2] is Forms.ToolStripMenuItem,
            "The tray menu must contain open, separator and exit actions.");
        Require(menu.Items[0].Text == open && menu.Items[2].Text == exit, "Tray actions must use the selected language.");
        return menu;
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }
}
