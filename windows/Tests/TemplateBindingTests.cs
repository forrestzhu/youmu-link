using System.Reflection;
using System.Runtime.ExceptionServices;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Markup;
using System.Windows.Media;
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
            try { MaterializeRows(); }
            catch (Exception error) { failure = ExceptionDispatchInfo.Capture(error); }
            finally { Dispatcher.CurrentDispatcher.InvokeShutdown(); }
        }) { IsBackground = true };
        thread.SetApartmentState(ApartmentState.STA);
        thread.Start();
        if (!thread.Join(TimeSpan.FromSeconds(15))) throw new TimeoutException("WPF template regression timed out.");
        failure?.Throw();
        Console.WriteLine("PASS: actual WPF share/transfer templates materialize getter-only production rows and display their bound values.");
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
                foreach (var attribute in markup.DescendantsAndSelf().Attributes("Click").ToArray()) attribute.Remove();
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
