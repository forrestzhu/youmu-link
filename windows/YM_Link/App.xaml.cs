using System.IO;
using System.Text.Json;
using System.Security.Principal;
using System.Windows;
using System.Windows.Media;

namespace YM_Link;

public partial class App : Application
{
    private Mutex? instance;
    private TrayIcon? tray;
    private TrayMenuWindow? trayMenu;
    private MainWindow? mainWindow;
    private readonly TrayStatusSource trayStatus = new();
    private Localizer localizer = new("en");
    private long reopenSuppressedUntil;
    internal static string StateDirectory => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Youmu", "YM_Link");

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        if (SystemParameters.HighContrast)
        {
            foreach (var key in new[] { "WindowSurface", "SidebarSurface", "SoftSurface", "ControlSurface", "HoverSurface", "SelectedSurface", "AccentSoft", "ErrorSurface" }) Resources[key] = SystemColors.WindowBrush;
            foreach (var key in new[] { "TextInk", "SecondaryInk", "ErrorInk", "SuccessInk", "Hairline", "ControlBorder" }) Resources[key] = SystemColors.WindowTextBrush;
            foreach (var key in new[] { "Accent", "AccentHover", "AccentPressed" }) Resources[key] = SystemColors.HighlightBrush;
            Resources["AccentInk"] = SystemColors.HighlightTextBrush;
            foreach (var key in new[] { "SelectionSurface", "HighlightSurface" }) Resources[key] = SystemColors.HighlightBrush;
            foreach (var key in new[] { "SelectionInk", "HighlightInk" }) Resources[key] = SystemColors.HighlightTextBrush;
        }
        var localizer = new Localizer(UserSettings.ReadLanguage());
        SetLanguageResources(localizer);
        var identity = WindowsIdentity.GetCurrent().User?.Value ?? Environment.UserName;
        // LocalAppData and Credential Manager are shared by this SID across
        // console / RDP sessions, so the single-instance gate must be as well.
        instance = new Mutex(true, @"Global\Youmu.YM_Link." + identity, out var firstInstance);
        if (!firstInstance)
        {
            instance.Dispose();
            instance = null;
            if (!e.Args.Contains("--smoke-test", StringComparer.Ordinal)) MessageBox.Show(localizer["already_running"], "YM_Link", MessageBoxButton.OK, MessageBoxImage.Information);
            Shutdown(4);
            return;
        }
        if (e.Args.Contains("--smoke-test", StringComparer.Ordinal))
        {
            // Exercise the real backend without opening capture or inventing a device.
            // A hung native startup/teardown cannot leave an unattended smoke process.
            using var watchdog = new Timer(_ => Environment.Exit(5), null, TimeSpan.FromSeconds(20), Timeout.InfiniteTimeSpan);
            var result = 1;
            try
            {
                WindowIconSmoke.Verify();
                await using var backend = new NativeBackend();
                if (await backend.StartAsync(StateDirectory, Environment.MachineName))
                {
                    Snapshot snapshot = new();
                    for (var attempt = 0; attempt < 30; attempt++)
                    {
                        snapshot = await backend.SnapshotAsync();
                        if (snapshot.ListenPort != 0 && snapshot.DiscoveryPort != 0 && snapshot.Status != "starting") break;
                        await Task.Delay(100);
                    }
                    result = snapshot.ListenPort != 0 && snapshot.DiscoveryPort != 0 && snapshot.Status != "error" ? 0 : 2;
                }
            }
            catch { result = 3; }
            Shutdown(result);
            return;
        }
        var window = new MainWindow(localizer);
        MainWindow = window;
        mainWindow = window;
        window.TrayStatus = trayStatus;
        try
        {
            tray = new TrayIcon(window.RestoreFromTray, window.ExitApplication);
            tray.MenuRequested += OpenTrayMenu;
            tray.Show();
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or ArgumentException
            or InvalidOperationException or System.Runtime.InteropServices.ExternalException)
        {
            tray?.Dispose();
            tray = null;
            // Keep a usable exit path when the shell cannot host a tray icon.
            window.MinimizeToTray = false;
        }
        window.Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        try
        {
            trayMenu?.Close();
            tray?.Dispose();
        }
        finally
        {
            instance?.ReleaseMutex();
            instance?.Dispose();
            base.OnExit(e);
        }
    }

    /// <summary>Opens the owner-drawn menu above the tray anchor; a second right-click on the
    /// icon closes it, and the inevitable click that lands just after a dismissal cannot
    /// reopen it. The anchor arrives in physical pixels and the card is placed in DIP.</summary>
    private void OpenTrayMenu(System.Drawing.Point physical)
    {
        if (Environment.TickCount64 < reopenSuppressedUntil) return;
        if (trayMenu is { } open)
        {
            open.Close();
            trayMenu = null;
            reopenSuppressedUntil = Environment.TickCount64 + 350;
            return;
        }
        var scale = TrayScreen.ScaleFor(physical);
        var anchor = new Point(physical.X / scale, physical.Y / scale);
        var workArea = TrayScreen.WorkAreaFor(physical, scale);
        var status = trayStatus.BackendReady
            ? TrayMenuLogic.Describe(trayStatus.Snapshot, localizer)
            : TrayMenuLogic.Starting(localizer);
        var menu = new TrayMenuWindow(status, SystemTheme.IsDark(), localizer);
        var actionTaken = false;
        menu.ActionRequested += action =>
        {
            actionTaken = true;
            if (action == "open") mainWindow?.RestoreFromTray();
            else mainWindow?.ExitApplication();
        };
        menu.Closed += (_, _) =>
        {
            if (ReferenceEquals(trayMenu, menu)) trayMenu = null;
            // Outside presses, Escape and focus loss all close without an action; the
            // icon's click for this gesture may still be in flight, so hold reopening briefly.
            if (!actionTaken) reopenSuppressedUntil = Environment.TickCount64 + 350;
        };
        trayMenu = menu;
        menu.OpenAt(anchor, workArea);
    }

    internal static void SetLanguageResources(Localizer localizer)
    {
        foreach (var key in localizer.Keys) Current.Resources[key] = localizer[key];
        if (Current is App app) app.localizer = localizer;
    }
}

internal static class UserSettings
{
    private static string FileName => Path.Combine(App.StateDirectory, "ui-settings.json");
    internal static string ReadLanguage()
    {
        try
        {
            var settings = JsonSerializer.Deserialize<Dictionary<string, string>>(File.ReadAllText(FileName));
            return settings?.GetValueOrDefault("language") is "en" or "zh-Hans" ? settings["language"] : "system";
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException or JsonException) { return "system"; }
    }
    internal static void SaveLanguage(string language)
    {
        Directory.CreateDirectory(App.StateDirectory);
        var temporary = FileName + ".tmp";
        File.WriteAllText(temporary, JsonSerializer.Serialize(new { language }));
        File.Move(temporary, FileName, true);
    }
}
