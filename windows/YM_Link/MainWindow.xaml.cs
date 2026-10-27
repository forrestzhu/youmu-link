using Microsoft.Win32;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.IO;
using System.Text.Json;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;

namespace YM_Link;

public partial class MainWindow : Window
{
    private readonly NativeBackend backend = new();
    private readonly CancellationTokenSource cancellation = new();
    private readonly ErrorCursor errors = new();
    private readonly ObservableCollection<PeerItem> peers = [];
    private readonly ObservableCollection<DisplayItem> displays = [];
    private readonly ObservableCollection<ShareItem> shares = [];
    private readonly ObservableCollection<TransferItem> transfers = [];
    private Localizer localizer;
    private Snapshot current = new();
    private Task? polling;
    private bool ready, closing, closed, rendering, responseError;
    private int pending;
    private string? errorKey, nativeError;

    internal MainWindow(Localizer localizer)
    {
        this.localizer = localizer;
        InitializeComponent();
        PeerList.ItemsSource = peers;
        DisplayChoice.ItemsSource = displays;
        ShareList.ItemsSource = shares;
        TransferList.ItemsSource = transfers;
        var language = UserSettings.ReadLanguage();
        rendering = true;
        LanguageChoice.SelectedItem = LanguageChoice.Items.Cast<ComboBoxItem>().First(item => (string)item.Tag == language);
        rendering = false;
        Render();
    }

    private string L(string key) => localizer[key];

    private async void WindowLoaded(object sender, RoutedEventArgs e)
    {
        try
        {
            ready = await backend.StartAsync(App.StateDirectory, Environment.MachineName);
            if (closing) return;
            if (!ready) { ShowErrorKey("backend_start_failed"); Render(); return; }
            Apply(await backend.SnapshotAsync());
            if (!closing) polling = PollAsync();
        }
        catch { if (!closing) { ShowErrorKey("backend_start_failed"); Render(); } }
    }

    private async Task PollAsync()
    {
        using var timer = new PeriodicTimer(TimeSpan.FromMilliseconds(500));
        try
        {
            while (await timer.WaitForNextTickAsync(cancellation.Token))
            {
                try
                {
                    var snapshot = await backend.SnapshotAsync();
                    if (!closing) { Apply(snapshot); responseError = false; }
                }
                catch when (!closing)
                {
                    if (!responseError) { ShowErrorKey("backend_response_failed"); responseError = true; }
                }
            }
        }
        catch (OperationCanceledException) { }
        catch (ObjectDisposedException) when (closing) { }
    }

    private void Apply(Snapshot snapshot)
    {
        if (closing) return;
        var oldPairing = current.Pairing;
        var oldMirror = current.Mirror;
        current = snapshot;
        rendering = true;
        Sync(peers, snapshot.Peers, value => value.Id, (value, l) => new PeerItem(value, l));
        Sync(displays, snapshot.Displays, value => value.Id.ToString(), (value, l) => new DisplayItem(value, l));
        Sync(shares, snapshot.Shares, value => value.Id, (value, l) => new ShareItem(value, l));
        Sync(transfers, snapshot.Transfers, value => value.Id, (value, l) => new TransferItem(value, l));
        if (snapshot.Mirror.IsActive && snapshot.Mirror.DisplayId != (DisplayChoice.SelectedItem as DisplayItem)?.Data.Id)
            DisplayChoice.SelectedItem = displays.FirstOrDefault(item => item.Data.Id == snapshot.Mirror.DisplayId);
        if (DisplayChoice.SelectedItem is not DisplayItem)
            DisplayChoice.SelectedItem = displays.FirstOrDefault(item => item.Data.Id == snapshot.Mirror.DisplayId) ?? displays.FirstOrDefault();
        if (snapshot.Mirror.IsActive && (oldMirror.Status != snapshot.Mirror.Status || oldMirror.Audio != snapshot.Mirror.Audio || oldMirror.Quality != snapshot.Mirror.Quality))
        {
            AudioChoice.IsChecked = snapshot.Mirror.Audio;
            QualityChoice.SelectedItem = QualityChoice.Items.Cast<ComboBoxItem>().FirstOrDefault(item => (string)item.Tag == snapshot.Mirror.Quality) ?? QualityChoice.Items[1];
        }
        if (snapshot.Pairing is { } pairing)
        {
            if (oldPairing is null || oldPairing.PeerName != pairing.PeerName || oldPairing.Mode != pairing.Mode || pairing.Code != oldPairing.Code)
            {
                PairingDraft.Text = pairing.Mode == "enter" ? "" : pairing.Code ?? "";
                if (pairing.Mode == "enter") PairingDraft.Focus();
            }
            if (pairing.ExpiresIn == 0 && pairing.Mode != "enter") PairingDraft.Text = "";
        }
        rendering = false;
        if (errors.Consume(snapshot) is { } error) ShowNativeError(error);
        Render();
    }

