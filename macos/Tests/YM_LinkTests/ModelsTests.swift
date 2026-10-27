import XCTest
@testable import YM_Link

final class ModelsTests: XCTestCase {
    func testRustSnapshotContractAndLocalizedControls() throws {
        let json = #"""
        {
          "device_name":"Test Mac","status":"connected","listen_port":47620,"discovery_port":47610,
          "peers":[{"id":"peer-1","address":"192.168.1.2:47620","name":"Test headset","platform":"android","status":"connected","trusted":true}],
          "pairing":{"peer_name":"Test headset","code":"123456","expires_in":120,"mode":"display"},
          "displays":[{"id":1,"name":"Display","width":1920,"height":1080}],
          "mirror":{"status":"running","permission":"granted","display_id":1,"audio":true,"quality":"balanced","fps":30.0,"bitrate_mbps":12.5,"latency_ms":null,"error":null},
          "shares":[{"id":"share-1","name":"Films","path":"/test/Films"}],
          "transfers":[{"id":"transfer-1","name":"Film.mp4","transferred":64,"total":128,"status":"transferring"}],
          "error":null
        }
        """#
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let snapshot = try decoder.decode(Snapshot.self, from: Data(json.utf8))
        XCTAssertEqual(snapshot.deviceName, "Test Mac")
        XCTAssertTrue(try XCTUnwrap(snapshot.peers.first).trusted)
        XCTAssertEqual(snapshot.pairing?.expiresIn, 120)
        XCTAssertEqual(snapshot.mirror.displayId, 1)
        XCTAssertEqual(snapshot.mirror.bitrateMbps, 12.5)
        XCTAssertNil(snapshot.mirror.latencyMs)
        XCTAssertEqual(snapshot.transfers.first?.fraction, 0.5)
        XCTAssertEqual(Localizer.string("start_mirror", language: "en"), "Start mirroring")
        XCTAssertEqual(Localizer.string("start_mirror", language: "zh-Hans"), "开始镜像")
        XCTAssertEqual(Localizer.backendMessage("pairing code rejected", language: "zh-Hans"), "配对码不正确，请重新连接后再试。")
        XCTAssertEqual(Localizer.backendMessage("connection timed out", language: "en"), "The connection timed out. Check that both devices are on the same network and try again.")
        XCTAssertEqual(Localizer.backendMessage("unexpected failure at /private/test/file", language: "zh-Hans"), Localizer.string("command_failed", language: "zh-Hans"))
    }
}
