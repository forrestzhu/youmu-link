using System.IO;
using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows;
using System.Windows.Automation;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Markup;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using System.Xml.Linq;
using YM_Link;

internal static class TemplateBindingTests
{
    internal static void Run()
    {
        ExceptionDispatchInfo? failure = null;
        var thread = new Thread(() =>
        {
            try { MaterializeRows(); RenderWorkspaces(); }
            catch (Exception error) { failure = ExceptionDispatchInfo.Capture(error); }
            finally { Dispatcher.CurrentDispatcher.InvokeShutdown(); }
        }) { IsBackground = true };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        if (!thread.Join(TimeSpan.FromSeconds(60))) throw new TimeoutException("WPF template regression timed out.");
        failure?.Throw();
        Console.WriteLine("PASS: production WPF row bindings and bilingual content viewport layouts, workspace selection, control templates, dropdown selection and recovery states.");
    }

    private static XDocument ReadXaml(string name)
    {
        using var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("YM_Link.Tests." + name)!;
        return XDocument.Load(stream);
    }

    private static void MaterializeRows()
    {
        XNamespace wpf = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
        XNamespace x = "http://schemas.microsoft.com/winfx/2006/xaml";
        var window = ReadXaml("MainWindow.xaml");
        var app = ReadXaml("App.xaml");
        var localizer = new Localizer("en");
        var failures = new List<Exception>();
        foreach (var name in new[] { "ShareList", "TransferList" })
        {
            try
            {
                var element = window.Descendants().Single(value => (string?)value.Attribute(x + "Name") == name);
                var markup = new XElement(element.Descendants(wpf + "DataTemplate").Single());
                markup.SetAttributeValue(XNamespace.Xmlns + "x", x.NamespaceName);
                // Loose XAML cannot connect code-behind events; all real bindings and styles stay intact.
                StripCodeBehind(markup);
                markup.SetAttributeValue(x + "Key", "TestRow");
                var dictionary = new XElement(wpf + "ResourceDictionary", app.Root!.Element(wpf + "Application.Resources")!.Elements().Select(value => new XElement(value)));
                dictionary.SetAttributeValue(XNamespace.Xmlns + "x", x.NamespaceName);
                dictionary.Add(markup);
                var resources = (ResourceDictionary)XamlReader.Parse(dictionary.ToString());
                var template = (DataTemplate)resources["TestRow"];
                foreach (var key in localizer.Keys) resources[key] = localizer[key];
                object row = name == "ShareList"
                    ? new ShareItem(new Share { Id = "test", Name = "Films", Path = @"\\?\C:\Films" }, localizer)
                    : new TransferItem(new Transfer { Id = "test", Name = "Film", Transferred = 25, Total = 100, Status = "transferring" }, localizer);
                var host = new ContentControl { Content = row, ContentTemplate = template, Resources = resources };
                host.ApplyTemplate();
                host.Measure(new Size(700, 500));
                host.Arrange(new Rect(0, 0, 700, 500));
                host.UpdateLayout();
                Dispatcher.CurrentDispatcher.Invoke(DispatcherPriority.ApplicationIdle, new Action(() => { }));
                if (name == "ShareList" && Find<TextBox>(host)?.Text != @"C:\Films") throw new InvalidOperationException("Readable share path was not rendered.");
                if (name == "TransferList" && Find<ProgressBar>(host)?.Value != 0.25) throw new InvalidOperationException("Transfer progress was not rendered.");
                if (row is ShareItem share) share.Update(share.Data with { Path = @"\\?\UNC\server\media" }, localizer);
                if (row is TransferItem transfer) transfer.Update(transfer.Data with { Transferred = 75 }, localizer);
                Dispatcher.CurrentDispatcher.Invoke(DispatcherPriority.ApplicationIdle, new Action(() => { }));
                if (name == "ShareList" && Find<TextBox>(host)?.Text != @"\\server\media") throw new InvalidOperationException("Readable share path snapshot update was not rendered.");
                if (name == "TransferList" && Find<ProgressBar>(host)?.Value != 0.75) throw new InvalidOperationException("Transfer progress snapshot update was not rendered.");
                host.Content = null;
            }
            catch (Exception error) { failures.Add(new InvalidOperationException(name + " template failed.", error)); }
        }
        if (failures.Count != 0) throw new AggregateException(failures);
    }