    // Preserve row identity, selection, keyboard focus and partially entered form values on polls.
    private void Sync<T, TRow>(ObservableCollection<TRow> rows, List<T> values, Func<T, string> id, Func<T, Localizer, TRow> create) where TRow : Row<T>
    {
        var ids = values.Select(id).ToHashSet();
        for (var i = rows.Count - 1; i >= 0; i--) if (!ids.Contains(id(rows[i].Data))) rows.RemoveAt(i);
        for (var i = 0; i < values.Count; i++)
        {
            var value = values[i];
            var row = rows.FirstOrDefault(item => id(item.Data) == id(value));
            if (row is null) rows.Insert(i, create(value, localizer));
            else { if (rows.IndexOf(row) != i) rows.Move(rows.IndexOf(row), i); row.Update(value, localizer); }
        }
    }

    private void Render()
    {
        if (!IsInitialized) return;
        var connected = current.Status == "connected";
        var mirror = current.Mirror;
        var active = mirror.IsActive;
        var enabled = ready && !closing && pending == 0;
        DeviceName.Text = current.DeviceName.Length == 0 ? L("this_pc") : current.DeviceName;
        PeerCount.Text = peers.Count.ToString();
        DiscoveryHint.Visibility = peers.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        DeviceHint.Visibility = PeerList.SelectedItem is null ? Visibility.Visible : Visibility.Collapsed;
        var peer = (PeerList.SelectedItem as PeerItem)?.Data;
        ConnectButton.IsEnabled = enabled && peer is not null && peer.Status is not ("connected" or "paired" or "connecting" or "pairing");
        DisconnectButton.IsEnabled = enabled && peer?.Status is "connected" or "paired" or "connecting" or "pairing";
        ForgetButton.IsEnabled = enabled && peer?.Trusted == true;
        RescanButton.IsEnabled = RefreshButton.IsEnabled = AddShareButton.IsEnabled = AddressConnectButton.IsEnabled = enabled;
        ShareList.IsEnabled = TransferList.IsEnabled = enabled;
        ConnectionNotice.Visibility = connected ? Visibility.Collapsed : Visibility.Visible;
        ConnectionTitle.Text = L(peers.Count == 0 ? "waiting_headset" : "choose_device");
        DisplayChoice.IsEnabled = enabled && displays.Count > 0;
        AudioChoice.IsEnabled = QualityChoice.IsEnabled = !active && !closing;
        var display = (DisplayChoice.SelectedItem as DisplayItem)?.Data;
        DisplaySummary.Text = display is null ? L("no_displays") : localizer.DisplayName(display);
        DisplayHint.Text = display is null ? L("display_permission_hint") : $"{display.Width} × {display.Height}";
        StartButton.Visibility = active ? Visibility.Collapsed : Visibility.Visible;
        StartButton.IsEnabled = enabled && connected && display is not null;
        PauseButton.Visibility = active && mirror.Status != "paused" ? Visibility.Visible : Visibility.Collapsed;
        PauseButton.IsEnabled = enabled && mirror.Status == "running";
        ResumeButton.Visibility = mirror.Status == "paused" ? Visibility.Visible : Visibility.Collapsed;
        ResumeButton.IsEnabled = enabled && connected;
        StopButton.Visibility = active ? Visibility.Visible : Visibility.Collapsed;
        StopButton.IsEnabled = enabled;
        MirrorHint.Text = L(mirror.Status switch { "paused" => "mirror_paused_hint", "starting" => "mirror_starting_hint", "running" => "mirror_live_hint", _ => "mirror_start_hint" });
        MirrorError.Text = mirror.Error is null ? "" : localizer.BackendMessage(mirror.Error);
        MirrorError.Visibility = mirror.Error is null ? Visibility.Collapsed : Visibility.Visible;
        NoShares.Visibility = shares.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        NoTransfers.Visibility = transfers.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        FooterStatus.Text = closing ? L("closing_app") : !ready && errorKey is null ? L("starting_app") : localizer.Status(ready ? current.Status : "error");
        if (active) FooterStatus.Text += " · " + localizer.Status(mirror.Status);
        if (mirror.Status == "running")
        {
            if (mirror.Fps is { } fps) FooterStatus.Text += $" · {fps:0} fps";
            if (mirror.BitrateMbps is { } bitrate) FooterStatus.Text += $" · {bitrate:0.0} Mbps";
            if (mirror.LatencyMs is { } latency) FooterStatus.Text += $" · {latency:0} ms";
        }
        FooterPorts.Text = current.ListenPort == 0 ? "" : string.Format(L("network_ports"), current.ListenPort, current.DiscoveryPort);
        RenderPairing(enabled);
        RenderError();
    }

