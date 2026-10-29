"""Host-independent checks for the macOS developer bundle's runtime dependencies."""

import unittest
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

import build_macos


class MacRuntimeDependencies(unittest.TestCase):
    def test_accepts_system_libraries_and_rejects_build_tree_dylib(self):
        system_libraries = ["/usr/lib/libSystem.B.dylib", "/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit",
                            "@rpath/libswiftCore.dylib"]
        build_macos.verify_runtime_dependencies(system_libraries)
        for dependency in ("/tmp/checkout/target/release/libymlink_desktop.dylib", "@rpath/libymlink_desktop.dylib"):
            with self.subTest(dependency=dependency), self.assertRaisesRegex(ValueError, "non-system"):
                build_macos.verify_runtime_dependencies([*system_libraries, dependency])

    @unittest.skipUnless(sys.platform == "darwin" and shutil.which("swift") and shutil.which("xcrun"), "requires macOS Swift and clang")
    def test_changed_native_archive_relinks_without_any_swift_source_change(self):
        with tempfile.TemporaryDirectory(prefix="ymlink-static-relink-") as temporary:
            package = Path(temporary)
            sources = package / "Sources/LinkProbe"
            sources.mkdir(parents=True)
            archive = package / "libprobe.a"
            (package / "Package.swift").write_text('''// swift-tools-version: 5.9
import PackageDescription
let package = Package(name: "LinkProbe", targets: [.executableTarget(name: "LinkProbe", linkerSettings: [.unsafeFlags(["-Xlinker", "%s"])])])
''' % archive, encoding="utf-8")
            (sources / "main.swift").write_text('@_silgen_name("NativeVersion") func nativeVersion() -> Int32\nprint(nativeVersion())\n', encoding="utf-8")
            for version in (1, 2):
                with self.subTest(native_version=version):
                    (package / "probe.c").write_text(f'int NativeVersion(void) {{ return {version}; }}\n', encoding="utf-8")
                    subprocess.run(["xcrun", "clang", "-c", str(package / "probe.c"), "-o", str(package / "probe.o")], check=True, capture_output=True)
                    subprocess.run(["ar", "rcs", str(archive), str(package / "probe.o")], check=True, capture_output=True)
                    binary = build_macos.build_swift_package(package) / "LinkProbe"
                    actual = subprocess.check_output([str(binary)], text=True).strip()
                    self.assertEqual(actual, str(version), "the executable must include the current archive, even when Swift inputs are unchanged")


if __name__ == "__main__":
    unittest.main()
