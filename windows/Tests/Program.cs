using System.Globalization;
using System.Text.Json;
using System.Reflection;
using System.Runtime.InteropServices;
using System.Text.RegularExpressions;
using System.IO;
using YM_Link;

static void Check(bool ok, string message)
{
    if (!ok) throw new InvalidOperationException(message);
}

var en = new Localizer("en");
var zh = new Localizer("zh-Hans");
Check(en.Keys.Order().SequenceEqual(zh.Keys.Order()), "Localization keys must match.");
Check(en.Keys.All(key => !string.IsNullOrWhiteSpace(en[key]) && !string.IsNullOrWhiteSpace(zh[key])), "Empty localization.");
Check(new Localizer("unsupported")["connect"] == en["connect"], "Unknown culture must fall back to English.");
Check(new Localizer("system", CultureInfo.GetCultureInfo("zh-CN"))["connect"] == zh["connect"], "System Chinese selection.");
Check(en.BackendMessage("connection error: secret user path") == en["error_connection"], "Transport details must not reach UI.");
Check(zh.BackendMessage("unknown C:\\private\\film") == zh["command_failed"], "Unknown errors must not expose paths.");
using (var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("YM_Link.Tests.MainWindow.xaml")!)
using (var reader = new StreamReader(stream))
    foreach (Match match in Regex.Matches(reader.ReadToEnd(), @"DynamicResource ([a-z][a-z_]+)"))
        Check(en.Keys.Contains(match.Groups[1].Value), "Missing UI resource: " + match.Groups[1].Value);
foreach (var error in new[] { "Cannot secure application data directory", "Invalid identity Credential Manager entry", "Unable to configure Windows H.264 encoder", "Windows Graphics Capture is unavailable", "No default system audio output is available", "Unable to write video buffer", "Windows media clock is invalid", "Display size changed; start screen mirroring again", "Media Foundation is unavailable", "Unable to allocate screen readback texture" })
    Check(en.BackendMessage(error) != en["command_failed"] && zh.BackendMessage(error) != zh["command_failed"], "Untranslated Windows error category: " + error);
using (var stream = Assembly.GetExecutingAssembly().GetManifestResourceStream("YM_Link.Tests.WindowsCaptureErrors.rs")!)
using (var reader = new StreamReader(stream))
    foreach (Match match in Regex.Matches(reader.ReadToEnd(), "\"((?:Unable to |Invalid |Windows |Direct3D |Screen |The captured |No default |H\\.264 |Empty H\\.264 |Unsupported H\\.264 |Display (?:dimensions|size) |Mirror |Unknown mirror |Media Foundation )[^\"]+)\""))
        Check(en.BackendMessage(match.Groups[1].Value) != en["command_failed"], "Untranslated capture failure: " + match.Groups[1].Value);
