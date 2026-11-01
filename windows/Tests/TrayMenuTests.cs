using System.Runtime.ExceptionServices;
using System.Runtime.InteropServices;
using System.Windows.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Automation;
using System.Windows.Input;
using System.Windows.Interop;
using YM_Link;

internal static class TrayMenuTests
{
    internal static void Run()
    {
        DescribeMappings();
        Placement();
        ThemeRegistry();
        ExceptionDispatchInfo? failure = null;
        var thread = new Thread(() =>
        {
            try { MenuWindow(); }
            catch (Exception error) { failure = ExceptionDispatchInfo.Capture(error); }
            finally { Dispatcher.CurrentDispatcher.InvokeShutdown(); }
        }) { IsBackground = true };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        if (!thread.Join(TimeSpan.FromSeconds(30))) throw new TimeoutException("Tray menu regression timed out.");
        failure?.Throw();
        Console.WriteLine("PASS: tray status mapping, popup placement bounds, owner-drawn menu rows, keyboard navigation and actions.");
    }

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static void DescribeMappings()
    {
        var en = new Localizer("en");
        var zh = new Localizer("zh-Hans");
        var connected = new Peer { Id = "p1", Name = "Quest 3S", Status = "connected", Trusted = true };
        var pairedIdle = new Peer { Id = "p1", Name = "Quest 3S", Status = "disconnected", Trusted = true };
        Mirror Mirror(string status, double? fps = null, double? bitrate = null) => new() { Status = status, Fps = fps, BitrateMbps = bitrate };
        Snapshot Snapshot(string status, List<Peer>? peers = null, Mirror? mirror = null) => new()
        {
            Status = status,
            Peers = peers ?? [],
            Mirror = mirror ?? new Mirror(),
        };

        var cases = new (Snapshot Snapshot, TrayDot Dot, string En, string Zh, string EnDetail, string ZhDetail)[]
        {
            (Snapshot("starting"), TrayDot.Muted, "Starting", "正在启动", "", ""),
            (Snapshot("ready"), TrayDot.Muted, "Searching for Quest…", "正在搜索 Quest…", "", ""),
            (Snapshot("searching"), TrayDot.Muted, "Searching for Quest…", "正在搜索 Quest…", "", ""),
            (Snapshot("error"), TrayDot.Red, "Status unavailable", "状态待确认", "Open the main window for details", "打开主窗口查看详情"),
            (Snapshot("connected", [connected]), TrayDot.Green, "Connected · Quest 3S", "已连接 · Quest 3S", "", ""),
            (Snapshot("ready", [pairedIdle]), TrayDot.GreenRipple, "Paired · Quest 3S", "已配对 · Quest 3S", "", ""),
            (Snapshot("connected", [connected], Mirror("running", 60, 12.36)), TrayDot.Green, "Mirroring", "镜像运行中", "Quest 3S · 60 fps · 12.4 Mbps", "Quest 3S · 60 fps · 12.4 Mbps"),
            (Snapshot("connected", [connected], Mirror("paused")), TrayDot.Green, "Mirror paused", "镜像已暂停", "Quest 3S", "Quest 3S"),
            (Snapshot("ready", [pairedIdle], Mirror("running", 30)), TrayDot.Green, "Mirroring", "镜像运行中", "Quest 3S · 30 fps", "Quest 3S · 30 fps"),
        };
        foreach (var (snapshot, dot, english, chinese, enDetail, zhDetail) in cases)
        {
            var enStatus = TrayMenuLogic.Describe(snapshot, en);
            var zhStatus = TrayMenuLogic.Describe(snapshot, zh);
            Require(enStatus.Dot == dot && zhStatus.Dot == dot, $"Tray dot for {snapshot.Status} must be {dot}.");
            Require(enStatus.Title == english, $"Tray title expected '{english}', got '{enStatus.Title}'.");
            Require(zhStatus.Title == chinese, $"Tray title expected '{chinese}', got '{zhStatus.Title}'.");
            Require(enStatus.Detail == enDetail && zhStatus.Detail == zhDetail, $"Tray detail expected '{enDetail}', got '{enStatus.Detail}'.");
        }
        // A connected backend without a readable name keeps the plain status line.
        var unnamed = TrayMenuLogic.Describe(Snapshot("connected"), en);
        Require(unnamed.Title == "Connected" && unnamed.Detail == "", "Unnamed connections keep the plain connected title.");
        Require(TrayMenuLogic.Starting(en).Title == "Starting", "Backend boot keeps the starting header.");
        // Discovery without trust must not read as paired.
        var stranger = new Peer { Id = "p2", Name = "Other", Status = "discovered", Trusted = false };
        var discovery = TrayMenuLogic.Describe(Snapshot("ready", [stranger]), en);
        Require(discovery.Dot == TrayDot.Muted && discovery.Title == "Searching for Quest…", "Discovered-only devices stay in search.");
        // The main-window footer arithmetic stays mirrored by the tray stats.
        Require(TrayMenuLogic.MirrorStats(new Mirror { Fps = 59.6, BitrateMbps = 8.05, LatencyMs = 33 }) == "60 fps · 8.1 Mbps",
            "Mirror stats format must match the footer.");
        Require(TrayMenuLogic.MirrorStats(new Mirror()) == "", "Idle mirrors have no stats.");
    }

