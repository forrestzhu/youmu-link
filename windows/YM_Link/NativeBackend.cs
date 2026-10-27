using Microsoft.Win32.SafeHandles;
using System.Runtime.InteropServices;
using System.Text.Json;

namespace YM_Link;

// A single task chain owns every native call, including creation and destruction.
// Work never runs on WPF's dispatcher; closing drains the chain before freeing Rust.
internal sealed class NativeBackend : IAsyncDisposable
{
    private readonly object gate = new();
    private Task tail = Task.CompletedTask;
    private bool stopping;
    private BackendHandle? handle;

    private Task<T> Enqueue<T>(Func<T> operation)
    {
        lock (gate)
        {
            ObjectDisposedException.ThrowIf(stopping, this);
            var previous = tail;
            var next = Task.Run(async () =>
            {
                await previous.ConfigureAwait(ConfigureAwaitOptions.SuppressThrowing);
                return operation();
            });
            tail = next;
            return next;
        }
    }

    public Task<bool> StartAsync(string directory, string name) => Enqueue(() =>
    {
        if (handle is not null) throw new InvalidOperationException("Backend already started.");
        handle = new BackendHandle(Native.Create(directory, name));
        return !handle.IsInvalid;
    });

    public Task<Snapshot> SnapshotAsync() => Enqueue(() =>
        JsonSerializer.Deserialize<Snapshot>(ReadString(Native.Snapshot(RequireHandle())), WireJson.Options)
        ?? throw new JsonException());

    public Task<CommandResult> CommandAsync(string command) => Enqueue(() =>
        JsonSerializer.Deserialize<CommandResult>(ReadString(Native.Command(RequireHandle(), command)), WireJson.Options)
        ?? throw new JsonException());

    private BackendHandle RequireHandle() => handle is { IsInvalid: false, IsClosed: false }
        ? handle : throw new InvalidOperationException("Backend unavailable.");

    private static string ReadString(IntPtr pointer)
    {
        if (pointer == IntPtr.Zero) throw new InvalidOperationException("Backend response unavailable.");
        try { return Marshal.PtrToStringUTF8(pointer) ?? throw new JsonException(); }
        finally { Native.StringFree(pointer); }
    }

    public ValueTask DisposeAsync()
    {
        lock (gate)
        {
            if (!stopping)
            {
                stopping = true;
                var previous = tail;
                tail = Task.Run(async () =>
                {
                    await previous.ConfigureAwait(ConfigureAwaitOptions.SuppressThrowing);
                    handle?.Dispose();
                    handle = null;
                });
            }
            return new ValueTask(tail);
        }
    }

    private sealed class BackendHandle : SafeHandleZeroOrMinusOneIsInvalid
    {
        public BackendHandle(IntPtr pointer) : base(true) { SetHandle(pointer); }
        protected override bool ReleaseHandle() { Native.Destroy(handle); return true; }
        // SafeHandle retains the pointer across P/Invoke; only the serialized owner disposes it.
    }

    private static class Native
    {
        private const string Library = "ymlink_desktop.dll";
        [DllImport(Library, EntryPoint = "ymlink_create", CallingConvention = CallingConvention.Cdecl)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]
        internal static extern IntPtr Create([MarshalAs(UnmanagedType.LPUTF8Str)] string directory, [MarshalAs(UnmanagedType.LPUTF8Str)] string name);
        [DllImport(Library, EntryPoint = "ymlink_snapshot", CallingConvention = CallingConvention.Cdecl)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]
        internal static extern IntPtr Snapshot(BackendHandle handle);
        [DllImport(Library, EntryPoint = "ymlink_command", CallingConvention = CallingConvention.Cdecl)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]
        internal static extern IntPtr Command(BackendHandle handle, [MarshalAs(UnmanagedType.LPUTF8Str)] string command);
        [DllImport(Library, EntryPoint = "ymlink_string_free", CallingConvention = CallingConvention.Cdecl)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]
        internal static extern void StringFree(IntPtr pointer);
        [DllImport(Library, EntryPoint = "ymlink_destroy", CallingConvention = CallingConvention.Cdecl)]
        [DefaultDllImportSearchPaths(DllImportSearchPath.AssemblyDirectory | DllImportSearchPath.System32)]
        internal static extern void Destroy(IntPtr pointer);
    }
}
