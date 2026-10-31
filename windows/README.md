# Native Windows application

YM_Link uses WPF (.NET 8) native controls and the same Rust `ymlink-desktop` C ABI as macOS. Rust owns discovery, encrypted pairing, trust, folder authorization, transfers and capture. The frontend does not host a WebView or implement a second protocol stack.

Build on Windows x64 with the .NET 8 SDK, Rust MSVC toolchain and Windows SDK:

```powershell
python scripts/build_windows.py --smoke-test
```

The unsigned local build is in `dist/YM_Link/YM_Link.exe`. It bundles the .NET runtime by default. `--debug` shortens local Rust rebuilds; `--framework-dependent` uses the installed .NET 8 Windows Desktop Runtime. This is a local development folder, not a signed installer or a distribution release. Only the built and tested architecture/OS has evidence; this does not establish ARM64 or older Windows compatibility.

The window's X closes it to the notification area while connections, transfers and mirroring continue. Use the tray menu's **Open YM_Link**, or left-double-click its icon, to restore the same window; **Exit** stops the backend and quits. The ordinary minimize button retains its Windows behavior.

Run the dependency-free frontend checks separately:

```powershell
dotnet run --project windows/Tests/YM_Link.Tests.csproj
```

`YM_Link.exe --smoke-test` verifies the published window icon and constructs/disposes invisible tray resources, then starts the real Rust backend, waits briefly for listening ports and a snapshot, destroys the backend and exits. It does not show a tray icon, connect a device or begin capture. Exit codes: 0 success, 1 startup failure, 2 snapshot not ready, 3 native/read failure, 4 another instance is running, 5 watchdog timeout. The executable watchdog bounds a stuck native call at 20 seconds; the build script also imposes a 30-second process timeout.

State is stored under the current user's Local Application Data in `Youmu/YM_Link`; the language preference is `ui-settings.json`. Windows device identity protection is provided by the Rust backend. A per-user global mutex spans console/RDP sessions and prevents two app instances (including smoke tests) from concurrently mutating this state. Choose **Exit** from the tray before running its smoke test; X leaves that instance running. Existing pairing identities and authorized folders are preserved.

The interface follows macOS's device sidebar and screen/file/transfer workspace. Every operation submits the existing Rust command. Polling retains row identity, text drafts and local screen/quality/audio choices; active mirror state follows Rust. Known native errors are translated into English/Simplified Chinese and unknown errors use a fixed safe message rather than exposing transport details or local paths. Explicit **Exit** cancels polling and waits for the serialized native queue before releasing its SafeHandle; hiding the window leaves that queue alive.

Smoke tests prove local lifecycle, not Quest interoperability, LAN connectivity or hardware capture quality. Real Quest pairing, transfers and mirror playback require the corresponding physical devices and acceptance evidence.
