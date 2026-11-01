using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Animation;
using System.Windows.Media.Effects;

namespace YM_Link;

/// <summary>
/// The tray right-click menu: an owner-drawn Fluent card per the approved 2026-10-31 demo
/// (220 DIP card, radius 10, 30 DIP rows, hover fill, status header) that mirrors the macOS
/// status item's structure: status header, open, exit. Opens above the tray anchor and
/// dismisses on deactivate, Escape or any press outside its bounds.
/// </summary>
internal sealed class TrayMenuWindow : Window
{
    private const double RowHeight = 30;
    private const double MenuWidth = 220;
    private const double ShellMargin = 16;

    private readonly bool dark, highContrast;
    private readonly Localizer localizer;
    private readonly Brush hover, ink, muted;
    private readonly List<Button> rows = [];
    private int highlight = -1;

    /// <summary>"open" or "exit"; the menu always closes after the action.</summary>
    internal event Action<string>? ActionRequested;

    public TrayMenuWindow(TrayMenuStatus status, bool dark, Localizer localizer)
    {
        this.dark = dark;
        this.localizer = localizer;
        highContrast = SystemParameters.HighContrast;
        if (highContrast)
        {
            // Follow the main window's high-contrast contract: system roles instead of the
            // Fluent palette, monochrome status dots like its overridden SuccessInk.
            hover = SystemColors.ControlDarkBrush;
            ink = SystemColors.WindowTextBrush;
            muted = SystemColors.GrayTextBrush;
        }
        else
        {
            hover = Frozen(Pick(dark, 0x34, 0xEB), Pick(dark, 0x37, 0xED), Pick(dark, 0x3C, 0xF0));
            ink = Frozen(Pick(dark, 0xF2, 0x1B), Pick(dark, 0xF3, 0x1E), Pick(dark, 0xF5, 0x24));
            muted = Frozen(Pick(dark, 0x8D, 0x94), Pick(dark, 0x95, 0x9B), Pick(dark, 0xA2, 0xA7));
        }
        WindowStyle = WindowStyle.None;
        ResizeMode = ResizeMode.NoResize;
        ShowInTaskbar = false;
        Topmost = true;
        ShowActivated = true;
        Focusable = true;
        AllowsTransparency = true;
        Background = Brushes.Transparent;
        FontFamily = new FontFamily("Segoe UI");
        TextOptions.SetTextFormattingMode(this, TextFormattingMode.Display);
        SnapsToDevicePixels = true;
        UseLayoutRounding = true;
        SizeToContent = SizeToContent.Height;
        Width = MenuWidth + ShellMargin * 2;
        Title = "YM_Link";
        Content = BuildShell(status);
        PreviewKeyDown += (_, args) => HandleNavigationKey(args);
        Deactivated += (_, _) => Dispatcher.BeginInvoke(() => { if (IsLoaded) Close(); });
        Loaded += (_, _) =>
        {
            // The foreground lock can deny activation to a tray-clicked menu, and a window
            // that never activated never raises Deactivated — so focus loss alone cannot
            // dismiss it. The hook closes the card on any press outside its bounds.
            if (System.Windows.Application.Current is not null) InstallOutsideClickHook();
            Focus();
        };
        Closed += (_, _) => RemoveOutsideClickHook();
    }

    protected override void OnSourceInitialized(EventArgs e)
    {
        base.OnSourceInitialized(e);
        // A borderless owned-less top-level window would appear as a blank Alt-Tab entry.
        var handle = new WindowInteropHelper(this).Handle;
        if (handle == IntPtr.Zero) return;
        var exStyle = GetWindowLong(handle, -20); // GWL_EXSTYLE
        SetWindowLong(handle, -20, exStyle | 0x80); // WS_EX_TOOLWINDOW
    }

    internal IReadOnlyList<Button> RowsForTest => rows;
    internal int HighlightForTest => highlight;

    /// <summary>Test seam: drives the activation path without showing a real window.</summary>
    internal void ActivateForTest(int index) => Activate(index);

