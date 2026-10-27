import SwiftUI

private enum Page: String, CaseIterable {
    case mirror, shares, transfers
    var symbol: String {
        switch self {
        case .mirror: "rectangle.on.rectangle"
        case .shares: "folder"
        case .transfers: "arrow.down.circle"
        }
    }
}

struct ContentView: View {
    @EnvironmentObject private var model: DesktopModel
    @State private var page: Page = .mirror
    @State private var selectedDisplay: UInt32?
    @State private var audio = true
    @State private var quality = "balanced"

    var body: some View {
        NavigationSplitView {
            sidebar
                .navigationSplitViewColumnWidth(min: 220, ideal: 245, max: 290)
        } detail: {
            VStack(spacing: 0) {
                header
                Divider()
                ScrollView {
                    VStack(alignment: .leading, spacing: 22) {
                        if !model.isConnected { connectionNotice }
                        switch page {
                        case .mirror: mirrorPage
                        case .shares: sharesPage
                        case .transfers: transfersPage
                        }
                    }
                    .padding(28)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                Divider()
                footer
            }
            .background(Color(nsColor: .windowBackgroundColor))
        }
        .navigationTitle("YM_Link")
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button { model.command("rescan") } label: {
                    Label(model.l("rescan"), systemImage: "arrow.clockwise")
                }
                .help(model.l("rescan"))
                Button { model.showAddress = true } label: {
                    Label(model.l("connect_address"), systemImage: "plus")
                }
                .help(model.l("connect_address"))
            }
        }
        .sheet(isPresented: Binding(
            get: { model.snapshot.pairing != nil || model.showAddress },
            set: {
                if !$0 {
                    model.showAddress = false
                    if model.snapshot.pairing != nil { model.command("cancel_pairing") }
                }
            }
        )) {
            if model.snapshot.pairing != nil { pairingSheet } else { addressSheet }
        }
        .alert(model.l("action_failed"), isPresented: Binding(
            get: { model.error != nil },
            set: { if !$0 { model.error = nil } }
        )) {
            Button(model.l("ok")) { model.error = nil }
        } message: {
            Text(model.error ?? "")
        }
        .onChange(of: model.snapshot.displays.map(\.id), initial: true) { _, ids in
            if model.isMirrorActive, let active = model.snapshot.mirror.displayId, ids.contains(active) {
                selectedDisplay = active
            } else if selectedDisplay == nil || !ids.contains(selectedDisplay!) {
                selectedDisplay = ids.first
            }
        }
        .onChange(of: model.snapshot.mirror.displayId, initial: true) { _, id in
            if let id { selectedDisplay = id }
        }
        .onChange(of: model.snapshot.mirror.status, initial: true) { _, _ in
            if model.isMirrorActive {
                audio = model.snapshot.mirror.audio
                quality = model.snapshot.mirror.quality
            }
        }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(spacing: 10) {
                Image(systemName: "link.circle.fill")
                    .font(.system(size: 30))
                    .foregroundStyle(Color.accentColor)
                VStack(alignment: .leading, spacing: 3) {
                    Text("YM_Link").font(.headline)
                    Text(model.snapshot.deviceName.isEmpty ? model.l("this_mac") : model.snapshot.deviceName)
                        .font(.caption).foregroundStyle(.secondary).lineLimit(1)
                }
            }
            .padding(.horizontal, 18).padding(.vertical, 22)

            HStack {
                Text(model.l("devices")).font(.caption.weight(.semibold)).foregroundStyle(.secondary)
                Spacer()
                Text("\(model.snapshot.peers.count)").font(.caption).foregroundStyle(.tertiary)
            }
            .padding(.horizontal, 18).padding(.bottom, 10)