    private void RenderPairing(bool enabled)
    {
        var pairing = current.Pairing;
        PairingPanel.Visibility = pairing is null ? Visibility.Collapsed : Visibility.Visible;
        if (pairing is null) return;
        PairingPeer.Text = pairing.PeerName;
        PairingHint.Text = L(pairing.ExpiresIn == 0 ? "pairing_expired" : pairing.Mode == "enter" ? "enter_pairing_hint" : "display_pairing_hint");
        PairingDraft.IsReadOnly = pairing.Mode != "enter" || pairing.ExpiresIn == 0;
        PairConfirmButton.Visibility = pairing.Mode == "enter" ? Visibility.Visible : Visibility.Collapsed;
        PairConfirmButton.IsEnabled = enabled && pairing.ExpiresIn > 0 && Pairing.IsValidCode(PairingDraft.Text);
        WaitingPair.Visibility = pairing.Mode == "enter" ? Visibility.Collapsed : Visibility.Visible;
        PairingExpiry.Text = string.Format(L("pairing_expiry"), pairing.ExpiresIn / 60, pairing.ExpiresIn % 60);
    }

    private void ShowErrorKey(string key) { errorKey = key; nativeError = null; RenderError(); }
    private void ShowNativeError(string message) { nativeError = message; errorKey = null; RenderError(); }
    private void RenderError()
    {
        ErrorText.Text = errorKey is not null ? L(errorKey) : nativeError is not null ? localizer.BackendMessage(nativeError) : "";
        ErrorPanel.Visibility = errorKey is not null || nativeError is not null ? Visibility.Visible : Visibility.Collapsed;
    }

    private async Task Send(string action, params (string Key, object Value)[] fields)
    {
        if (!ready || closing || pending > 0) return;
        pending++;
        Render();
        try
        {
            var result = await backend.CommandAsync(WireJson.Command(action, fields));
            if (closing) return;
            if (!result.Ok) ShowNativeError(result.Error ?? "backend busy or unavailable");
            Apply(await backend.SnapshotAsync());
        }
        catch { if (!closing) ShowErrorKey("backend_response_failed"); }
        finally { pending--; if (!closing) Render(); }
    }