    /// <summary>Places the card above the anchor (both DIP) before the window becomes visible.
    /// The placement contract speaks in visible-card coordinates; the window is the card plus
    /// the transparent shell margin on every side, so the window origin sits one margin up-left.</summary>
    public void OpenAt(Point anchor, Rect workArea)
    {
        if (Content is Border { } shell)
        {
            void Place(double cardHeight)
            {
                (Left, Top) = WindowPosition(anchor, workArea, cardHeight);
            }
            shell.Measure(new Size(MenuWidth, double.PositiveInfinity));
            Place(shell.DesiredSize.Height - ShellMargin * 2);
            SizeChanged += (_, args) => { if (args.HeightChanged && IsLoaded) Place(args.NewSize.Height - ShellMargin * 2); };
        }
        Show();
        // Tray-clicked menus can be denied activation by the foreground lock; without the
        // foreground the OS keyboard never reaches PreviewKeyDown. A denial degrades to
        // mouse-only, which the outside-click hook still dismisses correctly.
        var handle = new WindowInteropHelper(this).Handle;
        if (handle != IntPtr.Zero) SetForegroundWindow(handle);
    }

    internal (double Left, double Top) WindowPosition(Point anchor, Rect workArea, double cardHeight)
    {
        var (left, top) = TrayMenuLogic.Place(anchor, workArea, MenuWidth, cardHeight);
        return (left - ShellMargin, top - ShellMargin);
    }

    private Border BuildShell(TrayMenuStatus status)
    {
        var body = new StackPanel { Margin = new Thickness(0, 6, 0, 6) };
        body.Children.Add(StatusHeader(status));
        body.Children.Add(Separator());
        AddRow(body, "open", localizer["tray_open"]);
        body.Children.Add(Separator());
        AddRow(body, "exit", localizer["tray_exit"]);
        return new Border
        {
            Margin = new Thickness(ShellMargin),
            Background = highContrast ? SystemColors.WindowBrush
                : Frozen(Pick(dark, 0x25, 0xF9), Pick(dark, 0x28, 0xF9), Pick(dark, 0x2D, 0xFA), 0xF0),
            BorderBrush = highContrast ? SystemColors.WindowTextBrush
                : Frozen(Pick(dark, 0x45, 0xDD), Pick(dark, 0x48, 0xE0), Pick(dark, 0x4E, 0xE4)),
            BorderThickness = new Thickness(1),
            CornerRadius = new CornerRadius(10),
            Effect = new DropShadowEffect { BlurRadius = 20, ShadowDepth = 5, Direction = 270, Opacity = 0.3, Color = Colors.Black },
            Child = body,
        };
    }

    private FrameworkElement StatusHeader(TrayMenuStatus status)
    {
        var header = new StackPanel { Margin = new Thickness(0, 0, 0, 8) };
        var line = new StackPanel { Orientation = Orientation.Horizontal, Margin = new Thickness(14, 8, 14, 0) };
        line.Children.Add(StatusDot(status.Dot));
        line.Children.Add(new TextBlock
        {
            Text = status.Title,
            FontSize = 12,
            Foreground = ink,
            VerticalAlignment = VerticalAlignment.Center,
        });
        header.Children.Add(line);
        AutomationProperties.SetName(header, status.Detail.Length > 0 ? status.Title + ", " + status.Detail : status.Title);
        if (status.Detail.Length > 0)
        {
            header.Children.Add(new TextBlock
            {
                Text = status.Detail,
                FontSize = 10.5,
                Foreground = muted,
                Margin = new Thickness(30, 3, 14, 0),
                TextTrimming = TextTrimming.CharacterEllipsis,
            });
        }
        return header;
    }

