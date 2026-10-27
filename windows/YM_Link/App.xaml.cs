using System.IO;
using System.Text.Json;
using System.Security.Principal;
using System.Windows;
using System.Windows.Media;

namespace YM_Link;

public partial class App : Application
{
    private Mutex? instance;
    internal static string StateDirectory => Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Youmu", "YM_Link");

    protected override async void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        if (SystemParameters.HighContrast)
        {
            foreach (var key in new[] { "WindowSurface", "SidebarSurface", "SoftSurface", "ErrorSurface" }) Resources[key] = SystemColors.WindowBrush;
            foreach (var key in new[] { "TextInk", "SecondaryInk", "ErrorInk", "Hairline" }) Resources[key] = SystemColors.WindowTextBrush;
            Resources["Accent"] = SystemColors.HighlightBrush;
            Resources["AccentInk"] = SystemColors.HighlightTextBrush;
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
        window.Show();
    }

    protected override void OnExit(ExitEventArgs e)
    {
        instance?.ReleaseMutex();
        instance?.Dispose();
        base.OnExit(e);
    }

    internal static void SetLanguageResources(Localizer localizer)
    {
        foreach (var key in localizer.Keys) Current.Resources[key] = localizer[key];
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
