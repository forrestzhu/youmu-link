using Microsoft.Win32;

namespace YM_Link;

internal static class SystemTheme
{
    /// <summary>The tray popup follows the Windows personalization "app mode"; WPF exposes no
    /// API for it, so the per-user registry value is the authority. A missing value — slimmed
    /// or server images — stays light, matching the main window's light-first baseline.</summary>
    public static bool IsDark() => IsDark(Registry.CurrentUser);

    public static bool IsDark(RegistryKey root)
    {
        using var key = root.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
        return key?.GetValue("AppsUseLightTheme") is 0;
    }
}
