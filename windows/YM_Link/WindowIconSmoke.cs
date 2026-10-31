using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;

namespace YM_Link;

internal static class WindowIconSmoke
{
    internal static void Verify()
    {
        // Construct and release the shipped tray resources without showing an icon.
        using var tray = new TrayIcon(new Localizer("en"), () => { }, () => { });
        // Run inside the published app so WPF reads its real entry assembly's
        // ApplicationIcon, rather than the test runner's executable icon.
        // EnsureHandle creates a native WPF window without showing it or
        // starting MainWindow's backend/capture lifecycle.
        var window = new Window { ShowInTaskbar = false };
        try
        {
            var handle = new WindowInteropHelper(window).EnsureHandle();
            const uint getIcon = 0x007F; // WM_GETICON
            if (SendMessage(handle, getIcon, new IntPtr(1), IntPtr.Zero) == IntPtr.Zero ||
                SendMessage(handle, getIcon, IntPtr.Zero, IntPtr.Zero) == IntPtr.Zero)
                throw new InvalidOperationException("WPF did not assign the application icon to the native window.");
        }
        finally { window.Close(); }
    }

    [DllImport("user32.dll", EntryPoint = "SendMessageW")]
    private static extern IntPtr SendMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);
}
