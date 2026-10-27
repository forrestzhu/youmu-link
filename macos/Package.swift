// swift-tools-version: 5.9
import PackageDescription
import Foundation

let rustLibraryDirectory = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent().deletingLastPathComponent()
    .appendingPathComponent("target/release").path

let package = Package(
    name: "YM_Link",
    defaultLocalization: "en",
    platforms: [.macOS(.v14)],
    products: [.executable(name: "YM_Link", targets: ["YM_Link"])],
    targets: [
        .systemLibrary(name: "CYMLink", path: "Sources/CYMLink"),
        .executableTarget(
            name: "YM_Link",
            dependencies: ["CYMLink"],
            resources: [.process("Resources")],
            linkerSettings: [
                .unsafeFlags(["-L", rustLibraryDirectory]),
                .linkedLibrary("ymlink_desktop"),
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration"),
                .linkedFramework("ScreenCaptureKit"),
                .linkedFramework("VideoToolbox"),
                .linkedFramework("CoreMedia"),
                .linkedFramework("CoreVideo"),
                .linkedFramework("CoreGraphics"),
            ]
        ),
        .testTarget(name: "YM_LinkTests", dependencies: ["YM_Link"]),
    ]
)