    /// <summary>The paired-idle ripple mirrors the main window's PairedRipple: an expanding
    /// ring around the green dot, looping while the menu shows the state.</summary>
    private FrameworkElement StatusDot(TrayDot dot)
    {
        var ellipse = Dot(dot);
        if (dot != TrayDot.GreenRipple || highContrast || !SystemParameters.ClientAreaAnimation) return ellipse;
        var ring = new System.Windows.Shapes.Ellipse
        {
            Width = 8,
            Height = 8,
            Stroke = ellipse.Fill,
            StrokeThickness = 1,
            RenderTransformOrigin = new Point(0.5, 0.5),
            Opacity = 0.7,
            IsHitTestVisible = false,
        };
        var scale = new ScaleTransform(1, 1);
        ring.RenderTransform = scale;
        var expand = new DoubleAnimation(1, 2.6, TimeSpan.FromMilliseconds(2000))
        {
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = new CircleEase { EasingMode = EasingMode.EaseOut },
        };
        var fade = new DoubleAnimation(0.7, 0, TimeSpan.FromMilliseconds(2000))
        {
            RepeatBehavior = RepeatBehavior.Forever,
            EasingFunction = new CircleEase { EasingMode = EasingMode.EaseOut },
        };
        scale.BeginAnimation(ScaleTransform.ScaleXProperty, expand);
        scale.BeginAnimation(ScaleTransform.ScaleYProperty, expand);
        ring.BeginAnimation(OpacityProperty, fade);
        var grid = new Grid { Width = 8, Height = 8, VerticalAlignment = VerticalAlignment.Center };
        grid.Children.Add(ring);
        grid.Children.Add(ellipse);
        return grid;
    }

    private System.Windows.Shapes.Ellipse Dot(TrayDot dot) => dot switch
    {
        TrayDot.Green or TrayDot.GreenRipple => ThemeDot(0x24, 0x73, 0x3B, 0x6C, 0xCB, 0x5F),
        TrayDot.Red => ThemeDot(0xC4, 0x2B, 0x1C, 0xFF, 0x99, 0xA4),
        _ => ThemeDot(0x94, 0x9B, 0xA7, 0x8D, 0x95, 0xA2),
    };

    private System.Windows.Shapes.Ellipse ThemeDot(byte lr, byte lg, byte lb, byte dr, byte dg, byte db) => highContrast
        ? new() { Width = 8, Height = 8, VerticalAlignment = VerticalAlignment.Center, Fill = SystemColors.WindowTextBrush }
        : new() { Width = 8, Height = 8, VerticalAlignment = VerticalAlignment.Center, Fill = Frozen(Pick(dark, dr, lr), Pick(dark, dg, lg), Pick(dark, db, lb)) };

    private FrameworkElement Separator() => new Border
    {
        Height = 1,
        Margin = new Thickness(10, 5, 10, 5),
        Background = highContrast ? SystemColors.GrayTextBrush
            : Frozen(Pick(dark, 0x45, 0xDD), Pick(dark, 0x48, 0xDF), Pick(dark, 0x4E, 0xE3)),
    };

    private void AddRow(StackPanel body, string action, string label)
    {
        var row = new Button
        {
            Height = RowHeight,
            Margin = new Thickness(6, 0, 6, 0),
            Background = Brushes.Transparent,
            Cursor = Cursors.Hand,
            Focusable = false,
            Content = new TextBlock
            {
                Text = label,
                FontSize = 12,
                Foreground = ink,
                TextTrimming = TextTrimming.CharacterEllipsis,
            },
            Template = RowTemplate(),
        };
        AutomationProperties.SetAutomationId(row, "tray_menu_" + action);
        AutomationProperties.SetName(row, label);
        row.Click += (_, _) => Activate(rows.IndexOf(row));
        row.MouseEnter += (_, _) => SetHighlight(rows.IndexOf(row));
        row.Tag = action;
        rows.Add(row);
        body.Children.Add(row);
    }

    private ControlTemplate RowTemplate()
    {
        var template = new ControlTemplate(typeof(Button));
        var chrome = new FrameworkElementFactory(typeof(Border), "Chrome");
        chrome.SetValue(Border.CornerRadiusProperty, new CornerRadius(6));
        // Factory-built templates need real bindings: a TemplateBindingExtension handed to
        // SetValue is never evaluated, so keyboard highlight would lose its background too.
        chrome.SetBinding(Border.BackgroundProperty, Templated(Control.BackgroundProperty));
        // The row's TextBlock only renders through a presenter inside the template; the
        // 14/8 inset matches the demo's row padding-left 14 and right 8.
        var presenter = new FrameworkElementFactory(typeof(ContentPresenter));
        presenter.SetBinding(ContentPresenter.ContentProperty, Templated(ContentControl.ContentProperty));
        presenter.SetValue(FrameworkElement.MarginProperty, new Thickness(14, 0, 8, 0));
        presenter.SetValue(FrameworkElement.VerticalAlignmentProperty, VerticalAlignment.Center);
        chrome.AppendChild(presenter);
        template.VisualTree = chrome;
        var hoverTrigger = new Trigger { Property = UIElement.IsMouseOverProperty, Value = true };
        hoverTrigger.Setters.Add(new Setter(Border.BackgroundProperty, hover, "Chrome"));
        template.Triggers.Add(hoverTrigger);
        template.Seal();
        return template;
    }

