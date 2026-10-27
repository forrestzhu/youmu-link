fn main() {
    // ScreenCaptureKit's Swift bridge needs the system Swift runtime when linked
    // into Rust tests and the protocol test peer. SwiftUI apps already add it.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}