var snapshot = JsonSerializer.Deserialize<Snapshot>("""
{"device_name":"PC","status":"ready","listen_port":4,"discovery_port":5,"peers":[],"pairing":{"peer_name":"Quest","code":"0427","expires_in":50,"mode":"display","window_id":"window-1","attempt":1,"stage":"ready","error":null,"error_revision":0,"retry_after_ms":0},"displays":[{"id":7,"name":"Display 7","width":1920,"height":1080}],"mirror":{"status":"paused","permission":"granted","display_id":7,"audio":true,"quality":"balanced"},"shares":[],"transfers":[],"error_id":3,"error":"connection closed"}
""", WireJson.Options)!;
Check(snapshot.DeviceName == "PC" && snapshot.Pairing?.Code == "0427" && snapshot.Mirror.IsActive, "Snake case snapshot decoding.");
Check(snapshot.Displays[0].Id == 7, "Display decoding.");
var cursor = new ErrorCursor();
Check(cursor.Consume(snapshot) == "connection closed", "First error event.");
Check(cursor.Consume(snapshot) == null, "Repeated error event must be consumed once.");
Check(cursor.Consume(snapshot with { ErrorId = 4 }) == "connection closed", "Same message with new event id must surface.");
using (var command = JsonDocument.Parse(WireJson.Command("start_mirror", ("display_id", (object)7u), ("audio", true), ("quality", "smooth"))))
{
    Check(command.RootElement.GetProperty("action").GetString() == "start_mirror", "Command action.");
    Check(command.RootElement.GetProperty("display_id").GetUInt32() == 7, "Numeric display command.");
    Check(command.RootElement.GetProperty("audio").GetBoolean(), "Boolean audio command.");
}
var transfer = new Transfer { Total = 100, Transferred = 120 };
Check(transfer.Fraction == 1 && (transfer with { Total = 0 }).Fraction == 0, "Transfer fraction is bounded.");
Check(Pairing.IsValidCode("0427") && !Pairing.IsValidCode("１２３４５６") && !Pairing.IsValidCode("12345"), "ASCII pairing boundary.");
await using (var backend = new NativeBackend())
{
    await backend.DisposeAsync();
    await backend.DisposeAsync();
    try { await backend.SnapshotAsync(); throw new Exception("Disposed backend accepted work."); }
    catch (ObjectDisposedException) { }
}
Console.WriteLine("PASS: localization parity/fallback, redacted errors, snapshots, error cursor, commands, pairing, transfers, idempotent lifetime.");
TemplateBindingTests.Run();
if (args is ["--native", var dll])
{
    NativeLibrary.SetDllImportResolver(typeof(NativeBackend).Assembly, (library, _, _) => library == "ymlink_desktop.dll" ? NativeLibrary.Load(Path.GetFullPath(dll)) : IntPtr.Zero);
    var directory = Path.Combine(Path.GetTempPath(), "YM_Link-lifetime-" + Guid.NewGuid().ToString("N"));
    Directory.CreateDirectory(directory);
    try
    {
        var backend = new NativeBackend();
        try
        {
            Check(await backend.StartAsync(directory, "YM_Link lifecycle test"), "Real native backend start.");
            var reads = Enumerable.Range(0, 8).Select(_ => backend.SnapshotAsync()).ToArray();
            var command = backend.CommandAsync(WireJson.Command("rescan"));
            var close = backend.DisposeAsync().AsTask();
            await Task.WhenAll(reads);
            Check((await command).Ok, "Queued native command.");
            await close;
            Check(reads.All(read => read.Result.DeviceName == "YM_Link lifecycle test"), "Snapshot work completes before handle destruction.");
            await backend.DisposeAsync();
            try { await backend.CommandAsync(WireJson.Command("rescan")); throw new Exception("Native backend accepted work after disposal."); }
            catch (ObjectDisposedException) { }
        }
        finally { await backend.DisposeAsync(); }
        Console.WriteLine("PASS: real DLL lifecycle, concurrent queued snapshots/command, drain before disposal, repeat disposal, rejection after close.");
    }
    finally { Directory.Delete(directory, true); }
}

var pair = new Pairing { PeerName = "Synthetic PC", WindowId = "window-1", Attempt = 1, Stage = "ready", Mode = "enter", ExpiresIn = 120 };
var input = new PairingInput(); input.Apply(pair);
Check(input.Edit("042", pair) is null, "Partial input waits.");
Check(input.Edit("0427", pair, CommandAdmission.CanSend("pair", true, false, 1)) is null && input.Text == "042", "Busy dispatch cannot consume the fourth digit or mark the attempt submitted.");
Check(input.Edit("042", pair) is null && input.Edit("0427", pair) == "0427", "Fourth digit auto submits with leading zero.");
Check(CommandAdmission.CanSend("cancel_pairing", true, false, 1) && !CommandAdmission.CanSend("pair", true, false, 1) && !CommandAdmission.CanSend("rescan", true, false, 1), "Cancel queues while native work is pending; other actions wait.");
Check(!CommandAdmission.CanSend("cancel_pairing", true, true, 1) && !CommandAdmission.CanSend("cancel_pairing", false, false, 0), "Cancellation respects actual backend lifetime.");
Check(input.Edit("9999", pair) is null, "One submit per ready attempt.");
pair = pair with { Stage = "incorrect", Error = "incorrect", ErrorRevision = 1 };
Check(input.Apply(pair) && input.Feedback, "Wrong code starts feedback.");
pair = pair with { Stage = "ready", Attempt = 2 };
Check(!input.Apply(pair), "TCP retry does not repeat the same rejection.");
input.ClearRejected("window-1", input.RejectionCount);
Check(input.Text == "" && input.ErrorVisible, "Clear the code, keep the inline error.");
Check(input.Edit("0427", pair) == "0427" && !input.ErrorVisible, "Direct reentry after rejection.");
pair = pair with { WindowId = "window-2", Attempt = 3, Error = null, ErrorRevision = 0 }; input.Apply(pair); input.Edit("4", pair);
input.ClearRejected("window-1", 1);
Check(input.Text == "4", "Late feedback cannot erase a new window.");
