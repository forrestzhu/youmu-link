"""Host-independent checks for the macOS developer bundle's runtime dependencies."""

import unittest

import build_macos


class MacRuntimeDependencies(unittest.TestCase):
    def test_accepts_system_libraries_and_rejects_build_tree_dylib(self):
        system_libraries = ["/usr/lib/libSystem.B.dylib", "/System/Library/Frameworks/AppKit.framework/Versions/C/AppKit",
                            "@rpath/libswiftCore.dylib"]
        build_macos.verify_runtime_dependencies(system_libraries)
        for dependency in ("/tmp/checkout/target/release/libymlink_desktop.dylib", "@rpath/libymlink_desktop.dylib"):
            with self.subTest(dependency=dependency), self.assertRaisesRegex(ValueError, "non-system"):
                build_macos.verify_runtime_dependencies([*system_libraries, dependency])


if __name__ == "__main__":
    unittest.main()
