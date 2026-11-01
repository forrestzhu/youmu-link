using System.Runtime.InteropServices;
using System.Windows;
using Forms = System.Windows.Forms;

namespace YM_Link;

/// <summary>Per-monitor DPI facts for the tray anchor. NotifyIcon reports physical pixels;
/// the popup is positioned in DIP, so each query resolves the monitor under the anchor.</summary>
internal static class TrayScreen
{
    public static double ScaleFor(System.Drawing.Point physical) => Dpi(physical) / 96.0;

    public static Rect WorkAreaFor(System.Drawing.Point physical, double scale)
    {
        var area = Forms.Screen.FromPoint(physical).WorkingArea;
        return new Rect(area.Left / scale, area.Top / scale, area.Width / scale, area.Height / scale);
    }

    private static uint Dpi(System.Drawing.Point physical)
    {
        // MONITOR_DEFAULTTONEAREST, MDT_EFFECTIVE_DPI; DpiForMonitor writes two UINTs.
        // A missing Shcore answer or zero DPI falls back to unscaled.
        if (MonitorFromPoint(new NativePoint(physical.X, physical.Y), 2) is { IsNull: false } monitor
            && DpiForMonitor(monitor, 0, out var dpiX, out _) == 0 && dpiX > 0) return dpiX;
        return 96;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativePoint { public int X; public int Y; public NativePoint(int x, int y) { X = x; Y = y; } }

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeHandle { public IntPtr Value; public bool IsNull => Value == IntPtr.Zero; }

    [DllImport("user32.dll")] private static extern NativeHandle MonitorFromPoint(NativePoint point, uint flags);
    [DllImport("shcore.dll")] private static extern int DpiForMonitor(NativeHandle monitor, int type, out uint dpiX, out uint dpiY);
}