    // This parses the shipped XAML and styles, rather than maintaining a parallel mockup.
    // Removing the class/events prevents MainWindow construction, state access, networking and capture.
    private static void StripCodeBehind(XElement markup)
    {
        XNamespace x = "http://schemas.microsoft.com/winfx/2006/xaml";
        string[] events = ["Click", "Checked", "Closing", "Loaded", "SelectionChanged", "SizeChanged", "KeyDown", "TextChanged"];
        foreach (var attribute in markup.DescendantsAndSelf().Attributes().Where(attribute =>
            attribute.Name == x + "Class" || (attribute.Name.Namespace == XNamespace.None && events.Contains(attribute.Name.LocalName))).ToArray())
            attribute.Remove();
    }

    private static void RenderWorkspaces()
    {
        var english = new Localizer("en");
        foreach (var status in new[] { "connecting", "pairing", "connected", "paired", "discovered", "disconnected" })
        {
            var peer = new PeerItem(new Peer { Id = "fixture-peer", Status = status }, english);
            var canDisconnect = status is "connecting" or "pairing" or "connected" or "paired";
            Require(peer.CanDisconnect == canDisconnect && peer.ActionText == english[canDisconnect ? "disconnect" : "connect"], "Inline peer action must retain cancellation while connecting or pairing.");
        }
        foreach (var locale in new[] { "en", "zh-Hans" })
        foreach (var size in new[] { new Size(1024, 720), new Size(860, 580) })
        {
            var localizer = new Localizer(locale);
            var window = LoadPreview(localizer, size);
            var prefix = $"{locale}-{(int)size.Width}x{(int)size.Height}";
            try
            {
                PopulatePreview(window, localizer);
                MainWindow.ApplyWorkspaceSelection(window);
                window.Show();
                FlushLayout(window);
                var tabs = Need<FrameworkElement>(window, "WorkspaceTabs");
                MainWindow.ApplyHeaderLayout(((FrameworkElement)window.Content).ActualWidth, tabs, Need<FrameworkElement>(window, "PageHeading"));
                MainWindow.ApplyDisplayLayout(Need<ListBox>(window, "DisplayChoice"));
                FlushLayout(window);
                VerifyKeyboardContracts(window);

                SelectWorkspace(window, localizer, 0);
                var peerButton = Find<Button>(Need<ListBox>(window, "PeerList"));
                Require(peerButton?.Content?.ToString() == localizer["disconnect"] && peerButton.IsEnabled, "Connected peer action binding did not retain its disconnect control.");
                var peerContainer = (ListBoxItem)Need<ListBox>(window, "PeerList").ItemContainerGenerator.ContainerFromIndex(0);
                Require(peerContainer.ContextMenu is not null && peerContainer.Focusable, "Device rows must retain their native keyboard context menu.");
                var displays = Need<ListBox>(window, "DisplayChoice");
                Require(displays.Items.Count == 2, "The two-display fixture was lost.");
                foreach (var item in displays.Items)
                {
                    var card = (ListBoxItem)displays.ItemContainerGenerator.ContainerFromItem(item);
                    Require(card.ActualWidth >= 170 && card.ActualHeight >= 140, "Display card content was squeezed.");
                    RequireInside(card, displays, "Display card");
                }
                Require(displays.SelectedIndex == 0, "Initial display selection was lost.");
                displays.SelectedIndex = 1;
                FlushLayout(window);
                Require(((ListBoxItem)displays.ItemContainerGenerator.ContainerFromIndex(1)).IsSelected, "Display card selection did not update.");
                SavePreview(window, prefix + "-mirror");
                VerifyDropdown(window);

                SelectWorkspace(window, localizer, 1);
                var shares = Need<ListBox>(window, "ShareList");
                var path = Find<TextBox>(shares) ?? throw new InvalidOperationException("Shared path text was not materialized.");
                Require(path.Text.StartsWith(@"C:\Cinema archive\", StringComparison.Ordinal) && !path.Text.StartsWith(@"\\?\", StringComparison.Ordinal), "The actual share row did not display its normalized path.");
                Require(path.IsReadOnly && path.TextWrapping == TextWrapping.Wrap && path.ActualHeight > 30, "Long shared paths must remain selectable and wrap.");
                RequireInside(path, shares, "Shared path");
                Require(path.Template.FindName("PART_ContentHost", path) is FrameworkElement, "TextBox lost its native content host.");
                SavePreview(window, prefix + "-shares");

                SelectWorkspace(window, localizer, 2);
                var transfers = Need<ItemsControl>(window, "TransferList");
                var progress = Find<ProgressBar>(transfers) ?? throw new InvalidOperationException("Transfer progress was not materialized.");
                Require(progress.Value == 0.375 && progress.ActualWidth > 160 && progress.ActualHeight >= 4, "Transfer progress binding or layout regressed.");
                RequireInside(progress, transfers, "Transfer progress");
                SavePreview(window, prefix + "-transfers");

                // Empty views retain their actions and guidance in the same production layout.
                Need<ListBox>(window, "ShareList").ItemsSource = null;
                Need<FrameworkElement>(window, "NoShares").Visibility = Visibility.Visible;
                SelectWorkspace(window, localizer, 1);
                Require(Need<Button>(window, "AddShareButton").IsVisible, "Empty sharing view lost its action.");
                SavePreview(window, prefix + "-shares-empty");
                Need<ItemsControl>(window, "TransferList").ItemsSource = null;
                Need<FrameworkElement>(window, "NoTransfers").Visibility = Visibility.Visible;
                SelectWorkspace(window, localizer, 2);
                SavePreview(window, prefix + "-transfers-empty");

                SelectWorkspace(window, localizer, 0);
                Need<FrameworkElement>(window, "PairingPanel").Visibility = Visibility.Visible;
                Need<FrameworkElement>(window, "ErrorPanel").Visibility = Visibility.Visible;
                Need<TextBlock>(window, "PairingPeer").Text = "Quest 3S (test fixture)";
                Need<TextBlock>(window, "PairingHint").Text = localizer["enter_pairing_hint"];
                Need<TextBlock>(window, "PairingError").Text = localizer["pairing_incorrect"];
                Need<TextBlock>(window, "PairingExpiry").Text = string.Format(localizer["pairing_expiry"], 1, 23);
                Need<TextBlock>(window, "ErrorText").Text = localizer["error_connection_timeout"];
                Need<TextBox>(window, "PairingDraft").Text = "0427";
                for (var digit = 0; digit < 4; digit++) Need<TextBlock>(window, "Code" + digit).Text = "0427"[digit].ToString();
                Need<FrameworkElement>(window, "PairRetryButton").Visibility = Visibility.Collapsed;
                Need<FrameworkElement>(window, "WaitingPair").Visibility = Visibility.Collapsed;
                FlushLayout(window);
                RequireInside(Need<FrameworkElement>(window, "PairingCodeArea"), Need<FrameworkElement>(window, "PairingPanel"), "Four-digit pairing slots");
                RequireInside(Need<FrameworkElement>(window, "ErrorText"), Need<FrameworkElement>(window, "ErrorPanel"), "Error recovery text");
                SavePreview(window, prefix + "-pairing-error");
            }
            catch (Exception error)
            {
                try { SavePreview(window, prefix + "-failure"); } catch { /* Preserve the original layout failure. */ }
                throw new InvalidOperationException($"Production window layout failed for {locale} at {size.Width}x{size.Height}.", error);
            }
            finally { window.Close(); }
        }
    }

    private static Window LoadPreview(Localizer localizer, Size size)
    {
        XNamespace wpf = "http://schemas.microsoft.com/winfx/2006/xaml/presentation";
        XNamespace x = "http://schemas.microsoft.com/winfx/2006/xaml";
        var markup = new XElement(ReadXaml("MainWindow.xaml").Root!);
        StripCodeBehind(markup);
        var app = ReadXaml("App.xaml");
        var resources = new XElement(wpf + "ResourceDictionary", app.Root!.Element(wpf + "Application.Resources")!.Elements().Select(value => new XElement(value)));
        resources.SetAttributeValue(XNamespace.Xmlns + "x", x.NamespaceName);
        // Attach resources before parsing: StaticResource references must resolve while children load.
        // The root is a plain Window, matching the production Window style's TargetType.
        markup.AddFirst(new XElement(wpf + "Window.Resources", resources));
        var window = (Window)XamlReader.Parse(markup.ToString());
        foreach (var key in localizer.Keys) window.Resources[key] = localizer[key];
        window.Width = size.Width;
        window.Height = size.Height;
        // These fixtures name the content viewport, excluding the operating system's frame.
        // Hosted Windows desktops can clamp the HWND to their smaller work area.
        window.WindowStartupLocation = WindowStartupLocation.Manual;
        window.Left = 0;
        window.Top = 0;
        window.ShowInTaskbar = false;
        window.ShowActivated = false;
        return window;
    }

    private static void PopulatePreview(Window window, Localizer localizer)
    {
        Need<TextBlock>(window, "DeviceName").Text = "Cinema PC (test fixture)";
        Need<TextBlock>(window, "PeerCount").Text = "1";
        Need<FrameworkElement>(window, "DiscoveryHint").Visibility = Visibility.Collapsed;
        Need<ListBox>(window, "PeerList").ItemsSource = new[]
        {
            new PeerItem(new Peer { Id = "fixture-peer", Name = "Quest 3S", Address = "192.0.2.8:47620", Platform = "quest", Status = "connected", Trusted = true }, localizer),
        };
        Need<ListBox>(window, "DisplayChoice").ItemsSource = new[]
        {
            new DisplayItem(new Display { Id = 1, Name = "Display 1", Width = 2560, Height = 1440 }, localizer),
            new DisplayItem(new Display { Id = 2, Name = "Display 2", Width = 1920, Height = 1080 }, localizer),
        };
        Need<ListBox>(window, "DisplayChoice").SelectedIndex = 0;
        Need<FrameworkElement>(window, "NoDisplays").Visibility = Visibility.Collapsed;
        Need<ListBox>(window, "ShareList").ItemsSource = new[]
        {
            new ShareItem(new Share { Id = "fixture-share", Name = "Cinema archive", Path = @"\\?\C:\Cinema archive\Documentaries and family films\" + string.Concat(Enumerable.Repeat("A long synthetic folder name for wrapping ", 5)).TrimEnd() + @"\Films" }, localizer),
        };
        Need<FrameworkElement>(window, "NoShares").Visibility = Visibility.Collapsed;
        Need<ItemsControl>(window, "TransferList").ItemsSource = new[]
        {
            new TransferItem(new Transfer { Id = "fixture-transfer", Name = "Ocean documentary.mp4", Transferred = 384 * 1024 * 1024, Total = 1024UL * 1024 * 1024, Status = "transferring" }, localizer),
        };
        Need<FrameworkElement>(window, "NoTransfers").Visibility = Visibility.Collapsed;
        Need<FrameworkElement>(window, "ConnectionNotice").Visibility = Visibility.Collapsed;
        Need<TextBlock>(window, "MirrorHint").Text = localizer["mirror_start_hint"];
        foreach (var name in new[] { "PauseButton", "ResumeButton", "StopButton" }) Need<FrameworkElement>(window, name).Visibility = Visibility.Collapsed;
        Need<TextBlock>(window, "FooterStatus").Text = localizer["status_connected"];
        Need<TextBlock>(window, "FooterPorts").Text = string.Format(localizer["network_ports"], 47620, 47621);
        Need<ComboBox>(window, "LanguageChoice").SelectedIndex = localizer["connect"] == "Connect" ? 1 : 2;
    }

    private static void SelectWorkspace(Window window, Localizer localizer, int index)
    {
        var navigation = Need<StackPanel>(window, "WorkspaceNavigation");
        var choices = navigation.Children.OfType<RadioButton>().ToArray();
        choices[index].IsChecked = true;
        MainWindow.ApplyWorkspaceSelection(window);
        MainWindow.ApplyHeaderLayout(((FrameworkElement)window.Content).ActualWidth, Need<FrameworkElement>(window, "WorkspaceTabs"), Need<FrameworkElement>(window, "PageHeading"));
        FlushLayout(window);
        var key = (string)choices[index].Tag;
        Require(choices.Count(choice => choice.IsChecked == true) == 1, "Workspace choices must remain mutually exclusive.");
        Require(Need<TextBlock>(window, "PageTitle").Text == localizer[key] && Need<TextBlock>(window, "PageSubtitle").Text == localizer[key + "_subtitle"], "Workspace selection did not update its localized heading.");
        var pages = new[] { "MirrorPage", "SharesPage", "TransfersPage" }.Select(name => Need<FrameworkElement>(window, name)).ToArray();
        Require(pages.Count(page => page.Visibility == Visibility.Visible) == 1 && pages[index].IsVisible && pages[index].ActualWidth > 200, "Exactly the selected workspace must be presented.");
        VerifyHeader(window);
    }

    private static void VerifyHeader(Window window)
    {
        var header = Need<FrameworkElement>(window, "WorkspaceHeader");
        RequireInside(Need<FrameworkElement>(window, "WorkspaceTabs"), header, "Workspace navigation", vertical: true);
        RequireInside(Need<FrameworkElement>(window, "PageHeading"), header, "Workspace heading", vertical: true);
        Require(Need<TextBlock>(window, "PageTitle").ActualHeight < 60, "Workspace title wrapped into an unusable header.");
    }

    private static void VerifyKeyboardContracts(Window window)
    {
        foreach (var name in new[] { "StartButton", "RescanButton" })
        {
            var button = Need<Button>(window, name);
            Require(button.Focusable && button.IsTabStop && button.FocusVisualStyle is not null, "Button keyboard focus contract regressed: " + name);
        }
        foreach (var choice in Need<StackPanel>(window, "WorkspaceNavigation").Children.OfType<RadioButton>())
            Require(choice.Focusable && choice.IsEnabled && choice.FocusVisualStyle is not null, "Workspace choice keyboard focus contract regressed.");
        var draft = Need<TextBox>(window, "PairingDraft");
        Require(draft.Focusable && draft.MaxLength == 4 && !string.IsNullOrEmpty(AutomationProperties.GetName(draft)), "Pairing keyboard input or accessible name regressed.");
    }

    private static void VerifyDropdown(Window window)
    {
        var quality = Need<ComboBox>(window, "QualityChoice");
        Require(quality.Focusable && quality.IsTabStop && !string.IsNullOrEmpty(AutomationProperties.GetName(quality)), "Quality selector lost its keyboard or accessible-name contract.");
        quality.ApplyTemplate();
        var popup = quality.Template.FindName("PART_Popup", quality) as Popup;
        Require(popup is not null, "ComboBox lost its native popup part.");
        quality.IsDropDownOpen = true;
        FlushLayout(window);
        Require(popup!.IsOpen && popup.Child?.IsVisible == true, "Quality dropdown did not open with its real template.");
        quality.SelectedIndex = 2;
        FlushLayout(window);
        Require(quality.SelectedItem == quality.Items[2] && quality.SelectionBoxItem is not null, "Quality dropdown selection did not update.");
        quality.IsDropDownOpen = false;
        FlushLayout(window);
        Require(!popup.IsOpen, "Quality dropdown did not close.");
    }

    private static void RequireInside(FrameworkElement child, FrameworkElement parent, string name, bool vertical = false)
    {
        var bounds = child.TransformToAncestor(parent).TransformBounds(new Rect(new Point(), child.RenderSize));
        Require(child.ActualWidth > 0 && child.ActualHeight > 0 && bounds.Left >= -1 && bounds.Right <= parent.ActualWidth + 1
            && (!vertical || (bounds.Top >= -1 && bounds.Bottom <= parent.ActualHeight + 1)), name + " overflowed its containing layout.");
    }

    private static void FlushLayout(Window window)
    {
        window.UpdateLayout();
        Dispatcher.CurrentDispatcher.Invoke(DispatcherPriority.ApplicationIdle, new Action(() => { }));
        window.UpdateLayout();
        var background = ((window.Content as Panel)?.Background as SolidColorBrush);
        Require(background is not null && background.Color.A == 255 && background.Opacity == 1, "The production content root must paint an opaque background.");
    }

    private static void SavePreview(Window window, string name)
    {
        var directory = Environment.GetEnvironmentVariable("YML_UI_EVIDENCE_DIR");
        if (string.IsNullOrWhiteSpace(directory)) return;
        Directory.CreateDirectory(directory);
        FlushLayout(window);
        var content = (FrameworkElement)window.Content;
        var dpi = VisualTreeHelper.GetDpi(content);
        // Capture the actual client area of the shown native Window. The fixture window
        // sizes fit the hosted desktop; the image excludes the OS titlebar and borders.
        var bitmap = new RenderTargetBitmap((int)Math.Ceiling(content.ActualWidth * dpi.DpiScaleX), (int)Math.Ceiling(content.ActualHeight * dpi.DpiScaleY), dpi.PixelsPerInchX, dpi.PixelsPerInchY, PixelFormats.Pbgra32);
        bitmap.Render(content);
        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bitmap));
        using (var output = File.Create(Path.Combine(directory, name + ".png"))) encoder.Save(output);
        var stride = checked(bitmap.PixelWidth * 4);
        var pixels = new byte[checked(stride * bitmap.PixelHeight)];
        bitmap.CopyPixels(pixels, stride, 0);
        // WPF text antialiasing can round an opaque composited pixel to alpha 254.
        // Reject actual transparent/clipped regions, while allowing that quantization.
        for (var offset = 3; offset < pixels.Length; offset += 4)
            Require(pixels[offset] >= 250, $"Native client render {name} ({bitmap.PixelWidth}x{bitmap.PixelHeight}) has alpha {pixels[offset]} at ({offset / 4 % bitmap.PixelWidth}, {offset / stride}).");
    }

    private static T Need<T>(Window window, string name) where T : FrameworkElement =>
        window.FindName(name) as T ?? throw new InvalidOperationException("Missing production element: " + name);

    private static void Require(bool condition, string message)
    {
        if (!condition) throw new InvalidOperationException(message);
    }

    private static T? Find<T>(DependencyObject parent) where T : DependencyObject
    {
        for (var i = 0; i < VisualTreeHelper.GetChildrenCount(parent); i++)
        {
            var child = VisualTreeHelper.GetChild(parent, i);
            if (child is T match) return match;
            if (Find<T>(child) is { } descendant) return descendant;
        }
        return null;
    }
}