            ScrollView {
                VStack(spacing: 10) {
                    if model.snapshot.peers.isEmpty {
                        VStack(alignment: .leading, spacing: 10) {
                            HStack(spacing: 8) {
                                Image(systemName: "dot.radiowaves.left.and.right")
                                Text(model.l("searching_devices"))
                            }
                            .font(.subheadline)
                            Text(model.l("same_network_hint"))
                                .font(.caption).foregroundStyle(.secondary).fixedSize(horizontal: false, vertical: true)
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(12)
                    }
                    ForEach(model.snapshot.peers) { peer in deviceCard(peer) }
                }
                .padding(.horizontal, 12)
            }
            Spacer(minLength: 12)
            Divider()
            HStack(spacing: 7) {
                Image(systemName: "lock.shield")
                Text(model.l("local_private"))
            }
            .font(.caption).foregroundStyle(.secondary).padding(18)
        }
        .background(.regularMaterial)
    }

    private func deviceCard(_ peer: Peer) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top, spacing: 8) {
                Image(systemName: peer.platform.lowercased().contains("mac") ? "laptopcomputer" : "visionpro")
                    .font(.title3).foregroundStyle(.secondary)
                VStack(alignment: .leading, spacing: 4) {
                    Text(peer.name.isEmpty ? model.l("nearby_device") : peer.name)
                        .font(.subheadline.weight(.semibold)).lineLimit(2)
                    Text(peer.address).font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                }
                Spacer(minLength: 0)
                if peer.trusted {
                    Image(systemName: "checkmark.shield.fill").font(.caption).foregroundStyle(.green)
                        .help(model.l("trusted"))
                }
            }
            HStack {
                statusPill(peer.status)
                Spacer(minLength: 3)
                if ["connected", "paired"].contains(peer.status) {
                    Button(model.l("disconnect")) { model.command("disconnect", ["peer_id": peer.id]) }
                        .buttonStyle(.borderless)
                } else {
                    Button(model.l("connect")) { model.command("connect", ["peer_id": peer.id]) }
                        .buttonStyle(.bordered)
                        .disabled(["connecting", "pairing"].contains(peer.status))
                }
            }
            .font(.caption)
        }
        .padding(12)
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(.quaternary, lineWidth: 1))
        .contextMenu {
            Button(model.l("connect")) { model.command("connect", ["peer_id": peer.id]) }
            Button(model.l("disconnect")) { model.command("disconnect", ["peer_id": peer.id]) }
            if peer.trusted {
                Divider()
                Button(model.l("forget_device"), role: .destructive) { model.command("forget", ["peer_id": peer.id]) }
            }
        }
    }

    private var header: some View {
        HStack(alignment: .center) {
            VStack(alignment: .leading, spacing: 5) {
                Text(model.l(page.rawValue)).font(.title2.weight(.semibold))
                Text(model.l(page.rawValue + "_subtitle")).font(.subheadline).foregroundStyle(.secondary)
            }
            Spacer()
            Picker(model.l("workspace"), selection: $page) {
                ForEach(Page.allCases, id: \.self) { item in
                    Label(model.l(item.rawValue), systemImage: item.symbol).tag(item)
                }
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 310)
        }
        .padding(.horizontal, 28).padding(.vertical, 22)
    }

    private var connectionNotice: some View {
        HStack(alignment: .top, spacing: 12) {
            Image(systemName: "wifi").font(.title3).foregroundStyle(.secondary)
            VStack(alignment: .leading, spacing: 4) {
                Text(model.l(model.snapshot.peers.isEmpty ? "waiting_headset" : "choose_device"))
                    .font(.subheadline.weight(.medium))
                Text(model.l("connection_hint")).font(.caption).foregroundStyle(.secondary)
            }
            Spacer()
        }
        .padding(15)
        .background(.quaternary.opacity(0.35), in: RoundedRectangle(cornerRadius: 10))
    }

    private var mirrorPage: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack {
                Text(model.l("choose_display")).font(.headline)
                Spacer()
                Button { model.command("refresh_displays") } label: {
                    Label(model.l("refresh"), systemImage: "arrow.clockwise")
                }
                .buttonStyle(.borderless)
            }
            if model.snapshot.displays.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "display").font(.system(size: 40, weight: .light)).foregroundStyle(.secondary)
                    Text(model.l("no_displays")).font(.subheadline)
                    Text(model.l("display_permission_hint"))
                        .font(.caption).foregroundStyle(.secondary).multilineTextAlignment(.center)
                }
                .frame(maxWidth: .infinity).padding(.vertical, 38)
            } else {
                LazyVGrid(columns: [GridItem(.adaptive(minimum: 210), spacing: 16)], spacing: 16) {
                    ForEach(model.snapshot.displays) { display in
                        Button {
                            if model.isMirrorActive {
                                if selectedDisplay != display.id {
                                    model.command("select_mirror_display", ["display_id": display.id])
                                }
                            } else {
                                selectedDisplay = display.id
                            }
                        } label: {
                            VStack(alignment: .leading, spacing: 12) {
                                ZStack {
                                    RoundedRectangle(cornerRadius: 6).fill(Color.accentColor.opacity(0.055))
                                    Image(systemName: "display").font(.system(size: 42, weight: .ultraLight))
                                        .foregroundStyle(selectedDisplay == display.id ? Color.accentColor : .secondary)
                                    VStack { HStack { Spacer(); Image(systemName: selectedDisplay == display.id ? "checkmark.circle.fill" : "circle")
                                        .foregroundStyle(selectedDisplay == display.id ? Color.accentColor : .secondary) }; Spacer() }
                                        .padding(10)
                                }
                                .frame(height: 106)
                                VStack(alignment: .leading, spacing: 3) {
                                    Text(model.displayName(display)).font(.subheadline.weight(.semibold))
                                    Text("\(display.width) × \(display.height)").font(.caption).foregroundStyle(.secondary)
                                }
                            }
                            .padding(12)
                            .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 10))
                            .overlay(RoundedRectangle(cornerRadius: 10).stroke(
                                selectedDisplay == display.id ? Color.accentColor : Color(nsColor: .separatorColor),
                                lineWidth: selectedDisplay == display.id ? 2 : 1
                            ))
                        }
                        .buttonStyle(.plain)
                        .disabled(model.pendingActions.contains("select_mirror_display"))
                        .accessibilityLabel("\(model.displayName(display)), \(display.width) × \(display.height)")
                        .accessibilityAddTraits(selectedDisplay == display.id ? .isSelected : [])
                    }
                }
            }

            Divider()
            HStack(alignment: .top, spacing: 30) {
                VStack(alignment: .leading, spacing: 10) {
                    Text(model.l("system_audio")).font(.subheadline.weight(.medium))
                    Toggle(model.l("share_audio"), isOn: $audio).toggleStyle(.switch)
                        .disabled(model.isMirrorActive)
                }
                VStack(alignment: .leading, spacing: 10) {
                    Text(model.l("quality")).font(.subheadline.weight(.medium))
                    Picker(model.l("quality"), selection: $quality) {
                        Text(model.l("quality_smooth")).tag("smooth")
                        Text(model.l("quality_balanced")).tag("balanced")
                        Text(model.l("quality_quality")).tag("quality")
                    }
                    .labelsHidden().frame(width: 180).disabled(model.isMirrorActive)
                }
                Spacer()
            }
            HStack(spacing: 12) {
                if model.isMirrorActive {
                    if model.snapshot.mirror.status == "paused" {
                        Button { model.command("resume_mirror") } label: {
                            Label(model.l("resume_mirror"), systemImage: "play.fill")
                        }
                        .buttonStyle(.borderedProminent).controlSize(.large)
                        .disabled(!model.isConnected || model.pendingActions.contains("resume_mirror"))
                    } else {
                        Button { model.command("pause_mirror") } label: {
                            Label(model.l("pause_mirror"), systemImage: "pause.fill")
                        }
                        .controlSize(.large)
                        .disabled(model.snapshot.mirror.status != "running" || model.pendingActions.contains("pause_mirror"))
                    }
                    Button(role: .destructive) { model.command("stop_mirror") } label: {
                        Label(model.l("stop_mirror"), systemImage: "stop.fill")
                    }
                    .controlSize(.large)
                } else {
                    Button {
                        if let id = selectedDisplay {
                            model.command("start_mirror", ["display_id": id, "audio": audio, "quality": quality])
                        }
                    } label: {
                        Label(model.l("start_mirror"), systemImage: "play.fill")
                    }
                    .buttonStyle(.borderedProminent).controlSize(.large)
                    .disabled(!model.isConnected || selectedDisplay == nil)
                }
                Text(model.l(model.snapshot.mirror.status == "paused" ? "mirror_paused_hint" : (model.snapshot.mirror.status == "starting" ? "mirror_starting_hint" : (model.isMirroring ? "mirror_live_hint" : "mirror_start_hint"))))
                    .font(.caption).foregroundStyle(.secondary)
            }
            if let error = model.snapshot.mirror.error {
                Label(model.backendMessage(error), systemImage: "exclamationmark.triangle").font(.caption).foregroundStyle(.orange)
            }
            if model.snapshot.mirror.permission != "granted" {
                HStack(spacing: 10) {
                    Image(systemName: "hand.raised").foregroundStyle(.secondary)
                    Text(model.l("screen_permission")).font(.caption).foregroundStyle(.secondary)
                    Spacer()
                    Button(model.l("open_settings")) { model.openScreenSettings() }.font(.caption)
                }
                .padding(12).background(.quaternary.opacity(0.35), in: RoundedRectangle(cornerRadius: 8))
            }
        }
    }

    private var sharesPage: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                Text(model.l("authorized_folders")).font(.headline)
                Spacer()
                Button { model.chooseShare() } label: {
                    Label(model.l("add_share"), systemImage: "folder.badge.plus")
                }
                .buttonStyle(.borderedProminent)
            }
            Text(model.l("share_boundary")).font(.subheadline).foregroundStyle(.secondary)
            if model.snapshot.shares.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "folder.badge.plus").font(.system(size: 42, weight: .light)).foregroundStyle(.secondary)
                    Text(model.l("no_shares")).font(.headline)
                    Text(model.l("no_shares_hint")).font(.subheadline).foregroundStyle(.secondary)
                    Button(model.l("add_share")) { model.chooseShare() }.padding(.top, 6)
                }
                .frame(maxWidth: .infinity).padding(.vertical, 52)
            } else {
                ForEach(model.snapshot.shares) { share in
                    HStack(spacing: 14) {
                        Image(systemName: "folder.fill").font(.title2).foregroundStyle(Color.accentColor.opacity(0.8))
                        VStack(alignment: .leading, spacing: 4) {
                            Text(share.name).font(.subheadline.weight(.semibold))
                            Text(share.path).font(.caption).foregroundStyle(.secondary).textSelection(.enabled)
                        }
                        Spacer()
                        Button(model.l("remove"), role: .destructive) { model.command("remove_share", ["id": share.id]) }
                            .buttonStyle(.borderless)
                    }
                    .padding(16)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 10))
                    .overlay(RoundedRectangle(cornerRadius: 10).stroke(.quaternary, lineWidth: 1))
                }
            }
        }
    }

    private var transfersPage: some View {
        VStack(alignment: .leading, spacing: 16) {
            if model.snapshot.transfers.isEmpty {
                VStack(spacing: 12) {
                    Image(systemName: "arrow.down.circle").font(.system(size: 42, weight: .light)).foregroundStyle(.secondary)
                    Text(model.l("no_transfers")).font(.headline)
                    Text(model.l("no_transfers_hint")).font(.subheadline).foregroundStyle(.secondary)
                }
                .frame(maxWidth: .infinity).padding(.vertical, 72)
            } else {
                ForEach(model.snapshot.transfers) { transfer in
                    VStack(alignment: .leading, spacing: 10) {
                        HStack {
                            Image(systemName: "film").foregroundStyle(.secondary)
                            Text(transfer.name).font(.subheadline.weight(.medium)).lineLimit(1)
                            Spacer()
                            statusPill(transfer.status)
                        }
                        ProgressView(value: transfer.fraction)
                        HStack {
                            Text("\(ByteCountFormatter.string(fromByteCount: Int64(clamping: transfer.transferred), countStyle: .file)) / \(ByteCountFormatter.string(fromByteCount: Int64(clamping: transfer.total), countStyle: .file))")
                                .font(.caption).foregroundStyle(.secondary).monospacedDigit()
                            Spacer()
                            if ["transferring", "running"].contains(transfer.status) {
                                Button(model.l("pause")) { model.command("pause_transfer", ["id": transfer.id]) }
                            } else if transfer.status == "paused" {
                                Button(model.l("resume")) { model.command("resume_transfer", ["id": transfer.id]) }
                            }
                            if !["completed", "cancelled", "error", "failed"].contains(transfer.status) {
                                Button(model.l("cancel"), role: .destructive) { model.command("cancel_transfer", ["id": transfer.id]) }
                            }
                        }
                    }
                    .padding(16)
                    .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 10))
                }
            }
        }
    }

    private var footer: some View {
        HStack(spacing: 7) {
            Circle().fill(model.isConnected ? .green : .secondary).frame(width: 6, height: 6)
            Text(model.status(model.snapshot.status))
            if model.isMirrorActive {
                Divider().frame(height: 11)
                if model.snapshot.mirror.status == "paused" {
                    statusPill("paused")
                } else {
                    Text(model.status(model.snapshot.mirror.status))
                    if let fps = model.snapshot.mirror.fps { Text(String(format: "%.0f fps", fps)) }
                    if let bitrate = model.snapshot.mirror.bitrateMbps { Text(String(format: "%.1f Mbps", bitrate)) }
                    if let latency = model.snapshot.mirror.latencyMs { Text(String(format: "%.0f ms", latency)) }
                }
            }
            Spacer()
            Text(model.l("local_network"))
        }
        .font(.caption).foregroundStyle(.secondary)
        .padding(.horizontal, 22).padding(.vertical, 11)
    }

    private func statusPill(_ value: String) -> some View {
        Text(model.status(value)).font(.caption2.weight(.medium))
            .foregroundStyle(["connected", "paired", "completed", "running"].contains(value) ? Color.green : .secondary)
            .padding(.horizontal, 7).padding(.vertical, 3)
            .background(.quaternary.opacity(0.6), in: Capsule())
    }

    private var addressSheet: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text(model.l("connect_address")).font(.title2.weight(.semibold))
            Text(model.l("address_hint")).font(.subheadline).foregroundStyle(.secondary)
            TextField("192.168.1.10:47620", text: $model.addressDraft)
                .textFieldStyle(.roundedBorder).onSubmit { model.connectAddress() }
                .accessibilityLabel(model.l("device_address"))
            HStack {
                Button(model.l("cancel")) { model.showAddress = false }.keyboardShortcut(.cancelAction)
                Spacer()
                Button(model.l("connect")) { model.connectAddress() }
                    .buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
                    .disabled(model.addressDraft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(28).frame(width: 440)
    }

    @ViewBuilder private var pairingSheet: some View {
        if let pairing = model.snapshot.pairing {
            VStack(spacing: 18) {
                Image(systemName: "lock.shield").font(.system(size: 36, weight: .light)).foregroundStyle(Color.accentColor)
                VStack(spacing: 8) {
                    Text(model.l("pair_device")).font(.title2.weight(.semibold))
                    Text(pairing.peerName).font(.subheadline).foregroundStyle(.secondary)
                }
                Text(model.l(pairing.expiresIn == 0 ? "pairing_expired" : (pairing.mode == "enter" ? "enter_pairing_hint" : "display_pairing_hint")))
                    .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
                if pairing.mode == "enter" {
                    TextField("000000", text: $model.pairingDraft)
                        .font(.system(size: 32, weight: .medium, design: .monospaced))
                        .multilineTextAlignment(.center).textFieldStyle(.roundedBorder)
                        .accessibilityLabel(model.l("pairing_code"))
                        .onSubmit { model.command("pair", ["code": model.pairingDraft]) }
                        .onChange(of: model.pairingDraft) { _, value in
                            model.pairingDraft = String(value.filter { $0 >= "0" && $0 <= "9" }.prefix(6))
                        }
                } else {
                    Text(pairing.expiresIn == 0 ? "— — — — — —" : (pairing.code ?? "— — — — — —"))
                        .font(.system(size: 38, weight: .medium, design: .monospaced)).tracking(7)
                        .textSelection(.enabled).padding(.vertical, 14).frame(maxWidth: .infinity)
                        .background(.quaternary.opacity(0.4), in: RoundedRectangle(cornerRadius: 10))
                        .accessibilityLabel(model.l("pairing_code") + ": " + (pairing.code ?? ""))
                }
                Text(String(format: model.l("pairing_expiry"), pairing.expiresIn / 60, pairing.expiresIn % 60))
                    .font(.caption).foregroundStyle(.secondary).monospacedDigit()
                HStack {
                    Button(model.l("cancel")) { model.command("cancel_pairing") }.keyboardShortcut(.cancelAction)
                    Spacer()
                    if pairing.mode == "enter" {
                        Button(model.l("confirm")) { model.command("pair", ["code": model.pairingDraft]) }
                            .buttonStyle(.borderedProminent).keyboardShortcut(.defaultAction)
                            .disabled(model.pairingDraft.count != 6 || pairing.expiresIn == 0)
                    } else {
                        Text(model.l("waiting_confirmation")).font(.caption).foregroundStyle(.secondary)
                    }
                }
            }
            .padding(30).frame(width: 430)
            .interactiveDismissDisabled()
        }
    }
}