    private async void Rescan(object sender, RoutedEventArgs e) => await Send("rescan");
    private async void RefreshDisplays(object sender, RoutedEventArgs e) => await Send("refresh_displays");
    private void PeerSelectionChanged(object sender, SelectionChangedEventArgs e) { if (!rendering) Render(); }
    private async void ConnectPeer(object sender, RoutedEventArgs e) { if (PeerList.SelectedItem is PeerItem item) await Send("connect", ("peer_id", item.Id)); }
    private async void DisconnectPeer(object sender, RoutedEventArgs e) { if (PeerList.SelectedItem is PeerItem item) await Send("disconnect", ("peer_id", item.Id)); }
    private async void ForgetPeer(object sender, RoutedEventArgs e)
    {
        if (PeerList.SelectedItem is PeerItem item && MessageBox.Show(this, L("confirm_forget"), L("forget_device"), MessageBoxButton.YesNo, MessageBoxImage.Question, MessageBoxResult.No) == MessageBoxResult.Yes)
            await Send("forget", ("peer_id", item.Id));
    }
    private async void ConnectAddress(object sender, RoutedEventArgs e) => await ConnectAddressAsync();
    private async Task ConnectAddressAsync()
    {
        var address = AddressDraft.Text.Trim();
        if (address.Length == 0) { ShowErrorKey("error_address_invalid"); return; }
        await Send("connect", ("address", address));
    }
    private async void AddressKeyDown(object sender, KeyEventArgs e) { if (e.Key == Key.Enter) { e.Handled = true; await ConnectAddressAsync(); } }
    private async void CancelPairing(object sender, RoutedEventArgs e) => await Send("cancel_pairing");
    private async void ConfirmPairing(object sender, RoutedEventArgs e) => await SubmitPairing();
    private async Task SubmitPairing()
    {
        if (current.Pairing is { Mode: "enter", ExpiresIn: > 0 } && Pairing.IsValidCode(PairingDraft.Text)) await Send("pair", ("code", PairingDraft.Text));
    }
    private void PairingTextChanged(object sender, TextChangedEventArgs e)
    {
        if (rendering || !IsInitialized) return;
        var digits = new string(PairingDraft.Text.Where(c => c is >= '0' and <= '9').Take(6).ToArray());
        if (digits != PairingDraft.Text) { var caret = PairingDraft.CaretIndex; PairingDraft.Text = digits; PairingDraft.CaretIndex = Math.Min(caret, digits.Length); }
        RenderPairing(ready && pending == 0 && !closing);
    }
    private async void PairingKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Enter) { e.Handled = true; await SubmitPairing(); }
        if (e.Key == Key.Escape) { e.Handled = true; await Send("cancel_pairing"); }
    }
    private async void DisplaySelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (rendering || !IsInitialized) return;
        if (current.Mirror.IsActive && DisplayChoice.SelectedItem is DisplayItem item && item.Data.Id != current.Mirror.DisplayId)
            await Send("select_mirror_display", ("display_id", item.Data.Id));
        Render();
    }
    private async void StartMirror(object sender, RoutedEventArgs e)
    {
        if (DisplayChoice.SelectedItem is not DisplayItem item || current.Status != "connected") return;
        var audio = AudioChoice.IsChecked == true;
        var quality = (string)((ComboBoxItem)QualityChoice.SelectedItem).Tag;
        var message = localizer.DisplayName(item.Data) + "\n\n" + L("confirm_mirror") + (audio ? "\n\n" + L("confirm_mirror_audio") : "");
        if (MessageBox.Show(this, message, L("confirm_mirror_title"), MessageBoxButton.YesNo, MessageBoxImage.Question, MessageBoxResult.No) == MessageBoxResult.Yes)
            await Send("start_mirror", ("display_id", item.Data.Id), ("audio", audio), ("quality", quality));
    }
    private async void PauseMirror(object sender, RoutedEventArgs e) => await Send("pause_mirror");
    private async void ResumeMirror(object sender, RoutedEventArgs e) => await Send("resume_mirror");
    private async void StopMirror(object sender, RoutedEventArgs e) => await Send("stop_mirror");
    private async void AddShare(object sender, RoutedEventArgs e)
    {
        var picker = new OpenFolderDialog { Title = L("authorize_folder"), Multiselect = false };
        if (picker.ShowDialog(this) == true) await Send("add_share", ("path", picker.FolderName));
    }
    private async void RemoveShare(object sender, RoutedEventArgs e) => await Send("remove_share", ("id", (string)((Button)sender).Tag));
    private async void PauseTransfer(object sender, RoutedEventArgs e) => await Send("pause_transfer", ("id", (string)((Button)sender).Tag));
    private async void ResumeTransfer(object sender, RoutedEventArgs e) => await Send("resume_transfer", ("id", (string)((Button)sender).Tag));
    private async void CancelTransfer(object sender, RoutedEventArgs e) => await Send("cancel_transfer", ("id", (string)((Button)sender).Tag));
    private void DismissError(object sender, RoutedEventArgs e) { errorKey = nativeError = null; RenderError(); }

    private void LanguageChanged(object sender, SelectionChangedEventArgs e)
    {
        if (rendering || !IsInitialized || LanguageChoice.SelectedItem is not ComboBoxItem item) return;
        var language = (string)item.Tag;
        localizer = new Localizer(language);
        App.SetLanguageResources(localizer);
        foreach (var row in peers) row.Update(row.Data, localizer, true);
        foreach (var row in displays) row.Update(row.Data, localizer, true);
        foreach (var row in transfers) row.Update(row.Data, localizer, true);
        try { UserSettings.SaveLanguage(language); }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException) { ShowErrorKey("state_save_failed"); }
        Render();
    }

    private async void WindowClosing(object? sender, CancelEventArgs e)
    {
        if (closed) return;
        e.Cancel = true;
        if (closing) return;
        closing = true;
        cancellation.Cancel();
        IsEnabled = false;
        Render();
        try
        {
            try { if (polling is not null) await polling; }
            finally { await backend.DisposeAsync(); }
        }
        finally
        {
            closed = true;
            cancellation.Dispose();
            Close();
            Application.Current.Shutdown();
        }
    }
}