    private static Binding Templated(DependencyProperty property) => new()
    {
        Path = new PropertyPath(property),
        RelativeSource = new RelativeSource(RelativeSourceMode.TemplatedParent),
    };

    private static Brush Frozen(byte r, byte g, byte b, byte alpha = 0xff)
    {
        var brush = new SolidColorBrush(Color.FromArgb(alpha, r, g, b));
        brush.Freeze();
        return brush;
    }

    private static byte Pick(bool dark, byte darkByte, byte lightByte) => dark ? darkByte : lightByte;

    private void SetHighlight(int index)
    {
        if (index < 0 || index >= rows.Count) return;
        highlight = index;
        for (var i = 0; i < rows.Count; i++) rows[i].Background = i == index ? hover : Brushes.Transparent;
    }

    internal void Activate(int index)
    {
        if (index < 0 || index >= rows.Count) return;
        ActionRequested?.Invoke((string)rows[index].Tag);
        if (IsLoaded) Close();
    }

    internal void HandleNavigationKey(KeyEventArgs args)
    {
        switch (args.Key)
        {
            case Key.Down or Key.Up:
            {
                if (rows.Count == 0) return;
                var direction = args.Key == Key.Down ? 1 : -1;
                SetHighlight((highlight + direction + rows.Count) % rows.Count);
                args.Handled = true;
                return;
            }
            case Key.Enter or Key.Space when highlight >= 0:
                if (!args.IsRepeat) Activate(highlight);
                args.Handled = true;
                return;
            case Key.Escape:
                if (IsLoaded) Close();
                args.Handled = true;
                return;
        }
    }

    private IntPtr mouseHook;
    private LowLevelMouseProc? hookProc;

    private delegate IntPtr LowLevelMouseProc(int code, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")] private static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32.dll", EntryPoint = "GetWindowLong")] private static extern int GetWindowLong(IntPtr window, int index);
    [DllImport("user32.dll", EntryPoint = "SetWindowLong")] private static extern int SetWindowLong(IntPtr window, int index, int value);
    [DllImport("user32.dll")] private static extern IntPtr SetWindowsHookEx(int idHook, LowLevelMouseProc proc, IntPtr module, uint threadId);
    [DllImport("user32.dll")] private static extern bool UnhookWindowsHookEx(IntPtr hook);
    [DllImport("user32.dll")] private static extern IntPtr CallNextHookEx(IntPtr hook, int code, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr window, out NativeRect rectangle);
    [DllImport("kernel32.dll")] private static extern IntPtr GetModuleHandle(string? moduleName);

    [StructLayout(LayoutKind.Sequential)]
    private struct NativeRect { public int Left; public int Top; public int Right; public int Bottom; }

    private void InstallOutsideClickHook()
    {
        hookProc = HookCallback;
        mouseHook = SetWindowsHookEx(14, hookProc, GetModuleHandle(null), 0); // WH_MOUSE_LL
    }

    private void RemoveOutsideClickHook()
    {
        if (mouseHook == IntPtr.Zero) return;
        UnhookWindowsHookEx(mouseHook);
        mouseHook = IntPtr.Zero;
        hookProc = null;
    }

    private IntPtr HookCallback(int code, IntPtr wParam, IntPtr lParam)
    {
        if (code >= 0 && wParam.ToInt64() is 0x201 or 0x204 or 0x207 or 0x20B) // button downs
        {
            var x = Marshal.ReadInt32(lParam);
            var y = Marshal.ReadInt32(lParam, 4);
            var self = new WindowInteropHelper(this).Handle;
            if (self != IntPtr.Zero && GetWindowRect(self, out var bounds)
                && (x < bounds.Left || x > bounds.Right || y < bounds.Top || y > bounds.Bottom))
                Dispatcher.BeginInvoke(() => { if (IsLoaded) Close(); });
        }
        return CallNextHookEx(IntPtr.Zero, code, wParam, lParam);
    }
}
