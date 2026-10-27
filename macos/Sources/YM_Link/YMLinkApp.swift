import AppKit
import Darwin
import SwiftUI

@main
struct YMLinkApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate
    @StateObject private var model: DesktopModel

    init() {
        if CommandLine.arguments.contains("--verify-localizations") {
            let expected = Bundle.main.resourceURL?.appendingPathComponent(Localizer.resourceBundleName).resolvingSymlinksInPath().standardizedFileURL.path
            let actual = Localizer.resourceBundle(relativeTo: .main)?.bundleURL.resolvingSymlinksInPath().standardizedFileURL.path
            let valid = expected != nil && actual == expected
                && Localizer.string("start_mirror", language: "en") == "Start mirroring"
                && Localizer.string("start_mirror", language: "zh-Hans") == "开始镜像"
            print(valid ? "PASS: packaged English and Simplified Chinese resources." : "FAIL: packaged localization resources.")
            Darwin.exit(valid ? 0 : 1)
        }
        _model = StateObject(wrappedValue: DesktopModel())
    }

    var body: some Scene {
        WindowGroup("YM_Link") {
            ContentView()
                .environmentObject(model)
                .frame(minWidth: 920, minHeight: 620)
                .onAppear {
                    delegate.model = model
                    NSApp.setActivationPolicy(.regular)
                    NSApp.activate(ignoringOtherApps: true)
                }
        }
        .defaultSize(width: 1060, height: 720)
        .commands {
            CommandGroup(after: .newItem) {
                Button(model.l("rescan")) { model.command("rescan") }
                    .keyboardShortcut("r", modifiers: .command)
                Button(model.l("connect_address")) { model.showAddress = true }
                    .keyboardShortcut("k", modifiers: .command)
            }
        }
        Settings {
            SettingsView().environmentObject(model)
        }
    }
}

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var model: DesktopModel?

    func applicationWillTerminate(_ notification: Notification) { model?.shutdown() }
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }
}

struct SettingsView: View {
    @EnvironmentObject private var model: DesktopModel

    var body: some View {
        Form {
            LabeledContent(model.l("device_name"), value: model.snapshot.deviceName)
            Picker(model.l("language"), selection: $model.language) {
                Text(model.l("system_language")).tag("system")
                Text("简体中文").tag("zh-Hans")
                Text("English").tag("en")
            }
            LabeledContent(model.l("network")) {
                Text("TCP \(model.snapshot.listenPort) · UDP \(model.snapshot.discoveryPort)")
                    .monospacedDigit()
            }
            Text(model.l("privacy_note"))
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .formStyle(.grouped)
        .padding()
        .frame(width: 460)
    }
}