internal abstract class Row<T>(T data, Localizer localizer) : INotifyPropertyChanged
{
    public T Data { get; private set; } = data;
    protected Localizer Localizer { get; private set; } = localizer;
    public event PropertyChangedEventHandler? PropertyChanged;
    public void Update(T value, Localizer selected, bool force = false)
    {
        if (!force && EqualityComparer<T>.Default.Equals(Data, value)) return;
        Data = value;
        Localizer = selected;
        PropertyChanged?.Invoke(this, new PropertyChangedEventArgs(null));
    }
}
internal sealed class PeerItem(Peer data, Localizer localizer) : Row<Peer>(data, localizer)
{
    public string Id => Data.Id;
    public string Name => Data.Name.Length == 0 ? Localizer["nearby_device"] : Data.Name;
    public string Address => Data.Address;
    public string StatusText => Localizer.Status(Data.Status);
    public string TrustText => Data.Trusted ? Localizer["trusted_label"] : "";
}
internal sealed class DisplayItem(Display data, Localizer localizer) : Row<Display>(data, localizer)
{
    public string Label => $"{Localizer.DisplayName(Data)} · {Data.Width} × {Data.Height}";
}
internal sealed class ShareItem(Share data, Localizer localizer) : Row<Share>(data, localizer)
{
    public string Id => Data.Id;
    public string Name => Data.Name;
    public string Path => Data.Path;
}
internal sealed class TransferItem(Transfer data, Localizer localizer) : Row<Transfer>(data, localizer)
{
    public string Id => Data.Id;
    public string Name => Data.Name;
    public string StatusText => Localizer.Status(Data.Status);
    public double Fraction => Data.Fraction;
    public bool CanPause => Data.Status is "transferring" or "running";
    public bool CanResume => Data.Status == "paused";
    public bool CanCancel => Data.Status is not ("completed" or "cancelled" or "error" or "failed");
    public string Hint => Data.Status == "waiting" ? Localizer["transfer_waiting"] : "";
    public string SizeText => string.Format(Localizer["transfer_size"], Bytes(Data.Transferred), Bytes(Data.Total));
    private static string Bytes(ulong bytes)
    {
        string[] units = ["B", "KB", "MB", "GB", "TB"];
        var value = (double)bytes;
        var unit = 0;
        while (value >= 1024 && unit < units.Length - 1) { value /= 1024; unit++; }
        return $"{value:0.#} {units[unit]}";
    }
}
