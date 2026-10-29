import XCTest
@testable import YM_Link

final class ModelsTests: XCTestCase {
    func testExplicitLanguageIgnoresHostPreferenceAndLocaleFolderCase() throws {
        let resources = try XCTUnwrap(Localizer.resourceBundle(relativeTo: Bundle(for: Self.self)))
        XCTAssertEqual(Localizer.string("start_mirror", language: "ZH-HANS", in: resources, preferredLanguages: ["en"]), "开始镜像")
        XCTAssertEqual(Localizer.string("start_mirror", language: "en", in: resources, preferredLanguages: ["zh-Hans"]), "Start mirroring")
        XCTAssertEqual(Localizer.string("start_mirror", language: "system", in: resources, preferredLanguages: ["zh-Hans-CN", "en"]), "开始镜像")
        XCTAssertEqual(Localizer.string("start_mirror", language: "system", in: resources, preferredLanguages: ["fr-FR"]), "Start mirroring")
        XCTAssertEqual(Localizer.string("unknown-localization-key", language: "zh-Hans", in: resources), "unknown-localization-key")
    }

    func testLocalizationResourcesTravelWithRelocatedApp() throws {
        let files = FileManager.default
        let temporary = files.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try files.createDirectory(at: temporary, withIntermediateDirectories: true)
        defer { try? files.removeItem(at: temporary) }
        let source = try XCTUnwrap(Localizer.resourceBundle(relativeTo: Bundle(for: Self.self)))
        let original = temporary.appendingPathComponent("original/YM_Link.app")
        let resources = original.appendingPathComponent("Contents/Resources")
        try files.createDirectory(at: resources, withIntermediateDirectories: true)
        let info: [String: String] = ["CFBundleIdentifier": "app.youmu.link.test", "CFBundlePackageType": "APPL", "CFBundleExecutable": "YM_Link"]
        let data = try PropertyListSerialization.data(fromPropertyList: info, format: .xml, options: 0)
        try data.write(to: original.appendingPathComponent("Contents/Info.plist"))
        try files.copyItem(at: source.bundleURL, to: resources.appendingPathComponent(Localizer.resourceBundleName))
        let relocated = temporary.appendingPathComponent("relocated/YM_Link.app")
        try files.createDirectory(at: relocated.deletingLastPathComponent(), withIntermediateDirectories: true)
        try files.moveItem(at: original, to: relocated)
        XCTAssertFalse(files.fileExists(atPath: original.path))
        let host = try XCTUnwrap(Bundle(url: relocated))
        let moved = try XCTUnwrap(Localizer.resourceBundle(relativeTo: host))
        XCTAssertEqual(moved.bundleURL.resolvingSymlinksInPath().standardizedFileURL.path, relocated.appendingPathComponent("Contents/Resources/" + Localizer.resourceBundleName).resolvingSymlinksInPath().standardizedFileURL.path)
        XCTAssertEqual(Localizer.string("start_mirror", language: "en", in: moved), "Start mirroring")
        XCTAssertEqual(Localizer.string("start_mirror", language: "zh-Hans", in: moved), "开始镜像")
    }