    private static void Placement()
    {
        var workArea = new Rect(0, 0, 1920, 1040);
        var (left, top) = TrayMenuLogic.Place(new Point(742, 499), workArea, 220, 126);
        Require(left == 510 && top == 367, $"Placement follows the demo anchor: expected (510, 367), got ({left}, {top}).");
        (left, top) = TrayMenuLogic.Place(new Point(4, 500), workArea, 220, 126);
        Require(left == 4, "A left-edge anchor clamps to the 4 DIP margin.");
        (left, top) = TrayMenuLogic.Place(new Point(1900, 500), workArea, 220, 126);
        Require(left == 1668, "A right-edge anchor clamps inside the work area.");
        (left, top) = TrayMenuLogic.Place(new Point(1000, 1039), workArea, 220, 126);
        Require(top == 906, "Near the work-area bottom the card stops 8 DIP above it.");
        (left, top) = TrayMenuLogic.Place(new Point(1000, 400), new Rect(0, 0, 220, 40), 220, 126);
        Require(left == 4 && top == 4, "A tiny work area pins the card to its top-left margin.");
        (left, top) = TrayMenuLogic.Place(new Point(1000, 2000), workArea, 220, 126);
        Require(top == 906, "An anchor below the taskbar cannot push the card into it.");
        // A left-of-primary monitor keeps the same clamp semantics in negative space.
        (left, top) = TrayMenuLogic.Place(new Point(-1000, 500), new Rect(-1920, 0, 1920, 1040), 220, 126);
        Require(left == -1232 && top == 368, $"Negative-origin work areas clamp identically: expected (-1232, 368), got ({left}, {top}).");
    }

    private static void ThemeRegistry()
    {
        using var root = Microsoft.Win32.Registry.CurrentUser.CreateSubKey(@"Software\YM_Link_ThemeTests");
        try
        {
            Require(!SystemTheme.IsDark(root), "A missing personalization value stays light.");
            using (var key = root.CreateSubKey(@"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"))
            {
                key.SetValue("AppsUseLightTheme", 0, Microsoft.Win32.RegistryValueKind.DWord);
                Require(SystemTheme.IsDark(root), "AppsUseLightTheme=0 reads dark.");
                key.SetValue("AppsUseLightTheme", 1, Microsoft.Win32.RegistryValueKind.DWord);
                Require(!SystemTheme.IsDark(root), "AppsUseLightTheme=1 reads light.");
            }
            Require(!SystemTheme.IsDark(root), "A removed personalization value falls back to light.");
        }
        finally { Microsoft.Win32.Registry.CurrentUser.DeleteSubKeyTree(@"Software\YM_Link_ThemeTests", false); }
    }