    func testRustSnapshotContractAndLocalizedControls() throws {
        let json = #"""
        {
          "device_name":"Test Mac","status":"connected","listen_port":47620,"discovery_port":47610,
          "peers":[{"id":"peer-1","address":"192.168.1.2:47620","name":"Test headset","platform":"android","status":"connected","trusted":true}],
          "pairing":{"peer_name":"Test headset","code":"0427","expires_in":120,"mode":"display","window_id":"window-1","attempt":4,"stage":"ready","error":null,"error_revision":0,"retry_after_ms":0},
          "displays":[{"id":1,"name":"Display","width":1920,"height":1080}],
          "mirror":{"status":"running","permission":"granted","display_id":1,"audio":true,"quality":"balanced","fps":30.0,"bitrate_mbps":12.5,"latency_ms":null,"error":null},
          "shares":[{"id":"share-1","name":"Films","path":"/test/Films"}],
          "transfers":[{"id":"transfer-1","name":"Film.mp4","transferred":64,"total":128,"status":"transferring"}],
          "error_id":0,"error":null
        }
        """#
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let snapshot = try decoder.decode(Snapshot.self, from: Data(json.utf8))
        XCTAssertEqual(snapshot.deviceName, "Test Mac")
        XCTAssertEqual(snapshot.errorId, 0)
        XCTAssertTrue(try XCTUnwrap(snapshot.peers.first).trusted)
        XCTAssertEqual(snapshot.pairing?.expiresIn, 120)
        XCTAssertEqual(snapshot.mirror.displayId, 1)
        XCTAssertEqual(snapshot.mirror.bitrateMbps, 12.5)
        XCTAssertNil(snapshot.mirror.latencyMs)
        XCTAssertEqual(snapshot.transfers.first?.fraction, 0.5)
        XCTAssertEqual(Localizer.string("start_mirror", language: "en"), "Start mirroring")
        XCTAssertEqual(Localizer.string("start_mirror", language: "zh-Hans"), "开始镜像")
        XCTAssertEqual(Localizer.backendMessage("pairing code rejected", language: "zh-Hans"), "配对码不正确。")
        XCTAssertEqual(Localizer.backendMessage("connection timed out", language: "en"), "The connection timed out. Check that both devices are on the same network and try again.")
        XCTAssertEqual(Localizer.backendMessage("unexpected failure at /private/test/file", language: "zh-Hans"), Localizer.string("command_failed", language: "zh-Hans"))
        XCTAssertEqual(Localizer.backendMessage("mirror is not active", language: "zh-Hans"), "当前没有活动的屏幕镜像，请先在这台 Mac 上开始镜像。")
        for message in ["start mirroring on the Mac before remote resume", "start mirroring on the Mac before remote display selection", "first mirror start requires local confirmation"] {
            XCTAssertEqual(Localizer.backendMessage(message, language: "en"), "Start mirroring on this Mac first. Remote controls become available after local confirmation.")
        }
        XCTAssertEqual(Localizer.backendMessage("mirror command timed out", language: "en"), "The mirror command timed out. Check the connection and try again.")
        XCTAssertEqual(Localizer.backendMessage("connection no longer active", language: "zh-Hans"), Localizer.string("error_connection", language: "zh-Hans"))
        XCTAssertEqual(Localizer.backendMessage("incompatible application version; upgrade YM_Link on both devices", language: "zh-Hans"), "两端版本不兼容，请更新两台设备上的 YM_Link 后重新连接。")
        XCTAssertEqual(Localizer.backendMessage("protocol error: incompatible application version; upgrade YM_Link on both devices", language: "zh-Hans"), Localizer.string("error_version_incompatible", language: "zh-Hans"))
        XCTAssertEqual(Localizer.backendMessage("connection busy; try starting mirror again", language: "en"), "The connection is busy. Try starting mirroring again shortly.")
        XCTAssertEqual(Localizer.backendMessage("connection too slow for screen mirroring; turn off system audio or reconnect", language: "zh-Hans"), "连接速度不足，无法继续镜像。请关闭系统音频或重新连接。")
        XCTAssertEqual(Localizer.backendMessage("connection too slow for screen mirroring; turn off system audio or reconnect", language: "en"), "The connection is too slow to continue mirroring. Turn off system audio or reconnect.")

        let pausedJSON = json.replacingOccurrences(of: #""mirror":{"status":"running""#, with: #""mirror":{"status":"paused""#)
        let paused = try decoder.decode(Snapshot.self, from: Data(pausedJSON.utf8)).mirror
        XCTAssertEqual(paused.status, "paused")
        XCTAssertTrue(paused.isActive)
        XCTAssertFalse(paused.isMirroring)
        XCTAssertEqual(paused.displayId, 1)
        XCTAssertTrue(paused.audio)
        XCTAssertEqual(paused.quality, "balanced")
        XCTAssertEqual(Localizer.string("pause_mirror", language: "zh-Hans"), "暂停镜像")
        XCTAssertEqual(Localizer.string("resume_mirror", language: "en"), "Resume mirroring")
        var stopped = paused
        stopped.status = "stopped"
        XCTAssertFalse(stopped.isActive)
        XCTAssertFalse(stopped.isMirroring)
    }

    func testBackendErrorsAreDeliveredOncePerEventEvenWhenTextRepeats() {
        var cursor = SnapshotErrorCursor()
        var snapshot = Snapshot.empty
        XCTAssertNil(cursor.consume(snapshot))
        snapshot.error = "folder is already shared"
        snapshot.errorId = 1
        XCTAssertEqual(cursor.consume(snapshot), snapshot.error)
        XCTAssertNil(cursor.consume(snapshot))
        snapshot.errorId = 2
        XCTAssertEqual(cursor.consume(snapshot), snapshot.error)
        XCTAssertNil(cursor.consume(snapshot))
        snapshot.error = nil
        XCTAssertNil(cursor.consume(snapshot))
        snapshot.error = "folder is already shared"
        XCTAssertNil(cursor.consume(snapshot))
        snapshot.error = nil
        snapshot.errorId = 3
        XCTAssertNil(cursor.consume(snapshot))
        snapshot.error = "folder is already shared"
        XCTAssertNil(cursor.consume(snapshot))
        snapshot.errorId = 4
        XCTAssertEqual(cursor.consume(snapshot), snapshot.error)
    }
    func testNonAsciiGraphemesCannotConsumeTheReadyAttemptsSubmission() {
        let pairing = Pairing(peerName: "Synthetic PC", code: nil, expiresIn: 120, mode: "enter", windowId: "unicode-window", attempt: 1, stage: "ready", error: nil, errorRevision: 0, retryAfterMs: 0)
        for malformed in ["0\u{0301}427", "0\u{FE0F}427", "０４２７"] {
            var input = PairingInput()
            _ = input.apply(pairing)
            XCTAssertNil(input.edit(malformed, pairing: pairing))
            XCTAssertEqual(input.edit("0427", pairing: pairing), "0427", "rejected graphemes must leave the same attempt available")
        }
    }
    func testFourDigitInputSubmitsOnceAndWrongCodeClearsWithinItsLogicalWindow() {
        var pairing = Pairing(peerName: "Synthetic PC", code: nil, expiresIn: 120, mode: "enter", windowId: "window-1", attempt: 1, stage: "ready", error: nil, errorRevision: 0, retryAfterMs: 0)
        var input = PairingInput()
        XCTAssertFalse(input.apply(pairing))
        XCTAssertNil(input.edit("042", pairing: pairing))
        XCTAssertEqual(input.edit("0427", pairing: pairing), "0427")
        XCTAssertNil(input.edit("9999", pairing: pairing))
        pairing.stage = "incorrect"; pairing.error = "incorrect"; pairing.errorRevision = 1
        XCTAssertTrue(input.apply(pairing)); XCTAssertTrue(input.feedback)
        pairing.stage = "ready"; pairing.attempt = 2
        XCTAssertFalse(input.apply(pairing), "same rejection must not shake again on TCP retry")
        input.clearRejected(window: "window-1", revision: input.rejectionCount)
        XCTAssertEqual(input.text, ""); XCTAssertTrue(input.errorVisible)
        XCTAssertNil(input.edit("０４２７", pairing: pairing)); XCTAssertEqual(input.text, "")
        XCTAssertEqual(input.edit("0427", pairing: pairing), "0427"); XCTAssertFalse(input.errorVisible)
        pairing.windowId = "window-2"; pairing.attempt = 3; pairing.error = nil; pairing.errorRevision = 0
        XCTAssertFalse(input.apply(pairing)); XCTAssertNil(input.edit("4", pairing: pairing))
        input.clearRejected(window: "window-1", revision: 1)
        XCTAssertEqual(input.text, "4", "late clearing must not erase a new window")
        XCTAssertFalse(input.apply(nil)); XCTAssertEqual(input.text, "")
    }

}