    private static void MenuWindow()
    {
        var en = new Localizer("en");
        var status = new TrayMenuStatus(TrayDot.Green, "Connected · Quest 3S", "");
        var menu = new TrayMenuWindow(status, dark: false, en);
        var rows = menu.RowsForTest;

        // The window adds one transparent shell margin per side; the placement contract
        // speaks in visible-card coordinates, so the origin sits one margin up-left.
        var (windowLeft, windowTop) = menu.WindowPosition(new Point(742, 499), new Rect(0, 0, 1920, 1040), 126);
        Require(windowLeft == 494 && windowTop == 351, $"Window origin subtracts the shell margin: expected (494, 351), got ({windowLeft}, {windowTop}).");
        Require(windowLeft + 16 + 220 == 742 - 12, "The visible card's right edge stays 12 DIP left of the anchor.");
        Require(rows.Count == 2, "The menu shows exactly the open and exit rows.");
        Require(AutomationProperties.GetAutomationId(rows[0]) == "tray_menu_open"
            && AutomationProperties.GetAutomationId(rows[1]) == "tray_menu_exit", "Menu rows carry stable automation ids.");
        Require(rows[0].Content is TextBlock { Text: "Open YM_Link" }, "The open row uses the selected language.");
        // Content renders only through a presenter inside the row template; realize layout
        // and walk the visual tree so a template regression cannot hide behind Content-only
        // assertions. ContentPresenter materializes its content child during measure.
        rows[0].ApplyTemplate();
        rows[0].Measure(new Size(208, 30));
        Require(FindVisualText(rows[0]) == "Open YM_Link", "The open row must realize its label in the visual tree.");
        var requested = new List<string>();
        menu.ActionRequested += requested.Add;

        menu.ActivateForTest(1);
        Require(requested is ["exit"], "Activating the second row requests exit.");
        menu.ActivateForTest(-1);
        menu.ActivateForTest(7);
        Require(requested.Count == 1, "Out-of-range activations do nothing.");

        SendKey(menu, Key.Down);
        Require(menu.HighlightForTest == 0, "Arrow down highlights the first row.");
        SendKey(menu, Key.Down);
        Require(menu.HighlightForTest == 1, "Arrow down moves to the exit row.");
        SendKey(menu, Key.Down);
        Require(menu.HighlightForTest == 0, "Arrow down wraps to the open row.");
        SendKey(menu, Key.Up);
        Require(menu.HighlightForTest == 1, "Arrow up wraps backwards.");
        SendKey(menu, Key.Enter);
        Require(requested is ["exit", "exit"], "Enter activates the highlighted row.");

        var chinese = new TrayMenuWindow(status, dark: true, new Localizer("zh-Hans"));
        Require(chinese.RowsForTest[0].Content is TextBlock { Text: "打开 YM_Link" }, "Chinese open row.");
        Require(chinese.RowsForTest[1].Content is TextBlock { Text: "退出" }, "Chinese exit row.");

        // The dark palette must actually reach the row highlight, not just exist. A
        // high-contrast host correctly swaps in ControlDark, so pin the demo color only there.
        SendKey(chinese, Key.Down);
        var highlightBrush = chinese.RowsForTest[0].Background as System.Windows.Media.SolidColorBrush;
        if (!SystemParameters.HighContrast)
            Require(highlightBrush is { } && highlightBrush.Color == System.Windows.Media.Color.FromRgb(0x34, 0x37, 0x3C),
                $"Dark hover must use the demo highlight #34373C, got {highlightBrush?.Color}.");
        else
            Require(ReferenceEquals(highlightBrush, SystemColors.ControlDarkBrush), "High-contrast hover must use the system role.");
        SendKey(chinese, Key.Up);

        // The paired ripple realizes a ring + dot grid when motion is allowed, else a plain dot.
        var paired = new TrayMenuWindow(new TrayMenuStatus(TrayDot.GreenRipple, "Paired · Quest 3S", ""), false, en);
        var header = paired.StatusHeaderForTest;
        header.ApplyTemplate();
        var dots = CountVisuals<System.Windows.Shapes.Ellipse>(header, _ => true);
        Require(dots >= 1, "The paired header always shows a status dot.");
        if (SystemParameters.ClientAreaAnimation)
        {
            var ring = FindVisual<System.Windows.Shapes.Ellipse>(header, e => e.Stroke is not null);
            Require(ring is not null, "With motion enabled the ripple ring must exist.");
        }

        var detailed = new TrayMenuWindow(new TrayMenuStatus(TrayDot.Red, "Status unavailable", "Open the main window for details"), false, en);
        Require(detailed.RowsForTest.Count == 2, "The error status keeps both actions.");
    }

    private static T? FindVisual<T>(System.Windows.DependencyObject node, Func<T, bool> match) where T : System.Windows.DependencyObject
    {
        if (node is T hit && match(hit)) return hit;
        var children = System.Windows.Media.VisualTreeHelper.GetChildrenCount(node);
        for (var i = 0; i < children; i++)
        {
            if (FindVisual(System.Windows.Media.VisualTreeHelper.GetChild(node, i), match) is { } found) return found;
        }
        return null;
    }

    private static int CountVisuals<T>(System.Windows.DependencyObject node, Func<T, bool> match) where T : System.Windows.DependencyObject
    {
        var count = node is T hit && match(hit) ? 1 : 0;
        var children = System.Windows.Media.VisualTreeHelper.GetChildrenCount(node);
        for (var i = 0; i < children; i++)
            count += CountVisuals(System.Windows.Media.VisualTreeHelper.GetChild(node, i), match);
        return count;
    }

    private static void SendKey(TrayMenuWindow menu, Key key)
    {
        var source = new HwndSource(0, 0, 0, 0, 0, "", IntPtr.Zero);
        var args = new KeyEventArgs(Keyboard.PrimaryDevice, source, 0, key) { RoutedEvent = Keyboard.PreviewKeyDownEvent };
        menu.HandleNavigationKey(args);
        Require(args.Handled, key + " must be handled by the tray menu.");
        source.Dispose();
    }

    private static string? FindVisualText(System.Windows.DependencyObject node)
    {
        if (node is TextBlock { Text: { } text }) return text;
        var children = System.Windows.Media.VisualTreeHelper.GetChildrenCount(node);
        for (var i = 0; i < children; i++)
        {
            if (FindVisualText(System.Windows.Media.VisualTreeHelper.GetChild(node, i)) is { } match) return match;
        }
        return null;
    }
}
